//! Original MIT Issue #105 tests. PROVISIONAL / HARDWARE UNVERIFIED policy.
//! Copyright (c) 2026 snes-rust contributors. Literal software oracles only.
use super::tests::{timer_bus, timeup};
use super::*;

// Independent NTSC arithmetic: 262*1364=357368, odd V=240 loses four.
fn start(field: u64, v: u16) -> u64 {
    [0, 357368, 714732, 1072100][field as usize] + 1364 * u64::from(v)
        - u64::from(field % 2 == 1 && v > 240) * 4
}

#[test]
fn late_horizontal_irq_fresh_policy_matrix_and_no_rollover_carry() {
    for field in 1..=2 {
        for v in [0, 1, 239, 240, 241, 261] {
            let short = field == 1 && v == 240;
            let length = if short { 1360 } else { 1364 };
            for vtime in [0, 240, 262, 511] {
                // Literal selected policy, not an inferred hardware rule.
                for (htime, ordinary, shortened, old_fallback) in [
                    (336, Some(1358), Some(1358), 1344),
                    (337, Some(1362), None, 1348),
                    (338, None, None, 1352),
                    (339, None, None, 1356),
                    (340, None, None, 1360),
                    (341, None, None, 1364),
                    (511, None, None, 2044),
                ] {
                    let event = if short { shortened } else { ordinary };
                    let probes = if let Some(h) = event {
                        vec![old_fallback, h - 1, h, h + 1]
                    } else {
                        vec![
                            old_fallback.min(length) - 1,
                            old_fallback.min(length),
                            (old_fallback + 1).min(length),
                            length,
                        ]
                    };
                    for h in probes {
                        let mut b = timer_bus(0x10, htime, vtime);
                        b.tick(start(field, v) - 1);
                        timeup(&mut b); // before entry, never hide an early event
                        b.tick(1 + h);
                        assert_eq!(
                            timeup(&mut b),
                            event.is_some_and(|e| h >= e),
                            "HTIME={htime}, field={field}, V={v}, VTIME={vtime}, H={h}"
                        );
                        assert!(!timeup(&mut b), "acknowledged once");
                        if let Some(e) = event.filter(|e| h < *e) {
                            b.tick(e - h);
                            assert!(timeup(&mut b), "early acknowledgement preserves delivery");
                        }
                        b.tick(start(field, v) + length + 15 - b.clocks);
                        assert!(
                            !timeup(&mut b),
                            "no duplicate or carried event: HTIME={htime}, field={field}, V={v}, VTIME={vtime}, probe={h}"
                        );
                        let next_field = field + u64::from(v == 261);
                        let next_v = (v + 1) % 262;
                        assert_eq!((b.frames, b.v, b.h), (next_field, next_v, 15));
                        assert_eq!(
                            (b.ppu.v, b.ppu.h, b.ppu.field),
                            (next_v, 15, next_field % 2 == 1)
                        );
                        // The next line owns a fresh event; it may have a different geometry.
                        let next_event = match htime {
                            336 => Some(1358),
                            337 if !(next_field % 2 == 1 && next_v == 240) => Some(1362),
                            _ => None,
                        };
                        let next_length = if next_field % 2 == 1 && next_v == 240 {
                            1360
                        } else {
                            1364
                        };
                        if let Some(e) = next_event {
                            b.tick(e - 16);
                            assert!(!timeup(&mut b));
                            b.tick(1);
                            assert!(timeup(&mut b));
                        }
                        b.tick(next_length - b.h);
                        assert!(!timeup(&mut b));
                    }
                }
            }
        }
    }
}

