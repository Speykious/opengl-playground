pub mod blurring;
pub mod round_quads;

use blurring::BlurringScene;
use gl::types::GLuint;
use glam::Vec2;
use round_quads::RoundQuadsScene;
use winit::keyboard::NamedKey;
use winit::window::Window;

use crate::camera::Camera;

pub enum Scenes {
    Blurring(BlurringScene),
    RoundQuads(RoundQuadsScene),
}

impl Scenes {
    pub fn new(window: &Window) -> Self {
        Self::RoundQuads(RoundQuadsScene::new(window))
    }

    pub fn switch_scene(&mut self, window: &Window, keycode: NamedKey) {
        match keycode {
            NamedKey::F1 => *self = Self::RoundQuads(RoundQuadsScene::new(window)),
            NamedKey::F2 => *self = Self::Blurring(BlurringScene::new(window)),
            _ => (),
        }
    }

    pub fn draw(&mut self, camera: &Camera, mouse_pos: Vec2) {
        match self {
            Self::RoundQuads(scene) => scene.draw(camera, mouse_pos),
            Self::Blurring(scene) => scene.draw(camera, mouse_pos),
        }
    }

    pub fn resize(&mut self, camera: &Camera, width: i32, height: i32) {
        match self {
            Self::RoundQuads(scene) => scene.resize(camera, width, height),
            Self::Blurring(scene) => scene.resize(camera, width, height),
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
