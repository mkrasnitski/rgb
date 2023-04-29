pub struct MemoryBus {
    bootrom: [u8; 0x100],
    cartridge: Box<[u8; 0x8000]>,
    vram: Box<[u8; 0x2000]>,
    sram: Box<[u8; 0x2000]>,
    wram: Box<[u8; 0x4000]>,
    bootrom_enabled: bool,
}

impl MemoryBus {
    pub fn new(bootrom: [u8; 0x100], cartridge: Box<[u8; 0x8000]>) -> Self {
        Self {
            bootrom,
            cartridge,
            vram: vec![0; 0x2000].try_into().unwrap(),
            sram: vec![0; 0x2000].try_into().unwrap(),
            wram: vec![0; 0x4000].try_into().unwrap(),
            bootrom_enabled: false,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00ff if self.bootrom_enabled => self.bootrom[addr as usize],
            0x0000..=0x7fff => self.cartridge[addr as usize],
            0x8000..=0x9fff => self.vram[addr as usize - 0x8000],
            0xa000..=0xbfff => self.sram[addr as usize - 0xa000],
            0xc000..=0xffff => self.wram[addr as usize - 0xc000],
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7fff => self.cartridge[addr as usize] = val,
            0x8000..=0x9fff => self.vram[addr as usize - 0x8000] = val,
            0xa000..=0xbfff => self.sram[addr as usize - 0xa000] = val,
            0xc000..=0xffff => self.wram[addr as usize - 0xc000] = val,
        }
        if addr == 0xff02 && val == 0x81 {
            print!("{}", self.read(0xff01) as char);
        }
    }
}
