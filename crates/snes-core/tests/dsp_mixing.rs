//! Original MIT sample-level mixing vectors and SPC700 guest (Issue #73).
//! Copyright 2026 snes-rust contributors. See tests/roms/PROVENANCE.md.
use snes_core::{apu::Apu, dsp::Dsp};

// No private voice state: held noise, direct half GAIN, public key-on delay.
fn configure(dsp: &mut Dsp, voices: &[(u8, i8, i8)], eon: u8, master: (i8, i8)) {
    let mut mask = 0;
    for &(voice, left, right) in voices {
        mask |= 1 << voice;
        for (offset, value) in [(0, left as u8), (1, right as u8), (7, 64)] {
            dsp.write(voice * 16 + offset, value);
        }
    }
    for (reg, value) in [
        (0x6c, 0),
        (0x6d, 0x60),
        (0x3d, mask),
        (0x4d, eon),
        (0x0c, master.0 as u8),
        (0x1c, master.1 as u8),
        (0x4c, mask),
    ] {
        dsp.write(reg, value);
    }
}
fn keyed(voices: &[(u8, i8, i8)], eon: u8, master: (i8, i8)) -> (Dsp, Vec<u8>) {
    let mut dsp = Dsp::default();
    let mut ram = vec![0; 65536];
    configure(&mut dsp, voices, eon, master);
    for _ in 0..5 {
        assert_eq!(dsp.sample(&mut ram), (0, 0));
        assert_eq!(&ram[0x6000..0x6004], &[0; 4]);
    }
    (dsp, ram)
}
fn bytes(left: i16, right: i16) -> [u8; 4] {
    let [a, b] = left.to_le_bytes();
    let [c, d] = right.to_le_bytes();
    [a, b, c, d]
}

#[test]
fn ordered_overflow_cancellation_and_voice_slots() {
    // Literal dry/send values derived before running the core. Negative
    // volumes -128/-64 contribute +16384/+8192; +127 contributes -16256.
    for (volumes, slots, dry, word) in [
        (vec![-128, -128, -128, 64], vec![0, 1, 2, 3], 12287, 24574),
        (vec![-128, -128, -128], vec![0, 1, 2], 16383, 32766),
        (vec![-128, 64, -128, -128], vec![0, 1, 2, 3], 16383, 32766),
        (vec![127, 127, 127], vec![0, 1, 2], -16384, -32768),
        (vec![127, 127, 127, -64], vec![0, 1, 2, 3], -12288, -24576),
        (vec![127, -64, 127, 127], vec![0, 1, 2, 3], -16384, -32768),
        (vec![-128, -128, -128, 64], vec![1, 4, 6, 7], 12287, 24574),
        (vec![127, 127, 127, -64], vec![1, 4, 6, 7], -12288, -24576),
        (vec![-128, -128, -128], vec![5, 6, 7], 16383, 32766),
        (vec![-128, 64], vec![0, 7], 4096, 8192),
        (vec![], vec![], 0, 0),
        (vec![0; 8], (0..8).collect(), 0, 0),
        (vec![-128; 8], (0..8).collect(), 16383, 32766),
        (vec![127; 8], (0..8).collect(), -16384, -32768),
    ] {
        let voices: Vec<_> = slots
            .into_iter()
            .zip(volumes)
            .map(|(i, v)| (i, v, v))
            .collect();
        let (mut dsp, mut ram) = keyed(&voices, 255, (64, 64));
        for _ in 0..3 {
            assert_eq!(dsp.sample(&mut ram), (dry, dry), "{voices:?}");
            assert_eq!(&ram[0x6000..0x6004], &bytes(word, word), "{voices:?}");
        }
    }
}

#[test]
fn issue_49152_master_127_reproduction() {
    let (mut dsp, mut ram) = keyed(
        &[(0, -128, -128), (1, -128, -128), (2, -128, -128)],
        255,
        (127, 127),
    );
    assert_eq!(dsp.sample(&mut ram), (32511, 32511));
    assert_eq!(&ram[0x6000..0x6004], &[0xfe, 0x7f, 0xfe, 0x7f]);
}

