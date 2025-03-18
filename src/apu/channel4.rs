use crate::utils::BitExtract;

#[derive(Default)]
pub struct Channel4 {
    length: u8,
    initial_volume: u8,
    volume_direction: bool,
    volume_pace: u8,
    clock_divider: u8,
    lfsr_width: bool,
    clock_shift: u8,
    length_enable: bool,
    trigger: bool,
}

impl Channel4 {
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xff20 => 0xff,
            0xff21 => {
                (self.initial_volume << 4) | ((self.volume_direction as u8) << 3) | self.volume_pace
            }
            0xff22 => (self.clock_shift << 4) | ((self.lfsr_width as u8) << 3) | self.clock_divider,
            0xff23 => ((self.length_enable as u8) << 6) | 0xbf,

            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xff20 => self.length = val & 0b111111,
            0xff21 => {
                self.volume_pace = val & 0b111;
                self.volume_direction = val.bit(3);
                self.initial_volume = val >> 4;
            }
            0xff22 => {
                self.clock_divider = val & 0b111;
                self.lfsr_width = val.bit(3);
                self.clock_shift = val >> 4;
            }
            0xff23 => {
                self.length_enable = val.bit(6);
                self.trigger = val.bit(7);
            }

            _ => unreachable!(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.trigger
    }
}
