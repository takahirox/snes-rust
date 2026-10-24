//! Original MIT vectors and firmware-free SPC700 guest for Issue #87.
//! Copyright 2026 snes-rust contributors. See tests/roms/PROVENANCE.md.
//! Expectations: Anomie SPC700 revision 1126, SPC700 REGISTERS; derivations
//! and Fullsnes qualifications are recorded in docs/issue-87-validation.md.
use snes_core::apu::Apu;

const SELECTED: [u16; 5] = [0xf0, 0xf1, 0xfa, 0xfb, 0xfc];
const SENTINELS: [u8; 5] = [0xa1, 0xb2, 0xc3, 0xd4, 0xe5];
const PAGE_ONE: [u8; 5] = [0x61, 0x72, 0x83, 0x94, 0xa5];
const MASTER_HZ: u64 = 21_477_272;
const SPC_HZ: u64 = 1_024_000;

fn fixture() -> Apu {
    let mut apu = Apu::default();
    for ((address, sentinel), page_one) in SELECTED.into_iter().zip(SENTINELS).zip(PAGE_ONE) {
        // Backing bytes are fixture inputs, not register writes or results.
        apu.ram[address as usize] = sentinel;
        apu.ram[address as usize + 0x100] = page_one;
    }
    apu
}

fn zero_reads(apu: &mut Apu, address: u16) {
    let backing = apu.ram[address as usize];
    for _ in 0..3 {
        assert_eq!(apu.read(address), 0, "ordinary read at ${address:04x}");
        assert_eq!(apu.ram[address as usize], backing, "read preserves backing");
    }
}

#[test]
fn test_reads_zero_with_nonzero_backing_and_documented_normal_write() {
    let mut apu = fixture();
    zero_reads(&mut apu, 0xf0);
    // Only the documented normal setting: no zero/clock-mode TEST writes.
    apu.write(0xf0, 0x0a);
    assert_eq!(apu.ram[0xf0], 0x0a);
    zero_reads(&mut apu, 0xf0);
}

#[test]
fn control_reads_zero_without_repeating_write_effects() {
    let mut apu = fixture();
    zero_reads(&mut apu, 0xf1);
    for value in [0, 0xb7, 7] {
        apu.write(0xf1, value);
        assert_eq!(apu.ram[0xf1], value);
        for port in 0..4 {
            apu.port_write(port, 0x40 + port as u8);
            apu.write(0xf4 + port as u16, 0x50 + port as u8);
        }
        zero_reads(&mut apu, 0xf1);
        assert_eq!(apu.input, [0x40, 0x41, 0x42, 0x43]);
        assert_eq!(apu.output, [0x50, 0x51, 0x52, 0x53]);
    }
    apu.write(0xf1, 0x30);
    assert_eq!(
        apu.input, [0; 4],
        "actual CONTROL writes still clear inputs"
    );
    assert_eq!(apu.output, [0x50, 0x51, 0x52, 0x53]);
}

fn target_reads(timer: u16) {
    let mut apu = fixture();
    let address = 0xfa + timer;
    zero_reads(&mut apu, address);
    for value in [0, 0x35 + timer as u8, 0xff] {
        apu.write(address, value);
        assert_eq!(apu.ram[address as usize], value);
        zero_reads(&mut apu, address);
    }
}
#[test]
fn timer_zero_target_reads_zero() {
    target_reads(0);
}
#[test]
fn timer_one_target_reads_zero() {
    target_reads(1);
}
#[test]
fn timer_two_target_reads_zero() {
    target_reads(2);
}

fn advance(apu: &mut Apu, master: &mut u64, end: u64, split: bool) {
    let mut remaining = end - *master;
    for chunk in [1, 7, 666, 1001, 21, 8193].into_iter().cycle() {
        if remaining == 0 {
            break;
        }
        let step = if split {
            remaining.min(chunk)
        } else {
            remaining
        };
        apu.tick(step);
        remaining -= step;
    }
    *master = end;
    assert!(apu.error.is_none(), "{:?}", apu.error);
}
fn cycle(apu: &mut Apu, master: &mut u64, end: u64) {
    assert_eq!(end % 2, 0, "idle fixture advances in two-cycle units");
    advance(apu, master, (end * MASTER_HZ).div_ceil(SPC_HZ), false);
}

