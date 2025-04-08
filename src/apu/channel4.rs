use super::utils::{LengthCounter, VolumeEnvelope};
use crate::utils::BitExtract;

#[derive(Default)]
pub struct Channel4 {
    clock_divider: u8,
    lfsr_width: bool,
    clock_shift: u8,
    trigger: bool,

    lfsr: u16,
    period_counter: u16,
    frame_sequence: u8,
    length: LengthCounter<64>,
    volume: VolumeEnvelope,
    dac_enabled: bool,
}

impl Channel4 {
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xff20 => 0xff,
            0xff21 => {
                (self.volume.initial_level << 4)
                    | ((self.volume.direction as u8) << 3)
                    | self.volume.pace
            }
            0xff22 => (self.clock_shift << 4) | ((self.lfsr_width as u8) << 3) | self.clock_divider,
            0xff23 => ((self.length.enable as u8) << 6) | 0xbf,

            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xff20 => self.length.set_timer(val & 0b111111),
            0xff21 => {
                self.volume.pace = val & 0b111;
                self.volume.direction = val.bit(3);
                self.volume.initial_level = val >> 4;

                self.dac_enabled = val & 0b11111000 != 0;
                if !self.dac_enabled {
                    self.trigger = false;
                }
            }
            0xff22 => {
                self.clock_divider = val & 0b111;
                self.lfsr_width = val.bit(3);
                self.clock_shift = val >> 4;
            }
            0xff23 => {
                self.length.enable = val.bit(6);
                if val.bit(7) && self.dac_enabled {
                    self.trigger = true;
                    self.lfsr = 0x7fff;
                    self.length.trigger();
                    self.volume.trigger();
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
            self.period_counter = self.period_counter.saturating_sub(1);
            if self.period_counter == 0 {
                let divider = if self.clock_divider == 0 {
                    2
                } else {
                    (self.clock_divider << 2) as u16
                };
                self.period_counter = divider << self.clock_shift as u16;
                let xor = (self.lfsr & 1) ^ ((self.lfsr >> 1) & 1);
                self.lfsr = (self.lfsr >> 1) | (xor << 14);
                if self.lfsr_width {
                    self.lfsr &= !(1 << 6);
                    self.lfsr |= xor << 6;
                }
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
    }

    pub fn sample(&self) -> f32 {
        if self.dac_enabled {
            let sample = (!self.lfsr & 1) as u8;
            (self.volume.get_level() as f32 * (sample as f32 - 0.5)) / 15.0
        } else {
            0.0
        }
    }
}
