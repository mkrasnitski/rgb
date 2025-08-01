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
        }
    }
}

impl Timers {
    // TODO: M-cycle accuracy required for cancelling interrupts via TIMA writes
    pub fn increment(&mut self, apu: &mut Apu) -> bool {
        let old_div = self.div;
        self.div = self.div.wrapping_add(4);
        // Tick Apu FS on falling edge of bit 12
        if old_div.bit(12) && !self.div.bit(12) {
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
        }
        interrupt
    }
}

pub struct MemoryBus {
    bootrom: Option<[u8; 0x100]>,
    pub cartridge: Cartridge,
    ppu: Ppu,
    dma: Dma,
    pub apu: Apu,
    wram: Box<[u8; 0x2000]>,
    hram: Box<[u8; 0x7f]>,
    pub timers: Timers,
    pub joypad: Joypad,
    bootrom_enabled: bool,
    pub int_flag: u8,
    pub int_enable: u8,
}

impl MemoryBus {
    pub fn new(bootrom: Option<[u8; 0x100]>, cartridge: Cartridge, audio_volume: f32) -> Self {
        Self {
            bootrom,
            cartridge,
            apu: Apu::new(audio_volume),
            ppu: Ppu::new(),
            dma: Dma::default(),
            wram: vec![0; 0x2000].try_into().unwrap(),
            hram: vec![0; 0x7f].try_into().unwrap(),
            timers: Timers::default(),
            joypad: Joypad::default(),
            bootrom_enabled: bootrom.is_some(),
            int_flag: 0xE0,
            int_enable: 0,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00ff if self.bootrom_enabled => {
                if let Some(bootrom) = self.bootrom {
                    bootrom[addr as usize]
                } else {
                    self.cartridge.read(addr)
                }
            }
            0x0000..=0x7fff => self.cartridge.read(addr),
            0x8000..=0x9fff => self.ppu.read(addr),
            0xa000..=0xbfff => self.cartridge.read(addr),
            0xc000..=0xdfff => self.wram[addr as usize - 0xc000],
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

            0xff10..=0xff14 | 0xff16..=0xff1e | 0xff20..=0xff26 | 0xff30..=0xff3f => {
                self.apu.read(addr)
            }

            0xff40..=0xff45 | 0xff47..=0xff4b => self.ppu.read(addr),

            0xff46 => self.dma.base,
            0xff50 => 0xff,

            0xff0f => self.int_flag | 0xe0,
            0xffff => self.int_enable,

            // stubs
            0xff01 => 0x00,
            0xff02 => 0x7e,

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

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7fff => self.cartridge.write(addr, val),
            0x8000..=0x9fff => self.ppu.write(addr, val),
            0xa000..=0xbfff => self.cartridge.write(addr, val),
            0xc000..=0xdfff => self.wram[addr as usize - 0xc000] = val,
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
                self.timers.tima = val;
                self.timers.overflow = false;
            }
            0xff06 => self.timers.tma = val,
            0xff07 => self.timers.tac = val | 0xf8,

            0xff0f => self.int_flag = val | 0xE0,
            0xffff => self.int_enable = val,

            0xff10..=0xff14 | 0xff16..=0xff1e | 0xff20..=0xff26 | 0xff30..=0xff3f => {
                self.apu.write(addr, val)
            }

            0xff40..=0xff45 | 0xff47..=0xff4b => self.ppu.write(addr, val),

            0xff46 => {
                self.dma.base = val;
                self.dma.enabled = true;
            }
            0xff50 => {
                if self.bootrom_enabled && val & 1 == 1 {
                    self.bootrom_enabled = false;
                }
            }

            // stubs
            0xff01 | 0xff02 => {}

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

    pub fn tick_dma(&mut self) {
        if let Some((slot, addr)) = self.dma.tick() {
            let val = match addr {
                0x0000..=0xdfff => self.read(addr),
                0xe000..=0xffff => self.wram[addr as usize - 0xe000],
            };
            self.ppu.write_dma(slot, val);
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
