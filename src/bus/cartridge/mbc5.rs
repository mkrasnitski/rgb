use anyhow::Result;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use super::Mapper;

pub struct MBC5 {
    rom: Vec<u8>,
    num_banks: u16,
    bank: u16,
}

impl MBC5 {
    pub fn new(rom: Vec<u8>, num_banks: u16) -> Self {
        Self {
            rom,
            num_banks,
            bank: 1,
        }
    }
}

impl Mapper for MBC5 {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3fff => self.rom[addr as usize],
            0x4000..=0x7fff => self.rom[self.bank as usize * 0x4000 + addr as usize - 0x4000],
            _ => unreachable!(),
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1fff => {}
            0x2000..=0x2fff => self.bank = ((self.bank & 0x100) | val as u16) % self.num_banks,
            0x3000..=0x3fff => {
                self.bank = (((val as u16 & 1) << 8) | (self.bank & 0xff)) % self.num_banks;
            }
            0x4000..=0x7fff => {}
            _ => unreachable!(),
        }
    }
}

pub struct MBC5Ram {
    mbc5: MBC5,
    ram: Vec<u8>,
    ram_bank: u8,
    ram_enabled: bool,
}

impl MBC5Ram {
    pub fn new(rom: Vec<u8>, num_banks: u16, ram_size: u32) -> Self {
        Self {
            mbc5: MBC5::new(rom, num_banks),
            ram: vec![0; ram_size as usize],
            ram_bank: 0,
            ram_enabled: false,
        }
    }
}

impl Mapper for MBC5Ram {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7fff => self.mbc5.read(addr),
            0xa000..=0xbfff => {
                if self.ram_enabled {
                    self.ram[self.ram_bank as usize * 0x2000 + addr as usize - 0xa000]
                } else {
                    0xFF
                }
            }
            _ => unreachable!(),
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1fff => self.ram_enabled = (val & 0xf) == 0xA,
            0x2000..=0x3fff => self.mbc5.write(addr, val),
            0x4000..=0x5fff => {
                let num_ram_banks = self.ram.len().div_ceil(0x2000) as u8;
                self.ram_bank = (val & 0b1111) % num_ram_banks;
            }
            0x6000..=0x7fff => {}
            0xa000..=0xbfff => {
                if self.ram_enabled {
                    self.ram[self.ram_bank as usize * 0x2000 + addr as usize - 0xa000] = val;
                }
            }
            _ => unreachable!(),
        }
    }

    fn save_external_ram(&self, filename: &Path) -> Result<()> {
        let mut file = File::create(filename)?;
        file.write_all(&self.ram)?;
        Ok(())
    }

    fn load_external_ram(&mut self, filename: &Path) -> Result<()> {
        if let Ok(mut file) = File::open(filename) {
            file.read_exact(&mut self.ram)?;
        }
        Ok(())
    }
}
