//! Original MIT WRIO regressions; clock positioning uses ordinary CPU accesses.
//! Copyright (c) 2026 snes-rust contributors.
use super::*;

fn bus() -> SystemBus {
    SystemBus::new(Cartridge::load(include_bytes!("../../../../tests/roms/smoke.bin")).unwrap())
}
fn at(b: &mut SystemBus, line: u16, h: u64) {
    let target = u64::from(line) * LINE + h;
    while b.clocks < target {
        match target - b.clocks {
            n if n >= 8 && n % 6 != 0 => {
                b.read(0x7e0000);
            }
            n if n >= 6 => b.idle(),
            n => panic!("unreachable clock gap {n}"),
        }
    }
    assert_eq!((b.v, b.h), (line, h));
}
fn held(b: &mut SystemBus, bank: u32) -> [u16; 2] {
    let mut result = [0; 2];
    for (i, r) in [0x213c, 0x213d].into_iter().enumerate() {
        let low = b.read(bank << 16 | r);
        let high = b.read(bank << 16 | r) & 1;
        result[i] = u16::from(low) | u16::from(high) << 8;
    }
    result
}
#[test]
fn wrio_all_edges_unrelated_bits_and_shared_mirrors() {
    for bank in [0x00, 0x3f, 0x80, 0xbf] {
        for old in [0x00, 0x7f, 0x80, 0xff] {
            for new in [0x00, 0x7f, 0x80, 0xff] {
                let mut b = bus();
                assert_eq!(b.read(bank << 16 | 0x4213), 0xff);
                assert_eq!(b.read(0x213f) & 0x40, 0); // No synthetic reset edge.
                at(&mut b, 0, 100);
                b.read(0x2137); // A = 26,0.
                at(&mut b, 0, 194);
                b.write(bank << 16 | 0x4201, old); // old low captures 50,0.
                b.write(bank << 16 | 0x4201, new); // falling edge captures 51,0.
                let expected = if old & 128 != 0 && new & 128 == 0 {
                    51
                } else if old & 128 == 0 {
                    50
                } else {
                    26
                };
                assert_eq!(b.read(0x4213), new);
                assert_eq!(
                    held(&mut b, bank),
                    [expected, 0],
                    "{bank:02x}: {old:02x}->{new:02x}"
                );
            }
        }
    }
}
#[test]
fn crossing_scanline_captures_at_access_completion_and_low_gate_retains() {
    for bank in [0x00, 0x3f, 0x80, 0xbf] {
        for software in [false, true] {
            let mut b = bus();
            at(&mut b, 255, 1360);
            if software {
                b.read(bank << 16 | 0x2137);
            } else {
                b.write(bank << 16 | 0x4201, 0x7f);
            }
            assert_eq!((b.v, b.h), (256, 2));
            assert_eq!(held(&mut b, bank), [0, 256]);
            if software {
                b.write(bank << 16 | 0x4201, 0x7f);
            }
            // Numerical H and V bit 8, with repeated captures in the same line.
            b.write(bank << 16 | 0x4201, 0xff);
            at(&mut b, 256, 1194);
            b.write(bank << 16 | 0x4201, 0x7f); // (300,256)
            at(&mut b, 257, 100);
            b.write(0x7e0010, 0xa5); // seed deterministic CPU open bus
            assert_eq!(b.read(bank << 16 | 0x2137), 0xa5);
            assert_eq!(held(&mut b, bank), [300, 256]);
            for _ in 0..2 {
                assert_eq!(b.read(bank << 16 | 0x213f) & 0x40, 0x40);
            }
            b.write(bank << 16 | 0x4201, 0xff); // rising edge preserves flag and values
            assert_eq!(b.read(bank << 16 | 0x213f) & 0x40, 0x40);
            assert_eq!(b.read(bank << 16 | 0x213f) & 0x40, 0);
            assert_eq!(held(&mut b, bank), [300, 256]);
            b.write(0x7e0010, 0x5a);
            assert_eq!(b.read(bank << 16 | 0x2137), 0x5a);
            assert_ne!(held(&mut b, bank), [300, 256]);
        }
    }
}
#[test]
fn non_system_banks_cannot_drive_or_read_ppu_latches() {
    for bank in [0x40, 0x7d, 0x7e, 0x7f, 0xc0, 0xff] {
        let mut b = bus();
        at(&mut b, 0, 100);
        b.read(0x2137); // 26,0
        b.write(bank << 16 | 0x4201, 0);
        b.read(bank << 16 | 0x2137);
        b.read(bank << 16 | 0x213c); // must not advance selector
        b.read(bank << 16 | 0x213f); // must not acknowledge
        assert_eq!(b.read(0x4213), 0xff);
        assert_eq!(b.read(0x213f) & 0x40, 0x40);
        assert_eq!(held(&mut b, 0), [26, 0]);
    }
}
#[test]
fn machine_new_and_reset_restore_wrio_without_capture_and_preserve_sram() {
    let mut m = Machine::new(include_bytes!("../../../../tests/roms/smoke.bin")).unwrap();
    assert!(!m.bus.cart.sram.is_empty());
    for reset in [false, true] {
        if reset {
            m.bus.write(0x4201, 0);
            m.bus.write(0x4200, 0x81);
            m.bus.wram[0x123] = 0xaa;
            m.bus.cart.sram.fill(0x5a);
            m.reset();
            assert!(m.bus.cart.sram.iter().all(|&v| v == 0x5a));
            assert_eq!(m.bus.wram[0x123], 0);
        }
        assert_eq!(m.bus.read(0x4213), 0xff);
        assert_eq!(m.bus.io[0], 0);
        assert_eq!(m.bus.read(0x213f) & 0x40, 0);
        assert_eq!(held(&mut m.bus, 0), [0, 0]);
        m.bus.read(0x2137);
        assert_eq!(m.bus.read(0x213f) & 0x40, 0x40);
    }
}

#[test]
fn reverse_dma_b_bus_stat78_obeys_gate_and_resets_selectors() {
    for enabled in [false, true] {
        let mut b = bus();
        at(&mut b, 0, 1194);
        b.write(0x4201, 0x7f); // edge at 1200 -> H=300
        b.read(0x213c); // H selector high
        b.read(0x213d); // V selector high
        if enabled {
            b.write(0x4201, 0xff);
        }
        for (r, v) in [
            (0, 0x80),
            (1, 0x3f),
            (2, 0),
            (3, 0x10),
            (4, 0x7e),
            (5, 2),
            (6, 0),
        ] {
            b.write(0x4300 + r, v);
        }
        b.write(0x420b, 1);
        b.idle(); // service two reverse DMA B-bus STAT78 reads
        assert_eq!(b.wram[0x1000] & 0x40, 0x40);
        assert_eq!(b.wram[0x1001] & 0x40, if enabled { 0 } else { 0x40 });
        assert_eq!(held(&mut b, 0), [300, 0]); // both selectors reset
        assert_eq!(&b.dma[0][2..7], &[2, 0x10, 0x7e, 0, 0]);
    }
}
