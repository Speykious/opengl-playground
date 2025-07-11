use std::mem::{self, offset_of};
use std::rc::Rc;
use std::time::SystemTime;

use glam::{Mat4, Vec2, Vec4, ivec2, vec2};
use glow::HasContext;
use image::{EncodableLayout, ImageFormat};
use winit::window::Window;

use crate::common::{Framebuffer, TextureWrapping, create_framebuffer, slice_as_bytes, upload_texture};
use crate::scenes::osu_slider::slider_path::{SliderCurveType, SliderPath};
use crate::scenes::{
    SLIDERBODY3_PNG, SRC_FRAG_SLIDER, SRC_FRAG_SLIDER_POINT, SRC_FRAG_TEXTURE, SRC_VERT_SCREEN, SRC_VERT_SLIDER,
};
use crate::{camera::Camera, common::create_shader_program};

mod path_approx;
mod path_draw;
mod slider_path;

struct SliderVertices {
    ctrl: Vec<SliderVertex>,
    path: Vec<SliderVertex>,
    body: Vec<SliderVertex>,
}

impl SliderVertices {
    fn from_slider_path(slider: &SliderPath, radius: f32, p0: f64, p1: f64) -> Self {
        let ctrl = (slider.control_points.iter())
            .map(|cp| {
                let dist = match cp.curve_type {
                    SliderCurveType::Inherit => 1.0,
                    SliderCurveType::Bezier => 0.75,
                    SliderCurveType::Catmull => 0.5,
                    SliderCurveType::Linear => 0.25,
                    SliderCurveType::PerfectCircle => 0.0,
                };

                SliderVertex::new(cp.vec2().extend(0.0).extend(dist))
            })
            .collect::<Vec<_>>();

        let (calculated_path, calculated_length) = slider.calculate_path_and_length().unwrap();
        let progress_path = SliderPath::path_to_progress(&calculated_path, &calculated_length, p0, p1);

        let path = (progress_path.iter().enumerate())
            .map(|(i, &p)| {
                let dist = match () {
                    _ if i < 1 => 0.0,
                    _ if i >= progress_path.len() - 1 => 1.0,
                    _ => i as f32 / progress_path.len() as f32,
                };

                SliderVertex::new(p.extend(0.0).extend(dist))
            })
            .collect::<Vec<_>>();

        let body = path_draw::generate_slider_vertices(&progress_path, radius);

        Self { ctrl, path, body }
    }
}

pub struct OsuSliderScene {
    gl: Rc<glow::Context>,

    matrix: Mat4,
    viewport: Vec2,

    slider_radius: f32,

    slider_fb: Framebuffer,
    slider_paths: Vec<SliderPath>,
    slider_vertices: Vec<SliderVertices>,

    screen_shader: glow::Program,
    screen_vao: glow::VertexArray,
    screen_vbo: glow::Buffer,

    slider_shader: glow::Program,
    u_slider_mvp: glow::UniformLocation,
    u_slider_border_color: glow::UniformLocation,
    u_slider_border_width: glow::UniformLocation,
    u_slider_radius: glow::UniformLocation,
    u_slider_texture_progress: glow::UniformLocation,
    sliderbody_texture: glow::Texture,

    slider_point_shader: glow::Program,
    u_slider_point_mvp: glow::UniformLocation,
    u_slider_point_is_solid: glow::UniformLocation,
    u_slider_point_solid_color: glow::UniformLocation,

    path_vao: glow::VertexArray,
    path_vbo: glow::Buffer,

    ctrl_vao: glow::VertexArray,
    ctrl_vbo: glow::Buffer,

    body_vao: glow::VertexArray,
    body_vbo: glow::Buffer,
}

