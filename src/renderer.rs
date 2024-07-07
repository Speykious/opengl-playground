use std::{
    collections::HashSet,
    f32::consts::{PI, TAU},
    ffi::{c_void, CStr, CString},
    mem,
    time::Instant,
};

use gl::types::{GLchar, GLenum, GLfloat, GLsizei, GLsizeiptr, GLuint};
use glam::{vec2, vec4, Vec2, Vec4};
use glutin::display::GlDisplay;
use rand::Rng;
use winit::window::Window;

use crate::camera::Camera;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Square {
    pub position: Vec2,
    pub size: Vec2,
    pub rotation: f32,
    pub roundness: f32,
    pub stroke_width: f32,
    pub fill_color: Vec4,
    pub stroke_color: Vec4,
}

const AREA: f32 = 5000.0;
const AREA_BOUND: f32 = AREA * 0.5;

const N_SQUARES: usize = 100_000;

impl Square {
    fn random(rng: &mut impl Rng) -> Self {
        Self {
            position: vec2(
                rng.gen_range(-AREA_BOUND..=AREA_BOUND),
                rng.gen_range(-AREA_BOUND..=AREA_BOUND),
            ),
            size: Vec2::splat(rng.gen_range(10.0..=20.0)),
            rotation: rng.gen_range(0.0..TAU),
            roundness: rng.gen_range(1.0..=5.0),
            stroke_width: rng.gen_range(1.0..=5.0),
            fill_color: vec4(
                rng.gen_range(0.5..=1.0),
                rng.gen_range(0.5..=1.0),
                rng.gen_range(0.5..=1.0),
                rng.gen_range(0.5..=1.0),
            ),
            stroke_color: vec4(
                rng.gen_range(0.1..=0.5),
                rng.gen_range(0.1..=0.5),
                rng.gen_range(0.1..=0.5),
                rng.gen_range(0.5..=1.0),
            ),
        }
    }

    fn glsl_square(self) -> GlslSquare {
        GlslSquare {
            size: self.size,
            roundness: self.roundness,
            stroke_width: self.stroke_width,
            fill_color: self.fill_color,
            stroke_color: self.stroke_color,
        }
    }

    fn vertices(self) -> [Vertex; 4] {
        let Self {
            position,
            size,
            rotation,
            ..
        } = self;

        let r = vec2(rotation.cos(), rotation.sin());

        #[rustfmt::skip]
        let pos_dims = [
            ((vec2(-0.5, -0.5).rotate(r) * size) + position, vec2(0.0, 1.0)),
            ((vec2(-0.5,  0.5).rotate(r) * size) + position, vec2(0.0, 0.0)),
            ((vec2( 0.5,  0.5).rotate(r) * size) + position, vec2(1.0, 0.0)),
            ((vec2( 0.5, -0.5).rotate(r) * size) + position, vec2(1.0, 1.0)),
        ];

        pos_dims.map(|(position, uv)| Vertex { position, uv })
    }

