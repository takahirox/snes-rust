//! Original MIT CPU-executed OAM guest; no external assembler or assets.
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
fn address(code: &mut Vec<u8>, word: u16) {
    write(code, 0x2102, word as u8);
    write(code, 0x2103, (word >> 8) as u8);
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
    let mut c = vec![0x78]; // SEI; hardware-visible synchronization below.
    write(&mut c, 0x2100, 128);
    for (word, bytes) in [
        (4, [0x12, 0x34]),
        (5, [0x56, 0x78]),
        (0x100, [0xa1, 0xb2]),
        (0x101, [0xc3, 0xd4]),
    ] {
        address(&mut c, word);
        for byte in bytes {
            write(&mut c, 0x2104, byte);
        }
    }
    address(&mut c, 4);
    save(&mut c, 0x2138, 0x1800);
    save(&mut c, 0x2138, 0x1801);
    write(&mut c, 0x2100, 15);
    vblank(&mut c, true);
    vblank(&mut c, false);
    irq(&mut c, 100, 64);
    address(&mut c, 4);
    save(&mut c, 0x2138, 0x1802); // Policy high-even byte, not low sentinel.
    write(&mut c, 0x2104, 0x87); // Policy high-odd commit, latch stays C3.
    write(&mut c, 0x2104, 0x65); // Policy high-even commit AND latch capture.
    save(&mut c, 0x2138, 0x1803); // Policy high-odd 87.
    write(&mut c, 0x2100, 128);
    save(&mut c, 0x2138, 0x1804); // Preserved internal byte 514, C3.
    address(&mut c, 4);
    save(&mut c, 0x2138, 0x1805); // Low sentinel still 12.
    write(&mut c, 0x2104, 0x9a); // Latch 65 commits to low-even, 9A to odd.
    address(&mut c, 4);
    save(&mut c, 0x2138, 0x1806);
    save(&mut c, 0x2138, 0x1807);
    write(&mut c, 0x2100, 15);
    irq(&mut c, 120, 280);
    address(&mut c, 5);
    write(&mut c, 0x2104, 0xde);
    write(&mut c, 0x2104, 0xad); // HBlank still rendering-owned, under policy.
    write(&mut c, 0x2100, 128);
    address(&mut c, 5);
    save(&mut c, 0x2138, 0x1808);
    save(&mut c, 0x2138, 0x1809); // Software pair survives HBlank accesses.
    address(&mut c, 0x100);
    save(&mut c, 0x2138, 0x180a);
    save(&mut c, 0x2138, 0x180b); // Explicit policy corruption observation.
    address(&mut c, 4);
    read(&mut c, 0x2138); // Odd phase, latch DE retained.
    write(&mut c, 0x2100, 15);
    vblank(&mut c, true);
    save(&mut c, 0x2138, 0x180c); // Automatic normal VBlank reload, even.
    save(&mut c, 0x2138, 0x180d);
    address(&mut c, 4);
    save(&mut c, 0x2138, 0x180e); // Leave odd phase for frame wrap.
    vblank(&mut c, false);
    save(&mut c, 0x2138, 0x180f); // Scanline 0 policy high-odd AD.
    write(&mut c, 0x2100, 128);
    save(&mut c, 0x2138, 0x1810); // Internal address retained into forced blank.
    address(&mut c, 5);
    save(&mut c, 0x2138, 0x1811);
    write(&mut c, 0x2104, 0xbb); // Latch survives reads and reloads.
    address(&mut c, 5);
    save(&mut c, 0x2138, 0x1812);
    save(&mut c, 0x2138, 0x1813);
    write(&mut c, 0x18f0, 0xa5);
    c.push(0xdb);
    let mut rom = vec![0; 32768];
    assert!(c.len() < 0x7fc0);
    rom[..c.len()].copy_from_slice(&c);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"OAM ACCESS ORIGINAL  ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&[0, 0x80]);
    rom
}

#[test]
fn cpu_guest_initializes_synchronizes_recovers_and_publishes_oam_results() {
    let mut m = Machine::new(&guest()).unwrap();
    let mut completed = false;
    for _ in 0..150_000 {
        m.step();
        assert!(m.bus.frames < 4, "guest frame deadline");
        if m.cpu.stopped {
            assert_eq!(m.bus.peek(0x18f0), 0xa5);
            let results: Vec<_> = (0x1800..=0x1813).map(|a| m.bus.peek(a)).collect();
            // Source-backed forced-blank initialization controls.
            for (i, expected) in [(0, 0x12), (1, 0x34)] {
                assert_eq!(results[i], expected, "source-supported control {i}");
            }
            // Restricted addresses/values and their downstream latch effects
            // belong to the documented deterministic emulator approximation.
            assert_eq!(
                results,
                [
                    0x12, 0x34, 0xa1, 0x87, 0xc3, 0x12, 0x65, 0x9a, 0x56, 0x78, 0xde, 0xad, 0x65,
                    0x9a, 0x65, 0xad, 0xc3, 0x56, 0xde, 0xbb
                ]
            );
            completed = true;
            break;
        }
    }
    assert!(completed, "guest instruction deadline");
}
