//! Original MIT CPU-executed CGRAM guest; no external assembler or assets.
//! Copyright (c) 2026 snes-rust contributors.
use snes_core::Machine;

fn write(code: &mut Vec<u8>, address: u16, value: u8) {
    code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
}
fn read(code: &mut Vec<u8>, address: u16) {
    code.extend([0xad, address as u8, (address >> 8) as u8]);
}
fn save(code: &mut Vec<u8>, port: u16, result: u16) {
    read(code, port);
    code.extend([0x8d, result as u8, (result >> 8) as u8]);
}
fn vblank(code: &mut Vec<u8>, entering: bool) {
    code.extend([
        0xad,
        0x12,
        0x42,
        0x29,
        0x80,
        if entering { 0xf0 } else { 0xd0 },
        0xf9,
    ]);
}
fn irq(code: &mut Vec<u8>, line: u8, dot: u16) {
    write(code, 0x4200, 0);
    read(code, 0x4211);
    for (a, v) in [
        (0x4207, dot as u8),
        (0x4208, (dot >> 8) as u8),
        (0x4209, line),
        (0x420a, 0),
        (0x4200, 0x30),
    ] {
        write(code, a, v);
    }
    code.push(0xcb); // Masked IRQ wakes WAI without vectoring.
    read(code, 0x4211);
    write(code, 0x4200, 0);
}
fn guest() -> Vec<u8> {
    let mut c = vec![0x78];
    write(&mut c, 0x2100, 128);
    for (color, bytes) in [(0, [0xa1, 0x32]), (4, [0x12, 0x34]), (5, [0x56, 0x78])] {
        write(&mut c, 0x2121, color);
        for byte in bytes {
            write(&mut c, 0x2122, byte);
        }
    }
    write(&mut c, 0x2121, 4);
    save(&mut c, 0x213b, 0x1800);
    save(&mut c, 0x213b, 0x1801);
    write(&mut c, 0x2100, 15);
    irq(&mut c, 100, 64);
    write(&mut c, 0x2121, 4);
    save(&mut c, 0x213b, 0x1802);
    save(&mut c, 0x213b, 0x1803);
    write(&mut c, 0x2121, 4);
    write(&mut c, 0x2122, 0x65);
    write(&mut c, 0x2122, 0xfe); // Policy destination color 0.
    write(&mut c, 0x2100, 128);
    write(&mut c, 0x2121, 4);
    save(&mut c, 0x213b, 0x1804);
    save(&mut c, 0x213b, 0x1805);
    write(&mut c, 0x2121, 0);
    save(&mut c, 0x213b, 0x1806);
    save(&mut c, 0x213b, 0x1807);
    write(&mut c, 0x2100, 15);
    irq(&mut c, 120, 278);
    write(&mut c, 0x2121, 5);
    write(&mut c, 0x2122, 0xde);
    write(&mut c, 0x2122, 0xad); // Legal HBlank palette pair.
    write(&mut c, 0x2100, 128);
    write(&mut c, 0x2121, 5);
    save(&mut c, 0x213b, 0x1808);
    save(&mut c, 0x213b, 0x1809);
    write(&mut c, 0x2100, 15);
    vblank(&mut c, true);
    write(&mut c, 0x2121, 4);
    save(&mut c, 0x213b, 0x180a);
    save(&mut c, 0x213b, 0x180b);
    write(&mut c, 0x2121, 4);
    vblank(&mut c, false);
    save(&mut c, 0x213b, 0x180c); // Scanline 0 active policy.
    save(&mut c, 0x213b, 0x180d);
    write(&mut c, 0x2100, 128);
    save(&mut c, 0x213b, 0x180e); // Software progression survived ownership.
    save(&mut c, 0x213b, 0x180f);
    write(&mut c, 0x2121, 0xff);
    write(&mut c, 0x2122, 0xd3);
    write(&mut c, 0x2121, 4); // Reset phase, retain latch (policy).
    read(&mut c, 0x213b);
    write(&mut c, 0x2122, 0xff); // Mixed read-low/write-high.
    write(&mut c, 0x2121, 4);
    save(&mut c, 0x213b, 0x1810);
    save(&mut c, 0x213b, 0x1811);
    write(&mut c, 0x18f0, 0xa5);
    c.push(0xdb);
    let mut rom = vec![0; 32768];
    assert!(c.len() < 0x7fc0);
    rom[..c.len()].copy_from_slice(&c);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"CGRAM ACCESS ORIGINAL");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&[0, 0x80]);
    rom
}

#[test]
fn cpu_guest_initializes_synchronizes_recovers_and_publishes_cgram_results() {
    let mut m = Machine::new(&guest()).unwrap();
    let mut completed = false;
    for instructions in 1..=150_000 {
        m.step();
        assert!(m.bus.frames < 4, "guest frame deadline");
        if m.cpu.stopped {
            assert_eq!(m.bus.peek(0x18f0), 0xa5);
            let results: Vec<_> = (0x1800..=0x1811).map(|a| m.bus.peek(a)).collect();
            // Restricted bytes and retention assert the documented approximation.
            // Ordinary blanked/HBlank transfers assert the supported controls.
            assert_eq!(
                results,
                [
                    0x12, 0x34, 0xa1, 0xb2, 0x12, 0x34, 0x65, 0x7e, 0xde, 0xad, 0x12, 0x34, 0x65,
                    0x7e, 0xde, 0xad, 0xd3, 0xff
                ]
            );
            eprintln!(
                "CGRAM guest completed in {instructions} instructions, {} frame wraps",
                m.bus.frames
            );
            completed = true;
            break;
        }
    }
    assert!(completed, "guest instruction deadline");
}
