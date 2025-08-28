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
    viewport: Box<[[Pixel; 160]; 144]>,
    oam_sprites: Vec<Sprite>,
    cycles: u16,
    ticks: u16,
    pub draw: bool,

    first_lcd_frame: bool,
}

struct Sprite {
    tile: u8,
    x: u8,
    y: u8,
    priority: bool,
    x_flip: bool,
    y_flip: bool,
    palette: bool,
}

impl Sprite {
    fn from_oam_data(data: [u8; 4]) -> Self {
        Self {
            tile: data[2],
            x: data[1],
            y: data[0],
            priority: data[3].bit(7),
            x_flip: data[3].bit(5),
            y_flip: data[3].bit(6),
            palette: data[3].bit(4),
        }
    }
}

#[derive(Copy, Clone, Default)]
struct Pixel {
    color_idx: u8,
    palette: u8,
}

impl Pixel {
    fn color(&self) -> [u8; 4] {
        match (self.palette >> (2 * self.color_idx)) & 0b11 {
            0 => WHITE,
            1 => LIGHT_GRAY,
            2 => DARK_GRAY,
            3 => BLACK,
            _ => unreachable!(),
        }
    }
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
            viewport: Box::new([[Pixel::default(); 160]; 144]),
            oam_sprites: Vec::with_capacity(10),
            cycles: 0,
            ticks: 0,
            draw: false,

            first_lcd_frame: false,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0x9fff => match self.mode {
                // FIXME: Enable after implementing variable mode 3 length
                // PpuMode::Drawing => 0xff,
                _ => self.read_vram(addr),
            },
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

    pub fn read_oam(&self, slot: usize) -> u8 {
        match self.mode {
            PpuMode::OamScan | PpuMode::Drawing => 0xff,
            _ => self.oam_ram[slot],
        }
    }

