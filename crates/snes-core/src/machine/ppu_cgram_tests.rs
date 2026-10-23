//! Original MIT natural-scheduler vectors. Boundaries, restricted destinations
//! and retained phase/latch effects assert model policy, not electrical timing.
use super::*;

fn bus() -> SystemBus {
    let mut b = SystemBus::new(
        Cartridge::load(include_bytes!("../../../../tests/roms/smoke.bin")).unwrap(),
    );
    for (color, bytes) in [(0, [0xa1, 0x32]), (4, [0x12, 0x34]), (5, [0x56, 0x78])] {
        b.write(0x2121, color);
        for byte in bytes {
            b.write(0x2122, byte);
        }
    }
    b.write(0x2121, 4);
    b
}
// Reach even coordinates using timed CPU accesses only, with refresh included.
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
fn pair(b: &SystemBus, r: usize) -> u16 {
    u16::from_le_bytes([b.dma[0][r], b.dma[0][r + 1]])
}

#[test]
fn cpu_cgram_stable_display_hblank_scanline_zero_blank_controls_and_recovery() {
    for (line, h, force, software) in [
        (100, 300, false, false),
        (100, 1200, false, true),
        (0, 300, false, false),
        (0, 1200, false, true),
        (100, 300, true, true),
        (225, 300, false, true),
        (261, 1200, false, true),
    ] {
        let mut b = bus();
        b.write(0x2100, if force { 128 } else { 15 });
        at(&mut b, line * LINE + h);
        assert_eq!(b.read(0x213b), if software { 0x12 } else { 0xa1 });
        assert_eq!(b.read(0x213b), if software { 0x34 } else { 0xb2 });
        b.write(0x2122, 0x65);
        assert_eq!(&b.ppu.cgram[10..12], &[0x56, 0x78]);
        b.write(0x2122, 0xfe);
        assert_eq!(
            &b.ppu.cgram[10..12],
            if software {
                &[0x65, 0x7e]
            } else {
                &[0x56, 0x78]
            }
        );
        assert_eq!(b.ppu.cgram_port_state(), (12, 0x65));
        b.write(0x2100, 128);
        b.write(0x2121, 5);
        assert_eq!(b.read(0x213b), if software { 0x65 } else { 0x56 });
        // A subsequent high write commits the retained low latch, even after reads.
        b.write(0x2122, 0x87);
        assert_eq!(&b.ppu.cgram[10..12], &[0x65, 7]);
        assert_eq!(b.ppu.cgram_port_state(), (12, 0x65));
    }
}

#[test]
fn cpu_cgram_both_phases_before_at_after_model_access_boundaries() {
    for boundary in [100 * LINE + 1096, 101 * LINE, 225 * LINE, 262 * LINE] {
        for offset in [-2i64, 0, 2] {
            for force in [false, true] {
                for high in [false, true] {
                    for operation in [0x2121, 0x2122, 0x213b] {
                        let mut b = bus();
                        if high {
                            assert_eq!(b.read(0x213b), 0x12);
                        }
                        b.write(0x2100, if force { 128 } else { 15 });
                        let end = boundary.wrapping_add_signed(offset);
                        at(&mut b, end - 6);
                        let software = force || (end / LINE) % 262 >= 225 || end % LINE >= 1096;
                        match operation {
                            0x2121 => b.write(operation, 0xff),
                            0x2122 => b.write(operation, 0xfe),
                            _ => {
                                let expected = match (software, high) {
                                    (true, false) => 0x12,
                                    (true, true) => 0x34,
                                    (false, false) => 0xa1,
                                    (false, true) => 0x32,
                                };
                                assert_eq!(b.read(operation), expected);
                            }
                        }
                        assert_eq!(b.clocks, end);
                        assert_eq!(
                            b.ppu.cgram_port_state(),
                            if operation == 0x2121 {
                                (510, 0x56)
                            } else {
                                (
                                    if high { 10 } else { 9 },
                                    if operation == 0x2122 && !high {
                                        0xfe
                                    } else {
                                        0x56
                                    },
                                )
                            }
                        );
                        let commit = operation == 0x2122 && high;
                        assert_eq!(
                            &b.ppu.cgram[8..10],
                            if commit && software {
                                &[0x56, 0x7e]
                            } else {
                                &[0x12, 0x34]
                            }
                        );
                        assert_eq!(
                            &b.ppu.cgram[..2],
                            if commit && !software {
                                &[0x56, 0x7e]
                            } else {
                                &[0xa1, 0x32]
                            }
                        );
                    }
                }
            }
        }
    }
}

