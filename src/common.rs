// come on it's just OpenGL
#![allow(clippy::missing_safety_doc)]

use glam::UVec2;
use glow::HasContext;

use super::context::is_opengl_debug_enabled;

pub fn slice_as_bytes<T>(slice: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(slice.as_ptr() as *const u8, size_of_val(slice)) }
}

// --- debugging ---

pub unsafe fn push_debug_group(gl: &glow::Context, message: &str) {
    if is_opengl_debug_enabled() {
        unsafe {
            gl.push_debug_group(glow::DEBUG_SOURCE_APPLICATION, 0, message);
        }
    }
}

pub unsafe fn pop_debug_group(gl: &glow::Context) {
    if is_opengl_debug_enabled() {
        unsafe {
            gl.pop_debug_group();
        }
    }
}

// --- shader compilation ---

pub unsafe fn create_shader_program(gl: &glow::Context, vert_source: &str, frag_source: &str) -> glow::Program {
    unsafe {
        let vert_shader = gl.create_shader(glow::VERTEX_SHADER).unwrap();
        {
            gl.shader_source(vert_shader, vert_source);
            gl.compile_shader(vert_shader);
        }
        verify_shader(gl, vert_shader, "vert");

        let frag_shader = gl.create_shader(glow::FRAGMENT_SHADER).unwrap();
        {
            gl.shader_source(frag_shader, frag_source);
            gl.compile_shader(frag_shader);
        }
        verify_shader(gl, frag_shader, "frag");

        let program = gl.create_program().unwrap();
        {
            gl.attach_shader(program, vert_shader);
            gl.attach_shader(program, frag_shader);

            gl.link_program(program);
            gl.use_program(Some(program));

            gl.delete_shader(vert_shader);
            gl.delete_shader(frag_shader);
        }
        verify_program(gl, program);

        program
    }
}

pub unsafe fn verify_shader(gl: &glow::Context, shader: glow::Shader, ty: &str) {
    unsafe {
        if !gl.get_shader_compile_status(shader) {
            let log = gl.get_shader_info_log(shader);

            if !log.is_empty() {
                tracing::error!("SHADER COMPILE ERROR ({ty}): {log}");
            }
        }
    }
}

pub unsafe fn verify_program(gl: &glow::Context, program: glow::Program) {
    unsafe {
        if !gl.get_program_link_status(program) {
            let log = gl.get_program_info_log(program);

            if !log.is_empty() {
                tracing::error!("PROGRAM LINK ERROR: {log}");
            }
        }
    }
}

// --- framebuffers and textures ---

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Framebuffer {
    pub fbo: glow::Framebuffer,
    pub texture: glow::Texture,
    pub depth: Option<glow::Texture>,
    pub size: UVec2,
}

pub unsafe fn create_framebuffer(
    gl: &glow::Context,
    name: &str,
    size: UVec2,
    wrapping: TextureWrapping,
    depth: bool,
) -> Framebuffer {
    unsafe {
        let fbo = gl.create_framebuffer().unwrap();
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));

        let texture = gl.create_texture().unwrap();
        upload_texture(gl, texture, size.x, size.y, None, wrapping);

        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(texture),
            0,
        );

        let depth = if depth {
            let texture = gl.create_texture().unwrap();
            upload_depth_texture(gl, texture, size.x, size.y, None);

            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::DEPTH_STENCIL_ATTACHMENT,
                glow::TEXTURE_2D,
                Some(texture),
                0,
            );

            Some(texture)
        } else {
            None
        };

        if gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
            tracing::error!("{name} framebuffer ({}x{}) not complete", size.x, size.y);
        }

        Framebuffer {
            fbo,
            texture,
            depth,
            size,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(unused)]
pub enum TextureWrapping {
    /// The integer part of the coordinate will be ignored and a repeating pattern is formed.
    Repeat,
    /// The texture will also be repeated, but it will be mirrored when the integer part of the coordinate is odd.
    MirroredRepeat,
    /// The coordinate will simply be clamped between 0 and 1.
    ClampToEdge,
    /// The coordinates that fall outside the range will be given a specified border color.
    ClampToBorder,
}

impl TextureWrapping {
    pub fn to_glenum(self) -> u32 {
        match self {
            TextureWrapping::Repeat => glow::REPEAT,
            TextureWrapping::MirroredRepeat => glow::MIRRORED_REPEAT,
            TextureWrapping::ClampToEdge => glow::CLAMP_TO_EDGE,
            TextureWrapping::ClampToBorder => glow::CLAMP_TO_BORDER,
        }
    }
}

pub unsafe fn upload_texture(
    gl: &glow::Context,
    texture: glow::Texture,
    width: u32,
    height: u32,
    pixels: Option<&[u8]>,
    wrapping: TextureWrapping,
) {
    let clamp = wrapping.to_glenum();

    unsafe {
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));

        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            width as i32,
            height as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(pixels),
        );

        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, clamp as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, clamp as i32);
    }
}

pub unsafe fn upload_subtexture(
    gl: &glow::Context,
    texture: glow::Texture,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    pixels: Option<&[u8]>,
) {
    unsafe {
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_sub_image_2d(
            glow::TEXTURE_2D,
            0,
            x as i32,
            y as i32,
            width as i32,
            height as i32,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(pixels),
        );
    }
}

pub unsafe fn upload_depth_texture(
    gl: &glow::Context,
    texture: glow::Texture,
    width: u32,
    height: u32,
    pixels: Option<&[u8]>,
) {
    unsafe {
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::DEPTH24_STENCIL8 as i32,
            width as i32,
            height as i32,
            0,
            glow::DEPTH_STENCIL,
            glow::UNSIGNED_INT_24_8,
            glow::PixelUnpackData::Slice(pixels),
        );
    }
}
