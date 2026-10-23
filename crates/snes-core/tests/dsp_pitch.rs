//! Original MIT PMON diagnostics and SPC700 guest; no firmware or external code.
//! Copyright 2026 snes-rust contributors. See tests/roms/PROVENANCE.md.
use snes_core::{apu::Apu, dsp::Dsp};

fn data(ram: &mut [u8]) {
    ram[0x2000..0x2008].copy_from_slice(&[0, 0x30, 0, 0x30, 0x10, 0x30, 0x10, 0x30]);
    // Voice 0: constant decoded -16384, Gaussian three-tap wrap gives +32752
    // at phase zero; GAIN 64 yields +16376. Voice 1: original ascending ramp.
    ram[0x3000] = 0xc3;
    ram[0x3001..0x3009].fill(0x88);
    ram[0x3010] = 0x93;
    ram[0x3011..0x3019].copy_from_slice(&[0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);
}

fn configure(dsp: &mut Dsp, ram: &mut [u8], pmon: u8) {
    data(ram);
    for (r, v) in [
        (0x6c, 0x20),
        (0x5d, 0x20),
        (0x0c, 64),
        (0x1c, 64),
        (3, 0x10),
        (7, 64),
        (0x14, 1),
        (0x13, 0x10),
        (0x17, 64),
        (0x10, 64),
        (0x11, 0x80),
        (0x2d, pmon),
        (0x4c, 3),
    ] {
        dsp.write(r, v);
    }
}

#[test]
fn original_brr_pair_pmon_reproduction() {
    let mut off = Dsp::default();
    let mut on = Dsp::default();
    let mut a = vec![0; 65536];
    let mut b = a.clone();
    configure(&mut off, &mut a, 0);
    configure(&mut on, &mut b, 2);
    off.write(0x13, 0);
    on.write(0x13, 0);
    for _ in 0..9 {
        off.sample(&mut a);
        on.sample(&mut b);
    }
    off.write(0x13, 0x10);
    on.write(0x13, 0x10);
    for _ in 0..3 {
        off.sample(&mut a);
        on.sample(&mut b);
    }
    // Settled predecessor +16376 => step $17FC. Four steps give $5FF0
    // (five decoded nibbles); at index 255 taps are 0,374,1305,370.
    // Decoded window [256,512,768,1024] -> source 1534 -> envelope 766.
    assert_eq!(off.sample(&mut a), (64, -128));
    assert_eq!(on.sample(&mut b), (191, -383));
    assert_eq!([off.regs[25], on.regs[25]], [1, 2]);
}

fn noise_pair(dsp: &mut Dsp, ram: &mut [u8], pmon: u8) {
    configure(dsp, ram, pmon);
    ram[0x3000] = 3; // silent loop under the noise predecessor
    ram[0x3001..0x3009].fill(0);
    dsp.write(0x3d, 1);
    dsp.write(7, 127); // noise -32768 * 2032/2048 = -32512 -> increment $20
    dsp.write(3, 0);
}

fn guest(pmon: u8) -> Vec<u8> {
    fn write(code: &mut Vec<u8>, reg: u8, value: u8) {
        code.extend([0x8f, reg, 0xf2, 0x8f, value, 0xf3]);
    }
    let mut code = Vec::new();
    for (r, v) in [
        (0x6c, 0x20),
        (0x5d, 0x20),
        (7, 127),
        (0x3d, 1),
        (0x14, 1),
        (0x13, 0x10),
        (0x17, 64),
        (0x10, 64),
        (0x11, 0x80),
        (0x0c, 64),
        (0x1c, 64),
        (0x2d, pmon),
        (0x4c, 3),
    ] {
        write(&mut code, r, v);
    }
    code.extend([0x8f, 0x7f, 0xf4]);
    for command in 1..=3u8 {
        code.extend([0xe4, 0xf4, 0x68, command, 0xd0, 0xfa]); // bounded host wait
        if command == 2 {
            write(&mut code, 0x2d, 1);
        } // disable without KON
        for (offset, reg) in [0x19, 0x7c].into_iter().enumerate() {
            code.extend([0x8f, reg, 0xf2, 0xe4, 0xf3, 0xc5]);
            code.extend((0x1000u16 + 2 * (command as u16 - 1) + offset as u16).to_le_bytes());
        }
        code.extend([0x8f, command, 0xf4]);
    }
    code.push(0xff);
    code
}

fn wait_for(apu: &mut Apu, value: u8) {
    for _ in 0..1000 {
        assert!(apu.error.is_none(), "{:?}", apu.error);
        if apu.port_read(0) == value {
            return;
        }
        apu.tick(21);
    }
    panic!("SPC acknowledgement {value:02x} timed out");
}

fn upload(apu: &mut Apu, code: &[u8]) {
    assert!(code.len() < 256);
    apu.port_write(2, 0);
    apu.port_write(3, 2);
    apu.port_write(1, 1);
    apu.port_write(0, 0xcc);
    wait_for(apu, 0xcc);
    for (index, &byte) in code.iter().enumerate() {
        apu.port_write(1, byte);
        apu.port_write(0, index as u8);
        wait_for(apu, index as u8);
    }
    apu.port_write(2, 0);
    apu.port_write(3, 2);
    apu.port_write(1, 0);
    apu.port_write(0, (code.len() as u8).wrapping_add(1));
    wait_for(apu, 0x7f);
    assert_eq!(&apu.ram[0x200..0x200 + code.len()], code);
}

fn until_sample(apu: &mut Apu, target: usize) {
    for _ in 0..100_000 {
        assert!(apu.error.is_none(), "{:?}", apu.error);
        if apu.samples.len() / 2 >= target {
            break;
        }
        apu.tick(21);
    }
    assert_eq!(apu.samples.len() / 2, target, "checkpoint execution bound");
}

#[test]
fn uploaded_spc_changes_pmon_without_kon_and_records_target_endx() {
    for pmon in [0, 2] {
        let mut apu = Apu::default();
        data(&mut apu.ram);
        apu.ram[0x3000] = 3; // original zero loop for noise predecessor
        apu.ram[0x3001..0x3009].fill(0);
        apu.ram[0x3010] = 0x91; // non-looping target
        upload(&mut apu, &guest(pmon));
        assert!(apu.samples.len() / 2 < 50);
        // Before tick100, enabled pitch advances by at most 100*$20 < $1000:
        // no nibble consumed, ENDX clear. Disabled reaches ENDX in 17 samples.
        for (command, tick, end) in [
            (1, 100, if pmon == 2 { 0 } else { 2 }),
            (2, 110, if pmon == 2 { 0 } else { 2 }),
            (3, 140, 2),
        ] {
            until_sample(&mut apu, tick);
            apu.port_write(0, command);
            wait_for(&mut apu, command);
            assert_eq!(
                &apu.ram[0x1000 + 2 * (command as usize - 1)..0x1002 + 2 * (command as usize - 1)],
                &[0, end]
            );
            assert_eq!(apu.dsp.regs[0x7c], end);
            assert_eq!(apu.dsp.regs[0x4c], 3); // no retrigger write
            assert_eq!(apu.dsp.regs[0x2d], if command >= 2 { 1 } else { pmon });
        }
        for _ in 0..10 {
            apu.tick(21);
        }
        assert!(apu.cpu.halted);
        assert!(apu.error.is_none());
        assert!(apu.samples.len() / 2 < 150);
        assert!(apu.samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
    }
}

#[test]
fn apu_pcm_and_all_registers_match_irregular_partitions_with_live_pmon() {
    fn configured() -> Apu {
        let mut apu = Apu::default();
        noise_pair(&mut apu.dsp, &mut apu.ram, 2);
        apu
    }
    let mut whole = configured();
    let mut split = configured();
    let mut master = 0;
    // At step $20, every 128 output samples advance one nibble. Five key-delay
    // samples precede advancement. Literal phase-zero Gaussian/volume vectors.
    for (tick, outx, pcm, end) in [
        (6, 0, (0, 0), 0),
        (133, 0, (0, 0), 0),
        (261, 0, (0, 0), 0),
        (389, 0, (11, -23), 0),
        (517, 1, (64, -128), 0),
        (518, 2, (128, -256), 0),
        (519, 3, (192, -384), 0),
        (647, 4, (256, -512), 0),
        // Resume independent pitch after tick 647: 9 more nibbles to 16,
        // then the loop raises ENDX at 657, with history [-768,-512,-256,0].
        (656, 253, (-192, 384), 0),
        (657, 253, (-129, 257), 2),
    ] {
        let target = (tick * 32u64 * 21_477_272).div_ceil(1_024_000);
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
        assert_eq!(whole.samples.len(), tick as usize * 2);
        assert_eq!(whole.samples, split.samples, "every PCM sample tick {tick}");
        assert_eq!(whole.dsp.regs, split.dsp.regs, "all registers tick {tick}");
        assert_eq!(
            [whole.dsp.regs[25], whole.dsp.regs[0x7c]],
            [outx, end],
            "tick {tick}"
        );
        assert_eq!(
            &whole.samples[whole.samples.len() - 2..],
            &[pcm.0 as f32 / 32768.0, pcm.1 as f32 / 32768.0],
            "PCM tick {tick}"
        );
        if let Some(pmon) = match tick {
            517 | 647 => Some(1),
            519 => Some(2),
            _ => None,
        } {
            whole.dsp.write(0x2d, pmon);
            split.dsp.write(0x2d, pmon);
        }
    }
    assert!(whole.error.is_none() && split.error.is_none());
    assert!(whole
        .samples
        .iter()
        .all(|s| s.is_finite() && s.abs() <= 1.0));
}
