use std::{num::NonZeroU32, rc::Rc};

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
use renderer::Renderer;
use winit::{
    application::ApplicationHandler,
    event::{KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    raw_window_handle::HasWindowHandle as _,
    window::{Theme, Window, WindowAttributes},
};

pub mod renderer;
pub mod camera;

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new();

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
    renderer: Option<Renderer>,
    state: Option<AppState>,
}

impl App {
    fn new() -> Self {
        let win_attribs = WindowAttributes::default()
            .with_active(true)
            .with_transparent(true)
            .with_theme(Some(Theme::Dark))
            .with_title("OpenGL Squares")
            .with_resizable(true);

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
            renderer: None,
            state: None,
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

        println!("Picked a config with {} samples", gl_config.num_samples());

        let raw_window_handle = window
            .as_ref()
            .and_then(|window| window.window_handle().ok())
            .map(|handle| handle.as_raw());

        // XXX The display could be obtained from any object created by it, so we can
        // query it from the config.
        let gl_display = gl_config.display();

        // The context creation part.
        let context_attributes = ContextAttributesBuilder::new().build(raw_window_handle);

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

        // The context needs to be current for the Renderer to set up shaders and
        // buffers. It also performs function loading, which needs a current context on
        // WGL.
        self.renderer
            .get_or_insert_with(|| Renderer::new(&gl_display));

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
                    window: _,
                }) = self.state.as_ref()
                {
                    gl_surface.resize(
                        gl_context,
                        NonZeroU32::new(size.width).unwrap(),
                        NonZeroU32::new(size.height).unwrap(),
                    );
                    let renderer = self.renderer.as_mut().unwrap();
                    renderer.resize(size.width as i32, size.height as i32);
                }
            }
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Escape),
                        ..
                    },
                ..
            } => event_loop.exit(),
            _ => (),
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(AppState {
            gl_context,
            gl_surface,
            window,
        }) = self.state.as_ref()
        {
            let renderer = self.renderer.as_mut().unwrap();
            renderer.draw();
            window.request_redraw();

            gl_surface.swap_buffers(gl_context).unwrap();
        }
    }
}

// Find the config with the maximum number of samples, so our triangle will be
// smooth.
pub fn gl_config_picker(configs: Box<dyn Iterator<Item = Config> + '_>) -> Config {
    configs
        .reduce(|accum, config| {
            let transparency_check = config.supports_transparency().unwrap_or(false)
                & !accum.supports_transparency().unwrap_or(false);

            if transparency_check || config.num_samples() > accum.num_samples() {
                config
            } else {
                accum
            }
        })
        .unwrap()
}
