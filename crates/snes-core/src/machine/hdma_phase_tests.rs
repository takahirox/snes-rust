//! Original MIT natural-scheduler phase regressions for Issue #35.
//! Copyright (c) 2026 snes-rust contributors.
use super::*;

fn bus(indirect: bool, port: u8) -> SystemBus {
    let mut b = SystemBus::new(
        Cartridge::load(include_bytes!("../../../../tests/roms/smoke.bin")).unwrap(),
    );
    for (r, value) in [
        (0, if indirect { 0x40 } else { 0 }),
        (1, port),
        (2, 0),
        (3, 1),
        (4, 0x7e),
        (5, 0),
        (6, 0),
        (7, 0x7f),
    ] {
        b.write_raw(0x4300 + r, value);
    }
    let table: &[u8] = if indirect {
        &[0x81, 0, 2, 0x82, 0, 3, 0, 0x12]
    } else {
        &[0x81, 15, 0x82, 8, 1, 0]
    };
    b.wram[0x100..0x100 + table.len()].copy_from_slice(table);
    b.wram[0x10200] = 15;
    b.wram[0x10300..0x10302].copy_from_slice(&[8, 1]);
    b.write_raw(0x420c, 1);
    b
}
fn pair(b: &SystemBus, r: usize) -> u16 {
    u16::from_le_bytes([b.dma[0][r], b.dma[0][r + 1]])
}
// Reach an exact even clock using ordinary six/eight-clock CPU accesses.
// No raster positioning, tick/transfer calls or injected HDMA state.
fn at(b: &mut SystemBus, frame: u64, line: u16, h: u64) {
    let target = (frame * 262 + u64::from(line)) * LINE + h;
    while b.clocks < target {
        match target - b.clocks {
            n if n >= 8 && n % 6 != 0 => {
                b.read(0x7e0000);
            }
            n if n >= 6 => b.idle(),
            n => panic!("unreachable clock gap {n}"),
        }
    }
    assert_eq!((b.frames, b.v, b.h), (frame, line, h));
}
fn initial(b: &SystemBus, indirect: bool) {
    assert_eq!(pair(b, 8), if indirect { 0x103 } else { 0x101 });
    assert_eq!(pair(b, 5), if indirect { 0x200 } else { 0 });
    assert_eq!(b.dma[0][10], 0x81);
    assert_eq!(b.apu.input[0], 0);
}
#[test]
fn direct_and_indirect_hblank_and_phase_have_distinct_bus_boundaries() {
    for indirect in [false, true] {
        for straddle in [false, true] {
            let mut b = bus(indirect, 0x40);
            at(&mut b, 0, 0, if straddle { 1094 } else { 1090 });
            initial(&b, indirect);
            assert_eq!(b.read_raw(0x4212) & 0xc0, 0);
            if straddle {
                b.read(0x7e0000); // 1094 -> 1102, straddles HBlank
                assert_eq!(b.read_raw(0x4212) & 0x40, 0x40);
            } else {
                assert_eq!(b.read(0x4212) & 0x40, 0x40); // ends exactly at 1096
            }
            assert!(!b.hdma_pending);
            initial(&b, indirect);
            at(&mut b, 0, 0, 1110);
            initial(&b, indirect);
            if straddle {
                // CPU writes live data after crossing eligibility, before service.
                b.write(if indirect { 0x7f0200 } else { 0x7e0101 }, 0x5a);
                assert_eq!(b.h, 1118);
            } else {
                b.read(0x4212); // 1110 -> 1116, request only
            }
            initial(&b, indirect);
            assert!(b.hdma_pending);
            let start = b.h;
            b.read(0x430a); // service first: one byte then next-row metadata
            assert_eq!(b.apu.input[0], if straddle { 0x5a } else { 15 });
            assert_eq!(b.h - start, if indirect { 38 } else { 22 });
            assert_eq!(pair(&b, 8), if indirect { 0x106 } else { 0x103 });
            assert_eq!(pair(&b, 5), if indirect { 0x300 } else { 0 });
            assert_eq!(b.dma[0][10], 0x82); // fresh row waits for line 1
            let state = (pair(&b, 8), pair(&b, 5), b.dma[0][10]);
            b.read(0x430a);
            assert_eq!((pair(&b, 8), pair(&b, 5), b.dma[0][10]), state);
        }
        let mut b = bus(indirect, 0x40);
        at(&mut b, 0, 0, 1096);
        assert_eq!(b.read_raw(0x4212) & 0x40, 0x40);
        b.events();
        initial(&b, indirect);
        at(&mut b, 0, 0, 1112);
        initial(&b, indirect); // reaching threshold does not perform a bus read
        assert!(b.hdma_pending);
        b.events();
        assert_eq!(b.apu.input[0], 15);
        assert_eq!(b.h, if indirect { 1144 } else { 1128 });
    }
}
#[test]
fn visible_line_is_rendered_before_later_hdma_and_stays_completed() {
    for indirect in [false, true] {
        let mut b = bus(indirect, 0);
        b.ppu.cgram[..2].copy_from_slice(&[255, 127]);
        at(&mut b, 0, 1, 1094);
        // Raster progress already emitted 251 pixels before HBlank; tail
        // completes by H=1112, before the unchanged HDMA service boundary.
        assert_eq!(&b.ppu.frame[..251 * 4], &[255; 251 * 4]);
        assert_eq!(&b.ppu.frame[251 * 4..1024], &[0; 5 * 4]);
        assert_eq!(b.ppu.regs[0], 15); // line-0 event already serviced
        b.read(0x4212); // crosses HBlank, no HDMA
        assert_eq!(&b.ppu.frame[..4], &[255, 255, 255, 255]);
        assert_eq!(b.dma[0][10], 0x82);
        at(&mut b, 0, 1, 1112);
        assert_eq!(b.ppu.regs[0], 15);
        b.events();
        assert_eq!((b.ppu.regs[0], b.dma[0][10]), (8, 0x81));
        assert_eq!(&b.ppu.frame[..1024], &[255; 1024]);
        at(&mut b, 0, 1, 1360);
        b.read(0x4016); // straddle scanline boundary, no repeated request
        assert_eq!((b.v, b.h, b.dma[0][10]), (2, 8, 0x81));
        at(&mut b, 0, 2, 1096);
        assert_eq!(&b.ppu.frame[1024..1028], &[136, 136, 136, 255]);
        assert_eq!(b.dma[0][10], 0x81);
        at(&mut b, 0, 2, 1112);
        b.events();
        assert_eq!(
            (b.ppu.regs[0], b.dma[0][10], b.hdma_active, b.io[12]),
            (1, 0, 0, 1)
        );
    }
}
#[test]
fn final_line_holds_termination_vblank_and_following_frame_keep_one_event_each() {
    for indirect in [false, true] {
        let mut b = bus(indirect, 0x40);
        if indirect {
            b.wram[0x100..0x108].copy_from_slice(&[0x80, 0, 2, 0xe1, 0, 3, 0, 0x12]);
            b.wram[0x10300..0x10361].fill(0x55);
        } else {
            b.wram[0x100..0x103].copy_from_slice(&[0x80, 15, 0xe1]);
            b.wram[0x103..0x164].fill(0x55);
            b.wram[0x164] = 0;
        }
        for line in 0..225 {
            at(&mut b, 0, line, 1096);
            b.events();
            let prior = b.dma[0][10];
            assert_eq!(
                prior,
                if line < 128 {
                    0x80 - line as u8
                } else {
                    0xe1 - (line - 128) as u8
                }
            );
            let addresses = (pair(&b, 8), pair(&b, 5));
            at(&mut b, 0, line, 1112);
            assert_eq!(
                (b.dma[0][10], pair(&b, 8), pair(&b, 5)),
                (prior, addresses.0, addresses.1)
            );
            b.apu.input[0] = 0xee;
            b.events();
            assert_eq!(
                b.apu.input[0],
                if line == 0 {
                    15
                } else if line < 128 {
                    0xee
                } else {
                    0x55
                }
            );
            assert_eq!(
                b.dma[0][10],
                if line == 127 {
                    0xe1
                } else if line == 224 {
                    0
                } else {
                    prior - 1
                }
            );
        }
        assert_eq!((b.hdma_active, b.io[12]), (0, 1));
        assert_eq!(pair(&b, 8), if indirect { 0x108 } else { 0x165 });
        assert_eq!(pair(&b, 5), if indirect { 0x1200 } else { 0 });
        for line in 225..262 {
            at(&mut b, 0, line, 1112);
            assert_eq!(b.read_raw(0x4212) & 0xc0, 0xc0);
            assert!(!b.hdma_pending);
            b.events();
            assert_eq!(b.dma[0][10], 0);
        }
        at(&mut b, 1, 0, 20);
        assert_eq!(b.dma[0][10], 0);
        b.read(0x430a); // crosses initialization at 24, service next boundary
        assert_eq!(b.dma[0][10], 0);
        b.events();
        assert_eq!(b.dma[0][10], 0x80);
        at(&mut b, 1, 0, 1112);
        assert_eq!(b.dma[0][10], 0x80);
        b.events();
        assert_eq!((b.dma[0][10], b.apu.input[0]), (0x7f, 15));
    }
}
#[test]
fn dma_byte_crossing_phase_defers_hdma_to_next_byte_and_keeps_order() {
    // Shared CGRAM destination makes interleaving observable as a byte stream.
    for indirect in [false, true] {
        for count in [400u16, 600] {
            let mut b = bus(indirect, 0x22);
            b.write_raw(0x4300, if indirect { 0x42 } else { 2 });
            let table: &[u8] = if indirect {
                &[0x81, 0, 2, 0x82, 0, 3, 0, 0x12]
            } else {
                &[0x81, 0x11, 0x12, 0x82, 0x22, 0x23, 0x33, 0x34, 0]
            };
            b.wram[0x100..0x100 + table.len()].copy_from_slice(table);
            b.wram[0x10200..0x10202].copy_from_slice(&[0x11, 0x12]);
            b.wram[0x10300..0x10304].copy_from_slice(&[0x22, 0x23, 0x33, 0x34]);
            // Different-channel DMA to the same CGRAM latch. Distinct byte pairs
            // reveal exactly where HDMA interrupts the general DMA stream.
            for (r, v) in [
                (0, 2),
                (1, 0x22),
                (2, 0),
                (3, 0x10),
                (4, 0x7e),
                (5, 0x58),
                (6, 2),
            ] {
                b.write_raw(0x4310 + r, v);
            }
            b.write_raw(0x4315, count as u8);
            b.write_raw(0x4316, (count >> 8) as u8);
            for (i, byte) in b.wram[0x1000..0x1258].iter_mut().enumerate() {
                *byte = (i % 100) as u8;
            }
            // A short DMA retains the first insertion instead of wrapping CGRAM.
            let mut short = bus(indirect, 0x22);
            short.wram.copy_from_slice(&b.wram);
            for channel in [0, 1] {
                for r in 0..8 {
                    short.write_raw(
                        0x4300 + channel * 16 + r,
                        b.dma[channel as usize][r as usize],
                    );
                }
            }
            short.write_raw(0x4315, 2);
            short.write_raw(0x4316, 0);
            at(&mut short, 0, 0, 1082);
            short.write_raw(0x420b, 2);
            short.events();
            assert_eq!(&short.ppu.cgram[..6], &[0, 1, 0x11, 0x12, 0, 0]);
            assert_eq!(short.dma[0][10], 0x82);
            at(&mut b, 0, 0, 1082);
            let before = b.clocks;
            b.write_raw(0x420b, 2);
            b.events();
            // setup ends 1098, first two DMA bytes end 1106 and 1114.
            // The second byte crosses eligibility; the HDMA pair follows it.
            // DMA-byte insertion indices, derived from 1098 setup end, eight
            // clocks/byte, 40 refresh clocks/line and row-end metadata reads:
            // direct 2,165,328; indirect 2,163,326 (extra pointer reads).
            let insertions = if indirect {
                [2, 163, 326]
            } else {
                [2, 165, 328]
            };
            let mut expected = [0u8; 512];
            let mut cursor = 0;
            for i in 0..usize::from(count) {
                if let Some(row) = insertions.iter().position(|&index| index == i) {
                    for v in [[0x11, 0x12], [0x22, 0x23], [0x33, 0x34]][row] {
                        expected[cursor % 512] = v;
                        cursor += 1;
                    }
                }
                expected[cursor % 512] = (i % 100) as u8;
                cursor += 1;
            }
            assert_eq!(cursor, usize::from(count) + 6);
            assert_eq!(b.ppu.cgram, expected);
            let end = 0x1000 + count;
            assert_eq!(&b.dma[1][2..7], &[end as u8, (end >> 8) as u8, 0x7e, 0, 0]);
            assert_eq!(
                (b.dma[0][10], b.hdma_active, b.io[12], b.io[11]),
                (0, 0, 1, 0)
            );
            assert_eq!(pair(&b, 8), if indirect { 0x108 } else { 0x109 });
            assert_eq!(pair(&b, 5), if indirect { 0x1200 } else { 0 });
            assert!(b.v >= 3);
            assert_eq!(
                b.clocks - before,
                16 + u64::from(count) * 8
                    + if count == 400 { 2 * 40 } else { 4 * 40 }
                    + 6 * 8
                    + if indirect { 5 * 8 } else { 2 * 8 }
            );
            let samples = b.apu.samples.len() as f64 / 2.0;
            assert!((samples - b.clocks as f64 * 32000.0 / 21477272.0).abs() <= 1.0);
        }
    }
}

