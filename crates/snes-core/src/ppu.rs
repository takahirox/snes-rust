//! Scanline renderer for planar backgrounds and objects.
pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 224;
pub struct Ppu {
    pub vram: Vec<u8>,
    pub cgram: [u8; 512],
    pub oam: [u8; 544],
    pub regs: [u8; 64],
    pub frame: Vec<u8>,
    frame_width: usize,
    vaddr: u16,
    vbuffer: u16,
    caddr: usize,
    clatch: u8,
    oaddr: usize,
    // Memory address can be owned by rendering without changing rotation.
    oport_addr: usize,
    olatch: u8,
    scroll_latch: u8,
    scroll: [[u16; 2]; 4],
    m7: [i16; 6],
    m7_scroll: [u16; 2],
    m7_latch: u8,
    fixed: [u8; 3],
    pub h: u16,
    pub v: u16,
    hflip: bool,
    vflip: bool,
    counters: [u16; 2],
    counter_latched: bool,
    latch_enabled: bool,
    open1: u8,
    open2: u8,
    obj_overflow: u8,
}
impl Default for Ppu {
    fn default() -> Self {
        Self {
            vram: vec![0; 65536],
            cgram: [0; 512],
            oam: [0; 544],
            regs: {
                let mut r = [0; 64];
                r[0] = 0x80;
                r
            },
            frame: vec![0; WIDTH * HEIGHT * 4],
            frame_width: WIDTH,
            vaddr: 0,
            vbuffer: 0,
            caddr: 0,
            clatch: 0,
            oaddr: 0,
            oport_addr: 0,
            olatch: 0,
            scroll_latch: 0,
            scroll: [[0; 2]; 4],
            m7: [0; 6],
            m7_scroll: [0; 2],
            m7_latch: 0,
            fixed: [0; 3],
            h: 0,
            v: 0,
            hflip: false,
            vflip: false,
            counters: [0; 2],
            counter_latched: false,
            latch_enabled: true,
            open1: 0,
            open2: 0,
            obj_overflow: 0,
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
    pub(crate) fn obj_frame_start(&mut self) {
        // Fullsnes STAT77: end-of-VBlank reset is inhibited by forced blank.
        // Assertion remains scanline-granular; see issue-57-validation.md.
        if self.regs[0] & 128 == 0 {
            self.obj_overflow = 0;
        }
    }
    fn reload_oam_address(&mut self) {
        self.oaddr = (((self.regs[3] as usize & 1) << 8) | self.regs[2] as usize) * 2;
        self.oport_addr = self.oaddr;
        // No documented data-latch reset: preserve it as model policy.
    }
    fn rendering_oam_address(&mut self) {
        // No object-fetch pipeline: use two high-table bytes, preserving phase.
        // This address is policy, NOT a hardware-verified corruption address.
        // See docs/issue-43-validation.md for sources, conflicts and limits.
        self.oport_addr = 0x200 | (self.oport_addr & 1);
    }
    pub(crate) fn oam_scanline_start(&mut self, line: u16) {
        if self.regs[0] & 128 == 0 && line < 225 {
            self.rendering_oam_address();
        }
    }
    pub(crate) fn oam_vblank_reload(&mut self) {
        if self.regs[0] & 128 == 0 {
            self.reload_oam_address();
        }
    }
    fn oam_byte_address(&mut self) -> usize {
        if self.regs[0] & 128 == 0 && self.v < 225 {
            self.rendering_oam_address();
        }
        if self.oport_addr & 512 != 0 {
            512 | (self.oport_addr & 31)
        } else {
            self.oport_addr
        }
    }
    fn advance_oam_address(&mut self) {
        self.oport_addr = (self.oport_addr + 1) & 1023;
        self.oaddr = (self.oaddr + 1) & 1023;
    }
    #[cfg(test)]
    pub(crate) fn oam_port_state(&self) -> (usize, usize, u8) {
        (self.oport_addr, self.oaddr, self.olatch)
    }
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
    #[cfg(test)]
    pub(crate) fn vram_read_state(&self) -> (u16, u16) {
        (self.vaddr, self.vbuffer)
    }
    fn prefetch_vram(&mut self) {
        // Fullsnes VRAM access / INIDISP: no software-addressed fetch outside
        // forced blank or VBlank (including HBlank and scanline 0). Invalid
        // fetch bits are unspecified; retaining the buffer is model policy.
        // See docs/issue-41-validation.md for evidence and qualifications.
        if self.regs[0] & 128 != 0 || self.v >= 225 {
            self.vbuffer = self.vw(self.va());
        }
    }
    fn palette(&self, i: usize) -> u16 {
        u16::from_le_bytes([self.cgram[(i * 2) & 511], self.cgram[(i * 2 + 1) & 511]]) & 0x7fff
    }
    fn cgram_byte_address(&self) -> usize {
        // CGRAM permits HBlank palette transfers, unlike VRAM. Without a
        // pixel-fetch pipeline, use backdrop color 0 during rendering and
        // preserve the software byte phase/cursor. This address and the whole
        // HBlank window are model policy, NOT exact hardware arbitration.
        // See docs/issue-47-validation.md for source conflicts and limits.
        if self.regs[0] & 128 == 0 && self.v < 225 && self.h < 1096 {
            self.caddr & 1
        } else {
            self.caddr
        }
    }
    #[cfg(test)]
    pub(crate) fn cgram_port_state(&self) -> (usize, u8) {
        (self.caddr, self.clatch)
    }
    // Fullsnes Direct Color Mode: pixel BBGGGRRR and tile palette bgr
    // supply RRRr0, GGGg0, BBb00. Transparency is checked by the caller.
    fn direct_color(pixel: u8, palette: u8) -> u16 {
        let p = pixel as u16;
        let t = palette as u16;
        ((p & 7) << 2)
            | ((t & 1) << 1)
            | ((p & 0x38) << 4)
            | ((t & 2) << 5)
            | ((p & 0xc0) << 7)
            | ((t & 4) << 10)
    }
    pub fn write(&mut self, r: u8, v: u8) {
        let r = (r & 63) as usize;
        let was_forced = self.regs[0] & 128 != 0;
        self.regs[r] = v;
        match r {
            0 => {
                // Anomie INIDISP: any write with old forced blank set on the
                // first VBlank line reloads OAM. Fullsnes confirms deactivation.
                if was_forced && self.v == 225 {
                    self.reload_oam_address();
                } else if was_forced && v & 128 == 0 && self.v < 225 {
                    self.rendering_oam_address();
                }
            }
            2 | 3 => self.reload_oam_address(),
            4 => {
                let a = self.oam_byte_address();
                // Fullsnes EVEN rule also applies to high-table writes.
                if a & 1 == 0 {
                    self.olatch = v;
                }
                if a >= 512 {
                    self.oam[a] = v;
                } else if a & 1 != 0 {
                    self.oam[a - 1] = self.olatch;
                    self.oam[a] = v;
                }
                self.advance_oam_address();
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
                if bg == 0 {
                    // Both BG1 ports also capture independent Mode 7 offsets,
                    // sharing the matrix/center previous byte in every BGMODE.
                    self.m7_scroll[axis] = u16::from_le_bytes([self.m7_latch, v]);
                    self.m7_latch = v;
                }
            }
            0x16 | 0x17 => {
                let mut address = self.vaddr.to_le_bytes();
                address[r - 0x16] = v;
                self.vaddr = u16::from_le_bytes(address);
                self.prefetch_vram();
            }
            0x18 | 0x19 => {
                let high = r == 0x19;
                let a = self.va() + usize::from(high);
                // VRAM is unavailable during active display, including HBlank.
                // The address increment still occurs on a blocked write.
                if self.regs[0] & 128 != 0 || self.v >= 225 {
                    self.vram[a & 65535] = v;
                }
                self.vinc(high);
            }
            0x1b..=0x20 => {
                self.m7[r - 0x1b] = i16::from_le_bytes([self.m7_latch, v]);
                self.m7_latch = v;
            }
            0x21 => self.caddr = v as usize * 2,
            0x22 => {
                if self.caddr & 1 == 0 {
                    self.clatch = v;
                } else {
                    let a = self.cgram_byte_address();
                    self.cgram[a - 1] = self.clatch;
                    self.cgram[a] = v & 127;
                }
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
    // WRIO bit 7 drives the latch input; initialization starts high without
    // synthesizing an edge. Captures leave both read selectors untouched.
    pub(crate) fn set_latch_enabled(&mut self, enabled: bool) {
        if self.latch_enabled && !enabled {
            self.latch_counters();
        }
        self.latch_enabled = enabled;
    }
    pub fn latch_counters(&mut self) {
        // Retained scheduler model: four master clocks per dot (not long-dot accurate).
        self.counters = [self.h / 4, self.v];
        self.counter_latched = true;
    }
    pub fn read(&mut self, r: u8, open: u8) -> u8 {
        let r = r & 63;
        let value = match r {
            0x34..=0x36 => {
                let product = self.m7[0] as i32 * (self.m7[1] >> 8) as i32;
                (product >> ((r - 0x34) * 8)) as u8
            }
            0x37 => {
                if self.latch_enabled {
                    self.latch_counters();
                }
                open
            }
            0x38 => {
                let a = self.oam_byte_address();
                let v = self.oam[a];
                self.advance_oam_address();
                v
            }
            0x39 | 0x3a => {
                let high = r & 63 == 0x3a;
                let v = (self.vbuffer >> if high { 8 } else { 0 }) as u8;
                if (self.regs[0x15] & 128 != 0) == high {
                    self.prefetch_vram();
                    self.vinc(high);
                }
                v
            }
            0x3b => {
                let v = self.cgram[self.cgram_byte_address()]
                    | if self.caddr & 1 != 0 {
                        self.open2 & 128
                    } else {
                        0
                    };
                self.caddr = (self.caddr + 1) & 511;
                v
            }
            0x3c => {
                let v = if self.hflip {
                    (self.open2 & 0xfe) | (self.counters[0] >> 8) as u8
                } else {
                    self.counters[0] as u8
                };
                self.hflip = !self.hflip;
                v
            }
            0x3d => {
                let v = if self.vflip {
                    (self.open2 & 0xfe) | (self.counters[1] >> 8) as u8
                } else {
                    self.counters[1] as u8
                };
                self.vflip = !self.vflip;
                v
            }
            0x3e => self.obj_overflow | (self.open1 & 0x10) | 1,
            0x3f => {
                self.hflip = false;
                self.vflip = false;
                let status = (self.open2 & 0x20)
                    | if self.counter_latched || !self.latch_enabled {
                        0x40
                    } else {
                        0
                    }
                    | 3;
                if self.latch_enabled {
                    self.counter_latched = false;
                }
                status
            }
            0x04..=0x06 | 0x08..=0x0a | 0x14..=0x16 | 0x18..=0x1a | 0x24..=0x26 | 0x28..=0x2a => {
                self.open1
            }
            _ => open,
        };
        if matches!(r, 0x34..=0x36 | 0x38..=0x3a | 0x3e) {
            self.open1 = value;
        }
        if matches!(r, 0x3b..=0x3d | 0x3f) {
            self.open2 = value;
        }
        value
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
    fn bg_hires(&self) -> bool {
        matches!(self.regs[5] & 7, 5 | 6)
    }
    fn wide_output(&self) -> bool {
        self.bg_hires() || self.regs[0x33] & 8 != 0
    }
    fn bg_tile_width(&self, bg: usize) -> usize {
        if self.bg_hires() {
            16
        } else {
            self.bg_tile_size(bg)
        }
    }
    fn bg_tile_size(&self, bg: usize) -> usize {
        if self.regs[5] & (16 << bg) != 0 {
            16
        } else {
            8
        }
    }
    // Read the raw screen word: BG3 offset descriptors must not be interpreted
    // as character numbers, palettes, priorities or flips.
    fn bg_map_entry(&self, bg: usize, x: usize, y: usize) -> u16 {
        self.bg_map_word(bg, x, y, self.bg_tile_width(bg))
    }
    fn bg_map_word(&self, bg: usize, x: usize, y: usize, column_width: usize) -> u16 {
        let size = self.bg_tile_size(bg);
        let sc = self.regs[7 + bg];
        let width = if sc & 1 != 0 { 64 } else { 32 };
        let height = if sc & 2 != 0 { 64 } else { 32 };
        let tx = (x & 1023) / column_width % width;
        let ty = (y & 1023) / size % height;
        let page = usize::from(tx >= 32) + usize::from(ty >= 32) * if width == 64 { 2 } else { 1 };
        let address = ((sc as usize & 0xfc) << 9) + page * 2048 + ((ty & 31) * 32 + (tx & 31)) * 2;
        self.vw(address)
    }
    fn bg_source(&self, bg: usize, x: usize, y: usize) -> (usize, usize) {
        let hofs = self.scroll[bg][0] as usize;
        // HOFS counts full dots even in hires; x counts half-dot samples.
        // Map widths divide 1024, so mask after doubling (see Issue #49).
        let mut sx = (x + hofs * if self.bg_hires() { 2 } else { 1 }) & 1023;
        let mut sy = (y + 1 + self.scroll[bg][1] as usize) & 1023;
        let mode = self.regs[5] & 7;
        // Offset columns count full dots, including in Mode 6. Its BG3
        // columns are always eight dots; the BG3 size bit affects only height.
        // See docs/issue-51-validation.md for the unit/boundary derivation.
        let scale = if mode == 6 { 2 } else { 1 };
        let q = x / scale + (hofs & 7);
        if bg < 2 && matches!(mode, 2 | 4 | 6) && q >= 8 {
            // Offset columns are always eight pixels, independent of BGn's
            // character size. In Modes 2/4, BG3's size also controls width.
            let table_x = q - 8 + (self.scroll[2][0] as usize & !7);
            let table_y = self.scroll[2][1] as usize;
            let column_width = if mode == 6 { 8 } else { self.bg_tile_size(2) };
            let h = self.bg_map_word(2, table_x, table_y, column_width);
            let enable = 0x2000 << bg;
            if mode == 4 {
                if h & enable != 0 {
                    if h & 0x8000 == 0 {
                        sx = (q + (h as usize & 0x3f8)) & 1023;
                    } else {
                        sy = (y + 1 + (h as usize & 0x3ff)) & 1023;
                    }
                }
            } else {
                let v = self.bg_map_word(2, table_x, table_y + 8, column_width);
                if h & enable != 0 {
                    sx = (x + scale * ((hofs & 7) + (h as usize & 0x3f8))) & 1023;
                }
                if v & enable != 0 {
                    sy = (y + 1 + (v as usize & 0x3ff)) & 1023;
                }
            }
        }
        (sx, sy)
    }
    fn bg_pixel(&self, bg: usize, x: usize, y: usize) -> Option<Pixel> {
        let mode = self.regs[5] & 7;
        if mode == 7 {
            return if bg == 0 || (bg == 1 && self.regs[0x33] & 0x40 != 0) {
                self.mode7(bg, x, y)
            } else {
                None
            };
        }
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
        let mosaic = (self.regs[6] >> 4) as usize + 1;
        let (x, y) = if self.regs[6] & (1 << bg) != 0 {
            (x / mosaic * mosaic, y / mosaic * mosaic)
        } else {
            (x, y)
        };
        let (sx, sy) = self.bg_source(bg, x, y);
        let size = self.bg_tile_size(bg);
        let entry = self.bg_map_entry(bg, sx, sy);
        let tile_width = self.bg_tile_width(bg);
        let mut px = sx & (tile_width - 1);
        let mut py = sy & (size - 1);
        if entry & 0x4000 != 0 {
            px = tile_width - 1 - px;
        }
        if entry & 0x8000 != 0 {
            py = size - 1 - py;
        }
        let tile = ((entry as usize & 1023) + (px / 8) + (py / 8) * 16) & 1023;
        let base = ((self.regs[0x0b + bg / 2] >> ((bg & 1) * 4)) as usize & 15) * 8192;
        let color = self.tile_pixel(base, tile, px & 7, py & 7, bpp);
        if color == 0 {
            return None;
        }
        let pal = ((entry >> 10) & 7) as usize;
        let color = if bpp == 8 && self.regs[0x30] & 1 != 0 {
            Self::direct_color(color, pal as u8)
        } else {
            let index = if bpp == 8 {
                color as usize
            } else {
                color as usize + pal * (1 << bpp) + if mode == 0 { bg * 32 } else { 0 }
            };
            self.palette(index)
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
            color,
            rank,
            layer: bg as u8,
            math: true,
        })
    }
    fn mode7(&self, bg: usize, x: usize, y: usize) -> Option<Pixel> {
        let sel = self.regs[0x1a];
        let x = if sel & 1 != 0 { 255 - x } else { x } as i32;
        let y = if sel & 2 != 0 { 255 - y } else { y } as i32;
        let sx = ((self.m7_scroll[0] as i32) << 19) >> 19;
        let sy = ((self.m7_scroll[1] as i32) << 19) >> 19;
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
        let raw = self.vram[(tile * 128 + (py & 7) * 16 + (px & 7) * 2 + 1) & 65535];
        // EXTBG shares BG1's transformed byte, but keeps its own layer,
        // seven-bit palette index and per-pixel priority (Anomie, Mode 7).
        let color = if bg == 1 { raw & 0x7f } else { raw };
        if color == 0 {
            None
        } else {
            Some(Pixel {
                color: if bg == 0 && self.regs[0x30] & 1 != 0 {
                    Self::direct_color(color, 0)
                } else {
                    self.palette(color as usize)
                },
                // OBJ ranks are 2/4/6/8: BG2-low < OBJ0 < BG1 <
                // OBJ1 < BG2-high < OBJ2 < OBJ3.
                rank: if bg == 0 {
                    3
                } else if raw & 0x80 != 0 {
                    5
                } else {
                    1
                },
                layer: bg as u8,
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
    fn objects(&mut self, y: usize) -> [Option<Pixel>; 256] {
        let mut pixels = [None; 256];
        let sizes = [
            ((8, 8), (16, 16)),
            ((8, 8), (32, 32)),
            ((8, 8), (64, 64)),
            ((16, 16), (32, 32)),
            ((16, 16), (64, 64)),
            ((32, 32), (64, 64)),
            ((16, 32), (32, 64)),
            ((16, 32), (32, 32)),
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
        let mut selected = [(0, 0, 0, 0, 0); 32];
        for n in 0..128 {
            let i = (n + start) & 127;
            let hi = (self.oam[512 + i / 4] >> ((i & 3) * 2)) & 3;
            let (width, height) = if hi & 2 != 0 { large } else { small };
            let sy = self.oam[i * 4 + 1] as usize;
            let row = (y + 256 - sy) & 255;
            if row >= height {
                continue;
            }
            let sx = self.oam[i * 4] as i32 - if hi & 1 != 0 { 256 } else { 0 };
            // X=-256 participates at X=0 in both budgets, never in drawing.
            let evaluation_x = if sx == -256 { 0 } else { sx };
            if evaluation_x <= -(width as i32) || evaluation_x >= 256 {
                continue;
            }
            if count == 32 {
                self.obj_overflow |= 0x40;
                break;
            }
            selected[count] = (i, width, row, sx, evaluation_x);
            count += 1;
        }
        // Anomie Drawing the Sprites: reverse selection order for fetching,
        // destination columns left to right, including partially clipped tiles.
        // Compose back to front, so an earlier object's opaque pixels replace
        // later ones regardless of their BG-priority bits.
        let mut tiles = 0;
        'fetch: for &(i, width, row, sx, evaluation_x) in selected[..count].iter().rev() {
            let attr = self.oam[i * 4 + 3];
            let name = self.oam[i * 4 + 2] as usize;
            // Rectangles flip within each stacked square, preserving half order.
            // For squares this is the usual full-height reversal.
            let row = if attr & 128 != 0 {
                row ^ (width - 1)
            } else {
                row
            };
            for column in (0..width).step_by(8) {
                let tile_x = evaluation_x + column as i32;
                if tile_x <= -8 || tile_x >= 256 {
                    continue;
                }
                if tiles == 34 {
                    self.obj_overflow |= 0x80;
                    break 'fetch;
                }
                tiles += 1;
                for col in column..column + 8 {
                    let screen = sx + col as i32;
                    if !(0..256).contains(&screen) {
                        continue;
                    }
                    let tx = if attr & 64 != 0 { width - 1 - col } else { col };
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
                    if color == 0 {
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
        }
        pixels
    }
    pub fn framebuffer_width(&self) -> usize {
        self.frame_width
    }
    pub fn framebuffer_height(&self) -> usize {
        HEIGHT
    }
    fn prepare_frame_width(&mut self, y: usize, wide_output: bool) {
        let width = if wide_output { WIDTH * 2 } else { WIDTH };
        if y == 0 && self.frame_width != width {
            self.frame.resize(width * HEIGHT * 4, 0);
            self.frame.fill(0);
            self.frame_width = width;
        } else if wide_output && self.frame_width == WIDTH {
            // Promote packed rows backwards, preserving all earlier dots.
            // See issue-49-validation.md: this is a representation policy.
            self.frame.resize(WIDTH * 2 * HEIGHT * 4, 0);
            for i in (0..WIDTH * HEIGHT).rev() {
                let pixel: [u8; 4] = self.frame[i * 4..i * 4 + 4].try_into().unwrap();
                self.frame[i * 8..i * 8 + 4].copy_from_slice(&pixel);
                self.frame[i * 8 + 4..i * 8 + 8].copy_from_slice(&pixel);
            }
            self.frame_width = WIDTH * 2;
        }
    }
    pub fn render_line(&mut self, y: usize) {
        if y >= HEIGHT {
            return;
        }
        let wide_output = self.wide_output();
        self.prepare_frame_width(y, wide_output);
        if self.regs[0] & 128 != 0 {
            for x in 0..self.frame_width {
                let i = (y * self.frame_width + x) * 4;
                self.frame[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
            return;
        }
        let fixed_color =
            self.fixed[0] as u16 | ((self.fixed[1] as u16) << 5) | ((self.fixed[2] as u16) << 10);
        let objects = self.objects(y);
        let samples = if wide_output { WIDTH * 2 } else { WIDTH };
        for output_x in 0..samples {
            let x = if wide_output { output_x / 2 } else { output_x };
            let obj = &objects[x];
            let back = Pixel {
                color: self.palette(0),
                rank: 0,
                layer: 5,
                math: true,
            };
            // Keep COLDATA as the subscreen math backdrop, retaining the layer
            // marker so the color-math result is not halved there.
            let mut screens = [
                back,
                Pixel {
                    color: fixed_color,
                    ..back
                },
            ];
            for (screen, pixel) in screens.iter_mut().enumerate() {
                let enable = self.regs[0x2c + screen];
                let mask = self.regs[0x2e + screen];
                for bg in 0..4 {
                    if enable & (1 << bg) == 0 || (mask & (1 << bg) != 0 && self.window(bg, x)) {
                        continue;
                    }
                    // Mode 5/6 BGs use distinct character phases. SETINI
                    // pseudo-hires keeps both screens at the same full dot.
                    let sample_x = if self.bg_hires() {
                        x * 2 + usize::from(screen == 0)
                    } else {
                        x
                    };
                    if let Some(p) = self.bg_pixel(bg, sample_x, y) {
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
            let visible_sub = wide_output && output_x & 1 == 0;
            let mut main = screens[usize::from(visible_sub)];
            // Fullsnes Hires Notes: both displayed backdrops use CGRAM[0]
            // in true/pseudo hires. Leave the math operand in screens intact.
            if visible_sub && main.layer == 5 {
                main.color = back.color;
            }
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
            let math =
                !visible_sub && !prevent && main.math && self.regs[0x31] & (1 << main.layer) != 0;
            let use_sub = self.regs[0x30] & 2 != 0;
            let sub = if use_sub {
                screens[1].color
            } else {
                fixed_color
            };
            let bright = (self.regs[0] & 15) as u16;
            let repeats = if !wide_output && self.frame_width == WIDTH * 2 {
                2
            } else {
                1
            };
            let i = (y * self.frame_width + output_x * repeats) * 4;
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
            if repeats == 2 {
                let pixel: [u8; 4] = self.frame[i..i + 4].try_into().unwrap();
                self.frame[i + 4..i + 8].copy_from_slice(&pixel);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_color_routes_every_pixel_and_palette_bit() {
        // Independent wiring oracle, from the documented channel bit layout.
        // This tests conversion even for zero; opacity belongs to the renderer.
        let pixel_weights = [4, 8, 16, 128, 256, 512, 8192, 16384];
        let palette_weights = [2, 64, 4096];
        for pixel in 0..=255u8 {
            for palette in 0..8u8 {
                let expected: u16 = pixel_weights
                    .iter()
                    .enumerate()
                    .filter(|(bit, _)| pixel & (1 << bit) != 0)
                    .map(|(_, weight)| weight)
                    .chain(
                        palette_weights
                            .iter()
                            .enumerate()
                            .filter(|(bit, _)| palette & (1 << bit) != 0)
                            .map(|(_, weight)| weight),
                    )
                    .sum();
                assert_eq!(Ppu::direct_color(pixel, palette), expected);
            }
        }
        for (pixel, palette, expected) in [
            (0, 0, 0),
            (1, 0, 0x0004),
            (2, 0, 0x0008),
            (4, 0, 0x0010),
            (8, 0, 0x0080),
            (16, 0, 0x0100),
            (32, 0, 0x0200),
            (64, 0, 0x2000),
            (128, 0, 0x4000),
            (0, 1, 0x0002),
            (0, 2, 0x0040),
            (0, 4, 0x1000),
            (1, 1, 0x0006),
            (255, 0, 0x639c),
            (255, 7, 0x73de),
        ] {
            assert_eq!(Ppu::direct_color(pixel, palette), expected);
        }
    }

    #[test]
    fn counters_hold_the_latched_position_and_keep_independent_read_flipflops() {
        let mut p = Ppu {
            h: 1200,
            v: 260,
            ..Ppu::default()
        };
        assert_eq!(p.read(0x37, 0xa5), 0xa5);
        p.h = 4;
        p.v = 1;
        assert_eq!(p.read(0x3c, 0), 44);
        assert_eq!(p.read(0x3d, 0), 4);
        assert_eq!(p.read(0x3c, 0) & 1, 1);
        assert_eq!(p.read(0x3d, 0) & 1, 1);
        assert_eq!(p.read(0x3f, 0) & 0x40, 0x40);
        assert_eq!(p.read(0x3f, 0) & 0x40, 0);
        assert_eq!(p.read(0x3c, 0), 44);
    }

    #[test]
    fn captures_preserve_independent_selectors_and_stat78_always_resets_them() {
        let mut p = Ppu {
            h: 1200,
            v: 260,
            ..Ppu::default()
        };
        p.read(0x37, 0); // (300,260)
        assert_eq!(p.read(0x3c, 0), 44); // H high next; V still low.
        p.h = 40;
        p.v = 17;
        p.read(0x37, 0); // capture between H low/high
        assert_eq!(p.read(0x3c, 0) & 1, 0);
        assert_eq!(p.read(0x3d, 0), 17);
        p.h = 1280;
        p.v = 258;
        p.set_latch_enabled(false); // edge between V low/high
        assert_eq!(p.read(0x3d, 0) & 1, 1);
        assert_eq!(p.read(0x3c, 0), 64);
        assert_eq!(p.read(0x3d, 0), 2);
        for _ in 0..2 {
            assert_eq!(p.read(0x3f, 0) & 0x40, 0x40);
            assert_eq!(p.read(0x3c, 0), 64);
            assert_eq!(p.read(0x3d, 0), 2);
        }
        p.set_latch_enabled(true);
        assert_eq!(p.read(0x3f, 0) & 0x40, 0x40);
        assert_eq!(p.read(0x3c, 0), 64);
        assert_eq!(p.read(0x3d, 0), 2);
        assert_eq!(p.read(0x3f, 0) & 0x40, 0);
        assert_eq!(p.read(0x3c, 0), 64);
        assert_eq!(p.read(0x3d, 0), 2);
    }

    #[test]
    fn vram_prefetch_dummy_read_increment_remapping_and_display_write_gate() {
        let mut p = Ppu::default();
        p.vram[..6].copy_from_slice(&[0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc]);
        p.write(0x15, 0x80);
        p.write(0x16, 0);
        p.write(0x17, 0);
        // The address write prefetches, then the first high read reloads that
        // same word and increments: the first valid word is received twice.
        // A dummy read can discard either duplicate, not an invalid first word.
        for expected in [0x12, 0x12, 0x56] {
            assert_eq!(p.read(0x39, 0), expected);
            p.read(0x3a, 0);
        }
        p.write(0, 0x0f);
        p.v = 100;
        p.h = 1200; // HBlank is insufficient.
        p.write(0x16, 0);
        p.write(0x17, 0);
        p.write(0x18, 0xaa);
        p.write(0x19, 0xbb);
        assert_eq!(&p.vram[..2], &[0x12, 0x34]);
        assert_eq!(p.vaddr, 1);
        p.v = 225;
        p.write(0x18, 0xcc);
        p.write(0x19, 0xdd);
        assert_eq!(&p.vram[2..4], &[0xcc, 0xdd]);
        p.write(0x15, 0x84);
        p.write(0x16, 0x21);
        p.write(0x17, 0);
        p.write(0x18, 0xee);
        p.write(0x19, 0xff);
        // Remapping mode 1: word $21 -> word $09 -> byte $12.
        assert_eq!(&p.vram[0x12..0x14], &[0xee, 0xff]);
    }

    #[test]
    fn oam_pairs_high_table_mirror_and_cgram_open_bit_are_separate_latches() {
        let mut p = Ppu::default();
        p.write(2, 0);
        p.write(3, 0);
        p.write(4, 0x81);
        assert_eq!(p.oam[0], 0);
        p.write(4, 0x23);
        assert_eq!(&p.oam[..2], &[0x81, 0x23]);
        p.write(2, 0x10);
        p.write(3, 1);
        p.write(4, 0x55);
        assert_eq!(p.oam[512], 0x55);
        p.write(0x21, 0);
        p.write(0x22, 0x80);
        p.write(0x22, 0xff);
        p.write(0x21, 0);
        assert_eq!(p.read(0x3b, 0), 0x80);
        assert_eq!(p.read(0x3b, 0), 0xff); // bit 7 is PPU2's bus, not CGRAM.
        assert_eq!(p.cgram[1], 0x7f);
    }
}

#[cfg(test)]
#[path = "ppu_vram_tests.rs"]
mod vram_tests;

#[cfg(test)]
#[path = "ppu_oam_tests.rs"]
mod oam_tests;

#[cfg(test)]
#[path = "ppu_cgram_tests.rs"]
mod cgram_tests;

#[cfg(test)]
#[path = "ppu_mode7_tests.rs"]
mod mode7_tests;
