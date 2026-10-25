//! Original MIT Issue #99 regressions; no production timing helpers in oracles.
//! Copyright (c) 2026 snes-rust contributors.
use super::tests::{timer_bus, timeup};
use super::*;

// 262*1364=357368, with four clocks removed after odd-field V=240.
fn line_start(field: u64, v: u16) -> u64 {
    [0, 357368, 714732, 1072100][field as usize] + 1364 * u64::from(v)
        - u64::from(field % 2 == 1 && v > 240) * 4
}
const VECTORS: [(u16, u64); 8] = [
    (1, 18),
    (64, 270),
    (256, 1038),
    (321, 1298),
    (323, 1306),
    (327, 1322),
    (336, 1358),
    (337, 1362),
];

#[test]
fn horizontal_irq_boundary_matrix_fresh_before_at_after() {
    for mode in [0x10, 0x30] {
        for v in [0, 1, 224, 225, 239, 240, 241, 261] {
            for field in 1..=2 {
                let length = if field == 1 && v == 240 { 1360 } else { 1364 };
                for (htime, assertion) in VECTORS {
                    if assertion >= length {
                        continue;
                    } // explicitly unverified late case
                    for h in [assertion - 1, assertion, assertion + 1] {
                        // H-only must ignore even unreachable VTIME=511.
                        let mut b = timer_bus(mode, htime, if mode == 0x10 { 511 } else { v });
                        let start = line_start(field, v);
                        b.tick(start);
                        timeup(&mut b); // discard all earlier pending events
                        b.tick(h);
                        assert_eq!(
                            timeup(&mut b),
                            h >= assertion,
                            "mode={mode:02x}, HTIME={htime}, field={field}, V={v}, H={h}"
                        );
                        assert!(!timeup(&mut b), "read acknowledges");
                        if h < assertion {
                            b.tick(1);
                            assert!(timeup(&mut b), "early read cannot consume future event");
                        }
                        b.tick(length - b.h);
                        assert!(!timeup(&mut b), "no duplicate assertion or H=0 event");
                        assert_eq!(
                            (b.frames, b.v, b.h),
                            (field + u64::from(v == 261), (v + 1) % 262, 0)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn horizontal_irq_line_selection_pending_retention_and_unreachable_vtime() {
    for v in [0, 1, 224, 225, 239, 240, 241, 261] {
        let mut b = timer_bus(0x30, 64, v);
        let event = line_start(1, v) + 270;
        b.tick(line_start(1, v) - 1);
        timeup(&mut b);
        b.tick(270);
        assert!(!timeup(&mut b)); // selected line H=269
        b.tick(1);
        assert!(timeup(&mut b));
        b.tick(line_start(2, v) + 269 - b.clocks);
        assert!(!timeup(&mut b), "no nonmatching line may assert, VTIME={v}");
        b.tick(1);
        assert!(b.irq);
        b.tick(line_start(3, v) + 370 - b.clocks);
        assert!(
            timeup(&mut b),
            "pending survives lines/fields and another event"
        );
        assert!(!timeup(&mut b));
        assert!(b.clocks > event);
    }
    for v in [262, 511] {
        let mut b = timer_bus(0x30, 64, v);
        b.tick(1429464);
        assert!(!timeup(&mut b));
    }
    let mut b = timer_bus(0x10, 64, 511);
    b.tick(1429464); // H-only pending survives four natural fields
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
fn horizontal_irq_one_irregular_and_whole_intervals_cross_natural_boundaries() {
    for mode in [0x10, 0x30] {
        for (htime, v, events) in [
            (64, 0, [357638, 715002]),
            (1, 1, [1382, 358750]),
            (336, 240, [328718, 686086]),
            (337, 241, [330086, 687450]),
            (337, 261, [357366, 714730]),
        ] {
            let mut reference = None;
            for partition in 0..3 {
                let mut b = timer_bus(mode, htime, v);
                for event in events {
                    // Discard prior fields/lines before the target line. The
                    // last preceding H-only event is no later than start-2.
                    let assertion = if htime == 64 {
                        270
                    } else if htime == 1 {
                        18
                    } else if htime == 336 {
                        1358
                    } else {
                        1362
                    };
                    advance(&mut b, event - assertion, partition);
                    timeup(&mut b);
                    advance(&mut b, event - 1, partition);
                    assert!(!timeup(&mut b));
                    advance(&mut b, event, partition);
                    assert!(timeup(&mut b));
                    advance(&mut b, event + 1, partition);
                    assert!(!timeup(&mut b));
                }
                advance(&mut b, 715012, partition);
                assert_eq!((b.clocks, b.frames, b.v, b.h), (715012, 2, 0, 280));
                assert_eq!(
                    b.apu.samples.len(),
                    2 * (715012u64 * 32000 / 21477272) as usize
                );
                if let Some(samples) = &reference {
                    assert_eq!(&b.apu.samples, samples);
                } else {
                    reference = Some(b.apu.samples.clone());
                }
            }
        }
    }
}

#[test]
fn horizontal_irq_zero_delay_and_approximate_late_controls() {
    // H-only zero asserts at 14. Issue #105 supersedes its late fallback
    // with provisional suppression; combined late controls remain unverified.
    // Combined zero is covered by the Issue #101 ten-clock regressions.
    for mode in [0x10, 0x30] {
        for field in 1..=2 {
            for (htime, offset) in [
                (0, 14),
                (337, 1348),
                (338, 1352),
                (339, 1356),
                (340, 1360),
                (341, 1364),
            ] {
                if (htime == 0 && mode == 0x30) || (htime == 337 && field == 2) {
                    continue;
                } // corrected combined-zero or ordinary-line case
                let start = line_start(field, 240);
                let length = if field == 1 { 1360 } else { 1364 };
                let mut b = timer_bus(mode, htime, 240);
                b.tick(start - 1);
                timeup(&mut b);
                b.tick(1);
                assert!(!timeup(&mut b));
                b.tick(offset.min(length) - 1);
                assert!(!timeup(&mut b));
                b.tick(1);
                assert_eq!(
                    timeup(&mut b),
                    offset < length && (mode == 0x30 || htime == 0)
                );
            }
        }
    }
}

#[test]
fn horizontal_irq_timed_cpu_timeup_reads_use_access_end_convention() {
    for mode in [0x10, 0x30] {
        for (v, event) in [
            (1, 1634),
            (0, 357638),
            (240, 684998),
            (241, 686358),
            (0, 715002),
        ] {
            for delta in [-1i64, 0, 1] {
                let end = (event as i64 + delta) as u64;
                let mut b = timer_bus(mode, 64, v);
                b.tick(end - 6);
                timeup(&mut b);
                assert_eq!(b.read(0x4211) & 128 != 0, delta >= 0);
                assert_eq!(b.clocks, end);
                assert_eq!((b.v, b.h), (v, (270i64 + delta) as u64));
                assert!(!timeup(&mut b));
                if delta < 0 {
                    b.tick(1);
                    assert!(timeup(&mut b));
                }
            }
        }
    }
}

#[test]
fn horizontal_irq_real_dma_reads_straddle_assertion_using_existing_phases() {
    for mode in [0x10, 0x30] {
        for (v, event) in [
            (1, 1634),
            (0, 357638),
            (240, 684998),
            (241, 686358),
            (0, 715002),
        ] {
            for (lead, expected) in [(21, [0, 128, 0]), (20, [128, 0, 0]), (19, [128, 0, 0])] {
                let mut b = timer_bus(mode, 64, v);
                b.tick(event - lead);
                timeup(&mut b);
                // Fixed A=TIMEUP -> B=WMDATA. Existing DMA model: 16 setup,
                // byte reads at +20/+28/+36, writes at +24/+32/+40.
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
                assert_eq!(b.clocks, event - lead + 40);
                assert_eq!((b.v, b.h), (v, 270 + 40 - lead));
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
}

#[test]
fn horizontal_irq_dma_advances_devices_and_defers_waiting_cpu_service() {
    for mode in [0x10, 0x30] {
        let mut rom = include_bytes!("../../../../tests/roms/smoke.bin").to_vec();
        rom[0x100] = 0xea;
        rom[0x7ffe..0x8000].copy_from_slice(&[0, 0x81]);
        let mut m = Machine::new(&rom).unwrap();
        m.cpu.p &= !crate::cpu::I;
        m.cpu.waiting = true;
        for (a, value) in [
            (0x4207, 64),
            (0x4208, 0),
            (0x4209, 1),
            (0x420a, 0),
            (0x4200, mode),
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
        m.bus.tick(1613 - m.bus.clocks);
        timeup(&mut m.bus); // discard initial H-only event
        let (pc, s, instructions) = (m.cpu.pc, m.cpu.s, m.cpu.instructions);
        let audio = m.bus.apu.samples.len();
        m.bus.events();
        // 16 setup + 800 payload + one 40-clock refresh = 856.
        assert_eq!((m.bus.clocks, m.bus.v, m.bus.h), (2469, 1, 1105));
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
}
