//! Original MIT DMA/HDMA address and side-effect vectors for Issues #21/#23/#25/#27/#29.
//!
//! Issue #33 corrects row-end expectations below: all payloads precede reloads;
//! completed repeated rows immediately expose the next header, held final lines
//! terminate now, and indirect zero headers overwrite DAS with zero+high.
//! See docs/issue-33-validation.md for the per-vector derivations and sources.
use super::*;

fn bus() -> SystemBus {
    SystemBus::new(Cartridge::load(include_bytes!("../../../../tests/roms/smoke.bin")).unwrap())
}

// Retained exclusion/approximation controls; accessible $4210-$421f have
// separate specification-derived register vectors below.
// $41ff/$4200 are adjacent excluded regions, so neither is an allowed neighbor.
const OFFSETS: &[(u16, bool)] = &[
    (0x20ff, false),
    (0x2100, true),
    (0x2180, true),
    (0x21ff, true),
    (0x2200, false),
    (0x3fff, false),
    (0x4000, true),
    (0x4016, true),
    (0x4100, true),
    (0x41ff, true),
    (0x4200, true),
    (0x420b, true),
    (0x420f, true),
    (0x4220, false),
    (0x42ff, false),
    (0x4300, true),
    (0x4345, true),
    (0x437f, true),
    (0x4380, false),
];
const BANKS: &[(u8, bool)] = &[
    (0x00, true),
    (0x01, true),
    (0x3e, true),
    (0x3f, true),
    (0x40, false),
    (0x7d, false),
    (0x7e, false),
    (0x7f, false),
    (0x80, true),
    (0x81, true),
    (0xbe, true),
    (0xbf, true),
    (0xc0, false),
    (0xff, false),
];

#[test]
fn dma_a_map_boundaries_and_forward_values() {
    for &(bank, io_bank) in BANKS {
        for &(offset, excluded) in OFFSETS {
            let address = (u32::from(bank) << 16) | u32::from(offset);
            let blocked = io_bank && excluded;
            let mut b = bus();
            b.cart.rom.fill(0x6b);
            b.cart.sram.fill(0x6d);
            b.wram.fill(0x7c);
            b.open = 0x5a;
            b.dma[0][0] = 0;
            b.dma[0][1] = 6; // Legal B-bus MOSAIC write, no WRAM-port conflict.
            b.transfer(0, address, 0);
            let expected = if blocked {
                0
            } else if io_bank {
                0x5a
            } else if bank == 0x7e || bank == 0x7f {
                0x7c
            } else if bank == 0x7d || bank == 0xff {
                0x6d
            } else {
                0x6b
            };
            assert_eq!(b.ppu.regs[6], expected, "{address:06x}");
            assert_eq!(b.open, expected);
            assert_eq!(b.clocks, 8);
        }
    }
}

#[test]
fn dma_a_writes_preserve_cartridge_and_wram_decoding_outside_io_banks() {
    for bank in [0x40, 0x7d, 0x7e, 0x7f, 0xc0, 0xff] {
        for &(offset, _) in OFFSETS {
            let mut b = bus();
            b.cart.rom.fill(0x6b);
            b.cart.sram.fill(0x6d);
            b.ppu.cgram[..2].copy_from_slice(&[0x35, 0x12]);
            b.dma[0][0] = 0x80;
            b.dma[0][1] = 0x3b; // Legal B-bus CGRAM read.
            let a = (bank << 16) | u32::from(offset);
            b.transfer(0, a, 0);
            let expected = match bank {
                0x7d | 0x7e | 0x7f | 0xff => 0x35,
                _ => 0x6b, // ROM writes are ignored by cartridge decoding.
            };
            assert_eq!(b.read_raw(a), expected, "{a:06x}");
            assert_eq!(b.ppu.read(0x3b, 0) & 0x7f, 0x12);
        }
    }
}

fn general(b: &mut SystemBus, mode: u8, port: u8, a: u32, count: u16) {
    b.dma[0] = [0; 16];
    b.dma[0][0] = mode;
    b.dma[0][1] = port;
    b.dma[0][2..4].copy_from_slice(&(a as u16).to_le_bytes());
    b.dma[0][4] = (a >> 16) as u8;
    b.dma[0][5..7].copy_from_slice(&count.to_le_bytes());
    b.dma_pending = 1;
    b.events();
}

