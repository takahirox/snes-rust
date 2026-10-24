//! Original MIT row-end scheduler regressions for Issue #33.
//! Copyright (c) 2026 snes-rust contributors.
use super::*;

fn bus() -> SystemBus {
    SystemBus::new(Cartridge::load(include_bytes!("../../../../tests/roms/smoke.bin")).unwrap())
}
fn channel(b: &mut SystemBus, i: usize, mode: u8, table: u32, port: u8) {
    b.dma[i] = [0; 16];
    b.dma[i][0] = mode;
    b.dma[i][1] = port;
    b.dma[i][2..4].copy_from_slice(&(table as u16).to_le_bytes());
    b.dma[i][4] = (table >> 16) as u8;
    b.dma[i][7] = 0x7f;
}
fn until(b: &mut SystemBus, line: u16, h: u64) {
    let deadline = b.clocks + LINE * 263;
    while (b.frames, b.v, b.h) < (0, line, h) {
        b.idle();
        assert!(b.clocks < deadline);
    }
    b.events();
}
fn pair(b: &SystemBus, i: usize, r: usize) -> u16 {
    u16::from_le_bytes([b.dma[i][r], b.dma[i][r + 1]])
}
fn configured(indirect: bool) -> SystemBus {
    let mut b = bus();
    channel(&mut b, 0, if indirect { 0x40 } else { 0 }, 0x7e0100, 0x40);
    let table: &[u8] = if indirect {
        &[0x81, 0, 2, 0x82, 0, 3, 0, 0x12]
    } else {
        &[0x81, 0x11, 0x82, 0x22, 0x33, 0]
    };
    b.wram[0x100..0x100 + table.len()].copy_from_slice(table);
    b.wram[0x10200] = 0x11;
    b.wram[0x10300..0x10302].copy_from_slice(&[0x22, 0x33]);
    b.write_raw(0x420c, 1);
    b
}
fn metadata_edits(indirect: bool) {
    let mut b = configured(indirect);
    until(&mut b, 0, 1200);
    assert_eq!(b.apu.input[0], 0x11);
    assert_eq!(pair(&b, 0, 8), if indirect { 0x106 } else { 0x103 });
    assert_eq!(b.dma[0][10], 0x82);
    if indirect {
        assert_eq!(pair(&b, 0, 5), 0x300);
        b.wram[0x103..0x106].copy_from_slice(&[0, 0, 4]);
        b.wram[0x10400] = 0x55;
        b.wram[0x10300] = 0x66;
    } else {
        b.wram[0x102] = 0;
        b.wram[0x103] = 0x66;
    }
    until(&mut b, 1, 1200);
    assert_eq!(b.apu.input[0], 0x66);
    assert_eq!(b.dma[0][10], 0x81);
    until(&mut b, 2, 1200);
    assert_eq!(b.apu.input[0], 0x33);
    assert_eq!((b.dma[0][10], b.hdma_active, b.io[12]), (0, 0, 1));
    assert_eq!(pair(&b, 0, 8), if indirect { 0x108 } else { 0x106 });
    assert_eq!(pair(&b, 0, 5), if indirect { 0x1200 } else { 0 });
    assert_eq!(&b.dma[0][2..5], &[0, 1, 0x7e]);
    assert_eq!(b.dma[0][7], 0x7f);
    until(&mut b, 3, 1200);
    assert_eq!(b.apu.input[0], 0x33);
}

#[test]
fn direct_row_end_prepares_metadata_and_keeps_payload_live() {
    metadata_edits(false);
}
#[test]
fn indirect_row_end_prepares_metadata_and_keeps_payload_live() {
    metadata_edits(true);
}

