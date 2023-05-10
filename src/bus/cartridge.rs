trait Mapper {
    fn read(&self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
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

struct MBC1 {
    rom: Vec<u8>,
    num_banks: u16,
    bank: u16,
}

impl Mapper for MBC1 {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3fff => self.rom[addr as usize],
            0x4000..=0x7fff => self.rom[self.bank as usize * 0x4000 + addr as usize - 0x4000],
            _ => unreachable!(),
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x2000..=0x3fff => {
                self.bank = if val == 0 {
                    1
                } else {
                    (val as u16 & 0x1f) % self.num_banks
                }
            }
            _ => panic!("MBC1 write: ${addr:04x} = {val:02x}"),
        }
    }
}

pub struct Cartridge {
    mapper: Box<dyn Mapper>,
}

impl Cartridge {
    pub fn new(rom: Vec<u8>) -> Self {
        let mbc = rom[0x147];
        let rom_size = rom[0x148];
        let mapper: Box<dyn Mapper> = match mbc {
            0x00 => Box::new(NoMapper {
                rom: rom.try_into().unwrap(),
            }),
            0x01 => Box::new(MBC1 {
                rom,
                num_banks: match rom_size {
                    0x00..=0x08 => 2 << rom_size,
                    _ => unreachable!(),
                },
                bank: 1,
            }),
            _ => panic!("Invalid mapper value: {mbc:02x}"),
        };
        Self { mapper }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0..=0x7fff => self.mapper.read(addr),
            0x8000..=0xffff => unreachable!(),
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0..=0x7fff => self.mapper.write(addr, val),
            0x8000..=0xffff => unreachable!(),
        }
    }
}
