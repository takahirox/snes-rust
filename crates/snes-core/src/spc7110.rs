//! SPC7110 data decompression, data port, arithmetic and data-ROM bank unit
//! (no RTC). Behavior follows the ISC-licensed ares implementation
//! (sfc/coprocessor/spc7110, decompressor by neviksti and talarubi); work
//! that ares runs a few chip clocks after the starting write completes
//! immediately here.
//!
//! Portions ported from ares: Copyright (c) 2004-2025 ares team, Near et al.
//! Permission to use, copy, modify, and/or distribute this software for any
//! purpose with or without fee is hereby granted, provided that the above
//! copyright notice and this permission notice appear in all copies.
//! THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
//! WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
//! MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
//! ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
//! WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
//! ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
//! OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
use crate::snapshot;

/// The first 1 MiB of an SPC7110 image is program ROM; the rest is data ROM.
pub(crate) const PROGRAM_SIZE: usize = 0x100000;

#[derive(Clone)]
pub(crate) struct Spc7110 {
    // $4800-$483f, indexed by the low six address bits.
    r: [u8; 64],
    dcu_mode: u8,
    dcu_address: u32,
    dcu_offset: u8,
    dcu_tile: [u8; 32],
    decompressor: Decompressor,
}

impl Spc7110 {
    pub(crate) fn power() -> Self {
        let mut r = [0; 64];
        r[0x32] = 1;
        r[0x33] = 2;
        Self {
            r,
            dcu_mode: 0,
            dcu_address: 0,
            dcu_offset: 0,
            dcu_tile: [0; 32],
            decompressor: Decompressor::default(),
        }
    }

    /// The register selected by an address in $00-3f/$80-bf:4800-483f or banks $50/$58.
    pub(crate) fn register(a: u32) -> Option<usize> {
        let bank = a >> 16;
        match bank {
            0x50 => Some(0x00),
            0x58 => Some(0x08),
            _ if bank & 0x40 == 0 && (0x4800..0x4840).contains(&(a & 0xffff)) => {
                Some(a as usize & 0x3f)
            }
            _ => None,
        }
    }

    pub(crate) fn sram_enabled(&self) -> bool {
        self.r[0x30] & 0x80 != 0
    }

    /// Data at a linear address in $00-3f:8000-ffff / $c0-ff:0000-ffff.
    pub(crate) fn rom_read(&self, linear: usize, prom: &[u8], drom: &[u8]) -> u8 {
        let offset = linear & 0x0fffff;
        let bank = match linear >> 20 {
            0 => return prom[crate::cartridge::mirror(offset, prom.len())],
            1 if self.r[0x34] & 4 != 0 => {
                return prom[crate::cartridge::mirror(0x100000 + offset, prom.len())]
            }
            n => self.r[0x30 + n] & 7,
        };
        data_rom(drom, self.r[0x34], (bank as usize) << 20 | offset)
    }

    /// A register read, with the side effects of the decompression and data ports.
    pub(crate) fn read(&mut self, index: usize, drom: &[u8]) -> Option<u8> {
        Some(match index {
            0x00 => {
                let counter = u16::from_le_bytes([self.r[0x09], self.r[0x0a]]).wrapping_sub(1);
                [self.r[0x09], self.r[0x0a]] = counter.to_le_bytes();
                self.dcu_read(drom)
            }
            0x08 => 0,
            0x01..=0x0c | 0x11..=0x18 | 0x20..=0x34 => self.r[index],
            0x10 => {
                let v = self.r[0x10];
                self.increment_4810(drom);
                v
            }
            0x1a => {
                if self.r[0x18] >> 5 == 3 {
                    self.seek(drom);
                }
                0
            }
            _ => return None,
        })
    }

