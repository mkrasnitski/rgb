use std::collections::HashMap;
use winit::keyboard::KeyCode;

pub struct KeyMap {
    map: HashMap<KeyCode, Hotkey>,
}

impl KeyMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::from([
                (KeyCode::ArrowUp, Hotkey::Joypad(JoypadButton::Up)),
                (KeyCode::ArrowDown, Hotkey::Joypad(JoypadButton::Down)),
                (KeyCode::ArrowLeft, Hotkey::Joypad(JoypadButton::Left)),
                (KeyCode::ArrowRight, Hotkey::Joypad(JoypadButton::Right)),
                (KeyCode::KeyX, Hotkey::Joypad(JoypadButton::A)),
                (KeyCode::KeyZ, Hotkey::Joypad(JoypadButton::B)),
                (KeyCode::Enter, Hotkey::Joypad(JoypadButton::Start)),
                (KeyCode::Tab, Hotkey::Joypad(JoypadButton::Select)),
                (KeyCode::Space, Hotkey::ToggleFrameLimiter),
            ]),
        }
    }

    pub fn get_hotkey(&self, key: &KeyCode) -> Option<Hotkey> {
        self.map.get(key).copied()
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