fn dma(b: &mut SystemBus, reverse: bool, count: u8) {
    for (r, v) in [
        (0, if reverse { 0x82 } else { 2 }),
        (1, if reverse { 0x3b } else { 0x22 }),
        (2, 0),
        (3, 4),
        (4, 0x7e),
        (5, count),
        (6, 0),
    ] {
        b.write(0x4300 + r, v);
    }
}
#[test]
fn cgram_repeated_port_forward_reverse_dma_payload_phase_and_progression() {
    for (line, h, force, software) in [
        (100, 300, false, false),
        (100, 1200, false, true),
        (0, 900, false, false),
        (100, 300, true, true),
        (225, 300, false, true),
    ] {
        for reverse in [false, true] {
            let mut b = bus();
            for (i, v) in [0x65, 0xfe, 0x9a].into_iter().enumerate() {
                b.write(0x7e0400 + i as u32, v);
            }
            dma(&mut b, reverse, 3);
            b.write(0x2100, if force { 128 } else { 15 });
            at(&mut b, line * LINE + h);
            b.write(0x420b, 1);
            let start = b.clocks;
            b.read(0x4305);
            assert_eq!(b.clocks - start, 46); // 16 overhead + 24 payload + 6 CPU.
            assert_eq!((pair(&b, 2), pair(&b, 5), b.dma[0][4]), (0x403, 0, 0x7e));
            assert_eq!((b.io[11], b.dma_pending), (0, 0));
            assert_eq!(
                b.ppu.cgram_port_state(),
                (11, if reverse { 0x56 } else { 0x9a })
            );
            if reverse {
                assert_eq!(
                    &b.wram[0x400..0x403],
                    if software {
                        &[0x12, 0x34, 0x56]
                    } else {
                        &[0xa1, 0xb2, 0xa1]
                    }
                );
                assert_eq!(&b.ppu.cgram[8..12], &[0x12, 0x34, 0x56, 0x78]);
            } else {
                assert_eq!(
                    &b.ppu.cgram[8..10],
                    if software {
                        &[0x65, 0x7e]
                    } else {
                        &[0x12, 0x34]
                    }
                );
                assert_eq!(
                    &b.ppu.cgram[..2],
                    if software {
                        &[0xa1, 0x32]
                    } else {
                        &[0x65, 0x7e]
                    }
                );
                assert_eq!(&b.ppu.cgram[10..12], &[0x56, 0x78]); // Third byte stages only.
            }
        }
    }
}

