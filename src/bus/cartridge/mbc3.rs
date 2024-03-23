use super::Mapper;

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
        if ram_size != 0x8000 {
            panic!("MBC3 ram size not 0x8000");
        }
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
                    self.ram[self.ram_bank as usize * 0x2000 + addr as usize - 0xa000] = val
                }
            }
            _ => unreachable!(),
        }
    }
}