#[test]
fn every_counter_boundary_decrements_holds_and_terminates_in_the_last_hblank() {
    for indirect in [false, true] {
        for header in [0x01u8, 0x02, 0x80, 0x81, 0x82, 0xff] {
            let mut b = bus();
            channel(&mut b, 0, if indirect { 0x40 } else { 0 }, 0x7e0100, 0x40);
            let lines = if header == 0x80 {
                128
            } else {
                u16::from(header & 127)
            };
            let transfers = if header > 0x80 { lines } else { 1 };
            b.wram[0x100] = header;
            if indirect {
                b.wram[0x101..0x105].copy_from_slice(&[0, 2, 0, 0x34]);
            } else {
                b.wram[0x101..0x101 + usize::from(transfers)].fill(0x5a);
            }
            b.wram[0x10200..0x10200 + usize::from(transfers)].fill(0x5a);
            b.write_raw(0x420c, 1);
            until(&mut b, 0, 200);
            assert_eq!(b.dma[0][10], header);
            for line in 0..lines {
                b.apu.input[0] = 0xee;
                until(&mut b, line, 1200);
                let transfer = line == 0 || header > 0x80;
                assert_eq!(b.apu.input[0], if transfer { 0x5a } else { 0xee });
                let done = line + 1 == lines;
                assert_eq!(
                    b.dma[0][10],
                    if done {
                        0
                    } else {
                        header.wrapping_sub((line + 1) as u8)
                    }
                );
                assert_eq!((b.hdma_active, b.io[12]), (u8::from(!done), 1));
                let consumed = if header > 0x80 { line + 1 } else { 1 };
                assert_eq!(
                    pair(&b, 0, 8),
                    if indirect {
                        if done {
                            0x105
                        } else {
                            0x103
                        }
                    } else {
                        0x101 + consumed + u16::from(done)
                    }
                );
                assert_eq!(
                    pair(&b, 0, 5),
                    if indirect {
                        if done {
                            0x3400
                        } else {
                            0x200 + consumed
                        }
                    } else {
                        0
                    }
                );
            }
            b.apu.input[0] = 0xee;
            until(&mut b, lines, 1200);
            assert_eq!(b.apu.input[0], 0xee);
            assert_eq!(&b.dma[0][2..5], &[0, 1, 0x7e]);
            assert_eq!(b.dma[0][7], 0x7f);
        }
    }
}

#[test]
fn subsequent_fresh_80_transfers_once_then_holds_for_128_lines() {
    for indirect in [false, true] {
        let mut b = configured(indirect);
        b.wram[if indirect { 0x103 } else { 0x102 }] = 0x80;
        if !indirect {
            b.wram[0x104] = 0;
        }
        until(&mut b, 0, 1200);
        assert_eq!((b.apu.input[0], b.dma[0][10]), (0x11, 0x80));
        for line in 1..=128 {
            b.apu.input[0] = 0xee;
            until(&mut b, line, 1200);
            assert_eq!(b.apu.input[0], if line == 1 { 0x22 } else { 0xee });
            assert_eq!(b.dma[0][10], 128 - line as u8);
            assert_eq!(b.hdma_active, u8::from(line != 128));
            assert_eq!(
                pair(&b, 0, 8),
                if indirect {
                    if line == 128 {
                        0x108
                    } else {
                        0x106
                    }
                } else if line == 128 {
                    0x105
                } else {
                    0x104
                }
            );
            assert_eq!(
                pair(&b, 0, 5),
                if indirect {
                    if line == 128 {
                        0x1200
                    } else {
                        0x301
                    }
                } else {
                    0
                }
            );
        }
        assert_eq!(b.io[12], 1);
    }
}

