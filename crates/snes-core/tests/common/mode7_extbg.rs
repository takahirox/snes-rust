//! Original MIT 65C816 program, graphics and palette for Issue #61.
//! Copyright (c) 2026 snes-rust contributors.
fn write(c: &mut Vec<u8>, address: u16, value: u8) {
    c.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
}
fn pair(c: &mut Vec<u8>, address: u16, value: u16) {
    for b in value.to_le_bytes() {
        write(c, address, b);
    }
}
pub fn guest_rom() -> Vec<u8> {
    let mut c = vec![0x78, 0xd8]; // SEI, CLD; 8-bit accumulator
    write(&mut c, 0x2100, 128);
    for (r, v) in [
        (0x2105, 7),
        (0x2133, 0x40),
        (0x212c, 3),
        (0x212d, 0),
        (0x2130, 1),
        (0x2131, 0),
        (0x2123, 0x82),
        (0x2126, 0),
        (0x2127, 2),
        (0x2128, 6),
        (0x2129, 7),
        (0x212e, 3),
    ] {
        write(&mut c, r, v);
    }
    // BG1 is masked at x=0..2, exposing low BG2; BG2 is masked at 6..7,
    // exposing BG1 direct color. $80 at x=3 is transparent only on BG2.
    for (i, color) in [
        (0u8, 0x4210u16),
        (1, 0x001f),
        (127, 0x03e0),
        (128, 0x7c00),
        (129, 0x03ff),
        (255, 0x7fe0),
    ] {
        write(&mut c, 0x2121, i);
        pair(&mut c, 0x2122, color);
    }
    // Clear the map with executed writes. Native mode, 16-bit X, 8-bit A.
    c.extend([0x18, 0xfb, 0xc2, 0x10]);
    for (r, v) in [(0x2115, 0), (0x2116, 0), (0x2117, 0)] {
        write(&mut c, r, v);
    }
    c.extend([0xa2, 0, 0x40, 0x9c, 0x18, 0x21, 0xca, 0xd0, 0xfa]);
    for row in 0..28 {
        let word = row * 128;
        write(&mut c, 0x2116, word as u8);
        write(&mut c, 0x2117, (word >> 8) as u8);
        for _ in 0..32 {
            write(&mut c, 0x2118, 1);
        }
    }
    write(&mut c, 0x2115, 128);
    write(&mut c, 0x2116, 64);
    write(&mut c, 0x2117, 0);
    for _ in 0..8 {
        for raw in [0x00, 0x01, 0x7f, 0x80, 0x81, 0xff, 0x01, 0xff] {
            write(&mut c, 0x2119, raw);
        }
    }
    write(&mut c, 0x211a, 0);
    for (r, v) in [
        (0x211b, 256),
        (0x211c, 0),
        (0x211d, 0),
        (0x211e, 256),
        (0x211f, 0),
        (0x2120, 0),
        (0x210d, 0),
        (0x210e, 0),
    ] {
        pair(&mut c, r, v);
    }
    write(&mut c, 0x2100, 15);
    write(&mut c, 0x30, 0x61); // completion means CPU initialization reached its end
    c.extend([0x80, 0xfe]);
    assert!(c.len() < 0x7fc0);
    let mut rom = vec![0; 32768];
    rom[..c.len()].copy_from_slice(&c);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"MODE7 EXTBG MIT      ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&0x8000u16.to_le_bytes());
    rom
}
