//! Original source-built MIT 65C816 Issue #99 diagnostic.
//! Copyright (c) 2026 snes-rust contributors. No host timing/state injection.
use snes_core::Machine;

fn write(code: &mut Vec<u8>, address: u16, value: u8) {
    code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
}
fn save(code: &mut Vec<u8>, address: u16, result: u16, mask: u8) {
    code.extend([
        0xad,
        address as u8,
        (address >> 8) as u8,
        0x29,
        mask,
        0x8d,
        result as u8,
        (result >> 8) as u8,
    ]);
}
// Literal records: SLHV open bus, H low/high, V low/high, STAT78, TIMEUP twice.
// Independently derived captures: HV at V=240/master H=1330,1328,1330,1328
// gives PPU H=331,332,331,332. H-only at V=242/master H=304,300,304,300
// gives PPU H=76,75,76,75. See PROVENANCE and Issue #99 validation.
const EXPECTED: [u8; 64] = [
    0x21, 75, 1, 240, 0, 0x40, 0x80, 0, 0x21, 76, 0, 242, 0, 0x40, 0x80, 0, 0x21, 76, 1, 240, 0,
    0xc0, 0x80, 0, 0x21, 75, 0, 242, 0, 0xc0, 0x80, 0, 0x21, 75, 1, 240, 0, 0x40, 0x80, 0, 0x21,
    76, 0, 242, 0, 0x40, 0x80, 0, 0x21, 76, 1, 240, 0, 0xc0, 0x80, 0, 0x21, 75, 0, 242, 0, 0xc0,
    0x80, 0,
];

fn record(code: &mut Vec<u8>, dest: u16) {
    for (i, (port, mask)) in [
        (0x2137, 255),
        (0x213c, 255),
        (0x213c, 1),
        (0x213d, 255),
        (0x213d, 1),
        (0x213f, 0xc0),
        (0x4211, 0x80),
        (0x4211, 0x80),
    ]
    .into_iter()
    .enumerate()
    {
        save(code, port, dest + i as u16, mask);
    }
}
fn guest() -> Vec<u8> {
    let mut code = vec![0x78]; // SEI: masked IRQ wakes WAI without vectoring.
    write(&mut code, 0x2100, 0x80);
    write(&mut code, 0x2133, 0);
    for field in 0..4 {
        // Establish a late V=239 gate, then preconfigure the tested HV mode
        // before entering V=240. Both comparisons are nonzero and same-line.
        for (a, v) in [
            (0x4207, 0),
            (0x4208, 1),
            (0x4209, 239),
            (0x420a, 0),
            (0x4200, 0x30),
        ] {
            write(&mut code, a, v);
        }
        code.extend([0xcb, 0xad, 0x11, 0x42]); // WAI, acknowledge gate
        for (a, v) in [(0x4200, 0), (0x4207, 65), (0x4209, 240), (0x4200, 0x30)] {
            write(&mut code, a, v);
        }
        code.push(0xcb);
        record(&mut code, 0x1800 + field * 16);
        // These writes finish on V=241 after H=270. Configure H-only before
        // V=242; deliberately leave the nonmatching VTIME=240 programmed.
        for (a, v) in [(0x4200, 0), (0x4207, 64), (0x4208, 0), (0x4200, 0x10)] {
            write(&mut code, a, v);
        }
        code.push(0xcb);
        record(&mut code, 0x1808 + field * 16);
        write(&mut code, 0x4200, 0); // before the next H-only assertion
    }
    for (i, value) in EXPECTED.into_iter().enumerate() {
        let a = 0x1800 + i as u16;
        code.extend([0xad, a as u8, (a >> 8) as u8, 0xc9, value, 0xf0, 6]);
        write(&mut code, 0x18f1, 0xee); // explicit guest failure marker
        code.push(0xdb);
    }
    write(&mut code, 0x18f0, 0xa5);
    code.push(0xdb);
    let mut rom = vec![0; 32768];
    assert!(code.len() < 0x7fc0);
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"H IRQ DELAY ORIGINAL ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&[0, 0x80]);
    rom
}

#[test]
fn horizontal_irq_guest_self_checks_both_modes_in_four_natural_fields() {
    let mut m = Machine::new(&guest()).unwrap();
    for _ in 0..300_000 {
        m.step();
        assert!(m.bus.clocks < 1_500_000, "guest master-clock deadline");
        if m.cpu.stopped {
            let result: Vec<_> = (0x1800..0x1840).map(|a| m.bus.peek(a)).collect();
            assert_eq!(
                result,
                EXPECTED,
                "guest records, success={:02x}, failure={:02x}",
                m.bus.peek(0x18f0),
                m.bus.peek(0x18f1)
            );
            assert_eq!(m.bus.peek(0x18f1), 0, "guest failure marker");
            assert_eq!(m.bus.peek(0x18f0), 0xa5, "guest self-check success marker");
            assert_eq!(m.bus.frames, 3);
            return;
        }
    }
    panic!("guest instruction/step deadline");
}
