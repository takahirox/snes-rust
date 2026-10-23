//! Original MIT 65C816 guests for Issue #21; no external code/data/firmware.
//! Copyright (c) 2026 snes-rust contributors.
use snes_core::Machine;

fn write(code: &mut Vec<u8>, a: u16, value: u8) {
    // LDA #byte; STA absolute (reset's emulation-mode eight-bit accumulator).
    code.extend([0xa9, value, 0x8d, a as u8, (a >> 8) as u8]);
}

fn save(code: &mut Vec<u8>, source: u16, result: u16) {
    code.extend([0xad, source as u8, (source >> 8) as u8]);
    code.extend([0x8d, result as u8, (result >> 8) as u8]);
}

fn transfer(code: &mut Vec<u8>, mode: u8, port: u8, address: u16, count: u8) {
    for (reg, value) in [
        (0x4300, mode),
        (0x4301, port),
        (0x4302, address as u8),
        (0x4303, (address >> 8) as u8),
        (0x4304, 0),
        (0x4305, count),
        (0x4306, 0),
        (0x420b, 1),
    ] {
        write(code, reg, value);
    }
}

fn guest(reverse: bool) -> Vec<u8> {
    let mut code = vec![0x78]; // SEI
    for (reg, value) in [
        (0x2100, 0x80),
        (0x4200, 0),
        (0x1000, 0x5a),
        (0x1001, 0x12),
        (0x2181, 0),
        (0x2182, 0x10),
        (0x2183, 0),
        (0x2121, 0),
        (0x2122, 0x35),
        (0x2122, 0x12),
        (0x2122, 0x67),
        (0x2122, 0x23),
        (0x2121, 0),
    ] {
        write(&mut code, reg, value);
    }
    if reverse {
        write(&mut code, 0x4201, 0xa6);
        transfer(&mut code, 0x88, 0x3b, 0x4201, 2);
        save(&mut code, 0x4213, 0x1800); // CPU control unchanged.
        save(&mut code, 0x213b, 0x1801); // B-bus consumed both reads: $67.
        write(&mut code, 0x437f, 0xa6);
        transfer(&mut code, 0x88, 0x3b, 0x437f, 1);
        save(&mut code, 0x437f, 0x1802); // DMA register unchanged.
        write(&mut code, 0x2121, 0);
        transfer(&mut code, 0x88, 0x3b, 0x2180, 2);
        save(&mut code, 0x2180, 0x1803); // A-bus WRAM port write suppressed.
        save(&mut code, 0x2180, 0x1804);
    } else {
        // Exact Issue reproduction: fixed illegal A-bus $00:2180 -> CGRAM.
        transfer(&mut code, 8, 0x22, 0x2180, 2);
        save(&mut code, 0x2180, 0x1800);
        save(&mut code, 0x2180, 0x1801);
        write(&mut code, 0x2121, 0);
        save(&mut code, 0x213b, 0x1802);
        save(&mut code, 0x213b, 0x1803);
        for (reg, result) in (0x4302..=0x4306).zip(0x1804..=0x1808) {
            save(&mut code, reg, result);
        }
        // A prohibited serial read must leave the first button for the CPU.
        write(&mut code, 0x4016, 1);
        write(&mut code, 0x4016, 0);
        transfer(&mut code, 8, 6, 0x4016, 2);
        code.extend([0xad, 0x16, 0x40, 0x29, 1, 0x8d, 0x09, 0x18]);
    }
    write(&mut code, 0x1810, 0xa5);
    code.push(0xdb); // STP is a bounded diagnostic success terminator.
    assert!(code.len() < 0x7fc0);
    let mut rom = vec![0; 32768];
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"DMA ACCESS ORIGINAL  ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&[0, 0x80]);
    rom
}

fn run(reverse: bool) -> Machine {
    let mut m = Machine::new(&guest(reverse)).unwrap();
    m.set_buttons(0x8000);
    for _ in 0..10_000 {
        m.step();
        assert!(m.bus.frames < 2, "guest frame deadline");
        if m.cpu.stopped {
            assert_eq!(m.bus.peek(0x1810), 0xa5);
            return m;
        }
    }
    panic!("guest instruction deadline");
}

#[test]
fn guest_forward_prohibited_wram_port_reproduction() {
    let m = run(false);
    let results: Vec<_> = (0x1800..=0x1809).map(|a| m.bus.peek(a)).collect();
    // WRAM pointer stayed at $1000; CGRAM got zero; fixed A address/count
    // completed normally; the CPU still reads the first latched joypad bit.
    assert_eq!(results, [0x5a, 0x12, 0, 0, 0x80, 0x21, 0, 0, 0, 1]);
    assert_eq!(&m.bus.ppu.cgram[..4], &[0, 0, 0x67, 0x23]);
    assert_eq!(m.bus.ppu.regs[6], 0);
}

#[test]
fn guest_reverse_prohibited_writes_preserve_registers_and_wram_port() {
    let m = run(true);
    let results: Vec<_> = (0x1800..=0x1804).map(|a| m.bus.peek(a)).collect();
    assert_eq!(results, [0xa6, 0x67, 0xa6, 0x5a, 0x12]);
    assert_eq!(&m.bus.wram[0x1000..0x1002], &[0x5a, 0x12]);
}