    pub(crate) fn write(&mut self, index: usize, v: u8, drom: &[u8]) {
        match index {
            0x01 | 0x02 | 0x05 | 0x07 | 0x09 | 0x0a | 0x11 | 0x12 | 0x16 | 0x17 => {
                self.r[index] = v
            }
            0x20..=0x24 | 0x26 => self.r[index] = v,
            0x03 => self.r[3] = v & 0x7f,
            0x04 => {
                self.r[4] = v;
                let table = u32::from_le_bytes([self.r[1], self.r[2], self.r[3], 0]);
                let address = table + ((v as u32) << 2);
                let byte = |i| data_rom(drom, self.r[0x34], (address + i) as usize & 0xffffff);
                self.dcu_mode = byte(0) & 3;
                self.dcu_address = u32::from_be_bytes([0, byte(1), byte(2), byte(3)]) & 0x7fffff;
            }
            0x06 => {
                self.r[6] = v;
                self.r[0x0c] &= 0x7f;
                self.dcu_begin(drom);
            }
            0x0b => self.r[0x0b] = v & 3,
            0x13 => {
                self.r[0x13] = v & 0x7f;
                self.data_port_read(drom);
            }
            0x14 => {
                self.r[0x14] = v;
                if self.r[0x18] >> 5 == 1 {
                    self.seek(drom);
                }
            }
            0x15 => {
                self.r[0x15] = v;
                if self.r[0x18] & 2 != 0 {
                    self.data_port_read(drom);
                }
                if self.r[0x18] >> 5 == 2 {
                    self.seek(drom);
                }
            }
            0x18 => {
                self.r[0x18] = v & 0x7f;
                self.data_port_read(drom);
            }
            0x25 => {
                self.r[0x25] = v;
                self.r[0x2f] |= 0x01;
                self.multiply();
            }
            0x27 => {
                self.r[0x27] = v;
                self.divide();
            }
            0x2e => self.r[0x2e] = v & 1,
            0x30 => self.r[0x30] = v & 0x87,
            0x31..=0x34 => self.r[index] = v & 7,
            _ => {}
        }
    }

    fn dcu_begin(&mut self, drom: &[u8]) {
        if self.dcu_mode == 3 {
            return;
        }
        let d = &mut self.decompressor;
        d.initialize(self.dcu_mode, self.dcu_address, drom, self.r[0x34]);
        let seek = if self.r[0x0b] & 2 != 0 {
            u16::from_le_bytes([self.r[0x05], self.r[0x06]])
        } else {
            0
        };
        for _ in 0..=seek {
            d.decode(drom, self.r[0x34]);
        }
        self.r[0x0c] |= 0x80;
        self.dcu_offset = 0;
    }

    fn dcu_read(&mut self, drom: &[u8]) -> u8 {
        if self.r[0x0c] & 0x80 == 0 {
            return 0;
        }
        let d = &mut self.decompressor;
        if self.dcu_offset == 0 {
            for row in 0..8 {
                let w = d.result.to_le_bytes();
                match d.bpp {
                    1 => self.dcu_tile[row] = w[0],
                    2 => self.dcu_tile[row * 2..row * 2 + 2].copy_from_slice(&w[..2]),
                    _ => {
                        self.dcu_tile[row * 2..row * 2 + 2].copy_from_slice(&w[..2]);
                        self.dcu_tile[row * 2 + 16..row * 2 + 18].copy_from_slice(&w[2..]);
                    }
                }
                let seek = if self.r[0x0b] & 1 != 0 {
                    self.r[0x07]
                } else {
                    1
                };
                for _ in 0..seek {
                    d.decode(drom, self.r[0x34]);
                }
            }
        }
        let v = self.dcu_tile[self.dcu_offset as usize];
        self.dcu_offset = (self.dcu_offset + 1) & (8 * d.bpp - 1);
        v
    }

    fn data_offset(&self) -> u32 {
        u32::from_le_bytes([self.r[0x11], self.r[0x12], self.r[0x13], 0])
    }
    fn set_data_offset(&mut self, a: u32) {
        [self.r[0x11], self.r[0x12], self.r[0x13], _] = a.to_le_bytes();
        self.r[0x13] &= 0x7f;
    }
    // The adjust value, sign-extended when $4818 bit 3 is set.
    fn data_adjust(&self) -> u32 {
        let adjust = u16::from_le_bytes([self.r[0x14], self.r[0x15]]);
        if self.r[0x18] & 8 != 0 {
            adjust as i16 as u32
        } else {
            adjust as u32
        }
    }

