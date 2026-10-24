//! Original MIT BRR predictor vectors and firmware-free SPC700 guest.
//! Copyright 2026 snes-rust contributors. See tests/roms/PROVENANCE.md.
use snes_core::{apu::Apu, dsp::Dsp};

// Independently calculated from full-amplitude BRR recurrence, then phase-zero
// taps [370,1305,374,0]/1024, signed wrapping and preceding envelope E=1024.
// These literals never call a production decoder, interpolator or table.
const OUTPUTS: [[i32; 49]; 3] = [
    [
        226, 306, 496, 1444, 1652, 1246, 1144, 140, 482, 684, 1690, 1580, 432, 172, -376, -118,
        -484, -360, -128, 858, 1104, 732, 662, -312, 58, 288, 1318, 1232, 106, -132, -660, -386,
        -736, -594, -348, 652, 912, 550, 492, -472, -90, 146, 1186, 1110, -10, -240, -762, -482,
        -826,
    ],
    [
        1016, 1262, 1662, 2964, 4392, 5290, 5942, 5434, 5138, 4930, 5632, 6108, 5316, 4172, 2434,
        960, -826, -2380, -3556, -3566, -3166, -2994, -2766, -3396, -3532, -3316, -1960, -634,
        -418, -436, -978, -1220, -1786, -2166, -2246, -1272, -20, 852, 1618, 1352, 1412, 1656,
        2882, 3934, 3750, 3226, 2098, 1206, -40,
    ],
    [
        912, 1084, 1416, 2642, 3896, 4550, 4986, 4328, 4078, 4040, 4996, 5690, 5116, 4336, 3098,
        2276, 1198, 396, -52, 562, 1352, 1670, 1876, 1082, 770, 734, 1742, 2530, 2082, 1452, 382,
        -260, -1154, -1768, -2030, -1232, -268, 218, 584, -62, -236, -142, 986, 1882, 1530, 986,
        -6, -580, -1412,
    ],
];
const DATA: [u8; 8] = [0x4c, 0x2f, 0x61, 0xd3, 0x85, 0xe7, 0x09, 0x2b];
fn setup(filter: u8) -> (Dsp, Vec<u8>) {
    let mut ram = vec![0; 65536];
    ram[0x2000..0x2004].copy_from_slice(&[0, 0x30, 0, 0x30]);
    ram[0x3000] = 0x93 | filter << 2;
    ram[0x3001..0x3009].copy_from_slice(&DATA);
    let mut d = Dsp::default();
    for (r, v) in [
        (0x6c, 0x20),
        (0x5d, 0x20),
        (0x0c, 64),
        (0x1c, 64),
        (0, 64),
        (1, 128),
        (7, 64),
        (0x4c, 1),
    ] {
        d.write(r, v);
    }
    (d, ram)
}
fn numeric(d: &mut Dsp, ram: &mut [u8], out: i32, end: u8) {
    assert_eq!(d.sample(ram), ((out >> 2) as i16, (-out >> 1) as i16));
    assert_eq!(
        [d.regs[8], d.regs[9], d.regs[0x7c]],
        [64, (out >> 8) as u8, end]
    );
}
#[test]
fn each_filter_has_literal_pcm_outx_preload_hold_resume_and_loop_anchors() {
    for filter in 1..=3 {
        let outputs = OUTPUTS[filter as usize - 1];
        let (mut d, mut ram) = setup(filter);
        for _ in 0..5 {
            assert_eq!(d.sample(&mut ram), (0, 0));
            assert_eq!([d.regs[8], d.regs[9], d.regs[0x7c]], [0, 0, 0]);
        }
        for _ in 0..32 {
            numeric(&mut d, &mut ram, outputs[0], 0);
        }
        // Direct GAIN changes affect the next call, preserving #91 ordering.
        d.write(7, 0);
        numeric(&mut d, &mut ram, outputs[0], 0);
        for _ in 0..2 {
            assert_eq!(d.sample(&mut ram), (0, 0));
            assert_eq!([d.regs[8], d.regs[9]], [0, 0]);
        }
        d.write(7, 64);
        assert_eq!(d.sample(&mut ram), (0, 0));
        assert_eq!([d.regs[8], d.regs[9]], [0, 0]);
        numeric(&mut d, &mut ram, outputs[0], 0);
        d.write(3, 0x10);
        for (position, &out) in outputs.iter().enumerate().skip(1) {
            numeric(&mut d, &mut ram, out, u8::from(position >= 13));
            if position == 8 || position == 9 {
                d.write(3, 0);
                for _ in 0..16 {
                    numeric(&mut d, &mut ram, out, 0);
                }
                d.write(3, 0x10);
            }
        }
    }
}

