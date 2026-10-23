//! Original MIT SPC700/noise diagnostics; no firmware or assembler input.
//! Copyright 2026 snes-rust contributors. See tests/roms/PROVENANCE.md.
use snes_core::{apu::Apu, dsp::Dsp, Machine};

fn configure(dsp: &mut Dsp, ram: &mut [u8]) {
    ram[0x2000..0x2004].copy_from_slice(&[0, 0x30, 0, 0x30]);
    ram[0x3000] = 3; // original zero, filter-0, end+loop BRR block
    for (reg, value) in [
        (0x6c, 0x20),
        (0x5d, 0x20),
        (0x0c, 127),
        (0x1c, 127),
        (0, 127),
        (1, 0x80),
        (3, 0x10),
        (7, 127),
        (0x3d, 1),
        (0x4c, 1),
    ] {
        dsp.write(reg, value);
    }
}

#[test]
fn zero_brr_non_reproduction_has_signed_pcm_and_outx() {
    let mut dsp = Dsp::default();
    let mut ram = vec![0; 65536];
    configure(&mut dsp, &mut ram);
    for _ in 0..5 {
        assert_eq!(dsp.sample(&mut ram), (0, 0));
    }
    // Held seed $4000 -> signed -32768; GAIN=2032 -> -32512;
    // voice volumes 127/-128 and master 127, with arithmetic shifts.
    assert_eq!(dsp.sample(&mut ram), (-32006, 32258));
    assert_eq!([dsp.regs[8], dsp.regs[9]], [127, 129]);
}

