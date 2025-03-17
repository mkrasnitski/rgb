use crate::utils::BitExtract;

#[derive(Default)]
pub struct Channel3 {
    dac_enabled: bool,
    length: u8,
    volume: u8,
    period: u16,
    length_enable: bool,
    trigger: bool,
}

impl Channel3 {
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xff1a => ((self.dac_enabled as u8) << 7) | 0x7f,
            0xff1b => 0xff,
            0xff1c => (self.volume << 5) | 0x9f,
            0xff1d => 0xff,
            0xff1e => ((self.length_enable as u8) << 6) | 0xbf,

            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xff1a => self.dac_enabled = val.bit(7),
            0xff1b => self.length = val,
            0xff1c => self.volume = (val >> 5) & 0b11,
            0xff1d => {
                self.period &= !0xff;
                self.period |= val as u16;
            }
            0xff1e => {
                self.period &= 0xff;
                self.period |= ((val & 0b111) as u16) << 8;
                self.length_enable = val.bit(6);
                self.trigger = val.bit(7);
            }

            _ => unreachable!(),
        }
    }
}
