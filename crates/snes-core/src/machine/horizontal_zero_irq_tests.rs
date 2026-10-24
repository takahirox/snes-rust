//! Original MIT Issue #103 regressions. Expected clocks use literal NTSC geometry.
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
fn horizontal_zero_irq_fresh_boundary_matrix_and_no_duplicate() {
    for v in [0, 1, 224, 225, 239, 240, 241, 261] {
        for field in 1..=2 {
            for vtime in [0, 240, 262, 511] {
                for h in [0, 9, 10, 13, 14, 15] {
                    let mut b = timer_bus(0x10, 0, vtime);
                    let start = line_start(field, v);
                    b.tick(start - 1);
                    timeup(&mut b); // BEFORE entry: cannot hide a wrong H=0 event
                    b.tick(1 + h);
                    assert_eq!(
                        timeup(&mut b),
                        h >= 14,
                        "field={field}, V={v}, VTIME={vtime}, H={h}"
                    );
                    assert!(!timeup(&mut b), "acknowledged once");
                    if h < 14 {
                        b.tick(14 - h);
                        assert!(timeup(&mut b), "early read preserves future delivery");
                    }
                    let length = if field == 1 && v == 240 { 1360 } else { 1364 };
                    b.tick(length - b.h);
                    assert!(!timeup(&mut b), "no later or next-H=0 duplicate");
                    assert_eq!(
                        (b.frames, b.v, b.h),
                        (field + u64::from(v == 261), (v + 1) % 262, 0)
                    );
                    b.tick(13);
                    assert!(!timeup(&mut b), "next line is still early");
                    b.tick(1);
                    assert!(timeup(&mut b), "next line delivers again at 14");
                    assert!(!timeup(&mut b));
                }
            }
        }
    }
}

