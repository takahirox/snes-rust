//! Original MIT live-directory diagnostics, data and firmware-free SPC700 guest.
//! Copyright 2026 snes-rust contributors. See tests/roms/PROVENANCE.md.
use snes_core::{apu::Apu, dsp::Dsp};

const A: u16 = 0x3000;
const B: u16 = 0x3100;
const C: u16 = 0x3200;
const D: u16 = 0x3300;
const START: u16 = 0x3400;

fn word(ram: &mut [u8], address: u16, value: u16) {
    for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
        ram[address.wrapping_add(offset as u16) as usize] = byte;
    }
}
fn entry(ram: &mut [u8], address: u16, start: u16, restart: u16) {
    word(ram, address, start);
    word(ram, address.wrapping_add(2), restart);
}
fn block(ram: &mut [u8], address: u16, nibble: u8, flags: u8) {
    ram[address as usize] = 0xa0 | flags; // range 10, filter 0: nibble * 512
    for offset in 1..=8 {
        ram[address.wrapping_add(offset) as usize] = nibble * 17;
    }
}
fn data(ram: &mut [u8]) {
    for (address, nibble) in [(A, 4), (B, 12), (C, 2), (D, 1), (START, 6)] {
        block(ram, address, nibble, 3);
    }
    entry(ram, 0x2000, A, A);
    entry(ram, 0x2004, START, B);
    entry(ram, 0x2100, START, C);
    entry(ram, 0x2104, START, D);
}
fn configure(dsp: &mut Dsp, pitch: u16) {
    for (r, v) in [
        (0x6c, 0x20), // no echo writes, reset/mute off
        (0x5d, 0x20),
        (0x0c, 64),
        (0x1c, 64),
        (0, 64),
        (1, 128),
        (2, pitch as u8),
        (3, (pitch >> 8) as u8),
        (7, 64), // E=1024, ENVX=64, held without a key event
        (0x4c, 1),
    ] {
        dsp.write(r, v);
    }
}

// Independent arithmetic on an explicitly specified decoded-nibble sequence.
// Only phase-zero coefficients are needed: 370,1305,374,0 (Fullsnes table).
// Tests specify the sequence; this helper has no directory/BRR/loop logic.
fn expected(sequence: &[i32], count: usize) -> (i16, i16, u8) {
    let at = |back| count.checked_sub(back).map_or(0, |i| sequence[i]);
    let source = (((at(4) * 370) >> 10) + ((at(3) * 1305) >> 10) + ((at(2) * 374) >> 10)) & !1;
    let output = (source >> 1) & !1; // direct GAIN 64
    (
        (output >> 2) as i16,
        (-output >> 1) as i16,
        (output >> 8) as u8,
    )
}
fn check(dsp: &mut Dsp, ram: &mut [u8], sequence: &[i32], count: usize, end: u8) {
    let (l, r, out) = expected(sequence, count);
    assert_eq!(dsp.sample(ram), (l, r), "decoded count {count}");
    assert_eq!([dsp.regs[9], dsp.regs[8], dsp.regs[0x7c]], [out, 64, end]);
}

#[derive(Clone, Copy, Debug)]
enum Change {
    Ram,
    Dir,
    Srcn,
}
fn change(dsp: &mut Dsp, ram: &mut [u8], kind: Change, second: bool) {
    match (kind, second) {
        (Change::Ram, _) => word(ram, 0x2002, if second { C } else { B }),
        (Change::Dir, false) => dsp.write(0x5d, 0x21),
        (Change::Dir, true) => entry(ram, 0x2100, START, B),
        (Change::Srcn, false) => dsp.write(4, 1),
        (Change::Srcn, true) => entry(ram, 0x2004, START, C),
    }
}
fn live_changes(kind: Change, pitch: u16) {
    let mut dsp = Dsp::default();
    let mut ram = vec![0; 65536];
    data(&mut ram);
    configure(&mut dsp, pitch);
    for _ in 0..5 {
        assert_eq!(dsp.sample(&mut ram), (0, 0));
        assert_eq!([dsp.regs[8], dsp.regs[9], dsp.regs[0x7c]], [0; 3]);
    }
    let first = if matches!(kind, Change::Dir) {
        1024
    } else {
        -2048
    };
    let second = if matches!(kind, Change::Dir) {
        -2048
    } else {
        1024
    };
    let sequence = [vec![2048; 16], vec![first; 16], vec![second; 32]].concat();
    let step = pitch as usize / 0x1000;
    for tick in 1..=48 / step {
        let count = tick * step;
        check(&mut dsp, &mut ram, &sequence, count, u8::from(count > 16));
        if tick == 2 {
            // after KON, at least ten nibbles before the first loop
            change(&mut dsp, &mut ram, kind, false);
        }
        if count >= 20 && count - step < 20 {
            change(&mut dsp, &mut ram, kind, true);
        }
    }
    // Retrigger selects the CURRENT start word (6), not either loop word.
    if matches!(kind, Change::Ram) {
        word(&mut ram, 0x2000, START);
    }
    dsp.write(0x4c, 1);
    for _ in 0..5 {
        assert_eq!(dsp.sample(&mut ram), (0, 0));
        assert_eq!(dsp.regs[0x7c], 0);
    }
    let start = [3072; 16];
    for tick in 1..=4 {
        check(&mut dsp, &mut ram, &start, tick * step, 0);
    }
}
#[test]
fn live_loop_word_ram_edits() {
    live_changes(Change::Ram, 0x1000);
}
#[test]
fn live_dir_changes() {
    live_changes(Change::Dir, 0x1000);
}
#[test]
fn live_srcn_changes() {
    live_changes(Change::Srcn, 0x1000);
}
#[test]
fn three_nibbles_per_sample_preserve_history_across_live_loops() {
    for kind in [Change::Ram, Change::Dir, Change::Srcn] {
        live_changes(kind, 0x3000);
    }
}

