//! A nice scene controller to smoothly move around in the window.

use std::time::Instant;

use crate::camera::Camera;

use glam::{vec2, Vec2};
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};

pub struct SceneController {
    pub camera: Camera,

    // for camera position and mouse interactions
    mouse_pos: Vec2,
    mouse_pos_held: Vec2,
    mouse_state: ElementState,

    // for smooth scrolling
    pub scroll_speed: f32,
    hard_scale: Vec2,

    // for FPS-independent interactions
    start: Instant,
    prev_elapsed: f32,
    current_elapsed: f32,
}

impl SceneController {
    pub fn new(scale_factor: f32, scroll_speed: f32) -> Self {
        let scale = Vec2::splat(scale_factor);

        let camera = Camera {
            scale,
            ..Default::default()
        };

        Self {
            camera,
            mouse_pos: Vec2::default(),
            mouse_pos_held: Vec2::default(),
            mouse_state: ElementState::Released,
            scroll_speed,
            hard_scale: scale,
            start: Instant::now(),
            prev_elapsed: 0.0,
            current_elapsed: 0.0,
        }
    }

    pub fn update(&mut self) {
        // Smooth scrolling
        let time_delta = self.current_elapsed - self.prev_elapsed;
        self.camera.scale += time_delta.powf(0.6) * (self.hard_scale - self.camera.scale);

        // Mouse dragging
        if self.mouse_state == ElementState::Pressed {
            self.camera.position += (self.mouse_pos - self.mouse_pos_held) / self.camera.scale;
        }

        // Frame interval
        self.prev_elapsed = self.current_elapsed;
        self.current_elapsed = self.start.elapsed().as_secs_f32();
    }

    pub fn interact(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = vec2(position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput { state, .. } => {
                self.mouse_state = *state;
                if self.mouse_state == ElementState::Pressed {
                    self.mouse_pos_held = self.mouse_pos;
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // Handle mouse wheel (zoom)
                let my = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.,
                };

                self.hard_scale *= 2_f32.powf(self.scroll_speed * my);
            }
            _ => (),
        }
    }

    pub fn dt(&self) -> f32 {
        self.current_elapsed - self.prev_elapsed
    }

    pub fn current_elapsed(&self) -> f32 {
        self.current_elapsed
    }
}
