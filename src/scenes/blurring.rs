use std::{mem, time::Instant};

use gl::types::{GLfloat, GLint, GLsizei, GLsizeiptr, GLuint};
use glam::{vec2, Mat4, Vec2};
use image::ImageFormat;
use winit::keyboard::NamedKey;
use winit::{dpi::PhysicalSize, window::Window};

use crate::camera::Camera;

use super::create_shader_program;

const SRC_VERT_QUAD: &[u8] = include_bytes!("shaders/quad.vert");
const SRC_FRAG_TEXTURE: &[u8] = include_bytes!("shaders/texture.frag");

const SRC_VERT_SCREEN: &[u8] = include_bytes!("shaders/screen.vert");
const SRC_FRAG_BLUR: &[u8] = include_bytes!("shaders/blur.frag");

const GURA_JPG: &[u8] = include_bytes!("../../assets/gura.jpg");

pub struct BlurringScene {
    matrix: Mat4,
    viewport: Vec2,

    quad_shader: GLuint,
    quad_vao: GLuint,
    quad_vbo: GLuint,
    quad_ebo: GLuint,

    screen_fbo: GLuint,
    screen_texture: GLuint,
    screen_shader: GLuint,
    screen_vao: GLuint,
    screen_vbo: GLuint,

    ping_pong_fbo: GLuint,
    ping_pong_texture: GLuint,

    gura_texture: GLuint,

    u_mvp_quad: GLint,
    u_direction: GLint,
    u_screen_size: GLint,

    blur_passes: u32,

    indices: Vec<[u32; 6]>,

    last_instant: Instant,
}

impl BlurringScene {
    pub fn new(window: &Window) -> Self {
        let PhysicalSize { width, height } = window.inner_size();
        let viewport = Vec2::new(width as f32, height as f32);

        // They don't need to be vecs, but I'm too lazy to un-vector them now.
        let mut quads = Vec::with_capacity(1);
        let mut vertices = Vec::with_capacity(1);
        let mut indices = Vec::with_capacity(1);

        let quad = Quad {
            position: Vec2::ZERO,
            size: vec2(1280.0, 640.0),
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

            {
                let a_position = gl::GetAttribLocation(quad_shader, c"position".as_ptr()) as GLuint;
                gl::VertexAttribPointer(a_position, 2, gl::FLOAT, gl::FALSE, size_vertex, 0 as _);
                gl::EnableVertexAttribArray(a_position as GLuint);
            };

            // framebuffer and its texture
            let mut screen_fbo: GLuint = 0;
            gl::GenFramebuffers(1, &mut screen_fbo);
            gl::BindFramebuffer(gl::FRAMEBUFFER, screen_fbo);

            let mut screen_texture: GLuint = 0;
            gl::GenTextures(1, &mut screen_texture);
            Self::upload_texture(screen_texture, width, height, std::ptr::null());
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                screen_texture,
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
            Self::upload_texture(ping_pong_texture, width, height, std::ptr::null());
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

            // screen shader and vertices
            let screen_shader = create_shader_program(SRC_VERT_SCREEN, SRC_FRAG_BLUR);
            let u_direction = gl::GetUniformLocation(screen_shader, c"u_direction".as_ptr());
            let u_screen_size = gl::GetUniformLocation(screen_shader, c"u_screen_size".as_ptr());

            let mut screen_vao: GLuint = 0;
            gl::GenVertexArrays(1, &mut screen_vao);
            gl::BindVertexArray(screen_vao);

            let mut screen_vbo: GLuint = 0;
            gl::GenBuffers(1, &mut screen_vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, screen_vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                mem::size_of_val(SCREEN_VERTICES) as GLsizeiptr,
                SCREEN_VERTICES.as_ptr() as *const _,
                gl::STATIC_DRAW,
            );

            let size_screen_vertex = mem::size_of::<ScreenVertex>() as GLsizei;

            #[rustfmt::skip]
            {
                let a_position = gl::GetAttribLocation(screen_shader, c"position" .as_ptr()) as GLuint;
                let a_uv       = gl::GetAttribLocation(screen_shader, c"uv"       .as_ptr()) as GLuint;

                gl::VertexAttribPointer(a_position, 2, gl::FLOAT, gl::FALSE, size_screen_vertex,   0             as _);
                gl::VertexAttribPointer(a_uv,       2, gl::FLOAT, gl::FALSE, size_screen_vertex, ( 2 * size_f32) as _);

                gl::EnableVertexAttribArray(a_position as GLuint);
                gl::EnableVertexAttribArray(a_uv       as GLuint);
            };

            // Gura texture
            let gura = image::load_from_memory_with_format(GURA_JPG, ImageFormat::Jpeg);
            let gura = gura.unwrap().into_rgba8();

            let mut gura_texture: GLuint = 0;
            gl::GenTextures(1, &mut gura_texture);
            Self::upload_texture(gura_texture, gura.width(), gura.height(), gura.as_ptr());

            Self {
                matrix: Mat4::default(),
                viewport,

                quad_shader,
                quad_vao,
                quad_vbo,
                quad_ebo,

                screen_fbo,
                screen_texture,
                screen_shader,
                screen_vao,
                screen_vbo,

                ping_pong_fbo,
                ping_pong_texture,

                gura_texture,

                u_mvp_quad,
                u_direction,
                u_screen_size,

                blur_passes: 1,

                indices,

                last_instant: Instant::now(),
            }
        }
    }

