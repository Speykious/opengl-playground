use std::f32::consts::PI;
use std::{mem, time::Instant};

use gl::types::{GLfloat, GLint, GLsizei, GLsizeiptr, GLuint};
use glam::{uvec2, vec2, Mat4, Vec2};
use image::ImageFormat;
use winit::keyboard::{Key, NamedKey, SmolStr};
use winit::{dpi::PhysicalSize, window::Window};

use crate::camera::Camera;
use crate::common_gl::{create_framebuffer, create_shader_program, upload_texture, Framebuffer};

use super::{SRC_FRAG_BLUR, SRC_FRAG_DITHER, SRC_FRAG_TEXTURE, SRC_VERT_QUAD, SRC_VERT_SCREEN};

const GURA_JPG: &[u8] = include_bytes!("../../assets/gura.jpg");
// const BIG_SQUARES_PNG: &[u8] = include_bytes!("../../assets/big-squares.png");

const RESDIVS: &[u32] = &[2, 4, 8, 16, 32, 64];

struct BlurParams {
    pub kernel: i32,
    pub radius: f32,
    pub layers: usize,
    pub is_diagonal: bool,
    pub is_dithered: bool,
}

pub struct BlurringScene {
    matrix: Mat4,
    viewport: Vec2,

    quad_shader: GLuint,
    quad_vao: GLuint,
    quad_vbo: GLuint,
    quad_ebo: GLuint,

    composite_fbs: Vec<(Framebuffer, Framebuffer)>,
    comp_vao: GLuint,
    comp_vbo: GLuint,
    comp_shader: GLuint,
    blur_shader: GLuint,
    dither_shader: GLuint,

    gura_texture: GLuint,

    u_mvp_quad: GLint,
    u_mvp_dither: GLint,
    u_direction: GLint,
    u_kernel_size: GLint,

    blur: BlurParams,

    indices: Vec<[u32; 6]>,

    last_instant: Instant,
}

impl BlurringScene {
    pub fn new(window: &Window) -> Self {
        let PhysicalSize { width, height } = window.inner_size();
        let viewport = Vec2::new(width as f32, height as f32);

        let (gura, gura_texture) = unsafe {
            // Gura texture
            let gura = image::load_from_memory_with_format(GURA_JPG, ImageFormat::Jpeg);
            // let gura = image::load_from_memory_with_format(BIG_SQUARES_PNG, ImageFormat::Png);
            let gura = gura.unwrap().into_rgba8();

            let mut gura_texture: GLuint = 0;
            gl::GenTextures(1, &mut gura_texture);
            upload_texture(
                gura_texture,
                gura.width(),
                gura.height(),
                gura.as_ptr(),
                gl::CLAMP_TO_BORDER,
            );

            (gura, gura_texture)
        };

        let gura_size = uvec2(gura.width(), gura.height());

        // They don't need to be vecs, but I'm too lazy to un-vector them now.
        let mut quads = Vec::with_capacity(1);
        let mut vertices = Vec::with_capacity(1);
        let mut indices = Vec::with_capacity(1);

        let quad = Quad {
            position: Vec2::ZERO,
            size: gura_size.as_vec2(),
        };
        vertices.push(quad.vertices());
        indices.push(quad.indices(0));
        quads.push(quad);

        unsafe {
            // Normal blending
            gl::Enable(gl::BLEND);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);

            // framebuffers
            let composite_fbs = (RESDIVS.iter().copied())
                .map(|resdiv| {
                    (
                        create_framebuffer("composite", gura_size / resdiv),
                        create_framebuffer("ping_pong", gura_size / resdiv),
                    )
                })
                .collect::<Vec<_>>();

            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);

            // quad vertices
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

            // quad shaders
            let quad_shader = create_shader_program(SRC_VERT_QUAD, SRC_FRAG_TEXTURE);
            let u_mvp_quad = gl::GetUniformLocation(quad_shader, c"u_mvp".as_ptr());
            Self::set_pos_uv_vertex_attribs(quad_shader);

            let dither_shader = create_shader_program(SRC_VERT_QUAD, SRC_FRAG_DITHER);
            let u_mvp_dither = gl::GetUniformLocation(dither_shader, c"u_mvp".as_ptr());
            Self::set_pos_uv_vertex_attribs(dither_shader);

            // compositing vertices
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

            // compositing shaders
            let comp_shader = create_shader_program(SRC_VERT_SCREEN, SRC_FRAG_TEXTURE);
            Self::set_pos_uv_vertex_attribs(comp_shader);

            let blur_shader = create_shader_program(SRC_VERT_SCREEN, SRC_FRAG_BLUR);
            let u_direction = gl::GetUniformLocation(blur_shader, c"u_direction".as_ptr());
            let u_kernel_size = gl::GetUniformLocation(blur_shader, c"u_kernel_size".as_ptr());
            Self::set_pos_uv_vertex_attribs(blur_shader);

