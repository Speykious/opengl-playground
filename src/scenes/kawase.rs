use std::rc::Rc;
use std::{mem, time::Instant};

use glam::{Mat4, Vec2, uvec2, vec2};
use glow::HasContext;
use image::{EncodableLayout, ImageFormat};
use winit::keyboard::{Key, NamedKey, SmolStr};
use winit::{dpi::PhysicalSize, window::Window};

use crate::camera::Camera;
use crate::common::{
    Framebuffer, TextureWrapping, create_framebuffer, create_shader_program, pop_debug_group, push_debug_group,
    slice_as_bytes, upload_texture,
};

use super::{GURA_JPG, SRC_FRAG_DITHER, SRC_FRAG_KAWASE, SRC_FRAG_TEXTURE, SRC_VERT_QUAD, SRC_VERT_SCREEN};

const RESDIVS: &[u32] = &[2, 4, 8, 16, 32, 64];

struct BlurParams {
    pub radius: f32,
    pub layers: usize,
    pub is_dithered: bool,
}

pub struct KawaseScene {
    gl: Rc<glow::Context>,

    matrix: Mat4,
    viewport: Vec2,

    quad_shader: glow::Program,
    quad_vao: glow::VertexArray,
    quad_vbo: glow::Buffer,
    quad_ebo: glow::Buffer,

    composite_fbs: Vec<Framebuffer>,
    comp_vao: glow::VertexArray,
    comp_vbo: glow::Buffer,
    comp_shader: glow::Program,
    kawase_shader: glow::Program,
    dither_shader: glow::Program,

    gura_texture: glow::Texture,

    u_mvp_quad: glow::UniformLocation,
    u_mvp_dither: glow::UniformLocation,
    u_distance: glow::UniformLocation,
    u_upsample: glow::UniformLocation,

    blur: BlurParams,

    indices: Vec<[u32; 6]>,

    last_instant: Instant,
}

impl KawaseScene {
    pub fn new(gl: Rc<glow::Context>, window: &Window) -> Self {
        let PhysicalSize { width, height } = window.inner_size();
        let viewport = Vec2::new(width as f32, height as f32);

        let (gura, gura_texture) = unsafe {
            // Gura texture
            let gura = image::load_from_memory_with_format(GURA_JPG, ImageFormat::Jpeg);
            // let gura = image::load_from_memory_with_format(BIG_SQUARES_PNG, ImageFormat::Png);
            let gura = gura.unwrap().into_rgba8();

            let gura_texture = gl.create_texture().unwrap();
            upload_texture(
                &gl,
                gura_texture,
                gura.width(),
                gura.height(),
                Some(gura.as_bytes()),
                TextureWrapping::ClampToBorder,
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
            gl.enable(glow::BLEND);
            gl.blend_func_separate(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA, glow::SRC_ALPHA, glow::ONE);
            gl.blend_equation_separate(glow::FUNC_ADD, glow::FUNC_ADD);

            // framebuffers
            let composite_fbs = (RESDIVS.iter().copied())
                .map(|resdiv| {
                    create_framebuffer(
                        &gl,
                        "composite",
                        gura_size / resdiv,
                        TextureWrapping::ClampToBorder,
                        false,
                    )
                })
                .collect::<Vec<_>>();

            gl.bind_framebuffer(glow::FRAMEBUFFER, None);

            // quad vertices
            let quad_vao = gl.create_vertex_array().unwrap();
            gl.bind_vertex_array(Some(quad_vao));

            let quad_vbo = gl.create_buffer().unwrap();
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(quad_vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                slice_as_bytes(vertices.as_slice()),
                glow::DYNAMIC_DRAW,
            );

            let quad_ebo = gl.create_buffer().unwrap();
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(quad_ebo));
            gl.buffer_data_u8_slice(
                glow::ELEMENT_ARRAY_BUFFER,
                slice_as_bytes(indices.as_slice()),
                glow::STATIC_DRAW,
            );

            // quad shaders
            let quad_shader = create_shader_program(&gl, SRC_VERT_QUAD, SRC_FRAG_TEXTURE);
            let u_mvp_quad = gl.get_uniform_location(quad_shader, "u_mvp").unwrap();
            Self::set_pos_uv_vertex_attribs(&gl, quad_shader);

            let dither_shader = create_shader_program(&gl, SRC_VERT_QUAD, SRC_FRAG_DITHER);
            let u_mvp_dither = gl.get_uniform_location(dither_shader, "u_mvp").unwrap();
            Self::set_pos_uv_vertex_attribs(&gl, dither_shader);

            // compositing vertices
            let comp_vao = gl.create_vertex_array().unwrap();
            gl.bind_vertex_array(Some(comp_vao));

            let comp_vbo = gl.create_buffer().unwrap();
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(comp_vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, slice_as_bytes(SCREEN_VERTICES), glow::DYNAMIC_DRAW);

            // compositing shaders
            let comp_shader = create_shader_program(&gl, SRC_VERT_SCREEN, SRC_FRAG_TEXTURE);
            Self::set_pos_uv_vertex_attribs(&gl, comp_shader);

            let kawase_shader = create_shader_program(&gl, SRC_VERT_SCREEN, SRC_FRAG_KAWASE);
            let u_distance = gl.get_uniform_location(kawase_shader, "u_distance").unwrap();
            let u_upsample = gl.get_uniform_location(kawase_shader, "u_upsample").unwrap();
            Self::set_pos_uv_vertex_attribs(&gl, kawase_shader);

            // default blur parameters
            let blur = BlurParams {
                radius: 1.0,
                layers: 1,
                is_dithered: false,
            };

            Self {
                gl,

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
                kawase_shader,
                dither_shader,

                gura_texture,

                u_mvp_quad,
                u_mvp_dither,
                u_distance,
                u_upsample,

                blur,

                indices,

                last_instant: Instant::now(),
            }
        }
    }

