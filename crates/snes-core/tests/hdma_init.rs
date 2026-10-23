//! Original MIT 65C816 initialization-prefetch guests; no external code/data.
//! Copyright (c) 2026 snes-rust contributors.
use snes_core::Machine;

fn write(code: &mut Vec<u8>, address: u16, value: u8) {
    code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
}
fn save(code: &mut Vec<u8>, address: u16, result: u16) {
    code.extend([0xad, address as u8, (address >> 8) as u8]);
    code.extend([0x8d, result as u8, (result >> 8) as u8]);
}
fn wait_status(code: &mut Vec<u8>, mask: u8, set: bool) {
    // LDA $4212; AND #mask; BEQ/BNE back to LDA.
    code.extend([
        0xad,
        0x12,
        0x42,
        0x29,
        mask,
        if set { 0xf0 } else { 0xd0 },
        0xf9,
    ]);
}
fn guest(indirect: bool) -> Vec<u8> {
    let mut code = vec![0x78]; // SEI; reset's 8-bit emulation accumulator.
    write(&mut code, 0x2100, 0x80);
    write(&mut code, 0x4200, 0);
    wait_status(&mut code, 0x80, true); // Configure in VBlank, never after init.
    for (address, value) in [
        (0x1000, 0x82),
        (0x1001, if indirect { 0x00 } else { 0x11 }),
        (0x1002, 0x12),
        (0x1003, if indirect { 0 } else { 0x22 }),
        (0x1004, 0x23),
        (0x1005, 0),
        (0x1200, 0x11),
        (0x1201, 0x12),
        (0x1202, 0x22),
        (0x1203, 0x23),
        (0x1300, 0x55),
        (0x1301, 0x15),
        (0x2121, 0),
        (0x4300, if indirect { 0x42 } else { 2 }),
        (0x4301, 0x22),
        (0x4302, 0),
        (0x4303, 0x10),
        (0x4304, 0),
        (0x4305, 0xa6),
        (0x4306, 0x5a),
        (0x4307, 0),
        (0x4308, 0xcc),
        (0x4309, 0xdd),
        (0x430a, 0xee),
        (0x420c, 1),
    ] {
        write(&mut code, address, value);
    }
    wait_status(&mut code, 0x80, false);
    // Broad margin after approx H=6 dots, still well before HBlank H=274.
    code.extend([0xea; 12]);
    for (address, result) in (0x4308..=0x430a).zip(0x1800..) {
        save(&mut code, address, result);
    }
    for (address, result) in (0x4305..=0x4306).zip(0x1803..) {
        save(&mut code, address, result);
    }
    write(&mut code, 0x1000, 0); // Metadata edit must not terminate this frame's row.
    if indirect {
        write(&mut code, 0x1002, 0x13); // Replacement pointer $1300.
        write(&mut code, 0x1200, 0x66); // Prepared pointer must observe live data.
    } else {
        write(&mut code, 0x1001, 0x66);
    }
    wait_status(&mut code, 0x40, true);
    write(&mut code, 0x420c, 0);
    save(&mut code, 0x430a, 0x1805);
    write(&mut code, 0x2121, 0);
    save(&mut code, 0x213b, 0x1806);
    save(&mut code, 0x213b, 0x1807);
    for (address, result) in (0x4302..=0x4304).zip(0x1808..) {
        save(&mut code, address, result);
    }
    write(&mut code, 0x1810, 0xa5);
    code.push(0xdb);
    let mut rom = vec![0; 32768];
    assert!(code.len() < 0x7fc0);
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"HDMA INIT ORIGINAL   ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&[0, 0x80]);
    rom
}
fn run(indirect: bool) -> Vec<u8> {
    let mut machine = Machine::new(&guest(indirect)).unwrap();
    for _ in 0..100_000 {
        machine.step();
        assert!(machine.bus.frames < 3, "guest frame deadline");
        if machine.cpu.stopped {
            assert_eq!(machine.bus.peek(0x1810), 0xa5);
            return (0x1800..=0x180a).map(|a| machine.bus.peek(a)).collect();
        }
    }
    panic!("guest instruction deadline");
}
#[test]
fn direct_guest_retains_descriptor_and_reads_live_payload() {
    assert_eq!(
        run(false),
        [1, 0x10, 0x82, 0xa6, 0x5a, 0x81, 0x66, 0x12, 0, 0x10, 0]
    );
}
#[test]
fn indirect_guest_retains_descriptor_and_pointer_and_reads_live_payload() {
    assert_eq!(
        run(true),
        [3, 0x10, 0x82, 0, 0x12, 0x81, 0x66, 0x12, 0, 0x10, 0]
    );
}