    fn read_vram(&self, idx: u16) -> u8 {
        self.vram[idx as usize - 0x8000]
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x8000..=0x9fff => match self.mode {
                // FIXME: Enable after implementing variable mode 3 length
                // PpuMode::Drawing => {}
                _ => self.vram[addr as usize - 0x8000] = val,
            },
            0xff40 => {
                if val.bit(7) && !self.LCDC.bit(7) {
                    self.first_lcd_frame = true;
                }
                self.LCDC = val;
            }
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

    pub fn write_oam(&mut self, slot: u8, val: u8) {
        match self.mode {
            PpuMode::OamScan | PpuMode::Drawing => {}
            _ => self.write_dma(slot, val),
        }
    }

    pub fn write_dma(&mut self, oam_slot: u8, val: u8) {
        self.oam_ram[oam_slot as usize] = val;
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
            if scanline == 0 {
                self.WC = 0;
            }
        }

        if scanline < 144 {
            if clocks == 0 {
                self.oam_sprites.clear();
                self.set_mode(PpuMode::OamScan);
            } else if clocks == 20 {
                self.set_mode(PpuMode::Drawing);
                self.draw_line();
            } else {
                // TODO: Variable mode 3 length
                if clocks == 63 {
                    self.set_mode(PpuMode::HBlank);
                }
            }

            // OAM scan
            if clocks < 20 {
                // Fetch two sprites per cycle
                let oam_index = 2 * clocks as usize;
                for i in oam_index..oam_index + 2 {
                    if let Some(sprite) = self.fetch_sprite(i)
                        && self.oam_sprites.len() < 10
                    {
                        let idx = self
                            .oam_sprites
                            .binary_search_by(|s| sprite.x.cmp(&s.x))
                            .unwrap_or_else(|e| e);
                        self.oam_sprites.insert(idx, sprite);
                    }
                }
            }
        } else if scanline == 144 && clocks == 0 {
            self.set_mode(PpuMode::VBlank);
            vblank = true;
        } else if scanline == 153 && clocks == 1 {
            // On the second cycle of line 153, LY is set to 0, weirdly.
            self.LY = 0;
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

    pub fn render(&mut self, pixels: &mut Pixels) -> Result<()> {
        for (idx, pixel) in pixels.frame_mut().chunks_exact_mut(4).enumerate() {
            let color = if self.LCDC.bit(7) && !self.first_lcd_frame {
                self.viewport[idx / 160][idx % 160].color()
            } else {
                WHITE
            };
            pixel.copy_from_slice(&color);
        }
        self.first_lcd_frame = false;
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
        if self.LCDC.bit(1) {
            self.draw_sprite_line();
        }
    }

    fn draw_bg_line(&mut self) {
        let tilemap = self.LCDC.bit(3);
        let y = self.SCY.wrapping_add(self.LY);
        for tile in 0..32 {
            let tile_row = self.get_tile_row(tilemap, y, tile);
            for (i, &color_idx) in tile_row.iter().enumerate() {
                let x = (8 * tile + i as u8).wrapping_sub(self.SCX) as usize;
                if x < 160 {
                    self.viewport[self.LY as usize][x] = Pixel {
                        color_idx,
                        palette: self.BGP,
                    };
                }
            }
        }
    }

    fn draw_win_line(&mut self) {
        let tilemap = self.LCDC.bit(6);
        let mut window_visible = false;
        for tile in 0..32 {
            let tile_row = self.get_tile_row(tilemap, self.WC, tile);
            for (i, &color_idx) in tile_row.iter().enumerate() {
                let x = 8 * tile as usize + i + self.WX as usize - 7;
                if x < 160 {
                    window_visible = true;
                    self.viewport[self.LY as usize][x] = Pixel {
                        color_idx,
                        palette: self.BGP,
                    };
                }
            }
        }
        if window_visible {
            self.WC += 1;
        }
    }

    fn draw_sprite_line(&mut self) {
        let height = if self.LCDC.bit(2) { 16 } else { 8 };
        for sprite in &self.oam_sprites {
            let mut row = self.LY + 16 - sprite.y;
            if sprite.y_flip {
                row = height - row - 1;
            }
            let palette = if sprite.palette { self.OBP1 } else { self.OBP0 };
            let tile = sprite.tile & (0xFF - height / 8 + 1);
            let tile_row = self.decode_tile_row(tile, row, true);

            let scanline = &mut self.viewport[self.LY as usize];
            for i in 0..8 {
                let col = if sprite.x_flip { 7 - i } else { i };
                let color_idx = tile_row[col];
                let x = (sprite.x + i as u8).wrapping_sub(8);
                if x < 160
                    && color_idx != 0
                    && (!sprite.priority || scanline[x as usize].color_idx == 0)
                {
                    scanline[x as usize] = Pixel { color_idx, palette };
                }
            }
        }
    }

    fn fetch_sprite(&self, idx: usize) -> Option<Sprite> {
        let sprite_height = if self.LCDC.bit(2) { 16 } else { 8 };
        let sprite = Sprite::from_oam_data(self.oam_ram[4 * idx..4 * idx + 4].try_into().unwrap());
        let y = self.LY + 16;
        if sprite.x > 0 && (sprite.y..sprite.y + sprite_height).contains(&y) {
            Some(sprite)
        } else {
            None
        }
    }

    fn get_tile_row(&self, tilemap_bit: bool, line: u8, tile: u8) -> [u8; 8] {
        let tilemap = if tilemap_bit { 0x9c00 } else { 0x9800 };
        let tile_num = self.read_vram(tilemap + 32 * (line as u16 / 8) + tile as u16);
        self.decode_tile_row(tile_num, line % 8, false)
    }

    fn decode_tile_row(&self, tile_num: u8, row_num: u8, is_sprite: bool) -> [u8; 8] {
        let tile_addr = if self.LCDC.bit(4) || is_sprite {
            0x8000 + 16 * tile_num as u16
        } else {
            0x9000u16.wrapping_add_signed(16 * tile_num as i8 as i16)
        };

        let row_addr = tile_addr + 2 * row_num as u16;
        let hi = self.read_vram(row_addr + 1);
        let lo = self.read_vram(row_addr);

        let mut row = [0; 8];
        for col in 0..8 {
            row[7 - col] = (((hi >> col) & 1) << 1) | ((lo >> col) & 1);
        }
        row
    }
}
