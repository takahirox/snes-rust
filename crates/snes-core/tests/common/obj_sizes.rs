//! Original MIT 65C816 instructions, graphics and cartridge data for Issue #55.
//! Copyright (c) 2026 snes-rust contributors.
pub fn guest_rom(selector: u8) -> Vec<u8> {
    assert!(selector == 6 || selector == 7);
    fn write(code: &mut Vec<u8>, address: u16, value: u8) {
        code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
    }
    let mut code = vec![0x78, 0xd8]; // SEI, CLD; emulation, eight-bit A.
    for (r, v) in [
        (0x2100, 0x80),
        (0x4200, 0),
        (0x420c, 0),
        (0x2101, selector << 5),
        (0x2105, 0),
        (0x212c, 16),
        (0x212d, 0),
        (0x212e, 0),
        (0x212f, 0),
        (0x2130, 0),
        (0x2131, 0),
        (0x2133, 0),
        (0x2115, 0x80),
    ] {
        write(&mut code, r, v);
    }
    write(&mut code, 0x2121, 0);
    for i in 0..136 {
        let color: u16 = match i {
            129 => 31,
            130 => 0x3e0,
            131 => 0x7c00,
            132 => 0x3ff,
            133 => 0x7fe0,
            134 => 0x7c1f,
            135 => 0x7fff,
            _ => 0,
        };
        for b in color.to_le_bytes() {
            write(&mut code, 0x2122, b);
        }
    }
    // CPU writes every planar byte of the original asymmetric 32x64 image.
    for ty in 0..8 {
        for tx in 0..4 {
            let address = (ty * 16 + tx) * 16; // VRAM word address.
            write(&mut code, 0x2116, address as u8);
            write(&mut code, 0x2117, (address >> 8) as u8);
            for pair in 0..2 {
                for y in 0..8 {
                    for plane in pair * 2..pair * 2 + 2 {
                        let mut byte = 0;
                        for x in 0..8 {
                            let sx = tx * 8 + x;
                            let sy = ty * 8 + y;
                            let c =
                                ((sx * 3 + sy * 5 + tx * 2 + ty * 3 + sy / 16 * 2) % 7 + 1) as u8;
                            byte |= ((c >> plane) & 1) << (7 - x);
                        }
                        write(
                            &mut code,
                            if plane % 2 == 0 { 0x2118 } else { 0x2119 },
                            byte,
                        );
                    }
                }
            }
        }
    }
    write(&mut code, 0x2102, 0);
    write(&mut code, 0x2103, 0);
    for n in 0..128 {
        let object = match n {
            0 => [24, 24, 0, 0],
            1 => [80, 24, 0, 0],
            2 => [136, 24, 0, 128],
            3 => [176, 24, 0, 128],
            _ => [0, 224, 0, 0],
        };
        for b in object {
            write(&mut code, 0x2104, b);
        }
    }
    for n in 0..32 {
        write(&mut code, 0x2104, if n == 0 { 0x88 } else { 0 });
    }
    write(&mut code, 0x30, 0x55); // Initialization completion.
    write(&mut code, 0x31, selector);
    write(&mut code, 0x2100, 15);
    code.extend([0x80, 0xfe]); // Bounded externally; never STP.
    assert!(code.len() < 0x7fc0);
    let mut rom = vec![0; 32768];
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"OBJ RECTANGLES MIT   ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&0x8000u16.to_le_bytes());
    rom
}
