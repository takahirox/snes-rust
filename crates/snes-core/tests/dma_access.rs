//! Original MIT 65C816 guests for Issues #21/#23; no external code/data/firmware.
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
    cartridge(&code)
}

fn cartridge(code: &[u8]) -> Vec<u8> {
    assert!(code.len() < 0x7fc0);
    let mut rom = vec![0; 32768];
    rom[..code.len()].copy_from_slice(code);
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

fn status_guest() -> Vec<u8> {
    let mut code = vec![0x78]; // SEI: timer flag is observed without CPU IRQ entry.
    for (a, v) in [
        (0x2100, 0x80),
        (0x4209, 225),
        (0x420a, 0),
        (0x4200, 0x21), // V-only IRQ at VBlank + automatic controller read.
        (0x4201, 0xa6),
        (0x4204, 0xcd),
        (0x4205, 0xab),
        (0x4206, 19),
        (0x2181, 0),
        (0x2182, 0x19),
        (0x2183, 0),
    ] {
        write(&mut code, a, v);
    }
    // Wait for VBlank, then auto-read completion. No CPU status read acknowledges
    // RDNMI/TIMEUP before DMA. Host enforces instruction and frame deadlines.
    code.extend([0xad, 0x12, 0x42, 0x29, 0x80, 0xf0, 0xf9]);
    code.extend([0xad, 0x12, 0x42, 0x29, 1, 0xd0, 0xf9]);
    transfer(&mut code, 8, 0x80, 0x4210, 2); // $1900/1: NMI set then clear
    transfer(&mut code, 8, 0x80, 0x4211, 2); // $1902/3: IRQ set then clear
    transfer(&mut code, 0, 0x80, 0x4212, 14); // $1904..11: status, RDIO, results, joy

    // Observe the numeric DMA results with CPU instructions and mask floating bits.
    for (source, result, mask) in [
        (0x1900u16, 0x1800u16, 0x80),
        (0x1901, 0x1801, 0x80),
        (0x1902, 0x1802, 0x80),
        (0x1903, 0x1803, 0x80),
        (0x1904, 0x1804, 0x81),
    ] {
        code.extend([
            0xad,
            source as u8,
            (source >> 8) as u8,
            0x29,
            mask,
            0x8d,
            result as u8,
            (result >> 8) as u8,
        ]);
    }
    for offset in 5..18 {
        save(&mut code, 0x1900 + offset, 0x1800 + offset);
    }
    // CPU readback also proves both DMA acknowledgements remained cleared.
    for (a, result) in [(0x4210u16, 0x1820u16), (0x4211, 0x1821)] {
        code.extend([
            0xad,
            a as u8,
            (a >> 8) as u8,
            0x29,
            0x80,
            0x8d,
            result as u8,
            (result >> 8) as u8,
        ]);
    }
    // Verify completed multiplication separately ($ff * $ab = $aa55).
    write(&mut code, 0x4202, 255);
    write(&mut code, 0x4203, 171);
    transfer(&mut code, 0, 0x80, 0x4216, 2);
    save(&mut code, 0x1912, 0x1822);
    save(&mut code, 0x1913, 0x1823);
    for (a, result) in (0x4302..=0x4306).zip(0x1824..=0x1828) {
        save(&mut code, a, result);
    }
    write(&mut code, 0x18f0, 0xa5);
    code.push(0xdb);
    cartridge(&code)
}

#[test]
fn guest_dma_status_acknowledgement_arithmetic_and_automatic_controller_results() {
    let mut m = Machine::new(&status_guest()).unwrap();
    m.set_buttons(0xa590);
    for instruction in 0..100_000 {
        m.step();
        assert!(m.bus.frames < 2, "status guest frame deadline");
        if m.cpu.stopped {
            assert_eq!(m.bus.peek(0x18f0), 0xa5);
            let results: Vec<_> = (0x1800..0x1812).map(|a| m.bus.peek(a)).collect();
            assert_eq!(
                results,
                [
                    0x80, 0, 0x80, 0, 0x80, 0xa6, 0x0a, 0x09, 0x0f, 0, 0x90, 0xa5, 0, 0, 0, 0, 0,
                    0,
                ]
            );
            let results: Vec<_> = (0x1820..=0x1828).map(|a| m.bus.peek(a)).collect();
            assert_eq!(results, [0, 0, 0x55, 0xaa, 0x18, 0x42, 0, 0, 0]);
            return;
        }
        assert!(instruction < 99_999, "status guest instruction deadline");
    }
}