    fn data_port_read(&mut self, drom: &[u8]) {
        let adjust = if self.r[0x18] & 2 != 0 {
            self.data_adjust()
        } else {
            0
        };
        let address = self.data_offset().wrapping_add(adjust) & 0xffffff;
        self.r[0x10] = data_rom(drom, self.r[0x34], address as usize);
    }

    fn increment_4810(&mut self, drom: &[u8]) {
        let stride = if self.r[0x18] & 1 != 0 {
            u16::from_le_bytes([self.r[0x16], self.r[0x17]])
        } else {
            1
        };
        let stride = if self.r[0x18] & 4 != 0 {
            stride as i16 as u32
        } else {
            stride as u32
        };
        if self.r[0x18] & 16 == 0 {
            self.set_data_offset(self.data_offset().wrapping_add(stride));
        } else {
            let adjust = self.data_adjust().wrapping_add(stride) as u16;
            [self.r[0x14], self.r[0x15]] = adjust.to_le_bytes();
        }
        self.data_port_read(drom);
    }

    // Offset += adjust, as triggered by $4814, $4815 or $481a per $4818 bits 5-6.
    fn seek(&mut self, drom: &[u8]) {
        self.set_data_offset(self.data_offset().wrapping_add(self.data_adjust()));
        self.data_port_read(drom);
    }

    fn multiply(&mut self) {
        let r = &mut self.r;
        let a = u16::from_le_bytes([r[0x24], r[0x25]]);
        let b = u16::from_le_bytes([r[0x20], r[0x21]]);
        let product = if r[0x2e] & 1 != 0 {
            (a as i16 as i32 * b as i16 as i32) as u32
        } else {
            a as u32 * b as u32
        };
        r[0x28..0x2c].copy_from_slice(&product.to_le_bytes());
        r[0x2f] &= 0x7f;
    }

    fn divide(&mut self) {
        let r = &mut self.r;
        let dividend = u32::from_le_bytes([r[0x20], r[0x21], r[0x22], r[0x23]]);
        let divisor = u16::from_le_bytes([r[0x26], r[0x27]]);
        let (quotient, remainder) = if divisor == 0 {
            (0, dividend as u16)
        } else if r[0x2e] & 1 != 0 {
            let (n, d) = (dividend as i32, divisor as i16 as i32);
            (n.wrapping_div(d) as u32, n.wrapping_rem(d) as u16)
        } else {
            (
                dividend / divisor as u32,
                (dividend % divisor as u32) as u16,
            )
        };
        r[0x28..0x2c].copy_from_slice(&quotient.to_le_bytes());
        r[0x2c..0x2e].copy_from_slice(&remainder.to_le_bytes());
        r[0x2f] &= 0x7f;
    }

    pub(crate) fn valid(&self) -> bool {
        let d = &self.decompressor;
        self.r[0x03] < 0x80
            && self.r[0x13] < 0x80
            && self.dcu_mode < 4
            && self.dcu_address < 0x800000
            && matches!(d.bpp, 1 | 2 | 4)
            && self.dcu_offset < 8 * d.bpp
            && (1..=8).contains(&d.bits)
            && d.range <= 0x100
            && d.context
                .iter()
                .flatten()
                .all(|&(p, s)| (p as usize) < EVOLUTION.len() && s < 2)
    }
}

// Context-modeling arithmetic decoder for 1, 2 and 4 bits per pixel.
#[derive(Clone, Copy)]
struct Decompressor {
    // (prediction state, MPS/LPS swap) per context set and bit history.
    context: [[(u8, u8); 15]; 5],
    bpp: u8,
    offset: u32,
    bits: u8,
    range: u16,
    input: u16,
    output: u8,
    pixels: u64,
    colormap: u64,
    result: u32,
}