    unsafe fn set_pos_uv_vertex_attribs(gl: &glow::Context, shader: glow::Program) {
        // Both `screen.vert` and `quad.vert` have the same vertex
        // attributes, so I'm using this function for all shaders.

        let size_vertex = mem::size_of::<Vertex>() as i32;
        let size_f32 = mem::size_of::<f32>() as i32;

        #[rustfmt::skip]
        unsafe {
            let a_position = gl.get_attrib_location(shader, "position").unwrap();
            let a_uv       = gl.get_attrib_location(shader, "uv").unwrap();

            gl.vertex_attrib_pointer_f32(a_position, 2, glow::FLOAT, false, size_vertex, 0           );
            gl.vertex_attrib_pointer_f32(a_uv,       2, glow::FLOAT, false, size_vertex, 2 * size_f32);

            gl.enable_vertex_attrib_array(a_position);
            gl.enable_vertex_attrib_array(a_uv);
        };
    }

    pub fn on_key(&mut self, keycode: Key<SmolStr>) {
        match keycode {
            Key::Named(NamedKey::ArrowRight) => {
                self.blur.radius = (self.blur.radius + 0.1).min(*RESDIVS.last().unwrap() as f32 / 2.0);
            }
            Key::Named(NamedKey::ArrowLeft) => {
                self.blur.radius = (self.blur.radius - 0.1).max(0.2);
            }
            Key::Character(ch) => match ch.as_str() {
                "d" | "D" => {
                    self.blur.is_dithered = !self.blur.is_dithered;
                }
                "l" => {
                    self.blur.layers = (self.blur.layers + 1).min(5);
                }
                "L" => {
                    self.blur.layers = self.blur.layers.saturating_sub(1);
                }
                _ => return,
            },
            _ => return,
        };

        let dither_mode = if self.blur.is_dithered { " dithering" } else { "" };

        println!(
            "kawase config: r={:.2} l={} {}",
            self.blur.radius, self.blur.layers, dither_mode
        );
    }

    pub fn draw(&mut self, _camera: &Camera, _mouse_pos: Vec2) {
        self.last_instant = Instant::now();

        self.draw_with_clear_color(0.0, 0.2, 0.15, 0.5);
    }

    fn draw_with_clear_color(&self, r: f32, g: f32, b: f32, a: f32) {
        let gl = &self.gl;
        unsafe {
            let texture = if self.blur.layers == 0 {
                push_debug_group(gl, "Draw normally");

                self.gura_texture
            } else {
                push_debug_group(gl, "Draw with blurring");

                let mut input_fb = &self.composite_fbs[0];

                // draw Gura to framebuffer
                push_debug_group(gl, "Gura to framebuffer");
                {
                    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(input_fb.fbo));
                    gl.viewport(0, 0, input_fb.size.x as i32, input_fb.size.y as i32);

                    gl.clear_color(0.0, 0.0, 0.0, 0.0);
                    gl.clear(glow::COLOR_BUFFER_BIT);
                    gl.use_program(Some(self.comp_shader));

                    gl.bind_vertex_array(Some(self.comp_vao));
                    gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.comp_vbo));
                    gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
                    gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, 0, slice_as_bytes(SCREEN_VERTICES));

