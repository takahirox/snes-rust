//! Original MIT 65C816 program, sparse artwork and palette for Issue #65.
//! Copyright (c) 2026 snes-rust contributors. No external inputs.
fn write(c: &mut Vec<u8>, address: u16, value: u8) {
    c.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
}
fn pair(c: &mut Vec<u8>, address: u16, value: u16) {
    for b in value.to_le_bytes() {
        write(c, address, b);
    }
}
fn vram_address(c: &mut Vec<u8>, word: u16) {
    write(c, 0x2116, word as u8);
    write(c, 0x2117, (word >> 8) as u8);
}
pub fn guest_rom(flipped: bool, bg2: bool) -> Vec<u8> {
    let mut c = vec![0x78, 0xd8]; // SEI, CLD, reset's 8-bit accumulator
    write(&mut c, 0x2100, 128);
    for (r, v) in [
        (0x2105, 7),
        (0x2133, 0x40),
        (0x212c, if bg2 { 2 } else { 1 }),
        (0x212d, 0),
        (0x2130, 0),
        (0x2131, 0),
        (0x211a, if flipped { 2 } else { 0 }),
    ] {
        write(&mut c, r, v);
    }
    for (index, color) in [
        (0, 0),
        (1, 0x001f),
        (2, 0x03e0),
        (3, 0x7c00),
        (4, 0x03ff),
        (129, 0x001f),
        (130, 0x03e0),
        (131, 0x7c00),
        (132, 0x03ff),
    ] {
        write(&mut c, 0x2121, index);
        pair(&mut c, 0x2122, color);
    }
    c.extend([0x18, 0xfb, 0xc2, 0x10]); // native mode, 16-bit X, 8-bit A
    write(&mut c, 0x2115, 0);
    vram_address(&mut c, 0);
    // Every map cell selects green character 1, except six distinct cells.
    c.extend([0xa2, 0, 0x40, 0xa9, 1, 0x8d, 0x18, 0x21, 0xca, 0xd0, 0xfa]);
    // Literal source markers, independently derived in docs/issue-65-validation.md.
    // Normal: (8,1), (17,10), (231,224). VFLIP: (261,254), (252,245), (38,31).
    let markers = [
        (8u16, 1u16, 0x81),
        (17, 10, 0x83),
        (231, 224, 0x84),
        (261, 254, 0x81),
        (252, 245, 0x83),
        (38, 31, 0x84),
    ];
    for (i, &(x, y, _)) in markers.iter().enumerate() {
        vram_address(&mut c, (y / 8) * 128 + x / 8);
        write(&mut c, 0x2118, i as u8 + 2);
    }
    write(&mut c, 0x2115, 128);
    vram_address(&mut c, 64);
    for _ in 0..64 {
        write(&mut c, 0x2119, 0x82);
    }
    for (i, &(x, y, raw)) in markers.iter().enumerate() {
        vram_address(&mut c, (i as u16 + 2) * 64);
        for pixel in 0..64 {
            write(
                &mut c,
                0x2119,
                if pixel == (y & 7) * 8 + (x & 7) {
                    raw
                } else {
                    0x82
                },
            );
        }
    }
    // X = screen X + screen Y; Y = screen Y. Both B and D are nonzero.
    for (r, v) in [
        (0x211b, 256),
        (0x211c, 256),
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
    write(&mut c, 0x30, 0x65);
    c.extend([0x80, 0xfe]); // stable loop; host supplies no register/raster state
    assert!(c.len() < 0x7fc0);
    let mut rom = vec![0; 32768];
    rom[..c.len()].copy_from_slice(&c);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"MODE7 ROW ORIGIN MIT ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&0x8000u16.to_le_bytes());
    rom
}
