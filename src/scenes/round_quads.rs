use std::{
    f32::consts::{PI, TAU},
    mem::{self, offset_of},
    rc::Rc,
    time::Instant,
};

use glam::{Mat4, Vec2, vec2};
use glow::HasContext;
use rand::Rng;
use winit::window::Window;

use crate::{
    camera::Camera,
    common::{create_shader_program, slice_as_bytes},
};

use super::{SRC_FRAG_ROUND_RECT, SRC_VERT_ROUND_RECT};

const N_QUADS: usize = 100_000;

pub struct RoundQuadsScene {
    gl: Rc<glow::Context>,

    matrix: Mat4,
    viewport: Vec2,

    round_rect_shader: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    ebo: glow::Buffer,

    u_mvp_quad: glow::UniformLocation,

    quads: Vec<Quad>,
    vertices: Vec<[Vertex; 4]>,
    indices: Vec<[u32; 6]>,

    area_width: u32,

    last_instant: Instant,
}

impl RoundQuadsScene {
    pub fn new(gl: Rc<glow::Context>, window: &Window) -> Self {
        let area_width = (N_QUADS as f32).sqrt() as u32;

        let mut quads = Vec::with_capacity(N_QUADS);
        let mut vertices = Vec::with_capacity(N_QUADS);
        let mut indices = Vec::with_capacity(N_QUADS);

        let mut rng = rand::rng();
        for i in 0..(N_QUADS as u32) {
            let quad = Quad::random(&mut rng, i, area_width);
            vertices.push(quad.vertices(0.5));
            indices.push(quad.indices(i));
            quads.push(quad);
        }

        unsafe {
            // Normal blending
            gl.enable(glow::BLEND);
            gl.blend_func_separate(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA, glow::SRC_ALPHA, glow::ONE);
            gl.blend_equation_separate(glow::FUNC_ADD, glow::FUNC_ADD);

            let round_rect_shader = create_shader_program(&gl, SRC_VERT_ROUND_RECT, SRC_FRAG_ROUND_RECT);

            let u_mvp_quad = gl.get_uniform_location(round_rect_shader, "u_mvp").unwrap();

            let vao = gl.create_vertex_array().unwrap();
            gl.bind_vertex_array(Some(vao));

            let vbo = gl.create_buffer().unwrap();
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                slice_as_bytes(vertices.as_slice()),
                glow::DYNAMIC_DRAW,
            );

            let ebo = gl.create_buffer().unwrap();
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
            gl.buffer_data_u8_slice(
                glow::ELEMENT_ARRAY_BUFFER,
                slice_as_bytes(indices.as_slice()),
                glow::STATIC_DRAW,
            );

            let size_vertex = mem::size_of::<Vertex>() as i32;

            #[rustfmt::skip]
            {
                let a_position      = gl.get_attrib_location(round_rect_shader, "position"     ).unwrap();
                let a_size          = gl.get_attrib_location(round_rect_shader, "size"         ).unwrap();
                let a_fill_color    = gl.get_attrib_location(round_rect_shader, "fill_color"   ).unwrap();
                let a_stroke_color  = gl.get_attrib_location(round_rect_shader, "stroke_color" ).unwrap();
                let a_border_radius = gl.get_attrib_location(round_rect_shader, "border_radius").unwrap();
                let a_border_width  = gl.get_attrib_location(round_rect_shader, "border_width" ).unwrap();
                let a_intensity     = gl.get_attrib_location(round_rect_shader, "intensity"    ).unwrap();

                gl.vertex_attrib_pointer_f32(a_position,      2, glow::FLOAT, false, size_vertex, offset_of!(Vertex, position)      as _);
                gl.vertex_attrib_pointer_f32(a_size,          2, glow::FLOAT, false, size_vertex, offset_of!(Vertex, size)          as _);
                gl.vertex_attrib_pointer_i32(a_fill_color,    1, glow::INT,          size_vertex, offset_of!(Vertex, fill_color)    as _);
                gl.vertex_attrib_pointer_i32(a_stroke_color,  1, glow::INT,          size_vertex, offset_of!(Vertex, stroke_color)  as _);
                gl.vertex_attrib_pointer_f32(a_border_radius, 1, glow::FLOAT, false, size_vertex, offset_of!(Vertex, border_radius) as _);
                gl.vertex_attrib_pointer_f32(a_border_width,  1, glow::FLOAT, false, size_vertex, offset_of!(Vertex, border_width)  as _);
                gl.vertex_attrib_pointer_f32(a_intensity,     1, glow::FLOAT, false, size_vertex, offset_of!(Vertex, intensity)     as _);

                gl.enable_vertex_attrib_array(a_position);
                gl.enable_vertex_attrib_array(a_size);
                gl.enable_vertex_attrib_array(a_fill_color);
                gl.enable_vertex_attrib_array(a_stroke_color);
                gl.enable_vertex_attrib_array(a_border_radius);
                gl.enable_vertex_attrib_array(a_border_width);
                gl.enable_vertex_attrib_array(a_intensity);
            };

            let win_size = window.inner_size();
            let viewport = Vec2::new(win_size.width as f32, win_size.height as f32);

            Self {
                gl,

                matrix: Mat4::default(),
                viewport,

                round_rect_shader,
                vao,
                vbo,
                ebo,

                u_mvp_quad,

                quads,
                vertices,
                indices,

                area_width,

                last_instant: Instant::now(),
            }
        }
    }

    pub fn draw(&mut self, camera: &Camera, mouse_pos: Vec2) {
        let dt = self.last_instant.elapsed().as_secs_f32();
        self.last_instant = Instant::now();

        // rotate surroundings of mouse
        let mouse_pos = camera.pointer_to_pos(mouse_pos, self.viewport);
        let surround_radius = 320.0;
        let surround_area = Vec2::splat(surround_radius);

        let aw = self.area_width;
        let (x_beg, y_beg) = Quad::closest_grid_idx_from_pos(mouse_pos - surround_area, aw);
        let (x_end, y_end) = Quad::closest_grid_idx_from_pos(mouse_pos + surround_area, aw);

        for y in y_beg..=y_end {
            for x in x_beg..=x_end {
                let i = (y * self.area_width + x) as usize;

                if let Some(quad) = self.quads.get_mut(i) {
                    let distance = Vec2::distance(quad.position, mouse_pos);
                    let intensity = (surround_radius - distance).max(0.0) / surround_radius;

                    quad.rotation += (dt * PI) * 2.0 * intensity;
                    self.vertices[i] = quad.vertices(0.5 * intensity + 0.5);
                }
            }
        }

        self.update_vertices(x_beg, x_end, y_beg, y_end);

        self.draw_with_clear_color(0.0, 0.0, 0.0, 0.5);

        // reset intensity
        for y in y_beg..=y_end {
            for x in x_beg..=x_end {
                let i = (y * self.area_width + x) as usize;

                if let Some(quad) = self.quads.get_mut(i) {
                    self.vertices[i] = quad.vertices(0.5);
                }
            }
        }

        // reset vertices (otherwise artifacts appear if the mouse moves too quickly)
        self.update_vertices(x_beg, x_end, y_beg, y_end);
    }

    fn update_vertices(&mut self, x_beg: u32, x_end: u32, y_beg: u32, y_end: u32) {
        let gl = &self.gl;

        unsafe {
            gl.bind_vertex_array(Some(self.vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo));

            for y in y_beg..=y_end {
                let i_beg = (y * self.area_width + x_beg) as usize;
                let i_end = (y * self.area_width + x_end) as usize;

                gl.buffer_sub_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    mem::size_of_val(&self.vertices[..i_beg]) as i32,
                    slice_as_bytes(&self.vertices[i_beg..=i_end]),
                );
            }
        }
    }

    fn draw_with_clear_color(&self, r: f32, g: f32, b: f32, a: f32) {
        let gl = &self.gl;

        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);

            gl.bind_vertex_array(Some(self.vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo));

            gl.clear_color(r, g, b, a);
            gl.clear(glow::COLOR_BUFFER_BIT);

            gl.use_program(Some(self.round_rect_shader));
            gl.draw_elements(glow::TRIANGLES, self.indices.len() as i32 * 6, glow::UNSIGNED_INT, 0);
        }
    }

    pub fn resize(&mut self, camera: &Camera, width: i32, height: i32) {
        let gl = &self.gl;

        unsafe {
            gl.viewport(0, 0, width, height);

            self.viewport = Vec2::new(width as f32, height as f32);
            self.matrix = camera.matrix(self.viewport);

            gl.use_program(Some(self.round_rect_shader));
            gl.uniform_matrix_4_f32_slice(Some(&self.u_mvp_quad), false, self.matrix.as_ref());
        }
    }
}

