use super::DUTY_CYCLES;
use super::utils::{LengthCounter, SweepEnvelope, VolumeEnvelope};
use crate::utils::BitExtract;

#[derive(Default)]
pub struct Channel1 {
    duty: u8,
    period: u16,
    enabled: bool,

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
            0xff12 => self.volume.read(),
            0xff13 => 0xff,
            0xff14 => ((self.length.is_enabled() as u8) << 6) | 0xbf,
            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xff10 => {
                self.sweep.step = val & 0b111;
                if self.sweep.set_direction(val.bit(3)) {
                    self.enabled = false;
                }
                self.sweep.pace = (val >> 4) & 0b111;
            }
            0xff11 => {
                self.length.set_timer(val & 0b111111);
                self.duty = (val >> 6) & 0b11;
            }
            0xff12 => {
                self.volume.write(val, self.enabled);

                self.dac_enabled = val & 0b11111000 != 0;
                if !self.dac_enabled {
                    self.enabled = false;
                }
            }
            0xff13 => {
                self.period &= !0xff;
                self.period |= val as u16;
            }
            0xff14 => {
                self.period &= 0xff;
                self.period |= ((val & 0b111) as u16) << 8;

                if self.length.set_enable(val.bit(6)) {
                    self.enabled = false;
                }

                if val.bit(7) {
                    if self.dac_enabled {
                        self.enabled = true;
                    }
                    self.period_counter = self.period;
                    self.length.trigger();
                    self.volume.trigger();
                    if self.sweep.trigger(self.period) {
                        self.enabled = false;
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn power_on(&mut self) {
        self.frame_sequence = 7;
        self.length.reset_tick();
    }

    pub fn power_off(&mut self) {
        *self = Default::default()
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn tick(&mut self) {
        if self.enabled {
            self.period_counter += 1;
            if self.period_counter == 2048 {
                self.duty_position = (self.duty_position + 1) % 8;
                self.period_counter = self.period
            }
        }
    }

    pub fn tick_frame_sequencer(&mut self) {
        self.frame_sequence = (self.frame_sequence + 1) % 8;
        if self.length.tick() {
            self.enabled = false;
        }
        if self.frame_sequence == 7 {
            self.volume.tick();
        }
        if (self.frame_sequence == 2 || self.frame_sequence == 6)
            && let Some((next_period, disable)) = self.sweep.tick()
        {
            if let Some(period) = next_period {
                self.period = period;
            }
            if disable {
                self.enabled = false;
            }
        }
    }

    pub fn sample(&self) -> f32 {
        if self.dac_enabled {
            let sample = (DUTY_CYCLES[self.duty as usize] >> (7 - self.duty_position)) & 1;
            (self.volume.get_level() as f32 * (sample as f32 - 0.5)) / 15.0
        } else {
            0.0
        }
    }
}
