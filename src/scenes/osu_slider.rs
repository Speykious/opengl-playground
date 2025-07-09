use std::mem::{self, offset_of};

use gl::types::{GLfloat, GLint, GLsizei, GLsizeiptr, GLuint};
use glam::{vec2, Mat4, Vec2, Vec3};
use winit::window::Window;

use crate::scenes::osu_slider::slider_path::{SliderCurveType, SliderPath};
use crate::scenes::{SRC_FRAG_SLIDER, SRC_FRAG_SLIDER_POINT, SRC_VERT_SLIDER};
use crate::{camera::Camera, common_gl::create_shader_program};

mod path_approx;
mod path_draw;
mod slider_path;

struct SliderVertices {
    ctrl: Vec<Vertex>,
    path: Vec<Vertex>,
    body: Vec<Vertex>,
    radius: f32,
}

impl SliderVertices {
    fn from_slider_path(slider: &SliderPath) -> Self {
        let ctrl = (slider.control_points.iter())
            .map(|cp| {
                let uv = match cp.curve_type {
                    SliderCurveType::Inherit => Vec2::ONE,
                    SliderCurveType::Bezier => vec2(0.0, 1.0),
                    SliderCurveType::Catmull => vec2(0.5, 0.5),
                    SliderCurveType::Linear => vec2(1.0, 0.0),
                    SliderCurveType::PerfectCircle => vec2(1.0, 0.5),
                };

                Vertex::new(cp.vec2().extend(0.0), uv)
            })
            .collect::<Vec<_>>();

        let (calculated_path, _length) = slider.calculate_path_and_length().unwrap();
        // let path = SliderPath::path_to_progress(&calculated_path, &calculated_length, 0.0, 1.0);

        let path_len = calculated_path.points.len();
        let path = (calculated_path.points.iter().enumerate())
            .map(|(i, &p)| {
                let uv = if i < 1 {
                    Vec2::ZERO
                } else if i >= path_len - 1 {
                    Vec2::ONE
                } else {
                    let v = i as f32 / path_len as f32;
                    vec2(v, 1.0 - v)
                };

                Vertex::new(p.extend(0.0), uv)
            })
            .collect::<Vec<_>>();

        let body = path_draw::generate_slider_vertices(&calculated_path.points, slider.radius);

        Self {
            ctrl,
            path,
            body,
            radius: slider.radius,
        }
    }
}

pub struct OsuSliderScene {
    matrix: Mat4,
    viewport: Vec2,

    sliders: Vec<SliderVertices>,

    slider_shader: GLuint,
    u_mvp_slider: GLint,

    slider_point_shader: GLuint,
    u_mvp_slider_point: GLint,

    path_vao: GLuint,
    path_vbo: GLuint,

    ctrl_vao: GLuint,
    ctrl_vbo: GLuint,

    body_vao: GLuint,
    body_vbo: GLuint,
}

