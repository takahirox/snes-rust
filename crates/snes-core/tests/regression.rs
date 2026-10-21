use snes_core::{
    cartridge::{Cartridge, Mapping},
    cpu::{Bus, Cpu, C, D, I, M, N, V, X, Z},
    frame_hash, Machine,
};
struct Ram {
    data: Vec<u8>,
    reads: Vec<u32>,
    writes: Vec<(u32, u8)>,
    idle: usize,
}
impl Ram {
    fn new(code: &[u8]) -> Self {
        let mut r = Self {
            data: vec![0; 1 << 24],
            reads: Vec::new(),
            writes: Vec::new(),
            idle: 0,
        };
        r.data[0x8000..0x8000 + code.len()].copy_from_slice(code);
        r
    }
}
impl Bus for Ram {
    fn read(&mut self, a: u32) -> u8 {
        self.reads.push(a);
        self.data[a as usize]
    }
    fn write(&mut self, a: u32, v: u8) {
        self.writes.push((a, v));
        self.data[a as usize] = v;
    }
    fn idle(&mut self) {
        self.idle += 1;
    }
}
fn execute(code: &[u8], n: usize) -> (Cpu, Ram) {
    let mut ram = Ram::new(code);
    let mut cpu = Cpu {
        pc: 0x8000,
        ..Cpu::default()
    };
    for _ in 0..n {
        cpu.step(&mut ram);
    }
    (cpu, ram)
}
fn fixture() -> Vec<u8> {
    include_bytes!("../../../tests/roms/smoke.bin").to_vec()
}
#[test]
fn native_width_transitions_preserve_b_and_clear_index_high_bytes() {
    let (c, _) = execute(
        &[
            0x18, 0xfb, 0xc2, 0x30, 0xa9, 0x34, 0x12, 0xa2, 0xcd, 0xab, 0xe2, 0x30, 0xa9, 0x56,
        ],
        7,
    );
    assert!(!c.e);
    assert_eq!(c.a, 0x1256);
    assert_eq!(c.x, 0xcd);
    assert_eq!(c.p & (M | X), M | X);
}
#[test]
fn arithmetic_binary_and_decimal_carries() {
    let (c, _) = execute(&[0xa9, 0x7f, 0x18, 0x69, 1], 3);
    assert_eq!(c.a, 0x80);
    assert_eq!(c.p & (N | V | C | Z), N | V);
    let (c, _) = execute(&[0xf8, 0x18, 0xa9, 0x99, 0x69, 1, 0xe9, 1], 5);
    assert_eq!(c.a, 0x99);
    assert_eq!(c.p & (C | D), D);
    let (c, _) = execute(
        &[
            0x18, 0xfb, 0xc2, 0x20, 0xf8, 0x18, 0xa9, 0x99, 0x99, 0x69, 1, 0,
        ],
        7,
    );
    assert_eq!(c.a, 0);
    assert_ne!(c.p & C, 0);
    assert_ne!(c.p & Z, 0);
}
#[test]
fn direct_indirect_wrap_and_long_bank_carry() {
    let mut r = Ram::new(&[0xb2, 0xff, 0xaf, 0xff, 0xff, 0x7e]);
    r.data[255] = 0x00;
    r.data[0] = 0x90;
    r.data[0x9000] = 0x42;
    r.data[0x7effff] = 0x12;
    r.data[0x7f0000] = 0x34;
    let mut c = Cpu {
        pc: 0x8000,
        ..Cpu::default()
    };
    c.step(&mut r);
    assert_eq!(c.a, 0x42);
    c.e = false;
    c.p &= !M;
    c.step(&mut r);
    assert_eq!(c.a, 0x3412);
}
#[test]
fn native_bank_zero_word_operands_wrap_reads_writes_and_modifies() {
    // dp, dp,X, dp,Y and stack-relative all address $00ffff. DB must not
    // affect them, and the second byte must come from $000000, not $010000.
    for (load, store, modify, d, index, s) in [
        (0xa5, 0x85, Some(0xe6), 0xffff, 0, 0x1ff),
        (0xb5, 0x95, Some(0xf6), 0xfffe, 1, 0x1ff),
        (0xb6, 0x96, None, 0xfffe, 1, 0x1ff),
        (0xa3, 0x83, None, 0, 0, 0xffff),
    ] {
        let mut r = Ram::new(&[load, 0, store, 0, modify.unwrap_or(0xea), 0]);
        r.data[0xffff] = 0x12;
        r.data[0] = 0x34;
        r.data[0x10000] = 0x56;
        let mut c = Cpu {
            pc: 0x8000,
            e: false,
            p: 0,
            db: 0x7e,
            d,
            x: index,
            y: index,
            s,
            ..Cpu::default()
        };
        c.step(&mut r);
        assert_eq!(if load == 0xb6 { c.x } else { c.a }, 0x3412);
        assert_eq!(&r.reads[r.reads.len() - 2..], &[0xffff, 0]);
        if load == 0xb6 {
            c.x = 0xabcd;
        } else {
            c.a = 0xabcd;
        }
        c.step(&mut r);
        assert_eq!(r.writes, [(0xffff, 0xcd), (0, 0xab)]);
        if modify.is_some() {
            c.step(&mut r);
            assert_eq!(&r.reads[r.reads.len() - 2..], &[0xffff, 0]);
            assert_eq!(&r.writes[2..], &[(0xffff, 0xce), (0, 0xab)]);
        }
        assert_eq!(r.data[0x10000], 0x56);
    }
}
#[test]
fn indirect_pointers_wrap_in_bank_zero_but_resolved_data_can_carry() {
    for e in [false, true] {
        // [dp] and [dp],Y never page-wrap, even in emulation mode.
        for d in [0xff00, 0xffff] {
            for op in [0xa7, 0xb7] {
                let mut r = Ram::new(&[op, if d == 0xff00 { 0xff } else { 0 }]);
                r.data[0xffff] = 0xff;
                r.data[0] = 0xff;
                r.data[1] = 0x7e;
                r.data[0xff01] = 0x7d; // wrong emulation-page wrap sentinel
                r.data[0x10000] = 0x12; // wrong bank carry sentinel
                r.data[0x7effff] = 0x42;
                r.data[0x7f0000] = 0x43;
                let mut c = Cpu {
                    pc: 0x8000,
                    e,
                    p: M | X,
                    d,
                    y: 1,
                    ..Cpu::default()
                };
                c.step(&mut r);
                assert_eq!(c.a, if op == 0xa7 { 0x42 } else { 0x43 });
                assert_eq!(&r.reads[2..5], &[0xffff, 0, 1]);
            }
        }
    }
    // A stack-relative pointer also stays in bank zero; its resolved word
    // operand uses DB and can cross a bank boundary on both read and write.
    let mut r = Ram::new(&[0xb3, 0, 0x93, 0]);
    r.data[0xffff] = 0xff;
    r.data[0] = 0xff;
    r.data[0x10000] = 0x12;
    r.data[0x7effff] = 0x34;
    r.data[0x7f0000] = 0x56;
    let mut c = Cpu {
        pc: 0x8000,
        e: false,
        p: 0,
        s: 0xffff,
        db: 0x7e,
        ..Cpu::default()
    };
    c.step(&mut r);
    assert_eq!(c.a, 0x5634);
    assert_eq!(r.reads, [0x8000, 0x8001, 0xffff, 0, 0x7effff, 0x7f0000]);
    c.a = 0xabcd;
    c.step(&mut r);
    assert_eq!(r.writes, [(0x7effff, 0xcd), (0x7f0000, 0xab)]);
}
#[test]
fn pei_wraps_its_pointer_within_bank_zero() {
    for e in [false, true] {
        let mut r = Ram::new(&[0xd4, 0]); // PEI ($00) with D=$ffff
        r.data[0xffff] = 0x34;
        r.data[0] = 0x12;
        r.data[0x10000] = 0x56; // wrong bank-carry sentinel
        let mut c = Cpu {
            pc: 0x8000,
            e,
            d: 0xffff,
            db: 0x7e,
            ..Cpu::default()
        };
        let p = c.p;
        c.step(&mut r);
        assert_eq!(r.reads, [0x8000, 0x8001, 0xffff, 0]);
        assert_eq!(r.writes, [(0x1ff, 0x12), (0x1fe, 0x34)]);
        assert_eq!(c.s, 0x1fd);
        assert_eq!(c.pc, 0x8002);
        assert_eq!(c.p, p);
    }
}
#[test]
fn indirect_jumps_wrap_pointers_within_their_bank() {
    for e in [false, true] {
        for (op, base, index) in [
            (0x6c, 0xffffu16, 0), // JMP (absolute): bank zero
            (0xdc, 0xfffe, 0),    // JML [absolute]: bank byte wraps
            (0xdc, 0xffff, 0),    // JML [absolute]: high byte wraps
            (0x7c, 0xffff, 0),    // JMP (absolute,X): program bank
            (0xfc, 0xffff, 0),    // JSR (absolute,X): program bank
            (0x7c, 0xfffe, 1),
            (0xfc, 0xfffe, 1),
            (0x7c, 0xffff, 1), // indexing itself also wraps
            (0xfc, 0xffff, 1),
        ] {
            let mut r = Ram::new(&[]);
            r.data[0x7e8000..0x7e8003].copy_from_slice(&[op, base as u8, (base >> 8) as u8]);
            let bank = if matches!(op, 0x7c | 0xfc) {
                0x7e0000
            } else {
                0
            };
            let pointer = base.wrapping_add(index);
            let low = bank | pointer as u32;
            let high = bank | pointer.wrapping_add(1) as u32;
            r.data[low as usize] = 0x34;
            r.data[high as usize] = 0x12;
            r.data[(bank + 0x10000) as usize] = 0x56; // wrong bank-carry sentinel
            if op == 0xdc {
                r.data[pointer.wrapping_add(2) as usize] = 0x42;
            }
            let mut c = Cpu {
                pc: 0x8000,
                pb: 0x7e,
                db: 0x7f,
                e,
                x: index,
                ..Cpu::default()
            };
            c.step(&mut r);
            assert_eq!(c.pc, 0x1234, "opcode {op:02x}, base {base:04x}, E={e}");
            assert_eq!(c.pb, if op == 0xdc { 0x42 } else { 0x7e });
            assert!(r.reads.contains(&low) && r.reads.contains(&high));
            assert!(!r.reads.contains(&(bank + 0x10000)));
            if op == 0xdc {
                assert!(r.reads.contains(&(pointer.wrapping_add(2) as u32)));
            }
        }
    }
}
#[test]
fn jumps_and_subroutine_calls_have_fixed_cycle_counts() {
    // W65C816S tables 5-4 and 5-7: indexed JSR has one internal cycle,
    // just like indexed JMP, plus the two return-address writes.
    for e in [false, true] {
        for p in [M | X, 0] {
            if e && p == 0 {
                continue;
            }
            for index in [0, 1] {
                for (op, cycles, reads, writes) in [
                    (0x7c, 6, 5, 0),
                    (0xfc, 8, 5, 2),
                    (0x20, 6, 3, 2),
                    (0x22, 8, 4, 3),
                ] {
                    let mut r = Ram::new(&[]);
                    r.data[0x7e8000..0x7e8004].copy_from_slice(&[op, 0xfe, 0x90, 0x42]);
                    let pointer = 0x7e90fe + index as usize;
                    r.data[pointer..pointer + 2].copy_from_slice(&0x1234u16.to_le_bytes());
                    let mut c = Cpu {
                        pc: 0x8000,
                        pb: 0x7e,
                        e,
                        p,
                        x: index,
                        ..Cpu::default()
                    };
                    c.step(&mut r);
                    assert_eq!(
                        r.reads.len() + r.writes.len() + r.idle,
                        cycles,
                        "opcode {op:02x}, index {index}, E={e}, P={p:02x}"
                    );
                    assert_eq!((r.reads.len(), r.writes.len(), r.idle), (reads, writes, 1));
                    assert_eq!(
                        c.pc,
                        if matches!(op, 0x7c | 0xfc) {
                            0x1234
                        } else {
                            0x90fe
                        }
                    );
                    assert_eq!(c.pb, if op == 0x22 { 0x42 } else { 0x7e });
                    if writes != 0 {
                        let mut expected = Vec::new();
                        if op == 0x22 {
                            expected.push((0x1ff, 0x7e));
                        }
                        let stack = if op == 0x22 { 0x1fe } else { 0x1ff };
                        expected
                            .extend([(stack, 0x80), (stack - 1, if op == 0x22 { 3 } else { 2 })]);
                        assert_eq!(r.writes, expected);
                        assert_eq!(c.s, 0x1ff - writes as u16);
                    }
                }
            }
        }
    }
}
#[test]
fn hardware_and_software_interrupts_take_seven_or_eight_bus_cycles() {
    // W65C816S table 5-7: BRK/COP consume opcode + signature reads;
    // NMI/IRQ consume a discarded opcode read + internal cycle instead.
    for e in [true, false] {
        for kind in ["BRK", "COP", "NMI", "IRQ"] {
            let software = matches!(kind, "BRK" | "COP");
            let op = if kind == "COP" { 0x02 } else { 0x00 };
            let vector = match (kind, e) {
                ("COP", true) => 0xfff4,
                ("COP", false) => 0xffe4,
                ("NMI", true) => 0xfffa,
                ("NMI", false) => 0xffea,
                (_, true) => 0xfffe,
                ("BRK", false) => 0xffe6,
                _ => 0xffee,
            };
            let mut r = Ram::new(&[]);
            r.data[0x128000..0x128002].copy_from_slice(&[op, 0x42]);
            r.data[vector..vector + 2].copy_from_slice(&0x9000u16.to_le_bytes());
            let p = D | M | X;
            let mut c = Cpu {
                pc: 0x8000,
                pb: 0x12,
                e,
                p,
                waiting: !software,
                ..Cpu::default()
            };
            if software {
                c.step(&mut r);
            } else {
                c.interrupt(&mut r, kind == "NMI");
            }
            assert_eq!(
                r.reads.len() + r.writes.len() + r.idle,
                if e { 7 } else { 8 },
                "{kind}, E={e}"
            );
            let mut reads = vec![0x128000];
            if software {
                reads.push(0x128001);
            }
            reads.extend([vector as u32, vector as u32 + 1]);
            assert_eq!(r.reads, reads, "{kind}, E={e}");
            assert_eq!(r.idle, usize::from(!software));
            let mut writes = Vec::new();
            if !e {
                writes.push((0x1ff, 0x12));
            }
            let stack = if e { 0x1ff } else { 0x1fe };
            let stacked_p = if e {
                (p & !X) | if software { X } else { 0 }
            } else {
                p
            };
            writes.extend([
                (stack, 0x80),
                (stack - 1, if software { 2 } else { 0 }),
                (stack - 2, stacked_p),
            ]);
            assert_eq!(r.writes, writes);
            assert_eq!(c.s, if e { 0x1fc } else { 0x1fb });
            assert_eq!((c.pc, c.pb, c.p), (0x9000, 0, (p | I) & !D));
            assert!(!c.waiting);
        }
    }
    let mut r = Ram::new(&[]);
    let mut c = Cpu {
        waiting: true,
        ..Cpu::default()
    };
    c.interrupt(&mut r, false); // masked IRQ wakes WAI without entry cycles
    assert!(!c.waiting);
    c.stopped = true;
    c.interrupt(&mut r, true);
    assert!(r.reads.is_empty() && r.writes.is_empty() && r.idle == 0);
}
#[test]
fn nmi_stack_and_rti_restore_native_state() {
    let mut r = Ram::new(&[0xcb, 0xea]);
    r.data[0xffea] = 0;
    r.data[0xffeb] = 0x90;
    r.data[0x9000] = 0x40;
    let mut c = Cpu {
        pc: 0x8000,
        e: false,
        p: D,
        pb: 0,
        s: 0x1ff,
        ..Cpu::default()
    };
    c.step(&mut r);
    assert!(c.waiting);
    c.interrupt(&mut r, true);
    assert!(!c.waiting);
    assert_eq!(c.pc, 0x9000);
    assert_eq!(c.p & (D | I), I);
    c.step(&mut r);
    assert_eq!(c.pc, 0x8001);
    assert_eq!(c.s, 0x1ff);
    assert_eq!(c.p, D);
}
#[test]
fn masked_irq_wakes_wai_but_stp_needs_reset() {
    let (mut c, mut r) = execute(&[0xcb, 0xdb], 1);
    c.interrupt(&mut r, false);
    assert!(!c.waiting);
    assert_eq!(c.pc, 0x8001);
    c.step(&mut r);
    assert!(c.stopped);
    c.interrupt(&mut r, true);
    c.step(&mut r);
    assert!(c.stopped);
}
#[test]
fn every_cpu_opcode_decodes_without_panic() {
    for op in 0u8..=255 {
        let mut r = Ram::new(&[op, 0, 0, 0]);
        let mut c = Cpu {
            pc: 0x8000,
            ..Cpu::default()
        };
        c.step(&mut r);
    }
}
#[test]
fn cartridge_headers_mapping_and_sram() {
    let rom = fixture();
    let c = Cartridge::load(&rom).unwrap();
    assert_eq!(c.mapping, Mapping::LoRom);
    assert_eq!(c.read(0x008000), c.read(0x808000));
    let mut copy = vec![0; 512];
    copy.extend_from_slice(&rom);
    assert_eq!(Cartridge::load(&copy).unwrap().rom, rom);
    let mut c = c;
    c.write(0x700000, 0x42);
    assert_eq!(c.read(0xf00000), Some(0x42));
    let mut hi = vec![0; 65536];
    hi[0xffc0..].copy_from_slice(&rom[0x7fc0..]);
    hi[0xffd5] = 0x21;
    let c = Cartridge::load(&hi).unwrap();
    assert_eq!(c.mapping, Mapping::HiRom);
    assert_eq!(c.read(0xc08000), c.read(0x008000));
    assert!(Cartridge::load(&[]).is_err());
    let mut bad = rom.clone();
    bad[0x7fd6] = 0x13;
    assert!(Cartridge::load(&bad).is_err());
    bad = rom;
    bad[0x7fd9] = 2;
    assert!(Cartridge::load(&bad).is_err());
}
#[test]
fn memory_open_bus_wram_and_joypad_serial() {
    let mut m = Machine::new(&fixture()).unwrap();
    m.bus.write(0x7e1234, 0x82);
    assert_eq!(m.bus.read(0x001234), 0x82);
    assert_eq!(m.bus.read(0x003000), 0x82);
    m.set_buttons(0x9000);
    m.bus.write(0x4016, 1);
    m.bus.write(0x4016, 0);
    let mut bits = 0;
    for _ in 0..16 {
        bits = (bits << 1) | (m.bus.read(0x4016) & 1) as u16;
    }
    assert_eq!(bits, 0x9000);
    assert_eq!(m.bus.read(0x4016) & 1, 1);
}
#[test]
fn bus_access_timing_is_region_dependent() {
    let mut m = Machine::new(&fixture()).unwrap();
    let start = m.bus.clocks;
    m.bus.read(0x008000);
    assert_eq!(m.bus.clocks - start, 8);
    let start = m.bus.clocks;
    m.bus.read(0x002100);
    assert_eq!(m.bus.clocks - start, 6);
    let start = m.bus.clocks;
    m.bus.read(0x004016);
    assert_eq!(m.bus.clocks - start, 12);
    m.bus.write(0x420d, 1);
    let start = m.bus.clocks;
    m.bus.read(0x808000);
    assert_eq!(m.bus.clocks - start, 6);
}