#[test]
fn independent_eon_stereo_and_signed_master_endpoints() {
    let voices = [(0, -128, 127), (1, -128, 127), (2, -128, 127), (7, 64, -64)];
    for (eon, send) in [
        (0, (0, 0)),
        (255, (24574, -24576)),
        (7, (32766, -32768)),
        (128, (-8192, 8192)),
    ] {
        let (mut dsp, mut ram) = keyed(&voices, eon, (64, 64));
        assert_eq!(dsp.sample(&mut ram), (12287, -12288));
        assert_eq!(&ram[0x6000..0x6004], &bytes(send.0, send.1));
    }
    // Dry clips but selected send is bounded, and vice versa above.
    let (mut dsp, mut ram) = keyed(&voices[..3], 1, (-1, -128));
    assert_eq!(dsp.sample(&mut ram), (-256, -32768));
    assert_eq!(&ram[0x6000..0x6004], &[0, 0x40, 0x80, 0xc0]);
    let (mut dsp, mut ram) = keyed(&voices[..3], 0, (-128, 127));
    assert_eq!(dsp.sample(&mut ram), (-32767, -32512));
}

#[test]
fn negative_voice_product_rounds_down_before_ordered_addition() {
    let (mut dsp, mut ram) = keyed(&[(0, 127, 1), (7, -128, -1)], 255, (127, 127));
    // Clock the original LFSR from $4000 to $0002 in 13 rate-31 steps.
    // Freeze it: source=4, GAIN=64 -> output=2; A0=1/0, A7=-2/-1.
    dsp.write(0x6c, 31);
    for _ in 0..13 {
        dsp.sample(&mut ram);
    }
    dsp.write(0x6c, 0);
    assert_eq!(dsp.sample(&mut ram), (-1, -1));
    assert_eq!(&ram[0x6000..0x6004], &[0xfe, 0xff, 0xfe, 0xff]);
}

#[test]
fn clipped_send_feedback_audible_echo_mute_and_write_disable() {
    let voices = [(0, -128, 127), (1, -128, 127), (2, -128, 127), (7, 64, -64)];
    for (eon, efb, evol, flags, pcm, stored) in [
        (255, 64, 64, 0, (16383, -16384), (28670, -28672)),
        (7, 64, 64, 0, (16383, -16384), (32766, -32768)),
        (128, 64, 64, 0, (16383, -16384), (-4096, 4096)),
        (0, 64, 64, 0, (16383, -16384), (4096, -4096)),
        (7, -64, -128, 0, (4095, -4096), (28670, -28672)),
        (7, 64, 64, 64, (0, 0), (32766, -32768)),
        (7, 64, 64, 32, (16383, -16384), (16384, -16384)),
    ] {
        let (mut dsp, mut ram) = keyed(&voices, eon, (64, 64));
        for (reg, value) in [
            (0x7f, 64),
            (0x0d, efb as u8),
            (0x2c, evol as u8),
            (0x3c, evol as u8),
            (0x6c, flags),
        ] {
            dsp.write(reg, value);
        }
        ram[0x6000..0x6004].copy_from_slice(&[0, 0x40, 0, 0xc0]);
        // FIR7=64 -> +/-8192; EFB/EVOL=64 -> +/-4096.
        assert_eq!(dsp.sample(&mut ram), pcm);
        assert_eq!(&ram[0x6000..0x6004], &bytes(stored.0, stored.1));
    }
}

