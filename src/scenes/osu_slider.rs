use std::mem::{self, offset_of};
use std::time::{Instant, SystemTime};

use gl::types::{GLfloat, GLint, GLsizei, GLsizeiptr, GLuint};
use glam::{ivec2, vec2, Mat4, Vec2, Vec4};
use image::ImageFormat;
use winit::window::Window;

use crate::common_gl::{create_framebuffer, upload_texture, Framebuffer};
use crate::scenes::osu_slider::slider_path::{SliderCurveType, SliderPath};
use crate::scenes::{
    SLIDERBODY1_PNG, SLIDERBODY3_PNG, SRC_FRAG_SLIDER, SRC_FRAG_SLIDER_POINT, SRC_FRAG_TEXTURE, SRC_VERT_SCREEN, SRC_VERT_SLIDER
};
use crate::{camera::Camera, common_gl::create_shader_program};

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
        let progress_path =
            SliderPath::path_to_progress(&calculated_path, &calculated_length, p0, p1);

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
    matrix: Mat4,
    viewport: Vec2,

    slider_radius: f32,

    slider_fb: Framebuffer,
    slider_paths: Vec<SliderPath>,
    slider_vertices: Vec<SliderVertices>,

    screen_shader: GLuint,
    screen_vao: GLuint,
    screen_vbo: GLuint,

    slider_shader: GLuint,
    u_slider_mvp: GLint,
    u_slider_border_color: GLint,
    u_slider_border_width: GLint,
    u_slider_radius: GLint,
    u_slider_texture_progress: GLint,
    sliderbody_texture: GLuint,

    slider_point_shader: GLuint,
    u_slider_point_mvp: GLint,
    u_slider_point_is_solid: GLint,
    u_slider_point_solid_color: GLint,

    path_vao: GLuint,
    path_vbo: GLuint,

    ctrl_vao: GLuint,
    ctrl_vbo: GLuint,

    body_vao: GLuint,
    body_vbo: GLuint,
}

