//! Original MIT 65C816 counter-latch guest, built entirely in memory.
//! Copyright (c) 2026 snes-rust contributors.
use snes_core::Machine;

fn write(code: &mut Vec<u8>, address: u16, value: u8) {
    code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
}
fn save(code: &mut Vec<u8>, address: u16, result: u16, mask: u8) {
    code.extend([0xad, address as u8, (address >> 8) as u8]);
    if mask != 0xff {
        code.extend([0x29, mask]);
    }
    code.extend([0x8d, result as u8, (result >> 8) as u8]);
}
fn counters(code: &mut Vec<u8>, result: u16) {
    for (i, (r, mask)) in [(0x213c, 255), (0x213c, 1), (0x213d, 255), (0x213d, 1)]
        .into_iter()
        .enumerate()
    {
        save(code, r, result + i as u16, mask);
    }
}
fn guest() -> Vec<u8> {
    let mut code = vec![0x78]; // SEI: timer IRQ wakes WAI without vectoring.
    write(&mut code, 0x4201, 0xff);
    save(&mut code, 0x213f, 0x1800, 0x40); // No initialization capture.

    // Execute LDA HVBJOY / AND / BEQ until VBlank, then arm H/V IRQ.
    code.extend([0xad, 0x12, 0x42, 0x29, 0x80, 0xf0, 0xf9]);
    for (a, v) in [
        (0x4207, 0),
        (0x4208, 1),
        (0x4209, 0),
        (0x420a, 1),
        (0x4200, 0x30),
    ] {
        write(&mut code, a, v); // VTIME=256, HTIME=256, both ninth bits exercised.
    }
    code.push(0xcb);
    save(&mut code, 0x2137, 0x1801, 255); // Enabled capture A; CPU open bus $21.
    save(&mut code, 0x213c, 0x1802, 255); // H low, leave H selector high.
    save(&mut code, 0x213d, 0x1803, 255); // V low, leave V selector high.
    write(&mut code, 0x4201, 0x7f); // Falling-edge capture B between low/high reads.
    save(&mut code, 0x213c, 0x1804, 1);
    save(&mut code, 0x213d, 0x1805, 1);
    counters(&mut code, 0x1806); // B, not A.
    save(&mut code, 0x2137, 0x180a, 255); // Disabled software capture preserves B.
    write(&mut code, 0x4201, 0); // Repeated low, other bits only.
    counters(&mut code, 0x180b);
    // Leave both selectors high; every low STAT78 read must reset them.
    save(&mut code, 0x213c, 0x180f, 255);
    save(&mut code, 0x213d, 0x1810, 255);
    save(&mut code, 0x213f, 0x1811, 0x40);
    save(&mut code, 0x213c, 0x1812, 255);
    save(&mut code, 0x213d, 0x1813, 255);
    save(&mut code, 0x213f, 0x1814, 0x40);
    write(&mut code, 0x4201, 0xff); // Release alone preserves B and flag.
    counters(&mut code, 0x1815);
    save(&mut code, 0x213f, 0x1819, 0x40);
    save(&mut code, 0x213f, 0x181a, 0x40);
    save(&mut code, 0x2137, 0x181b, 255); // Enabled again: capture C.
    counters(&mut code, 0x181c);
    save(&mut code, 0x213f, 0x1820, 0x40);
    save(&mut code, 0x213f, 0x1821, 0x40);
    write(&mut code, 0x18f0, 0xa5);
    code.push(0xdb);
    let mut rom = vec![0; 32768];
    assert!(code.len() < 0x7fc0);
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"WRIO LATCH ORIGINAL  ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&[0, 0x80]);
    rom
}
#[test]
fn cpu_guest_captures_gates_acknowledges_and_preserves_selectors() {
    let mut m = Machine::new(&guest()).unwrap();
    for _ in 0..100_000 {
        m.step();
        assert!(m.bus.frames < 3, "guest frame deadline");
        if m.cpu.stopped {
            assert_eq!(m.bus.peek(0x18f0), 0xa5);
            let result: Vec<_> = (0x1800..=0x1821).map(|a| m.bus.peek(a)).collect();
            // Model arithmetic: assertion is 256*4+14=1038; six-clock WAI
            // polling wakes at (256,1040), SLHV completes at 1070 (dot 267).
            // The edge completes at 1272 (dot 318); re-enabled SLHV
            // completes at (258,324), or dot 81.
            assert_eq!(
                result,
                [
                    0, 0x21, 11, 0, 1, 1, 62, 1, 0, 1, 0x21, 62, 1, 0, 1, 62, 0, 0x40, 62, 0, 0x40,
                    62, 1, 0, 1, 0x40, 0, 0x21, 81, 0, 2, 1, 0x40, 0,
                ]
            );
            return;
        }
    }
    panic!("guest instruction deadline");
}
