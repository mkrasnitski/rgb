mod cartridge;
pub mod joypad;

use crate::apu::Apu;
use crate::ppu::Ppu;
use crate::utils::BitExtract;
pub use cartridge::*;
use joypad::Joypad;

pub struct Timers {
    div: u16,
    tima: u8,
    tma: u8,
    tac: u8,

    result: bool,
    overflow: bool,
    reload: bool,
}

impl Default for Timers {
    fn default() -> Self {
        Self {
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0xf8,

            result: false,
            overflow: false,
            reload: false,
        }
    }
}

impl Timers {
    pub fn increment(&mut self, apu: &mut Apu, apu_bit: u8) -> bool {
        let old_div = self.div;
        self.div = self.div.wrapping_add(4);
        // Tick Apu FS on falling edge of bit 12
        if old_div.bit(apu_bit) && !self.div.bit(apu_bit) {
            apu.tick_frame_sequencer();
        }
        let bit = match self.tac & 0b11 {
            0 => 9,
            1 => 3,
            2 => 5,
            3 => 7,
            _ => unreachable!(),
        };
        let new_result = self.div.bit(bit) && self.tac.bit(2);

        self.reload = false;
        let interrupt = self.overflow;
        if self.result && !new_result {
            let (tima, c) = self.tima.overflowing_add(1);
            if c {
                self.overflow = true;
                self.tima = 0;
            } else {
                self.tima = tima;
            }
        }

        self.result = new_result;
        if interrupt {
            self.tima = self.tma;
            self.overflow = false;
            self.reload = true;
        }
        interrupt
    }
}

pub struct MemoryBus {
    bootrom: Option<Vec<u8>>,
    pub cartridge: Cartridge,
    ppu: Ppu,
    dma: Dma,
    vdma: Vdma,
    pub apu: Apu,
    wram: Box<[u8; 0x8000]>,
    wram_bank: u8,
    hram: Box<[u8; 0x7f]>,
    pub timers: Timers,
    pub joypad: Joypad,
    bootrom_enabled: bool,
    pub double_speed: bool,
    pub prepare_speed_switch: bool,
    pub int_flag: u8,
    pub int_enable: u8,
}