fn write(code: &mut Vec<u8>, reg: u8, value: u8) {
    code.extend([0x8f, reg, 0xf2, 0x8f, value, 0xf3]);
}
fn guest() -> Vec<u8> {
    let mut code = Vec::new();
    for (r, v) in [
        (0x6c, 0x20),
        (0x5d, 0x20),
        (0x0c, 64),
        (0x1c, 64),
        (0, 64),
        (1, 128),
        (7, 64),
        (0x4c, 1),
    ] {
        write(&mut code, r, v);
    }
    code.extend([0x8f, 0x7f, 0xf4]);
    for command in 1..=11u8 {
        code.extend([0xe4, 0xf4, 0x68, command, 0xd0, 0xfa]);
        for (offset, reg) in [8, 9, 0x7c].into_iter().enumerate() {
            code.extend([0x8f, reg, 0xf2, 0xe4, 0xf3, 0xc5]);
            code.extend((0x1100u16 + 3 * (command as u16 - 1) + offset as u16).to_le_bytes());
        }
        if command < 11 {
            // MOV dp,#imm takes five SPC clocks. The two F3 effects are
            // 5 + eleven 2-clock NOPs + 5 = 32 clocks apart: exactly one
            // sample advances at pitch $1000, regardless of starting phase.
            write(&mut code, 3, 0x10);
            code.extend([0x00; 11]);
            write(&mut code, 3, 0);
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
fn upload_code(apu: &mut Apu, code: &[u8]) -> u64 {
    assert!(code.len() < 1024);
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

#[test]
fn uploaded_spc_records_each_filter_across_preload_with_independent_partition_anchors() {
    for filter in 1..=3 {
        let code = guest();
        let mut whole = Apu::default();
        let mut split = Apu::default();
        let (_, ram) = setup(filter);
        whole.ram.copy_from_slice(&ram);
        split.ram.copy_from_slice(&ram);
        let mut wm = upload_code(&mut whole, &code);
        let mut sm = upload_code(&mut split, &code);
        for position in 0..=10 {
            let sample = 100 * (position as u64 + 2);
            for (checkpoint, ack) in [(sample, false), (sample + 10, true)] {
                for (apu, master, partition) in
                    [(&mut whole, &mut wm, false), (&mut split, &mut sm, true)]
                {
                    tick_to(apu, master, checkpoint, partition);
                    if ack {
                        assert_eq!(apu.output[0], position as u8 + 1);
                        let expected: Vec<u8> = OUTPUTS[filter as usize - 1][..=position]
                            .iter()
                            .flat_map(|&out| [64, (out >> 8) as u8, 0])
                            .collect();
                        assert_eq!(&apu.ram[0x1100..0x1100 + expected.len()], expected);
                    } else {
                        let out = OUTPUTS[filter as usize - 1][position];
                        let pcm = [(out >> 2) as f32 / 32768.0, (-out >> 1) as f32 / 32768.0];
                        for n in sample as usize - 19..=sample as usize {
                            assert_eq!(
                                &apu.samples[2 * (n - 1)..2 * n],
                                pcm,
                                "filter {filter}, window {position}, frame {n}, split {partition}"
                            );
                        }
                        assert_eq!(
                            [apu.dsp.regs[8], apu.dsp.regs[9], apu.dsp.regs[0x7c]],
                            [64, (out >> 8) as u8, 0]
                        );
                        apu.port_write(0, position as u8 + 1);
                    }
                }
                // Numeric assertions above are required independently for BOTH
                // executions before equality is used as determinism evidence.
                assert_eq!(whole.samples, split.samples);
                assert_eq!(whole.dsp.regs, split.dsp.regs);
                assert_eq!(whole.ram, split.ram);
                assert_eq!(whole.output, split.output);
            }
        }
        assert!(whole.cpu.halted && split.cpu.halted, "bounded STOP");
    }
}
