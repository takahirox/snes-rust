//! Original MIT 65C816 guest distinguishing H=0 and H=40 automatic reload.
//! Copyright (c) 2026 snes-rust contributors.
use snes_core::Machine;

fn write(c: &mut Vec<u8>, address: u16, value: u8) {
    c.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
}
fn save(c: &mut Vec<u8>, port: u16, result: u16) {
    c.extend([
        0xad,
        port as u8,
        (port >> 8) as u8,
        0x8d,
        result as u8,
        (result >> 8) as u8,
    ]);
}
fn guest() -> Vec<u8> {
    let mut c = vec![0x78]; // SEI: masked IRQ wakes WAI without vectoring.
    for (a, v) in [
        (0x2102, 4),
        (0x2103, 0),
        (0x2104, 0x12),
        (0x2104, 0x34),
        (0x2104, 0x56),
        (0x2104, 0x78),
        (0x2100, 15),
    ] {
        write(&mut c, a, v);
    }
    // H/V IRQ at V=224, HTIME=327: 327*4+14=1322 master clocks.
    for (a, v) in [
        (0x4207, 0x47),
        (0x4208, 1),
        (0x4209, 224),
        (0x420a, 0),
        (0x4200, 0x30),
    ] {
        write(&mut c, a, v);
    }
    c.push(0xcb);
    // WAI idles in six-clock steps, waking at H=1322..1326. LDA immediate
    // costs 16, STA $2102 costs 30: address reload completes V=225/H=4..8.
    // Next LDA $2138 completes H=34..38, then STA WRAM crosses H=40.
    // Later LDA completes H=96..100 and must read the restored even byte again.
    // This uses software-set cursors in VBlank, independent of rendering bytes.
    write(&mut c, 0x2102, 4);
    save(&mut c, 0x2138, 0x1800);
    save(&mut c, 0x2138, 0x1801);
    save(&mut c, 0x2138, 0x1802);
    save(&mut c, 0x2138, 0x1803);
    write(&mut c, 0x18f0, 0xa5);
    c.push(0xdb);
    let mut rom = vec![0; 32768];
    rom[..c.len()].copy_from_slice(&c);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"OAM H40 ORIGINAL     ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&[0, 0x80]);
    rom
}

#[test]
fn cpu_guest_reads_before_h40_then_observes_one_even_reload() {
    let mut m = Machine::new(&guest()).unwrap();
    for _ in 0..100_000 {
        m.step();
        assert!(m.bus.frames < 2, "guest frame deadline");
        if m.cpu.stopped {
            assert_eq!(m.bus.peek(0x18f0), 0xa5);
            let results: Vec<_> = (0x1800..=0x1803).map(|a| m.bus.peek(a)).collect();
            assert_eq!(results, [0x12, 0x12, 0x34, 0x56]);
            return;
        }
    }
    panic!("guest instruction deadline");
}