impl OsuSliderScene {
    pub fn new(window: &Window) -> Self {
        let radius = 32.0;

        let sliders = [
            // sliders::tsd(radius),
            // sliders::good_random(radius),
            // sliders::euro(radius),
            sliders::pattern::a(radius),
            sliders::pattern::b(radius),
            sliders::pattern::c(radius),
            sliders::pattern::d(radius),
        ]
        .iter()
        .map(SliderVertices::from_slider_path)
        .collect::<Vec<_>>();

        unsafe {
            // Normal blending
            gl::Enable(gl::BLEND);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);

            let slider_shader = create_shader_program(SRC_VERT_SLIDER, SRC_FRAG_SLIDER);
            let u_mvp_slider = gl::GetUniformLocation(slider_shader, c"u_mvp".as_ptr());

            let slider_point_shader = create_shader_program(SRC_VERT_SLIDER, SRC_FRAG_SLIDER_POINT);
            let u_mvp_slider_point = gl::GetUniformLocation(slider_point_shader, c"u_mvp".as_ptr());

            let mut ctrl_vbo: u32 = 0;
            gl::GenBuffers(1, &mut ctrl_vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, ctrl_vbo);

            let mut ctrl_vao: u32 = 0;
            gl::GenVertexArrays(1, &mut ctrl_vao);
            gl::BindVertexArray(ctrl_vao);
            Self::setup_vao(slider_point_shader);

            let mut path_vbo: u32 = 0;
            gl::GenBuffers(1, &mut path_vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, path_vbo);

            let mut path_vao: u32 = 0;
            gl::GenVertexArrays(1, &mut path_vao);
            gl::BindVertexArray(path_vao);
            Self::setup_vao(slider_point_shader);

            let mut body_vbo: u32 = 0;
            gl::GenBuffers(1, &mut body_vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, body_vbo);

            let mut body_vao: u32 = 0;
            gl::GenVertexArrays(1, &mut body_vao);
            gl::BindVertexArray(body_vao);
            Self::setup_vao(slider_shader);

            let win_size = window.inner_size();
            let viewport = Vec2::new(win_size.width as f32, win_size.height as f32);

            Self {
                matrix: Mat4::default(),
                viewport,

                sliders,

                slider_shader,
                u_mvp_slider,

                slider_point_shader,
                u_mvp_slider_point,

                path_vao,
                path_vbo,

                ctrl_vao,
                ctrl_vbo,

                body_vao,
                body_vbo,

            }
        }
    }

    fn setup_vao(shader: GLuint) {
        #[rustfmt::skip]
        unsafe {
            let size_vertex = mem::size_of::<Vertex>() as GLsizei;

            let a_position = gl::GetAttribLocation(shader, c"position" .as_ptr()) as GLuint;
            let a_uv       = gl::GetAttribLocation(shader, c"uv"       .as_ptr()) as GLuint;

            gl::VertexAttribPointer(a_position, 3, gl::FLOAT, gl::FALSE, size_vertex, offset_of!(Vertex, position) as _);
            gl::VertexAttribPointer(a_uv,       2, gl::FLOAT, gl::FALSE, size_vertex, offset_of!(Vertex, uv)       as _);

            gl::EnableVertexAttribArray(a_position as GLuint);
            gl::EnableVertexAttribArray(a_uv       as GLuint);
        };
    }

    pub fn draw(&mut self, camera: &Camera, mouse_pos: Vec2) {
        self.draw_with_clear_color(0.0, 0.0, 0.0, 1.0);
    }

    fn draw_slider_body(&self, slider: &SliderVertices) {
        unsafe {
            gl::BindVertexArray(self.body_vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.body_vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                mem::size_of_val(slider.body.as_slice()) as GLsizeiptr,
                slider.body.as_slice().as_ptr() as *const _,
                gl::DYNAMIC_DRAW,
            );

            gl::UseProgram(self.slider_shader);

            // Depth testing for sliders
            gl::BindVertexArray(self.body_vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.body_vbo);
            gl::DrawArrays(gl::TRIANGLES, 0, slider.body.len() as GLsizei);
        }
    }

    fn draw_slider_debug(&self, slider: &SliderVertices) {
        unsafe {
            gl::BindVertexArray(self.path_vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.path_vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                mem::size_of_val(slider.path.as_slice()) as GLsizeiptr,
                slider.path.as_slice().as_ptr() as *const _,
                gl::DYNAMIC_DRAW,
            );

            gl::BindVertexArray(self.ctrl_vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.ctrl_vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                mem::size_of_val(slider.ctrl.as_slice()) as GLsizeiptr,
                slider.ctrl.as_slice().as_ptr() as *const _,
                gl::DYNAMIC_DRAW,
            );

            gl::UseProgram(self.slider_point_shader);

            gl::BindVertexArray(self.path_vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.path_vbo);
            gl::PointSize(5.0);
            gl::DrawArrays(gl::LINE_STRIP, 0, slider.path.len() as GLsizei);
            gl::DrawArrays(gl::POINTS, 0, slider.path.len() as GLsizei);

            gl::BindVertexArray(self.ctrl_vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.ctrl_vbo);
            gl::PointSize(10.0);
            gl::DrawArrays(gl::LINE_STRIP, 0, slider.ctrl.len() as GLsizei);
            gl::DrawArrays(gl::POINTS, 0, slider.ctrl.len() as GLsizei);
        }
    }

    fn draw_with_clear_color(&self, r: GLfloat, g: GLfloat, b: GLfloat, a: GLfloat) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);

            gl::ClearColor(r, g, b, a);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            gl::Enable(gl::DEPTH_TEST);
            gl::DepthFunc(gl::LESS);
            for slider in &self.sliders {
                gl::Clear(gl::DEPTH_BUFFER_BIT);
                self.draw_slider_body(slider);
            }
            gl::Disable(gl::DEPTH_TEST);

            for slider in &self.sliders {
                self.draw_slider_debug(slider);
            }
        }
    }

    pub fn resize(&mut self, camera: &Camera, width: i32, height: i32) {
        unsafe {
            gl::Viewport(0, 0, width, height);

            self.viewport = Vec2::new(width as f32, height as f32);
            self.matrix = camera.matrix(self.viewport);

            gl::UseProgram(self.slider_shader);
            gl::UniformMatrix4fv(self.u_mvp_slider, 1, gl::FALSE, self.matrix.as_ref().as_ptr());

            gl::UseProgram(self.slider_point_shader);
            gl::UniformMatrix4fv(self.u_mvp_slider_point, 1, gl::FALSE, self.matrix.as_ref().as_ptr());
        }
    }
}