fn guest() -> Vec<u8> {
    fn dsp(code: &mut Vec<u8>, reg: u8, value: u8) {
        code.extend([0x8f, reg, 0xf2, 0x8f, value, 0xf3]);
    }
    let mut code = Vec::new();
    for (r, v) in [
        (0x6c, 0),
        (0x6d, 0x60),
        (0x3d, 0x87),
        (0x4d, 0x87),
        (0x0c, 64),
        (0x1c, 64),
    ] {
        dsp(&mut code, r, v);
    }
    for i in [0, 1, 2, 7] {
        for (offset, v) in [
            (0, if i == 7 { 0 } else { 128 }),
            (1, if i == 7 { 0 } else { 127 }),
            (7, 64),
        ] {
            dsp(&mut code, i * 16 + offset, v);
        }
    }
    dsp(&mut code, 0x4c, 0x87);
    code.extend([0x8f, 0x7f, 0xf4]);
    for command in 1..=5u8 {
        code.extend([0xe4, 0xf4, 0x68, command, 0xd0, 0xfa]);
        if command != 2 {
            let dest = 0x1000
                + if command == 1 {
                    0
                } else {
                    (command as u16 - 2) * 5
                };
            for offset in 0..4u16 {
                code.push(0xe5); // MOV A,!echo; MOV !record,A
                code.extend((0x6000 + offset).to_le_bytes());
                code.push(0xc5);
                code.extend((dest + offset).to_le_bytes());
            }
            code.extend([0x8f, 0x4d, 0xf2, 0xe4, 0xf3, 0xc5]); // record EON readback
            code.extend((dest + 4).to_le_bytes());
        }
        if command == 2 {
            dsp(&mut code, 0x70, 64);
            dsp(&mut code, 0x71, 192);
        } else if command == 3 {
            dsp(&mut code, 0x4d, 7);
        } else if command == 4 {
            dsp(&mut code, 0x4d, 128);
        }
        code.extend([0x8f, command, 0xf4]);
    }
    code.push(0xff);
    code
}
fn wait_for(apu: &mut Apu, value: u8) {
    for _ in 0..1000 {
        if apu.port_read(0) == value {
            return;
        }
        apu.tick(21);
        assert!(apu.error.is_none(), "{:?}", apu.error);
    }
    panic!(
        "SPC acknowledgement {value:02x} timed out at {:04x}",
        apu.cpu.pc
    );
}
fn upload(apu: &mut Apu) {
    let code = guest();
    assert!(code.len() < 512);
    for (port, value) in [(2, 0), (3, 2), (1, 1), (0, 0xcc)] {
        apu.port_write(port, value);
    }
    wait_for(apu, 0xcc);
    for (i, &byte) in code.iter().enumerate() {
        apu.port_write(1, byte);
        apu.port_write(0, i as u8);
        wait_for(apu, i as u8);
    }
    for (port, value) in [
        (2, 0),
        (3, 2),
        (1, 0),
        (0, (code.len() as u8).wrapping_add(1)),
    ] {
        apu.port_write(port, value);
    }
    wait_for(apu, 0x7f);
    assert_eq!(&apu.ram[0x200..0x200 + code.len()], code);
    assert!(apu.samples.len() / 2 < 200);
}
fn steady(apu: &mut Apu, pcm: (i16, i16), echo: [u8; 4]) {
    let start = apu.samples.len();
    apu.tick(32 * 21_477_272 / 1_024_000 * 20);
    assert!(apu.error.is_none());
    // Ignore first ten samples for key-on and command completion, then check all.
    for pair in apu.samples[start + 20..].chunks_exact(2) {
        assert_eq!(pair, &[pcm.0 as f32 / 32768.0, pcm.1 as f32 / 32768.0]);
    }
    assert_eq!(&apu.ram[0x6000..0x6004], &echo);
}

