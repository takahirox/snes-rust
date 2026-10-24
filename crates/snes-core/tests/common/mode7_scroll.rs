//! Original MIT 65C816 guest and hand-drawn stripes for Issue #59.
//! Copyright (c) 2026 snes-rust contributors.
// These are input artwork, never a computed pixel oracle. Unmarked columns
// are opaque black. The crucial source columns 21 and 69 are red and green.
pub const STRIPES: [(usize, u8); 24] = [
    (0, 1),
    (1, 2),
    (2, 3),
    (6, 4),
    (7, 5),
    (8, 6),
    (14, 7),
    (15, 1),
    (16, 2),
    (20, 3),
    (21, 1),
    (22, 3),
    (23, 4),
    (24, 5),
    (31, 6),
    (32, 7),
    (33, 1),
    (34, 2),
    (68, 3),
    (69, 2),
    (70, 5),
    (71, 6),
    (72, 7),
    (127, 4),
];
pub const PALETTE: [u16; 9] = [0, 31, 0x3e0, 0x7c00, 0x7fff, 0x3ff, 0x7fe0, 0x7c1f, 0];

fn write(c: &mut Vec<u8>, a: u16, v: u8) {
    c.extend([0xa9, v, 0x8d, a as u8, (a >> 8) as u8]);
}
fn pair(c: &mut Vec<u8>, a: u16, value: u16) {
    for b in value.to_le_bytes() {
        write(c, a, b);
    }
}
fn sample(c: &mut Vec<u8>, slot: &mut u16, bytes: [u8; 3]) {
    c.extend([0xea; 16]); // settle; no latency assertion
    for (r, expected) in (0x2134u16..=0x2136).zip(bytes) {
        c.extend([
            0xad,
            r as u8,
            (r >> 8) as u8,
            0x8d,
            *slot as u8,
            (*slot >> 8) as u8,
            0xc9,
            expected,
            0xf0,
            5,
        ]);
        write(c, 0x31, 0); // sticky failure
        *slot += 1;
    }
}
pub fn guest_rom() -> Vec<u8> {
    let mut c = vec![0x78, 0xd8]; // SEI, CLD; emulation, eight-bit accumulator
    write(&mut c, 0x2100, 128);
    write(&mut c, 0x2105, 1); // all capture cases run outside Mode 7
    write(&mut c, 0x31, 1);
    let mut slot = 0x1800;
    // Both offsets and all C/D/center registers feed the shared previous byte.
    for r in [0x210d, 0x210e, 0x211d, 0x211e, 0x211f, 0x2120] {
        pair(&mut c, 0x211c, 0x0100);
        write(&mut c, r, 0x34);
        write(&mut c, 0x211b, 0x12);
        sample(&mut c, &mut slot, [0x34, 0x12, 0]);
    }
    pair(&mut c, 0x211c, 0x0100);
    write(&mut c, 0x210d, 0x34);
    write(&mut c, 0x210e, 0x56);
    write(&mut c, 0x211b, 0x12);
    sample(&mut c, &mut slot, [0x56, 0x12, 0]);
    pair(&mut c, 0x211c, 0x0100);
    pair(&mut c, 0x211b, 0x9234); // signed 16-bit coefficient, not 13-bit
    sample(&mut c, &mut slot, [0x34, 0x92, 0xff]);
    for r in 0x210f..=0x2114 {
        pair(&mut c, 0x211c, 0x0100);
        write(&mut c, 0x2120, 0x34);
        write(&mut c, r, 0x77);
        write(&mut c, 0x211b, 0x12);
        sample(&mut c, &mut slot, [0x34, 0x12, 0]);
    }
    assert_eq!(slot, 0x182a);
    for (r, v) in [
        (0x212c, 1),
        (0x2130, 0),
        (0x2131, 0),
        (0x2133, 0),
        (0x211a, 0),
    ] {
        write(&mut c, r, v);
    }
    write(&mut c, 0x2121, 0);
    for color in PALETTE {
        for b in color.to_le_bytes() {
            write(&mut c, 0x2122, b);
        }
    }
    // Clear all map low bytes via actual CPU writes. X is 16 bits; A stays 8.
    c.extend([0x18, 0xfb, 0xc2, 0x10]); // CLC, XCE, REP #$10
    for (r, v) in [(0x2115, 0), (0x2116, 0), (0x2117, 0)] {
        write(&mut c, r, v);
    }
    c.extend([0xa2, 0, 0x40, 0x9c, 0x18, 0x21, 0xca, 0xd0, 0xfa]);
    // First 64 map columns over 32 rows cover both candidate source positions
    // and the full displayed width/height. Each row repeats a 128-pixel strip.
    for row in 0..32 {
        let address = row * 128;
        write(&mut c, 0x2116, address as u8);
        write(&mut c, 0x2117, (address >> 8) as u8);
        for col in 0..64 {
            write(&mut c, 0x2118, (col % 16 + 1) as u8);
        }
    }
    let mut strip = [8u8; 128];
    for (x, color) in STRIPES {
        strip[x] = color;
    }
    write(&mut c, 0x2115, 128); // increment on high port: character bytes
    write(&mut c, 0x2116, 64); // tile 1 starts at word 64
    write(&mut c, 0x2117, 0);
    for tile in 0..16 {
        for _ in 0..8 {
            for x in 0..8 {
                write(&mut c, 0x2119, strip[tile * 8 + x]);
            }
        }
    }
    for (r, value) in [
        (0x211b, 0x100),
        (0x211c, 0),
        (0x211d, 0),
        (0x211e, 0x100),
        (0x211f, 0),
        (0x2120, 0),
        (0x210e, 0),
    ] {
        pair(&mut c, r, value);
    }
    write(&mut c, 0x210d, 0x15);
    write(&mut c, 0x210f, 0x42);
    write(&mut c, 0x210d, 0); // M7HOFS=$0015, BG1HOFS=$0045
    write(&mut c, 0x2105, 7);
    write(&mut c, 0x2100, 15);
    write(&mut c, 0x30, 0x59); // sticky completion marker, never cleared in loop
    c.extend([0x80, 0xfe]);
    assert!(c.len() < 0x7fc0);
    let mut rom = vec![0; 32768];
    rom[..c.len()].copy_from_slice(&c);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"MODE7 SCROLL MIT     ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&0x8000u16.to_le_bytes());
    rom
}