    fn indices(&self, square_index: u32) -> [u32; 6] {
        let i = square_index * 4;
        [i, 1 + i, 2 + i, i, 2 + i, 3 + i]
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct GlslSquare {
    /// dimension coordinates
    size: Vec2,
    /// radius of round corners
    roundness: f32,
    /// stroke width
    stroke_width: f32,
    /// color
    fill_color: Vec4,
    /// stroke color
    stroke_color: Vec4,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct Vertex {
    /// position of square
    position: Vec2,
    /// UV coordinates
    uv: Vec2,
}

pub struct Renderer {
    pub camera: Camera,

    square_shader: GLuint,
    vao: GLuint,
    vbo: GLuint,
    ebo: GLuint,

    u_mvp: i32,

    squares: Vec<Square>,
    vertices: Vec<[Vertex; 4]>,
    indices: Vec<[u32; 6]>,

    start: Instant,
    last_instant: Instant,
    frame_count: u128,
}

impl Renderer {
    pub fn new(gl_display: &glutin::display::Display, window: &Window) -> Self {
        let mut squares = Vec::with_capacity(N_SQUARES);
        let mut vertices = Vec::with_capacity(N_SQUARES);
        let mut indices = Vec::with_capacity(N_SQUARES);

        let mut rng = rand::thread_rng();
        for i in 0..(N_SQUARES as u32) {
            let square = Square::random(&mut rng);
            vertices.push(square.vertices());
            indices.push(square.indices(i));
            squares.push(square);
        }

        let storage_buffer = (squares.iter().copied())
            .map(Square::glsl_square)
            .collect::<Vec<_>>();

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

            gl::Enable(gl::BLEND);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);

            gl::Enable(gl::MULTISAMPLE);

            let program = create_shader_program(
                include_bytes!("shaders/basic.vert"),
                include_bytes!("shaders/basic.frag"),
            );

            let u_mvp = gl::GetUniformLocation(program, c"u_mvp".as_ptr());

            let mut vao: u32 = 0;
            gl::GenVertexArrays(1, &mut vao);
            gl::BindVertexArray(vao);

            let mut ssbo: u32 = 0;
            gl::GenBuffers(1, &mut ssbo);
            gl::BindBuffer(gl::SHADER_STORAGE_BUFFER, ssbo);

            let storage_buffer_len = mem::size_of_val(storage_buffer.as_slice()) as GLsizeiptr;
            dbg!(storage_buffer_len);
            gl::BufferData(
                gl::SHADER_STORAGE_BUFFER,
                storage_buffer_len,
                storage_buffer.as_slice().as_ptr() as *const _,
                gl::STATIC_DRAW,
            );
            gl::BindBufferRange(gl::SHADER_STORAGE_BUFFER, 0, ssbo, 0, storage_buffer_len);

            let camera = Camera {
                scale: Vec2::splat(window.scale_factor() as f32 * 1.8),
                ..Default::default()
            };

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
            let size_f32 = mem::size_of::<f32>();

            #[rustfmt::skip]
            {
                let a_position = gl::GetAttribLocation(program, c"position" .as_ptr()) as GLuint;
                let a_uv       = gl::GetAttribLocation(program, c"uv"       .as_ptr()) as GLuint;

                gl::VertexAttribPointer(a_position, 2, gl::FLOAT, gl::FALSE, size_vertex,  0             as _);
                gl::VertexAttribPointer(a_uv,       2, gl::FLOAT, gl::FALSE, size_vertex, (2 * size_f32) as _);

                gl::EnableVertexAttribArray(a_position   as GLuint);
                gl::EnableVertexAttribArray(a_uv         as GLuint);
            };

            Self {
                camera,

                square_shader: program,
                vao,
                vbo,
                ebo,

                u_mvp,

                squares,
                vertices,
                indices,

                start: Instant::now(),
                last_instant: Instant::now(),
                frame_count: 0,
            }
        }
    }

    pub fn draw(&mut self) {
        let dt = self.last_instant.elapsed().as_secs_f32();
        self.last_instant = Instant::now();

        for (square, verts) in (self.squares.iter_mut()).zip(self.vertices.iter_mut()) {
            square.rotation += dt * PI * 0.25;
            *verts = square.vertices();
        }

        self.draw_with_clear_color(0., 0., 0., 0.5);
        self.frame_count += 1;
    }

    pub fn draw_with_clear_color(&mut self, r: GLfloat, g: GLfloat, b: GLfloat, a: GLfloat) {
        unsafe {
            gl::UseProgram(self.square_shader);

            gl::BindVertexArray(self.vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.ebo);

            gl::BufferSubData(
                gl::ARRAY_BUFFER,
                0,
                mem::size_of_val(self.vertices.as_slice()) as GLsizeiptr,
                self.vertices.as_slice().as_ptr() as *const _,
            );

            gl::ClearColor(r, g, b, a);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::DrawElements(
                gl::TRIANGLES,
                mem::size_of_val(self.indices.as_slice()) as GLsizei,
                gl::UNSIGNED_INT,
                std::ptr::null(),
            );
        }
    }

    pub fn resize(&mut self, width: i32, height: i32) {
        unsafe {
            gl::Viewport(0, 0, width, height);

            let matrix = self.camera.matrix(Vec2::new(width as f32, height as f32));
            gl::UniformMatrix4fv(self.u_mvp, 1, gl::FALSE, matrix.as_ref().as_ptr());
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed().as_secs_f64();
        let fps = self.frame_count as f64 / elapsed;
        println!("Total frames: {}", self.frame_count);
        println!("Average FPS:  {}", fps);

        unsafe {
            gl::DeleteProgram(self.square_shader);
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
