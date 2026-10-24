//! Original MIT timer vectors and firmware-free SPC700 guest for Issue #83.
//! Copyright 2026 snes-rust contributors. See tests/roms/PROVENANCE.md.
use snes_core::apu::Apu;

const MASTER_HZ: u64 = 21_477_272;
const SPC_HZ: u64 = 1_024_000;

fn divider(timer: usize) -> u64 {
    if timer == 2 {
        16
    } else {
        128
    }
}
fn period(target: u8) -> u64 {
    if target == 0 {
        256
    } else {
        target as u64
    }
}
fn tick_to_master(apu: &mut Apu, master: &mut u64, end: u64, split: bool) {
    let mut remaining = end - *master;
    for chunk in [1, 7, 666, 1001, 21, 8193].into_iter().cycle() {
        if remaining == 0 {
            break;
        }
        let step = if split {
            remaining.min(chunk)
        } else {
            remaining
        };
        apu.tick(step);
        remaining -= step;
    }
    *master = end;
    assert!(apu.error.is_none(), "{:?}", apu.error);
}
fn tick_to_cycle(apu: &mut Apu, master: &mut u64, cycle: u64) {
    // The firmware-free idle path advances two SPC cycles at a time. All
    // checkpoints are even: boundary - 2 is the last supported point before it.
    assert_eq!(cycle % 2, 0);
    tick_to_master(apu, master, (cycle * MASTER_HZ).div_ceil(SPC_HZ), false);
}
fn output(apu: &mut Apu, timer: usize) -> u8 {
    apu.read(0xfd + timer as u16)
}
fn enabled(timer: usize, target: u8) -> Apu {
    let mut apu = Apu::default();
    apu.write(0xfa + timer as u16, target);
    apu.write(0xf1, 1 << timer);
    apu
}

#[test]
fn fixed_targets_increment_then_match_and_repeat_on_all_timers() {
    for timer in 0..3 {
        for target in [1, 255, 0] {
            let mut apu = enabled(timer, target);
            let mut master = 0;
            let clocks = divider(timer) * period(target);
            for event in 1..=3 {
                tick_to_cycle(&mut apu, &mut master, event * clocks - 2);
                assert_eq!(output(&mut apu, timer), 0, "T{timer}, target {target}");
                tick_to_cycle(&mut apu, &mut master, event * clocks);
                assert_eq!(output(&mut apu, timer), 1, "T{timer}, target {target}");
                assert_eq!(output(&mut apu, timer), 0, "read clears output");
            }
        }
    }
}

// Explicit independent pulse distances from the reconciled specification, not
// values obtained from a reference emulator or the implementation under test.
const LIVE_CASES: [(u8, u64, u8, u64); 7] = [
    (200, 150, 100, 206),
    (200, 150, 150, 256),
    (100, 50, 200, 150),
    (0, 254, 1, 3),
    (200, 150, 0, 106),
    (0, 250, 255, 5),
    (200, 150, 200, 50),
];

#[test]
fn live_targets_wait_for_equality_after_eight_bit_wrap() {
    for timer in 0..3 {
        for (old, stage, new, distance) in LIVE_CASES {
            for phase in [4, 10] {
                let mut apu = enabled(timer, old);
                let mut master = 0;
                let div = divider(timer);
                // Count through two complete old periods to retain nonzero
                // output, then reach the desired stage by ordinary counting.
                let pulses = 2 * period(old) + stage;
                tick_to_cycle(&mut apu, &mut master, pulses * div + phase);
                apu.write(0xfa + timer as u16, new);
                assert_eq!(output(&mut apu, timer), 2, "target write retains output");
                assert_eq!(output(&mut apu, timer), 0, "no write-generated event");
                let first = (pulses + distance) * div;
                for event in 0..3 {
                    let boundary = first + event * period(new) * div;
                    tick_to_cycle(&mut apu, &mut master, boundary - 2);
                    assert_eq!(
                        output(&mut apu, timer),
                        0,
                        "T{timer}: {old}/{stage} -> {new}, phase {phase}, event {event}, before"
                    );
                    tick_to_cycle(&mut apu, &mut master, boundary);
                    assert_eq!(
                        output(&mut apu, timer),
                        1,
                        "T{timer}: {old}/{stage} -> {new}, phase {phase}, event {event}, at"
                    );
                }
            }
        }
    }
}