#[test]
fn dma_forward_prohibited_reads_preserve_side_effects_and_cpu_controls() {
    for a in [0x2180, 0x4016, 0x4017] {
        let mut b = bus();
        b.wram[0x1000..0x1002].copy_from_slice(&[0x5a, 0x12]);
        b.waddr = 0x1000;
        b.serial = [0x9180, 0x5a60];
        b.nmi_flag = true;
        b.irq = true;
        b.open = 0xff;
        b.ppu.cgram[..2].fill(0x7f);
        general(&mut b, 8, 0x22, a, 2);
        assert_eq!(&b.ppu.cgram[..2], &[0, 0], "{a:04x}");
        assert_eq!(b.waddr, 0x1000);
        assert_eq!(b.serial, [0x9180, 0x5a60]);
        assert!(b.nmi_flag && b.irq);
        assert_eq!(b.clocks, 16 + 2 * 8);
        // Real CPU bus accesses still consume/acknowledge the same registers.
        match a {
            0x2180 => {
                assert_eq!(b.read(a), 0x5a);
                assert_eq!(b.waddr, 0x1001);
            }
            0x4016 => {
                assert_eq!(b.read(a) & 1, 1);
                assert_eq!(b.serial, [0x2301, 0x5a60]);
            }
            0x4017 => {
                assert_eq!(b.read(a) & 1, 0);
                assert_eq!(b.serial, [0x9180, 0xb4c1]);
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn dma_reverse_prohibited_writes_still_consume_b_bus_reads() {
    for a in [
        0x2106, 0x2180, 0x2181, 0x4016, 0x4200, 0x4203, 0x420b, 0x420c, 0x4300, 0x437f,
    ] {
        let mut b = bus();
        b.ppu.cgram[..3].copy_from_slice(&[0x35, 0x12, 0x67]);
        b.waddr = 0x1000;
        b.wram[0x1000] = 0x5a;
        b.serial = [0x9180, 0x5a60];
        b.nmi_flag = true;
        b.dma[0][0] = 0x88;
        b.dma[0][1] = 0x3b;
        let dma = b.dma;
        let io = b.io;
        b.transfer(0, a, 0);
        b.transfer(0, a, 0);
        assert_eq!(b.io, io, "{a:04x}");
        assert_eq!(b.dma, dma, "{a:04x}");
        assert_eq!(b.ppu.regs[6], 0);
        assert_eq!(b.waddr, 0x1000);
        assert_eq!(b.wram[0x1000], 0x5a);
        assert_eq!(b.serial, [0x9180, 0x5a60]);
        assert!(!b.strobe && !b.nmi_pending);
        assert!(b.math.is_none());
        assert_eq!(b.dma_pending, 0);
        assert_eq!(b.open, 0x12);
        assert_eq!(b.ppu.read(0x3b, 0), 0x67, "legal B read must advance");
        assert_eq!(b.clocks, 16);
        b.write(a, 0x35); // Legal CPU write control.
        match a {
            0x2106 => assert_eq!(b.ppu.regs[6], 0x35),
            0x2180 => assert_eq!((b.wram[0x1000], b.waddr), (0x35, 0x1001)),
            0x2181 => assert_eq!(b.waddr, 0x1035),
            0x4016 => assert!(b.strobe),
            0x4200 | 0x4203 | 0x420b | 0x420c => assert_eq!(b.io[(a - 0x4200) as usize], 0x35),
            0x4300 => assert_eq!(b.dma[0][0], 0x35),
            0x437f => assert_eq!(b.dma[7][15], 0x35),
            _ => unreachable!(),
        }
    }
}

#[test]
fn dma_progression_rechecks_each_byte_and_wraps_without_bank_carry() {
    // Literal output vectors: legal unmapped addresses read the previous open
    // value, excluded addresses inject zero, and $0000 in bank $80 conflicts.
    for (mode, start, count, end, expected) in [
        (8, 0x4016, 3, 0x4016, vec![0, 0, 0]),
        (0, 0x20ff, 3, 0x2102, vec![0x5a, 0, 0]),
        (0, 0x21ff, 3, 0x2202, vec![0, 0, 0]),
        (16, 0x2200, 3, 0x21fd, vec![0x5a, 0, 0]),
        (16, 0x2100, 3, 0x20fd, vec![0, 0, 0]),
        (0, 0xffff, 2, 0x0001, vec![0x6b]),
        (16, 0x0000, 2, 0xfffe, vec![0x6b]),
    ] {
        let mut b = bus();
        b.cart.rom.fill(0x6b);
        b.wram[0] = 0x37;
        b.wram[0x10000..0x10004].fill(0xa6);
        b.waddr = 0x10000;
        b.open = 0x5a;
        general(&mut b, mode, 0x80, 0x800000 | start, count);
        assert_eq!(&b.wram[0x10000..0x10000 + expected.len()], expected);
        assert_eq!(b.waddr, 0x10000 + expected.len());
        assert_eq!(b.wram[0x10000 + expected.len()], 0xa6);
        assert_eq!(u16::from_le_bytes([b.dma[0][2], b.dma[0][3]]), end);
        assert_eq!(b.dma[0][4..7], [0x80, 0, 0]);
        assert_eq!(b.clocks, 16 + u64::from(count) * 8);
        assert_eq!(b.dma_pending, 0);
    }
    // Reverse direction: B CGRAM continues incrementing across both boundaries;
    // a WRAM B port is deliberately avoided when A is WRAM.
    for (mode, start, end) in [
        (0x80, 0x21ff, 0x2202),
        (0x90, 0x2200, 0x21fd),
        (0x88, 0x4200, 0x4200),
    ] {
        let mut b = bus();
        b.ppu.cgram[..4].copy_from_slice(&[0x35, 0x12, 0x67, 0x23]);
        general(&mut b, mode, 0x3b, start, 3);
        assert_eq!(u16::from_le_bytes([b.dma[0][2], b.dma[0][3]]), end);
        assert_eq!(b.dma[0][4..7], [0, 0, 0]);
        assert_eq!(b.ppu.read(0x3b, 0) & 0x7f, 0x23);
        assert_eq!(b.clocks, 40);
        assert_eq!(b.io[0], 0);
    }
    for (mode, start, end, written) in [(0x80, 0x80ffff, 1, 0x12), (0x90, 0x800000, 0xfffe, 0x35)] {
        let mut b = bus();
        b.ppu.cgram[..2].copy_from_slice(&[0x35, 0x12]);
        general(&mut b, mode, 0x3b, start, 2);
        assert_eq!(b.wram[0], written);
        assert_eq!(u16::from_le_bytes([b.dma[0][2], b.dma[0][3]]), end);
        assert_eq!(b.dma[0][4..7], [0x80, 0, 0]);
        assert_eq!(b.clocks, 32);
    }
}

fn hdma(b: &mut SystemBus, mode: u8, table: u32) {
    // These isolate payload access rules with synthetic events. Start beyond
    // the natural scanline-0 init point; hdma_init_tests covers real scheduling.
    b.h = HDMA_INIT;
    b.dma[0] = [0; 16];
    b.dma[0][0] = mode;
    b.dma[0][1] = 6;
    b.dma[0][2..4].copy_from_slice(&(table as u16).to_le_bytes());
    b.dma[0][4] = (table >> 16) as u8;
    b.io[12] = 1;
    b.hdma_init = true;
    b.hdma_pending = true;
}

#[test]
fn hdma_prohibited_descriptors_terminate_without_register_side_effects() {
    for bank in [0x00, 0x3f, 0x80, 0xbf] {
        for a in [0x2180, 0x4016, 0x4017, 0x4300] {
            let mut b = bus();
            b.waddr = 0x1000;
            b.wram[0x1000] = 0x81;
            b.serial = [0x9180, 0x5a60];
            b.nmi_flag = true;
            b.irq = true;
            hdma(&mut b, 0, bank << 16 | a);
            b.ppu.regs[6] = 0x55;
            b.events();
            assert_eq!(b.hdma_active, 0, "{bank:02x}:{a:04x}");
            assert_eq!(u16::from_le_bytes([b.dma[0][8], b.dma[0][9]]), a as u16 + 1);
            assert_eq!(b.waddr, 0x1000);
            assert_eq!(b.serial, [0x9180, 0x5a60]);
            assert!(b.nmi_flag && b.irq);
            assert_eq!(b.ppu.regs[6], 0x55);
            assert_eq!(b.clocks, 8);
        }
    }
}

#[test]
fn hdma_direct_and_indirect_payloads_use_restricted_a_bus_in_both_directions() {
    for indirect in [false, true] {
        for reverse in [false, true] {
            for a in [0x2180u16, 0x4016, 0x4017, 0x4300] {
                let mut b = bus();
                let mode = if indirect { 0x40 } else { 0 } | if reverse { 0x80 } else { 0 };
                hdma(&mut b, mode, 0x7e0000);
                b.events(); // Initialize with a legal terminating table.
                b.hdma_active = 1;
                b.hdma_line[0] = 0x82; // Isolate a payload on an existing row.
                b.hdma_transfer[0] = true;
                b.dma[0][1] = if reverse { 0x3b } else { 6 };
                b.dma[0][4] = 0;
                b.dma[0][7] = 0;
                let pair = if indirect { 5 } else { 8 };
                b.dma[0][pair..pair + 2].copy_from_slice(&a.to_le_bytes());
                b.waddr = 0x1000;
                b.wram[0x1000] = 0x55;
                b.serial = [0x9180, 0x5a60];
                b.nmi_flag = true;
                b.irq = true;
                b.ppu.regs[6] = 0x55;
                b.ppu.cgram[..2].copy_from_slice(&[0x35, 0x12]);
                let table = b.dma[0][8..10].to_vec();
                let before = b.clocks;
                b.hdma_pending = true;
                b.events();
                assert_eq!(b.ppu.regs[6], if reverse { 0x55 } else { 0 });
                assert_eq!(b.waddr, 0x1000);
                assert_eq!(b.serial, [0x9180, 0x5a60]);
                assert!(b.nmi_flag && b.irq);
                assert_eq!(b.dma[0][0], mode);
                assert_eq!(
                    u16::from_le_bytes([b.dma[0][pair], b.dma[0][pair + 1]]),
                    a + 1
                );
                if indirect {
                    assert_eq!(&b.dma[0][8..10], table);
                }
                if reverse {
                    assert_eq!(b.ppu.read(0x3b, 0) & 0x7f, 0x12);
                }
                assert_eq!(b.hdma_line[0], 0x81);
                assert_eq!(b.clocks - before, 8);
            }
        }
    }
}

#[test]
fn hdma_indirect_pointer_fetches_recheck_boundaries_and_wrap() {
    // A legal open-bus descriptor immediately before $2100/$4300, followed
    // by blocked pointer bytes; and a descriptor/pointer crossing bank end.
    for (table, pointer, end, clocks, output) in [
        (0x0020ff, 0, 0x2104, 48, 1), // payload uses $0000; termination publishes $0000
        (0x0042ff, 0, 0x4304, 48, 1),
        (0x80fffe, 0, 0x0003, 48, 0x66), // initial pointer $0181; row-end zero then high from $0002
    ] {
        let mut b = bus();
        b.open = 0x81;
        b.cart.rom[0x7ffe..].fill(0x81);
        b.wram[0] = 1;
        b.wram[0x181] = 0x66;
        hdma(&mut b, 0x40, table);
        b.events();
        assert_eq!(u16::from_le_bytes([b.dma[0][5], b.dma[0][6]]), pointer);
        assert_eq!(u16::from_le_bytes([b.dma[0][8], b.dma[0][9]]), end);
        assert_eq!(b.dma[0][4], (table >> 16) as u8);
        assert_eq!(b.ppu.regs[6], output);
        assert_eq!(b.clocks, clocks);
    }
    // Each indirect-byte fetch uses the same restricted path even at ports
    // with read side effects (a preloaded row would normally have a descriptor).
    for a in [0x2180u16, 0x4016, 0x4017] {
        let mut b = bus();
        b.dma[0][4] = 0;
        b.dma[0][8..10].copy_from_slice(&a.to_le_bytes());
        b.waddr = 0x1000;
        b.serial = [0x9180, 0x5a60];
        b.nmi_flag = true;
        b.irq = true;
        assert_eq!(b.hdma_read(0), 0);
        assert_eq!(b.waddr, 0x1000);
        assert_eq!(b.serial, [0x9180, 0x5a60]);
        assert!(b.nmi_flag && b.irq);
        assert_eq!(u16::from_le_bytes([b.dma[0][8], b.dma[0][9]]), a + 1);
        assert_eq!(b.clocks, 8);
    }
}

#[test]
fn hdma_tables_feed_blocked_payloads_and_then_terminate() {
    for reverse in [false, true] {
        for a in [0x2180u16, 0x4016, 0x4017, 0x4300] {
            let mut b = bus();
            b.wram[0x100..0x104].copy_from_slice(&[0x81, a as u8, (a >> 8) as u8, 0]);
            hdma(&mut b, 0x40 | if reverse { 0x80 } else { 0 }, 0x7e0100);
            b.dma[0][7] = 0; // Indirect bank is independent of table bank.
            b.dma[0][1] = if reverse { 0x3b } else { 6 };
            b.ppu.cgram[..2].copy_from_slice(&[0x35, 0x12]);
            b.ppu.regs[6] = 0x55;
            b.waddr = 0x1000;
            b.wram[0x1000] = 0x55;
            b.serial = [0x9180, 0x5a60];
            b.nmi_flag = true;
            b.irq = true;
            b.events();
            assert_eq!(b.dma[0][8..10], [5, 1]);
            assert_eq!(u16::from_le_bytes([b.dma[0][5], b.dma[0][6]]), 0);
            assert_eq!(b.waddr, 0x1000);
            assert_eq!(b.serial, [0x9180, 0x5a60]);
            assert!(b.nmi_flag && b.irq);
            assert_eq!(b.ppu.regs[6], if reverse { 0x55 } else { 0 });
            if reverse {
                assert_eq!(b.ppu.read(0x3b, 0) & 0x7f, 0x12);
            }
            assert_eq!(b.clocks, 48);
            b.hdma_pending = true;
            b.events();
            assert_eq!(b.hdma_active, 0);
            assert_eq!(b.dma[0][8..10], [5, 1]);
            assert_eq!(b.clocks, 48);
        }
        let mut b = bus();
        b.open = 0x81; // Legal descriptor at $20ff, direct payload at $2100.
        hdma(&mut b, if reverse { 0x80 } else { 0 }, 0x0020ff);
        b.dma[0][1] = if reverse { 0x3b } else { 6 };
        b.ppu.regs[0] = 0x80;
        b.ppu.cgram[..2].copy_from_slice(&[0x35, 0x12]);
        b.events();
        assert_eq!(b.dma[0][8..10], [2, 0x21]);
        assert_eq!(b.ppu.regs[0], 0x80);
        assert_eq!(b.ppu.regs[6], 0);
        if reverse {
            assert_eq!(b.ppu.read(0x3b, 0) & 0x7f, 0x12);
        }
        assert_eq!(b.clocks, 24);
        b.hdma_pending = true;
        b.events();
        assert_eq!(b.hdma_active, 0);
        assert_eq!(b.dma[0][8..10], [2, 0x21]);
        assert_eq!(b.clocks, 24);
    }
}

// Fullsnes: Unpredictable Things / Open Bus for DMA; PPU Interrupts;
// Maths Multiply/Divide; Controllers. Only driven status bits are asserted.
const REGISTER_MASKS: [u8; 16] = [
    0x80, 0x80, 0xc1, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
];
const REGISTER_VALUES: [u8; 16] = [
    0x80, 0x80, 0xc1, 0xa6, 0x0a, 0x09, 0x0f, 0, 0x90, 0xa5, 0, 0, 0, 0, 0, 0,
];

fn register_bus() -> SystemBus {
    let mut b = bus();
    b.write_raw(0x4201, 0xa6); // Existing RDIO model, not external pin coverage.
    b.write_raw(0x4204, 0xcd);
    b.write_raw(0x4205, 0xab);
    b.write_raw(0x4206, 19); // $abcd / 19 = $090a, remainder $000f.
    for _ in 0..16 {
        b.idle();
    }
    b.buttons = 0xa590;
    b.write_raw(0x4200, 1);
    b.tick(225 * LINE + 298 + 4224); // Real automatic first-controller completion.
    assert_eq!(b.joy, [0xa590, 0, 0, 0]);
    b.hdma_pending = false;
    b.v = 230;
    b.h = 1200; // Stable V/H blank, away from comparisons and refresh.
    b.auto_poll = Some(super::AutoJoypad {
        next: b.clocks + 1000,
        step: 1,
    });
    b.nmi_flag = true;
    b.irq = true;
    b
}

#[test]
fn dma_reads_all_status_result_offsets_and_bank_controls() {
    for bank in [0x00, 0x3f, 0x80, 0xbf, 0x40, 0xc0, 0x7e, 0x7f] {
        for (offset, (&value, &mask)) in REGISTER_VALUES.iter().zip(&REGISTER_MASKS).enumerate() {
            let mut b = register_bus();
            b.cart.rom.fill(0x6b);
            b.wram.fill(0x7c);
            let a = (bank << 16) | (0x4210 + offset as u32);
            let io_bank = matches!(bank, 0x00 | 0x3f | 0x80 | 0xbf);
            let expected = if io_bank {
                value
            } else if bank == 0x7e || bank == 0x7f {
                0x7c
            } else {
                0x6b
            };
            let mask = if io_bank { mask } else { 0xff };
            let before = b.clocks;
            // MOSAIC avoids the separate WRAM/$2180 conflict for WRAM controls.
            general(&mut b, 8, 6, a, 1);
            assert_eq!(b.ppu.regs[6] & mask, expected & mask, "{a:06x}");
            assert_eq!(b.nmi_flag, !(io_bank && offset == 0));
            assert_eq!(b.irq, !(io_bank && offset == 1));
            assert_eq!(
                b.dma[0][2..7],
                [0x10 + offset as u8, 0x42, bank as u8, 0, 0]
            );
            assert_eq!(b.clocks - before, 24);
        }
    }
}

#[test]
fn dma_repeated_status_reads_acknowledge_only_the_addressed_flag() {
    for bank in [0x00, 0x3f, 0x80, 0xbf] {
        for offset in [0x10, 0x11] {
            let mut b = bus();
            b.nmi_flag = true;
            b.nmi_pending = true;
            b.irq = true;
            b.waddr = 0x10000;
            general(&mut b, 8, 0x80, bank << 16 | 0x4200 | offset, 2);
            assert_eq!([b.wram[0x10000] & 0x80, b.wram[0x10001] & 0x80], [0x80, 0]);
            assert_eq!(b.nmi_flag, offset != 0x10);
            assert_eq!(b.irq, offset != 0x11);
            assert!(
                b.nmi_pending,
                "read-clear must preserve the queued NMI event"
            );
            assert_eq!(b.dma[0][2..7], [offset as u8, 0x42, bank as u8, 0, 0]);
            assert_eq!(b.waddr, 0x10002);
            assert_eq!(b.clocks, 32);
        }
    }
}

#[test]
fn dma_acknowledgement_preserves_delivery_of_an_already_queued_nmi() {
    let mut m = Machine::new(include_bytes!("../../../../tests/roms/smoke.bin")).unwrap();
    m.bus.cart.rom[0x7ffa..0x7ffc].copy_from_slice(&[0, 0x90]);
    m.bus.cart.rom[0x1000..0x1002].copy_from_slice(&[0xe6, 0x10]); // INC $10
    m.bus.nmi_flag = true;
    m.bus.nmi_pending = true;
    general(&mut m.bus, 8, 6, 0x4210, 2);
    assert!(!m.bus.nmi_flag);
    assert!(m.bus.nmi_pending);
    m.step();
    assert!(!m.bus.nmi_pending);
    assert_eq!(m.bus.wram[0x10], 1);
    assert_eq!(m.cpu.pc, 0x9002);
}

#[test]
fn dma_hvbjoy_uses_driven_bits_at_stable_states() {
    for (v, h, busy, expected) in [
        (100, 200, false, 0),
        (100, 1200, false, 0x40),
        (230, 200, false, 0x80),
        (230, 1200, false, 0xc0),
        (226, 200, true, 0x81),
        (226, 1200, true, 0xc1),
    ] {
        let mut b = bus();
        b.v = v;
        b.h = h;
        b.auto_poll = busy.then_some(super::AutoJoypad {
            next: 1000,
            step: 1,
        });
        b.open = 0x3e; // Floating bits must not enter the expectation.
        general(&mut b, 8, 6, 0x4212, 2);
        assert_eq!(b.ppu.regs[6] & 0xc1, expected);
    }
}

#[test]
fn dma_reads_completed_multiplication_results() {
    let mut b = bus();
    b.write_raw(0x4202, 255);
    b.write_raw(0x4203, 171);
    for _ in 0..8 {
        b.idle();
    }
    b.waddr = 0x10000;
    general(&mut b, 0, 0x80, 0x4216, 2);
    assert_eq!(&b.wram[0x10000..0x10002], &[0x55, 0xaa]);
    assert_eq!(b.dma[0][2..7], [0x18, 0x42, 0, 0, 0]);
}

#[test]
fn dma_status_range_boundaries_keep_address_count_and_bus_progression() {
    // $420f exclusion and $4220 open-bus values are retained approximations.
    for (mode, start, end, values, masks) in [
        (0, 0x420f, 0x4212, [0, 0x80, 0x80], [0xff, 0x80, 0x80]),
        (16, 0x4211, 0x420e, [0x80, 0x80, 0], [0x80, 0x80, 0xff]),
        (0, 0x421f, 0x4222, [0, 0, 0], [0xff; 3]),
        (16, 0x4220, 0x421d, [0x5a, 0, 0], [0xff; 3]),
        (8, 0x420f, 0x420f, [0, 0, 0], [0xff; 3]),
        (8, 0x4210, 0x4210, [0x80, 0, 0], [0x80; 3]),
        (8, 0x421f, 0x421f, [0, 0, 0], [0xff; 3]),
        (8, 0x4220, 0x4220, [0x5a; 3], [0xff; 3]),
    ] {
        let mut b = bus();
        b.open = 0x5a;
        b.nmi_flag = true;
        b.irq = true;
        b.waddr = 0x10000;
        general(&mut b, mode, 0x80, 0xbf0000 | start, 3);
        for i in 0..3 {
            assert_eq!(b.wram[0x10000 + i] & masks[i], values[i]);
        }
        assert_eq!(u16::from_le_bytes([b.dma[0][2], b.dma[0][3]]), end);
        assert_eq!(b.dma[0][4..7], [0xbf, 0, 0]);
        assert_eq!((b.waddr, b.clocks, b.dma_pending), (0x10003, 40, 0));
    }
}

#[test]
fn dma_status_result_writes_remain_read_only_with_legal_b_bus_activity() {
    for bank in [0x00, 0x3f, 0x80, 0xbf] {
        for offset in 0x10..=0x1f {
            let mut b = register_bus();
            let io = b.io;
            let joy = b.joy;
            b.ppu.cgram[..4].copy_from_slice(&[0x35, 0x12, 0x67, 0x23]);
            general(&mut b, 0x88, 0x3b, bank << 16 | 0x4200 | offset, 2);
            assert_eq!(b.io, io);
            assert_eq!(b.joy, joy);
            assert!(b.nmi_flag && b.irq);
            assert_eq!(b.ppu.read(0x3b, 0) & 0x7f, 0x67);
            assert_eq!(b.dma[0][2..7], [offset as u8, 0x42, bank as u8, 0, 0]);
        }
    }
}

#[test]
fn hdma_direct_indirect_status_result_payloads_and_reverse_controls() {
    for indirect in [false, true] {
        for reverse in [false, true] {
            for bank in [0x00, 0x3f, 0x80, 0xbf] {
                for (offset, (&value, &mask)) in
                    REGISTER_VALUES.iter().zip(&REGISTER_MASKS).enumerate()
                {
                    let mut b = register_bus();
                    let a = 0x4210 + offset as u16;
                    // An existing row isolates payload decoding from table decoding.
                    b.hdma_active = 1;
                    b.io[12] = 1;
                    b.hdma_line[0] = 0x82;
                    b.hdma_transfer[0] = true;
                    b.dma[0] = [0; 16];
                    b.dma[0][0] = if indirect { 0x40 } else { 0 } | if reverse { 0x80 } else { 0 };
                    b.dma[0][1] = if reverse { 0x3b } else { 6 };
                    b.dma[0][4] = bank;
                    b.dma[0][7] = bank;
                    let pair = if indirect { 5 } else { 8 };
                    b.dma[0][pair..pair + 2].copy_from_slice(&a.to_le_bytes());
                    let io = b.io;
                    let table = b.dma[0][8..10].to_vec();
                    b.ppu.cgram[..2].copy_from_slice(&[0x35, 0x12]);
                    b.ppu.regs[6] = 0x55;
                    let before = b.clocks;
                    b.hdma_pending = true;
                    b.events();
                    assert_eq!(b.nmi_flag, reverse || offset != 0);
                    assert_eq!(b.irq, reverse || offset != 1);
                    assert_eq!(b.io, io);
                    assert_eq!(
                        b.ppu.regs[6] & mask,
                        if reverse { 0x55 & mask } else { value }
                    );
                    assert_eq!(
                        u16::from_le_bytes([b.dma[0][pair], b.dma[0][pair + 1]]),
                        a + 1
                    );
                    if indirect {
                        assert_eq!(&b.dma[0][8..10], table);
                    }
                    if reverse {
                        assert_eq!(b.ppu.read(0x3b, 0) & 0x7f, 0x12);
                    }
                    assert_eq!((b.hdma_active, b.hdma_line[0]), (1, 0x81));
                    assert_eq!(b.clocks - before, 8);
                }
            }
        }
    }
}

#[test]
fn hdma_accessible_descriptors_use_fetched_value_and_acknowledge() {
    for bank in [0x00, 0x3f, 0x80, 0xbf] {
        // TIMEUP with only bit 7 driven needs masking for status checks, but
        // its full zero value here is the existing deterministic bus model.
        for (a, pending, line, payload, end, active) in [
            (0x4210, true, 0x81, 0x80, 0x4212, 1), // $82 descriptor, IRQ payload
            (0x4210, false, 1, 0x80, 0x4212, 1),   // CPU version makes $02 nonzero
            (0x4211, true, 0x7f, 0, 0x4213, 1),    // $80 descriptor, HVBJOY payload
            (0x4211, false, 0, 0x55, 0x4212, 0),   // real zero descriptor
            (0x4213, false, 0, 0x55, 0x4214, 0),   // zero RDIO descriptor
        ] {
            let mut b = bus();
            if a == 0x4213 {
                b.write_raw(0x4201, 0); // Explicit zero RDIO descriptor, independent of reset.
            }
            b.nmi_flag = pending;
            b.nmi_pending = true;
            b.irq = a != 0x4211 || pending;
            b.ppu.regs[6] = 0x55;
            hdma(&mut b, 0, bank << 16 | a);
            b.events();
            assert_eq!(b.hdma_active, active);
            assert_eq!(b.hdma_line[0], line);
            assert_eq!(b.ppu.regs[6] & 0x80, payload & 0x80);
            assert_eq!(u16::from_le_bytes([b.dma[0][8], b.dma[0][9]]), end);
            if a == 0x4210 {
                assert!(!b.nmi_flag && !b.irq);
            }
            if a == 0x4211 {
                assert!(!b.irq);
            }
            assert!(b.nmi_pending);
            assert_eq!(b.clocks, if active == 0 { 8 } else { 16 });
        }
    }
}

#[test]
fn hdma_accessible_indirect_pointer_bytes_keep_values_and_side_effects() {
    for bank in [0x00, 0x3f, 0x80, 0xbf] {
        // $420f remains excluded. Starting at RDNMI gives a nonzero descriptor
        // followed by TIMEUP/HVBJOY as the two accessible pointer bytes.
        for (table, pointer, end, payload) in [
            (0x4210, 0x0080u16, 0x4213, 0x66),
            (0x4213, 0x090a, 0x4216, 0x77),
        ] {
            let mut b = register_bus();
            b.h = 0;
            b.v = 100;
            b.auto_poll = None;
            b.open = 0;
            b.io[1] = 1; // RDIO descriptor = one row.
            b.wram[usize::from(pointer)] = payload;
            hdma(&mut b, 0x40, bank << 16 | table);
            b.dma[0][7] = 0x7e;
            let before = b.clocks;
            b.events();
            assert_eq!(b.ppu.regs[6], payload);
            assert_eq!(
                u16::from_le_bytes([b.dma[0][5], b.dma[0][6]]),
                // RDIO $01 finishes now: reload $0F from $4216,
                // then $00,$90 from $4217/18; fresh row stays unconsumed.
                if table == 0x4213 { 0x9000 } else { pointer + 1 }
            );
            assert_eq!(
                u16::from_le_bytes([b.dma[0][8], b.dma[0][9]]),
                if table == 0x4213 { 0x4219 } else { end }
            );
            assert_eq!(b.hdma_active, 1);
            assert_eq!(b.dma[0][10], if table == 0x4213 { 0x0f } else { 0x81 });
            if table == 0x4210 {
                assert!(!b.nmi_flag && !b.irq);
            }
            assert_eq!(b.clocks - before, if table == 0x4213 { 56 } else { 32 });
        }
        // Exercise RDNMI and TIMEUP as low and high pointer fetches, without
        // inventing an accessible descriptor at excluded $420f.
        let mut b = bus();
        b.dma[0][4] = bank as u8;
        b.dma[0][8..10].copy_from_slice(&0x4210u16.to_le_bytes());
        b.nmi_flag = true;
        b.nmi_pending = true;
        b.irq = true;
        assert_eq!(b.hdma_read(0) & 0x80, 0x80);
        assert!(!b.nmi_flag && b.irq);
        assert_eq!(b.hdma_read(0) & 0x80, 0x80);
        assert!(!b.nmi_flag && !b.irq);
        assert!(b.nmi_pending);
        assert_eq!(b.dma[0][8..10], [0x12, 0x42]);
        assert_eq!(b.clocks, 16);
    }
}

#[test]
fn hdma_loaded_status_payload_acknowledges_then_table_terminates() {
    for bank in [0x00, 0x3f, 0x80, 0xbf] {
        for a in [0x4210u16, 0x4211] {
            let mut b = bus();
            b.wram[0x100..0x104].copy_from_slice(&[0x81, a as u8, 0x42, 0]);
            hdma(&mut b, 0x40, 0x7e0100);
            b.dma[0][7] = bank;
            b.dma[0][1] = 0x80;
            b.waddr = 0x10000;
            b.nmi_flag = true;
            b.irq = true;
            b.nmi_pending = true;
            b.events();
            assert_eq!(b.wram[0x10000] & 0x80, 0x80);
            assert_eq!(b.nmi_flag, a != 0x4210);
            assert_eq!(b.irq, a != 0x4211);
            assert!(b.nmi_pending);
            assert_eq!(b.hdma_active, 0);
            assert_eq!(b.dma[0][8..10], [5, 1]);
            assert_eq!(u16::from_le_bytes([b.dma[0][5], b.dma[0][6]]), 0);
            b.hdma_pending = true;
            b.events();
            assert_eq!(b.hdma_active, 0);
            assert_eq!(b.dma[0][8..10], [5, 1]);
            assert_eq!(b.waddr, 0x10001);
            assert_eq!(b.clocks, 48);
        }
    }
}

#[test]
fn dma_wram_conflict_reproduction() {
    let mut b = bus();
    b.wram[0x100] = 0x5a;
    b.waddr = 0x11000;
    b.wram[0x11000] = 0xa6;
    general(&mut b, 0, 0x80, 0x7e0100, 1);
    assert_eq!((b.wram[0x11000], b.waddr), (0xa6, 0x11000));
    assert_eq!(b.dma[0][2..7], [1, 1, 0x7e, 0, 0]);
    assert_eq!((b.dma_pending, b.clocks), (0, 24));
}

#[test]
fn dma_wram_conflict_full_banks_and_mirror_boundaries() {
    // Literal addresses, independent of the production decoder.
    for a in [
        0x7e0000, 0x7e1fff, 0x7e2000, 0x7effff, 0x7f0000, 0x7f1fff, 0x7f2000, 0x7fffff, 0x000000,
        0x001fff, 0x3f0000, 0x3f1fff, 0x800000, 0x801fff, 0xbf0000, 0xbf1fff,
    ] {
        let mut b = bus();
        b.wram.fill(0x5a);
        b.waddr = 0x11000;
        b.wram[0x11000..0x11003].fill(0xa6);
        general(&mut b, 8, 0x80, a, 3);
        assert_eq!(&b.wram[0x11000..0x11003], &[0xa6; 3], "{a:06x}");
        assert_eq!(b.waddr, 0x11000);
        assert_eq!(u16::from_le_bytes([b.dma[0][2], b.dma[0][3]]), a as u16);
        assert_eq!(b.dma[0][4..7], [(a >> 16) as u8, 0, 0]);
        assert_eq!((b.dma_pending, b.clocks), (0, 40));
        // The same WRAM source remains legal for ordinary B-bus registers.
        general(&mut b, 8, 6, a, 1);
        assert_eq!(b.ppu.regs[6], 0x5a);
        assert_eq!(b.waddr, 0x11000);
    }
}

#[test]
fn dma_wram_conflict_non_wram_and_legal_reverse_controls() {
    for a in [0x002000, 0x3f2000, 0x802000, 0xbf2000] {
        let mut b = bus();
        b.waddr = 0x11000;
        b.wram[0x11001] = 0xa6;
        general(&mut b, 8, 0x80, a, 1);
        // Unmapped data is not a hardware value assertion; only the port access.
        assert_eq!((b.waddr, b.wram[0x11001]), (0x11001, 0xa6));
        assert_eq!(b.dma[0][4..7], [(a >> 16) as u8, 0, 0]);
        assert_eq!(b.clocks, 24);
    }
    for a in [
        0x400000, 0x401fff, 0x402000, 0xc00000, 0xc01fff, 0xc02000, 0x008000, 0x700100,
    ] {
        let mut b = bus();
        b.cart.rom.fill(0x6b);
        b.cart.sram = vec![0x6d; 0x8000];
        b.waddr = 0x11000;
        b.wram[0x11000..0x11003].fill(0xa6);
        general(&mut b, 0, 0x80, a, 2);
        let expected = if a == 0x700100 { 0x6d } else { 0x6b };
        assert_eq!(&b.wram[0x11000..0x11003], &[expected, expected, 0xa6]);
        assert_eq!(b.waddr, 0x11002);
        assert_eq!(
            u16::from_le_bytes([b.dma[0][2], b.dma[0][3]]),
            (a as u16) + 2
        );
        assert_eq!(b.dma[0][4..7], [(a >> 16) as u8, 0, 0]);
        assert_eq!(b.clocks, 32);
    }
    // WMDATA -> cartridge SRAM is legal; no reverse WRAM conflict is asserted.
    let mut b = bus();
    b.cart.sram = vec![0xa6; 0x8000];
    b.wram[0x1ffff] = 0x5a;
    b.wram[0] = 0x37;
    b.waddr = 0x1ffff;
    general(&mut b, 0x80, 0x80, 0x700100, 2);
    assert_eq!(&b.cart.sram[0x100..0x103], &[0x5a, 0x37, 0xa6]);
    assert_eq!(b.waddr, 1);
    assert_eq!(b.dma[0][2..7], [2, 1, 0x70, 0, 0]);
    assert_eq!(b.clocks, 32);
}

#[test]
fn dma_wram_conflict_progression_enters_exits_and_wraps() {
    for (mode, a, end, writes) in [
        (0, 0x001ffe, 0x2002, 2),  // Exit mirrored WRAM.
        (16, 0x3f2001, 0x1ffd, 2), // Enter mirrored WRAM.
        (0, 0x80fffe, 2, 2),       // ROM then mirrored WRAM, no bank carry.
        (16, 0xbf0001, 0xfffd, 2), // Mirrored WRAM then ROM.
        (0, 0x7efffe, 2, 0),       // Full WRAM banks stay conflicting over wrap.
        (16, 0x7f0001, 0xfffd, 0),
    ] {
        let mut b = bus();
        b.cart.rom.fill(0x6b);
        b.wram.fill(0x5a);
        b.waddr = 0x11000;
        b.wram[0x11000..0x11005].fill(0xa6);
        general(&mut b, mode, 0x80, a, 4);
        assert_eq!(b.waddr, 0x11000 + writes, "{a:06x}");
        assert_eq!(&b.wram[0x11000 + writes..0x11005], &vec![0xa6; 5 - writes]);
        assert_eq!(u16::from_le_bytes([b.dma[0][2], b.dma[0][3]]), end);
        assert_eq!(b.dma[0][4..7], [(a >> 16) as u8, 0, 0]);
        assert_eq!((b.dma_pending, b.clocks), (0, 48));
    }
}

#[test]
fn dma_wram_conflict_effective_b_pattern_and_bbad_wrapping() {
    let mut b = bus();
    b.wram[0x100..0x104].copy_from_slice(&[0x11, 0x22, 0x33, 0x44]);
    b.waddr = 0x11000;
    b.wram[0x11000..0x11003].fill(0xa6);
    // Mode 1 alternates legal APU port $217f and conflicting WMDATA $2180.
    general(&mut b, 1, 0x7f, 0x7e0100, 4);
    assert_eq!(b.apu.input[3], 0x33);
    assert_eq!(&b.wram[0x11000..0x11003], &[0xa6; 3]);
    assert_eq!(b.waddr, 0x11000);
    assert_eq!(b.dma[0][2..7], [4, 1, 0x7e, 0, 0]);
    assert_eq!(b.clocks, 48);
    // Eight-bit BBAD wrapping reaches $2100, never $2200.
    b.wram[0x100..0x102].copy_from_slice(&[0x80, 0x8f]);
    general(&mut b, 1, 0xff, 0x7e0100, 2);
    assert_eq!(b.ppu.regs[0], 0x8f);
    assert_eq!(b.waddr, 0x11000);
    assert_eq!(b.dma[0][2..7], [2, 1, 0x7e, 0, 0]);
    assert_eq!(b.clocks, 80);
}

#[test]
fn dma_wram_conflict_zero_count_consumes_65536_slots() {
    for (mode, a) in [(0, 0x7e0100), (8, 0x7f0100), (16, 0x7f0100)] {
        let mut b = bus();
        b.wram.fill(0x5a);
        b.waddr = 0x11000;
        b.wram[0x11000] = 0xa6;
        general(&mut b, mode, 0x80, a, 0);
        assert_eq!((b.waddr, b.wram[0x11000]), (0x11000, 0xa6));
        assert_eq!(b.dma[0][2..7], [0, 1, (a >> 16) as u8, 0, 0]);
        assert_eq!(b.dma_pending, 0);
        // Retained model: 16 + 65536*8 work clocks plus 396 refresh holds.
        // Exactly 396 scanlines, not a claim about hardware setup/alignment.
        assert_eq!(b.clocks, 540144);
    }
}

#[test]
fn hdma_wram_conflict_complete_direct_and_indirect_tables() {
    // Two repeated rows, a two-line non-repeating entry, then termination.
    // Mixed B pattern proves ordinary payloads continue after each conflict.
    for indirect in [false, true] {
        let mut b = bus();
        let table: &[u8] = if indirect {
            &[0x82, 0, 2, 2, 4, 2, 0]
        } else {
            &[0x82, 0x11, 0x22, 0x33, 0x44, 2, 0x55, 0x66, 0]
        };
        b.wram[0x100..0x100 + table.len()].copy_from_slice(table);
        b.wram[0x200..0x206].copy_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);
        hdma(&mut b, if indirect { 0x41 } else { 1 }, 0x7e0100);
        b.dma[0][1] = 0x7f;
        b.dma[0][7] = 0x7e;
        b.waddr = 0x11000;
        b.wram[0x11000..0x11004].fill(0xa6);
        // Final held line includes the zero header and, for indirect,
        // the following high byte; the next synthetic event is inert.
        let states = if indirect {
            [
                (0x103, 0x202, 40),
                (0x106, 0x204, 80),
                (0x106, 0x206, 96),
                (0x108, 0, 112),
                (0x108, 0, 112),
            ]
        } else {
            [
                (0x103, 0, 24),
                (0x106, 0, 48),
                (0x108, 0, 64),
                (0x109, 0, 72),
                (0x109, 0, 72),
            ]
        };
        for (row, (table, payload, clocks)) in states.into_iter().enumerate() {
            b.hdma_pending = true;
            b.events();
            assert_eq!(u16::from_le_bytes([b.dma[0][8], b.dma[0][9]]), table);
            assert_eq!(u16::from_le_bytes([b.dma[0][5], b.dma[0][6]]), payload);
            assert_eq!(b.hdma_line[0], [0x81, 2, 1, 0, 0][row]);
            assert_eq!(b.hdma_active, [1, 1, 1, 0, 0][row]);
            assert_eq!(b.apu.input[3], [0x11, 0x33, 0x55, 0x55, 0x55][row]);
            assert_eq!(b.waddr, 0x11000);
            assert_eq!(&b.wram[0x11000..0x11004], &[0xa6; 4]);
            assert_eq!(b.dma[0][2..5], [0, 1, 0x7e]);
            assert_eq!(b.dma[0][7], 0x7e);
            assert_eq!(b.io[12], 1);
            assert_eq!(b.clocks, clocks);
        }
    }
}

#[test]
fn hdma_wram_conflict_uses_payload_bank_not_table_bank() {
    for (table, payload_bank, expected, pointer) in [
        (0x7e0100, 0x00, [0x5a, 0x37, 0xa6], 0x11002), // WRAM table, ROM payload.
        (0x008100, 0x7f, [0xa6, 0xa6, 0xa6], 0x11000), // ROM table, WRAM payload.
    ] {
        let mut b = bus();
        let bytes = [0x82, 0, 0x82, 0];
        b.wram[0x100..0x104].copy_from_slice(&bytes);
        b.cart.rom[0x100..0x104].copy_from_slice(&bytes);
        // Explicit terminating indirect high byte; independent of smoke code.
        b.cart.rom[0x104] = 0xa9;
        b.cart.rom[0x200..0x202].copy_from_slice(&[0x5a, 0x37]);
        b.wram[0x18200..0x18202].copy_from_slice(&[0x5a, 0x37]);
        b.waddr = 0x11000;
        b.wram[0x11000..0x11003].fill(0xa6);
        hdma(&mut b, 0x40, table);
        b.dma[0][1] = 0x80;
        b.dma[0][7] = payload_bank;
        for (row, (end, payload, line, active, clocks)) in [
            (0x103, 0x8201, 0x81, 1, 32),
            (0x105, if table == 0x008100 { 0xa900 } else { 0 }, 0, 0, 56),
            (0x105, if table == 0x008100 { 0xa900 } else { 0 }, 0, 0, 56),
        ]
        .into_iter()
        .enumerate()
        {
            b.hdma_pending = true;
            b.events();
            assert_eq!(
                u16::from_le_bytes([b.dma[0][8], b.dma[0][9]]),
                if table == 0x008100 { end + 0x8000 } else { end }
            );
            assert_eq!(u16::from_le_bytes([b.dma[0][5], b.dma[0][6]]), payload);
            assert_eq!(
                (b.hdma_line[0], b.hdma_active, b.clocks),
                (line, active, clocks)
            );
            assert_eq!(b.wram[0x11002], 0xa6);
            if row == 2 {
                assert_eq!(&b.wram[0x11000..0x11003], &expected);
                assert_eq!(b.waddr, pointer);
            }
            assert_eq!(b.dma[0][4], (table >> 16) as u8);
            assert_eq!(b.dma[0][7], payload_bank);
        }
    }
}

#[test]
fn cpu_wmdata_reads_writes_and_pointer_wrap_remain_legal() {
    let mut b = bus();
    b.write(0x2181, 0xff);
    b.write(0x2182, 0xff);
    b.write(0x2183, 1);
    b.write(0x2180, 0x5a);
    b.write(0x2180, 0x37);
    assert_eq!((b.wram[0x1ffff], b.wram[0], b.waddr), (0x5a, 0x37, 1));
    b.write(0x2181, 0xff);
    b.write(0x2182, 0xff);
    b.write(0x2183, 1);
    assert_eq!(b.read(0x2180), 0x5a);
    assert_eq!(b.read(0x2180), 0x37);
    assert_eq!(b.waddr, 1);
}

// Issue #27: literal pointer expectations never call the production predicate.
fn wram_address_register_reproduction(port: u8, pointer: usize, value: u8) {
    let mut b = bus();
    b.wram[0x100] = value;
    b.wram[0x12345..0x12348].copy_from_slice(&[0xa6, 0x37, 0xc9]);
    b.waddr = pointer;
    let before = b.wram.clone();
    general(&mut b, 0, port, 0x7e0100, 1);
    assert_eq!(b.waddr, pointer, "BBAD {port:02x}");
    assert_eq!(b.wram, before);
    assert_eq!(b.open, value); // The discarded write still reads its source.
    assert_eq!(b.dma[0][2..7], [1, 1, 0x7e, 0, 0]);
    assert_eq!((b.dma_pending, b.clocks), (0, 24));
}

#[test]
fn dma_wram_address_low_reproduction() {
    wram_address_register_reproduction(0x81, 0x12345, 0x5a);
}

#[test]
fn dma_wram_address_middle_reproduction() {
    wram_address_register_reproduction(0x82, 0x12345, 0x5a);
}

#[test]
fn dma_wram_address_high_clear_reproduction() {
    wram_address_register_reproduction(0x83, 0x12345, 0x5a);
}

#[test]
fn dma_wram_address_high_set_reproduction() {
    wram_address_register_reproduction(0x83, 0x02345, 0xff);
}

#[test]
fn dma_wram_address_full_banks_mirrors_and_fixed_sources() {
    for a in [
        0x7e0000, 0x7e1fff, 0x7e2000, 0x7effff, 0x7f0000, 0x7f1fff, 0x7f2000, 0x7fffff, 0x000000,
        0x001fff, 0x3f0000, 0x3f1fff, 0x800000, 0x801fff, 0xbf0000, 0xbf1fff,
    ] {
        for (port, pointer, value) in [
            (0x81, 0x12345, 0x5a),
            (0x82, 0x12345, 0x5a),
            (0x83, 0x12345, 0xfe),
            (0x83, 0x02345, 0xff),
        ] {
            let mut b = bus();
            b.wram.fill(value);
            b.wram[0x12345..0x12348].copy_from_slice(&[0xa6, 0x37, 0xc9]);
            b.waddr = pointer;
            let before = b.wram.clone();
            general(&mut b, 8, port, a, 3);
            assert_eq!(b.waddr, pointer, "{a:06x} -> 21{port:02x}");
            assert_eq!(b.wram, before);
            assert_eq!(b.open, value);
            assert_eq!(u16::from_le_bytes([b.dma[0][2], b.dma[0][3]]), a as u16);
            assert_eq!(b.dma[0][4..7], [(a >> 16) as u8, 0, 0]);
            assert_eq!((b.dma_pending, b.clocks), (0, 40));
        }
    }
}

#[test]
fn dma_wram_address_legal_cpu_rom_sram_and_unmapped_controls() {
    for (port, value, expected) in [
        (0x81, 0x5a, 0x1235a),
        (0x82, 0x5a, 0x15a45),
        (0x83, 0xfe, 0x02345),
        (0x83, 0xff, 0x12345),
    ] {
        let initial = if value == 0xff { 0x02345 } else { 0x12345 };
        let mut b = bus();
        b.waddr = initial;
        b.write(0x2100 | u32::from(port), value);
        assert_eq!(b.waddr, expected);
        for a in [
            0x400000, 0x401fff, 0x402000, 0xc00000, 0xc01fff, 0xc02000, 0x008000, 0x808000,
            0x700100,
        ] {
            let mut b = bus();
            b.cart.rom.fill(value);
            b.cart.sram = vec![value; 0x8000];
            b.waddr = initial;
            let before = b.wram.clone();
            general(&mut b, 0, port, a, 1);
            assert_eq!(b.waddr, expected, "{a:06x} -> 21{port:02x}");
            assert_eq!(b.wram, before);
            assert_eq!(b.open, value);
            assert_eq!(
                u16::from_le_bytes([b.dma[0][2], b.dma[0][3]]),
                (a as u16) + 1
            );
            assert_eq!(b.dma[0][4..7], [(a >> 16) as u8, 0, 0]);
            assert_eq!((b.dma_pending, b.clocks), (0, 24));
        }
        for a in [0x002000, 0x3f2000, 0x802000, 0xbf2000] {
            // Compare the routing side effect to an ordinary write of the
            // value actually read. No literal electrical/open-bus expectation.
            let mut b = bus();
            b.waddr = initial;
            b.open = value;
            let observed = b.read_raw(a);
            let mut cpu = bus();
            cpu.waddr = initial;
            cpu.write_raw(0x2100 | u32::from(port), observed);
            let before = b.wram.clone();
            general(&mut b, 8, port, a, 1);
            assert_eq!(b.waddr, cpu.waddr);
            assert_ne!(b.waddr, initial);
            assert_eq!(b.wram, before);
            assert_eq!(b.dma[0][2..7], [0, 0x20, (a >> 16) as u8, 0, 0]);
            assert_eq!((b.dma_pending, b.clocks), (0, 24));
        }
    }
}

#[test]
fn dma_wram_address_progression_reclassifies_each_byte_and_wraps() {
    for (mode, a, end, legal) in [
        (0, 0x001ffe, 0x2002, true),
        (16, 0x3f2001, 0x1ffd, true),
        (0, 0x80fffe, 2, true),
        (16, 0xbf0001, 0xfffd, true),
        (0, 0x7efffe, 2, false),
        (16, 0x7f0001, 0xfffd, false),
    ] {
        for (port, expected) in [(0x81, 0x0236b), (0x82, 0x06b45), (0x83, 0x12345)] {
            let mut b = bus();
            b.cart.rom.fill(0x6b);
            b.wram.fill(0x5a);
            b.open = 0x6b;
            b.waddr = 0x02345;
            let before = b.wram.clone();
            general(&mut b, mode, port, a, 4);
            // Unmapped controls assert routing using a separately observed
            // fixture read; their electrical value is outside this contract.
            let expected = if !legal {
                0x02345
            } else if a == 0x001ffe || a == 0x3f2001 {
                let mut control = bus();
                control.open = if a == 0x001ffe { 0x5a } else { 0x6b };
                let observed = control.read_raw(0x3f2001);
                control.waddr = 0x02345;
                control.write_raw(0x2100 | u32::from(port), observed);
                control.waddr
            } else {
                expected
            };
            assert_eq!(b.waddr, expected, "{a:06x} -> 21{port:02x}");
            assert_eq!(b.wram, before);
            assert_eq!(u16::from_le_bytes([b.dma[0][2], b.dma[0][3]]), end);
            assert_eq!(b.dma[0][4..7], [(a >> 16) as u8, 0, 0]);
            assert_eq!((b.dma_pending, b.clocks), (0, 48));
        }
    }
}

#[test]
fn dma_wram_address_all_modes_effective_ports_and_bbad_wrap() {
    // Eight payloads exercise repeated patterns. Expected legal ROM pointer
    // results are literal, independently worked from the documented modes.
    for (mode, expected80, expected81) in [
        (0, 0x1234d, 0x123fe),
        (1, 0x123fe, 0x1fe6b),
        (2, 0x1234d, 0x123fe),
        (3, 0x124fe, 0x1fe5a),
        (4, 0x06b5a, 0x15a11),
        (5, 0x123fe, 0x1fe6b),
        (6, 0x1234d, 0x123fe),
        (7, 0x124fe, 0x1fe5a),
    ] {
        for (a, conflicting) in [(0x7e0100, true), (0x008100, false)] {
            for (port, expected) in [(0x80, expected80), (0x81, expected81), (0x82, 0), (0x83, 0)] {
                if !conflicting && port >= 0x82 {
                    continue;
                }
                let mut b = bus();
                let payload = [0x11, 0x5a, 0x6b, 0xfe, 0x11, 0x5a, 0x6b, 0xfe];
                b.wram[0x100..0x108].copy_from_slice(&payload);
                b.cart.rom[0x100..0x108].copy_from_slice(&payload);
                b.waddr = 0x12345;
                let before = b.wram.clone();
                general(&mut b, mode, port, a, 8);
                assert_eq!(
                    b.waddr,
                    if conflicting { 0x12345 } else { expected },
                    "mode {mode} port {port:02x}"
                );
                if conflicting {
                    assert_eq!(b.wram, before);
                }
                assert_eq!(b.open, 0xfe);
                assert_eq!(
                    b.dma[0][2..7],
                    [8, if conflicting { 1 } else { 0x81 }, (a >> 16) as u8, 0, 0]
                );
                assert_eq!((b.dma_pending, b.clocks), (0, 80));
            }
        }
    }
    let mut b = bus();
    b.wram[0x100..0x108].copy_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]);
    b.waddr = 0x12345;
    let before = b.wram.clone();
    // Mode 4 at $217f: each ordinary APU write is followed by three conflicts.
    general(&mut b, 4, 0x7f, 0x7e0100, 8);
    assert_eq!(b.apu.input[3], 0x55);
    assert_eq!(b.waddr, 0x12345);
    assert_eq!(b.wram, before);
    assert_eq!(b.dma[0][2..7], [8, 1, 0x7e, 0, 0]);
    assert_eq!((b.dma_pending, b.clocks), (0, 80));
    // Mode 4 at $21ff wraps into $2100/$2101/$2102, never $2200.
    b.wram[0x100..0x104].copy_from_slice(&[0x11, 0x8f, 0x03, 0x25]);
    general(&mut b, 4, 0xff, 0x7e0100, 4);
    assert_eq!(&b.ppu.regs[..3], &[0x8f, 0x03, 0x25]);
    assert_eq!(b.waddr, 0x12345);
    assert_eq!(b.dma[0][2..7], [4, 1, 0x7e, 0, 0]);
    assert_eq!((b.dma_pending, b.clocks), (0, 128));
}

#[test]
fn hdma_wram_address_complete_tables_and_payload_bank_controls() {
    for (port, pointer, legal_pointers) in [
        (0x81, 0x12345, [0x1235a, 0x12337, 0x123fe]),
        (0x82, 0x12345, [0x15a45, 0x13745, 0x1fe45]),
        (0x83, 0x12345, [0x02345, 0x12345, 0x02345]),
        (0x83, 0x02345, [0x02345, 0x12345, 0x02345]),
    ] {
        for (indirect, table, bank, conflict) in [
            (false, 0x7e0100, 0, true),
            (false, 0x008100, 0, false),
            (true, 0x7e0100, 0x7e, true),
            (true, 0x7e0100, 0x00, false),
            (true, 0x008100, 0x7f, true),
        ] {
            let mut b = bus();
            let bytes: &[u8] = if indirect {
                &[0x82, 0, 0x82, 2, 2, 0x82, 0]
            } else {
                &[0x82, 0x5a, 0x37, 2, 0xfe, 0]
            };
            b.wram[0x100..0x100 + bytes.len()].copy_from_slice(bytes);
            b.cart.rom[0x100..0x100 + bytes.len()].copy_from_slice(bytes);
            if indirect {
                // Explicit terminating high byte; independent of smoke code.
                b.cart.rom[0x107] = 0x22;
            }
            b.wram[0x8200..0x8203].copy_from_slice(&[0x5a, 0x37, 0xfe]);
            b.wram[0x18200..0x18203].copy_from_slice(&[0x5a, 0x37, 0xfe]);
            b.cart.rom[0x200..0x203].copy_from_slice(&[0x5a, 0x37, 0xfe]);
            b.wram[0x12345..0x12348].copy_from_slice(&[0xa6, 0x37, 0xc9]);
            b.waddr = pointer;
            let before = b.wram.clone();
            hdma(&mut b, if indirect { 0x40 } else { 0 }, table);
            b.dma[0][1] = port;
            b.dma[0][7] = bank;
            let states = if indirect {
                [
                    (0x103, 0x8201, 32),
                    (0x106, 0x8202, 64),
                    (0x106, 0x8203, 72),
                    (0x108, if table == 0x008100 { 0x2200 } else { 0 }, 88),
                    (0x108, if table == 0x008100 { 0x2200 } else { 0 }, 88),
                ]
            } else {
                [
                    (0x102, 0, 16),
                    (0x104, 0, 32),
                    (0x105, 0, 40),
                    (0x106, 0, 48),
                    (0x106, 0, 48),
                ]
            };
            for (row, (end, payload, clocks)) in states.into_iter().enumerate() {
                b.hdma_pending = true;
                b.events();
                assert_eq!(
                    b.waddr,
                    if conflict {
                        pointer
                    } else {
                        legal_pointers[row.min(2)]
                    },
                    "port {port:02x} table {table:06x} bank {bank:02x} row {row}"
                );
                assert_eq!(b.wram, before);
                assert_eq!(
                    u16::from_le_bytes([b.dma[0][8], b.dma[0][9]]),
                    end + if table == 0x008100 { 0x8000 } else { 0 }
                );
                assert_eq!(u16::from_le_bytes([b.dma[0][5], b.dma[0][6]]), payload);
                assert_eq!(b.hdma_line[0], [0x81, 2, 1, 0, 0][row]);
                assert_eq!(b.dma[0][10], [0x81, 2, 1, 0, 0][row]);
                assert_eq!(b.hdma_transfer[0], [true, true, false, true, true][row]);
                assert_eq!(b.hdma_active, [1, 1, 1, 0, 0][row]);
                assert_eq!(b.io[12], 1);
                assert_eq!(
                    b.dma[0][2..5],
                    [
                        0,
                        if table == 0x008100 { 0x81 } else { 1 },
                        (table >> 16) as u8
                    ]
                );
                assert_eq!(b.dma[0][7], bank);
                if row < 3 {
                    assert_eq!(b.open, [0x5a, 0x37, 0xfe][row]);
                }
                assert_eq!(b.clocks, clocks);
            }
        }
    }
}

// Issue #29: hardware-derived side effects. No conflict byte is asserted here.
#[test]
fn dma_reverse_wmdata_conflict_reproduction() {
    let mut b = bus();
    b.wram[0x100] = 0x5a;
    b.wram[0x11000] = 0xa6;
    b.waddr = 0x11000;
    general(&mut b, 0x80, 0x80, 0x7e0100, 1);
    // Preserve baseline observations separately from the pointer contract.
    eprintln!(
        "reverse reproduction: destination={:02x}, pointer={:05x}",
        b.wram[0x100], b.waddr
    );
    assert_eq!(b.waddr, 0x11000);
    assert_eq!(b.wram[0x11000], 0xa6);
    assert_eq!(b.dma[0][2..7], [1, 1, 0x7e, 0, 0]);
    assert_eq!((b.dma_pending, b.clocks), (0, 24));
}

#[test]
fn dma_reverse_wmdata_full_banks_and_mirror_boundaries() {
    for a in [
        0x7e0000, 0x7e1fff, 0x7e2000, 0x7effff, 0x7f0000, 0x7f1fff, 0x7f2000, 0x7fffff, 0x000000,
        0x001fff, 0x3f0000, 0x3f1fff, 0x800000, 0x801fff, 0xbf0000, 0xbf1fff,
    ] {
        for pointer in [0x02345, 0x12345, 0x1ffff] {
            let mut b = bus();
            b.waddr = pointer;
            b.wram[pointer] = 0xa6;
            general(&mut b, 0x88, 0x80, a, 3);
            assert_eq!(b.waddr, pointer, "{a:06x}");
            // A writes may alias the WMDATA-addressed byte itself.
            if !(a == 0x7fffff && pointer == 0x1ffff) {
                assert_eq!(b.wram[pointer], 0xa6);
            }
            assert_eq!(u16::from_le_bytes([b.dma[0][2], b.dma[0][3]]), a as u16);
            assert_eq!(b.dma[0][4..7], [(a >> 16) as u8, 0, 0]);
            assert_eq!((b.dma_pending, b.clocks), (0, 40));
        }
    }
}

#[test]
fn dma_reverse_wmdata_progression_and_non_wram_controls() {
    // Independently count only non-WRAM A destinations as real WMDATA reads.
    for (mode, a, end, reads) in [
        (0x80, 0x001ffe, 0x2002, 2),
        (0x90, 0x3f2001, 0x1ffd, 2),
        (0x80, 0x80fffe, 2, 2),
        (0x90, 0xbf0001, 0xfffd, 2),
        (0x80, 0x7efffe, 2, 0),
        (0x90, 0x7f0001, 0xfffd, 0),
        (0x88, 0x7e0100, 0x0100, 0),
    ] {
        let mut b = bus();
        b.waddr = 0x11000;
        b.wram[0x11000..0x11005].copy_from_slice(&[0xa6, 0x37, 0xc9, 0xd8, 0xe5]);
        general(&mut b, mode, 0x80, a, 4);
        assert_eq!(b.waddr, 0x11000 + reads, "{a:06x}");
        assert_eq!(&b.wram[0x11000..0x11005], &[0xa6, 0x37, 0xc9, 0xd8, 0xe5]);
        assert_eq!(u16::from_le_bytes([b.dma[0][2], b.dma[0][3]]), end);
        assert_eq!(b.dma[0][4..7], [(a >> 16) as u8, 0, 0]);
        assert_eq!((b.dma_pending, b.clocks), (0, 48));
    }
    for a in [
        0x002000, 0x3f2000, 0x802000, 0xbf2000, 0x400000, 0xc00000, 0x008000, 0x700100,
    ] {
        let mut b = bus();
        b.cart.sram = vec![0x6d; 0x8000];
        let rom = b.cart.rom.clone();
        b.wram[0x1ffff] = 0x5a;
        b.wram[0] = 0x37;
        b.waddr = 0x1ffff;
        general(&mut b, 0x80, 0x80, a, 2);
        assert_eq!(b.waddr, 1, "{a:06x}");
        assert_eq!(b.cart.rom, rom);
        assert_eq!(
            b.dma[0][2..7],
            [
                (a as u8).wrapping_add(2),
                (a >> 8) as u8,
                (a >> 16) as u8,
                0,
                0
            ]
        );
        if a == 0x700100 {
            assert_eq!(&b.cart.sram[0x100..0x103], &[0x5a, 0x37, 0x6d]);
        }
        assert_eq!(b.clocks, 32);
    }
}

#[test]
fn dma_reverse_wmdata_zero_count_progresses_without_port_reads() {
    for (mode, a) in [(0x80, 0x7e0100), (0x88, 0x7f0100), (0x90, 0x7f0100)] {
        let mut b = bus();
        b.waddr = 0x11000;
        general(&mut b, mode, 0x80, a, 0);
        assert_eq!(b.waddr, 0x11000);
        assert_eq!(b.dma[0][2..7], [0, 1, (a >> 16) as u8, 0, 0]);
        assert_eq!(b.dma_pending, 0);
        // Retained emulator timing, including 396 refresh holds; not hardware alignment evidence.
        assert_eq!(b.clocks, 540144);
    }
}

#[test]
fn dma_reverse_wmdata_effective_patterns_and_bbad_wrap() {
    // Modes 0/2/6 use only WMDATA. Others mix APU mirrors with WMDATA;
    // mode 4 begins at $217d, avoiding out-of-scope $2181-$2183 reads.
    for (mode, port, count, legal) in [
        (0, 0x80, 4, vec![]),
        (1, 0x7f, 4, vec![(0, 0x44), (2, 0x44)]),
        (2, 0x80, 4, vec![]),
        (3, 0x7f, 4, vec![(0, 0x44), (1, 0x44)]),
        (4, 0x7d, 4, vec![(0, 0x22), (1, 0x33), (2, 0x44)]),
        (5, 0x7f, 4, vec![(0, 0x44), (2, 0x44)]),
        (6, 0x80, 4, vec![]),
        (7, 0x7f, 4, vec![(0, 0x44), (1, 0x44)]),
    ] {
        let mut b = bus();
        b.apu.output = [0x11, 0x22, 0x33, 0x44];
        b.wram[0x11000] = 0xa6;
        b.waddr = 0x11000;
        general(&mut b, 0x80 | mode, port, 0x7e0100, count);
        assert_eq!(b.waddr, 0x11000, "mode {mode}");
        assert_eq!(b.wram[0x11000], 0xa6);
        for (offset, value) in legal {
            assert_eq!(b.wram[0x100 + offset], value, "mode {mode}");
        }
        assert_eq!(b.dma[0][2..7], [4, 1, 0x7e, 0, 0]);
        assert_eq!(b.clocks, 48);
    }
    let mut b = bus();
    // $21ff -> $2100, not $2200; neither read is WMDATA.
    b.open = 0x3c;
    b.waddr = 0x11000;
    general(&mut b, 0x81, 0xff, 0x7e0100, 2);
    assert_eq!(b.waddr, 0x11000);
    assert_eq!(b.dma[0][2..7], [2, 1, 0x7e, 0, 0]);
    assert_eq!(b.clocks, 32);
}

#[test]
fn hdma_reverse_wmdata_complete_direct_and_indirect_tables() {
    for indirect in [false, true] {
        for table_bank in [0x7e, 0x00] {
            let mut b = bus();
            let bytes: &[u8] = if indirect {
                &[0x82, 0, 2, 2, 4, 2, 0]
            } else {
                &[0x82, 0x5a, 0x5a, 0x5a, 0x5a, 2, 0x5a, 0x5a, 0]
            };
            b.wram[0x100..0x100 + bytes.len()].copy_from_slice(bytes);
            b.cart.rom[0x100..0x100 + bytes.len()].copy_from_slice(bytes);
            if indirect {
                // Explicit terminating high byte; independent of smoke code.
                b.cart.rom[0x107] = 0x22;
            }
            b.apu.output[3] = 0x44;
            b.wram[0x11000] = 0xa6;
            b.waddr = 0x11000;
            let table = if table_bank == 0x7e {
                0x7e0100
            } else {
                0x008100
            };
            hdma(&mut b, if indirect { 0xc1 } else { 0x81 }, table);
            b.dma[0][1] = 0x7f;
            b.dma[0][7] = 0x7f;
            let states = if indirect {
                [
                    (3, 0x202, 40),
                    (6, 0x204, 80),
                    (6, 0x206, 96),
                    (8, if table_bank == 0 { 0x2200 } else { 0 }, 112),
                    (8, if table_bank == 0 { 0x2200 } else { 0 }, 112),
                ]
            } else {
                [(3, 0, 24), (6, 0, 48), (8, 0, 64), (9, 0, 72), (9, 0, 72)]
            };
            for (row, (advance, payload, clocks)) in states.into_iter().enumerate() {
                b.hdma_pending = true;
                b.events();
                assert_eq!(
                    u16::from_le_bytes([b.dma[0][8], b.dma[0][9]]),
                    table as u16 + advance
                );
                assert_eq!(u16::from_le_bytes([b.dma[0][5], b.dma[0][6]]), payload);
                assert_eq!(b.hdma_line[0], [0x81, 2, 1, 0, 0][row]);
                assert_eq!(b.dma[0][10], [0x81, 2, 1, 0, 0][row]);
                assert_eq!(b.hdma_transfer[0], [true, true, false, true, true][row]);
                assert_eq!(b.hdma_active, [1, 1, 1, 0, 0][row]);
                assert_eq!(
                    (b.waddr, b.wram[0x11000]),
                    (
                        0x11000
                            + if !indirect && table_bank == 0 {
                                [1, 2, 3, 3, 3][row]
                            } else {
                                0
                            },
                        0xa6
                    )
                );
                assert_eq!(
                    b.dma[0][2..5],
                    [table as u8, (table >> 8) as u8, table_bank]
                );
                assert_eq!((b.dma[0][7], b.io[12], b.clocks), (0x7f, 1, clocks));
            }
            // Descriptors/indirect-pointer bytes are never reverse payload writes.
            if indirect {
                assert_eq!(&b.wram[0x100..0x107], bytes);
                assert_eq!(
                    [b.wram[0x10200], b.wram[0x10202], b.wram[0x10204]],
                    [0x44; 3]
                );
            } else if table_bank == 0x7e {
                assert_eq!([b.wram[0x100], b.wram[0x105], b.wram[0x108]], [0x82, 2, 0]);
                assert_eq!([b.wram[0x101], b.wram[0x103], b.wram[0x106]], [0x44; 3]);
            }
        }
    }
}

#[test]
fn hdma_reverse_wmdata_indirect_payload_bank_controls() {
    for (table, bank, pointer) in [
        (0x7e0100, 0x70, 0x11003),
        (0x7e0100, 0x00, 0x11003),
        (0x008100, 0x7f, 0x11000),
    ] {
        let mut b = bus();
        // LoROM SRAM is mapped below $8000; ROM controls use $8200.
        let high = if bank == 0x70 { 2 } else { 0x82 };
        let bytes = [0x82, 0, high, 2, 2, high, 0];
        b.wram[0x100..0x107].copy_from_slice(&bytes);
        b.cart.rom[0x100..0x107].copy_from_slice(&bytes);
        // Explicit terminating indirect high byte; independent of smoke code.
        b.cart.rom[0x107] = 0x22;
        let rom = b.cart.rom.clone();
        b.cart.sram = vec![0x5a; 0x8000];
        b.wram[0x11000..0x11004].copy_from_slice(&[0xa6, 0x37, 0xc9, 0xd8]);
        b.waddr = 0x11000;
        hdma(&mut b, 0xc0, table);
        b.dma[0][1] = 0x80;
        b.dma[0][7] = bank;
        for (advance, payload, line, active, clocks) in [
            (3, 0x8201, 0x81, 1, 32),
            (6, 0x8202, 2, 1, 64),
            (6, 0x8203, 1, 1, 72),
            (8, if table == 0x008100 { 0x2200 } else { 0 }, 0, 0, 88),
            (8, if table == 0x008100 { 0x2200 } else { 0 }, 0, 0, 88),
        ] {
            b.hdma_pending = true;
            b.events();
            assert_eq!(
                u16::from_le_bytes([b.dma[0][8], b.dma[0][9]]),
                table as u16 + advance
            );
            assert_eq!(
                u16::from_le_bytes([b.dma[0][5], b.dma[0][6]]),
                if bank == 0x70 && active != 0 {
                    payload - 0x8000
                } else {
                    payload
                }
            );
            assert_eq!(
                (b.hdma_line[0], b.hdma_active, b.clocks),
                (line, active, clocks)
            );
            assert_eq!(
                (b.dma[0][4], b.dma[0][7], b.io[12]),
                ((table >> 16) as u8, bank, 1)
            );
        }
        assert_eq!(b.waddr, pointer);
        assert_eq!(&b.wram[0x11000..0x11004], &[0xa6, 0x37, 0xc9, 0xd8]);
        assert_eq!(b.cart.rom, rom);
        assert_eq!(&b.wram[0x100..0x107], &bytes);
        if bank == 0x70 {
            assert_eq!(&b.cart.sram[0x200..0x204], &[0xa6, 0x37, 0xc9, 0x5a]);
        }
    }
}

// Deterministic emulator approximation ONLY. These byte values are not hardware
// expectations: retain the previous SystemBus::open byte on a conflicting read.
#[test]
fn dma_reverse_wmdata_approximation_writes_previous_bus_byte() {
    for mode in [0x80, 0x88, 0x90] {
        let mut b = bus();
        b.wram.fill(0x5a);
        b.wram[0x11000] = 0xa6;
        b.waddr = 0x11000;
        b.open = 0x3c;
        let mut expected = b.wram.clone();
        for index in if mode == 0x88 {
            vec![0x100]
        } else if mode == 0x90 {
            vec![0x100, 0xff, 0xfe]
        } else {
            vec![0x100, 0x101, 0x102]
        } {
            expected[index] = 0x3c;
        }
        general(&mut b, mode, 0x80, 0x7e0100, 3);
        assert!(b.wram == expected); // A writes occur, no ordinary copy from $11000.
        assert_eq!(b.open, 0x3c);
    }
    let mut b = bus();
    b.apu.output[3] = 0x44;
    b.waddr = 0x11000;
    b.wram[0x11000] = 0xa6;
    b.open = 0x3c;
    general(&mut b, 0x81, 0x7f, 0x7e0100, 4);
    assert_eq!(&b.wram[0x100..0x104], &[0x44; 4]);
}

#[test]
fn hdma_reverse_wmdata_approximation_writes_payload_destinations_only() {
    for indirect in [false, true] {
        let mut b = bus();
        let table: &[u8] = if indirect {
            &[0x82, 0, 2, 0]
        } else {
            &[0x82, 0x5a, 0x5a, 0]
        };
        b.wram.fill(0x5a);
        b.wram[0x100..0x104].copy_from_slice(table);
        b.wram[0x11000] = 0xa6;
        let mut expected = b.wram.clone();
        if indirect {
            expected[0x200..0x202].fill(0x3c);
        } else {
            expected[0x101..0x103].fill(0x3c);
        }
        b.waddr = 0x11000;
        b.open = 0x3c;
        hdma(&mut b, if indirect { 0xc0 } else { 0x80 }, 0x7e0100);
        b.dma[0][1] = 0x80;
        b.dma[0][7] = 0x7e;
        for _ in 0..3 {
            b.hdma_pending = true;
            b.events();
        }
        // The emulator's hdma_read currently leaves open unchanged. This literal
        // is solely the documented emulator convention.
        assert!(b.wram == expected);
        assert_eq!(b.hdma_active, 0);
    }
}

#[test]
fn hdma_reverse_wmdata_reclassifies_each_indirect_destination_and_wraps() {
    for (bank, start, ends, reads) in [
        (0x00, 0x1fff_u16, [0x2000, 0x2001], [0, 1]),
        (0xbf, 0xffff, [0, 1], [1, 1]),
        (0x7e, 0xffff, [0, 1], [0, 0]),
    ] {
        let mut b = bus();
        b.wram[0x100..0x104].copy_from_slice(&[0x82, start as u8, (start >> 8) as u8, 0]);
        hdma(&mut b, 0xd8, 0x7f0100); // DMA fixed/decrement flags do not affect HDMA.
        b.wram[0x10100..0x10104].copy_from_slice(&[0x82, start as u8, (start >> 8) as u8, 0]);
        b.dma[0][1] = 0x80;
        b.dma[0][7] = bank;
        b.waddr = 0x11000;
        for row in 0..2 {
            b.hdma_pending = true;
            b.events();
            assert_eq!(
                u16::from_le_bytes([b.dma[0][5], b.dma[0][6]]),
                if row == 0 { ends[row] } else { 0 }
            );
            assert_eq!(b.waddr, 0x11000 + reads[row]);
            assert_eq!(b.dma[0][8..10], [if row == 0 { 3 } else { 5 }, 1]);
            assert_eq!(b.hdma_line[0], [0x81, 0][row]);
            assert_eq!(
                (b.hdma_active, b.dma[0][7]),
                (if row == 0 { 1 } else { 0 }, bank)
            );
        }
        b.hdma_pending = true;
        b.events();
        assert_eq!(b.dma[0][8..10], [5, 1]);
        assert_eq!((b.hdma_active, b.hdma_line[0], b.io[12]), (0, 0, 1));
        assert_eq!(b.dma[0][2..5], [0, 1, 0x7f]);
    }
}
