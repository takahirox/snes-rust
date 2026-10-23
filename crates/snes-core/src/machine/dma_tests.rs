//! Original MIT DMA/HDMA address and side-effect vectors for Issue #21.
use super::*;

fn bus() -> SystemBus {
    SystemBus::new(Cartridge::load(include_bytes!("../../../../tests/roms/smoke.bin")).unwrap())
}

// Literal expectations, independently transcribed from the Issue's contract.
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
    (0x4210, true),
    (0x421f, true),
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
    for a in [0x2180, 0x4016, 0x4210, 0x4211] {
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
            0x4210 => {
                assert_eq!(b.read(a) & 128, 128);
                assert!(!b.nmi_flag);
            }
            0x4211 => {
                assert_eq!(b.read(a) & 128, 128);
                assert!(!b.irq);
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
        for a in [0x2180, 0x4016, 0x4210, 0x4211, 0x4300] {
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
            for a in [0x2180u16, 0x4016, 0x4210, 0x4211, 0x4300] {
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
    for a in [0x2180u16, 0x4016, 0x4210, 0x4211] {
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
        for a in [0x2180u16, 0x4016, 0x4210, 0x4211, 0x4300] {
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