impl Drop for RoundQuadsScene {
    fn drop(&mut self) {
        let gl = &self.gl;

        unsafe {
            gl.delete_program(self.round_rect_shader);
            gl.delete_vertex_array(self.vao);

            gl.delete_buffer(self.vbo);
            gl.delete_buffer(self.ebo);
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Quad {
    pub position: Vec2,
    pub size: Vec2,
    pub rotation: f32,
    pub border_radius: f32,
    pub border_width: f32,
    pub fill_color: u32,
    pub stroke_color: u32,
}

impl Quad {
    fn pos_from_idx(i: u32, area_width: u32) -> Vec2 {
        Self::pos_from_grid_idx((i % area_width, i / area_width), area_width)
    }

    fn pos_from_grid_idx((x, y): (u32, u32), area_width: u32) -> Vec2 {
        (vec2(x as f32, y as f32) - area_width as f32 * 0.5) * 16.0
    }

    fn closest_grid_idx_from_pos(pos: Vec2, area_width: u32) -> (u32, u32) {
        let width = area_width as f32;
        let upper_limit = width - 1.0;

        let pos = pos / 16.0 + width * 0.5;
        (
            pos.x.round().clamp(0.0, upper_limit) as u32,
            pos.y.round().clamp(0.0, upper_limit) as u32,
        )
    }

    fn random(rng: &mut impl Rng, i: u32, area_width: u32) -> Self {
        Self {
            position: Self::pos_from_idx(i, area_width),
            size: vec2(rng.random_range(10.0..=20.0), rng.random_range(10.0..=20.0)),
            rotation: rng.random_range(0.0..TAU),
            border_radius: rng.random_range(1.0..=5.0),
            border_width: rng.random_range(1.0..=5.0),
            fill_color: u32::from_be_bytes([
                rng.random_range(100..=128),
                rng.random_range(100..=128),
                rng.random_range(100..=128),
                rng.random_range(200..=255),
            ]),
            stroke_color: u32::from_be_bytes([
                rng.random_range(24..=128),
                rng.random_range(24..=128),
                rng.random_range(24..=128),
                255,
            ]),
        }
    }

    fn vertices(self, intensity: f32) -> [Vertex; 4] {
        let Self {
            position,
            size,
            rotation,
            border_radius,
            border_width,
            fill_color,
            stroke_color,
        } = self;

        let r = vec2(rotation.cos(), rotation.sin());

        #[rustfmt::skip]
        let pos_dims = [
            ((vec2(-0.5, -0.5) * size).rotate(r)) + position,
            ((vec2(-0.5,  0.5) * size).rotate(r)) + position,
            ((vec2( 0.5,  0.5) * size).rotate(r)) + position,
            ((vec2( 0.5, -0.5) * size).rotate(r)) + position,
        ];

        pos_dims.map(|position| Vertex {
            position,
            size,
            fill_color: i32::from_ne_bytes(fill_color.to_le_bytes()),
            stroke_color: i32::from_ne_bytes(stroke_color.to_le_bytes()),
            border_radius,
            border_width,
            intensity,
        })
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
    fill_color: i32,
    stroke_color: i32,
    border_radius: f32,
    border_width: f32,
    intensity: f32,
}