fn guest() -> Vec<u8> {
    fn dsp(code: &mut Vec<u8>, reg: u8, value: u8) {
        code.extend([0x8f, reg, 0xf2, 0x8f, value, 0xf3]); // MOV dp,#imm
    }
    let mut code = Vec::new();
    for (reg, value) in [
        (0x6c, 0x23),
        (0x5d, 0x20),
        (0, 127),
        (1, 0x80),
        (3, 0x10),
        (7, 127),
        (0x0c, 127),
        (0x1c, 127),
        (0x3d, 1),
        (0x4c, 1),
    ] {
        dsp(&mut code, reg, value);
    }
    code.extend([0x8f, 0x7f, 0xf4]); // ready acknowledgement
    for command in 1..=5u8 {
        // wait: MOV A,$F4; CMP A,#command; BNE wait
        code.extend([0xe4, 0xf4, 0x68, command, 0xd0, 0xfa]);
        if command == 2 {
            dsp(&mut code, 0x6c, 0x22);
        } // live rate 3 -> 2
        let addr = 0x1000u16 + command as u16 - 1;
        code.extend([0x8f, 9, 0xf2, 0xe4, 0xf3, 0xc5]); // select OUTX; read; MOV abs,A
        code.extend(addr.to_le_bytes());
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

#[test]
fn uploaded_spc_records_numerical_outx_and_changes_noise_rate_live() {
    let mut apu = Apu::default();
    // Original zero BRR data, outside guest, result and echo addresses.
    apu.ram[0x2000..0x2004].copy_from_slice(&[0, 0x30, 0, 0x30]);
    apu.ram[0x3000] = 3;
    let code = guest();
    assert!(code.len() < 256);
    upload(&mut apu, &code);
    assert!(apu.samples.len() / 2 < 100);
    // Absolute sample ticks, including upload. Slow held values keep reads
    // clear of the sample/subcycle edges; no ROM timing assumption is needed.
    for (command, tick, outx) in [
        (1, 540, 63),
        (2, 550, 63),
        (3, 1050, 31),
        (4, 2100, 31),
        (5, 2585, 15),
    ] {
        for _ in 0..100_000 {
            if apu.samples.len() / 2 >= tick {
                break;
            }
            apu.tick(21);
        }
        assert_eq!(apu.samples.len() / 2, tick, "checkpoint execution bound");
        apu.port_write(0, command);
        wait_for(&mut apu, command);
        assert_eq!(apu.ram[0x1000 + command as usize - 1], outx, "tick {tick}");
        assert_eq!(apu.dsp.regs[9], outx);
        assert_eq!(apu.dsp.regs[8], 127);
    }
    for _ in 0..10 {
        apu.tick(21);
    }
    assert!(apu.cpu.halted);
    assert!(apu.error.is_none());
    assert!(apu.samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
}

#[test]
fn apu_noise_pcm_and_registers_match_whole_and_irregular_master_ticks() {
    fn configured() -> Apu {
        let mut apu = Apu::default();
        configure(&mut apu.dsp, &mut apu.ram);
        apu.dsp.write(0x6c, 0x23);
        apu
    }
    let mut whole = configured();
    let mut split = configured();
    let mut master = 0;
    // Independently derived literal (tick, OUTX, PCM) checkpoints. Changes are
    // applied identically after reaching a master-clock boundary in both runs.
    for (tick, outx, pcm) in [
        (535, 129, (-32006, 32258)),
        (536, 63, (16002, -16129)),
        (537, 63, (16002, -16129)),
        (1039, 63, (16002, -16129)),
        (1040, 31, (8001, -8065)),
        (1041, 31, (8001, -8065)),
        (2575, 31, (0, 0)),
        (2576, 15, (0, 0)),
        (2577, 15, (0, 0)),
        (2578, 15, (4000, -4033)),
        (30719, 213, (-10826, 10910)),
        (30720, 105, (26590, -26801)),
        (30721, 181, (-18713, 18859)),
        (61439, 121, (30506, -30748)),
        (61440, 189, (-16755, 16885)),
        (61441, 222, (-8379, 8443)),
    ] {
        let target = (tick * 32u64 * 21_477_272).div_ceil(1_024_000);
        let delta = target - master;
        whole.tick(delta);
        let mut remaining = delta;
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
        assert_eq!(whole.dsp.regs, split.dsp.regs, "registers tick {tick}");
        assert_eq!(whole.samples, split.samples, "every PCM sample tick {tick}");
        assert_eq!(
            [whole.dsp.regs[8], whole.dsp.regs[9]],
            [127, outx],
            "tick {tick}"
        );
        assert_eq!(
            &whole.samples[whole.samples.len() - 2..],
            &[pcm.0 as f32 / 32768.0, pcm.1 as f32 / 32768.0],
            "PCM tick {tick}"
        );
        let flag = match tick {
            537 => Some(0x22),
            1041 => Some(0x62),
            2577 => Some(0x22),
            2578 => Some(0x3f),
            _ => None,
        };
        if let Some(flag) = flag {
            whole.dsp.write(0x6c, flag);
            split.dsp.write(0x6c, flag);
        }
    }
    assert!(whole
        .samples
        .iter()
        .all(|s| s.is_finite() && s.abs() <= 1.0));
    assert!(whole.error.is_none() && split.error.is_none());
}

#[test]
fn fresh_machine_and_machine_reset_reproduce_the_seed_and_sequence() {
    let rom = include_bytes!("../../../tests/roms/smoke.bin");
    let mut machine = Machine::new(rom).unwrap();
    fn capture(apu: &mut Apu) -> Vec<(i16, i16)> {
        configure(&mut apu.dsp, &mut apu.ram);
        let mut result = Vec::new();
        for _ in 0..6 {
            result.push(apu.dsp.sample(&mut apu.ram));
        }
        assert_eq!(
            result,
            [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0), (-32006, 32258)]
        );
        apu.dsp.write(0x6c, 0x3f);
        for _ in 0..30 {
            result.push(apu.dsp.sample(&mut apu.ram));
        }
        // First advance $4000->$2000 and feedback/sign changes after 14/15.
        assert_eq!(result[6], (16002, -16129));
        assert_eq!(result[19], (-32006, 32258));
        assert_eq!(result[20], (-16003, 16129));
        result
    }
    let first = capture(&mut machine.bus.apu);
    machine.reset();
    let reset = capture(&mut machine.bus.apu);
    let fresh = capture(&mut Apu::default());
    assert_eq!(first, reset);
    assert_eq!(first, fresh);
}
