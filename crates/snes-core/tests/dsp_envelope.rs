//! Original MIT SPC700 guest, built in memory without firmware or an assembler.
//! Copyright 2026 snes-rust contributors. See tests/roms/PROVENANCE.md.
use snes_core::apu::Apu;

fn guest() -> Vec<u8> {
    // MOV dp,#imm; all DSP access goes through the SPC700 $F2/$F3 ports.
    fn dsp(code: &mut Vec<u8>, reg: u8, value: u8) {
        code.extend([0x8f, reg, 0xf2, 0x8f, value, 0xf3]);
    }
    let mut code = Vec::new();
    for (reg, value) in [
        (0x6c, 0x20),
        (5, 0),
        (7, 0xc3),
        (0x15, 0),
        (0x17, 0xc2),
        (0x4c, 3),
    ] {
        dsp(&mut code, reg, value);
    }
    code.extend([0x8f, 0x7f, 0xf4]); // ready acknowledgement
    for command in 1..=4u8 {
        // wait: MOV A,$F4; CMP A,#command; BNE wait
        code.extend([0xe4, 0xf4, 0x68, command, 0xd0, 0xfa]);
        if command == 2 {
            dsp(&mut code, 7, 0xc2); // mid-run rate 3 -> 2, without KON
        }
        for (voice, reg) in [8, 0x18].into_iter().enumerate() {
            let addr = 0x1000u16 + (command as u16 - 1) * 2 + voice as u16;
            // MOV $F2,#ENVX; MOV A,$F3; MOV abs,A
            code.extend([0x8f, reg, 0xf2, 0xe4, 0xf3, 0xc5]);
            code.extend(addr.to_le_bytes());
        }
        code.extend([0x8f, command, 0xf4]); // checkpoint complete
    }
    code.push(0xff); // STOP; bounded host execution still advances the DSP
    code
}

fn wait_for(apu: &mut Apu, value: u8) {
    for _ in 0..1000 {
        if apu.port_read(0) == value {
            return;
        }
        apu.tick(21); // about one SPC clock, at most one instruction at a time
        assert!(apu.error.is_none(), "{:?}", apu.error);
    }
    panic!(
        "SPC acknowledgement {value:02x} timed out at {:04x}",
        apu.cpu.pc
    );
}

fn upload(apu: &mut Apu, code: &[u8]) {
    // Exercise the existing firmware-free IPL port protocol, including launch.
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
fn uploaded_spc_records_offset_rate_checkpoints_and_a_live_rate_change() {
    let mut apu = Apu::default();
    let code = guest();
    assert!(code.len() < 256);
    upload(&mut apu, &code);
    assert!(apu.samples.len() / 2 < 100); // setup is far before either first event
                                          // Absolute DSP ticks since machine initialization, including the upload.
                                          // Snapshots are deliberately clear of sample/subcycle register boundaries.
    for (command, tick, expected) in [
        (1, 540, [2, 0]),
        (2, 1050, [2, 2]),
        (3, 2100, [2, 2]), // reject old rate 3 (1816) and restarted rate 2 (2090)
        (4, 2585, [4, 4]), // both share rate 2's tick 2576
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
        let start = 0x1000 + (command as usize - 1) * 2;
        assert_eq!(&apu.ram[start..start + 2], &expected, "tick {tick}");
        assert_eq!([apu.dsp.regs[8], apu.dsp.regs[24]], expected);
    }
    for _ in 0..10 {
        apu.tick(21);
    }
    assert!(apu.cpu.halted);
    assert!(apu.error.is_none());
}