impl Default for Decompressor {
    fn default() -> Self {
        Self {
            context: [[(0, 0); 15]; 5],
            bpp: 1,
            offset: 0,
            bits: 8,
            range: 0x100,
            input: 0,
            output: 0,
            pixels: 0,
            colormap: 0,
            result: 0,
        }
    }
}

// (MPS probability, next state after MPS, next state after LPS).
#[rustfmt::skip]
const EVOLUTION: [(u8, u8, u8); 53] = [
    (0x5a, 1, 1), (0x25, 2, 6), (0x11, 3, 8),
    (0x08, 4, 10), (0x03, 5, 12), (0x01, 5, 15),
    (0x5a, 7, 7), (0x3f, 8, 19), (0x2c, 9, 21),
    (0x20, 10, 22), (0x17, 11, 23), (0x11, 12, 25),
    (0x0c, 13, 26), (0x09, 14, 28), (0x07, 15, 29),
    (0x05, 16, 31), (0x04, 17, 32), (0x03, 18, 34),
    (0x02, 5, 35),
    (0x5a, 20, 20), (0x48, 21, 39), (0x3a, 22, 40),
    (0x2e, 23, 42), (0x26, 24, 44), (0x1f, 25, 45),
    (0x19, 26, 46), (0x15, 27, 25), (0x11, 28, 26),
    (0x0e, 29, 26), (0x0b, 30, 27), (0x09, 31, 28),
    (0x08, 32, 29), (0x07, 33, 30), (0x05, 34, 31),
    (0x04, 35, 33), (0x04, 36, 33), (0x03, 37, 34),
    (0x02, 38, 35), (0x02, 5, 36),
    (0x58, 40, 39), (0x4d, 41, 47), (0x43, 42, 48),
    (0x3b, 43, 49), (0x34, 44, 50), (0x2e, 45, 51),
    (0x29, 46, 44), (0x25, 24, 45),
    (0x56, 48, 47), (0x4f, 49, 47), (0x47, 50, 48),
    (0x41, 51, 49), (0x3c, 52, 50), (0x37, 43, 51),
];

impl Decompressor {
    fn next_byte(&mut self, drom: &[u8], settings: u8) -> u8 {
        let v = data_rom(drom, settings, self.offset as usize & 0xffffff);
        self.offset = self.offset.wrapping_add(1);
        v
    }

    fn initialize(&mut self, mode: u8, origin: u32, drom: &[u8], settings: u8) {
        *self = Self {
            bpp: 1 << mode,
            offset: origin,
            colormap: 0xfedcba9876543210,
            ..Self::default()
        };
        let high = self.next_byte(drom, settings);
        let low = self.next_byte(drom, settings);
        self.input = u16::from_be_bytes([high, low]);
    }

