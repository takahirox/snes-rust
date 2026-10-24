//! Original MIT vectors and firmware-free SPC700 guest for Issue #91.
//! Expectations are specification-derived literals, never emulator traces.
use snes_core::{apu::Apu, dsp::Dsp};

fn setup(gain: u8) -> (Dsp, Vec<u8>) {
    let mut d = Dsp::default();
    let ram = vec![0; 65536];
    for (r, v) in [
        (0x6c, 0x20),
        (0x3d, 1),
        (0, 64),
        (1, 64),
        (0x0c, 64),
        (0x1c, 64),
        (7, gain),
        (0x4c, 1),
    ] {
        d.write(r, v);
    }
    (d, ram)
}
fn check(d: &mut Dsp, ram: &mut [u8], pcm: i16, envx: u8, outx: u8) {
    assert_eq!(d.sample(ram), (pcm, pcm));
    assert_eq!([d.regs[8], d.regs[9]], [envx, outx]);
}
fn warm(d: &mut Dsp, ram: &mut [u8], pcm: i16, envx: u8, outx: u8) {
    for _ in 0..5 {
        check(d, ram, 0, 0, 0);
    }
    for _ in 0..3 {
        check(d, ram, pcm, envx, outx);
    }
}

#[test]
fn direct_and_hidden_bent_changes_apply_the_preceding_envelope() {
    let (mut d, mut ram) = setup(94);
    warm(&mut d, &mut ram, -6016, 94, 162);
    d.write(7, 96);
    check(&mut d, &mut ram, -6016, 94, 162); // commit 1536
    check(&mut d, &mut ram, -6144, 96, 160);
    check(&mut d, &mut ram, -6144, 96, 160);

    let (mut d, mut ram) = setup(94);
    warm(&mut d, &mut ram, -6016, 94, 162);
    d.write(7, 0xe0); // held E=1504, candidate H=1536
    check(&mut d, &mut ram, -6016, 94, 162);
    d.write(7, 0xff);
    check(&mut d, &mut ram, -6016, 94, 162); // unsigned H >= $600: +8
    check(&mut d, &mut ram, -6048, 94, 161); // applies 1512, commits 1544
    check(&mut d, &mut ram, -6176, 96, 159);
}

#[test]
fn all_dynamic_gain_modes_have_events_and_rate_zero_holds() {
    // Start E=1024; linear -32, exponential -4, linear/bent +32.
    for (gain, pcm, env, out) in [
        (0x9f, -3968, 62, 194),
        (0xbf, -4080, 63, 192),
        (0xdf, -4224, 66, 190),
        (0xff, -4224, 66, 190),
    ] {
        let (mut d, mut ram) = setup(64);
        warm(&mut d, &mut ram, -4096, 64, 192);
        d.write(7, gain);
        check(&mut d, &mut ram, -4096, 64, 192);
        d.write(7, gain & 0xe0); // hold newly committed E, not a second event
        for _ in 0..4 {
            check(&mut d, &mut ram, pcm, env, out);
        }
    }
}

#[test]
fn fifth_silent_call_crosses_rate_29_boundary_without_a_sixth_silence() {
    let (mut d, mut ram) = setup(0xdd);
    for _ in 0..5 {
        check(&mut d, &mut ram, 0, 0, 0);
    }
    // Rate 29 fires at absolute 2,5,8,11; initialization runs at 5.
    for (pcm, env, out) in [
        (-128, 2, 254),
        (-128, 2, 254),
        (-128, 2, 254),
        (-256, 4, 252),
        (-256, 4, 252),
        (-256, 4, 252),
        (-384, 6, 250),
    ] {
        check(&mut d, &mut ram, pcm, env, out);
    }
}