#[test]
fn output_accumulates_modulo_sixteen_and_read_does_not_restart_counting() {
    for timer in 0..3 {
        for target in [1, 255, 0] {
            for events in [15, 16, 17, 31, 32, 33] {
                let mut apu = enabled(timer, target);
                let mut master = 0;
                let clocks = divider(timer) * period(target);
                let read_at = events * clocks + 6;
                tick_to_cycle(&mut apu, &mut master, read_at);
                assert_eq!(output(&mut apu, timer), (events & 15) as u8);
                assert_eq!(output(&mut apu, timer), 0);
                tick_to_cycle(&mut apu, &mut master, (events + 1) * clocks - 2);
                assert_eq!(output(&mut apu, timer), 0);
                tick_to_cycle(&mut apu, &mut master, (events + 1) * clocks);
                assert_eq!(output(&mut apu, timer), 1);
            }
        }
    }
}

#[test]
fn timer_targets_and_output_reads_are_independent() {
    let mut apu = Apu::default();
    for (timer, target) in [3, 5, 7].into_iter().enumerate() {
        apu.write(0xfa + timer as u16, target);
    }
    apu.write(0xf1, 7);
    let mut master = 0;
    tick_to_cycle(&mut apu, &mut master, 1286);
    // Ten slow pulses and eighty fast pulses: 3, 2, 11 events.
    assert_eq!(output(&mut apu, 2), 11);
    apu.write(0xfc, 9); // fast stage is 3, so next match is pulse 86
    assert_eq!(output(&mut apu, 0), 3);
    tick_to_cycle(&mut apu, &mut master, 1374);
    assert_eq!(output(&mut apu, 2), 0);
    tick_to_cycle(&mut apu, &mut master, 1376);
    assert_eq!(output(&mut apu, 2), 1);
    assert_eq!(output(&mut apu, 1), 2, "other output was not cleared");
    tick_to_cycle(&mut apu, &mut master, 1534);
    assert_eq!(output(&mut apu, 0), 0);
    tick_to_cycle(&mut apu, &mut master, 1536);
    assert_eq!(
        output(&mut apu, 0),
        1,
        "slow stage/phase survived fast write"
    );
    tick_to_cycle(&mut apu, &mut master, 1918);
    assert_eq!(output(&mut apu, 1), 0);
    tick_to_cycle(&mut apu, &mut master, 1920);
    assert_eq!(
        output(&mut apu, 1),
        1,
        "slow stage/phase survived other reads"
    );
}

fn guest() -> Vec<u8> {
    fn delay(code: &mut Vec<u8>, mut iterations: u64) {
        // MOV Y,#n (2), DBNZ Y,loop (6 taken / 4 final): exactly 6*n
        // SPC cycles per block. The guest never polls a timer to time itself.
        while iterations != 0 {
            let n = iterations.min(255);
            code.extend([0x8d, n as u8, 0xfe, 0xfe]);
            iterations -= n;
        }
    }
    let mut code = vec![
        0x8f, 0x0c, 0xf2, 0x8f, 37, 0xf3, // independent DSP register control
        0x8f, 0x6c, 0xf2, 0x8f, 0x20, 0xf3, // disable echo writes
        0x8f, 0x7f, 0xf4, // ready
        0xe4, 0xf4, 0x68, 1, 0xd0, 0xfa, // wait for host start
        0x8f, 200, 0xfa, 0x8f, 255, 0xfb, 0x8f, 0, 0xfc, 0x8f, 7,
        0xf1, // enable all three timers; elapsed origin
    ];
    delay(&mut code, 3200);
    // At elapsed 19205 SPC cycles: slow stage 150, phase 5. The 5-cycle
    // MOV itself and whole-instruction approximation fit inside this window.
    code.extend([0x8f, 100, 0xfa]);
    for (row, wait) in [400, 2000, 2200, 2200].into_iter().enumerate() {
        delay(&mut code, wait);
        for timer in 0..3u8 {
            code.extend([0xe4, 0xfd + timer, 0xc5]); // MOV A,TnOUT / MOV !record,A
            code.extend((0x1000 + row as u16 * 3 + timer as u16).to_le_bytes());
        }
    }
    code.extend([0x8f, 0x83, 0xf4, 0xff]);
    code
}

