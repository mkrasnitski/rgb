use std::path::Path;

use crate::utils::BitExtract;
use anyhow::Result;
use image::{ImageBuffer, Rgba};
use pixels::Pixels;

const WHITE: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
const LIGHT_GRAY: [u8; 4] = [0xaa, 0xaa, 0xaa, 0xff];
const DARK_GRAY: [u8; 4] = [0x55, 0x55, 0x55, 0xff];
const BLACK: [u8; 4] = [0x00, 0x00, 0x00, 0xff];

#[allow(non_snake_case)]
pub struct Ppu {
    vram: Box<[u8; 0x4000]>,
    vram_bank: bool,
    oam_ram: Box<[u8; 0xA0]>,
    bgp_ram: Box<[u8; 0x40]>,
    obp_ram: Box<[u8; 0x40]>,
    LCDC: u8,
    STAT: u8,
    SCY: u8,
    SCX: u8,
    LY: u8,
    LYC: u8,
    BGP: u8,
    BGPI: u8,
    OBP0: u8,
    OBP1: u8,
    OBPI: u8,
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
    dmg_compat: bool,
    oam_sort: bool,
}

struct Sprite {
    tile: Tile,
    x: u8,
    y: u8,
    palette: bool,
}

struct Tile {
    tile_num: u8,
    priority: bool,
    x_flip: bool,
    y_flip: bool,
    bank: bool,
    cgb_palette: u8,
}

impl Tile {
    fn new(tile_num: u8, attributes: u8) -> Self {
        Self {
            tile_num,
            priority: attributes.bit(7),
            x_flip: attributes.bit(5),
            y_flip: attributes.bit(6),
            bank: attributes.bit(3),
            cgb_palette: attributes & 0b111,
        }
    }
}

impl Sprite {
    fn from_oam_data(data: [u8; 4]) -> Self {
        Self {
            tile: Tile::new(data[2], data[3]),
            x: data[1],
            y: data[0],
            palette: data[3].bit(4),
        }
    }
}

#[derive(Copy, Clone, Default)]
struct Pixel {
    color_idx: u8,
    palette: Palette,
    priority: bool,
}

#[derive(Copy, Clone)]
enum Palette {
    Monochrome(u8),
    Color { idx: u8, kind: ColorKind },
}

#[derive(Copy, Clone)]
enum ColorKind {
    Background,
    Object,
}

impl Default for Palette {
    fn default() -> Self {
        Palette::Monochrome(0)
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
            vram: vec![0; 0x4000].try_into().unwrap(),
            vram_bank: false,
            oam_ram: vec![0; 0xA0].try_into().unwrap(),
            bgp_ram: vec![0; 0x40].try_into().unwrap(),
            obp_ram: vec![0; 0x40].try_into().unwrap(),
            LCDC: 0,
            STAT: 0x80,
            SCY: 0,
            SCX: 0,
            LY: 0,
            LYC: 0,
            BGP: 0,
            BGPI: 0,
            OBP0: 0,
            OBP1: 0,
            OBPI: 0,
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
            dmg_compat: false,
            oam_sort: false,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0x9fff => match self.mode {
                // FIXME: Enable after implementing variable mode 3 length
                // PpuMode::Drawing => 0xff,
                _ => self.read_vram(addr, self.vram_bank),
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
            0xff4c => 0xfb | ((self.dmg_compat as u8) << 2),
            0xff4f => 0xfe | self.vram_bank as u8,
            0xff68 => self.BGPI,
            0xff69 => self.bgp_ram[self.BGPI as usize & 0x3f],
            0xff6a => self.OBPI,
            0xff6b => self.obp_ram[self.OBPI as usize & 0x3f],
            0xff6c => 0xfe | self.oam_sort as u8,
            _ => panic!("Invalid PPU Register read: {addr:04x}"),
        }
    }

    pub fn read_oam(&self, slot: usize) -> u8 {
        match self.mode {
            PpuMode::OamScan | PpuMode::Drawing => 0xff,
            _ => self.oam_ram[slot],
        }
    }