impl OsuSliderScene {
    pub fn new(gl: Rc<glow::Context>, window: &Window) -> Self {
        let slider_radius = 32.0;

        let slider_paths = vec![
            sliders::pattern::a(),
            sliders::pattern::b(),
            sliders::pattern::c(),
            sliders::pattern::d(),
            // sliders::catmull(),

            // sliders::tsd(),
            // sliders::good_random(),
            sliders::euro(),
        ];

        let slider_vertices = (slider_paths.iter())
            .map(|sp| SliderVertices::from_slider_path(sp, slider_radius, 0.0, 1.0))
            .collect::<Vec<_>>();

        let sliderbody_texture = unsafe {
            // Sliderbody texture
            let sliderbody = image::load_from_memory_with_format(SLIDERBODY3_PNG, ImageFormat::Png);
            let sliderbody = sliderbody.unwrap().into_rgba8();

            let sliderbody_texture = gl.create_texture().unwrap();
            upload_texture(
                &gl,
                sliderbody_texture,
                sliderbody.width(),
                sliderbody.height(),
                Some(sliderbody.as_bytes()),
                TextureWrapping::Repeat,
            );

            sliderbody_texture
        };

        unsafe {
            Self::blend(&gl, true);

            let screen_shader = create_shader_program(&gl, SRC_VERT_SCREEN, SRC_FRAG_TEXTURE);

            let slider_shader = create_shader_program(&gl, SRC_VERT_SLIDER, SRC_FRAG_SLIDER);
            let u_slider_mvp = gl.get_uniform_location(slider_shader, "u_mvp").unwrap();
            let u_slider_border_color = gl.get_uniform_location(slider_shader, "u_border_color").unwrap();
            let u_slider_border_width = gl.get_uniform_location(slider_shader, "u_border_width").unwrap();
            let u_slider_radius = gl.get_uniform_location(slider_shader, "u_radius").unwrap();
            let u_slider_texture_progress = gl.get_uniform_location(slider_shader, "u_texture_progress").unwrap();

            let slider_point_shader = create_shader_program(&gl, SRC_VERT_SLIDER, SRC_FRAG_SLIDER_POINT);
            let u_slider_point_mvp = gl.get_uniform_location(slider_point_shader, "u_mvp").unwrap();
            let u_slider_point_is_solid = gl.get_uniform_location(slider_point_shader, "u_is_solid").unwrap();
            let u_slider_point_solid_color = gl.get_uniform_location(slider_point_shader, "u_solid_color").unwrap();

            // screen vertices
            let screen_vbo = gl.create_buffer().unwrap();
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(screen_vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, slice_as_bytes(SCREEN_VERTICES), glow::STATIC_DRAW);

            let screen_vao = gl.create_vertex_array().unwrap();
            gl.bind_vertex_array(Some(screen_vao));
            Self::setup_screen_vao(&gl, screen_shader);

            // slider ctrl points
            let ctrl_vbo = gl.create_buffer().unwrap();
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(ctrl_vbo));

            let ctrl_vao = gl.create_vertex_array().unwrap();
            gl.bind_vertex_array(Some(ctrl_vao));
            Self::setup_slider_vao(&gl, slider_point_shader);

            // slider path points
            let path_vbo = gl.create_buffer().unwrap();
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(path_vbo));

            let path_vao = gl.create_vertex_array().unwrap();
            gl.bind_vertex_array(Some(path_vao));
            Self::setup_slider_vao(&gl, slider_point_shader);

