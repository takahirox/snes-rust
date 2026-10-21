//! Scanline renderer for planar backgrounds and objects.
pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 224;
pub struct Ppu {
    pub vram: Vec<u8>,
    pub cgram: [u8; 512],
    pub oam: [u8; 544],
    pub regs: [u8; 64],
    pub frame: Vec<u8>,
    vaddr: u16,
    vbuffer: u16,
    caddr: usize,
    oaddr: usize,
    olatch: u8,
    scroll_latch: u8,
    scroll: [[u16; 2]; 4],
    m7: [i16; 6],
    m7_latch: u8,
    fixed: [u8; 3],
    pub h: u16,
    pub v: u16,
    hflip: bool,
    vflip: bool,
}
impl Default for Ppu {
    fn default() -> Self {
        Self {
            vram: vec![0; 65536],
            cgram: [0; 512],
            oam: [0; 544],
            regs: [0; 64],
            frame: vec![0; WIDTH * HEIGHT * 4],
            vaddr: 0,
            vbuffer: 0,
            caddr: 0,
            oaddr: 0,
            olatch: 0,
            scroll_latch: 0,
            scroll: [[0; 2]; 4],
            m7: [0; 6],
            m7_latch: 0,
            fixed: [0; 3],
            h: 0,
            v: 0,
            hflip: false,
            vflip: false,
        }
    }
}
#[derive(Clone, Copy)]
struct Pixel {
    color: u16,
    rank: u8,
    layer: u8,
    math: bool,
}
impl Ppu {
    fn va(&self) -> usize {
        let a = self.vaddr as usize;
        let a = match (self.regs[0x15] >> 2) & 3 {
            0 => a,
            1 => (a & 0xff00) | ((a & 31) << 3) | ((a >> 5) & 7),
            2 => (a & 0xfe00) | ((a & 63) << 3) | ((a >> 6) & 7),
            _ => (a & 0xfc00) | ((a & 127) << 3) | ((a >> 7) & 7),
        };
        (a * 2) & 65535
    }
    fn vinc(&mut self, high: bool) {
        if (self.regs[0x15] & 128 != 0) == high {
            self.vaddr = self.vaddr.wrapping_add(match self.regs[0x15] & 3 {
                0 => 1,
                1 => 32,
                _ => 128,
            });
        }
    }
    fn vw(&self, a: usize) -> u16 {
        u16::from_le_bytes([self.vram[a & 65535], self.vram[(a + 1) & 65535]])
    }
    fn palette(&self, i: usize) -> u16 {
        u16::from_le_bytes([self.cgram[(i * 2) & 511], self.cgram[(i * 2 + 1) & 511]]) & 0x7fff
    }
    pub fn write(&mut self, r: u8, v: u8) {
        let r = (r & 63) as usize;
        self.regs[r] = v;
        match r {
            2 | 3 => {
                self.oaddr =
                    ((((self.regs[3] as usize & 1) << 8) | self.regs[2] as usize) * 2) & 1023
            }
            4 => {
                let a = if self.oaddr & 512 != 0 {
                    512 | (self.oaddr & 31)
                } else {
                    self.oaddr & 511
                };
                if a >= 512 {
                    self.oam[a] = v;
                } else if a & 1 == 0 {
                    self.olatch = v;
                } else {
                    self.oam[a - 1] = self.olatch;
                    self.oam[a] = v;
                }
                self.oaddr = (self.oaddr + 1) & 1023;
            }
            0x0d..=0x14 => {
                let bg = (r - 0x0d) / 2;
                let axis = (r - 0x0d) & 1;
                let old = self.scroll[bg][axis];
                self.scroll[bg][axis] = if axis == 0 {
                    ((v as u16) << 8) | ((self.scroll_latch as u16) & !7) | ((old >> 8) & 7)
                } else {
                    ((v as u16) << 8) | self.scroll_latch as u16
                };
                self.scroll_latch = v;
            }
            0x16 | 0x17 => {
                self.vaddr = u16::from_le_bytes([self.regs[0x16], self.regs[0x17]]);
                self.vbuffer = self.vw(self.va());
            }
            0x18 | 0x19 => {
                let high = r == 0x19;
                let a = self.va() + usize::from(high);
                self.vram[a & 65535] = v;
                self.vinc(high);
            }
            0x1b..=0x20 => {
                self.m7[r - 0x1b] = i16::from_le_bytes([self.m7_latch, v]);
                self.m7_latch = v;
            }
            0x21 => self.caddr = v as usize * 2,
            0x22 => {
                self.cgram[self.caddr] = if self.caddr & 1 != 0 { v & 127 } else { v };
                self.caddr = (self.caddr + 1) & 511;
            }
            0x32 => {
                for i in 0..3 {
                    if v & (32 << i) != 0 {
                        self.fixed[i] = v & 31;
                    }
                }
            }
            _ => {}
        }
    }
    pub fn read(&mut self, r: u8, open: u8) -> u8 {
        match r & 63 {
            0x34..=0x36 => {
                let product = self.m7[0] as i32 * (self.m7[1] >> 8) as i32;
                (product >> ((r - 0x34) * 8)) as u8
            }
            0x37 => {
                self.hflip = false;
                self.vflip = false;
                open
            }
            0x38 => {
                let a = if self.oaddr & 512 != 0 {
                    512 | (self.oaddr & 31)
                } else {
                    self.oaddr & 511
                };
                let v = self.oam[a];
                self.oaddr = (self.oaddr + 1) & 1023;
                v
            }
            0x39 | 0x3a => {
                let high = r & 63 == 0x3a;
                let v = (self.vbuffer >> if high { 8 } else { 0 }) as u8;
                if (self.regs[0x15] & 128 != 0) == high {
                    self.vbuffer = self.vw(self.va());
                    self.vinc(high);
                }
                v
            }
            0x3b => {
                let v = self.cgram[self.caddr];
                self.caddr = (self.caddr + 1) & 511;
                v
            }
            0x3c => {
                let v = ((self.h / 4) >> if self.hflip { 8 } else { 0 }) as u8;
                self.hflip = !self.hflip;
                v
            }
            0x3d => {
                let v = (self.v >> if self.vflip { 8 } else { 0 }) as u8;
                self.vflip = !self.vflip;
                v
            }
            0x3e => 1,
            0x3f => {
                self.hflip = false;
                self.vflip = false;
                3
            }
            _ => open,
        }
    }
    fn tile_pixel(&self, base: usize, tile: usize, x: usize, y: usize, bpp: usize) -> u8 {
        let address = base + tile * bpp * 8 + y * 2;
        let mut color = 0;
        for plane in 0..bpp {
            let a = address + (plane / 2) * 16 + (plane & 1);
            color |= ((self.vram[a & 65535] >> (7 - x)) & 1) << plane;
        }
        color
    }
    fn bg_pixel(&self, bg: usize, x: usize, y: usize) -> Option<Pixel> {
        let mode = self.regs[5] & 7;
        let depths = [
            [2, 2, 2, 2],
            [4, 4, 2, 0],
            [4, 4, 0, 0],
            [8, 4, 0, 0],
            [8, 2, 0, 0],
            [4, 2, 0, 0],
            [4, 0, 0, 0],
            [8, 0, 0, 0],
        ];
        let bpp = depths[mode as usize][bg];
        if bpp == 0 {
            return None;
        }
        if mode == 7 {
            return if bg == 0 { self.mode7(x, y) } else { None };
        }
        let mosaic = (self.regs[6] >> 4) as usize + 1;
        let (x, y) = if self.regs[6] & (1 << bg) != 0 {
            (x / mosaic * mosaic, y / mosaic * mosaic)
        } else {
            (x, y)
        };
        let sx = (x + self.scroll[bg][0] as usize) & 1023;
        let sy = (y + 1 + self.scroll[bg][1] as usize) & 1023;
        let size = if self.regs[5] & (16 << bg) != 0 {
            16
        } else {
            8
        };
        let tx = sx / size;
        let ty = sy / size;
        let sc = self.regs[7 + bg];
        let width = if sc & 1 != 0 { 64 } else { 32 };
        let height = if sc & 2 != 0 { 64 } else { 32 };
        let tx = tx % width;
        let ty = ty % height;
        let page = usize::from(tx >= 32) + usize::from(ty >= 32) * if width == 64 { 2 } else { 1 };
        let address = ((sc as usize & 0xfc) << 9) + page * 2048 + ((ty & 31) * 32 + (tx & 31)) * 2;
        let entry = self.vw(address);
        let mut px = sx & (size - 1);
        let mut py = sy & (size - 1);
        if entry & 0x4000 != 0 {
            px = size - 1 - px;
        }
        if entry & 0x8000 != 0 {
            py = size - 1 - py;
        }
        let tile = (entry as usize & 1023) + (px / 8) + (py / 8) * 16;
        let base = ((self.regs[0x0b + bg / 2] >> ((bg & 1) * 4)) as usize & 15) * 8192;
        let color = self.tile_pixel(base, tile, px & 7, py & 7, bpp);
        if color == 0 {
            return None;
        }
        let pal = ((entry >> 10) & 7) as usize;
        let color = if bpp == 8 {
            color as usize
        } else {
            color as usize + pal * (1 << bpp) + if mode == 0 { bg * 32 } else { 0 }
        };
        let high = entry & 0x2000 != 0;
        let rank = match mode {
            0 => [[8, 11], [7, 10], [2, 5], [1, 4]][bg][usize::from(high)],
            1 => {
                if bg == 2 && high && self.regs[5] & 8 != 0 {
                    12
                } else {
                    [[7, 10], [6, 9], [1, 3], [0, 0]][bg][usize::from(high)]
                }
            }
            _ => [[3, 7], [1, 5], [0, 0], [0, 0]][bg][usize::from(high)],
        };
        Some(Pixel {
            color: self.palette(color),
            rank,
            layer: bg as u8,
            math: true,
        })
    }
    fn mode7(&self, x: usize, y: usize) -> Option<Pixel> {
        let sel = self.regs[0x1a];
        let x = if sel & 1 != 0 { 255 - x } else { x } as i32;
        let y = if sel & 2 != 0 { 255 - y } else { y } as i32;
        let sx = ((self.scroll[0][0] as i32) << 19) >> 19;
        let sy = ((self.scroll[0][1] as i32) << 19) >> 19;
        let cx = ((self.m7[4] as i32) << 19) >> 19;
        let cy = ((self.m7[5] as i32) << 19) >> 19;
        let px =
            ((self.m7[0] as i32 * (x + sx - cx) + self.m7[1] as i32 * (y + sy - cy)) >> 8) + cx;
        let py =
            ((self.m7[2] as i32 * (x + sx - cx) + self.m7[3] as i32 * (y + sy - cy)) >> 8) + cy;
        let outside = !(0..1024).contains(&px) || !(0..1024).contains(&py);
        if outside && sel & 0xc0 == 0x80 {
            return None;
        }
        let px = (px & 1023) as usize;
        let py = (py & 1023) as usize;
        let tile = if outside && sel & 0xc0 == 0xc0 {
            0
        } else {
            self.vram[((py / 8) * 128 + px / 8) * 2] as usize
        };
        let color = self.vram[(tile * 128 + (py & 7) * 16 + (px & 7) * 2 + 1) & 65535];
        if color == 0 {
            None
        } else {
            Some(Pixel {
                color: self.palette(color as usize),
                rank: 3,
                layer: 0,
                math: true,
            })
        }
    }
    fn window(&self, layer: usize, x: usize) -> bool {
        let sel = (self.regs[0x23 + layer / 2] >> ((layer & 1) * 4)) & 15;
        let w1 =
            (self.regs[0x26] as usize..=self.regs[0x27] as usize).contains(&x) ^ (sel & 1 != 0);
        let w2 =
            (self.regs[0x28] as usize..=self.regs[0x29] as usize).contains(&x) ^ (sel & 4 != 0);
        match (sel & 2 != 0, sel & 8 != 0) {
            (false, false) => false,
            (true, false) => w1,
            (false, true) => w2,
            (true, true) => match (self.regs[0x2a + layer / 4] >> ((layer & 3) * 2)) & 3 {
                0 => w1 || w2,
                1 => w1 && w2,
                2 => w1 ^ w2,
                _ => !(w1 ^ w2),
            },
        }
    }
    fn objects(&self, y: usize) -> [Option<Pixel>; 256] {
        let mut pixels = [None; 256];
        let sizes = [
            (8, 16),
            (8, 32),
            (8, 64),
            (16, 32),
            (16, 64),
            (32, 64),
            (16, 32),
            (16, 32),
        ];
        let (small, large) = sizes[(self.regs[1] >> 5) as usize];
        let base = (self.regs[1] as usize & 7) * 16384;
        let gap = (((self.regs[1] >> 3) as usize & 3) + 1) * 8192;
        let start = if self.regs[3] & 128 != 0 {
            (self.oaddr / 4) & 127
        } else {
            0
        };
        let mut count = 0;
        let mut tiles = 0;
        for n in 0..128 {
            let i = (n + start) & 127;
            let hi = (self.oam[512 + i / 4] >> ((i & 3) * 2)) & 3;
            let size = if hi & 2 != 0 { large } else { small };
            let sy = self.oam[i * 4 + 1] as usize;
            let row = (y + 256 - sy) & 255;
            if row >= size {
                continue;
            }
            count += 1;
            if count > 32 {
                break;
            }
            let sx = self.oam[i * 4] as i32 - if hi & 1 != 0 { 256 } else { 0 };
            let attr = self.oam[i * 4 + 3];
            let name = self.oam[i * 4 + 2] as usize;
            let row = if attr & 128 != 0 { size - 1 - row } else { row };
            for col in 0..size {
                let screen = sx + col as i32;
                if !(0..256).contains(&screen) {
                    continue;
                }
                if col & 7 == 0 {
                    tiles += 1;
                    if tiles > 34 {
                        break;
                    }
                }
                let tx = if attr & 64 != 0 { size - 1 - col } else { col };
                let tile =
                    (name & 0xf0).wrapping_add((row / 8) * 16) & 255 | ((name + tx / 8) & 15);
                let color = self.tile_pixel(
                    base + if attr & 1 != 0 { gap } else { 0 },
                    tile,
                    tx & 7,
                    row & 7,
                    4,
                );
                let screen = screen as usize;
                if color == 0 || pixels[screen].is_some() {
                    continue;
                }
                let pal = ((attr >> 1) & 7) as usize;
                let rank = match self.regs[5] & 7 {
                    0 => [3, 6, 9, 12],
                    1 => [2, 4, 8, 11],
                    _ => [2, 4, 6, 8],
                }[((attr >> 4) & 3) as usize];
                pixels[screen] = Some(Pixel {
                    color: self.palette(128 + pal * 16 + color as usize),
                    rank,
                    layer: 4,
                    math: pal >= 4,
                });
            }
        }
        pixels
    }
    pub fn render_line(&mut self, y: usize) {
        if y >= HEIGHT {
            return;
        }
        if self.regs[0] & 128 != 0 {
            for x in 0..256 {
                let i = (y * 256 + x) * 4;
                self.frame[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
            return;
        }
        let objects = self.objects(y);
        for (x, obj) in objects.iter().enumerate() {
            let back = Pixel {
                color: self.palette(0),
                rank: 0,
                layer: 5,
                math: true,
            };
            let mut screens = [back; 2];
            for (screen, pixel) in screens.iter_mut().enumerate() {
                let enable = self.regs[0x2c + screen];
                let mask = self.regs[0x2e + screen];
                for bg in 0..4 {
                    if enable & (1 << bg) == 0 || (mask & (1 << bg) != 0 && self.window(bg, x)) {
                        continue;
                    }
                    if let Some(p) = self.bg_pixel(bg, x, y) {
                        if p.rank > pixel.rank {
                            *pixel = p;
                        }
                    }
                }
                if enable & 16 != 0 && !(mask & 16 != 0 && self.window(4, x)) {
                    if let Some(p) = obj {
                        if p.rank > pixel.rank {
                            *pixel = *p;
                        }
                    }
                }
            }
            let main = screens[0];
            let in_window = self.window(5, x);
            let clip = match self.regs[0x30] >> 6 {
                0 => false,
                1 => !in_window,
                2 => in_window,
                _ => true,
            };
            let prevent = match (self.regs[0x30] >> 4) & 3 {
                0 => false,
                1 => !in_window,
                2 => in_window,
                _ => true,
            };
            let color = if clip { 0 } else { main.color };
            let math = !prevent && main.math && self.regs[0x31] & (1 << main.layer) != 0;
            let use_sub = self.regs[0x30] & 2 != 0;
            let sub = if use_sub {
                screens[1].color
            } else {
                self.fixed[0] as u16
                    | ((self.fixed[1] as u16) << 5)
                    | ((self.fixed[2] as u16) << 10)
            };
            let bright = (self.regs[0] & 15) as u16;
            let i = (y * 256 + x) * 4;
            for ch in 0..3 {
                let mut value = ((color >> (ch * 5)) & 31) as i16;
                if math {
                    let s = ((sub >> (ch * 5)) & 31) as i16;
                    value = if self.regs[0x31] & 128 != 0 {
                        value - s
                    } else {
                        value + s
                    };
                    if self.regs[0x31] & 64 != 0 && !clip && (!use_sub || screens[1].layer != 5) {
                        value /= 2;
                    }
                    value = value.clamp(0, 31);
                }
                self.frame[i + ch] = (value as u32 * 255 * bright as u32 / (31 * 15)) as u8;
            }
            self.frame[i + 3] = 255;
        }
    }
}
