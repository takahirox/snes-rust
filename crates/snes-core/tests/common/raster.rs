//! Original MIT-licensed 65C816 raster fixture, Issue #107.
//! Copyright (c) 2026 snes-rust contributors. No imported code/assets.
//! Byte emitter is the auditable source: each instruction is annotated below.
pub fn guest_rom() -> Vec<u8> {
    fn write(code: &mut Vec<u8>, address: u16, value: u8) {
        code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]); // LDA #; STA abs
    }
    fn delay(code: &mut Vec<u8>, n: usize) {
        code.extend(std::iter::repeat_n(0xea, n)); // NOP: slow ROM fetch 8 + idle 6 clocks
    }
    let mut code = vec![0x78, 0xd8]; // SEI; CLD (reset emulation mode, 8-bit A/X)
    for (r, v) in [
        (0x2100, 0x80),
        (0x2105, 1),
        (0x212c, 0),
        (0x212d, 0),
        (0x2130, 0),
        (0x2131, 0x20),
        (0x2121, 0),
        (0x2122, 0),
        (0x2122, 0),
        // DMA fixed A address $00:0040 -> COLDATA, one byte ($9f = blue31).
        (0x0040, 0x9f),
        (0x4300, 8),
        (0x4301, 0x32),
        (0x4302, 0x40),
        (0x4303, 0),
        (0x4304, 0),
        (0x4306, 0),
        (0x0030, 0x6b),
    ] {
        write(&mut code, r, v);
    }
    let loop_pc = 0x8000 + code.len() as u16;
    // Poll HBlank assertion; reset next line's red/brightness during HBlank.
    code.extend([0xad, 0x12, 0x42, 0x29, 0x40, 0xf0, 0xf9]); // LDA; AND; BEQ -7
    for (r, v) in [(0x2100, 15), (0x2132, 0xe0), (0x2132, 0x3f), (0x4305, 1)] {
        write(&mut code, r, v);
    }
    code.extend([0xad, 0x12, 0x42, 0x29, 0x40, 0xd0, 0xf9]); // wait HBlank clear
    delay(&mut code, 12);
    write(&mut code, 0x2132, 0x20); // red off -> black stripe
    write(&mut code, 0x2132, 0x5f); // green on
    delay(&mut code, 4);
    write(&mut code, 0x2100, 7); // dim green
    delay(&mut code, 4);
    write(&mut code, 0x2100, 0x80); // forced blank
    delay(&mut code, 4);
    write(&mut code, 0x2100, 15); // green again
    write(&mut code, 0x2132, 0x40); // green off
    write(&mut code, 0x420b, 1); // DMA blue on at the existing timed B-bus commit
    code.extend([0x4c, loop_pc as u8, (loop_pc >> 8) as u8]); // JMP loop
    let mut rom = vec![0; 32768];
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"RASTER FOUNDATION MIT");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&0x8000u16.to_le_bytes());
    rom
}
