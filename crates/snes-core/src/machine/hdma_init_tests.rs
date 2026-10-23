//! Original MIT scheduler vectors for Issue #31. Events arise only from clocks.
//! Copyright (c) 2026 snes-rust contributors.
use super::*;

fn bus() -> SystemBus {
    SystemBus::new(Cartridge::load(include_bytes!("../../../../tests/roms/smoke.bin")).unwrap())
}
fn channel(b: &mut SystemBus, i: usize, mode: u8, table: u32, port: u8) {
    b.dma[i] = [0; 16];
    b.dma[i][0] = mode;
    b.dma[i][1] = port;
    b.dma[i][2..4].copy_from_slice(&(table as u16).to_le_bytes());
    b.dma[i][4] = (table >> 16) as u8;
    b.dma[i][7] = 0x7f;
}
fn until(b: &mut SystemBus, frame: u64, line: u16, h: u64) {
    let deadline = b.clocks + LINE * 263;
    while (b.frames, b.v, b.h) < (frame, line, h) {
        b.idle();
        assert!(b.clocks < deadline);
    }
    b.events();
}
fn configured(indirect: bool) -> SystemBus {
    let mut b = bus();
    channel(&mut b, 0, if indirect { 0x40 } else { 0 }, 0x7e0100, 0x40);
    b.wram[0x100..0x104].copy_from_slice(if indirect {
        &[0x82, 0, 2, 0]
    } else {
        &[0x82, 0x11, 0x22, 0]
    });
    b.wram[0x10200..0x10202].copy_from_slice(&[0x11, 0x22]);
    b.write_raw(0x420c, 1);
    b
}
#[test]
fn initial_frame_and_wrap_prepare_registers_before_payload() {
    for indirect in [false, true] {
        let mut b = configured(indirect);
        for frame in 0..=1 {
            until(&mut b, frame, 0, 200);
            assert_eq!(b.dma[0][10], 0x82);
            assert_eq!(&b.dma[0][8..10], &[if indirect { 3 } else { 1 }, 1]);
            assert_eq!(&b.dma[0][2..5], &[0, 1, 0x7e]);
            assert_eq!(b.dma[0][7], 0x7f);
            if indirect {
                assert_eq!(&b.dma[0][5..7], &[0, 2]);
            }
            assert_eq!(b.apu.input[0], 0);
            // Turn it off before the first HBlank, then enable during VBlank.
            b.write_raw(0x420c, 0);
            until(&mut b, frame, 225, 100);
            b.write_raw(0x420c, 1);
        }
    }
}
#[test]
fn metadata_edits_after_init_do_not_replace_first_row_but_payload_is_live() {
    for indirect in [false, true] {
        let mut b = configured(indirect);
        until(&mut b, 0, 0, 200);
        b.wram[0x100] = 0; // Replacement header must not terminate the prepared row.
        if indirect {
            b.wram[0x101..0x103].copy_from_slice(&[0, 3]);
            b.wram[0x10300] = 0x55;
            b.wram[0x10200] = 0x66;
        } else {
            b.wram[0x101] = 0x66;
        }
        until(&mut b, 0, 0, 1150);
        assert_eq!(b.apu.input[0], 0x66);
        assert_eq!(b.dma[0][10], 0x81);
        until(&mut b, 0, 1, 1150);
        assert_eq!(b.apu.input[0], 0x22);
        assert_eq!(b.dma[0][10], 0); // Issue #33: row-end termination is already visible.
    }
}
#[test]
fn zero_indirect_headers_fetch_pointer_even_when_terminating() {
    for later in [false, true] {
        let mut b = bus();
        channel(&mut b, 0, 0xc0, 0x7effff, 0x80);
        b.wram[0xffff] = 0;
        b.wram[..2].copy_from_slice(&[0x34, 0x12]);
        channel(&mut b, 2, 0, 0x7e0100, 0x40);
        b.wram[0x100] = 0; // Still enabled when channel 0 is initialized.
        b.write_raw(0x420c, if later { 5 } else { 1 });
        until(&mut b, 0, 0, 200);
        assert_eq!(b.dma[0][10], 0);
        assert_eq!(&b.dma[0][8..10], &[if later { 2 } else { 1 }, 0]);
        assert_eq!(
            &b.dma[0][5..7],
            if later { &[0x34, 0x12] } else { &[0, 0x34] }
        );
        assert_eq!(b.dma[0][4], 0x7e);
        assert_eq!(b.hdma_active, 0);
        assert_eq!(b.io[12], if later { 5 } else { 1 });
        assert_eq!(b.waddr, 0);
        until(&mut b, 0, 1, 1150);
        assert_eq!(b.waddr, 0);
    }
}
#[test]
fn init_channel_order_disabled_controls_and_a_bus_effects() {
    let mut b = bus();
    // Both read the IRQ status; channel 0 must acknowledge it before channel 2.
    for i in [0, 2] {
        channel(&mut b, i, 0x80, 0x004211, 0x80);
    }
    channel(&mut b, 1, 0, 0x004210, 0x80);
    b.irq = true;
    b.nmi_flag = true;
    b.write_raw(0x420c, 5);
    until(&mut b, 0, 0, 200);
    assert_eq!(b.dma[0][10], 0x80);
    assert_eq!(b.dma[2][10], 0);
    assert!(!b.irq && b.nmi_flag);
    assert_eq!(&b.dma[1][8..11], &[0, 0, 0]);
    assert_eq!(b.waddr, 0); // No reverse WMDATA payload reads.
    for table in [0x002180, 0x004016, 0x004300] {
        let mut b = bus();
        channel(&mut b, 0, 0x40, table, 0x80);
        b.waddr = 0x12345;
        b.serial = 0x9180;
        b.write_raw(0x420c, 1);
        until(&mut b, 0, 0, 200);
        assert_eq!(b.dma[0][10], 0);
        assert_eq!(b.hdma_active, 0);
        assert_eq!((b.waddr, b.serial), (0x12345, 0x9180));
    }
}
#[test]
fn nonzero_indirect_pointer_wrap_and_status_reads_ignore_direction() {
    for reverse in [false, true] {
        let mut b = bus();
        channel(&mut b, 0, if reverse { 0xc0 } else { 0x40 }, 0x7efffe, 0x80);
        b.wram[0xfffe..0x10000].copy_from_slice(&[0x82, 0x34]);
        b.wram[0] = 0x12;
        b.write_raw(0x420c, 1);
        until(&mut b, 0, 0, 200);
        assert_eq!(&b.dma[0][5..7], &[0x34, 0x12]);
        assert_eq!(&b.dma[0][8..10], &[1, 0]);
        assert_eq!(b.waddr, 0);
        let mut b = bus();
        // Header from $4210, pointer low from acknowledging $4211.
        channel(&mut b, 0, if reverse { 0xc0 } else { 0x40 }, 0x004210, 0x80);
        b.nmi_flag = true;
        b.irq = true;
        b.write_raw(0x420c, 1);
        until(&mut b, 0, 0, 200);
        assert_eq!(b.dma[0][10], 0x82);
        assert_eq!(&b.dma[0][5..7], &[0x80, 0]);
        assert!(!b.nmi_flag && !b.irq);
        assert_eq!(b.waddr, 0);
    }
}
#[test]
fn enabling_after_init_does_not_manufacture_an_event() {
    let mut b = configured(false);
    b.write_raw(0x420c, 0);
    until(&mut b, 0, 0, 200);
    b.write_raw(0x420c, 1);
    until(&mut b, 0, 1, 1150);
    assert_eq!(b.apu.input[0], 0);
    assert_eq!(b.dma[0][10], 0);
    until(&mut b, 1, 0, 200);
    assert_eq!(b.dma[0][10], 0x82);
}
#[test]
fn different_channel_dma_crosses_init_and_first_hblanks_once() {
    for indirect in [false, true] {
        for wrap in [false, true] {
            let mut b = configured(indirect);
            if wrap {
                b.write_raw(0x420c, 0);
                until(&mut b, 0, 261, 1200);
                b.write_raw(0x420c, 1);
            }
            channel(&mut b, 1, 0, 0x7e1000, 0x41);
            b.dma[1][5..7].copy_from_slice(&400u16.to_le_bytes());
            b.wram[0x1000..0x1190].fill(0x77);
            b.write_raw(0x420b, 2);
            b.cart.rom[0] = 0x1a; // INC A: only this instruction may execute.
            let mut cpu = Cpu {
                pc: 0x8000,
                a: 0x55,
                ..Cpu::default()
            };
            let pc = cpu.pc;
            let before = b.clocks;
            cpu.step(&mut b);
            assert_eq!((cpu.pc, cpu.a), (pc + 1, 0x56));
            assert!(b.clocks - before > 3200);
            assert_eq!(b.frames, u64::from(wrap));
            assert_eq!(&b.dma[1][2..7], &[0x90, 0x11, 0x7e, 0, 0]);
            assert_eq!(b.apu.input[..2], [0x22, 0x77]);
            // Issue #33: final payload reloads zero and its terminating pointer now.
            assert_eq!(&b.dma[0][8..11], &[if indirect { 5 } else { 4 }, 1, 0]);
            if indirect {
                assert_eq!(&b.dma[0][5..7], &[0, 0]);
            }
            assert_eq!(b.io[11], 0);
            let samples = b.apu.samples.len() as f64 / 2.0;
            assert!((samples - b.clocks as f64 * 32000.0 / 21477272.0).abs() <= 1.0);
        }
    }
}

