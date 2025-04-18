use super::utils::LengthCounter;
use crate::utils::BitExtract;

#[derive(Default)]
pub struct Channel3 {
    dac_enabled: bool,
    volume: u8,
    period: u16,
    trigger: bool,

    sample_index: u8,
    period_counter: u16,
    length: LengthCounter<256>,
}

impl Channel3 {
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xff1a => ((self.dac_enabled as u8) << 7) | 0x7f,
            0xff1b => 0xff,
            0xff1c => (self.volume << 5) | 0x9f,
            0xff1d => 0xff,
            0xff1e => ((self.length.is_enabled() as u8) << 6) | 0xbf,

            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xff1a => {
                self.dac_enabled = val.bit(7);
                if !self.dac_enabled {
                    self.trigger = false;
                }
            }
            0xff1b => self.length.set_timer(val),
            0xff1c => self.volume = (val >> 5) & 0b11,
            0xff1d => {
                self.period &= !0xff;
                self.period |= val as u16;
            }
            0xff1e => {
                self.period &= 0xff;
                self.period |= ((val & 0b111) as u16) << 8;

                if self.length.set_enable(val.bit(6)) {
                    self.trigger = false;
                }

                if val.bit(7) {
                    if self.dac_enabled {
                        self.trigger = true;
                    }
                    self.length.trigger();
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
                self.sample_index = (self.sample_index + 1) % 32;
                self.period_counter = self.period
            }
        }
    }

    pub fn tick_frame_sequencer(&mut self) {
        if self.length.tick() {
            self.trigger = false;
        }
    }

    pub fn sample(&self, aram: &[u8; 16]) -> f32 {
        let samples = aram.map(|byte| [byte >> 4, byte & 0xf]);
        if self.dac_enabled {
            let sample = samples[self.sample_index as usize / 2][self.sample_index as usize % 2];
            let shift = match self.volume {
                0 => 4,
                1 => 0,
                2 => 1,
                3 => 2,
                _ => unreachable!(),
            };
            let max = samples.into_iter().flatten().max().unwrap_or_default();
            ((sample >> shift) as f32 - (max >> shift) as f32 / 2.0) / 15.0
        } else {
            0.0
        }
    }
}