#[test]
fn cgram_dma_sampling_and_commit_straddle_model_access_boundaries() {
    for boundary in [100 * LINE + 1096, 101 * LINE, 225 * LINE, 262 * LINE] {
        for offset in [-2i64, 0, 2] {
            for reverse in [false, true] {
                let mut b = bus();
                b.write(0x2122, 0x65); // Odd phase; pending low byte.
                b.write(0x7e0400, 0xfe);
                dma(&mut b, reverse, 1);
                b.write(0x2100, 15);
                let access = boundary.wrapping_add_signed(offset);
                at(&mut b, access - if reverse { 26 } else { 30 });
                b.write(0x420b, 1);
                b.read(0x4305);
                let software = (access / LINE) % 262 >= 225 || access % LINE >= 1096;
                assert_eq!(b.clocks, access + if reverse { 10 } else { 6 });
                assert_eq!((pair(&b, 2), pair(&b, 5)), (0x401, 0));
                assert_eq!((b.io[11], b.dma_pending), (0, 0));
                assert_eq!(b.ppu.cgram_port_state(), (10, 0x65));
                if reverse {
                    assert_eq!(b.wram[0x400], if software { 0x34 } else { 0x32 });
                } else {
                    assert_eq!(
                        &b.ppu.cgram[8..10],
                        if software {
                            &[0x65, 0x7e]
                        } else {
                            &[0x12, 0x34]
                        }
                    );
                    assert_eq!(
                        &b.ppu.cgram[..2],
                        if software {
                            &[0xa1, 0x32]
                        } else {
                            &[0x65, 0x7e]
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn cgram_direct_indirect_forward_reverse_hdma_hblank_progression_and_termination() {
    for indirect in [false, true] {
        for reverse in [false, true] {
            for force in [false, true] {
                let mut b = bus();
                at(&mut b, 225 * LINE + 100);
                for (i, v) in if indirect {
                    vec![0x82, 0, 4, 0, 0]
                } else {
                    vec![0x82, 0x56, 0x78, 0x9a, 0xbc, 0]
                }
                .into_iter()
                .enumerate()
                {
                    b.write(0x7e0300 + i as u32, v);
                }
                for (i, v) in [0x56, 0x78, 0x9a, 0xbc].into_iter().enumerate() {
                    b.write(0x7f0400 + i as u32, v);
                }
                for (r, v) in [
                    (
                        0,
                        2 | if indirect { 0x40 } else { 0 } | if reverse { 0x80 } else { 0 },
                    ),
                    (1, if reverse { 0x3b } else { 0x22 }),
                    (2, 0),
                    (3, 3),
                    (4, 0x7e),
                    (5, 0),
                    (6, 0),
                    (7, 0x7f),
                ] {
                    b.write(0x4300 + r, v);
                }
                b.write(0x2121, 4);
                b.write(0x2100, if force { 128 } else { 15 });
                b.write(0x420c, 1);
                at(&mut b, 262 * LINE + 1112);
                b.read(0x430a); // Scanline-0 HBlank, legal even with display enabled.
                let destination = if indirect { 0x10400 } else { 0x301 };
                if reverse {
                    assert_eq!(&b.wram[destination..destination + 2], &[0x12, 0x34]);
                } else {
                    assert_eq!(&b.ppu.cgram[8..10], &[0x56, 0x78]);
                }
                assert_eq!(b.ppu.cgram_port_state(), (10, 0x56));
                assert_eq!(
                    (pair(&b, 8), pair(&b, 5), pair(&b, 2)),
                    (0x303, if indirect { 0x402 } else { 0 }, 0x300)
                );
                assert_eq!((b.dma[0][10], b.hdma_active, b.io[12]), (0x81, 1, 1));
                at(&mut b, 263 * LINE + 1112);
                b.read(0x430a);
                if reverse {
                    assert_eq!(&b.wram[destination + 2..destination + 4], &[0x56, 0x78]);
                } else {
                    assert_eq!(&b.ppu.cgram[10..12], &[0x9a, 0x3c]);
                }
                assert_eq!(&b.ppu.cgram[..2], &[0xa1, 0x32]);
                assert_eq!(
                    b.ppu.cgram_port_state(),
                    (12, if reverse { 0x56 } else { 0x9a })
                );
                assert_eq!(
                    (pair(&b, 8), pair(&b, 5), pair(&b, 2)),
                    (if indirect { 0x305 } else { 0x306 }, 0, 0x300)
                );
                assert_eq!((b.dma[0][10], b.hdma_active, b.io[12]), (0, 0, 1));
                assert_eq!((b.dma[0][4], b.dma[0][7]), (0x7e, 0x7f));
                at(&mut b, 264 * LINE + 1112);
                b.read(0x430a);
                assert_eq!(pair(&b, 8), if indirect { 0x305 } else { 0x306 });
                assert_eq!(b.ppu.cgram_port_state().0, 12);
            }
        }
    }
}

#[test]
fn cgram_pair_destination_is_selected_on_second_write_across_ownership_change() {
    for (boundary, offset, software) in [(100 * LINE + 1096, -4i64, true), (101 * LINE, -4, false)]
    {
        let mut b = bus();
        b.write(0x2100, 15);
        let low = boundary.wrapping_add_signed(offset);
        at(&mut b, low - 6);
        b.write(0x2122, 0x87);
        assert_eq!(b.clocks, low);
        assert_eq!(b.ppu.cgram_port_state(), (9, 0x87));
        assert_eq!(&b.ppu.cgram[8..10], &[0x12, 0x34]);
        assert_eq!(&b.ppu.cgram[..2], &[0xa1, 0x32]);
        b.write(0x2122, 0xff); // The pair belongs to the high-write destination.
        assert_eq!(b.clocks, boundary + 2);
        assert_eq!(b.ppu.cgram_port_state(), (10, 0x87));
        assert_eq!(
            &b.ppu.cgram[8..10],
            if software {
                &[0x87, 0x7f]
            } else {
                &[0x12, 0x34]
            }
        );
        assert_eq!(
            &b.ppu.cgram[..2],
            if software {
                &[0xa1, 0x32]
            } else {
                &[0x87, 0x7f]
            }
        );
    }
}