#[test]
fn late_horizontal_irq_pending_survives_suppression_and_acknowledges_once() {
    let mut b = timer_bus(0x10, 337, 511);
    b.tick(684725); // odd V=239 H=1361, just before an eligible event
    timeup(&mut b);
    b.tick(1);
    assert!(b.irq);
    b.tick(1362); // all of short V=240: suppressed, old pending retained
    assert_eq!((b.frames, b.v, b.h), (1, 241, 0));
    assert!(timeup(&mut b));
    assert!(!timeup(&mut b));
    b.tick(1361);
    assert!(!timeup(&mut b));
    b.tick(1);
    assert!(b.irq);
    b.tick(1072115 - b.clocks); // retain across more lines and a field
    assert!(timeup(&mut b));
    assert!(!timeup(&mut b));
}

fn advance(b: &mut SystemBus, end: u64, partition: usize) {
    let chunks = [1, 7, 1361, 2, 43, 8193, 5];
    let mut i = 0;
    while b.clocks < end {
        let n = match partition {
            0 => 1,
            1 => chunks[i % chunks.len()],
            _ => end - b.clocks,
        };
        b.tick(n.min(end - b.clocks));
        i += 1;
    }
}

#[test]
fn late_horizontal_irq_partitions_four_fields_devices_and_audio() {
    for htime in [337, 338] {
        let mut pcm = None;
        for partition in 0..3 {
            let mut b = timer_bus(0x11, htime, 262); // H-only + auto joypad
            b.buttons = 0xa590;
            b.buttons2 = 0x5a60;
            b.hdma_active = 0x85;
            b.hdma_line = [7; 8];
            b.hdma_transfer = [true; 8];
            let mut end = 0;
            for field in 0..4 {
                for v in 0..262 {
                    let short = field % 2 == 1 && v == 240;
                    let length = if short { 1360 } else { 1364 };
                    let eligible = htime == 337 && !short;
                    advance(&mut b, end + if short { 1359 } else { 1361 }, partition);
                    assert!(!timeup(&mut b), "no old fallback: field={field}, V={v}");
                    if eligible {
                        advance(&mut b, end + 1362, partition);
                        assert!(timeup(&mut b));
                        advance(&mut b, end + 1363, partition);
                        assert!(!timeup(&mut b));
                    }
                    end += length;
                    advance(&mut b, end, partition);
                    assert!(!timeup(&mut b), "no rollover carry");
                }
                assert_eq!(end, [357368, 714732, 1072100, 1429464][field]);
            }
            assert_eq!((b.clocks, b.frames, b.v, b.h), (1429464, 4, 0, 0));
            assert_eq!((b.ppu.v, b.ppu.h, b.ppu.field), (0, 0, true)); // reporting waits until H=4
            assert_eq!(b.joy, [0xa590, 0x5a60, 0, 0]);
            assert_eq!(b.serial, [0xffff; 2]);
            assert!(!b.strobe && !b.auto_latch && b.auto_poll.is_none() && b.auto_start.is_none());
            assert!(b.hdma_init && b.hdma_pending);
            assert_eq!(b.hdma_active, 0x85);
            assert_eq!(b.hdma_line, [7; 8]);
            assert_eq!(b.hdma_transfer, [true; 8]);
            assert_eq!(
                b.apu.samples.len(),
                2 * (1429464u64 * 32000 / 21477272) as usize
            );
            advance(&mut b, 1430001, partition);
            assert_eq!((b.frames, b.v, b.h, b.ppu.field), (4, 0, 537, false));
            assert_eq!(
                b.apu.samples.len(),
                2 * (1430001u64 * 32000 / 21477272) as usize
            );
            // Also cross every boundary without intermediate observations,
            // including a single multi-field tick in the whole-interval case.
            let mut uninterrupted = timer_bus(0x11, htime, 262);
            uninterrupted.buttons = 0xa590;
            uninterrupted.buttons2 = 0x5a60;
            uninterrupted.hdma_active = 0x85;
            uninterrupted.hdma_line = [7; 8];
            uninterrupted.hdma_transfer = [true; 8];
            advance(&mut uninterrupted, 1430001, partition);
            assert_eq!(uninterrupted.irq, htime == 337);
            assert_eq!(
                (
                    uninterrupted.frames,
                    uninterrupted.v,
                    uninterrupted.h,
                    uninterrupted.ppu.field
                ),
                (4, 0, 537, false)
            );
            assert_eq!(uninterrupted.joy, [0xa590, 0x5a60, 0, 0]);
            assert_eq!(uninterrupted.serial, [0xffff; 2]);
            assert!(uninterrupted.auto_poll.is_none() && uninterrupted.auto_start.is_none());
            assert!(uninterrupted.hdma_init && uninterrupted.hdma_pending);
            assert_eq!(uninterrupted.hdma_active, 0x85);
            assert_eq!(uninterrupted.hdma_line, [7; 8]);
            assert_eq!(uninterrupted.hdma_transfer, [true; 8]);
            assert_eq!(uninterrupted.apu.samples, b.apu.samples);
            if let Some(samples) = &pcm {
                assert_eq!(
                    &b.apu.samples, samples,
                    "PCM equality proves determinism only"
                );
            } else {
                pcm = Some(b.apu.samples.clone());
            }
        }
    }
}