#[test]
fn adsr_attack_decay_sustain_events_and_holds_publish_applied_state() {
    let (mut d, mut ram) = setup(0);
    d.write(5, 0xff);
    d.write(6, 0xc3);
    for _ in 0..5 {
        check(&mut d, &mut ram, 0, 0, 0);
    }
    for (pcm, env, out) in [
        (-4096, 64, 192),  // call 6 applies call 5's fast attack
        (-8188, 127, 128), // call 7 holds decay (rate 30, even ticks)
        (-8188, 127, 128), // call 8 commits 2039
        (-8156, 127, 128), // call 9 consumes 2039
    ] {
        check(&mut d, &mut ram, pcm, env, out);
    }
    for _ in 10..=69 {
        d.sample(&mut ram);
    }
    check(&mut d, &mut ram, -7196, 112, 143); // sustain entered from held candidate
    for _ in 71..=535 {
        check(&mut d, &mut ram, -7196, 112, 143);
    }
    check(&mut d, &mut ram, -7196, 112, 143); // tick 536 commits 1791
    check(&mut d, &mut ram, -7164, 111, 144);
    check(&mut d, &mut ram, -7164, 111, 144);

    let (mut d, mut ram) = setup(94);
    warm(&mut d, &mut ram, -6016, 94, 162); // through tick 8
    d.write(5, 0x81); // attack rate 3, event 536
    for _ in 9..=536 {
        check(&mut d, &mut ram, -6016, 94, 162);
    }
    check(&mut d, &mut ram, -6144, 96, 160);
    check(&mut d, &mut ram, -6144, 96, 160);
}

#[test]
fn release_through_zero_retrigger_and_voice_isolation() {
    let (mut d, mut ram) = setup(1);
    warm(&mut d, &mut ram, -64, 1, 255);
    // Voice 1 uses only right, constant noise E=1024 -> right=-4096.
    d.write(1, 0);
    for (r, v) in [(0x3d, 3), (0x11, 64), (0x17, 64), (0x4c, 2)] {
        d.write(r, v);
    }
    for _ in 0..8 {
        d.sample(&mut ram);
    }
    assert_eq!([d.regs[24], d.regs[25]], [64, 192]);
    d.write(0x5c, 1);
    for (left, env, out) in [(-64, 1, 255), (-32, 0, 255), (0, 0, 0), (0, 0, 0)] {
        assert_eq!(d.sample(&mut ram), (left, -4096));
        assert_eq!(
            [d.regs[8], d.regs[9], d.regs[24], d.regs[25]],
            [env, out, 64, 192]
        );
    }
    d.write(0x5c, 0);
    d.write(7, 96);
    d.write(0x4c, 1);
    for _ in 0..5 {
        assert_eq!(d.sample(&mut ram), (0, -4096));
        assert_eq!(
            [d.regs[8], d.regs[9], d.regs[24], d.regs[25]],
            [0, 0, 64, 192]
        );
    }
    for _ in 0..3 {
        assert_eq!(d.sample(&mut ram), (-6144, -4096));
        assert_eq!(
            [d.regs[8], d.regs[9], d.regs[24], d.regs[25]],
            [96, 160, 64, 192]
        );
    }
}

#[test]
fn signed_constant_filter_zero_at_zero_pitch_separates_waveform_from_envelope() {
    // Range 10 nibbles +/-4 decode +/-2048; phase-zero source +/-4098.
    // E=1024 -> O=2048/-2050. E=1536 -> O=3072/-3074.
    for (byte, old_pcm, old_out, next_pcm, next_out) in
        [(0x44, 512, 8, 768, 12), (0xcc, -513, 247, -769, 243)]
    {
        let (mut d, mut ram) = setup(64);
        d.write(0x3d, 0);
        d.write(0x5d, 0x20);
        ram[0x2000..0x2004].copy_from_slice(&[0, 0x30, 0, 0x30]);
        ram[0x3000] = 0xa3;
        ram[0x3001..0x3009].fill(byte);
        warm(&mut d, &mut ram, old_pcm, 64, old_out);
        d.write(7, 96);
        check(&mut d, &mut ram, old_pcm, 64, old_out);
        for _ in 0..8 {
            check(&mut d, &mut ram, next_pcm, 96, next_out);
        }
    }
}

#[test]
fn echo_send_consumes_the_same_preceding_output_as_dry_mix_and_outx() {
    let (mut d, mut ram) = setup(94);
    warm(&mut d, &mut ram, -6016, 94, 162);
    d.write(0x6c, 0);
    d.write(0x6d, 0x60);
    d.write(0x4d, 1);
    d.write(7, 96);
    for (pcm, env, out, send) in [
        (-6016, 94, 162, -12032i16),
        (-6144, 96, 160, -12288),
        (-6144, 96, 160, -12288),
    ] {
        check(&mut d, &mut ram, pcm, env, out);
        assert_eq!(
            &ram[0x6000..0x6004],
            &[send.to_le_bytes(), send.to_le_bytes()].concat()
        );
    }
}

