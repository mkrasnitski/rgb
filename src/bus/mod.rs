mod cartridge;
pub mod joypad;

use crate::ppu::Ppu;
use crate::utils::BitExtract;
use cartridge::*;
use joypad::Joypad;

pub struct Timers {
    div: u16,
    tima: u8,
    tma: u8,
    tac: u8,

    result: bool,
}

impl Default for Timers {
    fn default() -> Self {
        Self {
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0xf8,

            result: false,
        }
    }
}

impl Timers {
    pub fn increment(&mut self) -> bool {
        self.div = self.div.wrapping_add(4);
        let bit = match self.tac & 0b11 {
            0 => 9,
            1 => 3,
            2 => 5,
            3 => 7,
            _ => unreachable!(),
        };
        let new_result = self.div.bit(bit) && self.tac.bit(2);
        let interrupt = if self.result && !new_result {
            let (tima, c) = self.tima.overflowing_add(1);
            if c {
                self.tima = self.tma;
                true
            } else {
                self.tima = tima;
                false
            }
        } else {
            false
        };

        self.result = new_result;
        interrupt
    }
}

pub struct MemoryBus {
    bootrom: [u8; 0x100],
    pub cartridge: Cartridge,
    ppu: Ppu,
    wram: Box<[u8; 0x2000]>,
    hram: Box<[u8; 0x7f]>,
    pub timers: Timers,
    pub joypad: Joypad,
    dma_base: u8,
    bootrom_enabled: bool,
    pub int_flag: u8,
    pub int_enable: u8,
}

impl MemoryBus {
    pub fn new(bootrom: [u8; 0x100], cartridge: Vec<u8>) -> Self {
        Self {
            bootrom,
            cartridge: Cartridge::new(cartridge),
            ppu: Ppu::new(),
            wram: vec![0; 0x2000].try_into().unwrap(),
            hram: vec![0; 0x7f].try_into().unwrap(),
            timers: Timers::default(),
            joypad: Joypad::default(),
            dma_base: 0,
            bootrom_enabled: true,
            int_flag: 0xE0,
            int_enable: 0,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00ff if self.bootrom_enabled => self.bootrom[addr as usize],
            0x0000..=0x7fff => self.cartridge.read(addr),
            0x8000..=0x9fff => self.ppu.read(addr),
            0xa000..=0xbfff => self.cartridge.read(addr),
            0xc000..=0xdfff => self.wram[addr as usize - 0xc000],
            0xe000..=0xfdff => self.wram[addr as usize - 0xe000],
            0xfe00..=0xfe9f => self.ppu.read(addr),
            0xfea0..=0xfeff => 0x00,
            0xff80..=0xfffe => self.hram[addr as usize - 0xff80],

            0xff00 => self.joypad.read(),
            0xff04 => {
                let [_, msb] = self.timers.div.to_le_bytes();
                msb
            }
            0xff05 => self.timers.tima,
            0xff06 => self.timers.tma,
            0xff07 => self.timers.tac | 0xf8,

            0xff40..=0xff45 | 0xff47..=0xff4b => self.ppu.read(addr),

            0xff46 => self.dma_base,
            0xff50 => 0xff,

            0xff0f => self.int_flag | 0xe0,
            0xffff => self.int_enable,

            // stubs
            0xff01 => 0x00,
            0xff02 => 0x7e,
            0xff10..=0xff14 | 0xff16..=0xff1e | 0xff20..=0xff26 | 0xff30..=0xff3f => 0x00,

            // unused on DMG
            0xff03
            | 0xff08..=0xff0e
            | 0xff15
            | 0xff1f
            | 0xff27..=0xff2f
            | 0xff4c..=0xff4f
            | 0xff51..=0xff7f => 0xff,
        }
    }

    // During DMA, the entire region E000-FFFF is treated as echo ram, instead of E000-FDFF
    fn read_dma(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0xdfff => self.read(addr),
            0xe000..=0xffff => self.wram[addr as usize - 0xe000],
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7fff => self.cartridge.write(addr, val),
            0x8000..=0x9fff => self.ppu.write(addr, val),
            0xa000..=0xbfff => self.cartridge.write(addr, val),
            0xc000..=0xdfff => self.wram[addr as usize - 0xc000] = val,
            0xe000..=0xfdff => self.wram[addr as usize - 0xe000] = val,
            0xfe00..=0xfe9f => self.ppu.write(addr, val),
            0xfea0..=0xfeff => {}
            0xff80..=0xfffe => self.hram[addr as usize - 0xff80] = val,

            0xff00 => self.joypad.write(val),
            0xff04 => self.timers.div = 0,
            0xff05 => self.timers.tima = val,
            0xff06 => self.timers.tma = val,
            0xff07 => self.timers.tac = val | 0xf8,

            0xff0f => self.int_flag = val | 0xE0,
            0xffff => self.int_enable = val,

            0xff40..=0xff45 | 0xff47..=0xff4b => self.ppu.write(addr, val),

            0xff46 => {
                // All at once rather than one byte per cycle (160 total), and no lockout
                self.dma_base = val;
                for i in 0..0xa0 {
                    self.ppu
                        .write_dma(i, self.read_dma(u16::from_be_bytes([self.dma_base, i])))
                }
            }
            0xff50 => {
                if self.bootrom_enabled && val & 1 == 1 {
                    self.bootrom_enabled = false;
                }
            }

            // stubs
            0xff01 | 0xff02 => {}
            0xff10..=0xff14 | 0xff16..=0xff1e | 0xff20..=0xff26 | 0xff30..=0xff3f => {}

            // unused on DMG
            0xff03
            | 0xff08..=0xff0e
            | 0xff15
            | 0xff1f
            | 0xff27..=0xff2f
            | 0xff4c..=0xff4f
            | 0xff51..=0xff7f => {}
        }
    }

    pub fn ppu_mut(&mut self) -> &mut Ppu {
        &mut self.ppu
    }
}
