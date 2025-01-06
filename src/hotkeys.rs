use std::collections::HashMap;

use anyhow::Result;
use serde::de::{Deserializer, IntoDeserializer};
use serde::Deserialize;
use winit::keyboard::KeyCode as WinitKeyCode;

pub struct KeyMap {
    map: HashMap<WinitKeyCode, Hotkey>,
}

impl KeyMap {
    pub fn new(keys: &Keybindings) -> Self {
        Self {
            map: HashMap::from(
                [
                    (keys.joypad.up, Hotkey::Joypad(JoypadButton::Up)),
                    (keys.joypad.down, Hotkey::Joypad(JoypadButton::Down)),
                    (keys.joypad.left, Hotkey::Joypad(JoypadButton::Left)),
                    (keys.joypad.right, Hotkey::Joypad(JoypadButton::Right)),
                    (keys.joypad.a, Hotkey::Joypad(JoypadButton::A)),
                    (keys.joypad.b, Hotkey::Joypad(JoypadButton::B)),
                    (keys.joypad.start, Hotkey::Joypad(JoypadButton::Start)),
                    (keys.joypad.select, Hotkey::Joypad(JoypadButton::Select)),
                    (keys.emu.toggle_frame_limiter, Hotkey::ToggleFrameLimiter),
                ]
                .map(|(k, h)| (k.into(), h)),
            ),
        }
    }

    pub fn get_hotkey(&self, key: WinitKeyCode) -> Option<Hotkey> {
        self.map.get(&key).copied()
    }
}

#[derive(Copy, Clone)]
pub enum Hotkey {
    Joypad(JoypadButton),
    ToggleFrameLimiter,
}

#[derive(Copy, Clone)]
pub enum JoypadButton {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    Start,
    Select,
}

#[derive(Copy, Clone, Debug, Deserialize, Hash, Eq, PartialEq)]
#[serde(remote = "Self")]
pub enum KeyCode {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    Up,
    Down,
    Left,
    Right,

    Enter,
    Space,
    Tab,
}

impl From<KeyCode> for WinitKeyCode {
    fn from(keycode: KeyCode) -> Self {
        match keycode {
            KeyCode::A => Self::KeyA,
            KeyCode::B => Self::KeyB,
            KeyCode::C => Self::KeyC,
            KeyCode::D => Self::KeyD,
            KeyCode::E => Self::KeyE,
            KeyCode::F => Self::KeyF,
            KeyCode::G => Self::KeyG,
            KeyCode::H => Self::KeyH,
            KeyCode::I => Self::KeyI,
            KeyCode::J => Self::KeyJ,
            KeyCode::K => Self::KeyK,
            KeyCode::L => Self::KeyL,
            KeyCode::M => Self::KeyM,
            KeyCode::N => Self::KeyN,
            KeyCode::O => Self::KeyO,
            KeyCode::P => Self::KeyP,
            KeyCode::Q => Self::KeyQ,
            KeyCode::R => Self::KeyR,
            KeyCode::S => Self::KeyS,
            KeyCode::T => Self::KeyT,
            KeyCode::U => Self::KeyU,
            KeyCode::V => Self::KeyV,
            KeyCode::W => Self::KeyW,
            KeyCode::X => Self::KeyX,
            KeyCode::Y => Self::KeyY,
            KeyCode::Z => Self::KeyZ,

            KeyCode::Up => Self::ArrowUp,
            KeyCode::Down => Self::ArrowDown,
            KeyCode::Left => Self::ArrowLeft,
            KeyCode::Right => Self::ArrowRight,

            KeyCode::Enter => Self::Enter,
            KeyCode::Space => Self::Space,
            KeyCode::Tab => Self::Tab,
        }
    }
}

impl<'de> Deserialize<'de> for KeyCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        fn capitalize(s: &str) -> String {
            let mut c = s.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        }

        let val = String::deserialize(deserializer)?;
        Self::deserialize(capitalize(&val).into_deserializer())
    }
}

#[derive(Default, Deserialize)]
pub struct Keybindings {
    joypad: JoypadBindings,
    emu: EmuBindings,
}

#[derive(Deserialize)]
pub struct JoypadBindings {
    up: KeyCode,
    down: KeyCode,
    left: KeyCode,
    right: KeyCode,
    a: KeyCode,
    b: KeyCode,
    start: KeyCode,
    select: KeyCode,
}

impl Default for JoypadBindings {
    fn default() -> Self {
        JoypadBindings {
            up: KeyCode::Up,
            down: KeyCode::Down,
            left: KeyCode::Left,
            right: KeyCode::Right,
            a: KeyCode::X,
            b: KeyCode::Z,
            start: KeyCode::Enter,
            select: KeyCode::Tab,
        }
    }
}

#[derive(Deserialize)]
pub struct EmuBindings {
    toggle_frame_limiter: KeyCode,
}

impl Default for EmuBindings {
    fn default() -> Self {
        EmuBindings {
            toggle_frame_limiter: KeyCode::Space,
        }
    }
}