            // slider body mesh
            let body_vbo = gl.create_buffer().unwrap();
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(body_vbo));

            let body_vao = gl.create_vertex_array().unwrap();
            gl.bind_vertex_array(Some(body_vao));
            Self::setup_slider_vao(&gl, slider_shader);

            let win_size = window.inner_size();
            let viewport = Vec2::new(win_size.width as f32, win_size.height as f32);

            let slider_fb = create_framebuffer(
                &gl,
                "Slider Framebuffer",
                viewport.as_uvec2(),
                TextureWrapping::ClampToBorder,
                true,
            );

            Self {
                gl,

                matrix: Mat4::default(),
                viewport,

                slider_radius,

                slider_fb,
                slider_paths,
                slider_vertices,

                screen_shader,
                screen_vao,
                screen_vbo,

                slider_shader,
                u_slider_mvp,
                u_slider_border_color,
                u_slider_border_width,
                u_slider_radius,
                u_slider_texture_progress,
                sliderbody_texture,

                slider_point_shader,
                u_slider_point_mvp,
                u_slider_point_is_solid,
                u_slider_point_solid_color,

                path_vao,
                path_vbo,

                ctrl_vao,
                ctrl_vbo,

                body_vao,
                body_vbo,
            }
        }
    }

    fn blend(gl: &glow::Context, enabled: bool) {
        unsafe {
            if enabled {
                // Normal blending
                gl.enable(glow::BLEND);
                gl.blend_func_separate(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA, glow::SRC_ALPHA, glow::ONE);
                gl.blend_equation_separate(glow::FUNC_ADD, glow::FUNC_ADD);
            } else {
                gl.disable(glow::BLEND);
            }
        }
    }

    fn setup_screen_vao(gl: &glow::Context, shader: glow::Program) {
        #[rustfmt::skip]
        unsafe {
            let size_vertex = mem::size_of::<Vertex>() as i32;

            let a_position = gl.get_attrib_location(shader, "position").unwrap();
            let a_uv       = gl.get_attrib_location(shader, "uv"      ).unwrap();

            gl.vertex_attrib_pointer_f32(a_position, 2, glow::FLOAT, false, size_vertex, offset_of!(Vertex, position) as _);
            gl.vertex_attrib_pointer_f32(a_uv,       2, glow::FLOAT, false, size_vertex, offset_of!(Vertex, uv)       as _);

            gl.enable_vertex_attrib_array(a_position);
            gl.enable_vertex_attrib_array(a_uv);
        };
    }

    fn setup_slider_vao(gl: &glow::Context, shader: glow::Program) {
        #[rustfmt::skip]
        unsafe {
            let size_vertex = mem::size_of::<SliderVertex>() as i32;
            let a_position = gl.get_attrib_location(shader, "position").unwrap();
            gl.vertex_attrib_pointer_f32(a_position, 4, glow::FLOAT, false, size_vertex, offset_of!(SliderVertex, position) as _);
            gl.enable_vertex_attrib_array(a_position);
        };
    }

    pub fn draw(&mut self, camera: &Camera, mouse_pos: Vec2) {
        let p = (mouse_pos / self.viewport - 0.5) / 0.727 + 0.5;

        let millis = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let q = ((millis % 1000) as u32) as f32 / 1000.0;

        unsafe {
            self.gl.use_program(Some(self.slider_shader));
            self.gl.uniform_1_f32(Some(&self.u_slider_texture_progress), q);
        }

        self.slider_vertices = (self.slider_paths.iter())
            .map(|sp| SliderVertices::from_slider_path(sp, self.slider_radius, 0.0, p.x as f64))
            .collect::<Vec<_>>();

        self.draw_with_clear_color(0.0, 0.0, 0.0, 0.5);
    }

    fn draw_slider_body(&self, slider: &SliderVertices) {
        let gl = &self.gl;

        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.slider_fb.fbo));
            Self::blend(&self.gl, false);
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS);
            gl.clear_color(0.0, 0.0, 0.0, 0.0);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);

            gl.bind_vertex_array(Some(self.body_vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.body_vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                slice_as_bytes(slider.body.as_slice()),
                glow::DYNAMIC_DRAW,
            );

            gl.use_program(Some(self.slider_shader));

            // Depth testing for sliders
            gl.bind_vertex_array(Some(self.body_vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.body_vbo));
            gl.bind_texture(glow::TEXTURE_2D, Some(self.sliderbody_texture));
            gl.draw_arrays(glow::TRIANGLES, 0, slider.body.len() as i32);

            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            Self::blend(&self.gl, true);
            gl.disable(glow::DEPTH_TEST);

            gl.use_program(Some(self.screen_shader));

            gl.bind_vertex_array(Some(self.screen_vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.screen_vbo));
            gl.bind_texture(glow::TEXTURE_2D, Some(self.slider_fb.texture));
            gl.active_texture(glow::TEXTURE0);

            gl.draw_arrays(glow::TRIANGLES, 0, 6);

            gl.bind_texture(glow::TEXTURE_2D, None);
        }
    }

    fn draw_slider_debug(&self, slider: &SliderVertices) {
        let gl = &self.gl;

        unsafe {
            gl.bind_vertex_array(Some(self.path_vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.path_vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                slice_as_bytes(slider.path.as_slice()),
                glow::DYNAMIC_DRAW,
            );

            gl.bind_vertex_array(Some(self.ctrl_vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.ctrl_vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                slice_as_bytes(slider.ctrl.as_slice()),
                glow::DYNAMIC_DRAW,
            );

            gl.use_program(Some(self.slider_point_shader));

            gl.bind_vertex_array(Some(self.path_vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.path_vbo));
            // gl.point_size(5.0);

            gl.uniform_1_i32(Some(&self.u_slider_point_is_solid), 1);
            gl.draw_arrays(glow::LINE_STRIP, 0, slider.path.len() as i32);
            gl.uniform_1_i32(Some(&self.u_slider_point_is_solid), 0);
            gl.draw_arrays(glow::POINTS, 0, slider.path.len() as i32);

            gl.bind_vertex_array(Some(self.ctrl_vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.ctrl_vbo));
            // gl.point_size(10.0);
            gl.uniform_1_i32(Some(&self.u_slider_point_is_solid), 1);
            gl.draw_arrays(glow::LINE_STRIP, 0, slider.ctrl.len() as i32);
            gl.uniform_1_i32(Some(&self.u_slider_point_is_solid), 0);
            gl.draw_arrays(glow::POINTS, 0, slider.ctrl.len() as i32);
        }
    }

    fn draw_with_clear_color(&self, r: f32, g: f32, b: f32, a: f32) {
        let gl = &self.gl;

        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);

            gl.clear_color(r, g, b, a);
            gl.clear(glow::COLOR_BUFFER_BIT);

            gl.use_program(Some(self.slider_shader));
            gl.uniform_1_f32(Some(&self.u_slider_radius), self.slider_radius);
            gl.uniform_4_f32(Some(&self.u_slider_border_color), 1.0, 0.2, 0.2, 1.0);
            gl.uniform_1_f32(Some(&self.u_slider_border_width), 7.27);

            for slider in &self.slider_vertices {
                self.draw_slider_body(slider);
                self.draw_slider_debug(slider);
            }
        }
    }

    pub fn resize(&mut self, camera: &Camera, width: i32, height: i32) {
        let gl = &self.gl;

        let viewport = ivec2(width, height);

        unsafe {
            gl.viewport(0, 0, width, height);

            self.slider_fb.resize(gl, viewport.as_uvec2());

            self.viewport = viewport.as_vec2();
            self.matrix = camera.matrix(self.viewport);

            gl.use_program(Some(self.slider_shader));
            gl.uniform_matrix_4_f32_slice(Some(&self.u_slider_mvp), false, self.matrix.as_ref());

            gl.use_program(Some(self.slider_point_shader));
            gl.uniform_matrix_4_f32_slice(Some(&self.u_slider_point_mvp), false, self.matrix.as_ref());
        }
    }
}