#[test]
fn adjacent_pmon_consumes_current_output_from_the_preceding_envelope() {
    let (mut d, mut ram) = setup(64);
    d.write(0, 0);
    d.write(1, 0);
    d.write(0x5d, 0x20);
    ram[0x2000..0x2004].copy_from_slice(&[0, 0x30, 0, 0x30]);
    ram[0x3000] = 0xa3;
    ram[0x3001..0x3009].copy_from_slice(&[0x44, 0x44, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc]);
    for (r, v) in [(0x10, 64), (0x11, 64), (0x17, 64), (0x2d, 2), (0x4c, 3)] {
        d.write(r, v);
    }
    for _ in 0..8 {
        d.sample(&mut ram);
    }
    assert_eq!(
        [d.regs[8], d.regs[9], d.regs[24], d.regs[25]],
        [64, 192, 64, 8]
    );
    d.write(7, 0);
    d.write(0x13, 0x10);
    // O0=-16384 gives target increment $800: window0/bin128 remains +4096.
    assert_eq!(d.sample(&mut ram), (512, 512));
    assert_eq!([d.regs[8], d.regs[9], d.regs[25]], [64, 192, 8]);
    // Next O0=0 gives $1000: window1/bin128, taps [56,965,969,58],
    // source=112+1930+1938-116=3864, O1=1932, PCM=483.
    assert_eq!(d.sample(&mut ram), (483, 483));
    assert_eq!([d.regs[8], d.regs[9], d.regs[25]], [0, 0, 7]);
    // window2/bin128: source=112+1930-1938-116=-12, O=-6, PCM=-2.
    assert_eq!(d.sample(&mut ram), (-2, -2));
    assert_eq!(d.regs[25], 255);
}

