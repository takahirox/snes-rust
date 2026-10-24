//! Original MIT terminal-header vectors and firmware-free SPC700 guest.
//! Copyright 2026 snes-rust contributors. See tests/roms/PROVENANCE.md.
use snes_core::{apu::Apu, dsp::Dsp};

const A: u16 = 0x3000;
const B: u16 = 0x3100;
const C: u16 = 0x3400;
const D: u16 = 0x3500;

fn word(ram: &mut [u8], addr: u16, value: u16) {
    ram[addr as usize..addr as usize + 2].copy_from_slice(&value.to_le_bytes());
}
fn entry(ram: &mut [u8], addr: u16, start: u16, restart: u16) {
    word(ram, addr, start);
    word(ram, addr + 2, restart);
}
fn block(ram: &mut [u8], addr: u16, nibble: u8, flags: u8) {
    ram[addr as usize] = 0xa0 | flags;
    ram[addr as usize + 1..addr as usize + 9].fill(nibble * 17);
}
fn setup(pitch: u16) -> (Dsp, Vec<u8>) {
    let mut ram = vec![0; 65536];
    entry(&mut ram, 0x2000, A, B);
    entry(&mut ram, 0x2004, D, D);
    block(&mut ram, A, 4, 0);
    block(&mut ram, A + 9, 12, 1);
    block(&mut ram, B, 12, 3);
    block(&mut ram, D, 2, 3);
    let mut dsp = Dsp::default();
    for (r, v) in [
        (0x6c, 0x20),
        (0x5d, 0x20),
        (0x0c, 64),
        (0x1c, 64),
        (0, 64),
        (1, 128),
        (2, pitch as u8),
        (3, (pitch >> 8) as u8),
        (7, 64),
        (0x4c, 1),
    ] {
        dsp.write(r, v);
    }
    (dsp, ram)
}
fn delay(dsp: &mut Dsp, ram: &mut [u8]) {
    for _ in 0..5 {
        assert_eq!(dsp.sample(ram), (0, 0));
        assert_eq!([dsp.regs[8], dsp.regs[9], dsp.regs[0x7c]], [0; 3]);
    }
}

// Independent arithmetic on a specified sequence of decoded filter-0 values.
// No production BRR/phase/envelope helpers or state transitions are used.
fn output(sequence: &[i32], count: usize, half: bool, env: i32) -> i32 {
    let weights = if half {
        [56, 965, 969, 58]
    } else {
        [370, 1305, 374, 0]
    };
    let source: i32 = weights
        .into_iter()
        .enumerate()
        .map(|(i, weight)| {
            let value = count.checked_sub(4 - i).map_or(0, |index| sequence[index]);
            (value * weight) >> 10
        })
        .sum();
    (((source & !1) * env) >> 11) & !1
}

