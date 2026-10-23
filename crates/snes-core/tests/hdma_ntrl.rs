//! Original MIT live-NTRL CPU guests; no external code/data.
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
fn event(code: &mut Vec<u8>, index: u16) {
    wait_status(code, 0x40, true);
    wait_status(code, 0x40, false);
    let result = 0x1800 + index * 7;
    for (i, a) in [0x430a, 0x4308, 0x4309, 0x4305, 0x4306]
        .into_iter()
        .enumerate()
    {
        save(code, a, result + i as u16);
    }
    write(code, 0x2121, 0);
    save(code, 0x213b, result + 5);
    save(code, 0x213b, result + 6);
    write(code, 0x2121, 0);
}
fn live(code: &mut Vec<u8>, value: u8, index: u16) {
    write(code, 0x430a, value);
    save(code, 0x430a, 0x1900 + index);
}
fn guest(indirect: bool) -> Vec<u8> {
    let mut code = vec![0x78]; // SEI, emulation mode / 8-bit A.
    write(&mut code, 0x2100, 0x80);
    write(&mut code, 0x4200, 0);
    wait_status(&mut code, 0x80, true);
    let table: &[u8] = if indirect {
        &[0x82, 0, 0x12, 0x83, 0, 0x13, 0, 0x56]
    } else {
        &[
            0x82, 0x11, 0x12, 0x22, 0x23, 0x33, 0x34, 0x83, 0x55, 0x15, 0x66, 0x16, 0,
        ]
    };
    for (i, &v) in table.iter().enumerate() {
        write(&mut code, 0x1000 + i as u16, v);
    }
    for (base, values) in [
        (0x1200, &[0x11, 0x12, 0x22, 0x23, 0x33, 0x34][..]),
        (0x1300, &[0x55, 0x15, 0x66, 0x16][..]),
    ] {
        for (i, &v) in values.iter().enumerate() {
            write(&mut code, base + i as u16, v);
        }
    }
    for (address, value) in [
        (0x2121, 0),
        (0x4300, if indirect { 0x42 } else { 2 }),
        (0x4301, 0x22),
        (0x4302, 0),
        (0x4303, 0x10),
        (0x4304, 0),
        (0x4305, 0xa6),
        (0x4306, 0x5a),
        (0x4307, 0),
        (0x430a, 0xff),
        (0x420c, 1),
    ] {
        write(&mut code, address, value);
    }
    save(&mut code, 0x430a, 0x1905);
    wait_status(&mut code, 0x80, false);
    event(&mut code, 0); // $82 fetched at init, first payload -> $81.
    live(&mut code, 2, 0);
    event(&mut code, 1); // Prepared transfer still runs, count -> $01.
    live(&mut code, 0x82, 1);
    event(&mut code, 2); // Prepared hold still holds, count -> $81.
    event(&mut code, 3); // Transfer, then fetch next row ($83).
    live(&mut code, 0x80, 2);
    event(&mut code, 4); // Transfer, count -> $7F, next event holds.
    live(&mut code, 0, 3);
    event(&mut code, 5); // Hold, count -> $FF, next event transfers.
    live(&mut code, 1, 4);
    event(&mut code, 6); // Transfer, then fetched zero terminates.
    live(&mut code, 0x82, 6); // Counter write alone cannot restart termination.
    write(&mut code, 0x2122, 0x77);
    write(&mut code, 0x2122, 0x17);
    event(&mut code, 7);
    for (address, result) in (0x4302..=0x4304).zip(0x1910..) {
        save(&mut code, address, result);
    }
    write(&mut code, 0x18f0, 0xa5);
    code.push(0xdb);
    let mut rom = vec![0; 32768];
    assert!(code.len() < 0x7fc0);
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"HDMA NTRL ORIGINAL   ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&[0, 0x80]);
    rom
}
fn run(indirect: bool) -> Vec<u8> {
    let mut m = Machine::new(&guest(indirect)).unwrap();
    for _ in 0..100_000 {
        m.step();
        assert!(m.bus.frames < 3, "guest frame deadline");
        if m.cpu.stopped {
            assert_eq!(m.bus.peek(0x18f0), 0xa5);
            assert_eq!(
                (0x1900..=0x1906).map(|a| m.bus.peek(a)).collect::<Vec<_>>(),
                [2, 0x82, 0x80, 0, 1, 0xff, 0x82]
            );
            assert_eq!(
                (0x1910..=0x1912).map(|a| m.bus.peek(a)).collect::<Vec<_>>(),
                [0, 0x10, 0]
            );
            return (0x1800..0x1838).map(|a| m.bus.peek(a)).collect();
        }
    }
    panic!("guest instruction deadline");
}
#[test]
fn direct_guest_writes_live_counter_and_preserves_prepared_transfers_and_holds() {
    assert_eq!(
        run(false),
        [
            0x81, 3, 0x10, 0xa6, 0x5a, 0x11, 0x12, 1, 5, 0x10, 0xa6, 0x5a, 0x22, 0x23, 0x81, 5,
            0x10, 0xa6, 0x5a, 0x22, 0x23, 0x83, 8, 0x10, 0xa6, 0x5a, 0x33, 0x34, 0x7f, 10, 0x10,
            0xa6, 0x5a, 0x55, 0x15, 0xff, 10, 0x10, 0xa6, 0x5a, 0x55, 0x15, 0, 13, 0x10, 0xa6,
            0x5a, 0x66, 0x16, 0x82, 13, 0x10, 0xa6, 0x5a, 0x77, 0x17,
        ]
    );
}
#[test]
fn indirect_guest_writes_live_counter_and_publishes_reload_and_termination() {
    assert_eq!(
        run(true),
        [
            0x81, 3, 0x10, 2, 0x12, 0x11, 0x12, 1, 3, 0x10, 4, 0x12, 0x22, 0x23, 0x81, 3, 0x10, 4,
            0x12, 0x22, 0x23, 0x83, 6, 0x10, 0, 0x13, 0x33, 0x34, 0x7f, 6, 0x10, 2, 0x13, 0x55,
            0x15, 0xff, 6, 0x10, 2, 0x13, 0x55, 0x15, 0, 8, 0x10, 0, 0x56, 0x66, 0x16, 0x82, 8,
            0x10, 0, 0x56, 0x77, 0x17,
        ]
    );
}
