use std::{
    collections::HashSet,
    f32::consts::{PI, TAU},
    ffi::{c_void, CStr, CString},
    mem,
    time::Instant,
};

use gl::types::{GLchar, GLenum, GLfloat, GLsizei, GLsizeiptr, GLuint};
use glam::{vec2, Mat4, Vec2, Vec4};
use glutin::display::GlDisplay;
use rand::Rng;
use winit::window::Window;

use crate::camera::Camera;

const N_QUADS: usize = 100_000;

const SRC_VERT_QUAD: &[u8] = include_bytes!("shaders/quad.vert");
const SRC_FRAG_ROUND_RECT: &[u8] = include_bytes!("shaders/round-rect.frag");

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
            size: vec2(rng.gen_range(10.0..=20.0), rng.gen_range(10.0..=20.0)),
            rotation: rng.gen_range(0.0..TAU),
            border_radius: rng.gen_range(1.0..=5.0),
            border_width: rng.gen_range(1.0..=5.0),
            fill_color: u32::from_le_bytes([
                rng.gen_range(128..=255),
                rng.gen_range(128..=255),
                rng.gen_range(128..=255),
                rng.gen_range(128..=255),
            ]),
            stroke_color: u32::from_le_bytes([
                rng.gen_range(24..=128),
                rng.gen_range(24..=128),
                rng.gen_range(24..=128),
                rng.gen_range(128..=255),
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
            fill_color: Vec4::from_array(fill_color.to_le_bytes().map(|n| n as f32)) / 255.0,
            stroke_color: Vec4::from_array(stroke_color.to_le_bytes().map(|n| n as f32)) / 255.0,
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
    fill_color: Vec4,
    stroke_color: Vec4,
    border_radius: f32,
    border_width: f32,
    intensity: f32,
}

pub trait Renderer {
    fn draw(&mut self, camera: &Camera, mouse_pos: Vec2);
    fn resize(&mut self, camera: &Camera, width: i32, height: i32);
}

pub struct RoundQuadsRenderer {
    matrix: Mat4,
    viewport: Vec2,

    round_quad_shader: GLuint,
    vao: GLuint,
    vbo: GLuint,
    ebo: GLuint,

    u_mvp_quad: i32,

    quads: Vec<Quad>,
    vertices: Vec<[Vertex; 4]>,
    indices: Vec<[u32; 6]>,

    area_width: u32,

    start: Instant,
    last_instant: Instant,
    frame_count: u128,
}

impl RoundQuadsRenderer {
    pub fn new(gl_display: &glutin::display::Display, window: &Window) -> Self {
        let area_width = (N_QUADS as f32).sqrt() as u32;

        let mut quads = Vec::with_capacity(N_QUADS);
        let mut vertices = Vec::with_capacity(N_QUADS);
        let mut indices = Vec::with_capacity(N_QUADS);

        let mut rng = rand::thread_rng();
        for i in 0..(N_QUADS as u32) {
            let quad = Quad::random(&mut rng, i, area_width);
            vertices.push(quad.vertices(0.5));
            indices.push(quad.indices(i));
            quads.push(quad);
        }

        unsafe {
            gl::load_with(|symbol| {
                let symbol = CString::new(symbol).unwrap();
                gl_display.get_proc_address(symbol.as_c_str()).cast()
            });

            if let Some(renderer) = get_gl_string(gl::RENDERER) {
                println!("Running on {}", renderer.to_string_lossy());
            }
            if let Some(version) = get_gl_string(gl::VERSION) {
                println!("OpenGL Version {}", version.to_string_lossy());
            }
            if let Some(shaders_version) = get_gl_string(gl::SHADING_LANGUAGE_VERSION) {
                println!("Shaders version on {}", shaders_version.to_string_lossy());
            }

            // Check for "GL_KHR_debug" support (not present on Apple *OS).
            let extensions = get_opengl_extensions();

            if extensions.contains("GL_KHR_debug") {
                println!("Debug extension supported!\n");
                gl::DebugMessageCallback(Some(debug_message_callback), std::ptr::null());
                gl::Enable(gl::DEBUG_OUTPUT);
            }

            // Normal blending
            gl::Enable(gl::BLEND);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);

            let round_quad_shader = create_shader_program(SRC_VERT_QUAD, SRC_FRAG_ROUND_RECT);

            let u_mvp_quad = gl::GetUniformLocation(round_quad_shader, c"u_mvp".as_ptr());

            let mut vao: u32 = 0;
            gl::GenVertexArrays(1, &mut vao);
            gl::BindVertexArray(vao);

            let mut ssbo: u32 = 0;
            gl::GenBuffers(1, &mut ssbo);
            gl::BindBuffer(gl::SHADER_STORAGE_BUFFER, ssbo);

            let mut vbo: u32 = 0;
            gl::GenBuffers(1, &mut vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                mem::size_of_val(vertices.as_slice()) as GLsizeiptr,
                vertices.as_slice().as_ptr() as *const _,
                gl::DYNAMIC_DRAW,
            );

            let mut ebo: u32 = 0;
            gl::GenBuffers(1, &mut ebo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);
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
                let a_position      = gl::GetAttribLocation(round_quad_shader, c"position"      .as_ptr()) as GLuint;
                let a_size          = gl::GetAttribLocation(round_quad_shader, c"size"          .as_ptr()) as GLuint;
                let a_fill_color    = gl::GetAttribLocation(round_quad_shader, c"fill_color"    .as_ptr()) as GLuint;
                let a_stroke_color  = gl::GetAttribLocation(round_quad_shader, c"stroke_color"  .as_ptr()) as GLuint;
                let a_border_radius = gl::GetAttribLocation(round_quad_shader, c"border_radius" .as_ptr()) as GLuint;
                let a_border_width  = gl::GetAttribLocation(round_quad_shader, c"border_width"  .as_ptr()) as GLuint;
                let a_intensity     = gl::GetAttribLocation(round_quad_shader, c"intensity"     .as_ptr()) as GLuint;

                gl::VertexAttribPointer(a_position,      2, gl::FLOAT, gl::FALSE, size_vertex,   0             as _);
                gl::VertexAttribPointer(a_size,          2, gl::FLOAT, gl::FALSE, size_vertex, ( 2 * size_f32) as _);
                gl::VertexAttribPointer(a_fill_color,    4, gl::FLOAT, gl::FALSE, size_vertex, ( 4 * size_f32) as _);
                gl::VertexAttribPointer(a_stroke_color,  4, gl::FLOAT, gl::FALSE, size_vertex, ( 8 * size_f32) as _);
                gl::VertexAttribPointer(a_border_radius, 1, gl::FLOAT, gl::FALSE, size_vertex, (12 * size_f32) as _);
                gl::VertexAttribPointer(a_border_width,  1, gl::FLOAT, gl::FALSE, size_vertex, (13 * size_f32) as _);
                gl::VertexAttribPointer(a_intensity,     1, gl::FLOAT, gl::FALSE, size_vertex, (14 * size_f32) as _);

                gl::EnableVertexAttribArray(a_position      as GLuint);
                gl::EnableVertexAttribArray(a_size          as GLuint);
                gl::EnableVertexAttribArray(a_fill_color    as GLuint);
                gl::EnableVertexAttribArray(a_stroke_color  as GLuint);
                gl::EnableVertexAttribArray(a_border_radius as GLuint);
                gl::EnableVertexAttribArray(a_border_width  as GLuint);
                gl::EnableVertexAttribArray(a_intensity     as GLuint);
            };

            let win_size = window.inner_size();
            let viewport = Vec2::new(win_size.width as f32, win_size.height as f32);

            Self {
                matrix: Mat4::default(),
                viewport,

                round_quad_shader,
                vao,
                vbo,
                ebo,

                u_mvp_quad,

                quads,
                vertices,
                indices,

                area_width,

                start: Instant::now(),
                last_instant: Instant::now(),
                frame_count: 0,
            }
        }
    }

    fn update_vertices(&mut self, x_beg: u32, x_end: u32, y_beg: u32, y_end: u32) {
        unsafe {
            gl::BindVertexArray(self.vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.ebo);

            for y in y_beg..=y_end {
                let i_beg = (y * self.area_width + x_beg) as usize;
                let i_end = (y * self.area_width + x_end) as usize;

                gl::BufferSubData(
                    gl::ARRAY_BUFFER,
                    mem::size_of_val(&self.vertices[..i_beg]) as GLsizeiptr,
                    mem::size_of_val(&self.vertices[i_beg..=i_end]) as GLsizeiptr,
                    self.vertices[i_beg..=i_end].as_ptr() as *const _,
                );
            }
        }
    }

    fn draw_with_clear_color(&self, r: GLfloat, g: GLfloat, b: GLfloat, a: GLfloat) {
        unsafe {
            gl::BindVertexArray(self.vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.ebo);

            gl::ClearColor(r, g, b, a);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            gl::UseProgram(self.round_quad_shader);
            gl::DrawElements(
                gl::TRIANGLES,
                mem::size_of_val(self.indices.as_slice()) as GLsizei,
                gl::UNSIGNED_INT,
                std::ptr::null(),
            );
        }
    }
}

