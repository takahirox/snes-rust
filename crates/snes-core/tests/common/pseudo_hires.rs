//! Original MIT 65C816 code, graphics and data for Issue #53.
//! Copyright (c) 2026 snes-rust contributors.
// The host supplies only the cartridge and controller input. BGMODE remains 1.
pub fn guest_rom(enabled: bool, interactive: bool) -> Vec<u8> {
    fn write(code: &mut Vec<u8>, address: u16, value: u8) {
        code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
    }
    let mut code = vec![0x78, 0xd8]; // SEI, CLD; emulation, eight-bit A.
    for (r, v) in [
        (0x2100, 0x80),
        (0x4200, 1),
        (0x2105, 1),
        (0x2107, 0x60),
        (0x2108, 0x68),
        (0x210b, 0x21),
        (0x212c, 1),
        (0x212d, 2),
        (0x2130, 0),
        (0x2131, 0),
        (0x2132, 0xbf),
        (0x2133, if enabled { 8 } else { 0 }),
        (0x2115, 0x80),
    ] {
        write(&mut code, r, v);
    }
    for r in 0x210d..=0x2110 {
        write(&mut code, r, 0);
        write(&mut code, r, 0);
    }
    write(&mut code, 0x2121, 0);
    for i in 0..20 {
        let color: u16 = match i {
            0 => 0x7fe0,
            1 => 31,
            2 => 0x3e0,
            17 => 0x7c00,
            18 => 0x7fff,
            _ => 0,
        };
        for b in color.to_le_bytes() {
            write(&mut code, 0x2122, b);
        }
    }
    for address in [0x1000u16, 0x2000] {
        write(&mut code, 0x2116, address as u8);
        write(&mut code, 0x2117, (address >> 8) as u8);
        for pair in 0..2 {
            for _ in 0..8 {
                for plane in [pair * 2, pair * 2 + 1] {
                    let mut byte = 0;
                    for (x, color) in [1u8, 2, 0, 3, 1, 2, 1, 2].into_iter().enumerate() {
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
    for (address, entry) in [(0x6000u16, 0u16), (0x6800, 0x400)] {
        write(&mut code, 0x2116, address as u8);
        write(&mut code, 0x2117, (address >> 8) as u8);
        for _ in 0..1024 {
            write(&mut code, 0x2118, entry as u8);
            write(&mut code, 0x2119, (entry >> 8) as u8);
        }
    }
    write(&mut code, 0x30, 0x53); // Initialization completion.
    write(&mut code, 0x31, if enabled { 8 } else { 0 });
    write(&mut code, 0x2100, 15);
    if interactive {
        let loop_pc = 0x8000 + code.len() as u16;
        // Leave VBlank; enter next VBlank; wait for auto joypad completion.
        code.extend([0xad, 0x12, 0x42, 0x29, 0x80, 0xd0, 0xf9]);
        code.extend([0xad, 0x12, 0x42, 0x29, 0x80, 0xf0, 0xf9]);
        // Auto-read starts after VBlank: observe assertion before completion.
        code.extend([0xad, 0x12, 0x42, 0x29, 1, 0xf0, 0xf9]);
        code.extend([0xad, 0x12, 0x42, 0x29, 1, 0xd0, 0xf9]);
        code.extend([
            0xad, 0x18, 0x42, 0x29, 0x80, 0xf0, 4, 0xa9, 8, 0x80, 2, 0xa9, 0,
        ]);
        code.extend([
            0x8d,
            0x33,
            0x21,
            0x8d,
            0x31,
            0,
            0x4c,
            loop_pc as u8,
            (loop_pc >> 8) as u8,
        ]);
    } else {
        code.extend([0x80, 0xfe]);
    }
    assert!(code.len() < 0x7fc0);
    let mut rom = vec![0; 32768];
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"PSEUDO HIRES MIT     ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&0x8000u16.to_le_bytes());
    rom
}
