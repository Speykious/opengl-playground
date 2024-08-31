// come on it's just OpenGL
#![allow(clippy::missing_safety_doc)]

use std::ffi::CStr;
use std::sync::atomic::{AtomicBool, Ordering};

use gl::types::{GLchar, GLenum, GLint, GLsizei, GLuint};
use glam::UVec2;

// --- debugging ---

// Set in main when checking for the GL_KHR_debug extension.
pub static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

pub unsafe fn push_debug_group(message: &CStr) {
    if DEBUG_ENABLED.load(Ordering::Relaxed) {
        gl::PushDebugGroup(
            gl::DEBUG_SOURCE_APPLICATION,
            0,
            message.count_bytes() as i32,
            message.as_ptr() as *const GLchar,
        );
    }
}

pub unsafe fn pop_debug_group() {
    if DEBUG_ENABLED.load(Ordering::Relaxed) {
        gl::PopDebugGroup();
    }
}

// --- shader compilation ---

pub unsafe fn create_shader_program(vert_source: &[u8], frag_source: &[u8]) -> GLuint {
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

pub unsafe fn verify_shader(shader: GLuint, ty: &str) {
    let mut status = 0;
    gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut status);

    if status != 1 {
        let mut length = 0;
        gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut length);

        if length > 0 {
            let mut log = String::with_capacity(length as usize);
            log.extend(std::iter::repeat('\0').take(length as usize));
            gl::GetShaderInfoLog(shader, length, &mut length, log.as_mut_ptr().cast());
            log.truncate(length as usize);

            eprintln!("SHADER COMPILE ERROR ({ty}): {log}");
        }
    }
}

pub unsafe fn verify_program(shader: GLuint) {
    let mut status = 0;
    gl::GetProgramiv(shader, gl::LINK_STATUS, &mut status);

    if status != 1 {
        let mut length = 0;
        gl::GetProgramiv(shader, gl::INFO_LOG_LENGTH, &mut length);

        if length > 0 {
            let mut log = String::with_capacity(length as usize);
            log.extend(std::iter::repeat('\0').take(length as usize));
            gl::GetProgramInfoLog(shader, length, &mut length, log.as_mut_ptr().cast());
            log.truncate(length as usize);

            eprintln!("PROGRAM LINK ERROR: {log}");
        }
    }
}

// --- framebuffers and textures ---

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Framebuffer {
    pub fbo: GLuint,
    pub texture: GLuint,
    pub size: UVec2,
}

pub unsafe fn create_framebuffer(name: &str, size: UVec2) -> Framebuffer {
    let mut fbo: GLuint = 0;
    gl::GenFramebuffers(1, &mut fbo);
    gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);

    let mut texture: GLuint = 0;
    gl::GenTextures(1, &mut texture);
    upload_texture(texture, size.x, size.y, std::ptr::null(), gl::CLAMP_TO_EDGE);
    gl::FramebufferTexture2D(
        gl::FRAMEBUFFER,
        gl::COLOR_ATTACHMENT0,
        gl::TEXTURE_2D,
        texture,
        0,
    );

    if gl::CheckFramebufferStatus(gl::FRAMEBUFFER) != gl::FRAMEBUFFER_COMPLETE {
        eprintln!("{name} framebuffer ({}x{}) not complete", size.x, size.y);
    }

    Framebuffer { fbo, texture, size }
}

pub unsafe fn upload_texture(
    texture: GLuint,
    width: u32,
    height: u32,
    data: *const u8,
    clamp: GLenum,
) {
    gl::BindTexture(gl::TEXTURE_2D, texture);
    gl::TexImage2D(
        gl::TEXTURE_2D,
        0,
        gl::RGBA8 as GLint,
        width as GLsizei,
        height as GLsizei,
        0,
        gl::RGBA,
        gl::UNSIGNED_BYTE,
        data as *const _,
    );
    gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
    gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
    gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, clamp as GLint);
    gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, clamp as GLint);
}
