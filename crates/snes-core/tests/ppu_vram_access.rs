//! Original MIT CPU-executed VRAM access guests; no external assembler or assets.
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
fn address(code: &mut Vec<u8>, a: u16) {
    write(code, 0x2116, a as u8);
    write(code, 0x2117, (a >> 8) as u8);
}
fn vblank(code: &mut Vec<u8>, entering: bool) {
    // LDA HVBJOY; AND #$80; BEQ/BNE back to LDA. Actual guest bus polling.
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
    code.push(0xcb); // WAI wakes on IRQ despite SEI, without vectoring.
    read(code, 0x4211);
    write(code, 0x4200, 0);
}
fn guest(high: bool) -> Vec<u8> {
    let mut c = vec![0x78];
    write(&mut c, 0x2100, 0x80);
    write(&mut c, 0x2115, 0x80);
    for (a, word) in [(0, 0x3412u16), (1, 0x7856), (2, 0xbc9a), (0x101, 0xf0de)] {
        address(&mut c, a);
        write(&mut c, 0x2118, word as u8);
        write(&mut c, 0x2119, (word >> 8) as u8);
    }
    address(&mut c, 0);
    read(&mut c, 0x213a); // B0 A1, independently seeded under forced blank.
    write(&mut c, 0x2115, if high { 0x80 } else { 0 });
    let selected = if high { 0x213a } else { 0x2139 };
    let other = if high { 0x2139 } else { 0x213a };
    save(&mut c, other, 0x1800);
    write(&mut c, 0x2100, 15);
    vblank(&mut c, true);
    vblank(&mut c, false);
    irq(&mut c, 100, 64); // Stable active display, through hardware-visible IRQ.
    save(&mut c, other, 0x1801);
    save(&mut c, selected, 0x1802);
    save(&mut c, selected, 0x1803);
    address(&mut c, 0x101); // Both address writes are restricted.
    save(&mut c, other, 0x1804);
    save(&mut c, selected, 0x1805);
    write(&mut c, 0x2100, 0x80);
    address(&mut c, 1); // Forced-blank recovery to W1.
    save(&mut c, other, 0x1806);
    save(&mut c, selected, 0x1807); // A2, B1.
    write(&mut c, 0x2100, 15);
    irq(&mut c, 120, 280); // First two saved reads finish within HBlank.
    save(&mut c, other, 0x1808);
    save(&mut c, selected, 0x1809); // A3, B1.
    save(&mut c, selected, 0x180a); // A4, B1, still restricted.
    vblank(&mut c, true);
    address(&mut c, 2);
    save(&mut c, other, 0x180b);
    save(&mut c, selected, 0x180c); // A3, B2: duplicate first valid word.
    save(&mut c, selected, 0x180d); // A4, B3=0.
    save(&mut c, other, 0x180e);
    address(&mut c, 1);
    read(&mut c, selected); // A2, B1.
    vblank(&mut c, false); // Scanline zero is outside VBlank.
    save(&mut c, other, 0x180f);
    save(&mut c, selected, 0x1810); // A3, B1: blocked scanline-zero refill.
    save(&mut c, other, 0x1811);
    write(&mut c, 0x2100, 0x80);
    save(&mut c, selected, 0x1812); // A4, B3=0: proves restricted increment.
    save(&mut c, other, 0x1813);
    write(&mut c, 0x18f0, 0xa5);
    c.push(0xdb);
    let mut rom = vec![0; 32768];
    assert!(c.len() < 0x7fc0);
    rom[..c.len()].copy_from_slice(&c);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"VRAM ACCESS ORIGINAL ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&[0, 0x80]);
    rom
}
#[test]
fn cpu_guests_initialize_synchronize_and_publish_vram_access_results() {
    for high in [false, true] {
        let mut m = Machine::new(&guest(high)).unwrap();
        let mut completed = false;
        for _ in 0..150_000 {
            m.step();
            assert!(m.bus.frames < 4, "guest frame deadline");
            if m.cpu.stopped {
                assert_eq!(m.bus.peek(0x18f0), 0xa5);
                let results: Vec<_> = (0x1800..=0x1813).map(|a| m.bus.peek(a)).collect();
                let (s0, o0, s1, o1, s2, o2) = if high {
                    (0x34, 0x12, 0x78, 0x56, 0xbc, 0x9a)
                } else {
                    (0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc)
                };
                // Sentinels derived from the guest's literal words, independently
                // of production remapping, buffer or address helpers.
                assert_eq!(
                    results,
                    [
                        o0, o0, s0, s0, o0, s0, o1, s1, o1, s1, s1, o2, s2, s2, 0, o1, s1, o1, s1,
                        0
                    ],
                    "high={high}"
                );
                completed = true;
                break;
            }
        }
        assert!(completed, "guest instruction deadline");
    }
}
