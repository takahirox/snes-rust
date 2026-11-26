use snes_core::{cartridge::Mapping, frame_hash, Machine, MAX_SNAPSHOT_BYTES};

#[path = "common/hires.rs"]
mod hires;
#[path = "common/mode7_transform.rs"]
mod mode7;

const SMOKE: &[u8] = include_bytes!("../../../tests/roms/smoke.bin");

fn field(machine: &mut Machine, buttons: u16) -> Vec<f32> {
    machine.set_buttons(buttons);
    machine.set_buttons2(buttons.rotate_left(4));
    machine.run_frame().unwrap();
    machine.take_audio()
}

fn compare(machine: &mut Machine, inputs: &[u16]) {
    let saved = machine.save_snapshot().unwrap();
    let mut restored = machine.prepare_snapshot(&saved).unwrap();
    assert_eq!(restored.framebuffer(), machine.framebuffer());
    assert_eq!(restored.bus.cart.sram, machine.bus.cart.sram);
    assert_eq!(restored.save_snapshot().unwrap(), saved);
    for &mask in inputs {
        assert_eq!(field(machine, mask), field(&mut restored, mask));
        assert_eq!(
            frame_hash(machine.framebuffer()),
            frame_hash(restored.framebuffer())
        );
        assert_eq!(restored.bus.cart.sram, machine.bus.cart.sram);
        // Compare every stored CPU, bus, PPU, APU, and DSP field, not just the
        // next output or a few externally readable registers.
        assert_eq!(
            restored.save_snapshot().unwrap(),
            machine.save_snapshot().unwrap()
        );
    }
}

#[test]
fn diagnostic_round_trip_with_dma_controller_audio_and_battery_sram() {
    let mut machine = Machine::new(SMOKE).unwrap();
    machine.bus.cart.sram[3] = 0x79;
    for frame in 0..45 {
        let samples = field(&mut machine, if frame % 4 == 0 { 0x0100 } else { 0 });
        if frame == 42 {
            assert!(samples.iter().any(|&sample| sample != 0.0));
        }
    }
    assert!(machine.bus.cart.battery);
    compare(&mut machine, &[0x0080, 0x8000, 0x0100, 0x0200, 0, 0x1000]);
}

#[test]
fn hires_and_mode7_round_trip_without_a_battery() {
    let mut machine = Machine::new(&hires::guest_rom(5, true)).unwrap();
    assert!(!machine.bus.cart.battery);
    for _ in 0..10 {
        field(&mut machine, 0x0080);
    }
    assert_eq!(machine.framebuffer_width(), 512);
    compare(&mut machine, &[0x0080, 0, 0, 0x0080, 0x0080]);

    // Put the Mode 7 guest and vectors in the upper half of a normal HiROM.
    let mut hirom = vec![0; 32768];
    hirom.extend(mode7::guest_rom(false, false));
    hirom[0xffd5] = 0x21;
    let mut machine = Machine::new(&hirom).unwrap();
    assert_eq!(machine.bus.cart.mapping, Mapping::HiRom);
    for _ in 0..8 {
        field(&mut machine, 0);
    }
    assert_eq!(machine.bus.peek(0x0030), 0x63);
    compare(&mut machine, &[0, 0, 0]);
}

#[test]
fn both_permitted_demos_continue_from_interactive_scenes() {
    // Use the same launch inputs as the production performance workloads.
    for (rom, buttons) in [
        (
            &include_bytes!("../../../demos/shionari.sfc")[..],
            &[(70..76, 0x80), (184..190, 0x80), (212..218, 0x80)],
        ),
        (
            &include_bytes!("../../../demos/yonaki.sfc")[..],
            &[(146..152, 0x80), (182..188, 0x1000), (215..221, 0x1000)],
        ),
    ] {
        let mut machine = Machine::new(rom).unwrap();
        let mut audible = false;
        for frame in 0..245 {
            let mask = buttons
                .iter()
                .find(|(range, _)| range.contains(&frame))
                .map_or(0, |(_, mask)| *mask);
            let audio = field(&mut machine, mask);
            audible |= audio.iter().any(|&value| value != 0.0);
        }
        assert!(audible);
        compare(&mut machine, &[0x0080, 0x0080, 0, 0x1000]);
    }
}

#[test]
fn incompatible_files_fail_atomically_and_copier_headers_keep_identity() {
    let mut machine = Machine::new(SMOKE).unwrap();
    field(&mut machine, 0);
    let saved = machine.save_snapshot().unwrap();
    let mut version = saved.clone();
    version[8] = 2;
    let mut tampered = saved.clone();
    *tampered.last_mut().unwrap() ^= 1;
    let mut revision = SMOKE.to_vec();
    revision[0x1234] ^= 1;
    let wrong_rom = Machine::new(&revision).unwrap().save_snapshot().unwrap();

    for (file, message) in [
        (&[][..], "corrupt"),
        (&saved[..saved.len() - 1], "corrupt"),
        (&version, "version 2"),
        (&tampered, "corrupt"),
        (&wrong_rom, "different ROM"),
        (&vec![0; MAX_SNAPSHOT_BYTES + 1], "exceeds"),
    ] {
        assert!(machine.load_snapshot(file).unwrap_err().contains(message));
        assert_eq!(machine.save_snapshot().unwrap(), saved);
    }

    let mut headered = vec![0x81; 512];
    headered.extend_from_slice(SMOKE);
    let mut copy = Machine::new(&headered).unwrap();
    copy.load_snapshot(&saved).unwrap();
    assert_eq!(copy.save_snapshot().unwrap(), saved);
    copy.step();
    assert!(copy.save_snapshot().is_err());
    copy.reset();
    assert!(copy.save_snapshot().is_ok());
}

#[test]
fn hdma_and_live_spc_timers_continue_after_restore() {
    use snes_core::cpu::Bus;

    let mut machine = Machine::new(SMOKE).unwrap();
    for _ in 0..45 {
        field(&mut machine, 0);
    }
    // A repeat-brightness table in WRAM, distinct from the guest's CGRAM
    // and controller work. Leave the channel configured across the snapshot.
    machine.bus.wram[0x1000..0x1004].copy_from_slice(&[0x82, 15, 7, 0]);
    for (reg, value) in [
        (0x4310, 0),
        (0x4311, 0),
        (0x4312, 0),
        (0x4313, 0x10),
        (0x4314, 0x7e),
        (0x420c, 2),
    ] {
        machine.bus.write(reg, value);
    }
    machine.bus.apu.write(0xfa, 7);
    machine.bus.apu.write(0xfc, 13);
    machine.bus.apu.write(0xf1, 5);
    for _ in 0..2 {
        field(&mut machine, 0);
    }
    assert_eq!(machine.bus.ppu.regs[0] & 15, 7);
    assert_ne!(machine.bus.apu.read(0xfd), 0);
    assert_ne!(machine.bus.apu.read(0xff), 0);
    compare(&mut machine, &[0x0100, 0, 0x0800]);
}
