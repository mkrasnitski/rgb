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
    WC: u8,

    mode: PpuMode,
    stat_condition: bool,
    viewport: Box<[[u8; 160]; 144]>,
    cycles: u16,
    ticks: u16,
    pub draw: bool,
}

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
enum PpuMode {
    OamScan = 2,
    Drawing = 3,
    HBlank = 0,
    VBlank = 1,
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
            WC: 0,

            mode: PpuMode::HBlank,
            stat_condition: false,
            viewport: Box::new([[0; 160]; 144]),
            cycles: 0,
            ticks: 0,
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
            0xff41 => {
                self.STAT &= 0b10000111; // Clear writeable bits
                self.STAT |= val & 0b01111000; // Set those bits
            }
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

    pub fn step(&mut self) -> (bool, bool) {
        if self.ticks == 17556 {
            self.ticks = 0;
            self.draw = true;
        }

        if self.LCDC.bit(7) {
            let (vblank, stat) = self.cycle();
            self.ticks = self.cycles + 1;
            self.cycles = (self.cycles + 1) % 17556;
            (vblank, stat)
        } else {
            // Hold everything to 0 while PPU is disabled
            self.cycles = 0;
            self.LY = 0;
            self.set_mode(PpuMode::HBlank);
            self.ticks += 1;
            (false, false)
        }
    }

    fn cycle(&mut self) -> (bool, bool) {
        let mut vblank = false;

        let clocks = self.cycles % 114;
        let scanline = self.cycles / 114;

        if clocks == 0 {
            self.LY = scanline as u8;
            if self.LY == 0 {
                self.WC = 0;
            }
        }

        if self.LY < 144 {
            if clocks == 0 {
                self.set_mode(PpuMode::OamScan);
            } else if clocks == 20 {
                self.set_mode(PpuMode::Drawing);
                self.draw_line();
            } else if clocks == 43 {
                self.set_mode(PpuMode::HBlank);
            }
        } else if self.LY == 144 && clocks == 0 {
            self.set_mode(PpuMode::VBlank);
            vblank = true;
        }

        let ly_coincidence = self.check_lyc();
        let stat = self.check_stat(ly_coincidence);

        (vblank, stat)
    }

    fn set_mode(&mut self, mode: PpuMode) {
        self.mode = mode;

        self.STAT &= 0b11111100;
        self.STAT |= (mode as u8) & 0b11;
    }

    fn check_lyc(&mut self) -> bool {
        let c = self.LY == self.LYC;
        self.STAT &= 0b11111011;
        self.STAT |= (c as u8) << 2;
        c
    }

    fn check_stat(&mut self, ly_coincidence: bool) -> bool {
        let old = self.stat_condition;
        let mut new = self.STAT.bit(6) && ly_coincidence;
        for mode in 0..=2 {
            if self.STAT.bit(mode + 3) {
                new |= (self.mode as u8) == mode;
            }
        }
        self.stat_condition = new;
        new && !old
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
        for (idx, pixel) in pixels.frame_mut().chunks_exact_mut(4).enumerate() {
            let color = if self.LCDC.bit(7) {
                let i = idx / 160;
                let j = idx % 160;
                match (self.BGP >> (2 * self.viewport[i][j])) & 0b11 {
                    0 => WHITE,
                    1 => LIGHT_GRAY,
                    2 => DARK_GRAY,
                    3 => BLACK,
                    _ => unreachable!(),
                }
            } else {
                WHITE
            };
            pixel.copy_from_slice(&color);
        }
        pixels.render()?;
        Ok(())
    }

    fn draw_line(&mut self) {
        if self.LCDC.bit(0) {
            self.draw_bg_line();
            if self.LCDC.bit(5) && self.LY >= self.WY {
                self.draw_win_line();
            }
        }
    }

    fn draw_bg_line(&mut self) {
        let tilemap = self.LCDC.bit(3);
        let y = self.SCY.wrapping_add(self.LY);
        for tile in 0..32 {
            let tile_row = self.get_tile_row(tilemap, y, tile);
            for col in 0..8 {
                let x = (8 * tile + col as u8).wrapping_sub(self.SCX) as usize;
                if x < 160 {
                    self.viewport[self.LY as usize][x] = tile_row[col];
                }
            }
        }
    }

    fn draw_win_line(&mut self) {
        let tilemap = self.LCDC.bit(6);
        let mut window_visible = false;
        for tile in 0..32 {
            let tile_row = self.get_tile_row(tilemap, self.WC, tile);
            for col in 0..8 {
                let x = 8 * tile as usize + col + self.WX as usize - 7;
                if x < 160 {
                    window_visible = true;
                    self.viewport[self.LY as usize][x] = tile_row[col];
                }
            }
        }
        if window_visible {
            self.WC += 1;
        }
    }

    fn get_tile_row(&self, tilemap_bit: bool, line: u8, tile: u8) -> [u8; 8] {
        let tilemap = if tilemap_bit { 0x9c00 } else { 0x9800 };
        let tile_num = self.read(tilemap + 32 * (line as u16 / 8) + tile as u16);
        self.decode_tile_row(tile_num, line % 8)
    }

    fn decode_tile_row(&self, tile_num: u8, row_num: u8) -> [u8; 8] {
        let tile_addr = match self.LCDC.bit(4) {
            true => 0x8000 + 16 * tile_num as u16,
            false => 0x9000u16.wrapping_add_signed(16 * tile_num as i8 as i16),
        };
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