fn wait_for(apu: &mut Apu, master: &mut u64, value: u8) {
    for _ in 0..1000 {
        assert!(apu.error.is_none(), "{:?}", apu.error);
        if apu.port_read(0) == value {
            return;
        }
        apu.tick(21);
        *master += 21;
    }
    panic!("SPC acknowledgement {value:02x} exceeded 21,000 master clocks");
}
fn upload(apu: &mut Apu) -> u64 {
    let code = guest();
    assert!(code.len() < 512);
    let mut master = 0;
    apu.port_write(2, 0);
    apu.port_write(3, 2);
    apu.port_write(1, 1);
    apu.port_write(0, 0xcc);
    wait_for(apu, &mut master, 0xcc);
    for (index, &byte) in code.iter().enumerate() {
        apu.port_write(1, byte);
        apu.port_write(0, index as u8);
        wait_for(apu, &mut master, index as u8);
    }
    apu.port_write(2, 0);
    apu.port_write(3, 2);
    apu.port_write(1, 0);
    apu.port_write(0, (code.len() as u8).wrapping_add(1));
    wait_for(apu, &mut master, 0x7f);
    assert_eq!(&apu.ram[0x200..0x200 + code.len()], &code);
    assert!(master < 100_000, "bounded firmware-free upload");
    master
}

// Elapsed guest read times from enable, based only on instruction durations:
// T0 at 21610, 33634, 46858, 60082; T1/T2 follow at +8/+16 cycles.
// T0 matches at pulse 356 (45568 cycles), then 456 (58368 cycles).
// T1 is unchanged at 255*128, T2 unchanged at 256*16.
const RECORDS: [u8; 12] = [0, 0, 5, 0, 1, 3, 1, 0, 3, 1, 0, 3];
const CHECKPOINTS: [u64; 4] = [480_000, 740_000, 1_010_000, 1_300_000];

#[test]
fn uploaded_spc_writes_live_target_and_reads_all_timer_outputs() {
    let mut apu = Apu::default();
    let mut master = upload(&mut apu);
    let origin = master;
    apu.port_write(0, 1);
    for (row, elapsed) in CHECKPOINTS.into_iter().enumerate() {
        tick_to_master(&mut apu, &mut master, origin + elapsed, false);
        assert_eq!(&apu.ram[0x1000..0x1003 + row * 3], &RECORDS[..3 + row * 3]);
    }
    assert_eq!(apu.port_read(0), 0x83, "bounded guest completion");
    assert!(apu.cpu.halted);
}

#[test]
fn whole_and_irregular_ticks_match_with_independent_guest_expectations() {
    let mut whole = Apu::default();
    let mut split = Apu::default();
    let mut whole_master = upload(&mut whole);
    let mut split_master = upload(&mut split);
    assert_eq!(whole_master, split_master);
    let origin = whole_master;
    whole.port_write(0, 1);
    split.port_write(0, 1);
    for (row, elapsed) in CHECKPOINTS.into_iter().enumerate() {
        tick_to_master(&mut whole, &mut whole_master, origin + elapsed, false);
        tick_to_master(&mut split, &mut split_master, origin + elapsed, true);
        for apu in [&whole, &split] {
            assert_eq!(&apu.ram[0x1000..0x1003 + row * 3], &RECORDS[..3 + row * 3]);
            assert!(
                apu.samples.iter().all(|&v| v == 0.0),
                "no keyed voices: silence"
            );
            assert_eq!(apu.dsp.regs[0x6c], 0x20);
            assert_eq!(apu.dsp.regs[0x0c], 37);
        }
        assert_eq!(whole.samples, split.samples, "every PCM sample");
        assert_eq!(whole.dsp.regs, split.dsp.regs, "all DSP registers");
        assert_eq!(whole.ram, split.ram, "all RAM including guest records");
        assert_eq!(whole.input, split.input);
        assert_eq!(whole.output, split.output);
        assert_eq!(whole.cpu.pc, split.cpu.pc);
    }
    assert_eq!(whole.output[0], 0x83);
    assert!(whole.cpu.halted && split.cpu.halted);
}