#[test]
fn start_word_alone_does_not_restart_or_change_loop() {
    let mut dsp = Dsp::default();
    let mut ram = vec![0; 65536];
    data(&mut ram);
    configure(&mut dsp, 0x1000);
    for _ in 0..10 {
        dsp.sample(&mut ram);
    }
    word(&mut ram, 0x2000, START);
    for count in 6..=48 {
        check(&mut dsp, &mut ram, &[2048; 48], count, u8::from(count > 16));
    }
}

#[test]
fn aligned_directory_carry_wrap_and_last_loop_word() {
    for (dir, srcn, address) in [(0x20, 0x40, 0x2100), (0xff, 0x40, 0), (0xff, 0x3f, 0xfffc)] {
        let mut dsp = Dsp::default();
        let mut ram = vec![0; 65536];
        data(&mut ram);
        entry(&mut ram, address, A, A);
        configure(&mut dsp, 0x1000);
        dsp.write(0x5d, dir);
        dsp.write(4, srcn);
        for _ in 0..5 {
            dsp.sample(&mut ram);
        }
        let sequence = [vec![2048; 16], vec![-2048; 16]].concat();
        for count in 1..=32 {
            check(&mut dsp, &mut ram, &sequence, count, u8::from(count > 16));
            if count == 4 {
                // also exercise a live word at $fffe/$ffff
                word(&mut ram, address.wrapping_add(2), B);
            }
        }
    }
}

#[test]
fn brr_payload_wraps_ffff_without_redirecting_until_end() {
    let mut dsp = Dsp::default();
    let mut ram = vec![0; 65536];
    data(&mut ram);
    block(&mut ram, 0xfffb, 4, 3); // data $fffc..$0003
    entry(&mut ram, 0x2000, 0xfffb, A);
    configure(&mut dsp, 0x1000);
    for _ in 0..5 {
        dsp.sample(&mut ram);
    }
    let sequence = [vec![2048; 16], vec![-2048; 16]].concat();
    for count in 1..=32 {
        check(&mut dsp, &mut ram, &sequence, count, u8::from(count > 16));
        if count == 4 {
            word(&mut ram, 0x2002, B);
        }
    }
}

#[test]
fn simultaneous_voices_keep_independent_entries_and_share_dir() {
    let mut dsp = Dsp::default();
    let mut ram = vec![0; 65536];
    data(&mut ram);
    entry(&mut ram, 0x2004, C, C);
    entry(&mut ram, 0x2008, START, B);
    entry(&mut ram, 0x2108, START, D);
    entry(&mut ram, 0x2104, START, B);
    configure(&mut dsp, 0x1000);
    dsp.write(1, 0); // voice 0 left only, voice 1 right only
    for (r, v) in [(0x11, 128), (0x13, 0x10), (0x14, 1), (0x17, 64), (0x4c, 3)] {
        dsp.write(r, v);
    }
    for _ in 0..5 {
        dsp.sample(&mut ram);
    }
    // v0: A -> B (SRCN) -> D (shared DIR) -> A (entry edit)
    // v1: C -> C        -> B (shared DIR) -> B (unaffected)
    let zero = [
        vec![2048; 16],
        vec![-2048; 16],
        vec![512; 16],
        vec![2048; 16],
    ]
    .concat();
    let one = [vec![1024; 32], vec![-2048; 32]].concat();
    for count in 1..=64 {
        assert_eq!(
            dsp.sample(&mut ram),
            (expected(&zero, count).0, expected(&one, count).1)
        );
        assert_eq!(
            [dsp.regs[9], dsp.regs[25]],
            [expected(&zero, count).2, expected(&one, count).2]
        );
        assert_eq!([dsp.regs[8], dsp.regs[24]], [64, 64]);
        let end = u8::from(count > 16);
        assert_eq!(dsp.regs[0x7c], if end != 0 { 3 } else { 0 });
        if count == 4 {
            dsp.write(4, 2);
        }
        if count == 20 {
            dsp.write(0x5d, 0x21);
        }
        if count == 36 {
            word(&mut ram, 0x210a, A); // only v0's current entry
        }
    }
    dsp.write(0x7c, 0xff);
    assert_eq!(dsp.regs[0x7c], 0);
    dsp.sample(&mut ram); // both voices loop again; sticky reporting resumes
    assert_eq!(dsp.regs[0x7c], 3);
    dsp.write(0x4c, 1);
    dsp.sample(&mut ram);
    assert_eq!(dsp.regs[0x7c], 2); // KON clears only voice 0
}