#[test]
fn horizontal_zero_irq_pending_retention_and_initial_reset_policy() {
    for vtime in [0, 240, 262, 511] {
        let mut b = timer_bus(0x10, 0, vtime);
        b.tick(1363);
        assert!(
            !timeup(&mut b),
            "initial reset-created V=0 policy is unverified"
        );
        b.tick(1);
        assert!(!timeup(&mut b));
        b.tick(14);
        assert!(b.irq);
        // Retain the flag through lines, fields and subsequent assertions.
        for end in [2728, 357368, 357382, 714732, 714746, 1072114] {
            b.tick(end - b.clocks);
            assert!(b.irq, "pending retained at absolute clock {end}");
        }
        assert!(timeup(&mut b));
        assert!(!timeup(&mut b));
        b.tick(1350);
        assert!(!timeup(&mut b), "next H=0 does not reassert");
        b.tick(13);
        assert!(!timeup(&mut b));
        b.tick(1);
        assert!(timeup(&mut b));
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
fn horizontal_zero_irq_absolute_events_with_one_irregular_and_whole_intervals() {
    for (field, v, event) in [
        (0, 1, 1378),
        (1, 0, 357382),
        (2, 0, 714746),
        (0, 240, 327374),
        (1, 240, 684742),
        (0, 241, 328738),
        (1, 241, 686102),
    ] {
        let mut reference = None;
        for partition in 0..3 {
            let mut b = timer_bus(0x10, 0, 511);
            advance(&mut b, event - 15, partition); // target start minus one
            timeup(&mut b); // discard earlier lines before entry
            advance(&mut b, event - 1, partition);
            assert!(!timeup(&mut b), "event={event}, partition={partition}");
            advance(&mut b, event, partition);
            assert!(timeup(&mut b));
            advance(&mut b, event + 1, partition);
            assert!(!timeup(&mut b));
            assert_eq!((b.frames, b.v, b.h), (field, v, 15));
            assert_eq!((b.ppu.v, b.ppu.h, b.ppu.field), (v, 15, field % 2 == 1));
            assert_eq!(
                b.apu.samples.len(),
                2 * ((event + 1) * 32000 / 21477272) as usize
            );
            if let Some(samples) = &reference {
                assert_eq!(&b.apu.samples, samples, "PCM partition determinism only");
            } else {
                reference = Some(b.apu.samples.clone());
            }
        }
    }
}

#[test]
fn horizontal_zero_irq_six_clock_cpu_reads_sample_at_access_end() {
    // This core samples after all six clocks, including the coincident event.
    // It is a scheduler convention, not evidence of physical bus strobes.
    for (v, start) in [
        (1, 1364),
        (0, 357368),
        (240, 684728),
        (241, 686088),
        (0, 714732),
    ] {
        for end_h in [0, 9, 10, 13, 14, 15] {
            let mut b = timer_bus(0x10, 0, 511);
            b.tick(start - 7);
            timeup(&mut b); // acknowledge earlier fields, if any
            b.tick(1 + end_h);
            assert_eq!(b.read(0x4211) & 0x80 != 0, end_h >= 14);
            assert_eq!(b.clocks, start + end_h);
            assert_eq!((b.v, b.h), (v, end_h));
            assert!(!timeup(&mut b));
            if end_h < 14 {
                b.tick(14 - end_h);
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
fn horizontal_zero_irq_real_dma_timeup_reads_before_and_after_assertion() {
    for (v, event) in [
        (1, 1378),
        (0, 357382),
        (240, 684742),
        (241, 686102),
        (0, 714746),
    ] {
        for (lead, expected) in [(21, [0, 128, 0]), (20, [128, 0, 0]), (19, [128, 0, 0])] {
            let mut b = timer_bus(0x10, 0, 511);
            b.tick(event - lead);
            timeup(&mut b);
            b.wram[..3].fill(0x5a); // every destination must actually be written
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
            // read at +4 and write at +8. Samples: event-1/+7/+15,
            // event/+8/+16 or event+1/+9/+17. Model phases, not physical strobes.
            assert_eq!(b.clocks, event - lead + 16 + 3 * 8);
            assert_eq!((b.v, b.h), (v, 14 + 40 - lead));
            // Lower bits retain the model's initial zero open bus.
            assert_eq!(&b.wram[..3], &expected);
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
fn horizontal_zero_irq_dma_advances_devices_and_defers_cpu_service() {
    let mut rom = include_bytes!("../../../../tests/roms/smoke.bin").to_vec();
    rom[0x100] = 0xea; // IRQ handler NOP
    rom[0x7ffe..0x8000].copy_from_slice(&[0, 0x81]);
    let mut m = Machine::new(&rom).unwrap();
    m.cpu.p &= !crate::cpu::I;
    m.cpu.waiting = true;
    for (a, value) in [
        (0x4207, 0),
        (0x4208, 0),
        (0x4209, 1),
        (0x420a, 0),
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
    m.bus.tick(1353 - m.bus.clocks); // event 1378 lies inside DMA
    assert!(!m.bus.irq, "no earlier IRQ pending before DMA");
    let (pc, s, instructions) = (m.cpu.pc, m.cpu.s, m.cpu.instructions);
    let audio = m.bus.apu.samples.len();
    let stack = m.bus.wram[0x100..0x200].to_vec();
    m.bus.events();
    // 16 setup + 800 payload + exactly one 40-clock refresh hold.
    assert_eq!((m.bus.clocks, m.bus.v, m.bus.h), (2209, 1, 845));
    assert_eq!(
        (m.cpu.pc, m.cpu.s, m.cpu.instructions),
        (pc, s, instructions)
    );
    assert!(m.cpu.waiting);
    assert_eq!(m.bus.wram[0x100..0x200], stack);
    assert!(m.bus.irq);
    assert!(m.bus.apu.samples.len() > audio);
    assert_eq!(
        m.bus.apu.samples.len(),
        2 * (2209u64 * 32000 / 21477272) as usize
    );
    assert_eq!((m.bus.ppu.v, m.bus.ppu.h), (1, 845));
    for i in 0..100 {
        assert_eq!(m.bus.ppu.vram[2 * i], 0x5a);
    }
    assert_eq!(&m.bus.dma[0][2..7], &[100, 0, 0x7e, 0, 0]);
    assert_eq!(m.bus.io[11], 0);
    assert_eq!(m.bus.dma_pending, 0);
    m.step();
    assert!(!m.cpu.waiting);
    assert_eq!(m.cpu.s, s - 3);
    assert_eq!(m.cpu.pc, 0x8101);
    assert_eq!(m.cpu.instructions, instructions + 1);
    assert_ne!(m.cpu.p & crate::cpu::I, 0);
    assert!(timeup(&mut m.bus));
    assert!(!timeup(&mut m.bus));
}
