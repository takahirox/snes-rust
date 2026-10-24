//! Original MIT automatic two-pad 65C816 guests for Issue #69.
//! Copyright (c) 2026 snes-rust contributors.
use snes_core::{cpu::Bus, Machine};

fn write(code: &mut Vec<u8>, address: u16, value: u8) {
    code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
}

fn cartridge(code: &[u8]) -> Vec<u8> {
    let mut rom = vec![0; 32768];
    rom[..code.len()].copy_from_slice(code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"AUTO TWO PAD ORIGINAL");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&[0, 0x80]);
    rom
}

fn cycle(code: &mut Vec<u8>, destination: u16) {
    // Observe busy SET before waiting for CLEAR: VBlank itself is too early.
    code.extend([0xad, 0x12, 0x42, 0x29, 1, 0xf0, 0xf9]);
    code.extend([0x8d, (destination + 8) as u8, (destination >> 8) as u8]);
    code.extend([0xad, 0x12, 0x42, 0x29, 1, 0xd0, 0xf9]);
    code.extend([0x8d, (destination + 9) as u8, (destination >> 8) as u8]);
    for offset in 0..8 {
        // Independent system-bank mirrors, through actual CPU bus scheduling.
        let dest = destination + offset;
        code.extend([
            0xaf,
            0x18 + offset as u8,
            0x42,
            0x80 + offset as u8,
            0x8d,
            dest as u8,
            (dest >> 8) as u8,
        ]);
    }
}

fn guest(two_cycles: bool) -> Vec<u8> {
    let mut code = vec![0x78]; // SEI; reset's emulation/eight-bit mode.
    write(&mut code, 0x2100, 0x80);
    write(&mut code, 0x4016, 0);
    write(&mut code, 0x4200, 1);
    cycle(&mut code, 0x1800);
    write(&mut code, 0x18f0, 1); // Host observes, then supplies next inputs.
    if two_cycles {
        cycle(&mut code, 0x1810);
        // Guest checks swapped second-cycle inputs; a failure stops with $EE.
        for (offset, value) in [0x60, 0x5a, 0x90, 0xa5, 0, 0, 0, 0].into_iter().enumerate() {
            code.extend([0xad, 0x10 + offset as u8, 0x18, 0xc9, value, 0xf0, 6]);
            write(&mut code, 0x18f1, 0xee);
            code.push(0xdb);
        }
    }
    write(&mut code, 0x18f0, 0xa5);
    code.push(0xdb);
    cartridge(&code)
}

fn results(m: &Machine, start: u32) -> Vec<u8> {
    (start..start + 10).map(|a| m.bus.peek(a)).collect()
}

fn run(two_cycles: bool) -> Machine {
    let mut m = Machine::new(&guest(two_cycles)).unwrap();
    m.set_buttons(0xa59f);
    m.set_buttons2(0x5a6f);
    let mut changed = false;
    for _ in 0..150_000 {
        m.step();
        assert!(m.bus.frames < 3, "automatic guest frame deadline");
        assert!(m.bus.apu.error.is_none());
        if two_cycles && !changed && m.bus.peek(0x18f0) == 1 {
            m.set_buttons(0x5a60);
            m.set_buttons2(0xa590);
            changed = true;
        }
        if m.cpu.stopped {
            assert_eq!(m.bus.peek(0x18f1), 0, "guest comparison failed");
            assert_eq!(m.bus.peek(0x18f0), 0xa5);
            assert!(!two_cycles || changed);
            return m;
        }
    }
    panic!("automatic guest instruction deadline");
}

#[test]
fn automatic_second_pad_counterexample() {
    let m = run(false);
    assert_eq!(
        results(&m, 0x1800),
        [0x90, 0xa5, 0x60, 0x5a, 0, 0, 0, 0, 1, 0]
    );
}

#[test]
fn cpu_guest_observes_two_natural_cycles_with_changed_inputs() {
    let mut m = run(true);
    assert_eq!(
        results(&m, 0x1800),
        [0x90, 0xa5, 0x60, 0x5a, 0, 0, 0, 0, 1, 0]
    );
    assert_eq!(
        results(&m, 0x1810),
        [0x60, 0x5a, 0x90, 0xa5, 0, 0, 0, 0, 1, 0]
    );
    // Automatic reads consumed all sixteen bits on both ports.
    assert_eq!(m.bus.read(0x4016) & 3, 1);
    assert_eq!(m.bus.read(0x4017) & 3, 1);
}
