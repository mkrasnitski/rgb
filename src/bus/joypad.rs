use crate::hotkeys::JoypadButton;
use crate::utils::BitExtract;

pub struct Joypad {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    a: bool,
    b: bool,
    start: bool,
    select: bool,

    buttons: bool,
    dpad: bool,

    interrupt: bool,
}

impl Default for Joypad {
    fn default() -> Self {
        Self {
            up: false,
            down: false,
            left: false,
            right: false,
            a: false,
            b: false,
            start: false,
            select: false,

            buttons: true,
            dpad: true,

            interrupt: false,
        }
    }
}

impl Joypad {
    pub fn poll(&mut self) -> bool {
        if self.interrupt {
            self.interrupt = false;
            true
        } else {
            false
        }
    }

    pub fn update_button(&mut self, button: JoypadButton, pressed: bool) {
        let old_nibble = self.read_nibble();
        match button {
            JoypadButton::Up => self.up = pressed,
            JoypadButton::Down => self.down = pressed,
            JoypadButton::Left => self.left = pressed,
            JoypadButton::Right => self.right = pressed,
            JoypadButton::A => self.a = pressed,
            JoypadButton::B => self.b = pressed,
            JoypadButton::Start => self.start = pressed,
            JoypadButton::Select => self.select = pressed,
        }
        let new_nibble = self.read_nibble();
        if old_nibble == 0xF && new_nibble != 0xF {
            self.interrupt = true;
        }
    }

    pub fn read(&self) -> u8 {
        0xC0 | ((!self.buttons as u8) << 5) | ((!self.dpad as u8) << 4) | self.read_nibble()
    }

    fn read_nibble(&self) -> u8 {
        let mut nibble = 0;
        if self.dpad {
            nibble |= ((self.down as u8) << 3)
                | ((self.up as u8) << 2)
                | ((self.left as u8) << 1)
                | (self.right as u8);
        }
        if self.buttons {
            nibble |= ((self.start as u8) << 3)
                | ((self.select as u8) << 2)
                | ((self.b as u8) << 1)
                | (self.a as u8);
        }
        !nibble & 0xF
    }

    pub fn write(&mut self, val: u8) {
        let old_nibble = self.read_nibble();

        self.buttons = !val.bit(5);
        self.dpad = !val.bit(4);

        let new_nibble = self.read_nibble();
        if old_nibble == 0xF && new_nibble != 0xF {
            self.interrupt = true;
        }
    }
}