    fn read_vram(&self, addr: u16, bank: bool) -> u8 {
        let offset = if bank { 0x6000 } else { 0x8000 };
        self.vram[addr as usize - offset]
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x8000..=0x9fff => match self.mode {
                // FIXME: Enable after implementing variable mode 3 length
                // PpuMode::Drawing => {}
                _ => {
                    let offset = if self.vram_bank { 0x6000 } else { 0x8000 };
                    self.vram[addr as usize - offset] = val;
                }
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
            0xff4c => self.dmg_compat = val.bit(2),
            0xff4f => self.vram_bank = val.bit(0),
            0xff68 => self.BGPI = val | 0x40,
            0xff69 => {
                self.bgp_ram[self.BGPI as usize & 0x3f] = val;
                if self.BGPI.bit(7) {
                    self.BGPI = (self.BGPI + 1) & 0xbf;
                }
            }
            0xff6a => self.OBPI = val | 0x40,
            0xff6b => {
                self.obp_ram[self.OBPI as usize & 0x3f] = val;
                if self.OBPI.bit(7) {
                    self.OBPI = (self.OBPI + 1) & 0xbf;
                }
            }
            0xff6c => self.oam_sort = val.bit(0),
            _ => panic!("Invalid PPU Register write: {addr:04x} = {val:#04x}"),
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
                        let idx = if self.oam_sort || self.dmg_compat {
                            self.oam_sprites
                                .binary_search_by(|s| sprite.x.cmp(&s.x))
                                .unwrap_or_else(|e| e)
                        } else {
                            0
                        };
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
                self.pixel_color(self.viewport[idx / 160][idx % 160])
            } else {
                WHITE
            };
            pixel.copy_from_slice(&color);
        }
        self.first_lcd_frame = false;
        pixels.render()?;
        Ok(())
    }

    fn pixel_color(&self, pixel: Pixel) -> [u8; 4] {
        match pixel.palette {
            Palette::Monochrome(idx) => match (idx >> (2 * pixel.color_idx)) & 0b11 {
                0 => WHITE,
                1 => LIGHT_GRAY,
                2 => DARK_GRAY,
                3 => BLACK,
                _ => unreachable!(),
            },
            Palette::Color { idx, kind } => {
                let color_idx = 8 * idx as usize + 2 * pixel.color_idx as usize;
                let palette_ram = match kind {
                    ColorKind::Background => &self.bgp_ram,
                    ColorKind::Object => &self.obp_ram,
                };
                let color =
                    u16::from_le_bytes([palette_ram[color_idx], palette_ram[color_idx + 1]]);
                let blue = (color >> 10) & 0x1f;
                let green = (color >> 5) & 0x1f;
                let red = color & 0x1f;
                [
                    (red << 3 | red >> 2) as u8,
                    (green << 3 | green >> 2) as u8,
                    (blue << 3 | blue >> 2) as u8,
                    0xff,
                ]
            }
        }
    }

    pub fn screenshot(&self, path: impl AsRef<Path>) -> Result<()> {
        ImageBuffer::<Rgba<_>, _>::from_vec(
            160,
            144,
            self.viewport
                .iter()
                .flatten()
                .flat_map(|&p| self.pixel_color(p))
                .collect(),
        )
        .unwrap()
        .save(path)?;
        Ok(())
    }

    fn draw_line(&mut self) {
        if self.LCDC.bit(0) || !self.dmg_compat {
            // Background
            self.draw_tile_line(
                self.LCDC.bit(3),
                self.SCY.wrapping_add(self.LY),
                self.SCX,
                false,
            );
            if self.LCDC.bit(5) && self.LY >= self.WY {
                // Window
                self.draw_tile_line(self.LCDC.bit(6), self.WC, self.WX.wrapping_sub(7), true);
            }
        }
        if self.LCDC.bit(1) {
            self.draw_sprite_line();
        }
    }

    fn draw_tile_line(&mut self, tilemap_bit: bool, y: u8, x_offset: u8, window: bool) {
        let mut visible = false;
        for i in 0..32 {
            let tilemap = if tilemap_bit { 0x9c00 } else { 0x9800 };
            let tile_addr = tilemap + 32 * (y as u16 / 8) + i as u16;
            let tile = Tile::new(
                self.read_vram(tile_addr, false),
                self.read_vram(tile_addr, true),
            );
            let bank = !self.dmg_compat && tile.bank;
            let row = if !self.dmg_compat && tile.y_flip {
                7 - (y % 8)
            } else {
                y % 8
            };
            for (j, &color_idx) in self
                .decode_tile_row(tile.tile_num, bank, row, false)
                .iter()
                .enumerate()
            {
                let col = if !self.dmg_compat && tile.x_flip {
                    8 * i + 7 - j as u8
                } else {
                    8 * i + j as u8
                };
                let x = if window {
                    col.saturating_add(x_offset)
                } else {
                    col.wrapping_sub(x_offset)
                };

                if x < 160 {
                    visible = true;
                    self.viewport[self.LY as usize][x as usize] = Pixel {
                        color_idx,
                        palette: if self.dmg_compat {
                            Palette::Monochrome(self.BGP)
                        } else {
                            Palette::Color {
                                idx: tile.cgb_palette,
                                kind: ColorKind::Background,
                            }
                        },
                        priority: tile.priority,
                    }
                }
            }
        }
        if window && visible {
            self.WC += 1;
        }
    }

    fn draw_sprite_line(&mut self) {
        let obj_size = self.LCDC.bit(2);
        for sprite in &self.oam_sprites {
            let mut row = self.LY + 16 - sprite.y;
            if sprite.tile.y_flip {
                row = if obj_size { 15 - row } else { 7 - row };
            }

            let tile = if obj_size {
                sprite.tile.tile_num & 0xFE
            } else {
                sprite.tile.tile_num
            };

            let bank = !self.dmg_compat && sprite.tile.bank;
            let tile_row = self.decode_tile_row(tile, bank, row, true);
            for (i, &color_idx) in tile_row.iter().enumerate() {
                let col = if sprite.tile.x_flip { 7 - i } else { i };
                let x = (sprite.x + col as u8).wrapping_sub(8);
                if x < 160 {
                    let pixel = &mut self.viewport[self.LY as usize][x as usize];
                    let priority = pixel.priority && !self.dmg_compat;
                    if color_idx != 0
                        && (pixel.color_idx == 0
                            || (!sprite.tile.priority && !priority)
                            || (!self.LCDC.bit(0) && !self.dmg_compat))
                    {
                        let palette = if self.dmg_compat {
                            let dmg_palette = if sprite.palette { self.OBP1 } else { self.OBP0 };
                            Palette::Monochrome(dmg_palette)
                        } else {
                            Palette::Color {
                                idx: sprite.tile.cgb_palette,
                                kind: ColorKind::Object,
                            }
                        };
                        *pixel = Pixel {
                            color_idx,
                            palette,
                            priority: sprite.tile.priority,
                        }
                    }
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

    fn decode_tile_row(&self, tile_num: u8, bank: bool, row_num: u8, is_sprite: bool) -> [u8; 8] {
        let tile_addr = if self.LCDC.bit(4) || is_sprite {
            0x8000 + 16 * tile_num as u16
        } else {
            0x9000u16.wrapping_add_signed(16 * tile_num as i8 as i16)
        };

        let row_addr = tile_addr + 2 * row_num as u16;
        let hi = self.read_vram(row_addr + 1, bank);
        let lo = self.read_vram(row_addr, bank);

        let mut row = [0; 8];
        for col in 0..8 {
            row[7 - col] = (((hi >> col) & 1) << 1) | ((lo >> col) & 1);
        }
        row
    }
}
