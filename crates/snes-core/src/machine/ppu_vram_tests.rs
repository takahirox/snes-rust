//! Original MIT natural-scheduler tests. Clock boundaries are model conventions;
//! invalid-fetch retention is approximation policy, not electrical evidence.
use super::*;

fn bus(high: bool) -> SystemBus {
    let mut b = SystemBus::new(
        Cartridge::load(include_bytes!("../../../../tests/roms/smoke.bin")).unwrap(),
    );
    b.write(0x2115, 0x80);
    for (address, value) in [(0u16, 0x3412u16), (1, 0x7856), (2, 0xbc9a), (0x101, 0xf0de)] {
        b.write(0x2116, address as u8);
        b.write(0x2117, (address >> 8) as u8);
        b.write(0x2118, value as u8);
        b.write(0x2119, (value >> 8) as u8);
    }
    b.write(0x2116, 0);
    b.write(0x2117, 0);
    b.read(0x213a); // B=$3412, live VMADD=1.
    b.write(0x2115, if high { 0x80 } else { 0 });
    b
}
// Reach exact even master clocks exclusively through ordinary timed accesses.
fn at(b: &mut SystemBus, target: u64) {
    while b.clocks < target {
        match target - b.clocks {
            n if n >= 8 && n % 6 != 0 => {
                b.read(0x7e1000);
            }
            n if n >= 6 => b.idle(),
            n => panic!("unreachable clock gap {n}"),
        }
    }
    assert_eq!(b.clocks, target);
}
fn state(b: &SystemBus, address: u16, buffer: u16) {
    assert_eq!(b.ppu.vram_read_state(), (address, buffer));
}
fn pair(b: &SystemBus, r: usize) -> u16 {
    u16::from_le_bytes([b.dma[0][r], b.dma[0][r + 1]])
}

#[test]
fn cpu_prefetch_and_refill_before_at_after_vblank_and_frame_wrap() {
    for boundary in [225 * LINE, 262 * LINE] {
        for offset in [-2i64, 0, 2] {
            for force in [false, true] {
                for high in [false, true] {
                    // Both address writes and both read ports at each boundary.
                    for operation in [0x2116, 0x2117, 0x2139, 0x213a] {
                        let mut b = bus(high);
                        b.write(0x2100, if force { 0x80 } else { 15 });
                        let end = boundary.wrapping_add_signed(offset);
                        at(&mut b, end - 6);
                        let selected = operation == if high { 0x213a } else { 0x2139 };
                        let available = force || (end / LINE) % 262 >= 225;
                        let (address, fetched) = match operation {
                            0x2116 => {
                                b.write(operation, 2);
                                (2, 0xbc9a)
                            }
                            0x2117 => {
                                b.write(operation, 1);
                                (0x101, 0xf0de)
                            }
                            _ => {
                                assert_eq!(
                                    b.read(operation),
                                    if operation == 0x2139 { 0x12 } else { 0x34 }
                                );
                                (if selected { 2 } else { 1 }, 0x7856)
                            }
                        };
                        assert_eq!(b.clocks, end);
                        let buffer = if available && (operation < 0x2139 || selected) {
                            fetched
                        } else {
                            0x3412
                        };
                        state(&b, address, buffer);
                    }
                }
            }
        }
    }
}

#[test]
fn cpu_stable_active_hblank_forced_blank_changes_and_recovery() {
    for h in [300, 1200] {
        for high in [false, true] {
            let mut b = bus(high);
            b.write(0x2100, 15);
            at(&mut b, 100 * LINE + h);
            b.write(0x2116, 2);
            b.write(0x2117, 1); // VMADD=$0102, no ordinary memory fetch.
            state(&b, 0x102, 0x3412);
            let selected = if high { 0x213a } else { 0x2139 };
            let other = if high { 0x2139 } else { 0x213a };
            assert_eq!(b.read(other), if high { 0x12 } else { 0x34 });
            state(&b, 0x102, 0x3412);
            assert_eq!(b.read(selected), if high { 0x34 } else { 0x12 });
            state(&b, 0x103, 0x3412);
            b.write(0x2100, 0x80); // Immediate effects are a scheduler convention.
            b.write(0x2116, 1);
            state(&b, 0x101, 0xf0de);
            b.write(0x2100, 15);
            b.read(selected);
            state(&b, 0x102, 0xf0de);
            at(&mut b, 225 * LINE + 100);
            b.write(0x2117, 0); // $0002, accessible again.
            state(&b, 2, 0xbc9a);
        }
    }
}