    fn decode(&mut self, drom: &[u8], settings: u8) {
        let bpp = self.bpp as u32;
        for pixel in 0..8 {
            let mut map = self.colormap;
            let mut diff = 0;
            if bpp > 1 {
                let p = self.pixels;
                let (pa, pb, pc) = if bpp == 2 {
                    (p >> 2 & 3, p >> 14 & 3, p >> 16 & 3)
                } else {
                    (p & 15, p >> 28 & 15, p >> 32 & 15)
                };
                if pa != pb || pb != pc {
                    let matched = pa ^ pb ^ pc;
                    diff = if matched == pa {
                        1
                    } else if matched == pb {
                        2
                    } else if matched == pc {
                        3
                    } else {
                        4
                    };
                }
                self.colormap = move_to_front(self.colormap, pa);
                map = move_to_front(map, pc);
                map = move_to_front(map, pb);
                map = move_to_front(map, pa);
            }
            for plane in 0..bpp {
                let bit = if bpp > 1 {
                    1 << plane
                } else {
                    1 << (pixel & 3)
                };
                let history = (bit - 1) & self.output as u32;
                let set = match bpp {
                    1 => (pixel >= 4) as usize,
                    2 => diff,
                    _ if plane >= 2 && history <= 1 => diff,
                    _ => 0,
                };
                let slot = (bit + history - 1) as usize;
                let mut ctx = self.context[set][slot];
                let (probability, next_mps, next_lps) = EVOLUTION[ctx.0 as usize];
                let lps_offset = self.range.wrapping_sub(probability as u16) & 0xff;
                let lps = self.input as u32 >= (lps_offset as u32) << 8;
                self.output = self.output << 1 | (lps as u8 ^ ctx.1);
                if lps {
                    self.range -= lps_offset;
                    self.input = self.input.wrapping_sub(lps_offset << 8);
                } else {
                    self.range = lps_offset;
                }
                while self.range <= 0x7f {
                    ctx.0 = if lps { next_lps } else { next_mps };
                    self.range <<= 1;
                    self.input <<= 1;
                    self.bits -= 1;
                    if self.bits == 0 {
                        self.bits = 8;
                        let v = self.next_byte(drom, settings);
                        self.input = self.input.wrapping_add(v as u16);
                    }
                }
                if lps && probability > 0x55 {
                    ctx.1 ^= 1;
                }
                self.context[set][slot] = ctx;
            }
            let mut index = self.output as u32 & ((1 << bpp) - 1);
            if bpp == 1 {
                index ^= (self.pixels >> 15) as u32 & 1;
            }
            self.pixels = self.pixels << bpp | (map >> (4 * index) & 15);
        }
        self.result = match bpp {
            1 => self.pixels as u32,
            2 => deinterleave(self.pixels, 16),
            _ => deinterleave(deinterleave(self.pixels, 32) as u64, 32),
        };
    }
}

// A data-ROM byte, given the $4834 size setting (1, 2, 4 or 8 MiB).
fn data_rom(drom: &[u8], settings: u8, address: usize) -> u8 {
    let size = settings & 3;
    if size != 3 && address & 0x400000 != 0 {
        return 0;
    }
    drom[crate::cartridge::mirror(address & ((0x100000 << size) - 1), drom.len())]
}

// Inverse Morton transform of big-endian packed pixels: odd bits to the
// lower half, even bits to the upper half.
fn deinterleave(data: u64, bits: u32) -> u32 {
    let mut data = data & ((1u64 << bits) - 1);
    data = 0x5555555555555555 & (data << bits | data >> 1);
    data = 0x3333333333333333 & (data | data >> 1);
    data = 0x0f0f0f0f0f0f0f0f & (data | data >> 2);
    data = 0x00ff00ff00ff00ff & (data | data >> 4);
    data = 0x0000ffff0000ffff & (data | data >> 8);
    (data | data >> 16) as u32
}

// Move the first occurrence of `nibble` to the low four bits of the list.
fn move_to_front(list: u64, nibble: u64) -> u64 {
    for n in (0..64).step_by(4) {
        if list >> n & 15 == nibble {
            let above = !15u64 << n;
            return (list & above) + (list << 4 & !above) + nibble;
        }
    }
    list
}

snapshot::snapshot_fields!(
    Decompressor,
    context,
    bpp,
    offset,
    bits,
    range,
    input,
    output,
    pixels,
    colormap,
    result,
);
snapshot::snapshot_fields!(
    Spc7110,
    r,
    dcu_mode,
    dcu_address,
    dcu_offset,
    dcu_tile,
    decompressor,
);

#[cfg(test)]
mod tests {
    use crate::cartridge::{Cartridge, Mapping};

    // 1 MiB program ROM + 2 MiB data ROM with an SPC7110 header.
    fn image() -> Vec<u8> {
        let mut rom = vec![0; 0x300000];
        for (i, v) in rom.iter_mut().enumerate().skip(0x100000) {
            *v = (i >> 12) as u8 ^ i as u8;
        }
        rom[0xffc0..0xffd5].copy_from_slice(b"SPC7110 TEST         ");
        rom[0xffd5..0xffda].copy_from_slice(&[0x3a, 0xf5, 0x0c, 0x03, 0x00]);
        rom[0xffdc..0xffe0].copy_from_slice(&[0xff, 0xff, 0x00, 0x00]);
        rom[0xfffc..0xfffe].copy_from_slice(&[0x00, 0x80]);
        rom[0x8000] = 0x80; // BRA -2
        rom[0x8001] = 0xfe;
        rom[0x1234] = 0x5a;
        rom
    }
    fn cart() -> Cartridge {
        Cartridge::load(&image()).unwrap()
    }
    fn drom(i: usize) -> u8 {
        image()[0x100000 + i]
    }
    fn word(c: &mut Cartridge, a: u32) -> u32 {
        (0..4).fold(0, |v, i| v | (c.bus_read(a + i).unwrap() as u32) << (8 * i))
    }

