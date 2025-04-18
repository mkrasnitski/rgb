use anyhow::Result;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use super::Mapper;
use crate::utils::BitExtract;

pub struct MBC3 {
    rom: Vec<u8>,
    bank: u8,
}

impl MBC3 {
    pub fn new(rom: Vec<u8>) -> Self {
        Self { rom, bank: 1 }
    }
}

impl Mapper for MBC3 {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3fff => self.rom[addr as usize],
            0x4000..=0x7fff => self.rom[self.bank as usize * 0x4000 + addr as usize - 0x4000],
            _ => unreachable!(),
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x2000..=0x3fff => self.bank = if val == 0 { 1 } else { val },
            _ => panic!("MBC3 Write: ${addr:04x} = {val:02x}"),
        }
    }
}

pub struct MBC3Ram {
    mbc3: MBC3,
    ram: Vec<u8>,
    ram_bank: u8,
    ram_enabled: bool,
}

impl MBC3Ram {
    pub fn new(rom: Vec<u8>, ram_size: u32) -> Self {
        Self {
            mbc3: MBC3::new(rom),
            ram: vec![0; ram_size as usize],
            ram_bank: 0,
            ram_enabled: false,
        }
    }
}

impl Mapper for MBC3Ram {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7fff => self.mbc3.read(addr),
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
            0x2000..=0x3fff => self.mbc3.write(addr, val),
            0x4000..=0x5fff => {
                if val <= 0x03 {
                    self.ram_bank = val & 0b11;
                } else {
                    panic!("Invalid MBC3 ram bank: {val:02x}")
                }
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

pub struct MBC3Rtc {
    mbc3: MBC3,
    rtc: Rtc,
    rtc_register: u8,
    ram_enabled: bool,
}

impl MBC3Rtc {
    pub fn new(rom: Vec<u8>) -> Self {
        Self {
            mbc3: MBC3::new(rom),
            rtc: Rtc::default(),
            rtc_register: 0,
            ram_enabled: false,
        }
    }
}

impl Mapper for MBC3Rtc {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7fff => self.mbc3.read(addr),
            0xa000..=0xbfff => {
                if self.ram_enabled && (0x08..=0x0c).contains(&self.rtc_register) {
                    self.rtc.read(self.rtc_register)
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
            0x2000..=0x3fff => self.mbc3.write(addr, val),
            0x4000..=0x5fff => {
                self.rtc_register = val;
            }
            0x6000..=0x7fff => {
                if val == 0 && !self.rtc.prepare_latch {
                    self.rtc.prepare_latch = true;
                } else if val == 1 && self.rtc.prepare_latch {
                    self.rtc.prepare_latch = false;
                    self.rtc.latched_state = self.rtc.internal_state.clone();
                }
            }
            0xa000..=0xbfff => {
                if self.ram_enabled && (0x08..=0x0c).contains(&self.rtc_register) {
                    self.rtc.write(self.rtc_register, val);
                }
            }
            _ => unreachable!(),
        }
    }

    fn increment_rtc(&mut self) {
        self.rtc.increment();
    }

    fn save_external_ram(&self, filename: &Path) -> Result<()> {
        let mut file = File::create(filename)?;
        file.write_all(&self.rtc.internal_state.as_bytes())?;
        file.write_all(&self.rtc.latched_state.as_bytes())?;
        Ok(())
    }

    fn load_external_ram(&mut self, filename: &Path) -> Result<()> {
        if let Ok(mut file) = File::open(filename) {
            let mut bytes = [0; 5];
            file.read_exact(&mut bytes)?;
            self.rtc.internal_state = RtcState::from_bytes(bytes);
            file.read_exact(&mut bytes)?;
            self.rtc.latched_state = RtcState::from_bytes(bytes);
        }
        Ok(())
    }
}

pub struct MBC3RamRtc {
    mbc3: MBC3,
    ram: Vec<u8>,
    rtc: Rtc,
    register: u8,
    ram_enabled: bool,
}

impl MBC3RamRtc {
    pub fn new(rom: Vec<u8>, ram_size: u32) -> Self {
        Self {
            mbc3: MBC3::new(rom),
            ram: vec![0; ram_size as usize],
            rtc: Rtc::default(),
            register: 0,
            ram_enabled: false,
        }
    }
}

impl Mapper for MBC3RamRtc {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7fff => self.mbc3.read(addr),
            0xa000..=0xbfff => {
                if self.ram_enabled {
                    let reg = self.register & 0xf;
                    match reg {
                        0x00..=0x03 => self.ram[reg as usize * 0x2000 + addr as usize - 0xa000],
                        0x08..=0x0c => self.rtc.read(reg),
                        _ => 0xFF,
                    }
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
            0x2000..=0x3fff => self.mbc3.write(addr, val),
            0x4000..=0x5fff => {
                self.register = val;
            }
            0x6000..=0x7fff => {
                if val == 0 && !self.rtc.prepare_latch {
                    self.rtc.prepare_latch = true;
                } else if val == 1 && self.rtc.prepare_latch {
                    self.rtc.prepare_latch = false;
                    self.rtc.latched_state = self.rtc.internal_state.clone();
                }
            }
            0xa000..=0xbfff => {
                if self.ram_enabled {
                    let reg = self.register & 0xf;
                    match reg {
                        0x00..=0x03 => {
                            self.ram[reg as usize * 0x2000 + addr as usize - 0xa000] = val;
                        }
                        0x08..=0x0c => self.rtc.write(reg, val),
                        _ => {}
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    fn increment_rtc(&mut self) {
        self.rtc.increment();
    }

    fn save_external_ram(&self, filename: &Path) -> Result<()> {
        let mut file = File::create(filename)?;
        file.write_all(&self.ram)?;
        file.write_all(&self.rtc.internal_state.as_bytes())?;
        file.write_all(&self.rtc.latched_state.as_bytes())?;
        Ok(())
    }

    fn load_external_ram(&mut self, filename: &Path) -> Result<()> {
        if let Ok(mut file) = File::open(filename) {
            file.read_exact(&mut self.ram)?;

            let mut bytes = [0; 5];
            file.read_exact(&mut bytes)?;
            self.rtc.internal_state = RtcState::from_bytes(bytes);
            file.read_exact(&mut bytes)?;
            self.rtc.latched_state = RtcState::from_bytes(bytes);
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
struct RtcState {
    seconds: u8,
    minutes: u8,
    hours: u8,
    days: u16,
}

impl RtcState {
    fn from_bytes(bytes: [u8; 5]) -> Self {
        let [seconds, minutes, hours, days_hi, days_lo] = bytes;
        Self {
            seconds,
            minutes,
            hours,
            days: u16::from_be_bytes([days_hi, days_lo]),
        }
    }

    fn as_bytes(&self) -> [u8; 5] {
        let [days_hi, days_lo] = self.days.to_be_bytes();
        [self.seconds, self.minutes, self.hours, days_lo, days_hi]
    }
}

#[derive(Default)]
pub struct Rtc {
    prepare_latch: bool,
    carry: bool,
    halted: bool,
    cycles: u8,
    ticks: u16,
    internal_state: RtcState,
    latched_state: RtcState,
}

impl Rtc {
    pub fn read(&self, register: u8) -> u8 {
        match register {
            0x08 => self.latched_state.seconds,
            0x09 => self.latched_state.minutes,
            0x0A => self.latched_state.hours,
            0x0B => self.latched_state.days as u8,
            0x0C => {
                ((self.carry as u8) << 7)
                    | ((self.halted as u8) << 6)
                    | ((self.latched_state.days >> 8) as u8)
            }
            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, register: u8, val: u8) {
        match register {
            0x08 => {
                self.cycles = 0;
                self.ticks = 0;
                self.latched_state.seconds = val & 0x3F;
            }
            0x09 => self.latched_state.minutes = val & 0x3F,
            0x0A => self.latched_state.hours = val & 0x1F,
            0x0B => self.latched_state.days = (self.latched_state.days & 0b100000000) | val as u16,
            0x0C => {
                self.latched_state.days =
                    (self.latched_state.days & 0xff) | ((val as u16 & 1) << 8);
                self.halted = val.bit(6);
                self.carry = val.bit(7);
            }
            _ => unreachable!(),
        }
        self.internal_state = self.latched_state.clone();
    }

    pub fn increment(&mut self) {
        if !self.halted {
            self.cycles += 4;
            if self.cycles == 128 {
                self.cycles = 0;
                self.ticks += 1;
            }

            if self.ticks == 32768 {
                self.ticks = 0;
                self.internal_state.seconds = (self.internal_state.seconds + 1) & 0x3F;
                if self.internal_state.seconds == 60 {
                    self.internal_state.seconds = 0;
                    self.internal_state.minutes = (self.internal_state.minutes + 1) & 0x3F;
                    if self.internal_state.minutes == 60 {
                        self.internal_state.minutes = 0;
                        self.internal_state.hours = (self.internal_state.hours + 1) & 0x1F;
                        if self.internal_state.hours == 24 {
                            self.internal_state.hours = 0;
                            self.internal_state.days += 1;
                            if self.internal_state.days == 512 {
                                self.internal_state.days = 0;
                                self.carry = true;
                            }
                        }
                    }
                }
            }
        }
    }
}
