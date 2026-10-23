//! Original MIT 65C816 fixture generator for Issue #49.
//! Copyright (c) 2026 snes-rust contributors.
// The interactive variant selects hires with the A button, in VBlank after
// auto joypad completion. All graphics/register data go through CPU bus writes.
pub fn guest_rom(mode: u8, interactive: bool) -> Vec<u8> {
    let mut code = vec![0x78, 0xd8]; // SEI, CLD; emulation, 8-bit A.
    fn write(code: &mut Vec<u8>, address: u16, value: u8) {
        code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
    }
    for (r, v) in [
        (0x2100, 0x80),
        (0x4200, 1),
        (0x2105, if interactive { 1 } else { mode }),
        (0x2107, 0x60),
        (0x2108, 0x68),
        (0x2109, 0x70),
        (0x210b, 0x21),
        (0x212c, 1),
        (0x212d, if mode == 5 { 2 } else { 1 }),
        (0x2130, 0),
        (0x2131, 0),
        (0x2115, 0x80),
    ] {
        write(&mut code, r, v);
    }
    for reg in 0x210d..=0x2112 {
        write(&mut code, reg, 0);
        write(&mut code, reg, 0);
    }
    write(&mut code, 0x2121, 0);
    // black, red, green, white, blue, yellow, cyan, magenta.
    for color in [0u16, 0x001f, 0x03e0, 0x7fff, 0x7c00, 0x03ff, 0x7fe0, 0x7c1f] {
        for b in color.to_le_bytes() {
            write(&mut code, 0x2122, b);
        }
    }
    for (address, bpp, patterns) in [
        (
            0x1000u16,
            4,
            [[1, 2, 1, 2, 1, 2, 1, 2], [3, 4, 3, 4, 3, 4, 3, 4]],
        ),
        (
            0x2000,
            2,
            [[1, 2, 1, 2, 1, 2, 1, 2], [2, 3, 2, 3, 2, 3, 2, 3]],
        ),
    ] {
        write(&mut code, 0x2116, address as u8);
        write(&mut code, 0x2117, (address >> 8) as u8);
        for pattern in patterns {
            for pair in 0..bpp / 2 {
                for _ in 0..8 {
                    for plane in [pair * 2, pair * 2 + 1] {
                        let mut byte = 0;
                        for (x, color) in pattern.iter().enumerate() {
                            byte |= ((color >> plane) & 1) << (7 - x);
                        }
                        write(
                            &mut code,
                            if plane & 1 == 0 { 0x2118 } else { 0x2119 },
                            byte,
                        );
                    }
                }
            }
        }
    }
    // Repeat cell 0 (characters 0,1 in hires). BG2 palette 1.
    for (address, entry) in [(0x6000u16, 0u16), (0x6800, 0x0400)] {
        write(&mut code, 0x2116, address as u8);
        write(&mut code, 0x2117, (address >> 8) as u8);
        for _ in 0..1024 {
            write(&mut code, 0x2118, entry as u8);
            write(&mut code, 0x2119, (entry >> 8) as u8);
        }
    }
    write(&mut code, 0x0030, 0x49); // Initialization completion marker.
    write(&mut code, 0x2100, 15);
    if interactive {
        let loop_pc = 0x8000 + code.len() as u16;
        // Leave VBlank, wait for next VBlank, wait for auto joypad completion.
        code.extend([0xad, 0x12, 0x42, 0x29, 0x80, 0xd0, 0xf9]);
        code.extend([0xad, 0x12, 0x42, 0x29, 0x80, 0xf0, 0xf9]);
        code.extend([0xad, 0x12, 0x42, 0x29, 1, 0xd0, 0xf9]);
        code.extend([
            0xad, 0x18, 0x42, 0x29, 0x80, 0xf0, 4, 0xa9, mode, 0x80, 2, 0xa9, 1,
        ]);
        code.extend([
            0x8d,
            5,
            0x21,
            0x8d,
            0x31,
            0,
            0x4c,
            loop_pc as u8,
            (loop_pc >> 8) as u8,
        ]);
    } else {
        write(&mut code, 0x0031, mode);
        code.extend([0x80, 0xfe]);
    }
    assert!(code.len() < 0x7fc0);
    let mut rom = vec![0; 32768];
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"HIRES ORIGINAL MIT   ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&0x8000u16.to_le_bytes());
    rom
}