#[test]
fn each_selected_read_preserves_all_timer_stages_and_live_target_writes() {
    for timer in 0..3 {
        let d = if timer == 2 { 16 } else { 128 };
        for address in SELECTED {
            let mut apu = fixture();
            let mut master = 0;
            cycle(&mut apu, &mut master, 3 * d + d / 4);
            apu.write(0xfa + timer, 3);
            apu.write(0xf1, 0x30 | (1 << timer));
            for port in 0..4 {
                apu.port_write(port, 0x40 + port as u8);
                apu.write(0xf4 + port as u16, 0x50 + port as u8);
            }
            // Enabled pulses 4D..7D leave count=1, output=1, phase=D/2.
            cycle(&mut apu, &mut master, 7 * d + d / 2);
            apu.write(0xfa + timer, 2); // live target: next pulse must match
            zero_reads(&mut apu, address);
            assert_eq!(apu.input, [0x40, 0x41, 0x42, 0x43]);
            assert_eq!(apu.output, [0x50, 0x51, 0x52, 0x53]);
            cycle(&mut apu, &mut master, 8 * d - 2);
            assert_eq!(apu.read(0xfd + timer), 1, "unread output survives");
            cycle(&mut apu, &mut master, 8 * d);
            assert_eq!(apu.read(0xfd + timer), 1, "count/phase/target survive");
            assert_eq!(apu.read(0xfd + timer), 0, "only output reads clear");
            cycle(&mut apu, &mut master, 9 * d + d / 2);
            zero_reads(&mut apu, address);
            cycle(&mut apu, &mut master, 10 * d - 2);
            assert_eq!(apu.read(0xfd + timer), 0);
            cycle(&mut apu, &mut master, 10 * d);
            assert_eq!(apu.read(0xfd + timer), 1, "second separated window");
            apu.write(0xfa + timer, 0); // zero is 256 pulses, not disabled
            cycle(&mut apu, &mut master, 11 * d + d / 2);
            zero_reads(&mut apu, address);
            cycle(&mut apu, &mut master, 266 * d - 2);
            assert_eq!(apu.read(0xfd + timer), 0);
            cycle(&mut apu, &mut master, 266 * d);
            assert_eq!(apu.read(0xfd + timer), 1, "zero target write survives");
        }
    }
}

#[test]
fn neighboring_routes_and_page_one_remain_independent() {
    let mut apu = fixture();
    apu.write(0xf2, 0x0c);
    apu.write(0xf3, 37);
    assert_eq!(apu.read(0xf2), 0x0c);
    assert_eq!(apu.read(0xf3), 37);
    apu.write(0xf2, 0x8c);
    apu.write(0xf3, 0xe7); // high-bit DSP index ignores writes, masks reads
    assert_eq!(apu.read(0xf2), 0x8c);
    assert_eq!(apu.read(0xf3), 37);
    for port in 0..4 {
        apu.port_write(port, 0x40 + port as u8);
        apu.write(0xf4 + port as u16, 0x50 + port as u8);
        assert_eq!(apu.read(0xf4 + port as u16), 0x40 + port as u8);
        assert_eq!(apu.port_read(port), 0x50 + port as u8);
    }
    for (address, value) in [(0xf8, 0x98), (0xf9, 0xa9), (0xef, 0x76), (0x100, 0x87)] {
        apu.write(address, value);
        assert_eq!(apu.read(address), value);
    }
    for (address, value) in SELECTED.into_iter().zip(PAGE_ONE) {
        assert_eq!(apu.read(address + 0x100), value);
        apu.write(address + 0x100, value ^ 0xff);
        assert_eq!(apu.read(address + 0x100), value ^ 0xff);
        zero_reads(&mut apu, address);
    }
    let mut master = 0;
    for timer in 0..3 {
        apu.write(0xfa + timer, if timer == 2 { 8 } else { 1 });
    }
    apu.write(0xf1, 7);
    cycle(&mut apu, &mut master, 5 * 128 + 8);
    for timer in 0..3 {
        apu.write(0xfd + timer, 0xff);
        assert_eq!(apu.read(0xfd + timer), 5, "writes do not clear output");
        assert_eq!(apu.read(0xfd + timer), 0);
    }
}

// Values derived from the documented read/port rules and fixture inputs,
// independently of guest construction and implementation output.
const RECORDS: [u8; 56] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // repeated initial overlay reads
    0x0c, 37, 0x8c, 37, // DSP index/data and masked read / ignored write
    1, 0x42, 0x43, 0x44, // host inputs, independent of guest outputs
    0x98, 0xa9, 0x76, 0x87, // F8/F9 and ordinary RAM
    0x61, 0x72, 0x83, 0x94, 0xa5, // SETP: page 1 RAM
    0, 0, 0, 0, 0, // CLRP restores page 0 register decoding
    0, 0, 0, 0, 0, // after target/CONTROL writes
    1, 1, 1, // first separated timer-output window
    0, 0, 0, 0, 0, // reads between windows
    1, 1, 1, // second window
    0, 0, 0, 0, 0, 2, 2, 2, // third window
];

