use std::rc::Rc;

use glam::{IVec2, UVec2, Vec2};
use scene_controller::SceneController;
use scenes::Scenes;
use tracing::Level;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Theme, Window, WindowAttributes},
};

use crate::context::GraphicsContext;

pub mod camera;
pub mod common;
pub mod context;
pub mod scene_controller;
pub mod scenes;

fn main() {
    let filter = tracing_subscriber::filter::Targets::new()
        .with_target("winit", Level::WARN)
        .with_default(Level::INFO);

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

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
    window: Rc<Window>,
    gl: Rc<glow::Context>,

    scenes: Scenes,
    scene_ctrl: SceneController,
}

struct App {
    win_attribs: WindowAttributes,
    graphics_context: GraphicsContext,

    state: Option<AppState>,

    viewport: IVec2,
    mouse_pos: Vec2,
}

impl App {
    fn new(win_attribs: WindowAttributes) -> Self {
        let graphics_context = GraphicsContext::new(&win_attribs);

        Self {
            win_attribs,
            graphics_context,
            state: None,

            viewport: IVec2::default(),
            mouse_pos: Vec2::default(),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let (window, gl) = match (self.graphics_context).create_window(event_loop, &self.win_attribs) {
            Ok(window) => window,
            Err(e) => {
                tracing::error!("Error: {e}");
                event_loop.exit();
                return;
            }
        };

        let gl = Rc::new(gl);

        // The context needs to be current for the Renderer to set up shaders and
        // buffers.
        let scenes = Scenes::new(gl.clone(), window.as_ref());
        let scene_controller = SceneController::new(window.scale_factor() as f32, 0.5);

        let win_size = window.inner_size();
        self.viewport = IVec2::new(win_size.width as i32, win_size.height as i32);

        let prev_state = (self.state).replace(AppState {
            window,
            gl,
            scenes,
            scene_ctrl: scene_controller,
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
                self.graphics_context.resize(size);
                self.viewport = UVec2::new(size.width, size.height).as_ivec2();
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
                if let Some(AppState { window, gl, scenes, .. }) = self.state.as_mut() {
                    scenes.switch_scene(gl.clone(), window, logical_key.clone());
                    scenes.on_key(logical_key.clone());
                }
            }

            WindowEvent::RedrawRequested => {
                if let Some(AppState {
                    scenes,
                    scene_ctrl,
                    window,
                    ..
                }) = self.state.as_mut()
                {
                    scene_ctrl.update();
                    scenes.resize(&scene_ctrl.camera, self.viewport.x, self.viewport.y);
                    scenes.draw(&scene_ctrl.camera, self.mouse_pos);

                    self.graphics_context.swap_buffers();

                    window.request_redraw();
                }
            }

            _ => {}
        };

        if let Some(AppState { scene_ctrl, .. }) = self.state.as_mut() {
            scene_ctrl.interact(&event);
        }
    }
}
