//! Original MIT DMA/HDMA address and side-effect vectors for Issues #21/#23.
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
    for a in [0x2180, 0x4016] {
        let mut b = bus();
        b.wram[0x1000..0x1002].copy_from_slice(&[0x5a, 0x12]);
        b.waddr = 0x1000;
        b.serial = 0x9180;
        b.nmi_flag = true;
        b.irq = true;
        b.open = 0xff;
        b.ppu.cgram[..2].fill(0x7f);
        general(&mut b, 8, 0x22, a, 2);
        assert_eq!(&b.ppu.cgram[..2], &[0, 0], "{a:04x}");
        assert_eq!(b.waddr, 0x1000);
        assert_eq!(b.serial, 0x9180);
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
                assert_eq!(b.serial, 0x2301);
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
        b.serial = 0x9180;
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
        assert_eq!(b.serial, 0x9180);
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
    // value, excluded addresses inject zero, and $0000 is WRAM in bank $80.
    for (mode, start, count, end, expected) in [
        (8, 0x4016, 3, 0x4016, vec![0, 0, 0]),
        (0, 0x20ff, 3, 0x2102, vec![0x5a, 0, 0]),
        (0, 0x21ff, 3, 0x2202, vec![0, 0, 0]),
        (16, 0x2200, 3, 0x21fd, vec![0x5a, 0, 0]),
        (16, 0x2100, 3, 0x20fd, vec![0, 0, 0]),
        (0, 0xffff, 2, 0x0001, vec![0x6b, 0x37]),
        (16, 0x0000, 2, 0xfffe, vec![0x37, 0x6b]),
    ] {
        let mut b = bus();
        b.cart.rom.fill(0x6b);
        b.wram[0] = 0x37;
        b.waddr = 0x10000;
        b.open = 0x5a;
        general(&mut b, mode, 0x80, 0x800000 | start, count);
        assert_eq!(&b.wram[0x10000..0x10000 + expected.len()], expected);
        assert_eq!(b.waddr, 0x10000 + expected.len());
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
        for a in [0x2180, 0x4016, 0x4300] {
            let mut b = bus();
            b.waddr = 0x1000;
            b.wram[0x1000] = 0x81;
            b.serial = 0x9180;
            b.nmi_flag = true;
            b.irq = true;
            hdma(&mut b, 0, bank << 16 | a);
            b.ppu.regs[6] = 0x55;
            b.events();
            assert_eq!(b.hdma_active, 0, "{bank:02x}:{a:04x}");
            assert_eq!(u16::from_le_bytes([b.dma[0][8], b.dma[0][9]]), a as u16 + 1);
            assert_eq!(b.waddr, 0x1000);
            assert_eq!(b.serial, 0x9180);
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
            for a in [0x2180u16, 0x4016, 0x4300] {
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
                b.serial = 0x9180;
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
                assert_eq!(b.serial, 0x9180);
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
        (0x0020ff, 1, 0x2102, 32, 1), // pointer $0000, one payload -> $0001
        (0x0042ff, 1, 0x4302, 32, 1),
        (0x80fffe, 0x0182, 0x0001, 32, 0x66), // low=$81, high=$01
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
    for a in [0x2180u16, 0x4016] {
        let mut b = bus();
        b.dma[0][4] = 0;
        b.dma[0][8..10].copy_from_slice(&a.to_le_bytes());
        b.waddr = 0x1000;
        b.serial = 0x9180;
        b.nmi_flag = true;
        b.irq = true;
        assert_eq!(b.hdma_read(0), 0);
        assert_eq!(b.waddr, 0x1000);
        assert_eq!(b.serial, 0x9180);
        assert!(b.nmi_flag && b.irq);
        assert_eq!(u16::from_le_bytes([b.dma[0][8], b.dma[0][9]]), a + 1);
        assert_eq!(b.clocks, 8);
    }
}

#[test]
fn hdma_tables_feed_blocked_payloads_and_then_terminate() {
    for reverse in [false, true] {
        for a in [0x2180u16, 0x4016, 0x4300] {
            let mut b = bus();
            b.wram[0x100..0x104].copy_from_slice(&[0x81, a as u8, (a >> 8) as u8, 0]);
            hdma(&mut b, 0x40 | if reverse { 0x80 } else { 0 }, 0x7e0100);
            b.dma[0][7] = 0; // Indirect bank is independent of table bank.
            b.dma[0][1] = if reverse { 0x3b } else { 6 };
            b.ppu.cgram[..2].copy_from_slice(&[0x35, 0x12]);
            b.ppu.regs[6] = 0x55;
            b.waddr = 0x1000;
            b.wram[0x1000] = 0x55;
            b.serial = 0x9180;
            b.nmi_flag = true;
            b.irq = true;
            b.events();
            assert_eq!(b.dma[0][8..10], [3, 1]);
            assert_eq!(u16::from_le_bytes([b.dma[0][5], b.dma[0][6]]), a + 1);
            assert_eq!(b.waddr, 0x1000);
            assert_eq!(b.serial, 0x9180);
            assert!(b.nmi_flag && b.irq);
            assert_eq!(b.ppu.regs[6], if reverse { 0x55 } else { 0 });
            if reverse {
                assert_eq!(b.ppu.read(0x3b, 0) & 0x7f, 0x12);
            }
            assert_eq!(b.clocks, 32);
            b.hdma_pending = true;
            b.events();
            assert_eq!(b.hdma_active, 0);
            assert_eq!(b.dma[0][8..10], [4, 1]);
            assert_eq!(b.clocks, 40);
        }
        let mut b = bus();
        b.open = 0x81; // Legal descriptor at $20ff, direct payload at $2100.
        hdma(&mut b, if reverse { 0x80 } else { 0 }, 0x0020ff);
        b.dma[0][1] = if reverse { 0x3b } else { 6 };
        b.ppu.regs[0] = 0x80;
        b.ppu.cgram[..2].copy_from_slice(&[0x35, 0x12]);
        b.events();
        assert_eq!(b.dma[0][8..10], [1, 0x21]);
        assert_eq!(b.ppu.regs[0], 0x80);
        assert_eq!(b.ppu.regs[6], 0);
        if reverse {
            assert_eq!(b.ppu.read(0x3b, 0) & 0x7f, 0x12);
        }
        assert_eq!(b.clocks, 16);
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
    b.tick(225 * LINE + 4224); // Real automatic first-controller completion.
    assert_eq!(b.joy, 0xa590);
    b.hdma_pending = false;
    b.v = 230;
    b.h = 1200; // Stable V/H blank, away from comparisons and refresh.
    b.auto_until = b.clocks + 1000;
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
        b.auto_until = if busy { 1000 } else { 0 };
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
            b.auto_until = 0;
            b.open = 0;
            b.io[1] = 1; // RDIO descriptor = one row.
            b.wram[usize::from(pointer)] = payload;
            hdma(&mut b, 0x40, bank << 16 | table);
            b.dma[0][7] = 0x7e;
            let before = b.clocks;
            b.events();
            assert_eq!(b.ppu.regs[6], payload);
            assert_eq!(u16::from_le_bytes([b.dma[0][5], b.dma[0][6]]), pointer + 1);
            assert_eq!(u16::from_le_bytes([b.dma[0][8], b.dma[0][9]]), end);
            assert_eq!(b.hdma_active, 1);
            if table == 0x4210 {
                assert!(!b.nmi_flag && !b.irq);
            }
            assert_eq!(b.clocks - before, 32);
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
            assert_eq!(b.hdma_active, 1);
            assert_eq!(b.dma[0][8..10], [3, 1]);
            assert_eq!(u16::from_le_bytes([b.dma[0][5], b.dma[0][6]]), a + 1);
            b.hdma_pending = true;
            b.events();
            assert_eq!(b.hdma_active, 0);
            assert_eq!(b.dma[0][8..10], [4, 1]);
            assert_eq!(b.waddr, 0x10001);
            assert_eq!(b.clocks, 40);
        }
    }
}