#[test]
fn terminating_indirect_reads_count_only_later_enabled_unterminated_channels() {
    // none, disabled, paused, initial zero, holding, ending now, ended last line.
    for control in 0..7 {
        for reverse in [false, true] {
            let mut b = bus();
            channel(&mut b, 0, if reverse { 0xc0 } else { 0x40 }, 0x7e0100, 0x40);
            b.wram[0x100..0x106].copy_from_slice(&[2, 0, 2, 0, 0x34, 0x12]);
            b.wram[0x10200] = 0x11;
            b.apu.output[0] = 0x77;
            channel(&mut b, 2, 0, 0x7e0200, 0x41);
            b.wram[0x200..0x203].copy_from_slice(&[
                if control == 3 {
                    0
                } else if control == 6 {
                    1
                } else if control == 5 {
                    2
                } else {
                    3
                },
                0x55,
                0,
            ]);
            b.write_raw(0x420c, if control < 2 { 1 } else { 5 });
            until(&mut b, 0, 1200);
            if control == 2 {
                b.write_raw(0x420c, 1);
            }
            assert_eq!(b.dma[0][10], 1);
            assert_eq!(pair(&b, 0, 5), 0x201);
            until(&mut b, 1, 1200); // held line finishes row, then terminates.
            let later = control == 4 || control == 5;
            assert_eq!(pair(&b, 0, 8), if later { 0x106 } else { 0x105 });
            assert_eq!(pair(&b, 0, 5), if later { 0x1234 } else { 0x3400 });
            assert_eq!(b.dma[0][10], 0);
            assert_eq!(
                b.hdma_active,
                if control == 2 || control == 4 { 4 } else { 0 }
            );
            assert_eq!(b.io[12], if control < 3 { 1 } else { 5 });
            if control == 5 {
                assert_eq!(b.hdma_active, 0); // later channel still counted before its zero.
                assert_eq!(pair(&b, 2, 8), 0x203);
                assert_eq!(b.dma[2][10], 0);
            }
            if reverse {
                assert_eq!(b.wram[0x10200], 0x77);
            } else {
                assert_eq!(b.apu.input[0], 0x11);
            }
            assert_eq!(&b.wram[0x103..0x106], &[0, 0x34, 0x12]);
        }
    }
}

#[test]
fn all_payloads_precede_reload_reads_and_reload_channels_keep_priority() {
    let mut b = bus();
    // Channel 2's payload acknowledges IRQ before channel 0's row-end header.
    // A per-channel payload+reload loop would give ch0 a nonzero header instead of zero.
    channel(&mut b, 0, 0, 0x000100, 0x40);
    b.wram[0x100..0x102].copy_from_slice(&[0x81, 0x11]);
    channel(&mut b, 2, 0x40, 0x000200, 0x41);
    b.wram[0x200..0x205].copy_from_slice(&[0x81, 0x11, 0x42, 0, 0]);
    b.dma[2][7] = 0;
    channel(&mut b, 3, 0, 0x000300, 0x42);
    b.wram[0x300..0x303].copy_from_slice(&[0x82, 0, 0]);
    b.write_raw(0x420c, 13);
    until(&mut b, 0, 200);
    // CPU may change the current table registers; no synthetic scheduler state.
    b.write_raw(0x4308, 0x10); // direct payload at $4210, next header at $4211.
    b.write_raw(0x4309, 0x42);
    b.nmi_flag = true;
    b.irq = true;
    until(&mut b, 0, 1200);
    assert_eq!(b.apu.input[..2], [0x82, 0x82]);
    assert!(!b.nmi_flag && !b.irq);
    assert_eq!(
        (b.dma[0][10], b.dma[2][10], b.hdma_active, b.io[12]),
        (0, 0, 8, 13)
    );
    assert_eq!(pair(&b, 0, 8), 0x4212);
    assert_eq!(pair(&b, 2, 8), 0x206);

    let mut b = bus();
    for i in [0, 2] {
        channel(&mut b, i, 0x40, 0x000100 + i as u32 * 0x100, 0x40);
        let t = 0x100 + i * 0x100;
        b.wram[t..t + 3].copy_from_slice(&[0x81, 0, 2]);
        b.dma[i][7] = 0;
    }
    b.write_raw(0x420c, 5);
    until(&mut b, 0, 200);
    for i in [0, 2] {
        b.write_raw(0x4308 + i as u32 * 16, 0x11);
        b.write_raw(0x4309 + i as u32 * 16, 0x42);
    }
    b.irq = true;
    until(&mut b, 0, 1200);
    assert_eq!((b.dma[0][10], b.dma[2][10]), (0x80, 0));
    assert_eq!((b.hdma_active, b.io[12]), (1, 5));
    assert_eq!(b.apu.input[0], 0);
}

