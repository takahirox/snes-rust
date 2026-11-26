//! Raster-progress compositor for planar backgrounds and objects.
pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 224;
pub struct Ppu {
    pub vram: Box<[u8; 0x10000]>,
    pub cgram: [u8; 512],
    pub oam: [u8; 544],
    pub regs: [u8; 64],
    pub frame: Vec<u8>,
    frame_width: usize,
    raster_x: usize,
    // Packed OBJ line for the current scanline: color | rank << 16 | math << 24.
    obj_line: [u32; WIDTH],
    obj_line_valid: bool,
    line: Option<Box<LineBuffers>>,
    // Output channel levels for the brightness in `levels_for`.
    levels: [u8; 32],
    levels_for: u8,
    // RGBA output of each CGRAM entry at that brightness, for the CGRAM
    // contents in `rgba_cgram`. Compared every span, so any change to
    // `cgram` (including direct writes) refreshes the affected entries.
    rgba_cache: [u32; 256],
    rgba_cgram: [u8; 512],
    // For the OAM in `obj_oam` and size select `obj_sizes`: per scanline,
    // the objects whose rows cover it and whose X range is evaluated.
    // Compared every line, so any OAM change (including direct writes)
    // rebuilds it.
    obj_rows: Box<[u128; 256]>,
    obj_oam: [u8; 544],
    obj_sizes: u8,
    raster_overflow: u8,
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
    // Initial/reset field zero is an emulator convention; see Issue #95.
    pub(crate) field: bool,
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
            vram: crate::zeroed(),
            cgram: [0; 512],
            oam: [0; 544],
            regs: {
                let mut r = [0; 64];
                r[0] = 0x80;
                r
            },
            frame: vec![0; WIDTH * HEIGHT * 4],
            frame_width: WIDTH,
            raster_x: 0,
            obj_line: [0; WIDTH],
            obj_line_valid: false,
            line: Some(Box::new(LineBuffers {
                bg: [[0; WIDTH * 2]; 4],
                win: [[false; WIDTH]; 6],
                screen: [[0; WIDTH]; 2],
            })),
            levels: [0; 32],
            levels_for: 0,
            rgba_cache: [u32::from_le_bytes([0, 0, 0, 255]); 256],
            rgba_cgram: [0; 512],
            obj_rows: Box::new([0; 256]),
            obj_oam: [0; 544],
            obj_sizes: 0,
            raster_overflow: 0,
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
            field: false,
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
}
// Per-span scratch buffers. Layer pixels pack rank << 24 | math << 23 |
// layer << 16 | color. Zero is transparent because every opaque BG and OBJ
// pixel has a nonzero rank; the backdrop (layer 5) has rank zero.
struct LineBuffers {
    bg: [[u32; WIDTH * 2]; 4],
    win: [[bool; WIDTH]; 6],
    screen: [[u32; WIDTH]; 2],
}
const MATH: u32 = 1 << 23;
const BACKDROP: u32 = MATH | (5 << 16);
// The low byte is a CGRAM index, resolved only for the composited winner;
// otherwise the low 15 bits are the color itself.
const INDEXED: u32 = 1 << 15;
// Output-stage constants of one span. CGRAM cannot change within a span.
struct Blend<'a> {
    cgram: &'a [u8; 512],
    rgba_cache: &'a [u32; 256],
    levels: [u8; 32],
    back_color: u16,
    fixed_color: u16,
    math_control: u8,
    use_sub: bool,
    clip_mode: u8,
    math_window_mode: u8,
}
impl Blend<'_> {
    #[inline(always)]
    fn resolve(&self, pixel: u32) -> u16 {
        if pixel & INDEXED != 0 {
            let i = (pixel & 255) as usize * 2;
            u16::from_le_bytes([self.cgram[i], self.cgram[i + 1]]) & 0x7fff
        } else {
            pixel as u16 & 0x7fff
        }
    }
    // (clip to black, prevent color math) for a color-window state.
    #[inline(always)]
    fn windows(&self, in_window: bool) -> (bool, bool) {
        let apply = |mode: u8| match mode {
            0 => false,
            1 => !in_window,
            2 => in_window,
            _ => true,
        };
        (apply(self.clip_mode), apply(self.math_window_mode))
    }
    // Color math on one displayed main-screen pixel; `sub_pixel` is the
    // subscreen (or COLDATA backdrop) operand.
    #[inline(always)]
    fn blend(&self, pixel: u32, sub_pixel: u32, clip: bool, prevent: bool) -> u16 {
        let color = if clip { 0 } else { self.resolve(pixel) };
        let math =
            !prevent && pixel & MATH != 0 && self.math_control & (1 << ((pixel >> 16) & 7)) != 0;
        if !math {
            return color;
        }
        let sub = if self.use_sub {
            self.resolve(sub_pixel)
        } else {
            self.fixed_color
        };
        let halve =
            self.math_control & 64 != 0 && !clip && (!self.use_sub || (sub_pixel >> 16) & 7 != 5);
        let mut result = 0;
        for ch in 0..3 {
            let a = ((color >> (ch * 5)) & 31) as i16;
            let b = ((sub >> (ch * 5)) & 31) as i16;
            let mut v = if self.math_control & 128 != 0 {
                a - b
            } else {
                a + b
            };
            if halve {
                v /= 2;
            }
            result |= (v.clamp(0, 31) as u16) << (ch * 5);
        }
        result
    }
    // Fullsnes Hires Notes: both displayed backdrops use CGRAM[0] in
    // true/pseudo hires. The math operand keeps COLDATA.
    #[inline(always)]
    fn shown_sub(&self, sub_pixel: u32, clip: bool) -> u16 {
        if clip {
            0
        } else if (sub_pixel >> 16) & 7 == 5 {
            self.back_color
        } else {
            self.resolve(sub_pixel)
        }
    }
    #[inline(always)]
    fn rgba(&self, color: u16) -> [u8; 4] {
        rgba(&self.levels, color)
    }
    // Displayed main-screen output: cached when no math or clip applies.
    #[inline(always)]
    fn output(&self, pixel: u32, sub_pixel: u32, clip: bool, prevent: bool) -> [u8; 4] {
        let math =
            !prevent && pixel & MATH != 0 && self.math_control & (1 << ((pixel >> 16) & 7)) != 0;
        if !math && !clip && pixel & INDEXED != 0 {
            return self.rgba_cache[(pixel & 255) as usize].to_le_bytes();
        }
        self.rgba(self.blend(pixel, sub_pixel, clip, prevent))
    }
}
#[inline(always)]
fn rgba(levels: &[u8; 32], color: u16) -> [u8; 4] {
    [
        levels[usize::from(color & 31)],
        levels[usize::from((color >> 5) & 31)],
        levels[usize::from((color >> 10) & 31)],
        255,
    ]
}
const DEPTHS: [[usize; 4]; 8] = [
    [2, 2, 2, 2],
    [4, 4, 2, 0],
    [4, 4, 0, 0],
    [8, 4, 0, 0],
    [8, 2, 0, 0],
    [4, 2, 0, 0],
    [4, 0, 0, 0],
    [8, 0, 0, 0],
];
// Spread each bit of a plane byte into the low bit of one byte, so that
// leftmost column 0 (bit 7) lands in byte 0.
const SPREAD: [u64; 256] = {
    let mut table = [0; 256];
    let mut byte = 0;
    while byte < 256 {
        let mut bit = 0;
        while bit < 8 {
            if byte & (1 << bit) != 0 {
                table[byte] |= 1 << ((7 - bit) * 8);
            }
            bit += 1;
        }
        byte += 1;
    }
    table
};
// Transpose the 8x8 bit matrix of plane bytes (byte k = plane k, bit 7 =
// column 0): byte c of the result holds column 7 - c's color index, plane k
// in bit k. Hacker's Delight, transpose8.
#[inline(always)]
fn transpose_planes(mut x: u64) -> u64 {
    let t = (x ^ (x >> 7)) & 0x00aa_00aa_00aa_00aa;
    x ^= t ^ (t << 7);
    let t = (x ^ (x >> 14)) & 0x0000_cccc_0000_cccc;
    x ^= t ^ (t << 14);
    let t = (x ^ (x >> 28)) & 0x0000_0000_f0f0_f0f0;
    x ^ t ^ (t << 28)
}
fn pack(pixel: Option<Pixel>, bg: usize) -> u32 {
    pixel.map_or(0, |p| {
        (u32::from(p.rank) << 24) | MATH | ((bg as u32) << 16) | u32::from(p.color)
    })
}
#[derive(Clone, Copy)]
struct BgRow {
    key: usize,
    planes: u64,
    palette_base: usize,
    direct_palette: Option<u8>,
    rank: u8,
    flip: bool,
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
    // NTSC non-interlaced geometry, also during forced blank. SETINI interlace,
    // overscan and PAL timing remain unsupported.
    pub(crate) fn line_clocks(&self) -> u64 {
        if self.field && self.v == 240 {
            1360
        } else {
            1364
        }
    }
    pub fn latch_counters(&mut self) {
        // Fullsnes counter-latch quantities: H=323 and H=327 last six clocks.
        // Remove their extra clocks before converting to the latched dot index.
        let h = if self.line_clocks() == 1360 {
            self.h / 4
        } else {
            match self.h {
                0..=1291 => self.h / 4,
                1292..=1297 => 323,
                1298..=1309 => (self.h - 2) / 4,
                1310..=1315 => 327,
                _ => (self.h - 4) / 4,
            }
        };
        self.counters = [h, self.v];
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
                let status = (u8::from(self.field) << 7)
                    | (self.open2 & 0x20)
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
    fn bg_pixel(&self, bg: usize, x: usize, y: usize, cache: &mut Option<BgRow>) -> Option<Pixel> {
        let mode = self.regs[5] & 7;
        if mode == 7 {
            return if bg == 0 || (bg == 1 && self.regs[0x33] & 0x40 != 0) {
                self.mode7(bg, x, y)
            } else {
                None
            };
        }
        let bpp = DEPTHS[mode as usize][bg];
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
        let key = (sy << 7) | (sx >> 3);
        let row = match *cache {
            Some(row) if row.key == key => row,
            _ => {
                let row = self.bg_row(bg, mode, bpp, sx, sy, key);
                *cache = Some(row);
                row
            }
        };
        let shift = if row.flip { sx & 7 } else { 7 - (sx & 7) };
        let bits = (row.planes >> shift) & 0x0101_0101_0101_0101;
        // Each byte contributes its selected bit to a different position
        // in the top byte, with plane zero as the low bit of the color.
        let color = (bits.wrapping_mul(0x0102_0408_1020_4080) >> 56) as u8;
        if color == 0 {
            return None;
        }
        let color = match row.direct_palette {
            Some(palette) => Self::direct_color(color, palette),
            None => self.palette(row.palette_base + usize::from(color)),
        };
        Some(Pixel {
            color,
            rank: row.rank,
        })
    }
    fn bg_row(&self, bg: usize, mode: u8, bpp: usize, sx: usize, sy: usize, key: usize) -> BgRow {
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
        let address = base + tile * bpp * 8 + (py & 7) * 2;
        let mut planes = u64::from(self.vw(address));
        if bpp >= 4 {
            planes |= u64::from(self.vw(address + 16)) << 16;
        }
        if bpp == 8 {
            planes |= u64::from(self.vw(address + 32)) << 32;
            planes |= u64::from(self.vw(address + 48)) << 48;
        }
        let palette = ((entry >> 10) & 7) as usize;
        let palette_base = if bpp == 8 {
            0
        } else {
            palette * (1 << bpp) + if mode == 0 { bg * 32 } else { 0 }
        };
        let rank = self.bg_rank(mode, bg, entry & 0x2000 != 0);
        BgRow {
            key,
            planes,
            palette_base,
            direct_palette: (bpp == 8 && self.regs[0x30] & 1 != 0).then_some(palette as u8),
            rank,
            flip: entry & 0x4000 != 0,
        }
    }
    fn bg_rank(&self, mode: u8, bg: usize, high: bool) -> u8 {
        match mode {
            0 => [[8, 11], [7, 10], [2, 5], [1, 4]][bg][usize::from(high)],
            1 => {
                if bg == 2 && high && self.regs[5] & 8 != 0 {
                    12
                } else {
                    [[7, 10], [6, 9], [1, 3], [0, 0]][bg][usize::from(high)]
                }
            }
            _ => [[3, 7], [1, 5], [0, 0], [0, 0]][bg][usize::from(high)],
        }
    }
    fn mode7(&self, bg: usize, x: usize, y: usize) -> Option<Pixel> {
        let sel = self.regs[0x1a];
        let x = if sel & 1 != 0 { 255 - x } else { x } as i32;
        // The scheduler maps visible scanlines 1..=224 to framebuffer rows
        // 0..=223. Fullsnes SCREEN.Y uses the scanline, including before VFLIP.
        let y = y + 1;
        let y = if sel & 2 != 0 { 255 - y } else { y } as i32;
        let sx = ((self.m7_scroll[0] as i32) << 19) >> 19;
        let sy = ((self.m7_scroll[1] as i32) << 19) >> 19;
        let cx = ((self.m7[4] as i32) << 19) >> 19;
        let cy = ((self.m7[5] as i32) << 19) >> 19;
        // Anomie $211F/$2120 and Fullsnes Rotation/Scaling: select the
        // displacement's low ten bits and bit-13 sign, then discard six
        // fractional bits from each origin product separately. Horizontal
        // per-pixel A/C products retain all eight fractional bits.
        let clip = |d: i32| (d & 1023) - if d & 8192 != 0 { 1024 } else { 0 };
        let dh = clip(sx - cx);
        let dv = clip(sy - cy);
        let [a, b, c, d] = [self.m7[0], self.m7[1], self.m7[2], self.m7[3]].map(i32::from);
        let x0 = ((a * dh) & !63) + ((b * dv) & !63) + ((b * y) & !63) + cx * 256;
        let y0 = ((c * dh) & !63) + ((d * dv) & !63) + ((d * y) & !63) + cy * 256;
        let px = (x0 + a * x) >> 8;
        let py = (y0 + c * x) >> 8;
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
            })
        }
    }
    // Window masks for screen columns [first, first + out.len()).
    fn fill_window(&self, layer: usize, out: &mut [bool], first: usize) {
        let sel = (self.regs[0x23 + layer / 2] >> ((layer & 1) * 4)) & 15;
        // Column x is inside [left, right], XOR the inversion bit.
        let inside = |out: &mut [bool], left: u8, right: u8, invert: bool| {
            out.fill(invert);
            let start = usize::from(left).max(first);
            let end = (usize::from(right) + 1).min(first + out.len());
            if start < end {
                out[start - first..end - first].fill(!invert);
            }
        };
        let w1 = |out: &mut [bool]| inside(out, self.regs[0x26], self.regs[0x27], sel & 1 != 0);
        let w2 = |out: &mut [bool]| inside(out, self.regs[0x28], self.regs[0x29], sel & 4 != 0);
        match (sel & 2 != 0, sel & 8 != 0) {
            (false, false) => out.fill(false),
            (true, false) => w1(out),
            (false, true) => w2(out),
            (true, true) => {
                let mut second = [false; WIDTH];
                let second = &mut second[..out.len()];
                w1(out);
                w2(second);
                let pairs = out.iter_mut().zip(second.iter());
                match (self.regs[0x2a + layer / 4] >> ((layer & 3) * 2)) & 3 {
                    0 => pairs.for_each(|(o, &w)| *o |= w),
                    1 => pairs.for_each(|(o, &w)| *o &= w),
                    2 => pairs.for_each(|(o, &w)| *o ^= w),
                    _ => pairs.for_each(|(o, &w)| *o = !(*o ^ w)),
                }
            }
        }
    }
    // Evaluate and draw this line's objects into `obj_line`.
    fn objects(&mut self, y: usize) {
        self.obj_line.fill(0);
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
        if self.obj_oam != self.oam || self.obj_sizes != self.regs[1] >> 5 {
            self.obj_oam = self.oam;
            self.obj_sizes = self.regs[1] >> 5;
            self.obj_rows.fill(0);
            for i in 0..128 {
                let hi = (self.oam[512 + i / 4] >> ((i & 3) * 2)) & 3;
                let (width, height) = if hi & 2 != 0 { large } else { small };
                let sx = self.oam[i * 4] as i32 - if hi & 1 != 0 { 256 } else { 0 };
                // X=-256 participates at X=0 in both budgets, never in drawing.
                let evaluation_x = if sx == -256 { 0 } else { sx };
                if evaluation_x <= -(width as i32) || evaluation_x >= 256 {
                    continue;
                }
                let sy = self.oam[i * 4 + 1] as usize;
                for row in 0..height {
                    self.obj_rows[(sy + row) & 255] |= 1 << i;
                }
            }
        }
        let base = (self.regs[1] as usize & 7) * 16384;
        let gap = (((self.regs[1] >> 3) as usize & 3) + 1) * 8192;
        let start = if self.regs[3] & 128 != 0 {
            (self.oaddr / 4) & 127
        } else {
            0
        };
        let ranks = match self.regs[5] & 7 {
            0 => [3, 6, 9, 12],
            1 => [2, 4, 8, 11],
            _ => [2, 4, 6, 8],
        };
        let mut count = 0;
        let mut selected = [(0, 0, 0, 0, 0); 32];
        // Bit n of `pending` is object (n + start) & 127, in evaluation order.
        let mut pending = self.obj_rows[y & 255].rotate_right(start as u32);
        while pending != 0 {
            let i = (pending.trailing_zeros() as usize + start) & 127;
            pending &= pending - 1;
            let hi = (self.oam[512 + i / 4] >> ((i & 3) * 2)) & 3;
            let (width, _) = if hi & 2 != 0 { large } else { small };
            let row = (y + 256 - self.oam[i * 4 + 1] as usize) & 255;
            let sx = self.oam[i * 4] as i32 - if hi & 1 != 0 { 256 } else { 0 };
            let evaluation_x = if sx == -256 { 0 } else { sx };
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
            let hflip = attr & 64 != 0;
            let pal = ((attr >> 1) & 7) as usize;
            let tag =
                (ranks[((attr >> 4) & 3) as usize] << 24) | (u32::from(pal >= 4) << 23) | (4 << 16);
            let table = base + if attr & 1 != 0 { gap } else { 0 };
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
                // All eight columns of this group share one character.
                let tx = if hflip { width - 1 - column } else { column };
                let tile =
                    (name & 0xf0).wrapping_add((row / 8) * 16) & 255 | ((name + tx / 8) & 15);
                let address = table + tile * 32 + (row & 7) * 2;
                let planes = [0, 1, 16, 17].map(|offset| self.vram[(address + offset) & 65535]);
                for col in column..column + 8 {
                    let screen = sx + col as i32;
                    if !(0..256).contains(&screen) {
                        continue;
                    }
                    let tx = if hflip { width - 1 - col } else { col };
                    let bit = 7 - (tx & 7);
                    let color = planes
                        .iter()
                        .enumerate()
                        .fold(0, |c, (plane, &p)| c | (((p >> bit) & 1) << plane));
                    if color == 0 {
                        continue;
                    }
                    self.obj_line[screen as usize] =
                        tag | INDEXED | (128 + pal * 16 + color as usize) as u32;
                }
            }
        }
    }
    pub fn framebuffer_width(&self) -> usize {
        self.frame_width
    }
    pub fn framebuffer_height(&self) -> usize {
        HEIGHT
    }
    fn prepare_frame_width(&mut self, frame_start: bool, wide_output: bool) {
        let width = if wide_output { WIDTH * 2 } else { WIDTH };
        if frame_start && self.frame_width != width {
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
    pub(crate) fn raster_line_start(&mut self) {
        self.raster_x = 0;
        self.obj_line_valid = false;
        self.raster_overflow = 0;
    }
    /// PROVISIONAL / HARDWARE UNVERIFIED output convention (Issue #107):
    /// low-res pixel x occupies [88+4*x, 92+4*x), emitted at its right edge.
    /// A write committed at that edge affects x+1. HBlank is NOT the picture end.
    /// Hires emits both existing compositor phases at the same low-res edge.
    pub(crate) fn render_until(&mut self, y: usize, master_h: u64) {
        let end = (master_h.saturating_sub(88) / 4).min(WIDTH as u64) as usize;
        if y < HEIGHT && end > self.raster_x {
            self.render_span(y, self.raster_x, end);
            self.raster_x = end;
        }
        // Preserve the existing coarse STAT77 publication at HBlank. Early
        // composition needs OBJ pixels, but does not establish hardware flag timing.
        if master_h >= 1096 {
            self.obj_overflow |= self.raster_overflow;
        }
    }
    /// Whole-line convenience for static renderer tests and callers.
    pub fn render_line(&mut self, y: usize) {
        if y < HEIGHT {
            self.raster_line_start();
            self.render_span(y, 0, WIDTH);
            self.obj_overflow |= self.raster_overflow;
            self.raster_x = WIDTH;
        }
    }
    fn render_span(&mut self, y: usize, start: usize, end: usize) {
        let wide_output = self.wide_output();
        self.prepare_frame_width(y == 0 && start == 0, wide_output);
        if self.regs[0] & 128 != 0 {
            let scale = self.frame_width / WIDTH;
            for x in start * scale..end * scale {
                let i = (y * self.frame_width + x) * 4;
                self.frame[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
            return;
        }
        let fixed_color =
            self.fixed[0] as u16 | ((self.fixed[1] as u16) << 5) | ((self.fixed[2] as u16) << 10);
        // Reuse the existing scanline OBJ evaluation once per line. This is
        // deliberately not a dot-accurate OAM/fetch pipeline. BG/compositing
        // state is sampled only for the newly emitted span.
        if !self.obj_line_valid {
            let overflow = self.obj_overflow;
            self.objects(y);
            self.obj_line_valid = true;
            self.raster_overflow |= self.obj_overflow;
            self.obj_overflow = overflow;
        }
        let scale = if wide_output { 2 } else { 1 };
        let hires = self.bg_hires();
        let color_control = self.regs[0x30];
        let math_control = self.regs[0x31];
        let clip_mode = color_control >> 6;
        let math_window_mode = (color_control >> 4) & 3;
        let needs_color_window = matches!(clip_mode, 1 | 2) || matches!(math_window_mode, 1 | 2);
        let use_sub = color_control & 2 != 0;
        let needs_sub = wide_output || (use_sub && math_control & 0x3f != 0);
        let bright = u32::from(self.regs[0] & 15);
        let repeats = if !wide_output && self.frame_width == WIDTH * 2 {
            2
        } else {
            1
        };
        let back_color = self.palette(0);
        let enables = [self.regs[0x2c], self.regs[0x2d]];
        // A layer whose two windows are both disabled is never masked.
        let live_windows = (0..5).fold(0u8, |live, layer| {
            let sel = (self.regs[0x23 + layer / 2] >> ((layer & 1) * 4)) & 15;
            live | (u8::from(sel & 0xa != 0) << layer)
        });
        let masks = [self.regs[0x2e], self.regs[0x2f]].map(|mask| mask & live_windows);
        let screens = 1 + usize::from(needs_sub);
        let (used, windowed) = (0..screens).fold((0, 0), |(used, windowed), screen| {
            (
                used | enables[screen],
                windowed | (enables[screen] & masks[screen]),
            )
        });
        // A span has no intervening PPU access that can alter the sampled
        // tile data, palette selection, scroll, or background mode, so each
        // layer and window is evaluated once per sample before compositing.
        let mut line = self.line.take().expect("line buffers");
        // Mode 5/6 BGs use distinct character phases (main odd, sub even).
        // SETINI pseudo-hires keeps both screens at the same full dot.
        let samples = if hires { 2 } else { 1 };
        let count = (end - start) * samples;
        // A Mode 5 BG shown on only one screen needs only that phase; keep
        // it compact (one sample per dot) instead of interleaved.
        let mut compact = 0u8;
        for bg in 0..4 {
            if used & (1 << bg) == 0 {
                continue;
            }
            let phases = (0..screens).fold(0, |phases, screen| {
                phases | (u8::from(enables[screen] & (1 << bg) != 0) << usize::from(screen == 0))
            });
            if self.regs[5] & 7 == 5 && self.regs[6] & (1 << bg) == 0 && phases != 3 {
                compact |= 1 << bg;
                let phase = usize::from(phases == 2);
                self.fill_bg(bg, y, start * 2 + phase, 2, &mut line.bg[bg][..end - start]);
            } else {
                self.fill_bg(bg, y, start * samples, 1, &mut line.bg[bg][..count]);
            }
        }
        for layer in 0..6 {
            if (layer < 5 && windowed & (1 << layer) != 0) || (layer == 5 && needs_color_window) {
                self.fill_window(layer, &mut line.win[layer][start..end], start);
            }
        }
        if self.levels_for != self.regs[0] & 15 {
            self.levels_for = self.regs[0] & 15;
            self.levels =
                std::array::from_fn(|value| (value as u32 * 255 * bright / (31 * 15)) as u8);
            self.rgba_cgram = self.cgram;
            for (i, entry) in self.rgba_cache.iter_mut().enumerate() {
                let color = u16::from_le_bytes([self.cgram[i * 2], self.cgram[i * 2 + 1]]);
                *entry = u32::from_le_bytes(rgba(&self.levels, color & 0x7fff));
            }
        } else {
            for (chunk, (current, cached)) in self
                .cgram
                .as_chunks::<16>()
                .0
                .iter()
                .zip(self.rgba_cgram.as_chunks_mut::<16>().0.iter_mut())
                .enumerate()
            {
                if current != cached {
                    cached.copy_from_slice(current);
                    for (k, pair) in current.as_chunks::<2>().0.iter().enumerate() {
                        let color = u16::from_le_bytes([pair[0], pair[1]]) & 0x7fff;
                        self.rgba_cache[chunk * 8 + k] =
                            u32::from_le_bytes(rgba(&self.levels, color));
                    }
                }
            }
        }
        let levels = self.levels;
        let n = end - start;
        let LineBuffers { bg, win, screen } = &mut *line;
        // Keep COLDATA as the subscreen math backdrop, retaining the layer
        // marker so the color-math result is not halved there. Layers are
        // painted in BG1..BG4, OBJ order; only a strictly higher rank wins.
        for (index, pixels) in screen.iter_mut().enumerate() {
            let pixels = &mut pixels[..n];
            // The main backdrop is CGRAM[0]; the subscreen's is COLDATA.
            pixels.fill(if index == 0 {
                BACKDROP | INDEXED
            } else {
                BACKDROP | u32::from(fixed_color)
            });
            if index >= screens {
                continue;
            }
            let enable = enables[index];
            let phase = usize::from(hires && index == 0);
            for layer in 0..5 {
                if enable & (1 << layer) == 0 {
                    continue;
                }
                let contiguous = layer == 4 || samples == 1 || compact & (1 << layer) != 0;
                let source = if layer == 4 {
                    &self.obj_line[start..end]
                } else if contiguous {
                    &bg[layer][..n]
                } else {
                    &bg[layer][..count]
                };
                let windowed = masks[index] & (1 << layer) != 0;
                let win = &win[layer][start..end];
                // Branch-free selects so these passes vectorize.
                if contiguous {
                    if windowed {
                        for ((c, &p), &w) in pixels.iter_mut().zip(source).zip(win) {
                            *c = if !w & (p >> 24 > *c >> 24) { p } else { *c };
                        }
                    } else {
                        for (c, &p) in pixels.iter_mut().zip(source) {
                            *c = if p >> 24 > *c >> 24 { p } else { *c };
                        }
                    }
                } else {
                    for (i, c) in pixels.iter_mut().enumerate() {
                        let p = source[i * 2 + phase];
                        if !(windowed && win[i]) && p >> 24 > *c >> 24 {
                            *c = p;
                        }
                    }
                }
            }
        }
        let blend = Blend {
            cgram: &self.cgram,
            rgba_cache: &self.rgba_cache,
            levels,
            back_color,
            fixed_color,
            math_control,
            use_sub,
            clip_mode,
            math_window_mode,
        };
        let frame = &mut self.frame;
        let row = y * self.frame_width;
        if !wide_output && repeats == 1 {
            let out = &mut frame[(row + start) * 4..(row + end) * 4];
            let pixels = screen[0][..n].iter().zip(&screen[1][..n]);
            if !needs_color_window {
                let (clip, prevent) = blend.windows(false);
                for (o, (&main_pixel, &sub_pixel)) in
                    out.as_chunks_mut::<4>().0.iter_mut().zip(pixels)
                {
                    *o = blend.output(main_pixel, sub_pixel, clip, prevent);
                }
            } else {
                let pixels = pixels.zip(&win[5][start..end]);
                for (o, ((&main_pixel, &sub_pixel), &in_window)) in
                    out.as_chunks_mut::<4>().0.iter_mut().zip(pixels)
                {
                    let (clip, prevent) = blend.windows(in_window);
                    *o = blend.output(main_pixel, sub_pixel, clip, prevent);
                }
            }
        } else if wide_output && repeats == 1 {
            let out = &mut frame[(row + start * 2) * 4..(row + end * 2) * 4];
            for (i, o) in out.as_chunks_mut::<8>().0.iter_mut().enumerate() {
                let (main_pixel, sub_pixel) = (screen[0][i], screen[1][i]);
                let (clip, prevent) = blend.windows(needs_color_window && win[5][start + i]);
                o[..4].copy_from_slice(&blend.rgba(blend.shown_sub(sub_pixel, clip)));
                o[4..].copy_from_slice(&blend.output(main_pixel, sub_pixel, clip, prevent));
            }
        } else {
            for (i, x) in (start..end).enumerate() {
                let main_pixel = screen[0][i];
                let sub_pixel = screen[1][i];
                let (clip, prevent) = blend.windows(needs_color_window && win[5][x]);
                for phase in 0..scale {
                    let color = if wide_output && phase == 0 {
                        blend.shown_sub(sub_pixel, clip)
                    } else {
                        blend.blend(main_pixel, sub_pixel, clip, prevent)
                    };
                    let out = blend.rgba(color);
                    let i = (row + (x * scale + phase) * repeats) * 4;
                    frame[i..i + 4].copy_from_slice(&out);
                    if repeats == 2 {
                        frame[i + 4..i + 8].copy_from_slice(&out);
                    }
                }
            }
        }
        self.line = Some(line);
    }
    // Fill packed BG samples first, first + step, ... (step 2 only in Mode 5).
    fn fill_bg(&self, bg: usize, y: usize, first: usize, step: usize, out: &mut [u32]) {
        let mode = self.regs[5] & 7;
        if mode == 7 {
            if bg == 0 || (bg == 1 && self.regs[0x33] & 0x40 != 0) {
                for (i, o) in out.iter_mut().enumerate() {
                    *o = pack(self.mode7(bg, first + i, y), bg);
                }
            } else {
                out.fill(0);
            }
            return;
        }
        let bpp = DEPTHS[mode as usize][bg];
        if bpp == 0 {
            out.fill(0);
            return;
        }
        if self.regs[6] & (1 << bg) != 0 {
            // Mosaic uses the general sampler.
            let mut row = None;
            for (i, o) in out.iter_mut().enumerate() {
                *o = pack(self.bg_pixel(bg, first + i * step, y, &mut row), bg);
            }
            return;
        }
        let layout = self.bg_layout(bg, mode, bpp);
        debug_assert!(step == 1 || mode == 5);
        if bg < 2 && matches!(mode, 2 | 4 | 6) {
            // Offset-per-tile: `bg_source` is constant within an offset
            // column and advances one source dot per sample, so resolve it
            // once per run that stays inside one column group and one tile.
            let hofs = self.scroll[bg][0] as usize;
            let scale = if mode == 6 { 2 } else { 1 };
            let mut i = 0;
            while i < out.len() {
                let x = first + i;
                let (sx, sy) = self.bg_source(bg, x, y);
                let q = x / scale + (hofs & 7);
                let group = (8 - (q & 7)) * scale - (x % scale);
                let run = group.min(8 - (sx & 7)).min(out.len() - i);
                let row = layout.map_row(sy);
                self.emit_tile_row::<1>(&layout, row, sx, sy, &mut out[i..i + run]);
                i += run;
            }
            return;
        }
        // Unmodified columns: one source row, advancing one dot per sample.
        let hofs = self.scroll[bg][0] as usize;
        let mut sx = (first + hofs * if self.bg_hires() { 2 } else { 1 }) & 1023;
        let sy = (y + 1 + self.scroll[bg][1] as usize) & 1023;
        let row = layout.map_row(sy);
        let mut i = 0;
        if step == 2 {
            while i < out.len() {
                let run = ((9 - (sx & 7)) / 2).min(out.len() - i);
                self.emit_tile_row::<2>(&layout, row, sx, sy, &mut out[i..i + run]);
                i += run;
                sx = (sx + run * 2) & 1023;
            }
            return;
        }
        while i < out.len() {
            let run = (8 - (sx & 7)).min(out.len() - i);
            self.emit_tile_row::<1>(&layout, row, sx, sy, &mut out[i..i + run]);
            i += run;
            sx = (sx + run) & 1023;
        }
    }
    // Per-span constants of the `bg_map_word` / `bg_row` addressing.
    fn bg_layout(&self, bg: usize, mode: u8, bpp: usize) -> BgLayout {
        let size = self.bg_tile_size(bg);
        let tile_width = self.bg_tile_width(bg);
        let sc = self.regs[7 + bg] as usize;
        BgLayout {
            bpp,
            size,
            tile_width,
            width_shift: tile_width.trailing_zeros(),
            size_shift: size.trailing_zeros(),
            wide_map: sc & 1 != 0,
            tall_map: sc & 2 != 0,
            map_base: (sc & 0xfc) << 9,
            char_base: ((self.regs[0x0b + bg / 2] >> ((bg & 1) * 4)) as usize & 15) * 8192,
            palette_offset: if mode == 0 { bg * 32 } else { 0 },
            direct: bpp == 8 && self.regs[0x30] & 1 != 0,
            tags: [false, true].map(|high| {
                (u32::from(self.bg_rank(mode, bg, high)) << 24) | MATH | ((bg as u32) << 16)
            }),
        }
    }
    // Pack samples from source (sx, sy), STEP dots apart, within one tile
    // row; `out` must not extend past the eight-dot boundary of sx.
    #[inline(always)]
    fn emit_tile_row<const STEP: usize>(
        &self,
        l: &BgLayout,
        row: usize,
        sx: usize,
        sy: usize,
        out: &mut [u32],
    ) {
        let tx = (sx >> l.width_shift) & if l.wide_map { 63 } else { 31 };
        let entry = self.vw(row + usize::from(tx >= 32) * 2048 + (tx & 31) * 2);
        let mut px = sx & (l.tile_width - 1);
        let mut py = sy & (l.size - 1);
        if entry & 0x4000 != 0 {
            px = l.tile_width - 1 - px;
        }
        if entry & 0x8000 != 0 {
            py = l.size - 1 - py;
        }
        let tile = ((entry as usize & 1023) + (px / 8) + (py / 8) * 16) & 1023;
        let address = l.char_base + tile * l.bpp * 8 + (py & 7) * 2;
        let mut planes = u64::from(self.vw(address));
        if l.bpp >= 4 {
            planes |= u64::from(self.vw(address + 16)) << 16;
        }
        if l.bpp == 8 {
            planes |= u64::from(self.vw(address + 32)) << 32;
            planes |= u64::from(self.vw(address + 48)) << 48;
        }
        let palette = ((entry >> 10) & 7) as usize;
        let palette_base = if l.bpp == 8 {
            0
        } else {
            palette * (1 << l.bpp) + l.palette_offset
        };
        let tag = l.tags[usize::from(entry & 0x2000 != 0)];
        // Eight color indices, one byte per column (column 0 first).
        let plane = |k: usize| SPREAD[(planes >> (k * 8)) as usize & 255] << k;
        let colors = match l.bpp {
            2 => plane(0) | plane(1),
            4 => plane(0) | plane(1) | plane(2) | plane(3),
            _ => transpose_planes(planes).swap_bytes(),
        };
        let colors = if entry & 0x4000 != 0 {
            colors.swap_bytes()
        } else {
            colors
        };
        let colors = colors >> ((sx & 7) * 8);
        if l.direct {
            for (k, o) in out.iter_mut().enumerate() {
                let color = (colors >> (k * STEP * 8)) as u8;
                *o = if color == 0 {
                    0
                } else {
                    tag | u32::from(Self::direct_color(color, palette as u8))
                };
            }
            return;
        }
        let tag = tag | INDEXED | palette_base as u32;
        if STEP == 1 {
            if let Ok(out) = <&mut [u32; 8]>::try_from(&mut *out) {
                // Whole aligned tile: a fixed-width loop that vectorizes.
                let bytes = colors.to_le_bytes();
                for (o, &color) in out.iter_mut().zip(&bytes) {
                    let color = u32::from(color);
                    *o = if color == 0 { 0 } else { tag + color };
                }
                return;
            }
        }
        for (k, o) in out.iter_mut().enumerate() {
            let color = u32::from((colors >> (k * STEP * 8)) as u8);
            *o = if color == 0 { 0 } else { tag + color };
        }
    }
}

impl BgLayout {
    // Screen-map address of source row sy, at column zero of its page.
    fn map_row(&self, sy: usize) -> usize {
        let ty = (sy >> self.size_shift) & if self.tall_map { 63 } else { 31 };
        self.map_base
            + usize::from(ty >= 32) * if self.wide_map { 4096 } else { 2048 }
            + (ty & 31) * 64
    }
}
struct BgLayout {
    bpp: usize,
    size: usize,
    tile_width: usize,
    width_shift: u32,
    size_shift: u32,
    wide_map: bool,
    tall_map: bool,
    map_base: usize,
    char_base: usize,
    palette_offset: usize,
    direct: bool,
    tags: [u32; 2],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_background_row_preserves_eight_planes_and_horizontal_flip() {
        let mut ppu = Ppu::default();
        ppu.write(5, 3); // Mode 3 BG1 is 8bpp.
        ppu.write(7, 0x20); // Map at $4000.
        for (x, index) in [1u8, 2, 4, 8, 16, 32, 64, 128].into_iter().enumerate() {
            for plane in 0..8 {
                ppu.vram[2 + (plane / 2) * 16 + plane % 2] |= ((index >> plane) & 1) << (7 - x);
            }
            ppu.cgram[index as usize * 2] = x as u8 + 1;
        }
        for flipped in [false, true] {
            ppu.vram[0x4001] = if flipped { 0x40 } else { 0 };
            let mut row = None;
            for x in 0..8 {
                let pixel = ppu.bg_pixel(0, x, 0, &mut row).unwrap();
                assert_eq!(pixel.color, if flipped { 8 - x } else { x + 1 } as u16);
            }
        }
    }

    #[test]
    fn plane_transpose_matches_per_plane_spread() {
        let mut x = 0x9e37_79b9_7f4a_7c15u64;
        for _ in 0..100_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let spread = (0..8).fold(0u64, |colors, plane| {
                colors | (SPREAD[(x >> (plane * 8)) as usize & 255] << plane)
            });
            assert_eq!(transpose_planes(x).swap_bytes(), spread, "{x:016x}");
        }
    }

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

use crate::snapshot::{self, Reader, Value};

snapshot::snapshot_members!(
    Ppu,
    regs,
    frame_width,
    raster_x,
    obj_line,
    obj_line_valid,
    obj_oam,
    obj_sizes,
    raster_overflow,
    vaddr,
    vbuffer,
    caddr,
    clatch,
    oaddr,
    oport_addr,
    olatch,
    scroll_latch,
    scroll,
    m7,
    m7_scroll,
    m7_latch,
    fixed,
    h,
    v,
    field,
    hflip,
    vflip,
    counters,
    counter_latched,
    latch_enabled,
    open1,
    open2,
    obj_overflow,
);

impl Ppu {
    pub(crate) fn write_snapshot(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.vram[..]);
        out.extend_from_slice(&self.cgram);
        out.extend_from_slice(&self.oam);
        self.write_members(out);
        // The power-on candidate cache represents no objects until the
        // first OAM/size change, so retain it as well as the latched OBJ line.
        self.obj_rows.write(out);
        out.extend_from_slice(&self.frame);
    }

    pub(crate) fn read_snapshot(&mut self, input: &mut Reader<'_>) -> snapshot::Result<()> {
        self.vram.copy_from_slice(input.take(0x10000)?);
        self.cgram.copy_from_slice(input.take(512)?);
        self.oam.copy_from_slice(input.take(544)?);
        self.read_members(input)?;
        if !matches!(self.frame_width, WIDTH | 512)
            || self.raster_x > WIDTH
            || self.obj_sizes > 7
            || self.caddr >= 512
            || self.oaddr >= 1024
            || self.oport_addr >= 1024
            || self.fixed.iter().any(|&value| value > 31)
            || self.v >= 262
            || u64::from(self.h) >= self.line_clocks()
            || self.counters[0] > 340
            || self.counters[1] >= 262
            || (self.raster_overflow | self.obj_overflow) & 0x3f != 0
        {
            return Err(snapshot::invalid());
        }
        *self.obj_rows = Value::read(input)?;
        self.frame = input.take(self.frame_width * HEIGHT * 4)?.to_vec();
        Ok(())
    }
}
