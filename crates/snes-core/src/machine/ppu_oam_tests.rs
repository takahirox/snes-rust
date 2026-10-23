//! Original MIT natural-scheduler vectors. Timing edges and restricted numeric
//! addresses/bytes assert emulator policy, not physical access strobes.
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
                // A VBlank reload between reverse source and destination resets
                // the progressed cursor, but cannot change the sampled byte.
                if reverse {
                    assert_eq!(b.wram[0x400], if available { 0x12 } else { 0xa1 });
                } else {
                    assert_eq!(&b.ppu.oam[8..10], &[0x12, 0x34]);
                    assert_eq!(b.ppu.oam[512], if available { 0xa1 } else { 0x65 });
                }
                let crossed_after_access = offset == -2;
                let expected = if crossed_after_access && boundary == 225 * LINE {
                    (8, 8)
                } else if crossed_after_access {
                    (513, 9)
                } else {
                    (if available { 9 } else { 513 }, 9)
                };
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
