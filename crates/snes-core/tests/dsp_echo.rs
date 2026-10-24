//! Original MIT SPC700 guest and echo data; no assembler, firmware or ROM.
//! Copyright 2026 snes-rust contributors. See tests/roms/PROVENANCE.md.
use snes_core::apu::Apu;

fn guest() -> Vec<u8> {
    fn dsp(code: &mut Vec<u8>, reg: u8, value: u8) {
        code.extend([0x8f, reg, 0xf2, 0x8f, value, 0xf3]);
    }
    let mut code = Vec::new();
    for (r, v) in [
        (0x6c, 0x20),
        (0x6d, 0x60),
        (0x7d, 1),
        (0x7f, 64),
        (0x0d, 64),
        (0x2c, 64),
        (0x3c, 64),
    ] {
        dsp(&mut code, r, v);
    }
    code.extend([0x8f, 0x7f, 0xf4]); // ready
    for command in 1..=4u8 {
        code.extend([0xe4, 0xf4, 0x68, command, 0xd0, 0xfa]);
        if command == 1 {
            dsp(&mut code, 0x6c, 0); // start RAM feedback
        } else {
            for offset in 0..4u16 {
                // MOV A,!$6000+offset; MOV !$1000+result_offset,A
                code.push(0xe5);
                code.extend((0x6000 + offset).to_le_bytes());
                code.push(0xc5);
                code.extend((0x1000 + (command as u16 - 2) * 4 + offset).to_le_bytes());
            }
        }
        code.extend([0x8f, command, 0xf4]);
    }
    code.push(0xff); // STOP
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
fn upload(apu: &mut Apu, code: &[u8]) {
    apu.port_write(2, 0);
    apu.port_write(3, 2);
    apu.port_write(1, 1);
    apu.port_write(0, 0xcc);
    wait_for(apu, 0xcc);
    for (i, &byte) in code.iter().enumerate() {
        apu.port_write(1, byte);
        apu.port_write(0, i as u8);
        wait_for(apu, i as u8);
    }
    apu.port_write(2, 0);
    apu.port_write(3, 2);
    apu.port_write(1, 0);
    apu.port_write(0, (code.len() as u8).wrapping_add(1));
    wait_for(apu, 0x7f);
    assert_eq!(&apu.ram[0x200..0x200 + code.len()], code);
}
fn advance_to_sample(apu: &mut Apu, sample: usize) {
    for _ in 0..1_200_000 {
        if apu.samples.len() / 2 >= sample {
            break;
        }
        apu.tick(21);
    }
    assert_eq!(apu.samples.len() / 2, sample, "bounded sample advance");
    assert!(apu.error.is_none(), "{:?}", apu.error);
}
fn echo_data(apu: &mut Apu) {
    // Original repeated stereo words. No history or expected results injected.
    for frame in apu.ram[0x6000..0x6800].as_chunks_mut::<4>().0 {
        frame.copy_from_slice(&[0x02, 0x40, 0xfe, 0xdf]); // 16386,-8194
    }
}

#[test]
fn uploaded_spc_configures_fir_feedback_and_records_echo_ram() {
    let mut apu = Apu::default();
    echo_data(&mut apu);
    let code = guest();
    assert!(code.len() < 256);
    upload(&mut apu, &code);
    assert!(apu.samples.len() / 2 < 100);
    assert_eq!((apu.dsp.regs[0x7f], apu.dsp.regs[0x0d]), (64, 64));
    let ready = apu.samples.len() / 2;
    advance_to_sample(&mut apu, ready + 10);
    assert_eq!(
        &apu.samples[apu.samples.len() - 2..],
        &[4096.0 / 32768.0, -2049.0 / 32768.0]
    );
    apu.port_write(0, 1);
    // Observe the first public RAM write; this anchors the recurrence without
    // assuming an SPC subcycle offset or injecting the DSP's private offset.
    let original = apu.ram[0x6000..0x6800].to_vec();
    for _ in 0..1000 {
        apu.tick(21);
        if apu.ram[0x6000..0x6800] != original {
            break;
        }
    }
    assert_ne!(apu.ram[0x6000..0x6800], original);
    let first = apu.samples.len() / 2 - 1;
    wait_for(&mut apu, 1);
    for (command, elapsed, bytes) in [
        (2, 600, [0x00, 0x10, 0xfe, 0xf7]),  // 4096,-2050
        (3, 1120, [0x00, 0x04, 0xfe, 0xfd]), // 1024,-514
        (4, 1640, [0x00, 0x01, 0x7e, 0xff]), // 256,-130
    ] {
        advance_to_sample(&mut apu, first + elapsed);
        apu.port_write(0, command);
        wait_for(&mut apu, command);
        let addr = 0x1000 + (command as usize - 2) * 4;
        assert_eq!(&apu.ram[addr..addr + 4], &bytes);
    }
    for _ in 0..20 {
        apu.tick(21);
    }
    assert!(apu.cpu.halted);
    assert!(apu.error.is_none());
    assert!(apu.samples.len() / 2 < first + 1650);
    for (n, pair) in apu
        .samples
        .as_chunks::<2>()
        .0
        .iter()
        .enumerate()
        .skip(first)
    {
        let pcm = match (n - first) / 512 {
            0 => (4096, -2049),
            1 => (1024, -513),
            2 => (256, -129),
            3 => (64, -33),
            _ => panic!("execution bound"),
        };
        assert_eq!(
            pair,
            &[pcm.0 as f32 / 32768.0, pcm.1 as f32 / 32768.0],
            "sample {n}"
        );
    }
}

#[test]
fn every_pcm_register_and_echo_byte_matches_irregular_apu_partitions() {
    for delay in [0, 1] {
        fn configured(delay: u8) -> Apu {
            let mut apu = Apu::default();
            echo_data(&mut apu);
            for (r, v) in [
                (0x6c, 0),
                (0x6d, 0x60),
                (0x7d, delay),
                (0x7f, 64),
                (0x0d, 64),
                (0x2c, 64),
                (0x3c, 64),
            ] {
                apu.dsp.write(r, v);
            }
            apu
        }
        let mut whole = configured(delay);
        let mut split = configured(delay);
        let mut master = 0;
        for tick in [1, 8, 9, 511, 512, 513, 1024, 1025, 1537] {
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
            assert_eq!(whole.samples, split.samples);
            assert_eq!(whole.dsp.regs, split.dsp.regs);
            assert_eq!(&whole.ram[0x6000..0x6800], &split.ram[0x6000..0x6800]);
            assert!(whole.error.is_none() && split.error.is_none());
        }
        // Literal recurrence checkpoints independently constrain the equal runs.
        let points = if delay == 0 {
            [
                (0, (4096, -2049)),
                (1, (1024, -513)),
                (2, (256, -129)),
                (3, (64, -33)),
            ]
        } else {
            [
                (0, (4096, -2049)),
                (512, (1024, -513)),
                (1024, (256, -129)),
                (1536, (64, -33)),
            ]
        };
        for (index, (l, r)) in points {
            assert_eq!(
                &whole.samples[index * 2..index * 2 + 2],
                &[l as f32 / 32768.0, r as f32 / 32768.0]
            );
        }
    }
}
