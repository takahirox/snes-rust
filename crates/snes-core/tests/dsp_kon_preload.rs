//! Original MIT KON preload vectors and firmware-free SPC700 guest.
//! Copyright 2026 snes-rust contributors. See tests/roms/PROVENANCE.md.
use snes_core::{apu::Apu, dsp::Dsp};

fn block(ram: &mut [u8], addr: usize, flags: u8, bytes: [u8; 8]) {
    ram[addr] = 0xa0 | flags;
    ram[addr + 1..addr + 9].copy_from_slice(&bytes);
}
fn setup(pitch: u16, flags: u8, bytes: [u8; 8]) -> (Dsp, Vec<u8>) {
    let mut ram = vec![0; 65536];
    ram[0x2000..0x2008].copy_from_slice(&[0, 0x30, 0, 0x31, 0, 0x32, 0, 0x33]);
    block(&mut ram, 0x3000, flags, bytes);
    block(&mut ram, 0x3009, 3, [0x22; 8]);
    block(&mut ram, 0x3100, 3, [0xcc; 8]);
    block(&mut ram, 0x3200, 3, [0x66; 8]);
    block(&mut ram, 0x3300, 3, [0x11; 8]);
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
fn check(d: &mut Dsp, ram: &mut [u8], pcm: (i16, i16), regs: [u8; 3]) {
    assert_eq!(d.sample(ram), pcm);
    assert_eq!([d.regs[8], d.regs[9], d.regs[0x7c]], regs);
}
fn delay(d: &mut Dsp, ram: &mut [u8]) {
    for _ in 0..5 {
        check(d, ram, (0, 0), [0; 3]);
    }
}

#[test]
fn signed_constant_startup_at_zero_and_one_pitch_and_all_flags() {
    // Source-derived #1..#5 silence, #6 phase zero; pitch 1 stays in
    // coefficient bin zero for the following fifteen calls. No noise/PMON.
    // +4: decoded 2048, source 4098, E=1024 -> O=2048.
    // -4: decoded -2048, source -4098 -> O=-2050 (signed rounding).
    for pitch in [0, 1] {
        for flags in 0..=3 {
            for (byte, pcm, out) in [(0x44, (512, -1024), 8), (0xcc, (-513, 1025), 247)] {
                let (mut d, mut ram) = setup(pitch, flags, [byte; 8]);
                delay(&mut d, &mut ram);
                for _ in 0..16 {
                    if flags == 1 {
                        check(&mut d, &mut ram, (0, 0), [0; 3]);
                    } else {
                        check(&mut d, &mut ram, pcm, [64, out, 0]);
                    }
                }
                d.write(2, 0);
                for _ in 0..128 {
                    if flags == 1 {
                        check(&mut d, &mut ram, (0, 0), [0; 3]);
                    } else {
                        check(&mut d, &mut ram, pcm, [64, out, 0]);
                    }
                }
            }
        }
    }
}

// Independent signed filter-0 sequence (nibble *512). At phase zero each
// product shifts separately using [370,1305,374,0]/1024. These literals
// select windows 0..3, 1..4, ...; no production decoder/table is used.
const BYTES: [u8; 8] = [0x4c, 0x2f, 0x61, 0xd3, 0x85, 0xe7, 0x09, 0x2b];
const OUTPUTS: [i32; 17] = [
    -748, 188, 418, 1958, 600, -606, -48, -1866, 704, 464, 2098, -8, -2098, -464, -1822, -2142,
    -2050,
];
fn numeric(d: &mut Dsp, ram: &mut [u8], output: i32, end: u8) {
    check(
        d,
        ram,
        ((output >> 2) as i16, (-output >> 1) as i16),
        [64, (output >> 8) as u8, end],
    );
}
#[test]
fn signed_windows_hold_resume_return_to_zero_and_retrigger() {
    let (mut d, mut ram) = setup(0, 3, BYTES);
    delay(&mut d, &mut ram);
    for _ in 0..32 {
        numeric(&mut d, &mut ram, OUTPUTS[0], 0);
    }
    d.write(3, 0x10);
    for (index, &out) in OUTPUTS.iter().enumerate().take(9).skip(1) {
        numeric(&mut d, &mut ram, out, 0);
        if index == 8 {
            d.write(3, 0);
        }
    }
    for _ in 0..32 {
        numeric(&mut d, &mut ram, OUTPUTS[8], 0);
    }
    d.write(3, 0x10);
    for (index, &out) in OUTPUTS.iter().enumerate().skip(9) {
        numeric(&mut d, &mut ram, out, u8::from(index >= 13));
    }
    d.write(3, 0);
    d.write(0x7c, 0xff);
    for _ in 0..32 {
        numeric(&mut d, &mut ram, -2050, 0);
    }
    d.write(4, 1); // new start +6, distinct loop +1
    d.write(0x4c, 1);
    delay(&mut d, &mut ram);
    for _ in 0..32 {
        numeric(&mut d, &mut ram, 3072, 0);
    }
    d.write(3, 0x10);
    for position in 1..=20 {
        // Independent transition +6 -> +1 at windows 13,14,15,16.
        let out = match position {
            1..=13 => 3072,
            14 => 2606,
            15 => 974,
            _ => 512,
        };
        numeric(&mut d, &mut ram, out, u8::from(position >= 13));
    }
}

#[test]
fn retrigger_does_not_disturb_an_independently_running_voice_or_clock() {
    let (mut d, mut ram) = setup(0, 0, [0x44; 8]);
    // Voice 1 remains on a distinct constant loop; it alone uses right output.
    d.write(1, 0);
    for (r, v) in [(0x11, 128), (0x13, 0x10), (0x14, 1), (0x17, 64), (0x4c, 3)] {
        d.write(r, v);
    }
    for _ in 0..40 {
        d.sample(&mut ram);
    }
    assert_eq!([d.regs[24], d.regs[25]], [64, 2]); // loop +1
    d.write(4, 1);
    d.write(0x4c, 1);
    for _ in 0..5 {
        assert_eq!(d.sample(&mut ram), (0, -256));
        assert_eq!(
            [d.regs[8], d.regs[9], d.regs[24], d.regs[25], d.regs[0x7c]],
            [0, 0, 64, 2, 2]
        );
    }
    for _ in 0..20 {
        assert_eq!(d.sample(&mut ram), (768, -256));
        assert_eq!(
            [d.regs[8], d.regs[9], d.regs[24], d.regs[25], d.regs[0x7c]],
            [64, 12, 64, 2, 2]
        );
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
        (7, 64),
        (0x2d, 0),
        (0x3d, 0),
        (0x4c, 1),
    ] {
        write(&mut code, r, v);
    }
    code.extend([0x8f, 0x7f, 0xf4]);
    for command in 1..=6u8 {
        code.extend([0xe4, 0xf4, 0x68, command, 0xd0, 0xfa]);
        for (offset, reg) in [8, 9, 0x7c].into_iter().enumerate() {
            code.extend([0x8f, reg, 0xf2, 0xe4, 0xf3, 0xc5]);
            code.extend((0x1100u16 + 3 * (command as u16 - 1) + offset as u16).to_le_bytes());
        }
        match command {
            1 | 4 => write(&mut code, 3, 0x10),
            2 => {
                write(&mut code, 3, 0);
                write(&mut code, 0x7c, 0xff);
            }
            3 => {
                write(&mut code, 4, 1);
                write(&mut code, 0x4c, 1);
            }
            5 => write(&mut code, 3, 0),
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
    upload_code(apu, &guest())
}
fn upload_code(apu: &mut Apu, code: &[u8]) -> u64 {
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

#[test]
fn uploaded_spc_start_hold_resume_records_and_every_tick_partition_match() {
    let records = [
        64, 8, 0, 64, 247, 1, 64, 247, 0, 64, 12, 0, 64, 2, 1, 64, 2, 1,
    ];
    let pcms = [
        (512, -1024),
        (-513, 1025),
        (-513, 1025),
        (768, -1536),
        (128, -256),
        (128, -256),
    ];
    let mut whole = Apu::default();
    let mut split = Apu::default();
    let (_, ram) = setup(0, 3, [0x44; 8]);
    whole.ram.copy_from_slice(&ram);
    split.ram.copy_from_slice(&ram);
    let mut wm = upload(&mut whole);
    let mut sm = upload(&mut split);
    assert_eq!(wm, sm);
    for (index, pcm) in pcms.into_iter().enumerate() {
        let sample = (index as u64 + 1) * 100;
        for (checkpoint, ack) in [(sample, false), (sample + 10, true)] {
            tick_to(&mut whole, &mut wm, checkpoint, false);
            tick_to(&mut split, &mut sm, checkpoint, true);
            assert_eq!(whole.samples, split.samples, "every PCM sample");
            assert_eq!(whole.dsp.regs, split.dsp.regs, "all DSP registers");
            assert_eq!(whole.ram, split.ram, "all RAM");
            assert_eq!(whole.output, split.output, "all output ports");
            if ack {
                assert_eq!(whole.output[0], index as u8 + 1);
                assert_eq!(
                    &whole.ram[0x1100..0x1100 + 3 * (index + 1)],
                    &records[..3 * (index + 1)]
                );
            } else {
                for n in sample as usize - 19..=sample as usize {
                    assert_eq!(
                        &whole.samples[2 * (n - 1)..2 * n],
                        &[pcm.0 as f32 / 32768.0, pcm.1 as f32 / 32768.0]
                    );
                }
                assert_eq!(
                    [whole.dsp.regs[8], whole.dsp.regs[9], whole.dsp.regs[0x7c]],
                    records[3 * index..3 * index + 3]
                );
                whole.port_write(0, index as u8 + 1);
                split.port_write(0, index as u8 + 1);
            }
        }
    }
    assert!(whole.cpu.halted && split.cpu.halted, "bounded guest STOP");
}

#[test]
fn low_pitch_crosses_fractional_bins_then_resumes_the_next_window_once() {
    let (mut d, mut ram) = setup(1, 3, BYTES);
    delay(&mut d, &mut ram);
    // Published coefficient bins 0,1,128,255 then next integer window.
    // bin1 [366,1305,378,0]; bin128 [56,965,969,58];
    // bin255 [0,374,1305,370]. Per-product signed shifts, E=1024.
    for tick in 1..=4097 {
        let pcm = d.sample(&mut ram);
        assert_eq!([d.regs[8], d.regs[0x7c]], [64, 0]);
        let output = match tick {
            1 => Some(-748),
            17 => Some(-750),
            2049 => Some(-440),
            4081 => Some(186),
            4097 => Some(188),
            _ => None,
        };
        if let Some(out) = output {
            assert_eq!(pcm, ((out >> 2) as i16, (-out >> 1) as i16), "tick {tick}");
            assert_eq!(d.regs[9], (out >> 8) as u8);
        }
    }
    d.write(2, 0);
    for _ in 0..32 {
        numeric(&mut d, &mut ram, 188, 0);
    }
    d.write(3, 0x10);
    numeric(&mut d, &mut ram, 418, 0);
    d.write(3, 0);
    for _ in 0..32 {
        numeric(&mut d, &mut ram, 418, 0);
    }
}