    unsafe fn upload_texture(texture: GLuint, width: u32, height: u32, data: *const u8) {
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
        gl::TexParameteri(
            gl::TEXTURE_2D,
            gl::TEXTURE_WRAP_S,
            gl::CLAMP_TO_BORDER as GLint,
        );
        gl::TexParameteri(
            gl::TEXTURE_2D,
            gl::TEXTURE_WRAP_T,
            gl::CLAMP_TO_BORDER as GLint,
        );
        gl::GenerateMipmap(gl::TEXTURE_2D);
    }

    pub fn on_key(&mut self, keycode: NamedKey) {
        let update_blur = match keycode {
            NamedKey::ArrowUp => {
                self.blur_passes = (self.blur_passes + 1).min(32);
                true
            }
            NamedKey::ArrowDown => {
                self.blur_passes = (self.blur_passes - 1).max(1);
                true
            }
            _ => false,
        };

        if update_blur {
            println!("blur passes: {}", self.blur_passes);
        }
    }

    pub fn draw(&mut self, _camera: &Camera, _mouse_pos: Vec2) {
        self.last_instant = Instant::now();

        self.draw_with_clear_color(0.0, 0.2, 0.15, 0.5);
    }

    fn draw_with_clear_color(&self, r: GLfloat, g: GLfloat, b: GLfloat, a: GLfloat) {
        unsafe {
            // draw to framebuffer
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.screen_fbo);

            gl::ClearColor(r, g, b, a);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::UseProgram(self.quad_shader);

            gl::BindVertexArray(self.quad_vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.quad_vbo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.quad_ebo);

            gl::BindTexture(gl::TEXTURE_2D, self.gura_texture);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::DrawElements(
                gl::TRIANGLES,
                mem::size_of_val(self.indices.as_slice()) as GLsizei,
                gl::UNSIGNED_INT,
                std::ptr::null(),
            );

            // ping-pong blur
            gl::UseProgram(self.screen_shader);

            gl::BindVertexArray(self.screen_vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.screen_vbo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, 0);

            gl::ActiveTexture(gl::TEXTURE0);

            for i in 0..self.blur_passes {
                let (fbo, tex, (dx, dy)) = if i % 2 == 0 {
                    (self.ping_pong_fbo, self.screen_texture, (1.0, 0.0))
                } else {
                    (self.screen_fbo, self.ping_pong_texture, (0.0, 1.0))
                };

                // draw framebuffer to ping-pong framebuffer
                gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
                gl::ClearColor(0.0, 0.0, 0.0, 0.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);

                gl::UseProgram(self.screen_shader);
                gl::Uniform2f(self.u_direction, dx, dy);

                gl::BindTexture(gl::TEXTURE_2D, tex);
                gl::DrawArrays(gl::TRIANGLES, 0, 6);
            }

            let (tex, (dx, dy)) = if self.blur_passes % 2 == 0 {
                (self.screen_texture, (1.0, 0.0))
            } else {
                (self.ping_pong_texture, (0.0, 1.0))
            };

            // draw ping-pong framebuffer to framebuffer
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::ClearColor(0.0, 0.0, 0.0, 0.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            gl::Uniform2f(self.u_direction, dx, dy);

            gl::BindTexture(gl::TEXTURE_2D, tex);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
        }
    }

    pub fn resize(&mut self, camera: &Camera, width: i32, height: i32) {
        unsafe {
            gl::Viewport(0, 0, width, height);

            self.viewport = Vec2::new(width as f32, height as f32);
            self.matrix = camera.matrix(self.viewport);

            gl::UseProgram(self.quad_shader);
            gl::UniformMatrix4fv(self.u_mvp_quad, 1, gl::FALSE, self.matrix.as_ref().as_ptr());

            gl::UseProgram(self.screen_shader);
            gl::Uniform2f(self.u_screen_size, self.viewport.x, self.viewport.y);

            // update framebuffer texture sizes
            let (w, h) = (width as u32, height as u32);

            gl::BindFramebuffer(gl::FRAMEBUFFER, self.screen_fbo);
            Self::upload_texture(self.screen_texture, w, h, std::ptr::null());
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                self.screen_texture,
                0,
            );

            gl::BindFramebuffer(gl::FRAMEBUFFER, self.ping_pong_fbo);
            Self::upload_texture(self.ping_pong_texture, w, h, std::ptr::null());
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                self.ping_pong_texture,
                0,
            );
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
        let pos_dims = [
            (vec2(-0.5, -0.5) * size) + position,
            (vec2(-0.5,  0.5) * size) + position,
            (vec2( 0.5,  0.5) * size) + position,
            (vec2( 0.5, -0.5) * size) + position,
        ];

        pos_dims.map(|position| Vertex { position, size })
    }

    fn indices(&self, quad_index: u32) -> [u32; 6] {
        let i = quad_index * 4;
        [i, 1 + i, 2 + i, i, 2 + i, 3 + i]
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct Vertex {
    position: Vec2,
    size: Vec2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct ScreenVertex {
    position: Vec2,
    uv: Vec2,
}

impl ScreenVertex {
    const fn new(position: Vec2, uv: Vec2) -> Self {
        Self { position, uv }
    }
}

#[rustfmt::skip]
const SCREEN_VERTICES: &[ScreenVertex] = &[
                        // position       // uv
    ScreenVertex::new(vec2(-1.0,  1.0), vec2(0.0, 1.0)),
    ScreenVertex::new(vec2(-1.0, -1.0), vec2(0.0, 0.0)),
    ScreenVertex::new(vec2( 1.0, -1.0), vec2(1.0, 0.0)),
    ScreenVertex::new(vec2(-1.0,  1.0), vec2(0.0, 1.0)),
    ScreenVertex::new(vec2( 1.0, -1.0), vec2(1.0, 0.0)),
    ScreenVertex::new(vec2( 1.0,  1.0), vec2(1.0, 1.0)),
];
