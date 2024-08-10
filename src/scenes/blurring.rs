use std::f32::consts::PI;
use std::{mem, time::Instant};

use gl::types::{GLenum, GLfloat, GLint, GLsizei, GLsizeiptr, GLuint};
use glam::{ivec2, vec2, IVec2, Mat4, Vec2};
use image::ImageFormat;
use winit::keyboard::{Key, NamedKey, SmolStr};
use winit::{dpi::PhysicalSize, window::Window};

use crate::camera::Camera;

use super::create_shader_program;

const SRC_VERT_QUAD: &[u8] = include_bytes!("shaders/quad.vert");
const SRC_FRAG_TEXTURE: &[u8] = include_bytes!("shaders/texture.frag");

const SRC_VERT_SCREEN: &[u8] = include_bytes!("shaders/screen.vert");
const SRC_FRAG_BLUR: &[u8] = include_bytes!("shaders/blur.frag");

// const GURA_JPG: &[u8] = include_bytes!("../../assets/gura.jpg");
const BIG_SQUARES_PNG: &[u8] = include_bytes!("../../assets/big-squares.png");

pub struct BlurringScene {
    matrix: Mat4,
    viewport: Vec2,

    quad_shader: GLuint,
    quad_vao: GLuint,
    quad_vbo: GLuint,
    quad_ebo: GLuint,

    comp_fbo: GLuint,
    comp_texture: GLuint,
    comp_vao: GLuint,
    comp_vbo: GLuint,
    comp_shader: GLuint,
    blur_shader: GLuint,

    ping_pong_fbo: GLuint,
    ping_pong_texture: GLuint,

    gura_fb_size: IVec2,
    gura_texture: GLuint,

    u_mvp_quad: GLint,
    u_direction: GLint,
    u_kernel_size: GLint,

    kernel_size: i32,
    blur_radius: f32,
    is_kawase: bool,

    indices: Vec<[u32; 6]>,

    last_instant: Instant,
}

impl BlurringScene {
    pub fn new(window: &Window) -> Self {
        let PhysicalSize { width, height } = window.inner_size();
        let viewport = Vec2::new(width as f32, height as f32);

        let (gura, gura_texture) = unsafe {
            // Gura texture
            // let gura = image::load_from_memory_with_format(GURA_JPG, ImageFormat::Jpeg);
            let gura = image::load_from_memory_with_format(BIG_SQUARES_PNG, ImageFormat::Png);
            let gura = gura.unwrap().into_rgba8();

            let mut gura_texture: GLuint = 0;
            gl::GenTextures(1, &mut gura_texture);
            Self::upload_texture(
                gura_texture,
                gura.width(),
                gura.height(),
                gura.as_ptr(),
                gl::CLAMP_TO_BORDER,
            );

            (gura, gura_texture)
        };

        let gura_size = vec2(gura.width() as f32, gura.height() as f32);
        let gura_fb_size = ivec2(gura.width() as i32, gura.height() as i32) / 2;

        // They don't need to be vecs, but I'm too lazy to un-vector them now.
        let mut quads = Vec::with_capacity(1);
        let mut vertices = Vec::with_capacity(1);
        let mut indices = Vec::with_capacity(1);

        let quad = Quad {
            position: Vec2::ZERO,
            size: gura_size,
        };
        vertices.push(quad.vertices());
        indices.push(quad.indices(0));
        quads.push(quad);

        unsafe {
            // Normal blending
            gl::Enable(gl::BLEND);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);

            // quads shader and vertices
            let quad_shader = create_shader_program(SRC_VERT_QUAD, SRC_FRAG_TEXTURE);
            let u_mvp_quad = gl::GetUniformLocation(quad_shader, c"u_mvp".as_ptr());

            let mut quad_vao: GLuint = 0;
            gl::GenVertexArrays(1, &mut quad_vao);
            gl::BindVertexArray(quad_vao);

            let mut quad_vbo: GLuint = 0;
            gl::GenBuffers(1, &mut quad_vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, quad_vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                mem::size_of_val(vertices.as_slice()) as GLsizeiptr,
                vertices.as_slice().as_ptr() as *const _,
                gl::DYNAMIC_DRAW,
            );

            let mut quad_ebo: GLuint = 0;
            gl::GenBuffers(1, &mut quad_ebo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, quad_ebo);
            gl::BufferData(
                gl::ELEMENT_ARRAY_BUFFER,
                mem::size_of_val(indices.as_slice()) as GLsizeiptr,
                indices.as_slice().as_ptr() as *const _,
                gl::STATIC_DRAW,
            );

            let size_vertex = mem::size_of::<Vertex>() as GLsizei;
            let size_f32 = mem::size_of::<f32>() as GLsizei;

            #[rustfmt::skip]
            {
                let a_position = gl::GetAttribLocation(quad_shader, c"position" .as_ptr()) as GLuint;
                let a_uv       = gl::GetAttribLocation(quad_shader, c"uv"       .as_ptr()) as GLuint;

                gl::VertexAttribPointer(a_position, 2, gl::FLOAT, gl::FALSE, size_vertex,  0             as _);
                gl::VertexAttribPointer(a_uv,       2, gl::FLOAT, gl::FALSE, size_vertex, (2 * size_f32) as _);

                gl::EnableVertexAttribArray(a_position as GLuint);
                gl::EnableVertexAttribArray(a_uv       as GLuint);
            };

            // framebuffer and its texture
            let mut comp_fbo: GLuint = 0;
            gl::GenFramebuffers(1, &mut comp_fbo);
            gl::BindFramebuffer(gl::FRAMEBUFFER, comp_fbo);

            let mut comp_texture: GLuint = 0;
            gl::GenTextures(1, &mut comp_texture);
            Self::upload_texture(
                comp_texture,
                gura_fb_size.x as GLuint,
                gura_fb_size.y as GLuint,
                std::ptr::null(),
                gl::CLAMP_TO_EDGE,
            );
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                comp_texture,
                0,
            );

