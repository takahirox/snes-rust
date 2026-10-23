//! Original MIT 65C816 row-end reload guests; no external code/data.
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
fn color(code: &mut Vec<u8>, result: u16) {
    write(code, 0x2121, 0);
    save(code, 0x213b, result);
    save(code, 0x213b, result + 1);
}
fn guest(indirect: bool) -> Vec<u8> {
    let mut code = vec![0x78]; // SEI, emulation mode / 8-bit A.
    write(&mut code, 0x2100, 0x80);
    write(&mut code, 0x4200, 0);
    wait_status(&mut code, 0x80, true);
    let table: &[u8] = if indirect {
        &[0x81, 0, 0x12, 0x82, 0, 0x13, 0, 0x56]
    } else {
        &[0x81, 0x11, 0x12, 0x82, 0x22, 0x23, 0x33, 0x34, 0]
    };
    for (i, &v) in table.iter().enumerate() {
        write(&mut code, 0x1000 + i as u16, v);
    }
    for (address, value) in [
        (0x1200, 0x11),
        (0x1201, 0x12),
        (0x1300, 0x22),
        (0x1301, 0x23),
        (0x1302, 0x33),
        (0x1303, 0x34),
        (0x1400, 0x55),
        (0x1401, 0x15),
        (0x2121, 0),
        (0x4300, if indirect { 0x42 } else { 2 }),
        (0x4301, 0x22),
        (0x4302, 0),
        (0x4303, 0x10),
        (0x4304, 0),
        (0x4305, 0xa6),
        (0x4306, 0x5a),
        (0x4307, 0),
        (0x420c, 1),
    ] {
        write(&mut code, address, value);
    }
    wait_status(&mut code, 0x80, false);
    wait_status(&mut code, 0x40, true);
    wait_status(&mut code, 0x40, false);
    for (address, result) in [0x4308, 0x4309, 0x430a, 0x4305, 0x4306]
        .into_iter()
        .zip(0x1800..)
    {
        save(&mut code, address, result);
    }
    color(&mut code, 0x1805);
    write(&mut code, 0x1003, 0); // The second descriptor has already been fetched.
    if indirect {
        write(&mut code, 0x1004, 0x80); // Replace both pointer bytes with $1480.
        write(&mut code, 0x1005, 0x14);
        write(&mut code, 0x1300, 0x66); // Original pointer's payload stays live.
    } else {
        write(&mut code, 0x1004, 0x66);
    }
    write(&mut code, 0x2121, 0); // Write subsequent colors to slot zero again.
    wait_status(&mut code, 0x40, true);
    wait_status(&mut code, 0x40, false);
    save(&mut code, 0x430a, 0x1807);
    color(&mut code, 0x1808);
    write(&mut code, 0x2121, 0);
    wait_status(&mut code, 0x40, true);
    wait_status(&mut code, 0x40, false);
    for (address, result) in [0x4308, 0x4309, 0x430a, 0x4305, 0x4306]
        .into_iter()
        .zip(0x180a..)
    {
        save(&mut code, address, result);
    }
    color(&mut code, 0x180f);
    for (address, result) in (0x4302..=0x4304).zip(0x1811..) {
        save(&mut code, address, result);
    }
    // HDMAEN is write-only; internal termination is observed by no further write.
    write(&mut code, 0x2121, 0);
    write(&mut code, 0x2122, 0x77);
    write(&mut code, 0x2122, 0x17);
    wait_status(&mut code, 0x40, true);
    wait_status(&mut code, 0x40, false);
    color(&mut code, 0x1814);
    write(&mut code, 0x1816, 0xa5);
    code.push(0xdb);
    let mut rom = vec![0; 32768];
    assert!(code.len() < 0x7fc0);
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"HDMA RELOAD ORIGINAL ");
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
            assert_eq!(machine.bus.peek(0x1816), 0xa5);
            return (0x1800..=0x1815).map(|a| machine.bus.peek(a)).collect();
        }
    }
    panic!("guest instruction deadline");
}
#[test]
fn direct_guest_retains_next_descriptor_and_reads_live_payload() {
    assert_eq!(
        run(false),
        [
            4, 0x10, 0x82, 0xa6, 0x5a, 0x11, 0x12, 0x81, 0x66, 0x23, 9, 0x10, 0, 0xa6, 0x5a, 0x33,
            0x34, 0, 0x10, 0, 0x77, 0x17
        ]
    );
}
#[test]
fn indirect_guest_retains_next_pointer_and_publishes_termination() {
    assert_eq!(
        run(true),
        [
            6, 0x10, 0x82, 0, 0x13, 0x11, 0x12, 0x81, 0x66, 0x23, 8, 0x10, 0, 0, 0x56, 0x33, 0x34,
            0, 0x10, 0, 0x77, 0x17
        ]
    );
}