impl Drop for OsuSliderScene {
    fn drop(&mut self) {
        let gl = &self.gl;

        unsafe {
            gl.delete_program(self.slider_shader);
            gl.delete_program(self.slider_point_shader);

            gl.delete_vertex_array(self.path_vao);
            gl.delete_vertex_array(self.ctrl_vao);
            gl.delete_vertex_array(self.body_vao);

            gl.delete_buffer(self.path_vbo);
            gl.delete_buffer(self.ctrl_vbo);
            gl.delete_buffer(self.body_vbo);
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct SliderVertex {
    pub position: Vec4,
}

impl SliderVertex {
    pub const fn new(position: Vec4) -> Self {
        Self { position }
    }
}
/// Vertex used for the screen framebuffer.
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

mod sliders {
    use crate::scenes::osu_slider::slider_path::{SliderControlPoint, SliderCurveType, SliderPath};

    use SliderControlPoint as Scp;
    use SliderCurveType as Sct;

    /// Good random slider
    ///
    /// ```ignore
    /// 314,167,692156,6,0,B|299:147|299:147|350:107|421:141|430:207|379:192|423:257|448:266|486:275|486:275|333:221|238:349|238:349|194:337|177:272|210:224|220:268|251:208|224:175,1,769.50000293541
    /// ```
    pub fn good_random() -> SliderPath {
        SliderPath {
            control_points: vec![
                Scp::new(Sct::Bezier, 314.0, 167.0),
                Scp::new(Sct::Bezier, 299.0, 147.0),
                Scp::new(Sct::Inherit, 350.0, 107.0),
                Scp::new(Sct::Inherit, 421.0, 141.0),
                Scp::new(Sct::Inherit, 430.0, 207.0),
                Scp::new(Sct::Inherit, 379.0, 192.0),
                Scp::new(Sct::Inherit, 423.0, 257.0),
                Scp::new(Sct::Inherit, 448.0, 266.0),
                Scp::new(Sct::Bezier, 486.0, 275.0),
                Scp::new(Sct::Inherit, 333.0, 221.0),
                Scp::new(Sct::Bezier, 238.0, 349.0),
                Scp::new(Sct::Inherit, 194.0, 337.0),
                Scp::new(Sct::Inherit, 177.0, 272.0),
                Scp::new(Sct::Inherit, 210.0, 224.0),
                Scp::new(Sct::Inherit, 220.0, 268.0),
                Scp::new(Sct::Inherit, 251.0, 208.0),
                Scp::new(Sct::Inherit, 224.0, 175.0),
            ],
            length: 769.50000293541,
        }
    }

    /// €
    ///
    /// ```ignore
    /// 285,78,702227,6,0,B|293:56|293:56|201:27|133:105|144:163|144:163|110:169|110:169|107:174|107:174|220:154|220:154|224:149|224:149|144:163|144:163|153:220|153:220|116:226|116:226|113:231|113:231|225:207|225:207|228:202|228:202|153:220|153:220|165:276|281:331|357:234,1,1025.99996868897
    /// ```
    pub fn euro() -> SliderPath {
        SliderPath {
            control_points: vec![
                Scp::new(Sct::Bezier, 285.0, 78.0),
                Scp::new(Sct::Bezier, 293.0, 56.0),
                Scp::new(Sct::Inherit, 201.0, 27.0),
                Scp::new(Sct::Inherit, 133.0, 105.0),
                Scp::new(Sct::Bezier, 144.0, 163.0),
                Scp::new(Sct::Bezier, 110.0, 169.0),
                Scp::new(Sct::Bezier, 107.0, 174.0),
                Scp::new(Sct::Bezier, 220.0, 154.0),
                Scp::new(Sct::Bezier, 224.0, 149.0),
                Scp::new(Sct::Bezier, 144.0, 163.0),
                Scp::new(Sct::Bezier, 153.0, 220.0),
                Scp::new(Sct::Bezier, 116.0, 226.0),
                Scp::new(Sct::Bezier, 113.0, 231.0),
                Scp::new(Sct::Bezier, 225.0, 207.0),
                Scp::new(Sct::Bezier, 228.0, 202.0),
                Scp::new(Sct::Bezier, 153.0, 220.0),
                Scp::new(Sct::Inherit, 165.0, 276.0),
                Scp::new(Sct::Inherit, 281.0, 331.0),
                Scp::new(Sct::Inherit, 357.0, 234.0),
            ],
            length: 1025.99996868897,
        }
    }

    // How does it look for a time-series-like thing?
    pub fn tsd() -> SliderPath {
        SliderPath {
            control_points: vec![
                Scp::new(Sct::Linear, 0.0, 0.0),
                Scp::new(Sct::Inherit, 5.0, 200.0),
                Scp::new(Sct::Inherit, 10.0, 10.0),
                Scp::new(Sct::Inherit, 15.0, 190.0),
                Scp::new(Sct::Inherit, 20.0, 20.0),
                Scp::new(Sct::Inherit, 25.0, 180.0),
                Scp::new(Sct::Inherit, 30.0, 30.0),
                Scp::new(Sct::Inherit, 35.0, 170.0),
                Scp::new(Sct::Inherit, 40.0, 40.0),
                Scp::new(Sct::Inherit, 45.0, 160.0),
                Scp::new(Sct::Inherit, 50.0, 50.0),
                Scp::new(Sct::Inherit, 55.0, 150.0),
                Scp::new(Sct::Inherit, 60.0, 60.0),
                Scp::new(Sct::Inherit, 65.0, 140.0),
                Scp::new(Sct::Inherit, 70.0, 70.0),
                Scp::new(Sct::Inherit, 75.0, 130.0),
                Scp::new(Sct::Inherit, 80.0, 80.0),
                Scp::new(Sct::Inherit, 85.0, 120.0),
                Scp::new(Sct::Inherit, 90.0, 90.0),
                Scp::new(Sct::Inherit, 95.0, 110.0),
                Scp::new(Sct::Inherit, 100.0, 100.0),
            ],
            length: 1025.99996868897,
        }
    }

    pub fn catmull() -> SliderPath {
        SliderPath {
            control_points: vec![
                Scp::new(Sct::Catmull, 404.0, 157.0),
                Scp::new(Sct::Inherit, 412.0, 081.0),
                Scp::new(Sct::Inherit, 457.0, 039.0),
                Scp::new(Sct::Linear, 507.0, 093.0),
                Scp::new(Sct::Catmull, 486.0, 260.0),
                Scp::new(Sct::Inherit, 340.0, 392.0),
                Scp::new(Sct::Inherit, 198.0, 264.0),
            ],
            length: 800.0,
        }
    }

    /// Pattern of mine
    ///
    /// ```ignore
    /// 117,265,44667,2,0,P|133:250|L|148:212|P|161:120|118:73|68:121,1,300,0|0,2:0|2:0,2:0:0:0:
    /// 0,211,45333,6,0,L|111:189,1,100,0|0,2:0|2:0,2:0:0:0:
    /// 204,174,45667,2,0,L|307:150|P|294:240|355:293|387:240,1,350,0|0,2:0|2:0,2:0:0:0:
    /// 404,157,46333,2,0,L|P|412:81|457:39|L|507:93|P|486:260|340:392|198:264,1,800,0|0,2:0|2:0,2:0:0:0:
    /// ```
    pub mod pattern {
        use super::*;

        pub fn a() -> SliderPath {
            SliderPath {
                control_points: vec![
                    Scp::new(Sct::PerfectCircle, 117.0, 265.0),
                    Scp::new(Sct::Inherit, 133.0, 250.0),
                    Scp::new(Sct::Linear, 148.0, 212.0),
                    Scp::new(Sct::PerfectCircle, 161.0, 120.0),
                    Scp::new(Sct::Inherit, 118.0, 073.0),
                    Scp::new(Sct::Inherit, 68.0, 121.0),
                ],
                length: 300.0,
            }
        }

        pub fn b() -> SliderPath {
            SliderPath {
                control_points: vec![Scp::new(Sct::Linear, 0.0, 211.0), Scp::new(Sct::Inherit, 111.0, 189.0)],
                length: 100.0,
            }
        }

        pub fn c() -> SliderPath {
            SliderPath {
                control_points: vec![
                    Scp::new(Sct::Linear, 204.0, 174.0),
                    Scp::new(Sct::Inherit, 307.0, 150.0),
                    Scp::new(Sct::PerfectCircle, 294.0, 240.0),
                    Scp::new(Sct::Inherit, 355.0, 293.0),
                    Scp::new(Sct::Inherit, 387.0, 240.0),
                ],
                length: 350.0,
            }
        }

        pub fn d() -> SliderPath {
            SliderPath {
                control_points: vec![
                    Scp::new(Sct::Linear, 404.0, 157.0),
                    Scp::new(Sct::PerfectCircle, 412.0, 081.0),
                    Scp::new(Sct::Inherit, 457.0, 039.0),
                    Scp::new(Sct::Linear, 507.0, 093.0),
                    Scp::new(Sct::PerfectCircle, 486.0, 260.0),
                    Scp::new(Sct::Inherit, 340.0, 392.0),
                    Scp::new(Sct::Inherit, 198.0, 264.0),
                ],
                length: 800.0,
            }
        }
    }
}
