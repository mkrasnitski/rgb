use anyhow::Result;
use std::path::{Path, PathBuf};

mod mbc1;
mod mbc2;
mod mbc3;
mod mbc5;

use mbc1::{MBC1, MBC1Ram};
use mbc2::MBC2;
use mbc3::{MBC3, MBC3Ram, MBC3RamRtc, MBC3Rtc};
use mbc5::{MBC5, MBC5Ram};

trait Mapper {
    fn read(&self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);

    fn increment_rtc(&mut self) {}

    fn save_external_ram(&self, _: &Path) -> Result<()> {
        Ok(())
    }

    fn load_external_ram(&mut self, _: &Path) -> Result<()> {
        Ok(())
    }
}

struct NoMapper {
    rom: Box<[u8; 0x8000]>,
}

impl Mapper for NoMapper {
    fn read(&self, addr: u16) -> u8 {
        self.rom[addr as usize]
    }

    fn write(&mut self, _addr: u16, _val: u8) {}
}

pub struct Cartridge {
    mapper: Box<dyn Mapper>,
    save_path: PathBuf,
}

impl Cartridge {
    pub fn new(rom_path: PathBuf, saves_dir: PathBuf) -> Result<Self> {
        let rom = std::fs::read(&rom_path)?;
        let rom_name = rom_path.file_stem().unwrap();

        std::fs::create_dir_all(&saves_dir)?;
        let mut save_path = saves_dir;
        save_path.push(rom_name);
        save_path.set_extension("sav");

        let mbc = rom[0x147];
        let rom_type = rom[0x148];
        let ram_type = rom[0x149];

        let num_banks = match rom_type {
            0x00..=0x08 => 2 << rom_type,
            _ => unreachable!(),
        };
        let ram_size_kb = match ram_type {
            0x00 => 0,
            0x01 => 2,
            0x02 => 8,
            0x03 => 32,
            0x04 => 128,
            0x05 => 16,
            _ => unreachable!(),
        };

        let mapper: Box<dyn Mapper> = match mbc {
            0x00 => Box::new(NoMapper {
                rom: rom.try_into().unwrap(),
            }),
            0x01 => Box::new(MBC1::new(rom, num_banks)),
            0x02 | 0x03 => Box::new(MBC1Ram::new(rom, num_banks, 1024 * ram_size_kb)),
            0x05 | 0x06 => Box::new(MBC2::new(rom, num_banks)),
            0x0f => Box::new(MBC3Rtc::new(rom)),
            0x10 => Box::new(MBC3RamRtc::new(rom, 1024 * ram_size_kb)),
            0x11 => Box::new(MBC3::new(rom)),
            0x12 | 0x13 => Box::new(MBC3Ram::new(rom, 1024 * ram_size_kb)),
            0x19 => Box::new(MBC5::new(rom, num_banks)),
            0x1a | 0x1b => Box::new(MBC5Ram::new(rom, num_banks, 1024 * ram_size_kb)),
            _ => panic!("Invalid mapper value: {mbc:02x}"),
        };
        Ok(Self { mapper, save_path })
    }

    pub fn read(&self, addr: u16) -> u8 {
        self.mapper.read(addr)
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        self.mapper.write(addr, val);
    }

    pub fn increment_rtc(&mut self) {
        self.mapper.increment_rtc();
    }

    pub fn save_external_ram(&self) -> Result<()> {
        self.mapper.save_external_ram(&self.save_path)
    }

    pub fn load_external_ram(&mut self) -> Result<()> {
        self.mapper.load_external_ram(&self.save_path)
    }
}