impl OsuSliderScene {
    pub fn new(window: &Window) -> Self {
        let slider_radius = 32.0;

        let slider_paths = vec![
            sliders::pattern::a(),
            sliders::pattern::b(),
            sliders::pattern::c(),
            sliders::pattern::d(),
            // sliders::catmull(),

            // sliders::tsd(),
            // sliders::good_random(),
            // sliders::euro(),
        ];

        let slider_vertices = (slider_paths.iter())
            .map(|sp| SliderVertices::from_slider_path(sp, slider_radius, 0.0, 1.0))
            .collect::<Vec<_>>();

        let sliderbody_texture = unsafe {
            // Sliderbody texture
            let sliderbody = image::load_from_memory_with_format(SLIDERBODY3_PNG, ImageFormat::Png);
            let sliderbody = sliderbody.unwrap().into_rgba8();

            let mut sliderbody_texture: GLuint = 0;
            gl::GenTextures(1, &mut sliderbody_texture);
            upload_texture(
                sliderbody_texture,
                sliderbody.width(),
                sliderbody.height(),
                sliderbody.as_ptr(),
                gl::REPEAT,
            );

            sliderbody_texture
        };

        unsafe {
            Self::blend(true);

            let screen_shader = create_shader_program(SRC_VERT_SCREEN, SRC_FRAG_TEXTURE);

            let slider_shader = create_shader_program(SRC_VERT_SLIDER, SRC_FRAG_SLIDER);
            let u_slider_mvp = gl::GetUniformLocation(slider_shader, c"u_mvp".as_ptr());
            let u_slider_border_color = gl::GetUniformLocation(slider_shader, c"u_border_color".as_ptr());
            let u_slider_border_width = gl::GetUniformLocation(slider_shader, c"u_border_width".as_ptr());
            let u_slider_radius = gl::GetUniformLocation(slider_shader, c"u_radius".as_ptr());
            let u_slider_texture_progress = gl::GetUniformLocation(slider_shader, c"u_texture_progress".as_ptr());

            let slider_point_shader = create_shader_program(SRC_VERT_SLIDER, SRC_FRAG_SLIDER_POINT);
            let u_slider_point_mvp = gl::GetUniformLocation(slider_point_shader, c"u_mvp".as_ptr());
            let u_slider_point_is_solid = gl::GetUniformLocation(slider_point_shader, c"u_is_solid".as_ptr());
            let u_slider_point_solid_color = gl::GetUniformLocation(slider_point_shader, c"u_solid_color".as_ptr());

            // screen vertices
            let mut screen_vbo: GLuint = 0;
            gl::GenBuffers(1, &mut screen_vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, screen_vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                mem::size_of_val(SCREEN_VERTICES) as GLsizeiptr,
                SCREEN_VERTICES.as_ptr() as *const _,
                gl::STATIC_DRAW,
            );

            let mut screen_vao: GLuint = 0;
            gl::GenVertexArrays(1, &mut screen_vao);
            gl::BindVertexArray(screen_vao);
            Self::setup_screen_vao(screen_shader);

            // slider ctrl points
            let mut ctrl_vbo: u32 = 0;
            gl::GenBuffers(1, &mut ctrl_vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, ctrl_vbo);

            let mut ctrl_vao: u32 = 0;
            gl::GenVertexArrays(1, &mut ctrl_vao);
            gl::BindVertexArray(ctrl_vao);
            Self::setup_slider_vao(slider_point_shader);

            // slider path points
            let mut path_vbo: u32 = 0;
            gl::GenBuffers(1, &mut path_vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, path_vbo);

            let mut path_vao: u32 = 0;
            gl::GenVertexArrays(1, &mut path_vao);
            gl::BindVertexArray(path_vao);
            Self::setup_slider_vao(slider_point_shader);

            // slider body mesh
            let mut body_vbo: u32 = 0;
            gl::GenBuffers(1, &mut body_vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, body_vbo);

            let mut body_vao: u32 = 0;
            gl::GenVertexArrays(1, &mut body_vao);
            gl::BindVertexArray(body_vao);
            Self::setup_slider_vao(slider_shader);

            let win_size = window.inner_size();
            let viewport = Vec2::new(win_size.width as f32, win_size.height as f32);

            let slider_fb = create_framebuffer("Slider Framebuffer", viewport.as_uvec2(), true);

            Self {
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

    fn blend(enabled: bool) {
        unsafe {
            if enabled {
                // Normal blending
                gl::Enable(gl::BLEND);
                gl::BlendFuncSeparate(
                    gl::SRC_ALPHA,
                    gl::ONE_MINUS_SRC_ALPHA,
                    gl::SRC_ALPHA,
                    gl::ONE,
                );
                gl::BlendEquationSeparate(gl::FUNC_ADD, gl::FUNC_ADD);
            } else {
                gl::Disable(gl::BLEND);
            }
        }
    }

    fn setup_screen_vao(shader: GLuint) {
        #[rustfmt::skip]
        unsafe {
            let size_vertex: GLsizei = mem::size_of::<Vertex>() as GLsizei;

            let a_position = gl::GetAttribLocation(shader, c"position" .as_ptr()) as GLuint;
            let a_uv       = gl::GetAttribLocation(shader, c"uv"       .as_ptr()) as GLuint;

            gl::VertexAttribPointer(a_position, 2, gl::FLOAT, gl::FALSE, size_vertex, offset_of!(Vertex, position) as _);
            gl::VertexAttribPointer(a_uv,       2, gl::FLOAT, gl::FALSE, size_vertex, offset_of!(Vertex, uv)       as _);

            gl::EnableVertexAttribArray(a_position as GLuint);
            gl::EnableVertexAttribArray(a_uv       as GLuint);
        };
    }

    fn setup_slider_vao(shader: GLuint) {
        #[rustfmt::skip]
        unsafe {
            let size_vertex = mem::size_of::<SliderVertex>() as GLsizei;
            let a_position = gl::GetAttribLocation(shader, c"position" .as_ptr()) as GLuint;
            gl::VertexAttribPointer(a_position, 4, gl::FLOAT, gl::FALSE, size_vertex, offset_of!(SliderVertex, position) as _);
            gl::EnableVertexAttribArray(a_position as GLuint);
        };
    }

    pub fn draw(&mut self, camera: &Camera, mouse_pos: Vec2) {
        let p = (mouse_pos / self.viewport - 0.5) / 0.727 + 0.5;

        let millis = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();
        let q = ((millis % 1000) as u32) as f32 / 1000.0;

        unsafe {
            gl::UseProgram(self.slider_shader);
            gl::Uniform1f(self.u_slider_texture_progress, q);
        }

        self.slider_vertices = (self.slider_paths.iter())
            .map(|sp| SliderVertices::from_slider_path(sp, self.slider_radius, 0.0, p.x as f64))
            .collect::<Vec<_>>();

        self.draw_with_clear_color(0.0, 0.0, 0.0, 0.5);
    }

    fn draw_slider_body(&self, slider: &SliderVertices) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.slider_fb.fbo);
            Self::blend(false);
            gl::Enable(gl::DEPTH_TEST);
            gl::DepthFunc(gl::LESS);
            gl::ClearColor(0.0, 0.0, 0.0, 0.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

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
            gl::BindTexture(gl::TEXTURE_2D, self.sliderbody_texture);
            gl::DrawArrays(gl::TRIANGLES, 0, slider.body.len() as GLsizei);

            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            Self::blend(true);
            gl::Disable(gl::DEPTH_TEST);

            gl::UseProgram(self.screen_shader);

            gl::BindVertexArray(self.screen_vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.screen_vbo);
            gl::BindTexture(gl::TEXTURE_2D, self.slider_fb.texture);
            gl::ActiveTexture(gl::TEXTURE0);

            gl::DrawArrays(gl::TRIANGLES, 0, 6);

            gl::BindTexture(gl::TEXTURE_2D, 0);
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

            gl::Uniform1i(self.u_slider_point_is_solid, 1);
            gl::DrawArrays(gl::LINE_STRIP, 0, slider.path.len() as GLsizei);
            gl::Uniform1i(self.u_slider_point_is_solid, 0);
            gl::DrawArrays(gl::POINTS, 0, slider.path.len() as GLsizei);

            gl::BindVertexArray(self.ctrl_vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.ctrl_vbo);
            gl::PointSize(10.0);
            gl::Uniform1i(self.u_slider_point_is_solid, 1);
            gl::DrawArrays(gl::LINE_STRIP, 0, slider.ctrl.len() as GLsizei);
            gl::Uniform1i(self.u_slider_point_is_solid, 0);
            gl::DrawArrays(gl::POINTS, 0, slider.ctrl.len() as GLsizei);
        }
    }

    fn draw_with_clear_color(&self, r: GLfloat, g: GLfloat, b: GLfloat, a: GLfloat) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);

