pub mod blurring;
pub mod kawase;
pub mod round_quads;

use blurring::BlurringScene;
use kawase::KawaseScene;
use round_quads::RoundQuadsScene;

use glam::Vec2;
use winit::keyboard::{Key, NamedKey, SmolStr};
use winit::window::Window;

use crate::camera::Camera;

// shaders
const SRC_FRAG_BLUR: &[u8] = include_bytes!("../assets/shaders/blur.frag");
const SRC_FRAG_DITHER: &[u8] = include_bytes!("../assets/shaders/dither.frag");
const SRC_FRAG_KAWASE: &[u8] = include_bytes!("../assets/shaders/kawase.frag");
const SRC_VERT_QUAD: &[u8] = include_bytes!("../assets/shaders/quad.vert");
const SRC_VERT_ROUND_RECT: &[u8] = include_bytes!("../assets/shaders/round-rect.vert");
const SRC_FRAG_ROUND_RECT: &[u8] = include_bytes!("../assets/shaders/round-rect.frag");
const SRC_VERT_SCREEN: &[u8] = include_bytes!("../assets/shaders/screen.vert");
const SRC_FRAG_TEXTURE: &[u8] = include_bytes!("../assets/shaders/texture.frag");

// images
const GURA_JPG: &[u8] = include_bytes!("../assets/gura.jpg");
// const BIG_SQUARES_PNG: &[u8] = include_bytes!("../../assets/big-squares.png");

pub enum Scenes {
    RoundQuads(RoundQuadsScene),
    Blurring(BlurringScene),
    Kawase(KawaseScene),
}

impl Scenes {
    pub fn new(window: &Window) -> Self {
        Self::Kawase(KawaseScene::new(window))
    }

    pub fn switch_scene(&mut self, window: &Window, keycode: Key<SmolStr>) {
        match keycode {
            Key::Named(NamedKey::F1) => *self = Self::RoundQuads(RoundQuadsScene::new(window)),
            Key::Named(NamedKey::F2) => *self = Self::Blurring(BlurringScene::new(window)),
            Key::Named(NamedKey::F3) => *self = Self::Kawase(KawaseScene::new(window)),
            _ => (),
        }
    }

    pub fn on_key(&mut self, keycode: Key<SmolStr>) {
        match self {
            Self::RoundQuads(_) => {}
            Self::Blurring(scene) => scene.on_key(keycode),
            Self::Kawase(scene) => scene.on_key(keycode),
        }
    }

    pub fn draw(&mut self, camera: &Camera, mouse_pos: Vec2) {
        match self {
            Self::RoundQuads(scene) => scene.draw(camera, mouse_pos),
            Self::Blurring(scene) => scene.draw(camera, mouse_pos),
            Self::Kawase(scene) => scene.draw(camera, mouse_pos),
        }
    }

    pub fn resize(&mut self, camera: &Camera, width: i32, height: i32) {
        match self {
            Self::RoundQuads(scene) => scene.resize(camera, width, height),
            Self::Blurring(scene) => scene.resize(camera, width, height),
            Self::Kawase(scene) => scene.resize(camera, width, height),
        }
    }
}