#[test]
fn row_end_indirect_status_pointer_reads_are_a_bus_reads_in_both_directions() {
    for reverse in [false, true] {
        let mut b = bus();
        channel(&mut b, 0, if reverse { 0xc0 } else { 0x40 }, 0x000100, 0x40);
        b.wram[0x100..0x103].copy_from_slice(&[0x81, 0, 2]);
        b.wram[0x200] = 0;
        b.dma[0][7] = 0;
        b.apu.output[0] = 0;
        b.write_raw(0x420c, 1);
        until(&mut b, 0, 200);
        b.write_raw(0x4308, 0x10);
        b.write_raw(0x4309, 0x42);
        b.nmi_flag = true;
        b.irq = true;
        until(&mut b, 0, 1200);
        assert_eq!(
            (b.dma[0][10], pair(&b, 0, 5), pair(&b, 0, 8)),
            (0x82, 0x4080, 0x4213)
        );
        assert!(!b.nmi_flag && !b.irq);
        assert_eq!((b.hdma_active, b.io[12]), (1, 1));
        if reverse {
            assert_eq!(b.wram[0x200], 0);
        } else {
            assert_eq!(b.apu.input[0], 0);
        }
    }
}

#[test]
fn table_and_payload_wrap_within_their_banks_in_both_directions() {
    for indirect in [false, true] {
        for reverse in [false, true] {
            let mut b = bus();
            channel(
                &mut b,
                0,
                (u8::from(indirect) * 0x40) | (u8::from(reverse) * 0x80),
                if indirect { 0x7efffa } else { 0x7efffd },
                0x40,
            );
            if indirect {
                b.wram[0xfffa..0x10000].copy_from_slice(&[0x81, 0xff, 0xff, 0x82, 0xff, 0xff]);
                b.wram[0..2].copy_from_slice(&[0, 0x56]);
                b.wram[0x1ffff] = 0x22;
                b.wram[0x10000] = 0x33;
            } else {
                b.wram[0xfffd..0x10000].copy_from_slice(&[0x81, 0x11, 0x82]);
                b.wram[0..3].copy_from_slice(&[0x22, 0x33, 0]);
            }
            b.apu.output[0] = 0x66;
            b.write_raw(0x420c, 1);
            for line in 0..3 {
                until(&mut b, line, 1200);
                assert_eq!(b.dma[0][10], [0x82, 0x81, 0][line as usize]);
                assert_eq!(
                    pair(&b, 0, 8),
                    if indirect {
                        if line == 2 {
                            2
                        } else {
                            0
                        }
                    } else {
                        [0, 1, 3][line as usize]
                    }
                );
                assert_eq!(
                    pair(&b, 0, 5),
                    if indirect {
                        [0xffff, 0, 0x5600][line as usize]
                    } else {
                        0
                    }
                );
                if !reverse {
                    assert_eq!(
                        b.apu.input[0],
                        if indirect {
                            [0x22, 0x22, 0x33][line as usize]
                        } else {
                            [0x11, 0x22, 0x33][line as usize]
                        }
                    );
                }
            }
            if reverse {
                if indirect {
                    assert_eq!((b.wram[0x1ffff], b.wram[0x10000]), (0x66, 0x66));
                } else {
                    assert_eq!((b.wram[0xfffe], b.wram[0], b.wram[1]), (0x66, 0x66, 0x66));
                }
            }
            assert_eq!((b.hdma_active, b.io[12]), (0, 1));
            assert_eq!((b.dma[0][4], b.dma[0][7]), (0x7e, 0x7f));
        }
    }
}