#[test]
fn uploaded_spc_records_overflow_cancellation_and_eon_changes() {
    let mut apu = Apu::default();
    upload(&mut apu);
    steady(&mut apu, (16383, -16384), [0xfe, 0x7f, 0, 0x80]);
    apu.port_write(0, 1);
    wait_for(&mut apu, 1);
    assert_eq!(&apu.ram[0x1000..0x1005], &[0xfe, 0x7f, 0, 0x80, 0x87]);
    apu.port_write(0, 2);
    wait_for(&mut apu, 2);
    steady(&mut apu, (12287, -12288), [0xfe, 0x5f, 0, 0xa0]);
    for (command, dest, recorded, next) in [
        (
            3,
            0x1005,
            [0xfe, 0x5f, 0, 0xa0, 0x87],
            [0xfe, 0x7f, 0, 0x80],
        ),
        (4, 0x100a, [0xfe, 0x7f, 0, 0x80, 7], [0, 0xe0, 0, 0x20]),
        (5, 0x100f, [0, 0xe0, 0, 0x20, 128], [0, 0xe0, 0, 0x20]),
    ] {
        apu.port_write(0, command);
        wait_for(&mut apu, command);
        assert_eq!(&apu.ram[dest..dest + 5], &recorded);
        steady(&mut apu, (12287, -12288), next);
    }
    for i in [0, 1, 2, 7] {
        assert_eq!(&apu.dsp.regs[i * 16 + 8..i * 16 + 10], &[64, 192]);
    }
    assert!(apu.cpu.halted);
    assert!(apu.error.is_none());
    assert!(apu.samples.len() / 2 < 350);
}

#[test]
fn multi_voice_pcm_registers_and_echo_match_irregular_apu_partitions() {
    for delay in [0, 1] {
        let mut whole = Apu::default();
        let mut split = Apu::default();
        upload(&mut whole);
        upload(&mut split);
        assert_eq!(whole.samples, split.samples);
        // Public configuration before the measured interval; no private clocks.
        whole.dsp.write(0x7d, delay);
        split.dsp.write(0x7d, delay);
        let initial = whole.samples.len() / 2;
        let mut master = 0;
        for (command, pcm, echo) in [
            (0, (16383, -16384), [0xfe, 0x7f, 0, 0x80]),
            (2, (12287, -12288), [0xfe, 0x5f, 0, 0xa0]),
            (3, (12287, -12288), [0xfe, 0x7f, 0, 0x80]),
            (4, (12287, -12288), [0, 0xe0, 0, 0x20]),
            (5, (12287, -12288), [0, 0xe0, 0, 0x20]),
        ] {
            // Command 1 records the initial state before command 2.
            if command == 2 {
                for apu in [&mut whole, &mut split] {
                    apu.port_write(0, 1);
                }
                whole.tick(10_000);
                split.tick(10_000);
                master += 10_000;
                assert_eq!(whole.port_read(0), 1);
                assert_eq!(split.port_read(0), 1);
            }
            if command != 0 {
                whole.port_write(0, command);
                split.port_write(0, command);
            }
            let start = whole.samples.len() / 2;
            for elapsed in [1, 8, 9, 511, 512, 513, 1024, 1025, 1537] {
                let target =
                    ((start + elapsed - initial) as u64 * 32 * 21_477_272).div_ceil(1_024_000);
                whole.tick(target - master);
                let mut remaining = target - master;
                for chunk in [1, 7, 666, 1001, 21, 8193].into_iter().cycle() {
                    if remaining == 0 {
                        break;
                    }
                    let step = remaining.min(chunk);
                    split.tick(step);
                    remaining -= step;
                }
                master = target;
                assert_eq!(whole.samples.len() / 2, start + elapsed);
                assert_eq!(whole.samples, split.samples);
                assert_eq!(whole.dsp.regs, split.dsp.regs);
                assert_eq!(&whole.ram[0x6000..0x6800], &split.ram[0x6000..0x6800]);
                assert_eq!(&whole.ram[0x1000..0x1014], &split.ram[0x1000..0x1014]);
                assert!(whole.error.is_none() && split.error.is_none());
                if elapsed >= 511 {
                    for pair in whole.samples[(start + 10) * 2..].chunks_exact(2) {
                        assert_eq!(pair, &[pcm.0 as f32 / 32768.0, pcm.1 as f32 / 32768.0]);
                    }
                }
            }
            let size = if delay == 0 { 4 } else { 2048 };
            for pair in whole.ram[0x6000..0x6000 + size].chunks_exact(4) {
                assert_eq!(pair, &echo);
            }
            if command != 0 {
                assert_eq!(whole.port_read(0), command);
            }
        }
        assert!(whole.cpu.halted && split.cpu.halted);
    }
}