            // default blur parameters
            let blur = BlurParams {
                kernel: 5,
                layers: 4,
                radius: 2.0,
                is_diagonal: false,
                is_dithered: false,
            };

            Self {
                matrix: Mat4::default(),
                viewport,

                quad_shader,
                quad_vao,
                quad_vbo,
                quad_ebo,

                composite_fbs,
                comp_vao,
                comp_vbo,
                comp_shader,
                blur_shader,
                dither_shader,

                gura_texture,

                u_mvp_quad,
                u_mvp_dither,
                u_direction,
                u_kernel_size,

                blur,

                indices,

                last_instant: Instant::now(),
            }
        }
    }

    unsafe fn set_pos_uv_vertex_attribs(shader: GLuint) {
        // Both `screen.vert` and `quad.vert` have the same vertex
        // attributes, so I'm using this function for all shaders.

        const SIZE_VERTEX: GLsizei = mem::size_of::<Vertex>() as GLsizei;
        const SIZE_F32: GLsizei = mem::size_of::<f32>() as GLsizei;

        #[rustfmt::skip]
        {
            let a_position = gl::GetAttribLocation(shader, c"position" .as_ptr()) as GLuint;
            let a_uv       = gl::GetAttribLocation(shader, c"uv"       .as_ptr()) as GLuint;

            gl::VertexAttribPointer(a_position, 2, gl::FLOAT, gl::FALSE, SIZE_VERTEX,  0             as _);
            gl::VertexAttribPointer(a_uv,       2, gl::FLOAT, gl::FALSE, SIZE_VERTEX, (2 * SIZE_F32) as _);

            gl::EnableVertexAttribArray(a_position as GLuint);
            gl::EnableVertexAttribArray(a_uv       as GLuint);
        };
    }

    pub fn on_key(&mut self, keycode: Key<SmolStr>) {
        match keycode {
            Key::Named(NamedKey::ArrowUp) => {
                self.blur.kernel = (self.blur.kernel + 1).min(64);
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.blur.kernel = (self.blur.kernel - 1).max(0);
            }
            Key::Named(NamedKey::ArrowRight) => {
                self.blur.radius =
                    (self.blur.radius + 0.1).min(*RESDIVS.last().unwrap() as f32 / 2.0);
            }
            Key::Named(NamedKey::ArrowLeft) => {
                self.blur.radius = (self.blur.radius - 0.1).max(0.0);
            }
            Key::Character(ch) => match ch.as_str() {
                "d" | "D" => {
                    self.blur.is_dithered = !self.blur.is_dithered;
                }
                "/" => {
                    self.blur.is_diagonal = !self.blur.is_diagonal;
                }
                "l" => {
                    self.blur.layers = (self.blur.layers + 1).min(RESDIVS.len());
                }
                "L" => {
                    self.blur.layers = self.blur.layers.saturating_sub(1);
                }
                _ => return,
            },
            _ => return,
        };

        let mode = if self.blur.is_diagonal {
            "diagonal"
        } else {
            "vert/horz"
        };

        let dither_mode = if self.blur.is_dithered {
            " dithering"
        } else {
            ""
        };

        println!(
            "blur config: k={} r={:.2} l={} {}{}",
            self.blur.kernel, self.blur.radius, self.blur.layers, mode, dither_mode
        );
    }

    pub fn draw(&mut self, _camera: &Camera, _mouse_pos: Vec2) {
        self.last_instant = Instant::now();

        self.draw_with_clear_color(0.0, 0.2, 0.15, 0.5);
    }

    fn draw_with_clear_color(&self, r: GLfloat, g: GLfloat, b: GLfloat, a: GLfloat) {
        unsafe {
            let texture = if self.blur.layers == 0 {
                self.gura_texture
            } else {
                let mut input_fb = &self.composite_fbs[0].0;

                // draw Gura to framebuffer
                {
                    gl::BindFramebuffer(gl::FRAMEBUFFER, input_fb.fbo);
                    gl::Viewport(0, 0, input_fb.size.x as i32, input_fb.size.y as i32);

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

                let angles: &[f32] = if self.blur.is_diagonal {
                    &[PI / 4.0]
                } else {
                    &[0.0]
                };

                // blur at half-resolution, then quarter-res, then eighth-res, ...
                for fbi in 0..self.blur.layers {
                    // FBI OPEN UP

                    for angle in angles {
                        input_fb = self.ping_pong_blur_pass(
                            *angle,
                            input_fb,
                            &self.composite_fbs[fbi].0,
                            &self.composite_fbs[fbi].1,
                        );
                    }
                }

                // ..., then eighth-res, then quarter-res, then half-resolution
                for fbi in (0..(self.blur.layers - 1)).rev() {
                    // FBI OPEN UP

                    for angle in angles {
                        input_fb = self.ping_pong_blur_pass(
                            *angle,
                            input_fb,
                            &self.composite_fbs[fbi].0,
                            &self.composite_fbs[fbi].1,
                        );
                    }
                }

                input_fb.texture
            };

            // draw framebuffer to screen as quad
            {
                gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
                gl::Viewport(0, 0, self.viewport.x as i32, self.viewport.y as i32);

                gl::ClearColor(r, g, b, a);
                gl::Clear(gl::COLOR_BUFFER_BIT);
                if self.blur.is_dithered {
                    gl::UseProgram(self.dither_shader);
                } else {
                    gl::UseProgram(self.quad_shader);
                }

                gl::BindVertexArray(self.quad_vao);
                gl::BindBuffer(gl::ARRAY_BUFFER, self.quad_vbo);
                gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.quad_ebo);

                gl::BindTexture(gl::TEXTURE_2D, texture);
                gl::DrawElements(
                    gl::TRIANGLES,
                    mem::size_of_val(self.indices.as_slice()) as GLsizei,
                    gl::UNSIGNED_INT,
                    std::ptr::null(),
                );
            }
        }
    }

    fn ping_pong_blur_pass<'a>(
        &self,
        angle: f32,
        from_fb: &Framebuffer,
        composite_fb: &'a Framebuffer,
        ping_pong_fb: &Framebuffer,
    ) -> &'a Framebuffer {
        // draw framebuffer to ping-pong framebuffer, with X-blurring
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, ping_pong_fb.fbo);
            gl::Viewport(0, 0, ping_pong_fb.size.x as i32, ping_pong_fb.size.y as i32);

            gl::ClearColor(0.0, 0.0, 0.0, 0.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::UseProgram(self.blur_shader);

            gl::Uniform1i(self.u_kernel_size, self.blur.kernel);
            gl::Uniform2f(
                self.u_direction,
                angle.cos() * self.blur.radius,
                angle.sin() * self.blur.radius,
            );

            gl::BindVertexArray(self.comp_vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.comp_vbo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, 0);
            gl::BufferSubData(
                gl::ARRAY_BUFFER,
                0,
                mem::size_of_val(SCREEN_VERTICES) as GLsizeiptr,
                SCREEN_VERTICES.as_ptr() as *const _,
            );

            gl::BindTexture(gl::TEXTURE_2D, from_fb.texture);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
        }

        // draw ping-pong framebuffer to framebuffer, with Y-blurring
        let angle = angle + PI / 2.0;
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, composite_fb.fbo);
            gl::Viewport(0, 0, composite_fb.size.x as i32, composite_fb.size.y as i32);

            gl::ClearColor(0.0, 0.0, 0.0, 0.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::UseProgram(self.blur_shader);

            gl::Uniform1i(self.u_kernel_size, self.blur.kernel);
            gl::Uniform2f(
                self.u_direction,
                angle.cos() * self.blur.radius,
                angle.sin() * self.blur.radius,
            );

            gl::BindVertexArray(self.comp_vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.comp_vbo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, 0);
            gl::BufferSubData(
                gl::ARRAY_BUFFER,
                0,
                mem::size_of_val(SCREEN_VERTICES) as GLsizeiptr,
                SCREEN_VERTICES.as_ptr() as *const _,
            );

            gl::BindTexture(gl::TEXTURE_2D, ping_pong_fb.texture);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
        }

        composite_fb
    }

    pub fn resize(&mut self, camera: &Camera, width: i32, height: i32) {
        unsafe {
            gl::Viewport(0, 0, width, height);

            self.viewport = Vec2::new(width as f32, height as f32);
            self.matrix = camera.matrix(self.viewport);

            gl::UseProgram(self.quad_shader);
            gl::UniformMatrix4fv(self.u_mvp_quad, 1, gl::FALSE, self.matrix.as_ref().as_ptr());

            gl::UseProgram(self.dither_shader);
            gl::UniformMatrix4fv(
                self.u_mvp_dither,
                1,
                gl::FALSE,
                self.matrix.as_ref().as_ptr(),
            );
        }
    }
}

impl Drop for BlurringScene {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.quad_shader);
            gl::DeleteProgram(self.comp_shader);
            gl::DeleteProgram(self.blur_shader);
            gl::DeleteProgram(self.dither_shader);

            for comp_fb in &self.composite_fbs {
                let fbs = &[comp_fb.0.fbo, comp_fb.1.fbo];
                gl::DeleteFramebuffers(fbs.len() as GLsizei, fbs.as_ptr());

                let textures = &[comp_fb.0.texture, comp_fb.1.texture];
                gl::DeleteTextures(textures.len() as GLsizei, textures.as_ptr());
            }

            let buffers = &[self.quad_vbo, self.quad_ebo, self.comp_vbo];
            gl::DeleteBuffers(buffers.len() as GLsizei, buffers.as_ptr());

            let arrays = &[self.quad_vao, self.comp_vao];
            gl::DeleteVertexArrays(arrays.len() as GLsizei, arrays.as_ptr());

            gl::DeleteTextures(1, &self.gura_texture);
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
    pub position: Vec2,
    pub uv: Vec2,
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
