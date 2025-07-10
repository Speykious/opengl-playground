use std::error::Error;
use std::ffi::CString;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use glow::HasContext;
use glutin::config::{Config, ConfigTemplateBuilder, GlConfig};
use glutin::context::{ContextApi, ContextAttributesBuilder, PossiblyCurrentContext, Version};
use glutin::display::GetGlDisplay;
use glutin::prelude::{GlDisplay, NotCurrentGlContext};
use glutin::surface::{GlSurface, Surface, SwapInterval, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use winit::dpi::PhysicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::raw_window_handle::HasWindowHandle;
use winit::window::{Window, WindowAttributes};

struct GlContext {
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
}

pub struct GraphicsContext {
    template_builder: ConfigTemplateBuilder,
    display_builder: DisplayBuilder,
    ctx: Option<GlContext>,
}

impl GraphicsContext {
    pub fn new(win_attribs: &WindowAttributes) -> Self {
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
			// .with_transparency(cfg!(target_os = "macos"))
			// .with_multisampling(4)
			;

        let display_builder = DisplayBuilder::new().with_window_attributes(Some(win_attribs.clone()));

        Self {
            template_builder,
            display_builder,
            ctx: None,
        }
    }

    pub fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        win_attribs: &WindowAttributes,
    ) -> Result<(Rc<Window>, glow::Context), Box<dyn Error>> {
        let (mut window, gl_config) =
            self.display_builder
                .clone()
                .build(event_loop, self.template_builder.clone(), gl_config_picker)?;

        tracing::info!("Chosen OpenGL config:");
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
            .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
            .build(raw_window_handle);

        // Since glutin by default tries to create OpenGL core context, which may not be
        // present we should try gles.
        let fallback_context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(None))
            .build(raw_window_handle);

        let not_current_gl_context = unsafe {
            gl_display
                .create_context(&gl_config, &context_attributes)
                .unwrap_or_else(|_| {
                    gl_display
                        .create_context(&gl_config, &fallback_context_attributes)
                        .expect("failed to create context")
                })
        };

        let window =
            Rc::new(window.take().unwrap_or_else(|| {
                glutin_winit::finalize_window(event_loop, win_attribs.clone(), &gl_config).unwrap()
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
        let gl_context = not_current_gl_context.make_current(&gl_surface).unwrap();

        // Load OpenGL functions.
        let mut gl = unsafe {
            glow::Context::from_loader_function(|symbol| {
                gl_display.get_proc_address(&CString::new(symbol).unwrap()) as *const _
            })
        };

        // Print some OpenGL constants
        unsafe {
            let renderer = gl.get_parameter_string(glow::RENDERER);
            tracing::info!("Renderer:    {}", renderer);

            let version = gl.get_parameter_string(glow::VERSION);
            tracing::info!("OpenGL ver:  {}", version);

            let shaders_version = gl.get_parameter_string(glow::SHADING_LANGUAGE_VERSION);
            tracing::info!("Shaders ver: {}", shaders_version);

            // Check for "GL_KHR_debug" support (not present on Apple *OS).
            let extensions = gl.supported_extensions();

            if extensions.contains("GL_KHR_debug") {
                tracing::info!("Debug ext:   supported");
                gl.enable(glow::DEBUG_OUTPUT);
                gl.enable(glow::DEBUG_OUTPUT_SYNCHRONOUS);
                gl.debug_message_callback(debug_message_callback);

                OPENGL_DEBUG_ENABLED.store(true, Ordering::Relaxed);
            } else {
                tracing::info!("Debug ext:   unsupported");
            }

            tracing::info!("");

            let max_texture_size = gl.get_parameter_i32(glow::MAX_TEXTURE_SIZE);
            tracing::info!("Max texture size:           {}", max_texture_size);

            let max_3d_texture_size = gl.get_parameter_i32(glow::MAX_3D_TEXTURE_SIZE);
            tracing::info!("Max 3D texture size:        {}", max_3d_texture_size);

            let max_rectangle_texture_size = gl.get_parameter_i32(glow::MAX_RECTANGLE_TEXTURE_SIZE);
            tracing::info!("Max rectangle texture size: {}", max_rectangle_texture_size);

            if max_texture_size == 0 {
                tracing::warn!("Max texture size is a mystery. Weird GPU detected.");
            } else if max_texture_size < 2048 {
                tracing::warn!(
                    "Max texture size is {} (less than 2048), device might be very old. Performance might be suboptimal.",
                    max_texture_size
                );
            }
        }

        // Try unsetting vsync.
        if let Err(res) = gl_surface.set_swap_interval(&gl_context, SwapInterval::DontWait) {
            tracing::error!("Error unsetting vsync: {res:?}");
        }

        // // Try setting vsync.
        // if let Err(res) = gl_surface.set_swap_interval(&gl_context, SwapInterval::Wait(NonZeroU32::MIN)) {
        // 	tracing::error!("Error setting vsync: {res:?}");
        // }

        self.ctx = Some(GlContext { gl_context, gl_surface });

        Ok((window, gl))
    }

    pub fn resize(&self, size: PhysicalSize<u32>) {
        // Some platforms like EGL require resizing GL surface to update the size
        // Notable platforms here are Wayland and macOS, other don't require it
        // and the function is no-op, but it's wise to resize it for portability
        // reasons.
        if let Some(ctx) = &self.ctx {
            ctx.gl_surface.resize(
                &ctx.gl_context,
                NonZeroU32::new(size.width).unwrap(),
                NonZeroU32::new(size.height).unwrap(),
            );
        }
    }

    pub fn swap_buffers(&self) {
        if let Some(ctx) = &self.ctx {
            ctx.gl_surface.swap_buffers(&ctx.gl_context).unwrap();
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
            if !config.supports_transparency().unwrap_or(false) && accum.supports_transparency().unwrap_or(false) {
                config
            } else {
                accum
            }
        })
        .unwrap()
}

fn debug_gl_config(c: &glutin::config::Config) {
    tracing::info!("  Color buffer type:     {:?}", c.color_buffer_type());
    tracing::info!("  Float pixels:          {:?}", c.float_pixels());
    tracing::info!("  Alpha size:            {:?}", c.alpha_size());
    tracing::info!("  Depth size:            {:?}", c.depth_size());
    tracing::info!("  Stencil size:          {:?}", c.stencil_size());
    tracing::info!("  Num samples:           {:?}", c.num_samples());
    tracing::info!("  Srgb capable:          {:?}", c.srgb_capable());
    tracing::info!("  Config surface types:  {:?}", c.config_surface_types());
    tracing::info!("  Hardware accelerated:  {:?}", c.hardware_accelerated());
    tracing::info!("  Supports transparency: {:?}", c.supports_transparency());
    tracing::info!("  API:                   {:?}", c.api());
    tracing::info!("");
}

// --- debugging ---

// Set in main when checking for the GL_KHR_debug extension.
static OPENGL_DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn is_opengl_debug_enabled() -> bool {
    OPENGL_DEBUG_ENABLED.load(Ordering::Relaxed)
}

fn debug_message_callback(src: u32, ty: u32, _id: u32, severity: u32, msg: &str) {
    let ty = match ty {
        glow::DEBUG_TYPE_ERROR => "Error: ",
        glow::DEBUG_TYPE_DEPRECATED_BEHAVIOR => "Deprecated Behavior: ",
        glow::DEBUG_TYPE_MARKER => "Marker: ",
        glow::DEBUG_TYPE_OTHER => "",
        glow::DEBUG_TYPE_POP_GROUP => "Pop Group: ",
        glow::DEBUG_TYPE_PORTABILITY => "Portability: ",
        glow::DEBUG_TYPE_PUSH_GROUP => "Push Group: ",
        glow::DEBUG_TYPE_UNDEFINED_BEHAVIOR => "Undefined Behavior: ",
        glow::DEBUG_TYPE_PERFORMANCE => "Performance: ",
        ty => unreachable!("unknown debug type {ty}"),
    };

    match severity {
        glow::DEBUG_SEVERITY_NOTIFICATION => {
            if src != glow::DEBUG_SOURCE_APPLICATION {
                tracing::debug!(target: "opengl", "{ty}{msg}")
            }
        }
        glow::DEBUG_SEVERITY_LOW => tracing::info!(target: "opengl", "{ty}{msg}"),
        glow::DEBUG_SEVERITY_MEDIUM => tracing::warn!(target: "opengl", "{ty}{msg}"),
        glow::DEBUG_SEVERITY_HIGH => tracing::error!(target: "opengl", "{ty}{msg}"),
        sevr => unreachable!("unknown debug severity {sevr}"),
    };
}