            if gl::CheckFramebufferStatus(gl::FRAMEBUFFER) != gl::FRAMEBUFFER_COMPLETE {
                eprintln!("screen framebuffer not complete");
            }

            // ping-pong framebuffer
            let mut ping_pong_fbo: GLuint = 0;
            gl::GenFramebuffers(1, &mut ping_pong_fbo);
            gl::BindFramebuffer(gl::FRAMEBUFFER, ping_pong_fbo);

            let mut ping_pong_texture: GLuint = 0;
            gl::GenTextures(1, &mut ping_pong_texture);
            Self::upload_texture(
                ping_pong_texture,
                gura_fb_size.x as GLuint,
                gura_fb_size.y as GLuint,
                std::ptr::null(),
                gl::CLAMP_TO_EDGE,
            );
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                ping_pong_texture,
                0,
            );

            if gl::CheckFramebufferStatus(gl::FRAMEBUFFER) != gl::FRAMEBUFFER_COMPLETE {
                eprintln!("ping-pong framebuffer not complete");
            }

            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);

            // compositing shader and vertices
            let comp_shader = create_shader_program(SRC_VERT_SCREEN, SRC_FRAG_TEXTURE);

            #[rustfmt::skip]
            {
                let a_position = gl::GetAttribLocation(comp_shader, c"position" .as_ptr()) as GLuint;
                let a_uv       = gl::GetAttribLocation(comp_shader, c"uv"       .as_ptr()) as GLuint;

                gl::VertexAttribPointer(a_position, 2, gl::FLOAT, gl::FALSE, size_vertex,  0             as _);
                gl::VertexAttribPointer(a_uv,       2, gl::FLOAT, gl::FALSE, size_vertex, (2 * size_f32) as _);

                gl::EnableVertexAttribArray(a_position as GLuint);
                gl::EnableVertexAttribArray(a_uv       as GLuint);
            };

            let mut comp_vao: GLuint = 0;
            gl::GenVertexArrays(1, &mut comp_vao);
            gl::BindVertexArray(comp_vao);

            let mut comp_vbo: GLuint = 0;
            gl::GenBuffers(1, &mut comp_vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, comp_vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                mem::size_of_val(SCREEN_VERTICES) as GLsizeiptr,
                SCREEN_VERTICES.as_ptr() as *const _,
                gl::DYNAMIC_DRAW,
            );

            // blur shader
            let blur_shader = create_shader_program(SRC_VERT_SCREEN, SRC_FRAG_BLUR);
            let u_direction = gl::GetUniformLocation(blur_shader, c"u_direction".as_ptr());
            let u_kernel_size = gl::GetUniformLocation(blur_shader, c"u_kernel_size".as_ptr());

            #[rustfmt::skip]
            {
                let a_position = gl::GetAttribLocation(blur_shader, c"position" .as_ptr()) as GLuint;
                let a_uv       = gl::GetAttribLocation(blur_shader, c"uv"       .as_ptr()) as GLuint;

                gl::VertexAttribPointer(a_position, 2, gl::FLOAT, gl::FALSE, size_vertex,  0             as _);
                gl::VertexAttribPointer(a_uv,       2, gl::FLOAT, gl::FALSE, size_vertex, (2 * size_f32) as _);

                gl::EnableVertexAttribArray(a_position as GLuint);
                gl::EnableVertexAttribArray(a_uv       as GLuint);
            };

            Self {
                matrix: Mat4::default(),
                viewport,

                quad_shader,
                quad_vao,
                quad_vbo,
                quad_ebo,

                comp_fbo,
                comp_texture,
                comp_vao,
                comp_vbo,
                comp_shader,
                blur_shader,

                ping_pong_fbo,
                ping_pong_texture,

                gura_fb_size,
                gura_texture,

                u_mvp_quad,
                u_direction,
                u_kernel_size,

                kernel_size: 16,
                blur_radius: 1.0,
                is_kawase: false,

                indices,

                last_instant: Instant::now(),
            }
        }
    }

    unsafe fn upload_texture(
        texture: GLuint,
        width: u32,
        height: u32,
        data: *const u8,
        clamp: GLenum,
    ) {
        gl::BindTexture(gl::TEXTURE_2D, texture);
        gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            gl::RGBA8 as GLint,
            width as GLsizei,
            height as GLsizei,
            0,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            data as *const _,
        );
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, clamp as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, clamp as GLint);
    }

    pub fn on_key(&mut self, keycode: Key<SmolStr>) {
        match keycode {
            Key::Named(NamedKey::ArrowUp) => {
                self.kernel_size = (self.kernel_size + 1).min(64);
                println!("blur kernel size (+): {}", self.kernel_size);
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.kernel_size = (self.kernel_size - 1).max(0);
                println!("blur kernel size (-): {}", self.kernel_size);
            }
            Key::Named(NamedKey::ArrowRight) => {
                self.blur_radius = (self.blur_radius + 0.1).min(4.5);
                println!("blur radius (+): {:.2}", self.blur_radius);
            }
            Key::Named(NamedKey::ArrowLeft) => {
                self.blur_radius = (self.blur_radius - 0.1).max(0.0);
                println!("blur radius (-): {:.2}", self.blur_radius);
            }
            Key::Character(ch) => match ch.as_str() {
                "k" | "K" => {
                    self.is_kawase = !self.is_kawase;
                    println!("kawase: {}", self.is_kawase);
                }
                _ => {}
            },
            _ => {}
        };
    }

    pub fn draw(&mut self, _camera: &Camera, _mouse_pos: Vec2) {
        self.last_instant = Instant::now();

        self.draw_with_clear_color(0.0, 0.2, 0.15, 0.5);
    }

    fn draw_with_clear_color(&self, r: GLfloat, g: GLfloat, b: GLfloat, a: GLfloat) {
        unsafe {
            // 1st pass: draw Gura to framebuffer
            {
                gl::BindFramebuffer(gl::FRAMEBUFFER, self.comp_fbo);
                gl::Viewport(0, 0, self.gura_fb_size.x, self.gura_fb_size.y);

                gl::ClearColor(0.0, 0.0, 0.0, 0.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);
                gl::UseProgram(self.comp_shader);

                gl::BindVertexArray(self.comp_vao);
                gl::BindBuffer(gl::ARRAY_BUFFER, self.comp_vbo);
                gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, 0);
                gl::BufferSubData(
                    gl::ARRAY_BUFFER,
                    0,
                    mem::size_of_val(SCREEN_VERTICES) as GLsizeiptr,
                    SCREEN_VERTICES.as_ptr() as *const _,
                );

                gl::BindTexture(gl::TEXTURE_2D, self.gura_texture);
                gl::ActiveTexture(gl::TEXTURE0);
                gl::DrawArrays(gl::TRIANGLES, 0, 6);
            }

            let angles: &[f32] = if self.is_kawase { &[PI / 4.0] } else { &[0.0] };

            for angle in angles {
                // 2nd pass: draw framebuffer to ping-pong framebuffer, with X-blurring
                {
                    gl::BindFramebuffer(gl::FRAMEBUFFER, self.ping_pong_fbo);
                    gl::Viewport(0, 0, self.gura_fb_size.x, self.gura_fb_size.y);

                    gl::ClearColor(0.0, 0.0, 0.0, 0.0);
                    gl::Clear(gl::COLOR_BUFFER_BIT);
                    gl::UseProgram(self.blur_shader);

                    gl::Uniform1i(self.u_kernel_size, self.kernel_size);
                    gl::Uniform2f(self.u_direction, angle.cos() * self.blur_radius, angle.sin() * self.blur_radius);

                    gl::BindVertexArray(self.comp_vao);
                    gl::BindBuffer(gl::ARRAY_BUFFER, self.comp_vbo);
                    gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, 0);
                    gl::BufferSubData(
                        gl::ARRAY_BUFFER,
                        0,
                        mem::size_of_val(SCREEN_VERTICES) as GLsizeiptr,
                        SCREEN_VERTICES.as_ptr() as *const _,
                    );

                    gl::BindTexture(gl::TEXTURE_2D, self.comp_texture);
                    gl::DrawArrays(gl::TRIANGLES, 0, 6);
                }

                // 3rd pass: draw ping-pong framebuffer to framebuffer, with Y-blurring
                let angle = angle + PI / 2.0;
                {
                    gl::BindFramebuffer(gl::FRAMEBUFFER, self.comp_fbo);
                    gl::Viewport(0, 0, self.gura_fb_size.x, self.gura_fb_size.y);

                    gl::ClearColor(0.0, 0.0, 0.0, 0.0);
                    gl::Clear(gl::COLOR_BUFFER_BIT);
                    gl::UseProgram(self.blur_shader);

                    gl::Uniform1i(self.u_kernel_size, self.kernel_size);
                    gl::Uniform2f(self.u_direction, angle.cos() * self.blur_radius, angle.sin() * self.blur_radius);

                    gl::BindVertexArray(self.comp_vao);
                    gl::BindBuffer(gl::ARRAY_BUFFER, self.comp_vbo);
                    gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, 0);
                    gl::BufferSubData(
                        gl::ARRAY_BUFFER,
                        0,
                        mem::size_of_val(SCREEN_VERTICES) as GLsizeiptr,
                        SCREEN_VERTICES.as_ptr() as *const _,
                    );

                    gl::BindTexture(gl::TEXTURE_2D, self.ping_pong_texture);
                    gl::DrawArrays(gl::TRIANGLES, 0, 6);
                }
            }

            // 3rd pass: draw framebuffer to screen as quad
            {
                gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
                gl::Viewport(0, 0, self.viewport.x as i32, self.viewport.y as i32);

                gl::ClearColor(r, g, b, a);
                gl::Clear(gl::COLOR_BUFFER_BIT);
                gl::UseProgram(self.quad_shader);

                gl::BindVertexArray(self.quad_vao);
                gl::BindBuffer(gl::ARRAY_BUFFER, self.quad_vbo);
                gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.quad_ebo);

                gl::BindTexture(gl::TEXTURE_2D, self.comp_texture);
                gl::DrawElements(
                    gl::TRIANGLES,
                    mem::size_of_val(self.indices.as_slice()) as GLsizei,
                    gl::UNSIGNED_INT,
                    std::ptr::null(),
                );
            }
        }
    }

    pub fn resize(&mut self, camera: &Camera, width: i32, height: i32) {
        unsafe {
            gl::Viewport(0, 0, width, height);

            self.viewport = Vec2::new(width as f32, height as f32);
            self.matrix = camera.matrix(self.viewport);

            gl::UseProgram(self.quad_shader);
            gl::UniformMatrix4fv(self.u_mvp_quad, 1, gl::FALSE, self.matrix.as_ref().as_ptr());
        }
    }
}

