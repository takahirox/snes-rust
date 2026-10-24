//! Original source-built MIT 65C816 diagnostic for Issue #103.
//! Copyright (c) 2026 snes-rust contributors. No host beam/IRQ/result injection.
use snes_core::Machine;

fn write(code: &mut Vec<u8>, address: u16, value: u8) {
    code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
}
fn save(code: &mut Vec<u8>, address: u16, result: u16, mask: u8) {
    code.extend([
        0xad,
        address as u8,
        (address >> 8) as u8, // LDA absolute
        0x29,
        mask, // AND immediate
        0x8d,
        result as u8,
        (result >> 8) as u8, // STA absolute
    ]);
}
// Literal records derived from 3.5*4 assertion clocks and existing CPU costs.
// SLHV ends at master H=48/46/48/46, giving PPU H=12/11/12/11.
// See tests/roms/PROVENANCE.md for the independent arithmetic.
const EXPECTED: [u8; 32] = [
    0x21, 12, 0, 240, 0, 0x40, 0x80, 0, 0x21, 11, 0, 240, 0, 0xc0, 0x80, 0, 0x21, 12, 0, 240, 0,
    0x40, 0x80, 0, 0x21, 11, 0, 240, 0, 0xc0, 0x80, 0,
];

fn guest() -> Vec<u8> {
    let mut code = vec![0x78]; // SEI: IRQ wakes WAI without vectoring.
    write(&mut code, 0x2100, 0x80);
    write(&mut code, 0x2133, 0);
    for field in 0..4 {
        // Gate on V=239/H=1038 using the established nonzero HV delay.
        // This wakes late enough to configure zero after V=239's H=14,
        // and early enough to have both bytes zero before entering V=240.
        for (a, v) in [
            (0x4207, 0),
            (0x4208, 1),
            (0x4209, 239),
            (0x420a, 0),
            (0x4200, 0x30),
        ] {
            write(&mut code, a, v);
        }
        code.extend([0xcb, 0xad, 0x11, 0x42]); // WAI; acknowledge gate
        for (a, v) in [(0x4200, 0), (0x4207, 0), (0x4208, 0), (0x4200, 0x10)] {
            write(&mut code, a, v);
        }
        // VTIME deliberately stays 239. H-only must wake on the next line.
        code.push(0xcb);
        let dest = 0x1800 + field * 8;
        save(&mut code, 0x2137, dest, 0xff);
        save(&mut code, 0x213c, dest + 1, 0xff);
        save(&mut code, 0x213c, dest + 2, 1);
        save(&mut code, 0x213d, dest + 3, 0xff);
        save(&mut code, 0x213d, dest + 4, 1);
        save(&mut code, 0x213f, dest + 5, 0xc0);
        save(&mut code, 0x4211, dest + 6, 0x80); // observe and acknowledge
        save(&mut code, 0x4211, dest + 7, 0x80); // no same-line reassertion
        write(&mut code, 0x4200, 0); // disable before next H-only event
    }
    // Check every observation in guest code. Failure STPs before success.
    for (i, value) in EXPECTED.into_iter().enumerate() {
        let a = 0x1800 + i as u16;
        code.extend([0xad, a as u8, (a >> 8) as u8, 0xc9, value, 0xf0, 6]);
        write(&mut code, 0x18f1, 0xee); // explicit executed failure marker
        code.push(0xdb);
    }
    write(&mut code, 0x18f0, 0xa5);
    code.push(0xdb);
    let mut rom = vec![0; 32768];
    assert!(code.len() < 0x7fc0);
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"H ZERO IRQ ORIGINAL  ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&[0, 0x80]);
    rom
}

#[test]
fn horizontal_zero_irq_guest_self_checks_delayed_wakes_in_four_natural_fields() {
    let mut m = Machine::new(&guest()).unwrap();
    for _ in 0..300_000 {
        m.step();
        assert!(m.bus.clocks < 1_500_000, "guest master-clock deadline");
        if m.cpu.stopped {
            let result: Vec<_> = (0x1800..0x1820).map(|a| m.bus.peek(a)).collect();
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
