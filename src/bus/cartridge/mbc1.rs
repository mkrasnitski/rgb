use anyhow::Result;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use super::Mapper;

pub struct MBC1 {
    rom: Vec<u8>,
    num_banks: u16,
    bank1: u8,
    bank2: u8,
    mode: bool,
}

impl MBC1 {
    pub fn new(rom: Vec<u8>, num_banks: u16) -> Self {
        Self {
            rom,
            num_banks,
            bank1: 1,
            bank2: 0,
            mode: false,
        }
    }

    fn lo_bank(&self) -> usize {
        if self.mode {
            (self.bank2 << 5) as usize % self.num_banks as usize
        } else {
            0
        }
    }

    fn hi_bank(&self) -> usize {
        ((self.bank2 << 5) | self.bank1) as usize % self.num_banks as usize
    }
}

impl Mapper for MBC1 {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3fff => self.rom[self.lo_bank() * 0x4000 + addr as usize],
            0x4000..=0x7fff => self.rom[self.hi_bank() * 0x4000 + addr as usize - 0x4000],
            _ => unreachable!(),
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1fff => {}
            0x2000..=0x3fff => {
                let val = val & 0x1f;
                self.bank1 = if val != 0 {
                    val % self.num_banks as u8
                } else {
                    1
                };
            }
            0x4000..=0x5fff => {
                self.bank2 = val & 0b11;
            }
            0x6000..=0x7fff => self.mode = val != 0,
            0xa000..=0xbfff => {}
            _ => unreachable!(),
        }
    }
}

pub struct MBC1Ram {
    mbc1: MBC1,
    ram: Vec<u8>,
    ram_bank: u8,
    ram_enabled: bool,
}

impl MBC1Ram {
    pub fn new(rom: Vec<u8>, num_banks: u16, ram_size: u32) -> Self {
        Self {
            mbc1: MBC1::new(rom, num_banks),
            ram: vec![0; ram_size as usize],
            ram_bank: 0,
            ram_enabled: false,
        }
    }

    fn ram_bank(&self) -> usize {
        if self.mbc1.mode {
            self.ram_bank as usize
        } else {
            0
        }
    }
}

impl Mapper for MBC1Ram {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7fff => self.mbc1.read(addr),
            0xa000..=0xbfff => {
                if self.ram_enabled {
                    self.ram[self.ram_bank() * 0x2000 + addr as usize - 0xa000]
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
            0x2000..=0x3fff | 0x6000..=0x7fff => self.mbc1.write(addr, val),
            0x4000..=0x5fff => {
                self.mbc1.write(addr, val);
                if self.mbc1.mode {
                    let num_ram_banks = self.ram.len().div_ceil(0x2000) as u8;
                    self.ram_bank = (val & 0b11) % num_ram_banks;
                }
            }
            0xa000..=0xbfff => {
                if self.ram_enabled {
                    let ram_addr = self.ram_bank() * 0x2000 + addr as usize - 0xa000;
                    if ram_addr < self.ram.len() {
                        self.ram[ram_addr] = val;
                    }
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