#[test]
fn late_horizontal_irq_cpu_reads_use_access_end_policy_across_rollover() {
    // Six-clock access-end sampling/immediate read clear are model conventions.
    for (field, v) in [(0, 1), (1, 240), (1, 241), (1, 261), (2, 261)] {
        for (htime, event) in [
            (336, Some(1358)),
            (337, Some(1362)),
            (338, None),
            (339, None),
            (340, None),
        ] {
            let event = if field == 1 && v == 240 && htime == 337 {
                None
            } else {
                event
            };
            let length = if field == 1 && v == 240 { 1360 } else { 1364 };
            for h in [
                1347,
                1348,
                1352,
                1356,
                1357,
                1358,
                1359,
                1360,
                1361,
                1362,
                1363,
                length,
                length + 2,
                length + 10,
            ] {
                let mut b = timer_bus(0x10, htime, 511);
                b.tick(start(field, v) + 1300);
                timeup(&mut b); // before every possible target event
                b.tick(h - 1306);
                assert_eq!(
                    b.read(0x4211) & 128 != 0,
                    event.is_some_and(|e| h >= e),
                    "field={field}, V={v}, HTIME={htime}, H={h}"
                );
                assert_eq!(b.clocks, start(field, v) + h);
                assert_eq!(
                    (b.frames, b.v, b.h),
                    if h < length {
                        (field, v, h)
                    } else {
                        (field + u64::from(v == 261), (v + 1) % 262, h - length)
                    }
                );
                assert!(!timeup(&mut b));
                if let Some(e) = event.filter(|e| h < *e) {
                    b.tick(e - h);
                    assert!(timeup(&mut b), "early read preserves event");
                }
            }
        }
    }
}

