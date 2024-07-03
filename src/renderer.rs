use std::{
    ffi::{CStr, CString},
    mem,
};

use gl::types::{GLenum, GLfloat, GLsizei, GLsizeiptr, GLuint};
use glam::{vec3, vec4, Vec2, Vec3, Vec4};
use glutin::display::GlDisplay;

use crate::camera::Camera;

pub struct Renderer {
    camera: Camera,

    square_shader: GLuint,
    vao: GLuint,
    vbo: GLuint,
    ebo: GLuint,

    u_mvp: i32,
}

impl Renderer {
    pub fn new<D: GlDisplay>(gl_display: &D) -> Self {
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

            let vertex_shader = create_shader(gl::VERTEX_SHADER, VERTEX_SHADER_SOURCE);
            let fragment_shader = create_shader(gl::FRAGMENT_SHADER, FRAGMENT_SHADER_SOURCE);

            let program = gl::CreateProgram();

            gl::AttachShader(program, vertex_shader);
            gl::AttachShader(program, fragment_shader);

            gl::LinkProgram(program);
            gl::UseProgram(program);

            gl::DeleteShader(vertex_shader);
            gl::DeleteShader(fragment_shader);

            let u_mvp = gl::GetUniformLocation(program, c"mvp".as_ptr());

            let mut vao: u32 = 0;
            gl::GenVertexArrays(1, &mut vao);
            gl::BindVertexArray(vao);

            let mut vbo: u32 = 0;
            gl::GenBuffers(1, &mut vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                mem::size_of_val(VERTICES) as GLsizeiptr,
                VERTICES.as_ptr() as *const _,
                gl::DYNAMIC_DRAW,
            );

            let mut ebo: u32 = 0;
            gl::GenBuffers(1, &mut ebo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);
            gl::BufferData(
                gl::ELEMENT_ARRAY_BUFFER,
                mem::size_of_val(INDICES) as GLsizeiptr,
                INDICES.as_ptr() as *const _,
                gl::STATIC_DRAW,
            );

            let pos_attrib = gl::GetAttribLocation(program, c"pos".as_ptr());
            gl::VertexAttribPointer(
                pos_attrib as GLuint,
                3,
                gl::FLOAT,
                0,
                mem::size_of::<Vertex>() as GLsizei,
                std::ptr::null(),
            );
            gl::EnableVertexAttribArray(pos_attrib as GLuint);

            let col_attrib = gl::GetAttribLocation(program, c"col".as_ptr());
            gl::VertexAttribPointer(
                col_attrib as GLuint,
                4,
                gl::FLOAT,
                0,
                mem::size_of::<Vertex>() as GLsizei,
                (4 * std::mem::size_of::<f32>()) as *const _,
            );
            gl::EnableVertexAttribArray(col_attrib as GLuint);

            Self {
                camera: Camera::default(),

                square_shader: program,
                vao,
                vbo,
                ebo,

                u_mvp,
            }
        }
    }

    pub fn draw(&self) {
        self.draw_with_clear_color(0., 0., 0., 0.5)
    }

    pub fn draw_with_clear_color(&self, r: GLfloat, g: GLfloat, b: GLfloat, a: GLfloat) {
        unsafe {
            gl::UseProgram(self.square_shader);

            gl::BindVertexArray(self.vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.ebo);

            gl::ClearColor(r, g, b, a);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::DrawElements(
                gl::TRIANGLES,
                INDICES.len() as GLsizei,
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
        unsafe {
            gl::DeleteProgram(self.square_shader);
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteVertexArrays(1, &self.vao);
        }
    }
}

unsafe fn create_shader(shader: GLenum, source: &[u8]) -> GLuint {
    let shader = gl::CreateShader(shader);
    gl::ShaderSource(
        shader,
        1,
        [source.as_ptr().cast()].as_ptr(),
        std::ptr::null(),
    );
    gl::CompileShader(shader);
    shader
}

fn get_gl_string(variant: GLenum) -> Option<&'static CStr> {
    unsafe {
        let s = gl::GetString(variant);
        (!s.is_null()).then(|| CStr::from_ptr(s.cast()))
    }
}

#[repr(C)]
struct Vertex {
    pos: Vec3,
    col: Vec4,
}

const fn vertex(pos: Vec3, col: Vec4) -> Vertex {
    Vertex { pos, col }
}

static VERTICES: &[Vertex] = &[
    vertex(vec3(-100., -100., 0.0), vec4(1.0, 0.2, 0.3, 0.7)),
    vertex(vec3(-100., 100., 0.0), vec4(1.0, 0.2, 0.3, 0.7)),
    vertex(vec3(100., 100., 0.0), vec4(1.0, 0.2, 0.3, 0.7)),
    vertex(vec3(100., -100., 0.0), vec4(1.0, 0.2, 0.3, 0.7)),
];

static INDICES: &[u32] = &[0, 1, 2, 0, 2, 3];

const VERTEX_SHADER_SOURCE: &[u8] = b"
#version 100
precision mediump float;

uniform mat4 mvp;

attribute vec3 pos;
attribute vec4 col;

varying vec4 v_color;

void main() {
    gl_Position = mvp * vec4(pos, 1.0);
    v_color = col;
}
\0";

const FRAGMENT_SHADER_SOURCE: &[u8] = b"
#version 100
precision mediump float;

varying vec4 v_color;

void main() {
    gl_FragColor = v_color;
}
\0";