#[test]
fn normal_blocks_ignore_loop_bit_and_end_without_loop_stays_silent() {
    for flags in [0, 2] {
        let mut dsp = Dsp::default();
        let mut ram = vec![0; 65536];
        data(&mut ram);
        block(&mut ram, A, 4, flags);
        block(&mut ram, A + 9, 2, 1);
        configure(&mut dsp, 0x1000);
        for _ in 0..5 {
            dsp.sample(&mut ram);
        }
        let sequence = [vec![2048; 16], vec![1024; 16]].concat();
        for count in 1..=16 {
            check(&mut dsp, &mut ram, &sequence, count, 0);
            if count == 4 {
                dsp.write(0x5d, 0x21);
            }
        }
        // Header processing mutes immediately; ENDX waits for completion.
        for _ in 0..16 {
            assert_eq!(dsp.sample(&mut ram), (0, 0));
            assert_eq!([dsp.regs[8], dsp.regs[9], dsp.regs[0x7c]], [0; 3]);
        }
        for _ in 0..32 {
            assert_eq!(dsp.sample(&mut ram), (0, 0));
            assert_eq!([dsp.regs[8], dsp.regs[9], dsp.regs[0x7c]], [0, 0, 1]);
        }
        dsp.write(0x7c, 1);
        for _ in 0..32 {
            assert_eq!(dsp.sample(&mut ram), (0, 0));
            assert_eq!(dsp.regs[0x7c], 1);
        }
    }
}

fn guest() -> Vec<u8> {
    fn write(code: &mut Vec<u8>, reg: u8, value: u8) {
        code.extend([0x8f, reg, 0xf2, 0x8f, value, 0xf3]);
    }
    let mut code = Vec::new();
    for (r, v) in [
        (0x6c, 0x20),
        (0x5d, 0x20),
        (0x0c, 64),
        (0x1c, 64),
        (0, 64),
        (1, 128),
        (3, 0x10),
        (7, 64),
        (0x4c, 1),
    ] {
        write(&mut code, r, v);
    }
    code.extend([0x8f, 0x7f, 0xf4]);
    for command in 1..=4u8 {
        code.extend([0xe4, 0xf4, 0x68, command, 0xd0, 0xfa]);
        for (offset, reg) in [9, 8, 0x7c].into_iter().enumerate() {
            code.extend([0x8f, reg, 0xf2, 0xe4, 0xf3, 0xc5]);
            code.extend((0x1000u16 + 3 * (command as u16 - 1) + offset as u16).to_le_bytes());
        }
        match command {
            1 => {
                // little-endian loop word only, no KON
                code.extend([0xe8, 0, 0xc5, 2, 0x20]);
                code.extend([0xe8, 0x31, 0xc5, 3, 0x20]);
            }
            2 => write(&mut code, 0x5d, 0x21),
            3 => write(&mut code, 4, 1),
            _ => {}
        }
        code.extend([0x8f, command, 0xf4]);
    }
    code.push(0xff);
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
    panic!("SPC acknowledgement {value:02x} timed out");
}
fn upload(apu: &mut Apu) -> u64 {
    let code = guest();
    assert_eq!(code.len(), 212);
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
    assert_eq!(&apu.ram[0x200..0x200 + code.len()], code);
    assert!(apu.samples.len() / 2 < 80, "setup checkpoint bound");
    master
}
fn tick_to(apu: &mut Apu, master: &mut u64, sample: u64, split: bool) {
    let target = (sample * 32 * 21_477_272).div_ceil(1_024_000);
    let mut remaining = target - *master;
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
    *master = target;
    assert!(apu.error.is_none(), "{:?}", apu.error);
    assert_eq!(apu.samples.len() / 2, sample as usize);
}
fn anchor(apu: &Apu, begin: usize, end: usize, pcm: (i16, i16)) {
    for sample in begin..=end {
        assert_eq!(
            &apu.samples[2 * (sample - 1)..2 * sample],
            &[pcm.0 as f32 / 32768.0, pcm.1 as f32 / 32768.0],
            "PCM sample {sample}"
        );
    }
}