#[test]
fn late_horizontal_irq_actual_dma_timeup_reads_cross_line_and_field() {
    for (field, v, htime, event, lead, expected) in [
        (0, 1, 337, 1362, 21, [0, 128, 0]),
        (1, 240, 336, 1358, 20, [128, 0, 0]),
        (1, 240, 337, 1348, 21, [0, 0, 0]), // former fallback suppressed
        (1, 240, 337, 1362, 21, [0, 0, 0]), // no carried candidate
        (1, 241, 337, 1362, 19, [128, 0, 0]),
        (1, 261, 337, 1362, 21, [0, 128, 0]),
        (2, 261, 338, 1366, 21, [0, 0, 0]),
        (2, 261, 339, 1356, 21, [0, 0, 0]),
    ] {
        let mut b = timer_bus(0x10, htime, 262);
        let begin = start(field, v) + event - lead;
        b.tick(begin);
        timeup(&mut b);
        b.wram[..3].fill(0x5a);
        for (a, value) in [
            (0x4300, 8),
            (0x4301, 0x80),
            (0x4302, 0x11),
            (0x4303, 0x42),
            (0x4304, 0),
            (0x4305, 3),
            (0x4306, 0),
            (0x420b, 1),
        ] {
            b.write_raw(a, value);
        }
        b.events();
        // Existing phases: 16 setup; reads +20/+28/+36, writes +24/+32/+40.
        assert_eq!(b.clocks, begin + 40);
        assert_eq!(&b.wram[..3], &expected);
        assert_eq!(b.waddr, 3);
        assert_eq!(&b.dma[0][2..7], &[0x11, 0x42, 0, 0, 0]);
        assert_eq!((b.dma_pending, b.io[11]), (0, 0));
        let length = if field == 1 && v == 240 { 1360 } else { 1364 };
        let h = event - lead + 40;
        assert_eq!(
            (b.frames, b.v, b.h),
            if h < length {
                (field, v, h)
            } else {
                (field + u64::from(v == 261), (v + 1) % 262, h - length)
            }
        );
        assert!(!timeup(&mut b));
        b.tick(100);
        assert!(!timeup(&mut b));
    }
}

#[test]
fn late_horizontal_irq_dma_holds_cpu_retains_eligible_and_suppresses_late() {
    for (htime, pending) in [(336, true), (337, false), (338, false), (339, false)] {
        let mut rom = include_bytes!("../../../../tests/roms/smoke.bin").to_vec();
        rom[0x100] = 0xea;
        rom[0x7ffe..0x8000].copy_from_slice(&[0, 0x81]);
        let mut m = Machine::new(&rom).unwrap();
        m.cpu.p &= !crate::cpu::I;
        m.cpu.waiting = true;
        for (a, value) in [
            (0x4207, htime as u8),
            (0x4208, (htime >> 8) as u8),
            (0x4209, 0xff),
            (0x420a, 1),
            (0x4200, 0x10),
            (0x4300, 0),
            (0x4301, 0x18),
            (0x4302, 0),
            (0x4303, 0),
            (0x4304, 0x7e),
            (0x4305, 100),
            (0x4306, 0),
            (0x420b, 1),
        ] {
            m.bus.write_raw(a, value);
        }
        m.bus.wram[..100].fill(0x5a);
        m.bus.tick(686066 - m.bus.clocks); // odd V=240/H=1338, before old fallback
        timeup(&mut m.bus);
        let state = (m.cpu.pc, m.cpu.s, m.cpu.instructions);
        let stack = m.bus.wram[0x100..0x200].to_vec();
        m.bus.events();
        assert_eq!(
            (m.bus.clocks, m.bus.frames, m.bus.v, m.bus.h),
            (686922, 1, 241, 834)
        );
        assert_eq!((m.cpu.pc, m.cpu.s, m.cpu.instructions), state);
        assert_eq!(&m.bus.wram[0x100..0x200], &stack);
        assert!(m.cpu.waiting);
        assert_eq!(m.bus.irq, pending);
        assert_eq!(
            m.bus.apu.samples.len(),
            2 * (686922u64 * 32000 / 21477272) as usize
        );
        for i in 0..100 {
            assert_eq!(m.bus.ppu.vram[2 * i], 0x5a);
        }
        assert_eq!(&m.bus.dma[0][2..7], &[100, 0, 0x7e, 0, 0]);
        assert_eq!((m.bus.dma_pending, m.bus.io[11]), (0, 0));
        m.step();
        if pending {
            assert!(!m.cpu.waiting);
            assert_eq!(m.cpu.s, state.1 - 3);
            assert_eq!(m.cpu.pc, 0x8101);
            assert!(timeup(&mut m.bus));
        } else {
            assert!(m.cpu.waiting);
            assert_eq!((m.cpu.pc, m.cpu.s, m.cpu.instructions), state);
            assert!(!timeup(&mut m.bus));
        }
        assert!(!timeup(&mut m.bus));
    }
}
