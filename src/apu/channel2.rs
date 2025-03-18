use super::DUTY_CYCLES;
use crate::utils::BitExtract;

#[derive(Default)]
pub struct Channel2 {
    length: u8,
    duty: u8,
    initial_volume: u8,
    volume_direction: bool,
    volume_pace: u8,
    period: u16,
    length_enable: bool,
    trigger: bool,

    duty_position: u8,
    period_counter: u16,
    frame_sequence: u8,
    length_timer: u8,
    dac_enabled: bool,
}

impl Channel2 {
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xff16 => (self.duty << 6) | 0x3f,
            0xff17 => {
                (self.initial_volume << 4) | ((self.volume_direction as u8) << 3) | self.volume_pace
            }
            0xff18 => 0xff,
            0xff19 => ((self.length_enable as u8) << 6) | 0xbf,
            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xff16 => {
                self.length = val & 0b111111;
                self.length_timer = 64 - self.length;
                self.duty = (val >> 6) & 0b11;
            }
            0xff17 => {
                self.volume_pace = val & 0b111;
                self.volume_direction = val.bit(3);
                self.initial_volume = val >> 4;

                self.dac_enabled = val & 0b11111000 != 0;
                if !self.dac_enabled {
                    self.trigger = false;
                }
            }
            0xff18 => {
                self.period &= !0xff;
                self.period |= val as u16;
            }
            0xff19 => {
                self.period &= 0xff;
                self.period |= ((val & 0b111) as u16) << 8;
                self.length_enable = val.bit(6);

                if val.bit(7) && self.dac_enabled {
                    self.trigger = true;
                    self.period_counter = self.period;
                    if self.length_timer == 0 {
                        self.length_timer = 64;
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.trigger
    }

    pub fn tick(&mut self) {
        if self.trigger {
            self.period_counter += 1;
            if self.period_counter == 2048 {
                self.duty_position = (self.duty_position + 1) % 8;
                self.period_counter = self.period
            }
        }
    }

    pub fn tick_frame_sequencer(&mut self) {
        self.frame_sequence = (self.frame_sequence + 1) % 8;
        if self.frame_sequence % 2 == 0 && self.length_enable {
            self.length_timer = self.length_timer.saturating_sub(1);
            if self.length_timer == 0 {
                self.trigger = false;
            }
        }
    }

    pub fn sample(&self) -> f32 {
        ((DUTY_CYCLES[self.duty as usize] >> (7 - self.duty_position)) & 1) as f32
    }
}
