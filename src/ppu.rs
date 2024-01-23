use crate::utils::BitExtract;
use anyhow::Result;
use pixels::Pixels;

const WHITE: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
const LIGHT_GRAY: [u8; 4] = [0xaa, 0xaa, 0xaa, 0xff];
const DARK_GRAY: [u8; 4] = [0x55, 0x55, 0x55, 0xff];
const BLACK: [u8; 4] = [0x00, 0x00, 0x00, 0xff];

#[allow(non_snake_case)]
pub struct Ppu {
    vram: Box<[u8; 0x2000]>,
    oam_ram: Box<[u8; 0xA0]>,
    LCDC: u8,
    STAT: u8,
    SCY: u8,
    SCX: u8,
    LY: u8,
    LYC: u8,
    BGP: u8,
    OBP0: u8,
    OBP1: u8,
    WY: u8,
    WX: u8,

    cycles: u64,
    pub draw: bool,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            vram: vec![0; 0x2000].try_into().unwrap(),
            oam_ram: vec![0; 0xA0].try_into().unwrap(),
            LCDC: 0,
            STAT: 0x80,
            SCY: 0,
            SCX: 0,
            LY: 0,
            LYC: 0,
            BGP: 0,
            OBP0: 0,
            OBP1: 0,
            WY: 0,
            WX: 0,
            cycles: 0,
            draw: false,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0x9fff => self.vram[addr as usize - 0x8000],
            0xfe00..=0xfe9f => self.oam_ram[addr as usize - 0xfe00],
            0xff40 => self.LCDC,
            0xff41 => self.STAT,
            0xff42 => self.SCY,
            0xff43 => self.SCX,
            0xff44 => self.LY,
            0xff45 => self.LYC,
            0xff47 => self.BGP,
            0xff48 => self.OBP0,
            0xff49 => self.OBP1,
            0xff4a => self.WY,
            0xff4b => self.WX,
            _ => panic!("Invalid PPU Register read: {addr:04x}"),
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x8000..=0x9fff => self.vram[addr as usize - 0x8000] = val,
            0xfe00..=0xfe9f => self.oam_ram[addr as usize - 0xfe00] = val,
            0xff40 => self.LCDC = val,
            0xff41 => self.STAT = val,
            0xff42 => self.SCY = val,
            0xff43 => self.SCX = val,
            0xff44 => self.LY = val,
            0xff45 => self.LYC = val,
            0xff47 => self.BGP = val,
            0xff48 => self.OBP0 = val,
            0xff49 => self.OBP1 = val,
            0xff4a => self.WY = val,
            0xff4b => self.WX = val,
            _ => panic!("Invalid PPU Register write: {addr:04x} = {val:#02x}"),
        }
    }

    pub fn step(&mut self) -> bool {
        if self.cycles == 17556 {
            self.cycles = 0;
            self.draw = true;
        }
        let vblank = self.cycle();
        self.cycles += 1;
        vblank
    }

    fn cycle(&mut self) -> bool {
        if self.cycles % 114 == 0 {
            self.LY = (self.cycles / 114) as u8;
            if self.LY == 144 {
                return true;
            }
        }
        false
    }

    pub fn draw_check(&mut self) -> bool {
        if self.draw {
            self.draw = false;
            true
        } else {
            false
        }
    }

    pub fn render(&self, pixels: &mut Pixels) -> Result<()> {
        let mut frame = [[0; 160]; 144];
        let bg_tilemap = match self.LCDC.bit(3) {
            true => 0x9c00,
            false => 0x9800,
        };
        for row in 0..144 {
            let y = self.SCY.wrapping_add(row as u8);
            for tile in 0..32 {
                let tile_num = self.read(bg_tilemap + 32 * (y as u16 / 8) + tile as u16);
                let tile_row = self.decode_tile_row(tile_num, y % 8);
                for col in 0..8 {
                    let x = (8 * tile + col as u8).wrapping_sub(self.SCX) as usize;
                    if x < 160 {
                        frame[row][x] = tile_row[col];
                    }
                }
            }
        }
        for (idx, pixel) in pixels.frame_mut().chunks_exact_mut(4).enumerate() {
            let i = idx / 160;
            let j = idx % 160;
            let color = match (self.BGP >> (2 * frame[i][j])) & 0b11 {
                0 => WHITE,
                1 => LIGHT_GRAY,
                2 => DARK_GRAY,
                3 => BLACK,
                _ => unreachable!(),
            };
            pixel.copy_from_slice(&color);
        }
        pixels.render()?;
        Ok(())
    }

    fn decode_tile_row(&self, tile_num: u8, row_num: u8) -> [u8; 8] {
        let tile_addr = 0x8000 + 16 * tile_num as u16;
        let row_addr = tile_addr + 2 * row_num as u16;

        let hi = self.read(row_addr + 1);
        let lo = self.read(row_addr);

        let mut row = [0; 8];
        for col in 0..8 {
            row[7 - col] = (((hi >> col) & 1) << 1) | ((lo >> col) & 1);
        }
        row
    }
}