impl Renderer for RoundQuadsRenderer {
    fn draw(&mut self, camera: &Camera, mouse_pos: Vec2) {
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
                    self.vertices[i] = quad.vertices(2.0 * intensity + 0.5);
                }
            }
        }

        self.update_vertices(x_beg, x_end, y_beg, y_end);

        self.draw_with_clear_color(0.0, 0.0, 0.0, 0.5);
        self.frame_count += 1;

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

    fn resize(&mut self, camera: &Camera, width: i32, height: i32) {
        unsafe {
            gl::Viewport(0, 0, width, height);

            self.viewport = Vec2::new(width as f32, height as f32);
            self.matrix = camera.matrix(self.viewport);

            gl::UseProgram(self.round_quad_shader);
            gl::UniformMatrix4fv(
                self.u_mvp_quad,
                1,
                gl::FALSE,
                self.matrix.as_ref().as_ptr(),
            );
        }
    }
}

impl Drop for RoundQuadsRenderer {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed().as_secs_f64();
        let fps = self.frame_count as f64 / elapsed;
        println!("Total frames: {}", self.frame_count);
        println!("Average FPS:  {}", fps);

        unsafe {
            gl::DeleteProgram(self.round_quad_shader);
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteVertexArrays(1, &self.vao);
        }
    }
}

