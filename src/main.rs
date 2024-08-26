use std::{
    collections::HashSet,
    ffi::{c_void, CStr, CString},
    num::NonZeroU32,
    rc::Rc,
    sync::atomic::Ordering,
};

use gl::types::{GLchar, GLenum, GLsizei, GLuint};
use glam::{IVec2, Vec2};
use glutin::{
    config::{Config, ConfigTemplateBuilder, GlConfig as _},
    context::{
        ContextApi, ContextAttributesBuilder, NotCurrentContext, NotCurrentGlContext as _,
        PossiblyCurrentContext, Version,
    },
    display::{GetGlDisplay as _, GlDisplay as _},
    surface::{GlSurface as _, Surface, SwapInterval, WindowSurface},
};
use glutin_winit::{DisplayBuilder, GlWindow as _};
use scene_controller::SceneController;
use scenes::Scenes;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    raw_window_handle::HasWindowHandle as _,
    window::{Theme, Window, WindowAttributes},
};

pub mod camera;
pub mod common_gl;
pub mod scene_controller;
pub mod scenes;

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new(
        WindowAttributes::default()
            .with_active(true)
            .with_theme(Some(Theme::Dark))
            .with_title("OpenGL Playground")
            .with_resizable(true),
    );

    event_loop.run_app(&mut app).unwrap();
}

struct AppState {
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    window: Rc<Window>,
}

struct App {
    win_attribs: WindowAttributes,
    template_builder: ConfigTemplateBuilder,
    display_builder: DisplayBuilder,
    not_current_gl_context: Option<NotCurrentContext>,
    scenes: Option<(Scenes, SceneController)>,
    state: Option<AppState>,

    viewport: IVec2,
    mouse_pos: Vec2,
}

impl App {
    fn new(win_attribs: WindowAttributes) -> Self {
        // The template will match only the configurations supporting rendering
        // to windows.
        //
        // XXX We force transparency only on macOS, given that EGL on X11 doesn't
        // have it, but we still want to show window. The macOS situation is like
        // that, because we can query only one config at a time on it, but all
        // normal platforms will return multiple configs, so we can find the config
        // with transparency ourselves inside the `reduce`.
        let template_builder = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .with_transparency(cfg!(target_os = "macos"));

        let display_builder =
            DisplayBuilder::new().with_window_attributes(Some(win_attribs.clone()));

        Self {
            win_attribs,
            template_builder,
            display_builder,
            not_current_gl_context: None,
            scenes: None,
            state: None,

            viewport: IVec2::default(),
            mouse_pos: Vec2::default(),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let (mut window, gl_config) = match self.display_builder.clone().build(
            event_loop,
            self.template_builder.clone(),
            gl_config_picker,
        ) {
            Ok(ok) => ok,
            Err(e) => {
                eprintln!("Error: {e}");
                event_loop.exit();
                return;
            }
        };

        println!("Chosen OpenGL config:");
        debug_gl_config(&gl_config);

        let raw_window_handle = window
            .as_ref()
            .and_then(|window| window.window_handle().ok())
            .map(|handle| handle.as_raw());

        // XXX The display could be obtained from any object created by it, so we can
        // query it from the config.
        let gl_display = gl_config.display();

        // The context creation part.
        let context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(None))
            .build(raw_window_handle);

        // Since glutin by default tries to create OpenGL core context, which may not be
        // present we should try gles.
        let fallback_context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(None))
            .build(raw_window_handle);