            gl::ClearColor(r, g, b, a);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            gl::UseProgram(self.slider_shader);
            gl::Uniform1f(self.u_slider_radius, self.slider_radius);
            gl::Uniform4f(self.u_slider_border_color, 1.0, 0.2, 0.2, 1.0);
            gl::Uniform1f(self.u_slider_border_width, 7.27);

            for slider in &self.slider_vertices {
                self.draw_slider_body(slider);
                self.draw_slider_debug(slider);
            }
        }
    }

    pub fn resize(&mut self, camera: &Camera, width: i32, height: i32) {
        let viewport = ivec2(width, height);

        unsafe {
            gl::Viewport(0, 0, width, height);

            self.slider_fb = create_framebuffer("Slider Framebuffer", viewport.as_uvec2(), true);

            self.viewport = viewport.as_vec2();
            self.matrix = camera.matrix(self.viewport);

            gl::UseProgram(self.slider_shader);
            gl::UniformMatrix4fv(
                self.u_slider_mvp,
                1,
                gl::FALSE,
                self.matrix.as_ref().as_ptr(),
            );

            gl::UseProgram(self.slider_point_shader);
            gl::UniformMatrix4fv(
                self.u_slider_point_mvp,
                1,
                gl::FALSE,
                self.matrix.as_ref().as_ptr(),
            );
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
                control_points: vec![
                    Scp::new(Sct::Linear, 0.0, 211.0),
                    Scp::new(Sct::Inherit, 111.0, 189.0),
                ],
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
