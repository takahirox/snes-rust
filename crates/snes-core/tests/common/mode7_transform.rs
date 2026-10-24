//! Original MIT 65C816 code, sparse map, character artwork and palette, Issue #63.
//! Copyright (c) 2026 snes-rust contributors. No external binary/code inputs.
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
pub fn guest_rom(clipped: bool, bg2: bool) -> Vec<u8> {
    let mut c = vec![0x78, 0xd8]; // SEI, CLD, reset's 8-bit accumulator
    write(&mut c, 0x2100, 128);
    for (r, v) in [
        (0x2105, 7),
        (0x2133, 0x40),
        (0x212c, if bg2 { 2 } else { 1 }),
        (0x212d, 0),
        (0x2130, 0),
        (0x2131, 0),
        (0x211a, 0),
    ] {
        write(&mut c, r, v);
    }
    for (index, color) in [
        (0, 0),
        (1, 0x001f),
        (2, 0x03e0),
        (129, 0x001f),
        (130, 0x03e0),
    ] {
        write(&mut c, 0x2121, index);
        pair(&mut c, 0x2122, color);
    }
    c.extend([0x18, 0xfb, 0xc2, 0x10]); // native mode, 16-bit X, 8-bit A
    for (r, v) in [(0x2115, 0), (0x2116, 0), (0x2117, 0)] {
        write(&mut c, r, v);
    }
    // Every one of the 16384 map entries selects green character 1.
    c.extend([0xa2, 0, 0x40, 0xa9, 1, 0x8d, 0x18, 0x21, 0xca, 0xd0, 0xfa]);
    // Only map cell (0,0) selects character 2, so column 512 cannot alias it.
    vram_address(&mut c, 0);
    write(&mut c, 0x2118, 2);
    write(&mut c, 0x2115, 128);
    vram_address(&mut c, 64);
    for _ in 0..128 {
        write(&mut c, 0x2119, 0x82);
    }
    vram_address(&mut c, 128);
    write(&mut c, 0x2119, 0x81); // source (0,0) red
    for (r, v) in [
        (0x211b, if clipped { 128 } else { 32 }),
        (0x211c, 0),
        (0x211d, 0),
        (0x211e, 0),
        (0x211f, 0),
        (0x2120, 0),
        (0x210d, if clipped { 1024 } else { 1 }),
        (0x210e, 0),
    ] {
        pair(&mut c, r, v);
    }
    write(&mut c, 0x2100, 15);
    write(&mut c, 0x30, 0x63);
    c.extend([0x80, 0xfe]);
    assert!(c.len() < 0x7fc0);
    let mut rom = vec![0; 32768];
    rom[..c.len()].copy_from_slice(&c);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"MODE7 TRANSFORM MIT  ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&0x8000u16.to_le_bytes());
    rom
}
