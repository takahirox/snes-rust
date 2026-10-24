//! Original MIT CPU-executed phase guests for Issue #35.
//! Copyright (c) 2026 snes-rust contributors.
use snes_core::Machine;

fn write(code: &mut Vec<u8>, address: u16, value: u8) {
    code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
}
fn save(code: &mut Vec<u8>, address: u16, result: u16) {
    code.extend([
        0xad,
        address as u8,
        (address >> 8) as u8,
        0x8d,
        result as u8,
        (result >> 8) as u8,
    ]);
}
fn guest(indirect: bool) -> Vec<u8> {
    let mut code = vec![0x78]; // SEI: IRQ wakes WAI without vectoring.
    write(&mut code, 0x2100, 0x80);
    // LDA $4212; AND #$80; BEQ back: execute until VBlank.
    code.extend([0xad, 0x12, 0x42, 0x29, 0x80, 0xf0, 0xf9]);
    let table: &[u8] = if indirect {
        &[0x81, 0, 0x12, 0x82, 0, 0x13, 0, 0x56]
    } else {
        &[0x81, 0x11, 0x82, 0x22, 0x33, 0]
    };
    for (i, &v) in table.iter().enumerate() {
        write(&mut code, 0x1000 + i as u16, v);
    }
    for (address, value) in [
        (0x1200, 0x11),
        (0x1300, 0x22),
        (0x1301, 0x33),
        (0x4300, if indirect { 0x40 } else { 0 }),
        (0x4301, 0x22),
        (0x4302, 0),
        (0x4303, 0x10),
        (0x4304, 0),
        (0x4305, 0),
        (0x4306, 0),
        (0x4307, 0),
        (0x420c, 1),
        (0x4207, 11),
        (0x4208, 1), // HTIME=267 -> 267*4+14=1082 assertion clocks
        (0x4209, 0),
        (0x420a, 0),
        (0x4200, 0x30), // V=0 only
    ] {
        write(&mut code, address, value);
    }
    // WAI waits for next frame V=0, master H=1082. Six-clock polling
    // wakes at 1082..1086. Slow-ROM LDA abs: 3*8+6=30 clocks; its data access
    // starts at 1106..1110 and completes at 1112..1116. Corrected HDMA
    // cannot service before that read. H=274 baseline services during the
    // operand fetches and returns the already reloaded next-row address.
    code.push(0xcb);
    save(&mut code, 0x4308, 0x1800);
    save(&mut code, 0x4212, 0x1806);
    // Subsequent opcode/operand bus boundaries service the pending event.
    for (address, result) in [0x4308, 0x4309, 0x430a, 0x4305, 0x4306]
        .into_iter()
        .zip(0x1801..)
    {
        save(&mut code, address, result);
    }
    write(&mut code, 0x1807, 0xa5);
    code.push(0xdb);
    let mut rom = vec![0; 32768];
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"HDMA PHASE ORIGINAL  ");
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
            assert_eq!(machine.bus.peek(0x1807), 0xa5);
            let mut result: Vec<_> = (0x1800..=0x1806).map(|a| machine.bus.peek(a)).collect();
            result[6] &= 0xc0; // assert driven H/VBlank bits only
            return result;
        }
    }
    panic!("guest instruction deadline");
}
#[test]
fn direct_guest_observes_pre_phase_table_then_row_end_reload() {
    assert_eq!(run(false), [1, 3, 0x10, 0x82, 0, 0, 0x40]);
}
#[test]
fn indirect_guest_observes_pre_phase_table_then_pointer_reload() {
    assert_eq!(run(true), [3, 6, 0x10, 0x82, 0, 0x13, 0x40]);
}