#[test]
fn all_eight_channels_prepare_then_transfer_in_priority_order() {
    let mut b = bus();
    for i in 0..8 {
        channel(
            &mut b,
            i,
            if i & 1 != 0 { 0x40 } else { 0 },
            0x7e0100 + i as u32 * 4,
            0x40,
        );
        let table = 0x100 + i * 4;
        b.wram[table..table + 4].copy_from_slice(&[0x82, 0, 2, 0]);
        if i & 1 == 0 {
            b.wram[table + 1] = i as u8 + 1;
        }
        b.dma[i][7] = 0x7e;
    }
    b.wram[0x200] = 8;
    b.write_raw(0x420c, 255);
    until(&mut b, 0, 0, 300);
    assert_eq!(b.apu.input[0], 0);
    for i in 0..8 {
        assert_eq!(b.dma[i][10], 0x82);
        assert_eq!(b.dma[i][8], (i * 4 + if i & 1 != 0 { 3 } else { 1 }) as u8);
    }
    until(&mut b, 0, 0, 1250);
    assert_eq!(b.apu.input[0], 8); // Last enabled channel owns the final write.
    for i in 0..8 {
        assert_eq!(b.dma[i][10], 0x81);
    }
}

#[test]
fn initial_80_header_is_a_fresh_row_and_zero_direct_header_has_no_payload() {
    for indirect in [false, true] {
        let mut b = configured(indirect);
        b.wram[0x100] = 0x80;
        until(&mut b, 0, 0, 200);
        assert_eq!(b.dma[0][10], 0x80);
        until(&mut b, 0, 0, 1150);
        assert_eq!(b.apu.input[0], 0x11);
        assert_eq!(b.dma[0][10], 0x7f);
        until(&mut b, 0, 1, 1150);
        assert_eq!(b.apu.input[0], 0x11);
        assert_eq!(b.dma[0][10], 0x7e);
    }
    let mut b = configured(false);
    b.wram[0x100] = 0;
    b.dma[0][1] = 0x22;
    until(&mut b, 0, 0, 200);
    assert_eq!(&b.dma[0][8..11], &[1, 1, 0]);
    assert_eq!(b.io[12], 1);
    until(&mut b, 0, 1, 1150);
    b.write_raw(0x2122, 0x35);
    b.write_raw(0x2122, 0x12);
    assert_eq!(&b.ppu.cgram[..2], &[0x35, 0x12]); // Init did not flip the port latch.
}