#[test]
fn normal_to_terminal_header_mutes_before_end_with_an_unaffected_voice() {
    for pitch in [0x1000, 0x1800, 0x3000] {
        for adsr in [false, true] {
            for flags in [0, 2] {
                for (first, terminal) in [(4, 12), (12, 6)] {
                    let (mut dsp, mut ram) = setup(pitch);
                    block(&mut ram, A, first, flags);
                    block(&mut ram, A + 9, terminal, 1);
                    dsp.write(1, 0); // voice 0 left; unaffected voice 1 right
                    for (r, v) in [
                        (0x11, 128),
                        (0x12, pitch as u8),
                        (0x13, (pitch >> 8) as u8),
                        (0x14, 1),
                        (0x17, 64),
                        (0x4c, 3),
                    ] {
                        dsp.write(r, v);
                    }
                    if adsr {
                        dsp.write(5, 0x8f);
                        dsp.write(6, 0xe0);
                    }
                    delay(&mut dsp, &mut ram);
                    let signed = if first == 12 { -2048 } else { 2048 };
                    let terminal_signed = if terminal == 12 { -2048 } else { 3072 };
                    let zero = [
                        vec![signed; 16],
                        vec![terminal_signed; 16],
                        vec![-2048; 112],
                    ]
                    .concat();
                    let one = [1024; 144];
                    for tick in 1..=40usize {
                        let position = tick * pitch as usize;
                        let count = position / 4096;
                        let env = if count > 16 {
                            0
                        } else if adsr && tick > 1 {
                            2047
                        } else {
                            1024
                        };
                        let left = output(&zero, count, !position.is_multiple_of(4096), env);
                        let right = output(&one, count, !position.is_multiple_of(4096), 1024);
                        assert_eq!(
                            dsp.sample(&mut ram),
                            ((left >> 2) as i16, (-right >> 1) as i16),
                            "pitch {pitch:04x}, adsr {adsr}, flags {flags}, tick {tick}"
                        );
                        assert_eq!(
                            [dsp.regs[8], dsp.regs[9]],
                            [(env >> 4) as u8, (left >> 8) as u8]
                        );
                        assert_eq!([dsp.regs[24], dsp.regs[25]], [64, (right >> 8) as u8]);
                        assert_eq!(
                            dsp.regs[0x7c],
                            u8::from(count > 32) | (u8::from(count > 16) << 1)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn terminal_at_start_and_all_flag_controls_have_separate_header_and_end_events() {
    for pitch in [0x1000, 0x1800, 0x3000] {
        for adsr in [false, true] {
            for flags in 0..=3 {
                let (mut dsp, mut ram) = setup(pitch);
                block(&mut ram, A, 12, flags);
                block(&mut ram, A + 9, 4, 3);
                block(&mut ram, B, 4, 3);
                if adsr {
                    dsp.write(5, 0x8f);
                    dsp.write(6, 0xe0);
                }
                delay(&mut dsp, &mut ram);
                let sequence = [vec![-2048; 16], vec![2048; 128]].concat();
                for tick in 1..=40usize {
                    let position = tick * pitch as usize;
                    let count = position / 4096;
                    let env = if flags == 1 {
                        0
                    } else if adsr && tick > 1 {
                        2047
                    } else {
                        1024
                    };
                    let out = output(&sequence, count, !position.is_multiple_of(4096), env);
                    assert_eq!(
                        dsp.sample(&mut ram),
                        ((out >> 2) as i16, (-out >> 1) as i16)
                    );
                    assert_eq!(
                        [dsp.regs[8], dsp.regs[9]],
                        [(env >> 4) as u8, (out >> 8) as u8]
                    );
                    assert_eq!(
                        dsp.regs[0x7c],
                        u8::from(count > if flags & 1 != 0 { 16 } else { 32 })
                    );
                }
            }
        }
    }
}

#[test]
fn silent_end_chain_uses_live_selection_and_only_kon_revives_it() {
    let (mut dsp, mut ram) = setup(0x1000);
    block(&mut ram, A, 4, 1);
    for (addr, nibble, flags) in [(B, 12, 0), (B + 9, 6, 3), (C, 1, 2), (C + 9, 14, 3)] {
        block(&mut ram, addr, nibble, flags);
    }
    entry(&mut ram, 0x2104, D, C);
    delay(&mut dsp, &mut ram);
    for count in 1..=100 {
        assert_eq!(dsp.sample(&mut ram), (0, 0), "count {count}");
        assert_eq!([dsp.regs[8], dsp.regs[9]], [0, 0]);
        // A -> B/B+9 -> C/C+9 -> D -> D. Clear each event immediately.
        let event = [17, 49, 81, 97].contains(&count);
        assert_eq!(dsp.regs[0x7c], u8::from(event), "count {count}");
        if event {
            dsp.write(0x7c, 0);
        }
        if count == 24 {
            dsp.write(0x5d, 0x21);
            dsp.write(4, 1);
            dsp.write(7, 127);
            dsp.write(5, 0x8f);
            dsp.write(6, 0xff);
        }
        if count == 56 {
            word(&mut ram, 0x2106, D); // stable live RAM change while silent
            dsp.write(5, 0);
            dsp.write(7, 64);
        }
    }
    // Change only the current start word: KON must distinguish it from loop D.
    block(&mut ram, D + 9, 6, 3);
    word(&mut ram, 0x2104, D + 9);
    dsp.write(0x4c, 1);
    delay(&mut dsp, &mut ram);
    let restarted = [vec![3072; 16], vec![1024; 16]].concat();
    for count in 1..=20 {
        let out = output(&restarted, count, false, 1024);
        assert_eq!(
            dsp.sample(&mut ram),
            ((out >> 2) as i16, (-out >> 1) as i16)
        );
        assert_eq!(
            [dsp.regs[8], dsp.regs[9], dsp.regs[0x7c]],
            [64, (out >> 8) as u8, u8::from(count > 16)]
        );
    }
}

#[test]
fn koff_fades_then_continues_silent_end_processing_without_revival() {
    let (mut dsp, mut ram) = setup(0x1000);
    block(&mut ram, A, 4, 3);
    block(&mut ram, B, 4, 3);
    delay(&mut dsp, &mut ram);
    for _ in 0..8 {
        dsp.sample(&mut ram);
    }
    dsp.write(0x5c, 1);
    for tick in 1..=128 {
        let env = 1024 - tick * 8;
        let out = ((4098 * env) >> 11) & !1;
        assert_eq!(
            dsp.sample(&mut ram),
            ((out >> 2) as i16, (-out >> 1) as i16)
        );
        assert_eq!(
            [dsp.regs[8], dsp.regs[9]],
            [(env >> 4) as u8, (out >> 8) as u8]
        );
    }
    dsp.write(0x5c, 0);
    dsp.write(7, 127);
    dsp.write(0x7c, 0xff);
    for _ in 0..16 {
        assert_eq!(dsp.sample(&mut ram), (0, 0));
    }
    assert_eq!([dsp.regs[8], dsp.regs[9], dsp.regs[0x7c]], [0, 0, 1]);
}

fn guest_data(ram: &mut [u8]) {
    entry(ram, 0x2000, A, B);
    entry(ram, 0x2004, D, C);
    block(ram, A, 4, 1);
    // The first terminal completion redirects into 64 sequential blocks.
    // This leaves a wide, deterministic window for clearing and reading ENDX.
    for index in 0..64 {
        block(ram, B + index * 9, 12, if index == 63 { 3 } else { 0 });
    }
    block(ram, C, 6, 3);
    block(ram, D, 2, 3);
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
    for command in 1..=5u8 {
        code.extend([0xe4, 0xf4, 0x68, command, 0xd0, 0xfa]);
        for (offset, reg) in [8, 9, 0x7c].into_iter().enumerate() {
            code.extend([0x8f, reg, 0xf2, 0xe4, 0xf3, 0xc5]);
            code.extend((0x1000u16 + 3 * (command as u16 - 1) + offset as u16).to_le_bytes());
        }
        match command {
            1 => write(&mut code, 0x7c, 0xff),
            2 => {
                // stable loop-word edit before the long chain completes
                code.extend([0xe8, 0, 0xc5, 2, 0x20, 0xe8, 0x34, 0xc5, 3, 0x20]);
                write(&mut code, 7, 127); // direct GAIN cannot revive release
                write(&mut code, 5, 0x8f); // neither can ADSR
                write(&mut code, 6, 0xe0);
            }
            3 => {
                // current SRCN selects a distinct start/loop, still no KON
                write(&mut code, 4, 1);
                write(&mut code, 5, 0);
                write(&mut code, 7, 64);
            }
            4 => write(&mut code, 0x4c, 1),
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
    assert_eq!(&apu.ram[0x200..0x200 + code.len()], code);
    assert!(apu.samples.len() / 2 < 100, "bounded setup");
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
fn anchor(apu: &Apu, sample: usize, pcm: (i16, i16)) {
    for index in sample - 19..=sample {
        assert_eq!(
            &apu.samples[2 * (index - 1)..2 * index],
            &[pcm.0 as f32 / 32768.0, pcm.1 as f32 / 32768.0],
            "PCM sample {index}"
        );
    }
}
const CHECKPOINTS: [u64; 5] = [100, 200, 1300, 1400, 1500];
const RECORDS: [u8; 15] = [0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 1, 64, 12, 1];

#[test]
fn uploaded_spc_records_terminal_silence_clear_reassert_and_kon() {
    let mut apu = Apu::default();
    guest_data(&mut apu.ram);
    let mut master = upload(&mut apu);
    for (index, sample) in CHECKPOINTS.into_iter().enumerate() {
        tick_to(&mut apu, &mut master, sample, false);
        // After KON, D is the start-only transient; the live loop C settles.
        anchor(
            &apu,
            sample as usize,
            if index == 4 { (768, -1536) } else { (0, 0) },
        );
        apu.port_write(0, index as u8 + 1);
        wait_for(&mut apu, &mut master, index as u8 + 1);
        assert_eq!(
            &apu.ram[0x1000..0x1000 + 3 * (index + 1)],
            &RECORDS[..3 * (index + 1)]
        );
    }
    tick_to(&mut apu, &mut master, 1510, false);
    assert!(apu.cpu.halted);
    assert_eq!(&apu.ram[0x2002..0x2004], &C.to_le_bytes());
    assert_eq!(apu.dsp.regs[4], 1);
}

#[test]
fn uploaded_guest_matches_every_pcm_register_ram_and_port_across_tick_partitions() {
    let mut whole = Apu::default();
    let mut split = Apu::default();
    guest_data(&mut whole.ram);
    guest_data(&mut split.ram);
    let mut whole_master = upload(&mut whole);
    let mut split_master = upload(&mut split);
    assert_eq!(whole_master, split_master);
    for (index, sample) in CHECKPOINTS.into_iter().enumerate() {
        for (checkpoint, acknowledge) in [(sample, false), (sample + 10, true)] {
            tick_to(&mut whole, &mut whole_master, checkpoint, false);
            tick_to(&mut split, &mut split_master, checkpoint, true);
            assert_eq!(
                whole.samples, split.samples,
                "every PCM sample through {checkpoint}"
            );
            assert_eq!(whole.dsp.regs, split.dsp.regs, "all 128 DSP registers");
            assert_eq!(whole.ram, split.ram, "all 64 KiB RAM");
            assert_eq!(whole.output, split.output);
            if acknowledge {
                assert_eq!(whole.output[0], index as u8 + 1, "bounded acknowledgement");
                assert_eq!(
                    &whole.ram[0x1000..0x1000 + 3 * (index + 1)],
                    &RECORDS[..3 * (index + 1)]
                );
            } else {
                anchor(
                    &whole,
                    sample as usize,
                    if index == 4 { (768, -1536) } else { (0, 0) },
                );
                whole.port_write(0, index as u8 + 1);
                split.port_write(0, index as u8 + 1);
            }
        }
    }
    assert!(whole.cpu.halted && split.cpu.halted);
}