fn dma(b: &mut SystemBus, mode: u8, port: u8, count: u8) {
    for (r, value) in [
        (0, mode),
        (1, port),
        (2, 0),
        (3, 4),
        (4, 0x7e),
        (5, count),
        (6, 0),
    ] {
        b.write(0x4300 + r, value);
    }
}
#[test]
fn reverse_dma_source_sampling_before_at_after_boundaries_and_commit_straddles() {
    for boundary in [225 * LINE, 262 * LINE] {
        for offset in [-2i64, 0, 2] {
            for high in [false, true] {
                let mut b = bus(high);
                b.write(0x2100, 15);
                dma(&mut b, 0x80, if high { 0x3a } else { 0x39 }, 1);
                let sample = boundary.wrapping_add_signed(offset);
                // DMA overhead 16, then source +4, destination +8. For -2,
                // source and destination are in different display states.
                at(&mut b, sample - 26);
                b.write(0x420b, 1);
                b.read(0x4305); // Services the armed DMA through the ordinary bus.
                assert_eq!(b.clocks, sample + 10);
                assert_eq!(b.wram[0x400], if high { 0x34 } else { 0x12 });
                let accessible = (sample / LINE) % 262 >= 225;
                state(&b, 2, if accessible { 0x7856 } else { 0x3412 });
                assert_eq!((pair(&b, 2), pair(&b, 5), b.dma[0][4]), (0x401, 0, 0x7e));
                assert_eq!(b.io[11], 0);
            }
        }
    }
}
#[test]
fn reverse_dma_alternating_ports_active_hblank_and_accessible_controls() {
    for (line, h, force, accessible) in [
        (100, 300, false, false),
        (100, 1200, false, false),
        (225, 300, false, true),
        (100, 1200, true, true),
    ] {
        let mut b = bus(true);
        b.write(0x2100, if force { 0x80 } else { 15 });
        dma(&mut b, 0x81, 0x39, 4);
        at(&mut b, line * LINE + h);
        b.write(0x420b, 1);
        b.read(0x4305);
        assert_eq!(
            &b.wram[0x400..0x404],
            if accessible {
                &[0x12, 0x34, 0x56, 0x78]
            } else {
                &[0x12, 0x34, 0x12, 0x34]
            }
        );
        state(&b, 3, if accessible { 0xbc9a } else { 0x3412 });
        assert_eq!((pair(&b, 2), pair(&b, 5)), (0x404, 0));
    }
}

#[test]
fn direct_indirect_reverse_hdma_hblank_controls_and_table_progression() {
    for indirect in [false, true] {
        for force in [false, true] {
            for high in [false, true] {
                let mut b = bus(high);
                // Program before next frame's natural initialization, then prime
                // the buffer in VBlank. No scheduler/HDMA state injection.
                at(&mut b, 225 * LINE + 100);
                b.write(0x2100, if force { 0x80 } else { 15 });
                for (i, value) in if indirect {
                    vec![0x82, 0, 4, 0, 0]
                } else {
                    vec![0x82, 0, 0, 0, 0, 0]
                }
                .into_iter()
                .enumerate()
                {
                    b.write(0x7e0300 + i as u32, value);
                }
                for (r, value) in [
                    (0, if indirect { 0xc1 } else { 0x81 }),
                    (1, 0x39),
                    (2, 0),
                    (3, 3),
                    (4, 0x7e),
                    (5, 0),
                    (6, 0),
                    (7, 0x7f),
                ] {
                    b.write(0x4300 + r, value);
                }
                b.write(0x2116, 0);
                b.write(0x2117, 0);
                b.write(0x2115, 0x80);
                b.read(0x213a); // B0, address1.
                b.write(0x2115, if high { 0x80 } else { 0 });
                b.write(0x420c, 1);
                at(&mut b, 262 * LINE + 1112);
                b.read(0x430a); // Services scanline-zero HBlank payload.
                let destination = if indirect { 0x10400 } else { 0x301 };
                assert_eq!(
                    &b.wram[destination..destination + 2],
                    if force && !high {
                        &[0x12, 0x78]
                    } else {
                        &[0x12, 0x34]
                    }
                );
                state(&b, 2, if force { 0x7856 } else { 0x3412 });
                assert_eq!(pair(&b, 8), 0x303);
                assert_eq!(pair(&b, 5), if indirect { 0x402 } else { 0 });
                assert_eq!(pair(&b, 2), 0x300);
                assert_eq!(b.dma[0][10], 0x81);
                assert_eq!(b.io[12], 1);
                assert_eq!(b.hdma_active, 1);
                at(&mut b, 263 * LINE + 1112);
                b.read(0x430a); // Visible line 1 HBlank, then row-end termination.
                assert_eq!(
                    &b.wram[destination + 2..destination + 4],
                    if !force {
                        &[0x12, 0x34]
                    } else if high {
                        &[0x56, 0x78]
                    } else {
                        &[0x56, 0xbc]
                    }
                );
                state(&b, 3, if force { 0xbc9a } else { 0x3412 });
                assert_eq!(pair(&b, 8), if indirect { 0x305 } else { 0x306 });
                assert_eq!(pair(&b, 5), 0); // Indirect zero-header pointer reload.
                assert_eq!(pair(&b, 2), 0x300);
                assert_eq!(b.dma[0][10], 0);
                assert_eq!(b.hdma_active, 0);
                assert_eq!(b.io[12], 1); // Termination preserves programmed enable.
            }
        }
    }
}