    #[test]
    fn header_sram_gate_and_factory_marker() {
        let mut c = cart();
        assert_eq!(c.mapping, Mapping::Spc7110);
        assert!(c.battery);
        assert_eq!(c.sram.len(), 0x2000);
        assert_eq!(c.read(0x007ff0), Some(0));
        c.write(0x006000, 0x11);
        assert_eq!(c.sram[0], 0);
        c.write(0x804830, 0xff);
        assert_eq!(c.bus_read(0x004830), Some(0x87));
        assert_eq!(c.read(0x3f7ff0), Some(b'S'));
        c.write(0xa06000, 0x11);
        assert_eq!(c.read(0x006000), Some(0x11));
        let mut copy = image();
        copy[0xffd6] = 0xf9; // SPC7110 with RTC is not implemented.
        assert!(Cartridge::load(&copy).is_err());
        assert!(Cartridge::load(&copy[..0x100000]).is_err());
    }

    #[test]
    fn program_and_switchable_data_rom_banks() {
        let mut c = cart();
        assert_eq!(c.read(0xc01234), Some(0x5a));
        assert_eq!(c.read(0x801234), None);
        assert_eq!(c.read(0x409234), None);
        assert_eq!(c.read(0x00fffc), Some(0x00));
        assert_eq!(c.read(0xd00007), Some(drom(7)));
        assert_eq!(c.read(0xe00007), Some(drom(7))); // $4832 = 1, 1 MiB size
        c.write(0x004834, 1);
        assert_eq!(c.read(0xe00007), Some(drom(0x100007)));
        assert_eq!(c.read(0xa08007), Some(drom(0x108007)));
        c.write(0x004831, 1);
        assert_eq!(c.read(0xd00007), Some(drom(0x100007)));
        c.write(0x004833, 4); // Beyond 2 MiB without 8 MiB mode: open data.
        assert_eq!(c.read(0xf00007), Some(0));
        c.write(0x004834, 5); // Program ROM also in $d0-df.
        assert_eq!(c.read(0xd01234), Some(0x5a));
    }

    #[test]
    fn multiply_and_divide() {
        let mut c = cart();
        let set = |c: &mut Cartridge, base: u32, bytes: &[u8]| {
            for (i, &v) in bytes.iter().enumerate() {
                c.write(base + i as u32, v);
            }
        };
        set(&mut c, 0x4820, &[0xff, 0xff]);
        set(&mut c, 0x4824, &[0xff, 0xff]);
        assert_eq!(word(&mut c, 0x4828), 0xfffe0001);
        assert_eq!(c.bus_read(0x482f), Some(0x01));
        c.write(0x482e, 1);
        set(&mut c, 0x4820, &[0xfe, 0xff]);
        set(&mut c, 0x4824, &[0x03, 0x00]);
        assert_eq!(word(&mut c, 0x4828), (-6i32) as u32);
        set(&mut c, 0x4820, &[0xf9, 0xff, 0xff, 0xff]);
        set(&mut c, 0x4826, &[0x02, 0x00]);
        assert_eq!(word(&mut c, 0x4828), (-3i32) as u32);
        assert_eq!(c.bus_read(0x482c), Some(0xff));
        c.write(0x482e, 0);
        set(&mut c, 0x4820, &[0x10, 0x27, 0x00, 0x00]);
        set(&mut c, 0x4826, &[0x03, 0x00]);
        assert_eq!(word(&mut c, 0x4828), 3333);
        assert_eq!(c.bus_read(0x482c), Some(1));
        set(&mut c, 0x4826, &[0x00, 0x00]);
        assert_eq!(word(&mut c, 0x4828), 0);
        assert_eq!(c.bus_read(0x482c), Some(0x10));
        assert_eq!(c.bus_read(0x482f), Some(0x01));
    }