fn guest() -> Vec<u8> {
    fn store(code: &mut Vec<u8>, records: &mut u16) {
        code.push(0xc5); // MOV !record,A (5 cycles)
        code.extend((0x1200 + *records).to_le_bytes());
        *records += 1;
    }
    fn read(code: &mut Vec<u8>, elapsed: &mut u64, records: &mut u16, address: u8) {
        code.extend([0xe4, address]); // MOV A,dp (3 cycles)
        store(code, records);
        *elapsed += 8;
    }
    fn write(code: &mut Vec<u8>, elapsed: &mut u64, address: u8, value: u8) {
        code.extend([0x8f, value, address]); // MOV dp,#imm (5)
        *elapsed += 5;
    }
    fn until(code: &mut Vec<u8>, elapsed: &mut u64, at: u64) {
        let mut cycles = at - *elapsed;
        if cycles % 2 == 1 {
            assert!(cycles >= 3);
            code.extend([0xe4, 0]); // harmless RAM read (3)
            cycles -= 3;
        }
        let mut iterations = cycles / 6;
        while iterations != 0 {
            let n = iterations.min(255);
            // MOV Y,#n (2), DBNZ Y (6 taken / 4 final): exactly 6*n.
            code.extend([0x8d, n as u8, 0xfe, 0xfe]);
            iterations -= n;
        }
        code.extend(std::iter::repeat_n(0, (cycles % 6 / 2) as usize)); // NOP (2)
        *elapsed = at;
    }
    fn selected(code: &mut Vec<u8>, elapsed: &mut u64, records: &mut u16) {
        for address in SELECTED {
            read(code, elapsed, records, address as u8);
        }
    }
    let mut code = vec![
        0x8f, 0x0c, 0xf2, 0x8f, 37, 0xf3, // independent DSP register
        0x8f, 0x6c, 0xf2, 0x8f, 0x20, 0xf3, // disable echo writes
        0x8f, 0x0c, 0xf2, // restore DSP index control
        0x8f, 0x7f, 0xf4, // ready
        0xe4, 0xf4, 0x68, 1, 0xd0, 0xfa, // bounded by host start/budget
    ];
    let (mut elapsed, mut records) = (0, 0);
    selected(&mut code, &mut elapsed, &mut records);
    selected(&mut code, &mut elapsed, &mut records);
    for address in [0xf2, 0xf3] {
        read(&mut code, &mut elapsed, &mut records, address);
    }
    write(&mut code, &mut elapsed, 0xf2, 0x8c);
    write(&mut code, &mut elapsed, 0xf3, 0xe7);
    for address in [0xf2, 0xf3] {
        read(&mut code, &mut elapsed, &mut records, address);
    }
    for port in 0..4 {
        write(&mut code, &mut elapsed, 0xf4 + port, 0x50 + port);
        read(&mut code, &mut elapsed, &mut records, 0xf4 + port);
    }
    for (address, value) in [(0xf8, 0x98), (0xf9, 0xa9)] {
        write(&mut code, &mut elapsed, address, value);
        read(&mut code, &mut elapsed, &mut records, address);
    }
    for address in [0x00efu16, 0x0100] {
        code.push(0xe5); // MOV A,!abs (4)
        code.extend(address.to_le_bytes());
        store(&mut code, &mut records);
        elapsed += 9;
    }
    code.push(0x40); // SETP (2): direct-page reads must now access RAM
    elapsed += 2;
    selected(&mut code, &mut elapsed, &mut records);
    code.push(0x20); // CLRP (2)
    elapsed += 2;
    selected(&mut code, &mut elapsed, &mut records);
    assert_eq!(records, 32);
    for (address, value) in [(0xfa, 3), (0xfb, 3), (0xfc, 24)] {
        write(&mut code, &mut elapsed, address, value);
    }
    until(&mut code, &mut elapsed, 600);
    write(&mut code, &mut elapsed, 0xf1, 7);
    until(&mut code, &mut elapsed, 620);
    selected(&mut code, &mut elapsed, &mut records);
    for (window, at) in [1100, 1500, 2200].into_iter().enumerate() {
        for timer in 0..3 {
            until(&mut code, &mut elapsed, at + 10 * timer);
            read(&mut code, &mut elapsed, &mut records, 0xfd + timer as u8);
        }
        if window < 2 {
            until(&mut code, &mut elapsed, [1300, 1800][window]);
            selected(&mut code, &mut elapsed, &mut records);
        }
    }
    assert_eq!(records as usize, RECORDS.len());
    code.extend([0x8f, 0x87, 0xf4, 0xff]); // completion + STOP, no TEST writes
    assert!(code.len() < 512);
    code
}

