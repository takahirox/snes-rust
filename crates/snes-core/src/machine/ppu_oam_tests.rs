//! Original MIT natural-scheduler vectors. Timing edges and restricted numeric
//! addresses/bytes assert emulator policy, not physical access strobes.
//! Coincident raster events precede CPU/DMA sampling or commitment in this
//! model; physical strobe ordering and forced-blank latency remain unverified.
use super::*;

fn bus() -> SystemBus {
    let mut b = SystemBus::new(
        Cartridge::load(include_bytes!("../../../../tests/roms/smoke.bin")).unwrap(),
    );
    for (word, bytes) in [
        (4, [0x12, 0x34]),
        (0x100, [0xa1, 0xb2]),
        (0x101, [0xc3, 0xd4]),
    ] {
        address(&mut b, word);
        for byte in bytes {
            b.write(0x2104, byte);
        }
    }
    address(&mut b, 4);
    b
}
fn address(b: &mut SystemBus, word: u16) {
    b.write(0x2102, word as u8);
    b.write(0x2103, (word >> 8) as u8);
}
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
fn cpu_active_hblank_scanline_zero_and_permitted_controls() {
    for (line, h, force, available) in [
        (100, 300, false, false),
        (100, 1200, false, false),
        (0, 800, false, false),
        (0, 1200, false, false),
        (100, 300, true, true),
        (100, 1200, true, true),
        (225, 300, false, true),
        (261, 1200, false, true),
    ] {
        let mut b = bus();
        b.write(0x2100, if force { 128 } else { 15 });
        at(&mut b, line * LINE + h);
        address(&mut b, 4);
        assert_eq!(b.ppu.oam_port_state(), (8, 8, 0xc3));
        assert_eq!(b.read(0x2138), if available { 0x12 } else { 0xa1 });
        b.write(0x2104, 0x77); // Odd write, read did not replace latch.
        assert_eq!(
            &b.ppu.oam[8..10],
            if available {
                &[0xc3, 0x77]
            } else {
                &[0x12, 0x34]
            }
        );
        assert_eq!(
            b.ppu.oam_port_state(),
            (if available { 10 } else { 514 }, 10, 0xc3)
        );
        b.write(0x2104, 0x66); // Low-even latch or policy high-even commit/latch.
        b.write(0x2100, 128);
        assert_eq!(
            b.ppu.oam_port_state(),
            (if available { 11 } else { 513 }, 11, 0x66)
        );
        address(&mut b, 4);
        assert_eq!(b.read(0x2138), if available { 0xc3 } else { 0x12 });
        assert_eq!(b.read(0x2138), if available { 0x77 } else { 0x34 });
    }
}