impl MemoryBus {
    pub fn new(bootrom: Option<Vec<u8>>, cartridge: Cartridge, apu: Apu) -> Self {
        Self {
            bootrom_enabled: bootrom.is_some(),
            bootrom,
            cartridge,
            apu,
            ppu: Ppu::new(),
            dma: Dma::default(),
            vdma: Vdma::default(),
            wram: vec![0; 0x8000].try_into().unwrap(),
            wram_bank: 0,
            hram: vec![0; 0x7f].try_into().unwrap(),
            timers: Timers::default(),
            joypad: Joypad::default(),
            double_speed: false,
            prepare_speed_switch: false,
            int_flag: 0xE0,
            int_enable: 0,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        // Bootrom is mapped on top of the cartridge, except for 0x100-0x1ff to allow reading the
        // cartridge header.
        if let Some(bootrom) = &self.bootrom
            && self.bootrom_enabled
            && !(0x100..=0x1ff).contains(&addr)
            && (addr as usize) < bootrom.len()
        {
            return bootrom[addr as usize];
        }

        match addr {
            0x0000..=0x7fff => self.cartridge.read(addr),
            0x8000..=0x9fff => self.ppu.read(addr),
            0xa000..=0xbfff => self.cartridge.read(addr),
            0xc000..=0xcfff => self.wram[addr as usize - 0xc000],
            0xd000..=0xdfff => {
                let bank = if self.wram_bank == 0 {
                    1
                } else {
                    self.wram_bank as usize
                };
                self.wram[bank * 0x1000 + addr as usize - 0xd000]
            }
            0xe000..=0xfdff => self.wram[addr as usize - 0xe000],
            0xfe00..=0xfe9f => {
                if self.dma.slot.is_some() {
                    0xff
                } else {
                    self.ppu.read_oam(addr as usize - 0xfe00)
                }
            }
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

            0xff0f => self.int_flag | 0xe0,
            0xffff => self.int_enable,

            0xff10..=0xff14 | 0xff16..=0xff1e | 0xff20..=0xff26 | 0xff30..=0xff3f => {
                self.apu.read(addr)
            }

            0xff40..=0xff45 | 0xff47..=0xff4c | 0xff4f | 0xff68..=0xff6c => self.ppu.read(addr),

            0xff46 => self.dma.base,
            0xff50 => 0xff,

            0xff4d => ((self.double_speed as u8) << 7) | self.prepare_speed_switch as u8 | 0x7e,

            0xff51..=0xff54 => 0xff,
            0xff55 => ((self.vdma.cancelled as u8) << 7) | self.vdma.length.unwrap_or(0xff),

            0xff70 => self.wram_bank | 0xf8,

            // stubs
            0xff01 => 0x00,
            0xff02 => 0x7e,

            // unused
            0xff03
            | 0xff08..=0xff0e
            | 0xff15
            | 0xff1f
            | 0xff27..=0xff2f
            | 0xff4e
            | 0xff56..=0xff67
            | 0xff6d..=0xff6f
            | 0xff71..=0xff7f => 0xff,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7fff => self.cartridge.write(addr, val),
            0x8000..=0x9fff => self.ppu.write(addr, val),
            0xa000..=0xbfff => self.cartridge.write(addr, val),
            0xc000..=0xcfff => self.wram[addr as usize - 0xc000] = val,
            0xd000..=0xdfff => {
                let bank = if self.wram_bank == 0 {
                    1
                } else {
                    self.wram_bank as usize
                };
                self.wram[bank * 0x1000 + addr as usize - 0xd000] = val;
            }
            0xe000..=0xfdff => self.wram[addr as usize - 0xe000] = val,
            0xfe00..=0xfe9f => {
                if self.dma.slot.is_none() {
                    let [_, slot] = addr.to_be_bytes();
                    self.ppu.write_oam(slot, val);
                }
            }
            0xfea0..=0xfeff => {}
            0xff80..=0xfffe => self.hram[addr as usize - 0xff80] = val,

            0xff00 => self.joypad.write(val),
            0xff04 => self.timers.div = 0,
            0xff05 => {
                // TIMA writes are ignored on this M-cycle
                if !self.timers.reload {
                    self.timers.tima = val;
                    self.timers.overflow = false;
                }
            }
            0xff06 => {
                self.timers.tma = val;
                // Hack for TMA write to flow into TIMA when writing on this M-cycle
                if self.timers.reload {
                    self.timers.tima = val;
                }
            }
            0xff07 => self.timers.tac = val | 0xf8,

            0xff0f => self.int_flag = val | 0xE0,
            0xffff => self.int_enable = val,

            0xff10..=0xff14 | 0xff16..=0xff1e | 0xff20..=0xff26 | 0xff30..=0xff3f => {
                self.apu.write(addr, val)
            }

            0xff40..=0xff45 | 0xff47..=0xff4b | 0xff4f | 0xff68..=0xff6c => {
                self.ppu.write(addr, val)
            }
            0xff4c => {
                if self.bootrom_enabled {
                    self.ppu.write(addr, val)
                }
            }

            0xff46 => {
                self.dma.base = val;
                self.dma.enabled = true;
            }
            0xff50 => {
                if self.bootrom_enabled && val & 1 == 1 {
                    self.bootrom_enabled = false;
                }
            }

            0xff4d => self.prepare_speed_switch = val.bit(0),

            0xff51 => self.vdma.src = (self.vdma.src & 0x00ff) | ((val as u16) << 8),
            0xff52 => self.vdma.src = (self.vdma.src & 0xff00) | (val & 0xf0) as u16,
            0xff53 => self.vdma.dest = (self.vdma.dest & 0x00ff) | ((val as u16) << 8),
            0xff54 => self.vdma.dest = (self.vdma.dest & 0xff00) | (val & 0xf0) as u16,
            0xff55 => {
                let length = val & 0x7f;
                if val.bit(7) {
                    self.vdma.start(length, true);
                } else {
                    self.vdma.start(length, false);
                    while self.vdma.remaining_length().is_some() {
                        self.tick_vdma(false)
                    }
                }
            }

            0xff70 => self.wram_bank = val & 0b111,

            // stubs
            0xff01 | 0xff02 => {}

            // unused
            0xff03
            | 0xff08..=0xff0e
            | 0xff15
            | 0xff1f
            | 0xff27..=0xff2f
            | 0xff4e
            | 0xff56..=0xff67
            | 0xff6d..=0xff6f
            | 0xff71..=0xff7f => {}
        }
    }

    pub fn tick_dma(&mut self) {
        if let Some((slot, addr)) = self.dma.tick() {
            let val = match addr {
                0x0000..=0xdfff => self.read(addr),
                0xe000..=0xffff => self.wram[addr as usize - 0xe000],
            };
            self.ppu.write_dma(slot, val);
        }
    }

    pub fn tick_vdma(&mut self, hblank: bool) {
        if let Some(length) = self.vdma.remaining_length()
            && (!hblank || self.vdma.hblank)
        {
            for _ in 0..16 {
                let val = self.read_vdma(self.vdma.src);
                self.write(0x8000 + (self.vdma.dest & 0x1fff), val);
                self.vdma.src = self.vdma.src.wrapping_add(1);
                let (dest, overflow) = self.vdma.dest.overflowing_add(1);
                if overflow {
                    self.vdma.cancelled = true;
                }
                self.vdma.dest = dest;
            }
            self.vdma.length = length.checked_sub(1);
        }
    }

    fn read_vdma(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7fff | 0xa000..=0xdfff => self.read(addr),
            _ => 0xff,
        }
    }

    pub fn ppu_mut(&mut self) -> &mut Ppu {
        &mut self.ppu
    }
}

#[derive(Default)]
struct Dma {
    base: u8,
    enabled: bool,
    slot: Option<u8>,
}

impl Dma {
    fn tick(&mut self) -> Option<(u8, u16)> {
        if self.enabled {
            match self.slot {
                Some(slot) => {
                    let addr = u16::from_be_bytes([self.base, slot]);
                    if slot == 0x9f {
                        self.enabled = false;
                        self.slot = None;
                    } else {
                        self.slot = Some(slot + 1);
                    }
                    return Some((slot, addr));
                }
                None => self.slot = Some(0),
            }
        }
        None
    }
}

#[derive(Default)]
struct Vdma {
    src: u16,
    dest: u16,
    length: Option<u8>,
    hblank: bool,
    cancelled: bool,
}

impl Vdma {
    fn start(&mut self, length: u8, hblank: bool) {
        self.length = Some(length);
        self.hblank = hblank;
        self.cancelled = false;
    }

    fn remaining_length(&self) -> Option<u8> {
        if self.cancelled { None } else { self.length }
    }
}