// Instruction cycles, independent of the region-dependent master-clock bus waits.
// WDC W65C816S tables 3-1, 5-4 and 5-7 (especially note 4).
fn timed_instruction(code: &[u8], index: u16, p: u8, d: u16, base: u16) -> usize {
    let mut ram = Ram::new(code);
    ram.data[d as usize + 0x20..d as usize + 0x22].copy_from_slice(&base.to_le_bytes());
    let mut cpu = Cpu {
        pc: 0x8000,
        e: false,
        p,
        x: index,
        y: index,
        d,
        ..Cpu::default()
    };
    cpu.step(&mut ram);
    ram.reads.len() + ram.writes.len() + ram.idle
}

#[test]
fn direct_indexing_always_takes_a_cycle_even_with_zero_index() {
    for index in [0, 1] {
        for (op, cycles) in [(0xb5, 4), (0xb6, 4), (0xa1, 6), (0x95, 4), (0x16, 6)] {
            for d in [0, 1] {
                assert_eq!(
                    timed_instruction(&[op, 0x20], index, M | X, d, 0x9000),
                    cycles + usize::from(d != 0),
                    "opcode {op:02x}, index {index}, D {d}"
                );
            }
        }
    }
}

#[test]
fn indexed_reads_add_one_cycle_for_page_crossing_or_wide_indexes() {
    for (index, base, xb, extra) in [
        (0, 0x9000, X, 0),
        (1, 0x9000, X, 0),
        (1, 0x90ff, X, 1),
        (1, 0xffff, X, 1),
        (0, 0x9000, 0, 1),
        (1, 0x9000, 0, 1),
        (1, 0x90ff, 0, 1), // the two conditions do not add two cycles
        (0xffff, 0x9001, 0, 1),
    ] {
        for mb in [M, 0] {
            for op in [
                0x1d, 0x3d, 0x5d, 0x7d, 0xbd, 0xdd, 0xfd, 0x3c, 0x19, 0x39, 0x59, 0x79, 0xb9, 0xd9,
                0xf9, 0xbc, 0xbe,
            ] {
                let width = if matches!(op, 0xbc | 0xbe) { xb } else { mb };
                assert_eq!(
                    timed_instruction(
                        &[op, base as u8, (base >> 8) as u8],
                        index,
                        mb | xb,
                        0,
                        base
                    ),
                    4 + extra + usize::from(width == 0),
                    "opcode {op:02x}, index {index}, base {base:04x}, P {:02x}",
                    mb | xb
                );
            }
            for op in [0x11, 0x31, 0x51, 0x71, 0xb1, 0xd1, 0xf1] {
                assert_eq!(
                    timed_instruction(&[op, 0x20], index, mb | xb, 0, base),
                    5 + extra + usize::from(mb == 0),
                    "opcode {op:02x}, index {index}, base {base:04x}"
                );
            }
        }
    }
}

