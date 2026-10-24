//! Original MIT Issue #97 regressions. Expected clocks use literal NTSC geometry.
//! Copyright (c) 2026 snes-rust contributors.
use super::tests::{timer_bus, timeup};
use super::*;

// Independent arithmetic: 262*1364=357368; odd field V=240 is four clocks
// shorter. No production timing/conversion helper supplies these expectations.
fn line_start(field: u64, v: u16) -> u64 {
    let start = [0, 357368, 714732, 1072100][field as usize];
    start + u64::from(v) * 1364 - u64::from(field % 2 == 1 && v > 240) * 4
}

#[test]
fn v_only_irq_boundary_matrix_ignores_htime_and_other_scanlines() {
    for v in [0, 1, 225, 239, 240, 241, 261] {
        for htime in [0, 321, 511] {
            let mut b = timer_bus(0x20, htime, v);
            // Initial V=0 has no synthetic comparison: explicit reset convention.
            if v == 0 {
                b.tick(1364);
                assert!(!timeup(&mut b));
            }
            for field in if v == 0 { 1..3 } else { 0..2 } {
                let start = line_start(field, v);
                b.tick(start - b.clocks);
                assert!(!timeup(&mut b), "V={v}, field={field}, HTIME={htime}, H=0");
                b.tick(9);
                assert!(!timeup(&mut b), "H=9");
                b.tick(1);
                assert!(timeup(&mut b), "H=10, V={v}, field={field}, HTIME={htime}");
                assert!(!timeup(&mut b), "acknowledged");
                b.tick(1);
                assert!(!timeup(&mut b), "H=11 must not reassert");
                let length = if field % 2 == 1 && v == 240 {
                    1360
                } else {
                    1364
                };
                b.tick(length - 11);
                assert!(!timeup(&mut b), "rest of line and next H=0");
                assert_eq!(
                    (b.frames, b.v, b.h),
                    (field + u64::from(v == 261), (v + 1) % 262, 0)
                );
            }
        }
    }
    for v in [262, 511] {
        for h in [0, 321, 511] {
            let mut b = timer_bus(0x20, h, v);
            b.tick(1429464); // two complete pairs; neither VTIME can match
            assert!(!timeup(&mut b), "unreachable VTIME={v}");
        }
    }
}

