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
                self.duty = (val >> 6) & 0b11;
            }
            0xff17 => {
                self.volume_pace = val & 0b111;
                self.volume_direction = val.bit(3);
                self.initial_volume = val >> 4;
            }
            0xff18 => {
                self.period &= !0xff;
                self.period |= val as u16;
            }
            0xff19 => {
                self.period &= 0xff;
                self.period |= ((val & 0b111) as u16) << 8;
                self.length_enable = val.bit(6);

                if val.bit(7) {
                    self.trigger = true;
                    self.period_counter = self.period;
                }
            }
            _ => unreachable!(),
        }
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

    pub fn sample(&self) -> f32 {
        ((DUTY_CYCLES[self.duty as usize] >> (7 - self.duty_position)) & 1) as f32
    }
}
