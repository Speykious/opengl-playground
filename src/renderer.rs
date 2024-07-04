use std::{
    f32::consts::{PI, TAU},
    ffi::{CStr, CString},
    mem,
    time::Instant,
};

use gl::types::{GLenum, GLfloat, GLsizei, GLsizeiptr, GLuint};
use glam::{vec2, vec3, vec4, Vec2, Vec3, Vec4};
use glutin::display::GlDisplay;
use rand::Rng;

use crate::camera::Camera;

#[repr(C)]
#[derive(Debug, Clone)]
struct Square {
    pub pos: Vec3,
    pub rot: f32,
    pub size: Vec2,
    pub col: Vec4,
}

impl Square {
    fn random(rng: &mut impl Rng) -> Self {
        Self {
            pos: vec3(
                rng.gen_range(-727.0..=727.0),
                rng.gen_range(-727.0..=727.0),
                rng.gen_range(-1.0..=1.0),
            ),
            rot: rng.gen_range(0.0..TAU),
            size: Vec2::splat(rng.gen_range(50.0..=100.0)),
            col: vec4(
                rng.gen_range(0.5..=1.0),
                rng.gen_range(0.5..=1.0),
                rng.gen_range(0.5..=1.0),
                rng.gen_range(0.5..=1.0),
            ),
        }
    }

    fn vertices(&self) -> [Vertex; 4] {
        let pos = self.pos;
        let size = self.size;
        let col = self.col;

        let r = vec2(self.rot.cos(), self.rot.sin());

        #[rustfmt::skip]
        return [
            vertex((vec2(-0.5, -0.5).rotate(r) * size).extend(0.0) + pos, col),
            vertex((vec2(-0.5,  0.5).rotate(r) * size).extend(0.0) + pos, col),
            vertex((vec2( 0.5,  0.5).rotate(r) * size).extend(0.0) + pos, col),
            vertex((vec2( 0.5, -0.5).rotate(r) * size).extend(0.0) + pos, col),
        ];
    }

    fn indices(&self, square_index: u32) -> [u32; 6] {
        let i = square_index * 4;
        [i, 1 + i, 2 + i, i, 2 + i, 3 + i]
    }
}

pub struct Renderer {
    camera: Camera,

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

const N_SQUARES: usize = 10_000;

impl Renderer {
    pub fn new<D: GlDisplay>(gl_display: &D) -> Self {
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

            gl::Enable(gl::BLEND);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);

            let program = create_shader_program(
                include_bytes!("shaders/basic.vert"),
                include_bytes!("shaders/basic.frag"),
            );

            let u_mvp = gl::GetUniformLocation(program, c"mvp".as_ptr());

            let mut vao: u32 = 0;
            gl::GenVertexArrays(1, &mut vao);
            gl::BindVertexArray(vao);

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

        for (square, verts) in self.squares.iter_mut().zip(self.vertices.iter_mut()) {
            square.rot += dt * PI;
            *verts = square.vertices();
        }

        self.frame_count += 1;

        self.draw_with_clear_color(0., 0., 0., 0.5)
    }

    pub fn draw_with_clear_color(&mut self, r: GLfloat, g: GLfloat, b: GLfloat, a: GLfloat) {
        unsafe {
            gl::UseProgram(self.square_shader);

            gl::BindVertexArray(self.vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.ebo);

            // upload new data here
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

    let frag_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
    {
        let length = frag_source.len() as i32;
        let source = frag_source.as_ptr() as *const i8;
        gl::ShaderSource(frag_shader, 1, &source, &length);
        gl::CompileShader(frag_shader);
    }

    let program = gl::CreateProgram();

    gl::AttachShader(program, vert_shader);
    gl::AttachShader(program, frag_shader);

    gl::LinkProgram(program);
    gl::UseProgram(program);

    gl::DeleteShader(vert_shader);
    gl::DeleteShader(frag_shader);

    program
}

fn get_gl_string(variant: GLenum) -> Option<&'static CStr> {
    unsafe {
        let s = gl::GetString(variant);
        (!s.is_null()).then(|| CStr::from_ptr(s.cast()))
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Vertex {
    pos: Vec3,
    col: Vec4,
}

const fn vertex(pos: Vec3, col: Vec4) -> Vertex {
    Vertex { pos, col }
}