#[test]
fn cpu_read_write_and_both_address_registers_before_at_after_vblank_and_wrap() {
    for boundary in [225 * LINE, 262 * LINE] {
        for offset in [-2i64, 0, 2] {
            for force in [false, true] {
                for operation in [0x2102, 0x2103, 0x2104, 0x2138] {
                    let mut b = bus();
                    b.write(0x2100, if force { 128 } else { 15 });
                    let end = boundary.wrapping_add_signed(offset);
                    at(&mut b, end - 12);
                    b.write(0x2102, 4); // Whole reload, even phase before edge.
                    let available = force || (end / LINE) % 262 >= 225;
                    match operation {
                        0x2102 => b.write(operation, 5),
                        0x2103 => b.write(operation, 1),
                        0x2104 => b.write(operation, 0x65),
                        _ => {
                            assert_eq!(b.read(operation), if available { 0x12 } else { 0xa1 });
                        }
                    }
                    assert_eq!(b.clocks, end);
                    let expected = match operation {
                        0x2102 => (10, 10, 0xc3),
                        0x2103 => (520, 520, 0xc3),
                        0x2104 => (if available { 9 } else { 513 }, 9, 0x65),
                        _ => (if available { 9 } else { 513 }, 9, 0xc3),
                    };
                    assert_eq!(b.ppu.oam_port_state(), expected);
                    assert_eq!(&b.ppu.oam[8..10], &[0x12, 0x34]);
                    assert_eq!(
                        b.ppu.oam[512],
                        if operation == 0x2104 && !available {
                            0x65
                        } else {
                            0xa1
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn natural_vblank_reload_and_old_forced_inidisp_write_rule() {
    for force in [false, true] {
        let mut b = bus();
        b.write(0x2100, if force { 128 } else { 15 });
        at(&mut b, 224 * LINE + 1200);
        address(&mut b, 4);
        b.read(0x2138); // Odd phase before VBlank entry.
        at(&mut b, 225 * LINE + 100);
        assert_eq!(
            b.ppu.oam_port_state(),
            if force { (9, 9, 0xc3) } else { (8, 8, 0xc3) }
        );
        if force {
            b.write(0x2100, 128); // Anomie's old-F rule includes F→F.
            assert_eq!(b.ppu.oam_port_state(), (8, 8, 0xc3));
        }
        b.read(0x2138);
        b.write(0x2100, 15);
        assert_eq!(
            b.ppu.oam_port_state(),
            if force { (8, 8, 0xc3) } else { (9, 9, 0xc3) }
        );
        at(&mut b, 262 * LINE);
        assert_eq!(
            b.ppu.oam_port_state(),
            if force {
                (512, 8, 0xc3)
            } else {
                (513, 9, 0xc3)
            }
        );
    }
}

fn dma(b: &mut SystemBus, reverse: bool, count: u8) {
    for (r, v) in [
        (0, if reverse { 0x82 } else { 2 }),
        (1, if reverse { 0x38 } else { 4 }),
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
fn forward_reverse_dma_active_hblank_and_permitted_controls() {
    for (line, h, force, available) in [
        (100, 300, false, false),
        (100, 1200, false, false),
        (0, 900, false, false),
        (100, 1200, true, true),
        (225, 300, false, true),
    ] {
        for reverse in [false, true] {
            let mut b = bus();
            b.write(0x7e0400, 0x56);
            b.write(0x7e0401, 0x78);
            dma(&mut b, reverse, 2);
            b.write(0x2100, if force { 128 } else { 15 });
            at(&mut b, line * LINE + h);
            address(&mut b, 4);
            b.write(0x420b, 1);
            let start = b.clocks;
            b.read(0x4305); // Natural pending-DMA service, overhead + payload.
            assert_eq!(b.clocks - start, 38); // 16 + 2*8 + CPU read 6.
            assert_eq!((pair(&b, 2), pair(&b, 5), b.dma[0][4]), (0x402, 0, 0x7e));
            assert_eq!((b.io[11], b.dma_pending), (0, 0));
            assert_eq!(
                b.ppu.oam_port_state(),
                (
                    if available { 10 } else { 514 },
                    10,
                    if reverse { 0xc3 } else { 0x56 }
                )
            );
            if reverse {
                assert_eq!(
                    &b.wram[0x400..0x402],
                    if available {
                        &[0x12, 0x34]
                    } else {
                        &[0xa1, 0xb2]
                    }
                );
                assert_eq!(&b.ppu.oam[8..10], &[0x12, 0x34]);
            } else {
                assert_eq!(
                    &b.ppu.oam[8..10],
                    if available {
                        &[0x56, 0x78]
                    } else {
                        &[0x12, 0x34]
                    }
                );
                assert_eq!(
                    &b.ppu.oam[512..514],
                    if available {
                        &[0xa1, 0xb2]
                    } else {
                        &[0x56, 0x78]
                    }
                );
            }
        }
    }
}

#[test]
fn dma_source_and_destination_sampling_straddle_blanking_boundaries() {
    for boundary in [225 * LINE, 262 * LINE] {
        for offset in [-2i64, 0, 2] {
            for reverse in [false, true] {
                let mut b = bus();
                b.write(0x7e0400, 0x65);
                dma(&mut b, reverse, 1);
                b.write(0x2100, 15);
                let access = boundary.wrapping_add_signed(offset);
                // Arm +6, overhead +16, source +4 / destination +8.
                at(&mut b, access - if reverse { 26 } else { 30 });
                b.write(0x420b, 1);
                b.read(0x4305);
                let available = (access / LINE) % 262 >= 225;
                assert_eq!(b.clocks, access + if reverse { 10 } else { 6 });
                assert_eq!((pair(&b, 2), pair(&b, 5)), (0x401, 0));
                // H=0 grants VBlank access but does not restore the cursor.
                // At frame wrap, rendering owns the address again. Neither
                // boundary is the H=40 automatic reload tested separately.
                let software_address = available && boundary != 225 * LINE;
                if reverse {
                    assert_eq!(b.wram[0x400], if software_address { 0x12 } else { 0xa1 });
                } else {
                    assert_eq!(&b.ppu.oam[8..10], &[0x12, 0x34]);
                    assert_eq!(b.ppu.oam[512], if software_address { 0xa1 } else { 0x65 });
                }
                let crossed_after_access = offset == -2;
                let expected = (
                    if software_address && !crossed_after_access {
                        9
                    } else {
                        513
                    },
                    9,
                );
                let (port, rotation, latch) = b.ppu.oam_port_state();
                assert_eq!((port, rotation), expected);
                assert_eq!(latch, if reverse { 0xc3 } else { 0x65 });
            }
        }
    }
}

#[test]
fn direct_indirect_forward_reverse_hdma_port_effects_progression_and_termination() {
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
                    (1, if reverse { 0x38 } else { 4 }),
                    (2, 0),
                    (3, 3),
                    (4, 0x7e),
                    (5, 0),
                    (6, 0),
                    (7, 0x7f),
                ] {
                    b.write(0x4300 + r, v);
                }
                address(&mut b, 4);
                b.write(0x2100, if force { 128 } else { 15 });
                b.write(0x420c, 1);
                at(&mut b, 262 * LINE + 1112);
                b.read(0x430a); // Scanline 0 payload through shared transfer path.
                let destination = if indirect { 0x10400 } else { 0x301 };
                if reverse {
                    assert_eq!(
                        &b.wram[destination..destination + 2],
                        if force { &[0x12, 0x34] } else { &[0xa1, 0xb2] }
                    );
                } else {
                    assert_eq!(
                        &b.ppu.oam[8..10],
                        if force { &[0x56, 0x78] } else { &[0x12, 0x34] }
                    );
                    assert_eq!(
                        &b.ppu.oam[512..514],
                        if force { &[0xa1, 0xb2] } else { &[0x56, 0x78] }
                    );
                }
                assert_eq!(
                    b.ppu.oam_port_state(),
                    (
                        if force { 10 } else { 514 },
                        10,
                        if reverse { 0xc3 } else { 0x56 }
                    )
                );
                assert_eq!(
                    (pair(&b, 8), pair(&b, 5), pair(&b, 2)),
                    (0x303, if indirect { 0x402 } else { 0 }, 0x300)
                );
                assert_eq!((b.dma[0][10], b.hdma_active, b.io[12]), (0x81, 1, 1));
                at(&mut b, 263 * LINE + 1112);
                b.read(0x430a); // Visible-line HBlank, second payload then zero.
                if reverse {
                    assert_eq!(
                        &b.wram[destination + 2..destination + 4],
                        if force { &[0, 0] } else { &[0xa1, 0xb2] }
                    );
                } else {
                    assert_eq!(
                        &b.ppu.oam[10..12],
                        if force { &[0x9a, 0xbc] } else { &[0, 0] }
                    );
                    assert_eq!(
                        &b.ppu.oam[512..514],
                        if force { &[0xa1, 0xb2] } else { &[0x9a, 0xbc] }
                    );
                }
                assert_eq!(
                    b.ppu.oam_port_state(),
                    (
                        if force { 12 } else { 514 },
                        12,
                        if reverse { 0xc3 } else { 0x9a }
                    )
                );
                assert_eq!(
                    (pair(&b, 8), pair(&b, 5), pair(&b, 2)),
                    (if indirect { 0x305 } else { 0x306 }, 0, 0x300)
                );
                assert_eq!((b.dma[0][10], b.hdma_active, b.io[12]), (0, 0, 1));
                at(&mut b, 264 * LINE + 1112);
                b.read(0x430a); // Terminated channel stays inactive.
                assert_eq!(pair(&b, 8), if indirect { 0x305 } else { 0x306 });
                assert_eq!(b.ppu.oam_port_state().1, 12);
            }
        }
    }
}

#[test]
fn automatic_reload_waits_for_h40_and_is_partition_invariant() {
    for force in [false, true] {
        for partitioned in [false, true] {
            let mut b = bus();
            at(&mut b, 224 * LINE + 1200);
            address(&mut b, 0x104);
            b.write(0x2104, 0x65); // High-even commit/latch, odd cursor 521.
            b.write(0x2100, if force { 128 } else { 15 });
            let old = if force {
                (521, 521, 0x65)
            } else {
                (513, 521, 0x65)
            };
            b.tick(225 * LINE - b.clocks);
            assert_eq!((b.v, b.h), (225, 0));
            assert_eq!(b.ppu.oam_port_state(), old, "H=0 must not reload");
            b.tick(38);
            assert_eq!(b.ppu.oam_port_state(), old, "H=38");
            if partitioned {
                b.tick(1);
                assert_eq!(b.ppu.oam_port_state(), old, "H=39");
                b.tick(1);
            } else {
                b.tick(2);
            }
            let reloaded = if force { old } else { (520, 520, 0x65) };
            assert_eq!(b.ppu.oam_port_state(), reloaded, "H=40");
            b.tick(2);
            assert_eq!(b.ppu.oam_port_state(), reloaded, "H=42");
            assert_eq!(&b.ppu.regs[2..4], &[4, 1]);
            b.read(0x2138);
            let advanced = if force {
                (522, 522, 0x65)
            } else {
                (521, 521, 0x65)
            };
            b.tick(2 * LINE); // Cross several line starts, no repeated reload.
            assert_eq!(b.ppu.oam_port_state(), advanced);
            b.tick(487 * LINE + 38 - b.clocks); // Next frame, V=225/H=38.
            assert_eq!(
                b.ppu.oam_port_state(),
                if force { advanced } else { (513, 521, 0x65) }
            );
            b.tick(2);
            assert_eq!(
                b.ppu.oam_port_state(),
                if force { advanced } else { (520, 520, 0x65) }
            );
        }
    }
}

#[test]
fn automatic_reload_crossing_lines_in_one_advance_matches_partitions() {
    for chunks in [&[2 * LINE + 100][..], &[LINE, 38, 2, LINE + 60][..]] {
        let mut b = bus();
        b.write(0x2100, 15);
        at(&mut b, 224 * LINE);
        address(&mut b, 4);
        b.read(0x2138);
        at(&mut b, 224 * LINE + 100);
        for &n in chunks {
            b.tick(n);
        }
        assert_eq!((b.v, b.h), (226, 200));
        assert_eq!(b.ppu.oam_port_state(), (8, 8, 0xc3));
        assert_eq!(&b.ppu.regs[2..4], &[4, 0]);
        assert_eq!(b.read(0x2138), 0x12);
        assert_eq!(b.ppu.oam_port_state(), (9, 9, 0xc3));
    }
}

#[test]
fn cpu_oam_access_completion_before_at_after_h40() {
    for h in [38, 40, 42] {
        for write in [false, true] {
            let mut b = bus();
            b.write(0x2100, 15);
            at(&mut b, 224 * LINE + 1200);
            b.read(0x2138); // Rendering-owned odd cursor, rotation 9.
            at(&mut b, 225 * LINE + h - 6);
            if write {
                b.write(0x2104, 0x65);
            } else {
                assert_eq!(b.read(0x2138), if h < 40 { 0xb2 } else { 0x12 });
            }
            assert_eq!(b.clocks, 225 * LINE + h);
            assert_eq!(
                b.ppu.oam_port_state(),
                if h < 40 {
                    (514, 10, 0xc3)
                } else {
                    (9, 9, if write { 0x65 } else { 0xc3 })
                }
            );
            if h < 40 {
                b.idle(); // Cross H=40 through an ordinary six-clock CPU idle.
                assert_eq!(b.ppu.oam_port_state(), (8, 8, 0xc3));
            }
            assert_eq!(&b.ppu.oam[8..10], &[0x12, 0x34]);
            assert_eq!(b.ppu.oam[513], if write && h < 40 { 0x65 } else { 0xb2 });
        }
    }
}

#[test]
fn dma_oam_sampling_and_pair_commit_before_at_after_h40() {
    for h in [38, 40, 42] {
        for reverse in [false, true] {
            let mut b = bus();
            b.write(0x7e0400, 0x65);
            b.write(0x7e0401, 0x78);
            dma(&mut b, reverse, 2);
            b.write(0x2100, 15);
            at(&mut b, 224 * LINE + 1200);
            b.read(0x2138); // Odd cursor, distinct from programmed byte 8.

            // Arm +6, overhead +16, source +4 / destination +8.
            // Reverse h names source sampling; forward h names commitment.
            at(&mut b, 225 * LINE + h - if reverse { 26 } else { 30 });
            b.write(0x420b, 1);
            b.read(0x4305); // Two transfers, then the CPU read.
            assert_eq!(b.clocks, 225 * LINE + h + if reverse { 18 } else { 14 });
            assert_eq!((pair(&b, 2), pair(&b, 5), b.dma[0][4]), (0x402, 0, 0x7e));
            assert_eq!((b.io[11], b.dma_pending), (0, 0));
            let cursor = if h < 40 { 9 } else { 10 };
            let latch = if reverse {
                0xc3
            } else if h < 40 {
                0x78
            } else {
                0x65
            };
            assert_eq!(b.ppu.oam_port_state(), (cursor, cursor, latch));
            if reverse {
                // Source at 38 survives reload at 40 before A commitment at 42.
                assert_eq!(
                    &b.wram[0x400..0x402],
                    if h < 40 { &[0xb2, 0x12] } else { &[0x12, 0x34] }
                );
                assert_eq!(&b.ppu.oam[8..10], &[0x12, 0x34]);
            } else {
                // Destination at 42 uses byte 8 after source was sampled at 38.
                assert_eq!(
                    &b.ppu.oam[8..10],
                    if h < 40 { &[0x12, 0x34] } else { &[0x65, 0x78] }
                );
                assert_eq!(b.ppu.oam[513], if h < 40 { 0x65 } else { 0xb2 });
            }
            at(&mut b, 225 * LINE + 100);
            assert_eq!(b.ppu.oam_port_state(), (cursor, cursor, latch));
            b.read(0x2138);
            assert_eq!(b.ppu.oam_port_state(), (cursor + 1, cursor + 1, latch));
        }
    }
}