#[test]
fn v_only_irq_fresh_before_at_after_and_pending_retention() {
    for v in [0, 1, 225, 239, 240, 241, 261] {
        for field in 1..=2 {
            let event = line_start(field, v) + 10;
            for offset in [0, 9, 10, 11] {
                let mut b = timer_bus(0x20, 511, v);
                // Discard the prior field's event, then observe a fresh target.
                b.tick(line_start(field, v) - 1);
                timeup(&mut b);
                b.tick(1 + offset);
                assert_eq!(
                    timeup(&mut b),
                    offset >= 10,
                    "field={field}, V={v}, H={offset}"
                );
                assert!(!timeup(&mut b));
            }
            let mut b = timer_bus(0x20, 321, v);
            b.tick(event);
            // Pending survives line and field rollover, including another match.
            let next = line_start(field + 1, v) + 10;
            b.tick(next + 100 - b.clocks);
            assert!(timeup(&mut b));
            assert!(!timeup(&mut b));
            b.tick(100);
            assert!(!timeup(&mut b), "no later same-line reassertion");
        }
    }
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
fn v_only_irq_absolute_events_with_one_irregular_and_whole_intervals() {
    for (v, events) in [
        (0, [357378, 714742]),
        (1, [1374, 358742]),
        (240, [327370, 684738]),
        (241, [328734, 686098]),
    ] {
        let mut reference = None;
        for partition in 0..3 {
            let mut b = timer_bus(0x20, 511, v);
            for event in events {
                advance(&mut b, event - 1, partition);
                assert!(
                    !timeup(&mut b),
                    "VTIME={v}, partition={partition}, event={event}"
                );
                advance(&mut b, event, partition);
                assert!(timeup(&mut b));
                advance(&mut b, event + 1, partition);
                assert!(!timeup(&mut b));
            }
            advance(&mut b, 714752, partition);
            assert!(!timeup(&mut b));
            assert_eq!((b.frames, b.v, b.h), (2, 0, 20));
            assert_eq!(
                b.apu.samples.len(),
                2 * (714752u64 * 32000 / 21477272) as usize
            );
            if let Some(samples) = &reference {
                assert_eq!(&b.apu.samples, samples);
            } else {
                reference = Some(b.apu.samples.clone());
            }
        }
    }
}

#[test]
fn v_only_irq_six_clock_cpu_reads_sample_at_access_end() {
    // This core samples after all six clocks, including the coincident event.
    // It is a scheduler convention, not evidence of physical bus strobes.
    for (v, start) in [
        (1, 1364),
        (0, 357368),
        (240, 684728),
        (241, 686088),
        (0, 714732),
    ] {
        for end_h in [0, 9, 10, 11] {
            let mut b = timer_bus(0x20, 321, v);
            b.tick(start - 7);
            timeup(&mut b); // acknowledge earlier fields, if any
            b.tick(1 + end_h);
            assert_eq!(b.read(0x4211) & 0x80 != 0, end_h >= 10);
            assert_eq!(b.clocks, start + end_h);
            assert_eq!((b.v, b.h), (v, end_h));
            assert!(!timeup(&mut b));
            if end_h < 10 {
                b.tick(10 - end_h);
                assert!(
                    timeup(&mut b),
                    "early read must not suppress later assertion"
                );
            }
            assert_eq!(b.read(0x4211) & 0x80, 0);
        }
    }
}

#[test]
fn v_only_irq_real_dma_timeup_reads_before_and_after_assertion() {
    for (v, event) in [(1, 1374), (0, 357378), (241, 686098), (0, 714742)] {
        for (lead, expected) in [(21, [0, 128, 0]), (19, [128, 0, 0])] {
            let mut b = timer_bus(0x20, 511, v);
            b.tick(event - lead);
            timeup(&mut b);
            // Fixed A=$004211 -> B=WMDATA; TIMEUP is an allowed DMA read.
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
            // Existing phases: 8 global + 8 channel setup, then each byte's
            // read at +4 and write at +8. Samples: event-1/+7/+15 or +1/+9/+17.
            assert_eq!(b.clocks, event - lead + 16 + 3 * 8);
            assert_eq!((b.v, b.h), (v, 10 + 40 - lead));
            assert_eq!(
                b.wram[..3].iter().map(|x| x & 128).collect::<Vec<_>>(),
                expected
            );
            assert_eq!(b.waddr, 3);
            assert_eq!(&b.dma[0][2..7], &[0x11, 0x42, 0, 0, 0]);
            assert_eq!(b.dma_pending, 0);
            assert_eq!(b.io[11], 0);
            assert!(!timeup(&mut b));
            b.tick(100);
            assert!(!timeup(&mut b));
        }
    }
}

#[test]
fn v_only_irq_dma_advances_devices_and_defers_cpu_service() {
    let mut rom = include_bytes!("../../../../tests/roms/smoke.bin").to_vec();
    rom[0x100] = 0xea; // IRQ handler NOP
    rom[0x7ffe..0x8000].copy_from_slice(&[0, 0x81]);
    let mut m = Machine::new(&rom).unwrap();
    m.cpu.p &= !crate::cpu::I;
    m.cpu.waiting = true;
    for (a, value) in [
        (0x4209, 1),
        (0x420a, 0),
        (0x4200, 0x20),
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
    m.bus.tick(1353 - m.bus.clocks); // event 1374 lies inside DMA
    let (pc, s, instructions) = (m.cpu.pc, m.cpu.s, m.cpu.instructions);
    let audio = m.bus.apu.samples.len();
    m.bus.events();
    // 16 setup + 800 payload + exactly one 40-clock refresh hold.
    assert_eq!((m.bus.clocks, m.bus.v, m.bus.h), (2209, 1, 845));
    assert_eq!(
        (m.cpu.pc, m.cpu.s, m.cpu.instructions),
        (pc, s, instructions)
    );
    assert!(m.cpu.waiting);
    assert!(m.bus.irq);
    assert!(m.bus.apu.samples.len() > audio);
    for i in 0..100 {
        assert_eq!(m.bus.ppu.vram[2 * i], 0x5a);
    }
    assert_eq!(&m.bus.dma[0][2..7], &[100, 0, 0x7e, 0, 0]);
    assert_eq!(m.bus.io[11], 0);
    m.step();
    assert!(!m.cpu.waiting);
    assert_eq!(m.cpu.s, s - 3);
    assert_eq!(m.cpu.pc, 0x8101);
    assert!(timeup(&mut m.bus));
    assert!(!timeup(&mut m.bus));
}
