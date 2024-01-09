mod cartridge;

use crate::ppu::Ppu;
use crate::utils::BitExtract;
use cartridge::*;

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
    cartridge: Cartridge,
    ppu: Ppu,
    sram: Box<[u8; 0x2000]>,
    wram: Box<[u8; 0x2000]>,
    hram: Box<[u8; 0x7f]>,
    pub timers: Timers,
    io_ram: Box<[u8; 0x80]>,
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
            sram: vec![0; 0x2000].try_into().unwrap(),
            wram: vec![0; 0x2000].try_into().unwrap(),
            hram: vec![0; 0x7f].try_into().unwrap(),
            timers: Timers::default(),
            io_ram: vec![0; 0x80].try_into().unwrap(),
            bootrom_enabled: false,
            int_flag: 0xE0,
            int_enable: 0,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00ff if self.bootrom_enabled => self.bootrom[addr as usize],
            0x0000..=0x7fff => self.cartridge.read(addr),
            0x8000..=0x9fff => self.ppu.read(addr),
            0xa000..=0xbfff => self.sram[addr as usize - 0xa000],
            0xc000..=0xdfff => self.wram[addr as usize - 0xc000],
            0xe000..=0xfdff => self.wram[addr as usize - 0xe000],
            0xfe00..=0xfe9f => self.ppu.read(addr),
            0xfea0..=0xfeff => panic!("Illegal address read: {addr:04x}"),
            0xff80..=0xfffe => self.hram[addr as usize - 0xff80],

            0xff04 => {
                let [_, msb] = self.timers.div.to_le_bytes();
                msb
            }
            0xff05 => self.timers.tima,
            0xff06 => self.timers.tma,
            0xff07 => self.timers.tac | 0xf8,

            0xff0f => self.int_flag | 0xe0,
            0xffff => self.int_enable,
            0xff40..=0xff45 | 0xff47..=0xff4b => self.ppu.read(addr),

            0xff00..=0xff7f => self.io_ram[addr as usize - 0xff00],
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7fff => self.cartridge.write(addr, val),
            0x8000..=0x9fff => self.ppu.write(addr, val),
            0xa000..=0xbfff => self.sram[addr as usize - 0xa000] = val,
            0xc000..=0xdfff => self.wram[addr as usize - 0xc000] = val,
            0xe000..=0xfdff => self.wram[addr as usize - 0xe000] = val,
            0xfe00..=0xfe9f => self.ppu.write(addr, val),
            0xfea0..=0xfeff => panic!("Illegal address write: {addr:04x}"),
            0xff80..=0xfffe => self.hram[addr as usize - 0xff80] = val,

            0xff04 => self.timers.div = 0,
            0xff05 => self.timers.tima = val,
            0xff06 => self.timers.tma = val,
            0xff07 => self.timers.tac = val | 0xf8,

            0xff0f => self.int_flag = val | 0xE0,
            0xffff => self.int_enable = val,

            0xff40..=0xff45 | 0xff47..=0xff4b => self.ppu.write(addr, val),

            0xff00..=0xff7f => self.io_ram[addr as usize - 0xff00] = val,
        }
        if addr == 0xff02 && val == 0x81 {
            print!("{}", self.read(0xff01) as char);
            use std::io::Write;
            std::io::stdout().flush().unwrap();
        }
    }

    pub fn ppu_mut(&mut self) -> &mut Ppu {
        &mut self.ppu
    }
}
