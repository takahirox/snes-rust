//! Original source-built MIT 65C816 guest; no host timing or result injection.
//! Copyright (c) 2026 snes-rust contributors.
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
const EXPECTED: [u8; 24] = [
    0x21, 75, 1, 240, 0, 0x40, 0x21, 76, 1, 240, 0, 0xc0, 0x21, 75, 1, 240, 0, 0x40, 0x21, 76, 1,
    240, 0, 0xc0,
];

fn guest() -> Vec<u8> {
    let mut code = vec![0x78]; // SEI: IRQ wakes WAI without taking a vector.
    for (a, v) in [
        (0x2100, 0x80),
        (0x2133, 0),
        (0x4207, 65),
        (0x4208, 1),
        (0x4209, 240),
        (0x420a, 0),
        (0x4200, 0x30),
    ] {
        write(&mut code, a, v);
    }
    // H/V assertion: 321*4+14=1298 CPU timer clocks, independent of
    // PPU long dots. Six-clock WAI polling and 30-clock SLHV access give
    // captures at 1330,1328,1330,1328. Ordinary [1328,1332) represents
    // PPU H=331; short [1328,1332) represents H=332. See Issue #99 validation.
    for field in 0..4 {
        code.push(0xcb); // Naturally wait for V=240 with HTIME=321 in the next field.
        let dest = 0x1800 + field * 6;
        save(&mut code, 0x2137, dest, 0xff);
        save(&mut code, 0x213c, dest + 1, 0xff);
        save(&mut code, 0x213c, dest + 2, 1);
        save(&mut code, 0x213d, dest + 3, 0xff);
        save(&mut code, 0x213d, dest + 4, 1);
        save(&mut code, 0x213f, dest + 5, 0xc0);
        code.extend([0xad, 0x11, 0x42]); // acknowledge timer before the next WAI
    }
    // Guest checks its own WRAM observations before writing the success marker.
    // A mismatch stops immediately, leaving the marker zero.
    for (i, value) in EXPECTED.into_iter().enumerate() {
        let a = 0x1800 + i as u16;
        code.extend([0xad, a as u8, (a >> 8) as u8, 0xc9, value, 0xf0, 1, 0xdb]);
    }
    write(&mut code, 0x18f0, 0xa5);
    code.push(0xdb);
    let mut rom = vec![0; 32768];
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"NTSC TIMING ORIGINAL ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&[0, 0x80]);
    rom
}
#[test]
fn cpu_guest_records_counters_and_four_naturally_reached_fields() {
    let mut m = Machine::new(&guest()).unwrap();
    for _ in 0..300_000 {
        m.step();
        assert!(m.bus.clocks < 1_500_000, "guest clock deadline");
        if m.cpu.stopped {
            assert_eq!(m.bus.peek(0x18f0), 0xa5);
            let result: Vec<_> = (0x1800..0x1818).map(|a| m.bus.peek(a)).collect();
            assert_eq!(result, EXPECTED);
            return;
        }
    }
    panic!("guest instruction deadline");
}