impl Drop for BlurringScene {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.quad_shader);
            gl::DeleteBuffers(1, &self.quad_vbo);
            gl::DeleteVertexArrays(1, &self.quad_vao);
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Quad {
    pub position: Vec2,
    pub size: Vec2,
}

impl Quad {
    fn vertices(self) -> [Vertex; 4] {
        let Self { position, size } = self;

        #[rustfmt::skip]
        return [
            Vertex::new((vec2(-0.5, -0.5) * size) + position, vec2(0.0, 0.0)),
            Vertex::new((vec2(-0.5,  0.5) * size) + position, vec2(0.0, 1.0)),
            Vertex::new((vec2( 0.5,  0.5) * size) + position, vec2(1.0, 1.0)),
            Vertex::new((vec2( 0.5, -0.5) * size) + position, vec2(1.0, 0.0)),
        ];
    }

    fn indices(&self, quad_index: u32) -> [u32; 6] {
        let i = quad_index * 4;
        [i, 1 + i, 2 + i, i, 2 + i, 3 + i]
    }
}

/// Vertex used both for quads and for compositing.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct Vertex {
    position: Vec2,
    uv: Vec2,
}

impl Vertex {
    const fn new(position: Vec2, uv: Vec2) -> Self {
        Self { position, uv }
    }
}

#[rustfmt::skip]
const SCREEN_VERTICES: &[Vertex] = &[
                  // position       // uv
    Vertex::new(vec2(-1.0,  1.0), vec2(0.0, 1.0)),
    Vertex::new(vec2(-1.0, -1.0), vec2(0.0, 0.0)),
    Vertex::new(vec2( 1.0, -1.0), vec2(1.0, 0.0)),
    Vertex::new(vec2(-1.0,  1.0), vec2(0.0, 1.0)),
    Vertex::new(vec2( 1.0, -1.0), vec2(1.0, 0.0)),
    Vertex::new(vec2( 1.0,  1.0), vec2(1.0, 1.0)),
];
