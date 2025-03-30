#[derive(Default)]
pub struct LengthCounter {
    pub enable: bool,
    timer: u8,
}

impl LengthCounter {
    pub fn trigger(&mut self) {
        if self.timer == 64 {
            self.timer = 0;
        }
    }

    pub fn set_timer(&mut self, val: u8) {
        self.timer = val;
    }

    pub fn tick(&mut self) -> bool {
        if self.enable {
            let next = self.timer + 1;
            self.timer = std::cmp::min(next, 64);
            return self.timer == 64;
        }
        false
    }
}

#[derive(Default)]
pub struct VolumeEnvelope {
    pub pace: u8,
    pub direction: bool,
    pub initial_level: u8,

    level: u8,
    pace_timer: u8,
}

impl VolumeEnvelope {
    pub fn trigger(&mut self) {
        self.level = self.initial_level;
        self.pace_timer = self.get_pace();
    }

    pub fn tick(&mut self) {
        if self.pace != 0 {
            self.pace_timer -= 1;
            if self.pace_timer == 0 {
                if self.direction {
                    self.level = std::cmp::min(15, self.level + 1);
                } else {
                    self.level = self.level.saturating_sub(1);
                }
                self.pace_timer = self.get_pace();
            }
        }
    }

    pub fn get_level(&self) -> u8 {
        self.level
    }

    fn get_pace(&self) -> u8 {
        if self.pace == 0 { 8 } else { self.pace }
    }
}
