pub struct LengthCounter<const N: u16> {
    enable: bool,
    timer: u16,
    tick: bool,
}

impl<const N: u16> LengthCounter<N> {
    pub fn trigger(&mut self) {
        if self.timer == 0 {
            self.timer = if self.tick && self.enable { N - 1 } else { N };
        }
    }

    pub fn set_timer(&mut self, val: u8) {
        self.timer = N - val as u16;
    }

    pub fn set_enable(&mut self, enable: bool) -> bool {
        let extra_clock = self.tick && enable && !self.enable;
        self.enable = enable;
        if extra_clock { self.clock() } else { false }
    }

    pub fn is_enabled(&self) -> bool {
        self.enable
    }

    pub fn tick(&mut self) -> bool {
        self.tick = !self.tick;
        self.tick && self.clock()
    }

    fn clock(&mut self) -> bool {
        if self.enable {
            self.timer = self.timer.saturating_sub(1);
            self.timer == 0
        } else {
            false
        }
    }
}

impl<const N: u16> Default for LengthCounter<N> {
    fn default() -> Self {
        Self {
            enable: false,
            timer: 0,
            tick: true,
        }
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

#[derive(Default)]
pub struct SweepEnvelope {
    pub step: u8,
    direction: bool,
    pub pace: u8,

    shadow_period: u16,
    pace_timer: u8,
    enabled: bool,
    negate_mode: bool,
}

impl SweepEnvelope {
    pub fn trigger(&mut self, period: u16) -> bool {
        self.shadow_period = period;
        self.pace_timer = self.get_pace();
        self.enabled = self.pace != 0 || self.step != 0;
        self.negate_mode = false;
        self.step != 0 && self.next_period() > 2047
    }

    pub fn tick(&mut self) -> Option<(Option<u16>, bool)> {
        if self.pace_timer != 0 {
            self.pace_timer -= 1;
            if self.pace_timer == 0 {
                self.pace_timer = self.get_pace();
                if self.enabled && self.pace != 0 {
                    let next_period = self.next_period();
                    if next_period > 2047 {
                        return Some((None, true));
                    } else if self.step > 0 {
                        self.shadow_period = next_period;
                        return Some((Some(next_period), self.next_period() > 2047));
                    }
                }
            }
        }
        None
    }

    pub fn get_direction(&self) -> bool {
        self.direction
    }

    pub fn set_direction(&mut self, direction: bool) -> bool {
        self.direction = direction;
        self.negate_mode && !direction
    }

    fn get_pace(&self) -> u8 {
        if self.pace == 0 { 8 } else { self.pace }
    }

    fn next_period(&mut self) -> u16 {
        let step = self.shadow_period >> self.step;
        if self.direction {
            self.negate_mode = true;
            self.shadow_period.wrapping_sub(step)
        } else {
            self.shadow_period.wrapping_add(step)
        }
    }
}