unsafe fn create_shader_program(vert_source: &[u8], frag_source: &[u8]) -> GLuint {
    let vert_shader = gl::CreateShader(gl::VERTEX_SHADER);
    {
        let length = vert_source.len() as i32;
        let source = vert_source.as_ptr() as *const i8;
        gl::ShaderSource(vert_shader, 1, &source, &length);
        gl::CompileShader(vert_shader);
    }
    verify_shader(vert_shader, "vert");

    let frag_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
    {
        let length = frag_source.len() as i32;
        let source = frag_source.as_ptr() as *const i8;
        gl::ShaderSource(frag_shader, 1, &source, &length);
        gl::CompileShader(frag_shader);
    }
    verify_shader(frag_shader, "frag");

    let program = gl::CreateProgram();
    {
        gl::AttachShader(program, vert_shader);
        gl::AttachShader(program, frag_shader);

        gl::LinkProgram(program);
        gl::UseProgram(program);

        gl::DeleteShader(vert_shader);
        gl::DeleteShader(frag_shader);
    }
    verify_program(program);

    program
}

unsafe fn verify_shader(shader: GLuint, ty: &str) {
    let mut status = 0;
    gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut status);

    if status != 1 {
        let mut length = 0;
        gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut length);

        if length > 0 {
            let mut log = String::with_capacity(length as usize);
            log.extend(std::iter::repeat('\0').take(length as usize));
            gl::GetShaderInfoLog(shader, length, &mut length, log.as_str().as_ptr() as *mut _);
            log.truncate(length as usize);

            eprintln!("SHADER COMPILE ERROR ({ty}): {log}");
        }
    }
}

unsafe fn verify_program(shader: GLuint) {
    let mut status = 0;
    gl::GetShaderiv(shader, gl::LINK_STATUS, &mut status);

    if status != 1 {
        let mut length = 0;
        gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut length);

        if length > 0 {
            let mut log = String::with_capacity(length as usize);
            log.extend(std::iter::repeat('\0').take(length as usize));
            gl::GetProgramInfoLog(shader, length, &mut length, log.as_str().as_ptr() as *mut _);
            log.truncate(length as usize);

            eprintln!("PROGRAM LINK ERROR: {log}");
        }
    }
}

fn get_gl_string(variant: GLenum) -> Option<&'static CStr> {
    unsafe {
        let s = gl::GetString(variant);
        (!s.is_null()).then(|| CStr::from_ptr(s.cast()))
    }
}

unsafe fn get_opengl_extensions() -> HashSet<String> {
    let mut num_extensions = 0;
    gl::GetIntegerv(gl::NUM_EXTENSIONS, &mut num_extensions);

    (0..num_extensions)
        .map(|i| {
            let extension_name = gl::GetStringi(gl::EXTENSIONS, i as u32) as *const _;
            CStr::from_ptr(extension_name).to_string_lossy().to_string()
        })
        .collect()
}

extern "system" fn debug_message_callback(
    _src: GLenum,
    ty: GLenum,
    _id: GLuint,
    sevr: GLenum,
    _len: GLsizei,
    msg: *const GLchar,
    _user_param: *mut c_void,
) {
    let ty = match ty {
        gl::DEBUG_TYPE_ERROR => "Error: ",
        gl::DEBUG_TYPE_DEPRECATED_BEHAVIOR => "Deprecated Behavior: ",
        gl::DEBUG_TYPE_MARKER => "Marker: ",
        gl::DEBUG_TYPE_OTHER => "",
        gl::DEBUG_TYPE_POP_GROUP => "Pop Group: ",
        gl::DEBUG_TYPE_PORTABILITY => "Portability: ",
        gl::DEBUG_TYPE_PUSH_GROUP => "Push Group: ",
        gl::DEBUG_TYPE_UNDEFINED_BEHAVIOR => "Undefined Behavior: ",
        gl::DEBUG_TYPE_PERFORMANCE => "Performance: ",
        ty => unreachable!("unknown debug type {ty}"),
    };

    let msg = unsafe { CStr::from_ptr(msg) }.to_string_lossy();

    match sevr {
        gl::DEBUG_SEVERITY_NOTIFICATION => println!("[opengl debug] {ty}{msg}"),
        gl::DEBUG_SEVERITY_LOW => println!("[opengl  info] {ty}{msg}"),
        gl::DEBUG_SEVERITY_MEDIUM => println!("[opengl  warn] {ty}{msg}"),
        gl::DEBUG_SEVERITY_HIGH => println!("[opengl error] {ty}{msg}"),
        sevr => unreachable!("unknown debug severity {sevr}"),
    };
}