fn guest() -> (Vec<u8>, u16) {
    fn write(code: &mut Vec<u8>, r: u8, v: u8) {
        code.extend([0x8f, r, 0xf2, 0x8f, v, 0xf3]);
    }
    let mut code = Vec::new();
    for (r, v) in [
        (0x6c, 0x20),
        (0x3d, 1),
        (0, 64),
        (1, 64),
        (0x0c, 64),
        (0x1c, 64),
        (7, 94),
        (0x4c, 1),
    ] {
        write(&mut code, r, v);
    }
    code.extend([0x8f, 0x7f, 0xf4]);
    let mut first_write = 0;
    for command in 1..=8u8 {
        code.extend([0xe4, 0xf4, 0x68, command, 0xd0, 0xfa]);
        for (offset, reg) in [8, 9].into_iter().enumerate() {
            code.extend([0x8f, reg, 0xf2, 0xe4, 0xf3, 0xc5]);
            code.extend((0x1100u16 + 2 * (command as u16 - 1) + offset as u16).to_le_bytes());
        }
        match command {
            1 => {
                // Address of MOV $F3,#96, after selecting GAIN in $F2.
                first_write = 0x200 + code.len() as u16 + 3;
                write(&mut code, 7, 96);
            }
            2 => write(&mut code, 7, 0xc0),
            3 => write(&mut code, 7, 0xff),
            4 => write(&mut code, 7, 1),
            5 => write(&mut code, 0x5c, 1),
            6 => {
                write(&mut code, 0x5c, 0);
                write(&mut code, 7, 94);
                write(&mut code, 0x4c, 1);
            }
            7 => write(&mut code, 7, 96),
            _ => {}
        }
        code.extend([0x8f, command, 0xf4]);
    }
    code.push(0xff);
    (code, first_write)
}
fn wait_for(a: &mut Apu, master: &mut u64, value: u8) {
    for _ in 0..1000 {
        assert!(a.error.is_none(), "{:?}", a.error);
        if a.port_read(0) == value {
            return;
        }
        a.tick(21);
        *master += 21;
    }
    panic!("guest acknowledgement {value} timed out");
}
fn upload(a: &mut Apu, code: &[u8]) -> u64 {
    assert!(code.len() < 512);
    let mut master = 0;
    a.port_write(2, 0);
    a.port_write(3, 2);
    a.port_write(1, 1);
    a.port_write(0, 0xcc);
    wait_for(a, &mut master, 0xcc);
    for (index, &byte) in code.iter().enumerate() {
        a.port_write(1, byte);
        a.port_write(0, index as u8);
        wait_for(a, &mut master, index as u8);
    }
    a.port_write(2, 0);
    a.port_write(3, 2);
    a.port_write(1, 0);
    a.port_write(0, (code.len() as u8).wrapping_add(1));
    wait_for(a, &mut master, 0x7f);
    assert_eq!(&a.ram[0x200..0x200 + code.len()], code);
    assert!(a.samples.len() / 2 < 100);
    master
}
fn tick_to(a: &mut Apu, master: &mut u64, sample: u64, split: bool) {
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
        a.tick(step);
        remaining -= step;
    }
    *master = target;
    assert_eq!(a.samples.len() / 2, sample as usize);
}
fn same(a: &Apu, b: &Apu) {
    assert!(a.error.is_none() && b.error.is_none());
    assert_eq!(a.samples, b.samples);
    assert_eq!(a.dsp.regs, b.dsp.regs);
    assert_eq!(a.ram, b.ram);
    assert_eq!(a.output, b.output);
}
#[test]
fn uploaded_guest_live_changes_retrigger_and_counter_wrap_match_all_tick_partitions() {
    let (code, first_write) = guest();
    let mut a = Apu::default();
    let mut b = Apu::default();
    let mut am = upload(&mut a, &code);
    let mut bm = upload(&mut b, &code);
    same(&a, &b);
    for (command, tick, pcm, regs) in [
        (1, 100, -6016, [94, 162]),
        (2, 200, -6144, [96, 160]),
        (3, 300, -6144, [96, 160]),
        (4, 400, -8188, [127, 128]),
        (5, 500, -64, [1, 255]),
        (6, 600, 0, [0, 0]),
        (7, 30719, -6016, [94, 162]),
        (8, 30731, -6144, [96, 160]),
    ] {
        tick_to(&mut a, &mut am, tick, false);
        tick_to(&mut b, &mut bm, tick, true);
        same(&a, &b);
        let start = if command == 8 {
            30724
        } else {
            tick as usize - 20
        };
        assert!(
            a.samples[start * 2..]
                .iter()
                .all(|&s| s == pcm as f32 / 32768.0),
            "command {command}"
        );
        assert_eq!([a.dsp.regs[8], a.dsp.regs[9]], regs);
        a.port_write(0, command);
        b.port_write(0, command);
        if command == 1 {
            // At this PC the five-clock MOV $F3,#96 has not executed yet.
            // The interpreter applies its write before advancing those clocks.
            // The first subsequent DSP call applies 1504; the next applies 1536.
            for _ in 0..1000 {
                if a.cpu.pc == first_write && b.cpu.pc == first_write {
                    break;
                }
                a.tick(1);
                b.tick(1);
                am += 1;
                bm += 1;
            }
            assert_eq!(a.cpu.pc, first_write);
            assert_eq!(b.cpu.pc, first_write);
            let before = a.samples.len() / 2;
            tick_to(&mut a, &mut am, before as u64 + 2, false);
            tick_to(&mut b, &mut bm, before as u64 + 2, true);
            assert_eq!(
                &a.samples[before * 2..],
                &[
                    -6016.0 / 32768.0,
                    -6016.0 / 32768.0,
                    -6144.0 / 32768.0,
                    -6144.0 / 32768.0
                ]
            );
            same(&a, &b);
        }
        wait_for(&mut a, &mut am, command);
        wait_for(&mut b, &mut bm, command);
        let addr = 0x1100 + 2 * (command as usize - 1);
        assert_eq!(&a.ram[addr..addr + 2], &regs);
        if command != 7 {
            tick_to(&mut a, &mut am, tick + 10, false);
            tick_to(&mut b, &mut bm, tick + 10, true);
        }
        same(&a, &b);
    }
    assert!(a.cpu.halted && b.cpu.halted);
}