#[test]
fn uploaded_spc_performs_all_live_changes_and_records_numerical_results() {
    let mut apu = Apu::default();
    data(&mut apu.ram);
    let mut master = upload(&mut apu);
    for (command, outx, pcm) in [
        (1, 8, (512, -1024)),
        (2, 247, (-513, 1025)),
        (3, 4, (256, -512)),
        (4, 2, (128, -256)),
    ] {
        let sample = command as u64 * 100;
        tick_to(&mut apu, &mut master, sample, false);
        anchor(&apu, sample as usize - 19, sample as usize, pcm);
        apu.port_write(0, command);
        wait_for(&mut apu, &mut master, command);
        let offset = 0x1000 + 3 * (command as usize - 1);
        assert_eq!(&apu.ram[offset..offset + 3], &[outx, 64, 1]);
        assert_eq!(apu.dsp.regs[0x4c], 1);
    }
    tick_to(&mut apu, &mut master, 410, false);
    assert!(apu.cpu.halted);
    assert_eq!(&apu.ram[0x2002..0x2004], &B.to_le_bytes());
    assert_eq!([apu.dsp.regs[0x5d], apu.dsp.regs[4]], [0x21, 1]);
}

#[test]
fn apu_guest_pcm_registers_and_ram_match_irregular_tick_partitions() {
    let mut whole = Apu::default();
    let mut split = Apu::default();
    data(&mut whole.ram);
    data(&mut split.ram);
    let mut whole_master = upload(&mut whole);
    let mut split_master = upload(&mut split);
    assert_eq!(whole_master, split_master);
    for sample in [100, 110, 200, 210, 300, 310, 400, 410] {
        tick_to(&mut whole, &mut whole_master, sample, false);
        tick_to(&mut split, &mut split_master, sample, true);
        assert_eq!(
            whole.samples, split.samples,
            "every PCM sample through {sample}"
        );
        assert_eq!(
            whole.dsp.regs, split.dsp.regs,
            "all 128 registers at {sample}"
        );
        assert_eq!(
            whole.ram, split.ram,
            "guest code/results, directory, BRR and all RAM"
        );
        assert_eq!(whole.output, split.output);
        if sample % 100 == 0 {
            let command = (sample / 100) as u8;
            let pcm = match command {
                1 => (512, -1024),
                2 => (-513, 1025),
                3 => (256, -512),
                4 => (128, -256),
                _ => unreachable!(),
            };
            anchor(&whole, sample as usize - 19, sample as usize, pcm);
            whole.port_write(0, command);
            split.port_write(0, command);
        } else {
            let command = (sample / 100) as u8;
            assert_eq!(whole.output[0], command, "bounded guest acknowledgement");
            assert_eq!(
                &whole.ram[0x1000..0x1000 + command as usize * 3],
                &[8, 64, 1, 247, 64, 1, 4, 64, 1, 2, 64, 1,][..command as usize * 3]
            );
        }
    }
    assert!(whole.cpu.halted && split.cpu.halted);
}

#[test]
fn fractional_pitch_and_running_envelope_continue_across_live_loops() {
    let mut dsp = Dsp::default();
    let mut ram = vec![0; 65536];
    data(&mut ram);
    configure(&mut dsp, 0x1800);
    dsp.write(7, 0xdf); // linear +32 every sample; no saturation in this window
    for _ in 0..5 {
        assert_eq!(dsp.sample(&mut ram), (0, 0));
    }
    let sequence = [vec![2048; 16], vec![-2048; 16], vec![1024; 16]].concat();
    for tick in 1usize..=32 {
        // 1.5 nibbles per output; phase alternates $800/$000, including loops.
        let count = tick * 3 / 2;
        let taps = if tick % 2 == 0 {
            [370, 1305, 374, 0]
        } else {
            [56, 965, 969, 58]
        };
        let source: i32 = taps
            .into_iter()
            .enumerate()
            .map(|(i, weight)| {
                let decoded = count.checked_sub(4 - i).map_or(0, |index| sequence[index]);
                (decoded * weight) >> 10
            })
            .sum();
        let env = 32 * tick as i32;
        let output = (((source & !1) * env) >> 11) & !1;
        assert_eq!(
            dsp.sample(&mut ram),
            ((output >> 2) as i16, (-output >> 1) as i16),
            "tick {tick}"
        );
        assert_eq!(
            [dsp.regs[9], dsp.regs[8], dsp.regs[0x7c]],
            [(output >> 8) as u8, (env >> 4) as u8, u8::from(count > 16)]
        );
        if tick == 2 {
            word(&mut ram, 0x2002, B);
        }
        if tick == 14 {
            word(&mut ram, 0x2002, C);
        }
    }
}