        // There are also some old devices that support neither modern OpenGL nor GLES.
        // To support these we can try and create a 2.1 context.
        let legacy_context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(Some(Version::new(2, 1))))
            .build(raw_window_handle);

        self.not_current_gl_context.replace(unsafe {
            gl_display
                .create_context(&gl_config, &context_attributes)
                .unwrap_or_else(|_| {
                    gl_display
                        .create_context(&gl_config, &fallback_context_attributes)
                        .unwrap_or_else(|_| {
                            gl_display
                                .create_context(&gl_config, &legacy_context_attributes)
                                .expect("failed to create context")
                        })
                })
        });

        let window = Rc::new(window.take().unwrap_or_else(|| {
            glutin_winit::finalize_window(event_loop, self.win_attribs.clone(), &gl_config).unwrap()
        }));

        let surface_attribs = window
            .build_surface_attributes(Default::default())
            .expect("Failed to build surface attributes");
        let gl_surface = unsafe {
            gl_config
                .display()
                .create_window_surface(&gl_config, &surface_attribs)
                .unwrap()
        };

        // Make it current.
        let gl_context = (self.not_current_gl_context)
            .take()
            .unwrap()
            .make_current(&gl_surface)
            .unwrap();

        // Load OpenGL functions.
        gl::load_with(|symbol| {
            let symbol = CString::new(symbol).unwrap();
            gl_display.get_proc_address(symbol.as_c_str()).cast()
        });

        // Print some OpenGL constants
        unsafe {
            if let Some(renderer) = get_gl_string(gl::RENDERER) {
                println!("Renderer:    {}", renderer.to_string_lossy());
            }
            if let Some(version) = get_gl_string(gl::VERSION) {
                println!("OpenGL ver:  {}", version.to_string_lossy());
            }
            if let Some(shaders_version) = get_gl_string(gl::SHADING_LANGUAGE_VERSION) {
                println!("Shaders ver: {}", shaders_version.to_string_lossy());
            }

            // Check for "GL_KHR_debug" support (not present on Apple *OS).
            let extensions = get_opengl_extensions();

            if extensions.contains("GL_KHR_debug") {
                println!("Debug ext:   supported\n");
                gl::DebugMessageCallback(Some(debug_message_callback), std::ptr::null());
                gl::Enable(gl::DEBUG_OUTPUT);

                common_gl::DEBUG_ENABLED.store(true, Ordering::Relaxed);
            } else {
                println!("Debug ext:   unsupported\n");
            }
        }

        // The context needs to be current for the Renderer to set up shaders and
        // buffers.
        self.scenes.get_or_insert_with(|| {
            let scenes = Scenes::new(window.as_ref());
            let scene_controller = SceneController::new(window.scale_factor() as f32, 0.5);
            (scenes, scene_controller)
        });

        let win_size = window.inner_size();
        self.viewport = IVec2::new(win_size.width as i32, win_size.height as i32);

        // Try setting vsync.
        if let Err(res) = gl_surface
            .set_swap_interval(&gl_context, SwapInterval::Wait(NonZeroU32::new(1).unwrap()))
        {
            eprintln!("Error setting vsync: {res:?}");
        }

        let prev_state = (self.state).replace(AppState {
            gl_context,
            gl_surface,
            window,
        });

        assert!(prev_state.is_none());
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(size) if size.width != 0 && size.height != 0 => {
                // Some platforms like EGL require resizing GL surface to update the size
                // Notable platforms here are Wayland and macOS, other don't require it
                // and the function is no-op, but it's wise to resize it for portability
                // reasons.
                if let Some(AppState {
                    gl_context,
                    gl_surface,
                    ..
                }) = self.state.as_mut()
                {
                    gl_surface.resize(
                        gl_context,
                        NonZeroU32::new(size.width).unwrap(),
                        NonZeroU32::new(size.height).unwrap(),
                    );

                    self.viewport = IVec2::new(size.width as i32, size.height as i32);
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = Vec2::new(position.x as f32, position.y as f32);
            }

            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => event_loop.exit(),

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        ref logical_key,
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                if let Some(AppState { window, .. }) = self.state.as_ref() {
                    let (scenes, _) = self.scenes.as_mut().unwrap();
                    scenes.switch_scene(window, logical_key.clone());
                    scenes.on_key(logical_key.clone());
                }
            }

            _ => {}
        };

        if let Some((_, scene_ctrl)) = &mut self.scenes {
            scene_ctrl.interact(&event);
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(AppState {
            gl_context,
            gl_surface,
            window,
            ..
        }) = self.state.as_ref()
        {
            let (scenes, scene_ctrl) = self.scenes.as_mut().unwrap();

            scene_ctrl.update();
            scenes.resize(&scene_ctrl.camera, self.viewport.x, self.viewport.y);
            scenes.draw(&scene_ctrl.camera, self.mouse_pos);

            window.request_redraw();
            gl_surface.swap_buffers(gl_context).unwrap();
        }
    }
}

// Find the config with the maximum number of samples, so our triangle will be
// smooth.
pub fn gl_config_picker(configs: Box<dyn Iterator<Item = Config> + '_>) -> Config {
    configs
        // .map(|config| {
        //     debug_gl_config(&config);
        //     config
        // })
        .reduce(|accum, config| {
            if config.supports_transparency().unwrap_or(false)
                && !accum.supports_transparency().unwrap_or(false)
            {
                config
            } else {
                accum
            }
        })
        .unwrap()
}

fn debug_gl_config(gl_config: &glutin::config::Config) {
    println!(
        "  Color buffer type:     {:?}",
        gl_config.color_buffer_type()
    );
    println!("  Float pixels:          {:?}", gl_config.float_pixels());
    println!("  Alpha size:            {:?}", gl_config.alpha_size());
    println!("  Depth size:            {:?}", gl_config.depth_size());
    println!("  Stencil size:          {:?}", gl_config.stencil_size());
    println!("  Num samples:           {:?}", gl_config.num_samples());
    println!("  Srgb capable:          {:?}", gl_config.srgb_capable());
    println!(
        "  Config surface types:  {:?}",
        gl_config.config_surface_types()
    );
    println!(
        "  Hardware accelerated:  {:?}",
        gl_config.hardware_accelerated()
    );
    println!(
        "  Supports transparency: {:?}",
        gl_config.supports_transparency()
    );
    println!("  API:                   {:?}", gl_config.api());
    println!();
}

unsafe fn get_gl_string(variant: GLenum) -> Option<&'static CStr> {
    let s = gl::GetString(variant);
    (!s.is_null()).then(|| CStr::from_ptr(s.cast()))
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
    src: GLenum,
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
        gl::DEBUG_SEVERITY_NOTIFICATION => {
            if src != gl::DEBUG_SOURCE_APPLICATION {
                println!("[opengl debug] {ty}{msg}")
            }
        }
        gl::DEBUG_SEVERITY_LOW => println!("[opengl  info] {ty}{msg}"),
        gl::DEBUG_SEVERITY_MEDIUM => println!("[opengl  warn] {ty}{msg}"),
        gl::DEBUG_SEVERITY_HIGH => println!("[opengl error] {ty}{msg}"),
        sevr => unreachable!("unknown debug severity {sevr}"),
    };
}