fn wait_for(apu: &mut Apu, master: &mut u64, value: u8) {
    for _ in 0..1000 {
        assert!(apu.error.is_none(), "{:?}", apu.error);
        if apu.port_read(0) == value {
            return;
        }
        apu.tick(21);
        *master += 21;
    }
    panic!("SPC acknowledgement {value:02x} exceeded 21,000 master clocks");
}
fn upload(apu: &mut Apu, code: &[u8]) -> u64 {
    let mut master = 0;
    apu.port_write(2, 0);
    apu.port_write(3, 2);
    apu.port_write(1, 1);
    apu.port_write(0, 0xcc);
    wait_for(apu, &mut master, 0xcc);
    for (index, &byte) in code.iter().enumerate() {
        apu.port_write(1, byte);
        apu.port_write(0, index as u8);
        wait_for(apu, &mut master, index as u8);
    }
    apu.port_write(2, 0);
    apu.port_write(3, 2);
    apu.port_write(1, 0);
    apu.port_write(0, (code.len() as u8).wrapping_add(1));
    wait_for(apu, &mut master, 0x7f);
    assert_eq!(&apu.ram[0x200..0x200 + code.len()], code);
    assert!(master < 100_000, "bounded firmware-free upload");
    master
}

#[test]
fn uploaded_reads_have_independent_records_in_whole_and_irregular_runs() {
    let code = guest();
    let mut whole = fixture();
    let mut split = fixture();
    let (mut whole_master, mut split_master) = (0, 0);
    for (apu, master) in [
        (&mut whole, &mut whole_master),
        (&mut split, &mut split_master),
    ] {
        apu.ram[0xef] = 0x76;
        apu.ram[0x100] = 0x87;
        apu.ram[0x1200..0x1200 + RECORDS.len()].fill(0xee); // unwritten marker
        *master = upload(apu, &code);
        cycle(apu, master, 8192);
        assert_eq!(apu.port_read(0), 0x7f, "ready before fixed start");
        for (port, value) in [1, 0x42, 0x43, 0x44].into_iter().enumerate() {
            apu.port_write(port, value);
        }
    }
    assert_eq!(whole_master, split_master);
    let origin = whole_master;
    // First post-handshake instruction lies in cycles 8192+[0,18].
    // Slow matches: 896,1280,1664,2048. Fast matches: 600+s+384*k-q,
    // q in 0..15. Read windows 1100/1500/2200 (+0/10/20, +s) stay
    // separated from every event for both timer rates; records are 1/1/2.
    for (elapsed, records) in [(10_000, 32), (26_000, 40), (36_000, 48), (60_000, 56)] {
        advance(&mut whole, &mut whole_master, origin + elapsed, false);
        advance(&mut split, &mut split_master, origin + elapsed, true);
        for apu in [&whole, &split] {
            assert_eq!(&apu.ram[0x1200..0x1200 + records], &RECORDS[..records]);
            assert_eq!(apu.dsp.regs[0x0c], 37);
            assert_eq!(apu.dsp.regs[0x6c], 0x20);
            assert!(apu.samples.iter().all(|&sample| sample == 0.0));
            assert!(apu.error.is_none(), "{:?}", apu.error);
        }
        // Only after independently requiring values in each run: determinism.
        assert_eq!(whole.samples, split.samples);
        assert_eq!(whole.dsp.regs, split.dsp.regs);
        assert_eq!(whole.ram, split.ram);
        assert_eq!(whole.input, split.input);
        assert_eq!(whole.output, split.output);
        assert_eq!(
            format!("{:?}", whole.cpu),
            format!("{:?}", split.cpu),
            "all public SPC registers, PC and halted state"
        );
    }
    for apu in [&mut whole, &mut split] {
        assert_eq!(apu.output, [0x87, 0x51, 0x52, 0x53], "bounded completion");
        assert_eq!(apu.input, [1, 0x42, 0x43, 0x44]);
        assert!(apu.cpu.halted);
        // Halted time leaves timers running. Freeze counting with an actual
        // CONTROL write, then compare public timer state (and read-clear).
        apu.write(0xf1, 0);
    }
    for timer in 0..3 {
        assert_eq!(whole.read(0xfd + timer), split.read(0xfd + timer));
        assert_eq!(whole.read(0xfd + timer), 0);
        assert_eq!(split.read(0xfd + timer), 0);
    }
}