    #[test]
    fn data_port_offset_stride_and_adjust() {
        let mut c = cart();
        for (a, v) in [(0x4811, 0x10), (0x4812, 0x00), (0x4813, 0x01)] {
            c.write(a, v);
        }
        assert_eq!(c.bus_read(0x4810), Some(drom(0x10010)));
        assert_eq!(c.bus_read(0x4810), Some(drom(0x10011)));
        assert_eq!(c.bus_read(0x4811), Some(0x12));
        // Stride -2 applied to the offset after each read.
        for (a, v) in [(0x4816, 0xfe), (0x4817, 0xff), (0x4818, 0x05)] {
            c.write(a, v);
        }
        assert_eq!(c.bus_read(0x4810), Some(drom(0x10012)));
        assert_eq!(c.bus_read(0x4810), Some(drom(0x10010)));
        // Reads use offset + adjust; writing $4814 in mode 1 adds it to the offset.
        c.write(0x4818, 0x22);
        c.write(0x4815, 0);
        c.write(0x4814, 0x20);
        assert_eq!(c.bus_read(0x4811), Some(0x2e));
        assert_eq!(c.bus_read(0x4810), Some(drom(0x1004e)));
        assert_eq!(c.bus_read(0x481a), Some(0));
    }

    #[test]
    fn decompression_port() {
        let mut rom = image();
        // Table entry 2 at $000100: mode 1 (2 bpp), stream at $000200.
        rom[0x100108..0x10010c].copy_from_slice(&[1, 0x00, 0x02, 0x00]);
        rom[0x100200..0x100300].fill(0);
        rom[0x100100..0x100104].copy_from_slice(&[3, 0, 0, 0]);
        let mut c = Cartridge::load(&rom).unwrap();
        for (a, v) in [(0x4801, 0x00), (0x4802, 0x01), (0x4803, 0x00), (0x4804, 2)] {
            c.write(a, v);
        }
        assert_eq!(c.bus_read(0x480c), Some(0));
        c.write(0x4809, 0x20);
        c.write(0x480a, 0x00);
        c.write(0x4805, 0);
        c.write(0x4806, 0);
        assert_eq!(c.bus_read(0x480c), Some(0x80));
        // An all-zero stream decodes to colour 0 everywhere.
        for _ in 0..16 {
            assert_eq!(c.bus_read(0x504321), Some(0));
        }
        assert_eq!(c.bus_read(0x4809), Some(0x10));
        assert_eq!(c.bus_read(0x580000), Some(0));
        // Mode 3 is invalid: the unit stays busy and reads zero.
        c.write(0x4804, 0);
        assert_eq!(c.bus_read(0x804803), Some(0));
        c.write(0x4806, 0);
        assert_eq!(c.bus_read(0x480c), Some(0));
    }

    #[test]
    fn machine_snapshot_and_reset_restore_chip_state() {
        let mut m = crate::Machine::new(&image()).unwrap();
        m.bus.cart.write(0x4831, 3);
        m.run_frame().unwrap();
        let state = m.save_snapshot().unwrap();
        m.bus.cart.write(0x4831, 5);
        m.load_snapshot(&state).unwrap();
        assert_eq!(m.bus.cart.bus_read(0x4831), Some(3));
        m.reset();
        assert_eq!(m.bus.cart.bus_read(0x4831), Some(0));
        assert_eq!(m.bus.cart.bus_read(0x4833), Some(2));
        let fresh = m.restart_with_sram(None).unwrap();
        assert_eq!(&fresh.bus.cart.sram[0x1ff0..], b"SPC7110 CHECK OK");
    }
}
