use super::DUTY_CYCLES;
use super::utils::{LengthCounter, SweepEnvelope, VolumeEnvelope};
use crate::utils::BitExtract;

#[derive(Default)]
pub struct Channel1 {
    duty: u8,
    period: u16,
    trigger: bool,

    duty_position: u8,
    period_counter: u16,
    frame_sequence: u8,
    length: LengthCounter<64>,
    volume: VolumeEnvelope,
    sweep: SweepEnvelope,
    dac_enabled: bool,
}

impl Channel1 {
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xff10 => {
                (self.sweep.pace << 4)
                    | ((self.sweep.get_direction() as u8) << 3)
                    | self.sweep.step
                    | 0x80
            }
            0xff11 => (self.duty << 6) | 0x3f,
            0xff12 => {
                (self.volume.initial_level << 4)
                    | ((self.volume.direction as u8) << 3)
                    | self.volume.pace
            }
            0xff13 => 0xff,
            0xff14 => ((self.length.enable as u8) << 6) | 0xbf,
            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xff10 => {
                self.sweep.step = val & 0b111;
                if self.sweep.set_direction(val.bit(3)) {
                    self.trigger = false;
                }
                self.sweep.pace = (val >> 4) & 0b111;
            }
            0xff11 => {
                self.length.set_timer(val & 0b111111);
                self.duty = (val >> 6) & 0b11;
            }
            0xff12 => {
                self.volume.pace = val & 0b111;
                self.volume.direction = val.bit(3);
                self.volume.initial_level = val >> 4;

                self.dac_enabled = val & 0b11111000 != 0;
                if !self.dac_enabled {
                    self.trigger = false;
                }
            }
            0xff13 => {
                self.period &= !0xff;
                self.period |= val as u16;
            }
            0xff14 => {
                self.period &= 0xff;
                self.period |= ((val & 0b111) as u16) << 8;
                self.length.enable = val.bit(6);

                if val.bit(7) && self.dac_enabled {
                    self.trigger = true;
                    self.period_counter = self.period;
                    self.length.trigger();
                    self.volume.trigger();
                    if self.sweep.trigger(self.period) {
                        self.trigger = false;
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
        if self.frame_sequence % 2 == 0 && self.length.tick() {
            self.trigger = false;
        }
        if self.frame_sequence == 7 {
            self.volume.tick();
        }
        if self.frame_sequence == 2 || self.frame_sequence == 6 {
            if let Some((next_period, disable)) = self.sweep.tick() {
                if let Some(period) = next_period {
                    self.period = period;
                }
                if disable {
                    self.trigger = false;
                }
            }
        }
    }

    pub fn sample(&self) -> f32 {
        let sample = (DUTY_CYCLES[self.duty as usize] >> (7 - self.duty_position)) & 1;
        if self.dac_enabled {
            ((self.volume.get_level() * sample) as f32 / 15.0) * 2.0 - 1.0
        } else {
            0.0
        }
    }
}
