//! Original MIT 65C816 program, OAM scenes and planar graphics for Issue #57.
//! Copyright (c) 2026 snes-rust contributors.
fn write(c: &mut Vec<u8>, a: u16, v: u8) {
    c.extend([0xa9, v, 0x8d, a as u8, (a >> 8) as u8]);
}
fn read(c: &mut Vec<u8>, a: u16) {
    c.extend([0xad, a as u8, (a >> 8) as u8]);
}
fn vblank(c: &mut Vec<u8>, entering: bool) {
    c.extend([
        0xad,
        0x12,
        0x42,
        0x29,
        0x80,
        if entering { 0xf0 } else { 0xd0 },
        0xf9,
    ]);
}
fn irq(c: &mut Vec<u8>, line: u8) {
    write(c, 0x4200, 0);
    read(c, 0x4211);
    for (a, v) in [
        (0x4207, 44),
        (0x4208, 1),
        (0x4209, line),
        (0x420a, 0),
        (0x4200, 0x30),
    ] {
        write(c, a, v);
    }
    c.push(0xcb); // SEI + WAI: masked IRQ wakes without vectoring, H=300 dots.
    read(c, 0x4211);
    write(c, 0x4200, 0);
}
fn sample(c: &mut Vec<u8>, result: u16, expected: u8) {
    read(c, 0x213e);
    c.extend([
        0x29,
        0xc0,
        0x8d,
        result as u8,
        (result >> 8) as u8,
        0xc9,
        expected,
        0xf0,
        5,
    ]);
    write(c, 0x31, 0); // A failed independent literal comparison is sticky.
}
fn oam(c: &mut Vec<u8>, mode: Option<u8>) {
    write(c, 0x2102, 0);
    write(c, 0x2103, 0);
    let mut table = [0u8; 544];
    for n in 0..128 {
        let (x, y, name, large) = match mode {
            Some(0) if n == 0 => (24, 24, 0, false),
            Some(1) if n < 31 => (0, 24, 15, false),
            Some(1) if n == 31 => (200, 24, 1, false),
            Some(1) if n == 32 => (216, 24, 2, false),
            Some(2) if n == 0 => (24, 24, 2, false),
            Some(2) if n == 1 => (64, 24, 0, true),
            Some(2) if n < 18 => (128, 24, 0, true),
            Some(3) if n == 0 => (24, 24, 0, true),
            Some(3) if n == 31 => (200, 24, 0, true),
            Some(3) if n == 32 => (232, 24, 2, true),
            Some(3) if n < 31 => (64, 24, 0, true),
            _ => (128, 100, 15, false),
        };
        table[n * 4..n * 4 + 4].copy_from_slice(&[x, y, name, 0]);
        // Unused objects live at X=-128, ineligible even when vertically present.
        let offscreen = y == 100;
        table[512 + n / 4] |= (u8::from(offscreen) | (u8::from(large) << 1)) << ((n % 4) * 2);
    }
    for b in table {
        write(c, 0x2104, b);
    }
}
pub fn guest_rom(mode: u8) -> Vec<u8> {
    assert!(mode < 4);
    let flags = [0, 0x40, 0x80, 0xc0][mode as usize];
    let mut c = vec![0x78, 0xd8]; // SEI, CLD; eight-bit accumulator/index.
    for (a, v) in [
        (0x2100, 128),
        (0x4200, 0),
        (0x420c, 0),
        (0x2101, 0),
        (0x2105, 0),
        (0x212c, 16),
        (0x212d, 0),
        (0x212e, 0),
        (0x212f, 0),
        (0x2130, 0),
        (0x2131, 0),
        (0x2133, 0),
        (0x2115, 128),
        (0x2116, 0),
        (0x2117, 0),
        (0x31, 1),
    ] {
        write(&mut c, a, v);
    }
    // Red tile 0, green tile 1, blue tile 2, transparent through tile 15.
    for tile in 0..16 {
        let color = [1u8, 2, 3].get(tile).copied().unwrap_or(0);
        for plane_pair in 0..2 {
            for _y in 0..8 {
                for lane in 0..2 {
                    write(
                        &mut c,
                        0x2118 + lane,
                        if color & (1 << (plane_pair * 2 + lane)) != 0 {
                            255
                        } else {
                            0
                        },
                    );
                }
            }
        }
    }
    write(&mut c, 0x2121, 0);
    for i in 0..144 {
        let color: u16 = match i {
            129 => 31,
            130 => 0x3e0,
            131 => 0x7c00,
            _ => 0,
        };
        for b in color.to_le_bytes() {
            write(&mut c, 0x2122, b);
        }
    }
    oam(&mut c, Some(mode));
    vblank(&mut c, true);
    write(&mut c, 0x2100, 15);
    vblank(&mut c, false);
    irq(&mut c, 30); // OAM Y24..31 has already overflowed at row 24.
    sample(&mut c, 0x1800, flags);
    sample(&mut c, 0x1801, flags);
    irq(&mut c, 50); // non-overflow line preserves flags
    sample(&mut c, 0x1802, flags);
    vblank(&mut c, true);
    sample(&mut c, 0x1803, flags);
    write(&mut c, 0x2100, 128);
    sample(&mut c, 0x1804, flags); // forcing blank doesn't clear
    oam(&mut c, None); // remove candidates using CPU register writes
    vblank(&mut c, false);
    sample(&mut c, 0x1805, flags); // frame reset inhibited by forced blank
    irq(&mut c, 10);
    write(&mut c, 0x2100, 15);
    sample(&mut c, 0x1806, flags); // disabling blank doesn't clear either
    vblank(&mut c, true);
    vblank(&mut c, false);
    sample(&mut c, 0x1807, 0); // V0 boundary, rendering enabled: clear
    irq(&mut c, 50);
    sample(&mut c, 0x1808, 0); // non-overflow rendering stays clear
    write(&mut c, 0x2100, 128);
    oam(&mut c, Some(mode));
    vblank(&mut c, true);
    write(&mut c, 0x2100, 15);
    write(&mut c, 0x32, mode);
    write(&mut c, 0x30, 0x57);
    c.extend([0x80, 0xfe]); // host observes, never patches; explicit host deadlines
    assert!(c.len() < 0x7fc0);
    let mut rom = vec![0; 32768];
    rom[..c.len()].copy_from_slice(&c);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"OBJ LIMITS MIT       ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&0x8000u16.to_le_bytes());
    rom
}
#[allow(dead_code)] // Fixture example only needs the cartridge builder.
pub fn vectors(mode: u8) -> Vec<(usize, [u8; 4])> {
    let black = [0, 0, 0, 255];
    let red = [255, 0, 0, 255];
    let green = [0, 255, 0, 255];
    match mode {
        0 => vec![(23, black), (24, red), (31, red), (32, black), (200, black)],
        1 => vec![
            (0, black),
            (200, green),
            (207, green),
            (208, black),
            (216, black),
        ],
        2 => vec![
            (24, black),
            (64, red),
            (71, red),
            (72, green),
            (79, green),
            (80, black),
            (128, red),
        ],
        3 => vec![
            (24, black),
            (64, red),
            (200, red),
            (207, red),
            (208, green),
            (215, green),
            (216, black),
            (232, black),
        ],
        _ => unreachable!(),
    }
}
