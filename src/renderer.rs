pub mod round_quads;

use std::collections::HashSet;
use std::ffi::{c_void, CStr};

use gl::types::{GLchar, GLenum, GLsizei, GLuint};
use glam::Vec2;
use round_quads::RoundQuadsRenderer;
use winit::keyboard::NamedKey;
use winit::window::Window;

use crate::camera::Camera;

pub enum Renderer {
    RoundQuads(RoundQuadsRenderer),
}

impl Renderer {
    pub fn new(gl_display: &glutin::display::Display, window: &Window) -> Self {
        Self::RoundQuads(RoundQuadsRenderer::new(gl_display, window))
    }

    pub fn switch_scene(
        &mut self,
        gl_display: &glutin::display::Display,
        window: &Window,
        keycode: NamedKey,
    ) {
        match keycode {
            NamedKey::F1 => *self = Self::RoundQuads(RoundQuadsRenderer::new(gl_display, window)),
            NamedKey::F2 => (),
            _ => (),
        }
    }

    pub fn draw(&mut self, camera: &Camera, mouse_pos: Vec2) {
        match self {
            Self::RoundQuads(r) => r.draw(camera, mouse_pos),
        }
    }

    pub fn resize(&mut self, camera: &Camera, width: i32, height: i32) {
        match self {
            Self::RoundQuads(r) => r.resize(camera, width, height),
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