#[test]
fn different_channel_dma_spans_row_reload_and_termination_without_losing_payloads() {
    for indirect in [false, true] {
        let mut b = configured(indirect);
        b.dma[0][1] = 0x22;
        b.dma[0][0] |= 2; // CGRAM unit, two bytes; latch detects duplicates/loss.
        let table: &[u8] = if indirect {
            &[0x81, 0, 2, 0x82, 0, 3, 0, 0x12]
        } else {
            &[0x81, 0x11, 0x12, 0x82, 0x22, 0x23, 0x33, 0x34, 0]
        };
        b.wram[0x100..0x100 + table.len()].copy_from_slice(table);
        b.wram[0x10200..0x10202].copy_from_slice(&[0x11, 0x12]);
        b.wram[0x10300..0x10304].copy_from_slice(&[0x22, 0x23, 0x33, 0x34]);
        channel(&mut b, 1, 0, 0x7e1000, 0x41);
        b.dma[1][5..7].copy_from_slice(&600u16.to_le_bytes());
        b.wram[0x1000..0x1258].fill(0x77);
        until(&mut b, 0, 1000);
        let before = b.clocks;
        b.write_raw(0x420b, 2);
        b.events();
        assert_eq!(
            &b.ppu.cgram[..8],
            &[0x11, 0x12, 0x22, 0x23, 0x33, 0x34, 0, 0]
        );
        assert_eq!(b.apu.input[1], 0x77);
        assert_eq!(&b.dma[1][2..7], &[0x58, 0x12, 0x7e, 0, 0]);
        assert_eq!(
            (b.io[11], b.io[12], b.hdma_active, b.dma[0][10]),
            (0, 1, 0, 0)
        );
        assert_eq!(pair(&b, 0, 8), if indirect { 0x108 } else { 0x109 });
        assert_eq!(pair(&b, 0, 5), if indirect { 0x1200 } else { 0 });
        assert!(b.v >= 3);
        // Model accounting: DMA setup + payloads + 4 refreshes, six HDMA bytes,
        // plus 5 indirect metadata reads (initial fetch already serviced) or 2 direct.
        assert_eq!(
            b.clocks - before,
            16 + 600 * 8 + 4 * 40 + 6 * 8 + if indirect { 5 * 8 } else { 2 * 8 }
        );
        let samples = b.apu.samples.len() as f64 / 2.0;
        assert!((samples - b.clocks as f64 * 32000.0 / 21477272.0).abs() <= 1.0);
    }
}

#[test]
fn subsequent_metadata_keeps_a_bus_exclusions_and_read_side_effect_controls() {
    for reverse in [false, true] {
        for table in [0x2180u16, 0x4016, 0x4017, 0x4300] {
            let mut b = bus();
            channel(&mut b, 0, if reverse { 0xc0 } else { 0x40 }, 0x000100, 0x40);
            b.wram[0x100..0x103].copy_from_slice(&[0x81, 0, 2]);
            b.wram[0x10200] = 0x11;
            b.apu.output[0] = 0x66;
            b.waddr = 0x12345;
            b.serial = [0x9180, 0x5a60];
            b.write_raw(0x420c, 1);
            until(&mut b, 0, 200);
            b.write_raw(0x4308, table as u8);
            b.write_raw(0x4309, (table >> 8) as u8);
            until(&mut b, 0, 1200);
            assert_eq!(
                (b.dma[0][10], pair(&b, 0, 8), pair(&b, 0, 5)),
                (0, table + 2, 0)
            );
            assert_eq!((b.hdma_active, b.io[12]), (0, 1));
            assert_eq!((b.waddr, b.serial), (0x12345, [0x9180, 0x5a60]));
            if reverse {
                assert_eq!(b.wram[0x10200], 0x66);
            } else {
                assert_eq!(b.apu.input[0], 0x11);
            }
        }
    }
}

#[test]
fn terminating_indirect_high_read_acknowledges_status_instead_of_skipping_pointer() {
    for later in [false, true] {
        for reverse in [false, true] {
            let mut b = bus();
            channel(&mut b, 0, if reverse { 0xc0 } else { 0x40 }, 0x000100, 0x40);
            b.wram[0x100..0x103].copy_from_slice(&[0x81, 0, 2]);
            b.apu.output[0] = 0;
            channel(&mut b, 2, 0, 0x000300, 0x41);
            b.wram[0x300..0x302].copy_from_slice(&[3, 0]);
            b.write_raw(0x420c, if later { 5 } else { 1 });
            until(&mut b, 0, 200);
            // Excluded $420F is a zero header. Last channel uses RDNMI as
            // pointer high; with a later active channel TIMEUP is also read.
            b.write_raw(0x4308, 0x0f);
            b.write_raw(0x4309, 0x42);
            b.nmi_flag = true;
            b.irq = true;
            until(&mut b, 0, 1200);
            assert_eq!(pair(&b, 0, 8), if later { 0x4212 } else { 0x4211 });
            assert_eq!(pair(&b, 0, 5), if later { 0x8082 } else { 0x8200 });
            assert!(!b.nmi_flag);
            assert_eq!(b.irq, !later);
            assert_eq!(
                (b.dma[0][10], b.hdma_active, b.io[12]),
                (0, if later { 4 } else { 0 }, if later { 5 } else { 1 })
            );
            assert_eq!((b.apu.input[0], b.wram[0x10200]), (0, 0));
        }
    }
}