#[test]
fn indexed_writes_and_stack_indirect_have_fixed_cycles() {
    for (index, base) in [(0, 0x9000), (1, 0x9000), (1, 0x90ff)] {
        for xb in [X, 0] {
            for mb in [M, 0] {
                for op in [0x99, 0x9d, 0x9e] {
                    assert_eq!(
                        timed_instruction(
                            &[op, base as u8, (base >> 8) as u8],
                            index,
                            mb | xb,
                            0,
                            base
                        ),
                        5 + usize::from(mb == 0)
                    );
                }
                for op in [0x1e, 0x3e, 0x5e, 0x7e, 0xde, 0xfe] {
                    assert_eq!(
                        timed_instruction(
                            &[op, base as u8, (base >> 8) as u8],
                            index,
                            mb | xb,
                            0,
                            base
                        ),
                        7 + 2 * usize::from(mb == 0)
                    );
                }
                assert_eq!(
                    timed_instruction(&[0x91, 0x20], index, mb | xb, 0, base),
                    6 + usize::from(mb == 0)
                );
                for op in [0xb3, 0x93] {
                    assert_eq!(
                        timed_instruction(&[op, 0x20], index, mb | xb, 0, base),
                        7 + usize::from(mb == 0)
                    );
                }
                // Long indexed modes never pay a page-crossing/index-width penalty.
                for op in [0xbf, 0x9f] {
                    assert_eq!(
                        timed_instruction(
                            &[op, base as u8, (base >> 8) as u8, 0],
                            index,
                            mb | xb,
                            0,
                            base
                        ),
                        5 + usize::from(mb == 0)
                    );
                }
                for op in [0xb7, 0x97] {
                    assert_eq!(
                        timed_instruction(&[op, 0x20], index, mb | xb, 0, base),
                        6 + usize::from(mb == 0)
                    );
                }
            }
        }
    }
}
#[test]
fn diagnostic_has_deterministic_video_dma_input_and_spc_audio() {
    let mut a = Machine::new(&fixture()).unwrap();
    let mut b = Machine::new(&fixture()).unwrap();
    let mut hashes = std::collections::BTreeSet::new();
    let mut energy = 0.0;
    for _ in 0..90 {
        a.set_buttons(0x8100);
        b.set_buttons(0x8100);
        a.run_frame().unwrap();
        b.run_frame().unwrap();
        assert_eq!(a.cpu.pc, b.cpu.pc);
        assert_eq!(a.bus.clocks, b.bus.clocks);
        assert_eq!(a.framebuffer(), b.framebuffer());
        hashes.insert(frame_hash(a.framebuffer()));
        let aa = a.take_audio();
        assert_eq!(aa, b.take_audio());
        energy += aa.iter().map(|v| (*v as f64).powi(2)).sum::<f64>();
    }
    assert!(hashes.len() >= 30);
    assert!(energy > 1.0);
    assert_eq!(a.bus.wram[0x11], 0x81);
    assert!(a.bus.wram[0x10] > 80);
    assert_eq!(&a.bus.ppu.vram[..16], &fixture()[0x1000..0x1010]);
    assert!(a.framebuffer().as_chunks::<4>().0.iter().any(|p| p[0] > 0));
}
#[test]
fn reset_preserves_sram_and_invalid_restore_is_atomic() {
    let mut m = Machine::new(&fixture()).unwrap();
    let save = vec![0x93; m.bus.cart.sram.len()];
    m.load_sram(&save).unwrap();
    m.bus.wram[0] = 12;
    assert!(m.load_sram(&[1]).is_err());
    assert_eq!(m.bus.cart.sram, save);
    m.reset();
    assert_eq!(m.bus.cart.sram, save);
    assert_eq!(m.bus.wram[0], 0);
    assert_eq!(m.bus.buttons, 0);
}
#[test]
fn spc_word_arithmetic_timer_and_direct_page() {
    let mut apu = snes_core::apu::Apu::default();
    let program = [0xe8, 0xff, 0x8d, 0x7f, 0x7a, 0x10, 0x40, 0x8f, 0x34, 0x20];
    apu.ram[0x200..0x200 + program.len()].copy_from_slice(&program);
    apu.ram[0x10] = 1;
    apu.cpu.pc = 0x200;
    for _ in 0..5 {
        apu.step();
    }
    assert_eq!(apu.cpu.a, 0);
    assert_eq!(apu.cpu.y, 0x80);
    assert_eq!(apu.cpu.p & 0xc0, 0xc0);
    assert_eq!(apu.ram[0x120], 0x34);
    apu.write(0xfa, 1);
    apu.write(0xf1, 1);
    apu.tick(21_478);
    let count = apu.read(0xfd);
    assert_eq!(count, 8);
    assert_eq!(apu.read(0xfd), 0);
}
#[test]
fn every_spc_opcode_decodes_without_fallback() {
    for op in 0u8..=255 {
        let mut apu = snes_core::apu::Apu::default();
        apu.cpu.pc = 0x200;
        apu.ram[0x200] = op;
        apu.step();
        assert!(apu.error.is_none(), "opcode {op:02x}");
    }
}