impl Drop for OsuSliderScene {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.slider_shader);
            gl::DeleteProgram(self.slider_point_shader);

            let vaos = &[self.path_vao, self.ctrl_vao, self.body_vao];
            gl::DeleteVertexArrays(vaos.len() as GLsizei, vaos.as_ptr());

            let vbos = &[self.path_vbo, self.ctrl_vbo, self.body_vbo];
            gl::DeleteBuffers(vbos.len() as GLsizei, vbos.as_ptr());
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct Vertex {
    pub position: Vec3,
    pub uv: Vec2,
}

impl Vertex {
    pub const fn new(position: Vec3, uv: Vec2) -> Self {
        Self { position, uv }
    }
}

mod sliders {
    use crate::scenes::osu_slider::slider_path::{SliderControlPoint, SliderCurveType, SliderPath};

    use SliderControlPoint as Scp;
    use SliderCurveType as Sct;

    /// Good random slider
    ///
    /// ```ignore
    /// 314,167,692156,6,0,B|299:147|299:147|350:107|421:141|430:207|379:192|423:257|448:266|486:275|486:275|333:221|238:349|238:349|194:337|177:272|210:224|220:268|251:208|224:175,1,769.50000293541
    /// ```
    pub fn good_random(radius: f32) -> SliderPath {
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
            radius,
        }
    }

    /// €
    ///
    /// ```ignore
    /// 285,78,702227,6,0,B|293:56|293:56|201:27|133:105|144:163|144:163|110:169|110:169|107:174|107:174|220:154|220:154|224:149|224:149|144:163|144:163|153:220|153:220|116:226|116:226|113:231|113:231|225:207|225:207|228:202|228:202|153:220|153:220|165:276|281:331|357:234,1,1025.99996868897
    /// ```
    pub fn euro(radius: f32) -> SliderPath {
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
            radius,
        }
    }

    // How does it look for a time-series-like thing?
    pub fn tsd(radius: f32) -> SliderPath {
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
            radius,
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

        pub fn a(radius: f32) -> SliderPath {
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
                radius,
            }
        }

        pub fn b(radius: f32) -> SliderPath {
            SliderPath {
                control_points: vec![
                    Scp::new(Sct::Linear, 0.0, 211.0),
                    Scp::new(Sct::Inherit, 111.0, 189.0),
                ],
                length: 100.0,
                radius,
            }
        }

        pub fn c(radius: f32) -> SliderPath {
            SliderPath {
                control_points: vec![
                    Scp::new(Sct::Linear, 204.0, 174.0),
                    Scp::new(Sct::Inherit, 307.0, 150.0),
                    Scp::new(Sct::PerfectCircle, 294.0, 240.0),
                    Scp::new(Sct::Inherit, 355.0, 293.0),
                    Scp::new(Sct::Inherit, 387.0, 240.0),
                ],
                length: 350.0,
                radius,
            }
        }

        pub fn d(radius: f32) -> SliderPath {
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
                radius,
            }
        }
    }
}
