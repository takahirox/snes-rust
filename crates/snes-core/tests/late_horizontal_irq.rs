//! Original source-built MIT 65C816 Issue #105 diagnostic.
//! Copyright (c) 2026 snes-rust contributors. PROVISIONAL / HARDWARE UNVERIFIED.
//! Host code only constructs, loads, steps and observes; no state injection.
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
fn wait_v(code: &mut Vec<u8>, v: u8, irq_paced: bool) {
    let begin = code.len();
    if irq_paced {
        // SEI leaves IRQ masked but it wakes WAI; acknowledge before next wait.
        code.extend([0xcb, 0xad, 0x11, 0x42]);
    }
    code.extend([
        0xad, 0x3f, 0x21, // reset both counter byte selectors
        0xad, 0x37, 0x21, // capture naturally reached beam
        0xad, 0x3d, 0x21, // V low
        0xc9, v, 0xd0, 0, // CMP target; BNE loop
    ]);
    let end = code.len();
    code[end - 1] = (begin as isize - end as isize) as i8 as u8;
}

// Six bytes/field: SLHV open bus, V low/high, STAT78, TIMEUP, acknowledgement.
// With HTIME=337, V=240's ordinary event wakes just before rollover; SLHV
// captures V=241. Odd V=240 suppresses, so the next wake captures V=242.
const EXPECTED_337: [u8; 24] = [
    0x21, 241, 0, 0x40, 128, 0, 0x21, 242, 0, 0xc0, 128, 0, 0x21, 241, 0, 0x40, 128, 0, 0x21, 242,
    0, 0xc0, 128, 0,
];
// HTIME=338 suppresses on every line. Poll to V=241 without IRQ assistance.
const EXPECTED_338: [u8; 24] = [
    0x21, 241, 0, 0x40, 0, 0, 0x21, 241, 0, 0xc0, 0, 0, 0x21, 241, 0, 0x40, 0, 0, 0x21, 241, 0,
    0xc0, 0, 0,
];

fn guest(htime: u16, expected: &[u8]) -> Vec<u8> {
    let mut code = vec![0x78]; // SEI; no handler affects timing or results
    for (a, v) in [
        (0x2100, 0x80),
        (0x2133, 0),
        (0x4207, htime as u8),
        (0x4208, (htime >> 8) as u8),
        (0x4209, 0xff),
        (0x420a, 1),
        (0x4200, 0x10),
    ] {
        write(&mut code, a, v);
    }
    // HTIME, VTIME=511 and H-only enable remain stable across four natural fields.
    for field in 0..4 {
        if htime == 337 {
            wait_v(&mut code, 240, true); // wake from V=239's eligible event
            code.push(0xcb); // V=240 event, or V=241 when V=240 suppresses
        } else {
            wait_v(&mut code, 241, false); // no IRQ must be needed for progress
        }
        for (i, (port, mask)) in [
            (0x2137, 255),
            (0x213d, 255),
            (0x213d, 1),
            (0x213f, 0xc0),
            (0x4211, 128),
            (0x4211, 128),
        ]
        .into_iter()
        .enumerate()
        {
            // wait_v consumed V low; reset its selector before the record.
            if i == 0 {
                code.extend([0xad, 0x3f, 0x21]);
            }
            save(&mut code, port, 0x1800 + field * 6 + i as u16, mask);
        }
        if htime == 338 {
            wait_v(&mut code, 242, false);
        } // leave target line
    }
    // Every result is checked by executed guest code before a success marker.
    for (i, &value) in expected.iter().enumerate() {
        let a = 0x1800 + i as u16;
        code.extend([0xad, a as u8, (a >> 8) as u8, 0xc9, value, 0xf0, 6]);
        write(&mut code, 0x18f1, 0xee);
        code.push(0xdb);
    }
    write(&mut code, 0x18f0, 0xa5);
    code.push(0xdb);
    let mut rom = vec![0; 32768];
    assert!(code.len() < 0x7fc0);
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"LATE H IRQ ORIGINAL  ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&[0, 0x80]);
    rom
}

#[test]
fn late_horizontal_irq_guest_self_checks_policy_in_four_natural_fields() {
    for (htime, expected) in [(337, EXPECTED_337), (338, EXPECTED_338)] {
        let mut m = Machine::new(&guest(htime, &expected)).unwrap();
        for step in 0..300_000 {
            m.step();
            assert!(
                m.bus.clocks < 1_500_000,
                "guest clock deadline, HTIME={htime}"
            );
            if m.cpu.stopped {
                let result: Vec<_> = (0x1800..0x1818).map(|a| m.bus.peek(a)).collect();
                assert_eq!(
                    result,
                    expected,
                    "HTIME={htime}, success={:02x}, failure={:02x}",
                    m.bus.peek(0x18f0),
                    m.bus.peek(0x18f1)
                );
                assert_eq!(m.bus.peek(0x18f1), 0, "executed guest failure marker");
                assert_eq!(m.bus.peek(0x18f0), 0xa5, "executed guest success marker");
                assert_eq!(m.bus.frames, 3);
                break;
            }
            assert!(step < 299_999, "guest step deadline, HTIME={htime}");
        }
    }
}
