use super::Mapper;
use crate::utils::BitExtract;

pub struct MBC2 {
    rom: Vec<u8>,
    num_banks: u16,
    bank: u8,
    ram: Box<[u8; 0x1000]>,
    ram_enabled: bool,
}

impl MBC2 {
    pub fn new(rom: Vec<u8>, num_banks: u16) -> Self {
        Self {
            rom,
            num_banks,
            bank: 1,
            ram: Box::new([0xf0; 0x1000]),
            ram_enabled: false,
        }
    }
}

impl Mapper for MBC2 {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3fff => self.rom[addr as usize],
            0x4000..=0x7fff => self.rom[self.bank as usize * 0x4000 + addr as usize - 0x4000],
            0xa000..=0xbfff => {
                if self.ram_enabled {
                    self.ram[(addr as usize - 0xa000) % 0x200]
                } else {
                    0xFF
                }
            }
            _ => unreachable!(),
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x3fff => {
                if addr.bit(8) {
                    let val = val & 0xf;
                    self.bank = if val != 0 {
                        val % self.num_banks as u8
                    } else {
                        1
                    }
                } else {
                    self.ram_enabled = (val & 0xf) == 0xA
                }
            }
            0x4000..=0x7fff => {}
            0xa000..=0xbfff => {
                if self.ram_enabled {
                    self.ram[(addr as usize - 0xa000) % 0x200] = val | 0xf0;
                }
            }
            _ => unreachable!(),
        }
    }
}
