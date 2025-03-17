use crate::utils::BitExtract;

mod channel1;
mod channel2;
mod channel3;
mod channel4;

pub struct Apu {
    channel1: channel1::Channel1,
    channel2: channel2::Channel2,
    channel3: channel3::Channel3,
    channel4: channel4::Channel4,
    aram: [u8; 0x10],
    master_volume_vin_panning: u8,
    sound_panning: u8,
    master_enable: bool,
}

impl Apu {
    pub fn new() -> Self {
        Self {
            channel1: Default::default(),
            channel2: Default::default(),
            channel3: Default::default(),
            channel4: Default::default(),
            aram: [0; 0x10],
            master_volume_vin_panning: 0,
            sound_panning: 0,
            master_enable: false,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xff10..=0xff14 => self.channel1.read(addr),
            0xff16..=0xff19 => self.channel2.read(addr),
            0xff1a..=0xff1e => self.channel3.read(addr),
            0xff20..=0xff23 => self.channel4.read(addr),

            0xff24 => self.master_volume_vin_panning,
            0xff25 => self.sound_panning,
            0xff26 => ((self.master_enable as u8) << 7) | 0x70,

            0xff30..=0xff3f => self.aram[addr as usize - 0xff30],
            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        if addr != 0xff26 && !self.master_enable {
            return;
        }

        match addr {
            0xff10..=0xff14 => self.channel1.write(addr, val),
            0xff16..=0xff19 => self.channel2.write(addr, val),
            0xff1a..=0xff1e => self.channel3.write(addr, val),
            0xff20..=0xff23 => self.channel4.write(addr, val),

            0xff24 => self.master_volume_vin_panning = val,
            0xff25 => self.sound_panning = val,
            0xff26 => {
                let bit = val.bit(7);
                self.master_enable = bit;
                if !bit {
                    // Powering off APU resets all registers to 0
                    self.channel1 = Default::default();
                    self.channel2 = Default::default();
                    self.channel3 = Default::default();
                    self.channel4 = Default::default();
                    self.master_volume_vin_panning = 0;
                    self.sound_panning = 0;
                }
            }

            0xff30..=0xff3f => self.aram[addr as usize - 0xff30] = val,
            _ => unreachable!(),
        }
    }
}