                    gl.bind_texture(glow::TEXTURE_2D, Some(self.gura_texture));
                    gl.active_texture(glow::TEXTURE0);
                    gl.draw_arrays(glow::TRIANGLES, 0, 6);
                }
                pop_debug_group(gl);

                // blur at half-resolution, then quarter-res, then eighth-res, ...
                push_debug_group(gl, "Kawase downsampling");
                #[allow(clippy::needless_range_loop)]
                for fbi in 1..=self.blur.layers {
                    // FBI OPEN UP

                    let output_fb = &self.composite_fbs[fbi];
                    let distance = self.blur.radius;
                    input_fb = self.kawase_pass(distance, false, input_fb, output_fb);
                }
                pop_debug_group(gl);

                // ..., then eighth-res, then quarter-res, then half-resolution
                push_debug_group(gl, "Kawase upsampling");
                for fbi in (0..self.blur.layers).rev() {
                    // FBI OPEN UP

                    let output_fb = &self.composite_fbs[fbi];
                    let distance = self.blur.radius * 0.5;
                    input_fb = self.kawase_pass(distance, true, input_fb, output_fb);
                }
                pop_debug_group(gl);

                input_fb.texture
            };

            // draw framebuffer to screen as quad
            push_debug_group(gl, "Final draw to quad");
            {
                gl.bind_framebuffer(glow::FRAMEBUFFER, None);
                gl.viewport(0, 0, self.viewport.x as i32, self.viewport.y as i32);

                gl.clear_color(r, g, b, a);
                gl.clear(glow::COLOR_BUFFER_BIT);
                if self.blur.is_dithered {
                    gl.use_program(Some(self.dither_shader));
                } else {
                    gl.use_program(Some(self.quad_shader));
                }

                gl.bind_vertex_array(Some(self.quad_vao));
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.quad_vbo));
                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.quad_ebo));

                gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                gl.draw_elements(glow::TRIANGLES, self.indices.len() as i32 * 6, glow::UNSIGNED_INT, 0);
            }
            pop_debug_group(gl);

            pop_debug_group(gl); // Draw normally / with blurring
        }
    }

    fn kawase_pass<'a>(
        &self,
        distance: f32,
        upsample: bool,
        from_fb: &Framebuffer,
        to_fb: &'a Framebuffer,
    ) -> &'a Framebuffer {
        let gl = &self.gl;

        unsafe {
            push_debug_group(gl, "Kawase pass");

            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(to_fb.fbo));
            gl.viewport(0, 0, to_fb.size.x as i32, to_fb.size.y as i32);

            gl.clear_color(0.0, 0.0, 0.0, 0.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.use_program(Some(self.kawase_shader));

            gl.uniform_1_f32(Some(&self.u_distance), distance);
            gl.uniform_1_i32(Some(&self.u_upsample), upsample as i32);

            gl.bind_vertex_array(Some(self.comp_vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.comp_vbo));
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);

            gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, 0, slice_as_bytes(SCREEN_VERTICES));

            gl.bind_texture(glow::TEXTURE_2D, Some(from_fb.texture));
            gl.draw_arrays(glow::TRIANGLES, 0, 6);

            pop_debug_group(gl);
        }

        to_fb
    }

    pub fn resize(&mut self, camera: &Camera, width: i32, height: i32) {
        let gl = &self.gl;

        unsafe {
            gl.viewport(0, 0, width, height);

            self.viewport = Vec2::new(width as f32, height as f32);
            self.matrix = camera.matrix(self.viewport);

            gl.use_program(Some(self.quad_shader));
            gl.uniform_matrix_4_f32_slice(Some(&self.u_mvp_quad), false, self.matrix.as_ref());

            gl.use_program(Some(self.dither_shader));
            gl.uniform_matrix_4_f32_slice(Some(&self.u_mvp_dither), false, self.matrix.as_ref());
        }
    }
}

impl Drop for KawaseScene {
    fn drop(&mut self) {
        let gl = &self.gl;

        unsafe {
            gl.delete_program(self.quad_shader);
            gl.delete_program(self.comp_shader);
            gl.delete_program(self.kawase_shader);
            gl.delete_program(self.dither_shader);

            gl.delete_buffer(self.quad_vbo);
            gl.delete_buffer(self.quad_ebo);
            gl.delete_buffer(self.comp_vbo);

            gl.delete_vertex_array(self.quad_vao);
            gl.delete_vertex_array(self.comp_vao);

            gl.delete_texture(self.gura_texture);
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