#[test]
fn payload_samples_at_service_plus_four_and_commits_at_plus_eight() {
    for (indirect, enabled) in [(false, true), (true, true), (false, false), (true, false)] {
        let mut b = bus(indirect, 0x37); // SLHV read latches the source-sample raster
        b.write_raw(0x4300, if indirect { 0xc0 } else { 0x80 });
        b.wram[0x100] = 0x82; // no row-end read on this event
        let destination = if indirect { 0x10200 } else { 0x101 };
        b.wram[destination] = 0x55;
        b.write(0x4201, 0xff);
        at(&mut b, 0, 0, 100);
        if !enabled {
            b.write(0x4201, 0x7f); // Capture at 106/4=26, retained across HDMA SLHV.
        }
        at(&mut b, 0, 0, 1112);
        assert_eq!(b.wram[destination], 0x55);
        assert_eq!(b.dma[0][10], 0x82);
        b.events();
        assert_eq!(b.h, 1120);
        // SLHV returns the prior CPU open byte; its latch independently
        // records the modeled read at 1116/4=279, before commitment at 1120.
        assert_eq!(b.read_raw(0x213c), if enabled { 279u16 as u8 } else { 26 });
        assert_eq!(b.read_raw(0x213c) & 1, u8::from(enabled));
        assert_eq!(b.wram[destination], 0);
        assert_eq!(b.dma[0][10], 0x81);
    }
}
