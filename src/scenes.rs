pub mod blurring;
pub mod kawase;
pub mod osu_slider;
pub mod round_quads;

use std::rc::Rc;

use blurring::BlurringScene;
use kawase::KawaseScene;
use round_quads::RoundQuadsScene;

use glam::Vec2;
use winit::keyboard::{Key, NamedKey, SmolStr};
use winit::window::Window;

use crate::camera::Camera;
use crate::scenes::osu_slider::OsuSliderScene;

// shaders
const SRC_FRAG_BLUR: &str = include_str!("../assets/shaders/blur.frag");
const SRC_FRAG_DITHER: &str = include_str!("../assets/shaders/dither.frag");
const SRC_FRAG_KAWASE: &str = include_str!("../assets/shaders/kawase.frag");
const SRC_VERT_QUAD: &str = include_str!("../assets/shaders/quad.vert");
const SRC_VERT_ROUND_RECT: &str = include_str!("../assets/shaders/round-rect.vert");
const SRC_FRAG_ROUND_RECT: &str = include_str!("../assets/shaders/round-rect.frag");
const SRC_VERT_SCREEN: &str = include_str!("../assets/shaders/screen.vert");
const SRC_VERT_SLIDER: &str = include_str!("../assets/shaders/slider.vert");
const SRC_FRAG_SLIDER: &str = include_str!("../assets/shaders/slider.frag");
const SRC_FRAG_SLIDER_POINT: &str = include_str!("../assets/shaders/slider_point.frag");
const SRC_FRAG_TEXTURE: &str = include_str!("../assets/shaders/texture.frag");

// images
const GURA_JPG: &[u8] = include_bytes!("../assets/gura.jpg");
const SLIDERBODY1_PNG: &[u8] = include_bytes!("../assets/sliderbody1.png");
const SLIDERBODY2_PNG: &[u8] = include_bytes!("../assets/sliderbody2.png");
const SLIDERBODY3_PNG: &[u8] = include_bytes!("../assets/sliderbody3.png");
// const BIG_SQUARES_PNG: &[u8] = include_bytes!("../../assets/big-squares.png");

pub enum Scenes {
    RoundQuads(RoundQuadsScene),
    Blurring(BlurringScene),
    Kawase(KawaseScene),
    OsuSlider(OsuSliderScene),
}

impl Scenes {
    pub fn new(gl: Rc<glow::Context>, window: &Window) -> Self {
        Self::OsuSlider(OsuSliderScene::new(gl, window))
    }

    pub fn switch_scene(&mut self, gl: Rc<glow::Context>, window: &Window, keycode: Key<SmolStr>) {
        match keycode {
            Key::Named(NamedKey::F1) => *self = Self::RoundQuads(RoundQuadsScene::new(gl, window)),
            Key::Named(NamedKey::F2) => *self = Self::Blurring(BlurringScene::new(gl, window)),
            Key::Named(NamedKey::F3) => *self = Self::Kawase(KawaseScene::new(gl, window)),
            Key::Named(NamedKey::F4) => *self = Self::OsuSlider(OsuSliderScene::new(gl, window)),
            _ => (),
        }
    }

    pub fn on_key(&mut self, keycode: Key<SmolStr>) {
        match self {
            Self::RoundQuads(_) => {}
            Self::Blurring(scene) => scene.on_key(keycode),
            Self::Kawase(scene) => scene.on_key(keycode),
            Self::OsuSlider(_) => {}
        }
    }

    pub fn draw(&mut self, camera: &Camera, mouse_pos: Vec2) {
        match self {
            Self::RoundQuads(scene) => scene.draw(camera, mouse_pos),
            Self::Blurring(scene) => scene.draw(camera, mouse_pos),
            Self::Kawase(scene) => scene.draw(camera, mouse_pos),
            Self::OsuSlider(scene) => scene.draw(camera, mouse_pos),
        }
    }

    pub fn resize(&mut self, camera: &Camera, width: i32, height: i32) {
        match self {
            Self::RoundQuads(scene) => scene.resize(camera, width, height),
            Self::Blurring(scene) => scene.resize(camera, width, height),
            Self::Kawase(scene) => scene.resize(camera, width, height),
            Self::OsuSlider(scene) => scene.resize(camera, width, height),
        }
    }
}
