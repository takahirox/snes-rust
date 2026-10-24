//! Original MIT Issue #69 vectors. Times and expectations come from the
//! documented event table, independently of production constants/helpers.
use super::*;

const START: u64 = 225 * 1364 + 298;
// First two opportunities are 1396*256 apart, even with the short field.
// This is a polling interval, not a field length. Later lattice wraps differ.
const NEXT: u64 = START + 357376;
const END: u64 = START + 4224;
const PREFIXES: [(u16, u16); 16] = [
    (0x0001, 0x0000),
    (0x0002, 0x0001),
    (0x0005, 0x0002),
    (0x000a, 0x0005),
    (0x0014, 0x000b),
    (0x0029, 0x0016),
    (0x0052, 0x002d),
    (0x00a5, 0x005a),
    (0x014b, 0x00b4),
    (0x0296, 0x0169),
    (0x052c, 0x02d3),
    (0x0a59, 0x05a6),
    (0x14b2, 0x0b4c),
    (0x2964, 0x1698),
    (0x52c8, 0x2d30),
    (0xa590, 0x5a60),
];

fn machine(first: u16, second: u16) -> Machine {
    let mut m = Machine::new(include_bytes!("../../../../tests/roms/smoke.bin")).unwrap();
    m.set_buttons(first);
    m.set_buttons2(second);
    m.bus.write_raw(0x4200, 1);
    m
}
fn until(b: &mut SystemBus, time: u64) {
    assert!(time >= b.clocks);
    b.tick(time - b.clocks);
}
fn bytes(b: &mut SystemBus, bank: u32) -> [u8; 8] {
    std::array::from_fn(|i| b.read_raw((bank << 16) | (0x4218 + i as u32)))
}
fn words(b: &mut SystemBus, first: u16, second: u16) {
    for bank in [0, 0x3f, 0x80, 0xbf] {
        assert_eq!(
            bytes(b, bank),
            [
                first as u8,
                (first >> 8) as u8,
                second as u8,
                (second >> 8) as u8,
                0,
                0,
                0,
                0
            ]
        );
    }
}
fn busy(b: &mut SystemBus) -> u8 {
    b.read_raw(0x4212) & 1
}

#[test]
fn every_button_both_pads_complete_bytes_and_mirrors() {
    let vectors = [
        (0xa59f, 0x5a6f),
        (0x5a6f, 0xa59f),
        (0, 0),
        (0xffff, 0xffff),
        (15, 15),
        (0xffff, 0),
        (0, 0xffff),
    ];
    for (first, second) in vectors
        .into_iter()
        .chain((0..12).flat_map(|bit| [(1 << (15 - bit) | 15, 15), (15, 1 << (15 - bit) | 15)]))
    {
        let mut m = machine(first, second);
        until(&mut m.bus, END);
        words(&mut m.bus, first & 0xfff0, second & 0xfff0);
        assert_eq!(busy(&mut m.bus), 0);
        for port in [0x4016, 0x4017] {
            for _ in 0..8 {
                assert_eq!(m.bus.read_raw(port) & 3, 1);
            }
        }
    }
}

#[test]
fn before_at_after_vblank_start_release_and_all_sixteen_samples() {
    let mut m = machine(0xa590, 0x5a60);
    let b = &mut m.bus;
    for time in [225 * 1364 - 1, 225 * 1364, 225 * 1364 + 1, START - 1] {
        until(b, time);
        assert_eq!(busy(b), 0);
        words(b, 0, 0);
    }
    // First complete a frame, so next start must clear *nonzero* prior results.
    until(b, END);
    for time in [NEXT - 1, NEXT, NEXT + 1, NEXT + 255, NEXT + 256, NEXT + 257] {
        until(b, time);
        let started = time >= NEXT;
        assert_eq!(busy(b), u8::from(started));
        assert_eq!(b.auto_latch, started && time < NEXT + 256);
        words(
            b,
            if started { 0 } else { 0xa590 },
            if started { 0 } else { 0x5a60 },
        );
    }
    let mut prior = (0, 0);
    for (index, &(first, second)) in PREFIXES.iter().enumerate() {
        let event = NEXT + 384 + index as u64 * 256;
        until(b, event - 1);
        assert_eq!(busy(b), 1);
        words(b, prior.0, prior.1);
        until(b, event);
        assert_eq!(busy(b), u8::from(index < 15));
        words(b, first, second);
        until(b, event + 1);
        words(b, first, second);
        assert_eq!(busy(b), u8::from(index < 15));
        prior = (first, second);
    }
}

#[test]
fn input_changes_while_latch_high_capture_on_release_and_freeze_afterward() {
    let mut m = machine(0x8000, 0);
    until(&mut m.bus, START + 50);
    m.set_buttons(0x5a6f);
    m.set_buttons2(0xa59f);
    assert_eq!(m.bus.read_raw(0x4016) & 1, 0);
    assert_eq!(m.bus.read_raw(0x4017) & 1, 1);
    until(&mut m.bus, START + 256);
    m.set_buttons(0xffff);
    m.set_buttons2(0);
    until(&mut m.bus, END);
    words(&mut m.bus, 0x5a60, 0xa590);
    // New physical inputs enter the following cycle, not the captured one.
    until(&mut m.bus, NEXT + 4224);
    words(&mut m.bus, 0xfff0, 0);
}

#[test]
fn unequal_manual_clocks_change_only_the_selected_ports_future_samples() {
    for (first, second, expected) in [
        (0xa590, 0x5a60, (0xb643, 0x54c1)),
        (0x5a60, 0xa590, (0x4983, 0xab21)),
    ] {
        let mut m = machine(first, second);
        until(&mut m.bus, START + 384 + 2 * 256 + 40); // Three automatic bits.
                                                       // Pad 1 skips positions 3/4; pad 2 skips position 3. The 13 remaining
                                                       // samples include respectively two/one trailing ones. Literal outputs
                                                       // concatenate original prefixes with those shortened tails.
        assert_eq!(m.bus.read_raw(0x4016) & 1, ((first >> 12) & 1) as u8);
        assert_eq!(m.bus.read_raw(0x804016) & 1, ((first >> 11) & 1) as u8);
        assert_eq!(m.bus.read_raw(0xbf4017) & 1, ((second >> 12) & 1) as u8);
        until(&mut m.bus, END);
        words(&mut m.bus, expected.0, expected.1);
        assert_eq!(m.bus.read_raw(0x4016) & 1, 1);
        assert_eq!(m.bus.read_raw(0x4017) & 1, 1);
    }
}

#[test]
fn manual_relatch_replaces_both_tails_and_leaves_twelve_bits_consumed() {
    let mut m = machine(0xa590, 0x5a60);
    until(&mut m.bus, START + 1152 + 40); // Four samples: prefixes A and 5.
    m.set_buttons(0x3c90);
    m.set_buttons2(0xc360);
    m.bus.write_raw(0x804016, 1);
    m.bus.tick(8);
    m.bus.write_raw(0xbf4016, 0);
    until(&mut m.bus, END);
    // Twelve remaining samples take bits 15..4 of the replacement words.
    words(&mut m.bus, 0xa3c9, 0x5c36);
    for position in 0..8 {
        for port in [0x4016, 0x4017] {
            assert_eq!(m.bus.read_raw(port) & 1, u8::from(position >= 4));
        }
    }
}

#[test]
fn manual_out0_held_high_survives_automatic_release() {
    let mut m = machine(0x8000, 0x4000);
    m.bus.write_raw(0x4016, 1);
    until(&mut m.bus, END);
    words(&mut m.bus, 0xffff, 0);
    m.bus.write_raw(0x4016, 0);
    assert_eq!(m.bus.read_raw(0x4016) & 1, 1);
    assert_eq!(m.bus.read_raw(0x4017) & 1, 0);
    assert_eq!(m.bus.read_raw(0x4017) & 1, 1);
}

#[test]
fn disabled_enable_before_start_and_disable_reenable_after_completion() {
    let mut m = machine(0xa590, 0x5a60);
    m.bus.write_raw(0x4200, 0);
    // Disable one clock before the opportunity, preserving manual progress.
    m.bus.write_raw(0x4016, 1);
    m.bus.write_raw(0x4016, 0);
    assert_eq!(m.bus.read_raw(0x4016) & 1, 1);
    until(&mut m.bus, START - 1);
    until(&mut m.bus, END + 1);
    words(&mut m.bus, 0, 0);
    assert_eq!(busy(&mut m.bus), 0);
    assert_eq!(m.bus.read_raw(0x4016) & 1, 0); // Y, no automatic relatch/clock.
    until(&mut m.bus, NEXT - 1);
    m.bus.write_raw(0x4200, 1);
    until(&mut m.bus, NEXT);
    assert_eq!(busy(&mut m.bus), 1);
    until(&mut m.bus, NEXT + 4224 + 1);
    m.bus.write_raw(0x4200, 0);
    m.set_buttons(0);
    m.set_buttons2(0);
    until(&mut m.bus, NEXT + 357376 + 4224);
    words(&mut m.bus, 0xa590, 0x5a60);
    assert_eq!(busy(&mut m.bus), 0);
    m.bus.write_raw(0x4200, 1);
    assert_eq!(busy(&mut m.bus), 0); // No immediate restart in this VBlank.
    until(&mut m.bus, NEXT + 2 * 357376 + 4224);
    words(&mut m.bus, 0, 0);
}

#[test]
fn disabling_immediately_before_opportunity_skips_it() {
    let mut m = machine(0xa590, 0x5a60);
    until(&mut m.bus, START - 1);
    m.bus.write_raw(0x4200, 0);
    until(&mut m.bus, END + 1);
    assert_eq!(busy(&mut m.bus), 0);
    words(&mut m.bus, 0, 0);
}

#[test]
fn reset_during_latch_or_sampling_clears_poll_results_inputs_and_preserves_sram() {
    for time in [START + 40, START + 640] {
        let mut m = machine(0xffff, 0xffff);
        m.bus.cart.sram = vec![0xa6; 2048];
        until(&mut m.bus, time);
        assert_eq!(busy(&mut m.bus), 1);
        m.reset();
        assert_eq!(m.bus.cart.sram, vec![0xa6; 2048]);
        assert_eq!(busy(&mut m.bus), 0);
        words(&mut m.bus, 0, 0);
        until(&mut m.bus, END + 1);
        words(&mut m.bus, 0, 0);
        m.bus.write_raw(0x4016, 1);
        m.bus.write_raw(0x4016, 0);
        for _ in 0..16 {
            assert_eq!(m.bus.read_raw(0x4016) & 3, 0);
            assert_eq!(m.bus.read_raw(0x4017) & 3, 0);
        }
    }
}

#[test]
fn fine_irregular_and_whole_interval_ticks_have_identical_state() {
    let mut fine = machine(0xa590, 0x5a60);
    let mut irregular = machine(0xa590, 0x5a60);
    let mut whole = machine(0xa590, 0x5a60);
    // Include changing inputs and unequal manual reads at noncoincident times.
    for (time, action) in [
        (START + 80, 0),
        (START + 1000, 1),
        (END + 1, 2),
        (NEXT + 4225, 2),
    ] {
        while fine.bus.clocks < time {
            fine.bus.tick(1);
        }
        for n in [3, 507, 31, 4099, 8, 137].into_iter().cycle() {
            let remaining = time - irregular.bus.clocks;
            if remaining == 0 {
                break;
            }
            irregular.bus.tick(n.min(remaining));
        }
        until(&mut whole.bus, time);
        for m in [&mut fine, &mut irregular, &mut whole] {
            if action == 0 {
                m.set_buttons(0x5a60);
                m.set_buttons2(0xa590);
            }
            if action == 1 {
                m.bus.read_raw(0x4016);
                m.bus.read_raw(0x4017);
                m.bus.read_raw(0x4017);
            }
        }
        assert_eq!(bytes(&mut fine.bus, 0), bytes(&mut irregular.bus, 0));
        assert_eq!(bytes(&mut fine.bus, 0), bytes(&mut whole.bus, 0));
        assert_eq!(fine.bus.serial, irregular.bus.serial);
        assert_eq!(fine.bus.serial, whole.bus.serial);
        assert_eq!(busy(&mut fine.bus), busy(&mut irregular.bus));
        assert_eq!(busy(&mut fine.bus), busy(&mut whole.bus));
    }
    // The last whole tick crossed the complete second polling interval.
    words(&mut whole.bus, 0x5a60, 0xa590);
}

#[test]
fn dma_naturally_crosses_poll_and_leaves_both_serial_ports_consumed() {
    let mut m = machine(0xa590, 0x5a60);
    until(&mut m.bus, START - 80);
    let b = &mut m.bus;
    b.dma[0] = [0; 16];
    b.dma[0][1] = 6;
    b.dma[0][4] = 0x7e;
    b.dma[0][5..7].copy_from_slice(&700u16.to_le_bytes());
    b.write_raw(0x420b, 1);
    let pc = m.cpu.pc;
    b.events();
    assert_eq!(m.cpu.pc, pc);
    assert!(b.clocks > END);
    assert_eq!(busy(b), 0);
    words(b, 0xa590, 0x5a60);
    assert_eq!(b.read_raw(0x4016) & 1, 1);
    assert_eq!(b.read_raw(0x4017) & 1, 1);
    assert_eq!(&b.dma[0][5..7], &[0, 0]);
}

#[test]
fn starts_follow_the_256_clock_lattice_across_its_frame_phase_wrap() {
    let mut m = machine(0, 0);
    // Independent alternating-field derivation: the H phase advances 8 then
    // 12 clocks on the 256-clock lattice, wrapping 378 -> 130.
    let offsets = [
        298, 306, 318, 326, 338, 346, 358, 366, 378, 130, 142, 150, 162,
    ];
    for (frame, h) in offsets.into_iter().enumerate() {
        let frame_start = (frame as u64 / 2) * 714732 + (frame as u64 % 2) * 357368;
        let start = frame_start + 306900 + h;
        m.set_buttons(if frame % 2 == 0 { 0xa590 } else { 0x5a60 });
        m.set_buttons2(if frame % 2 == 0 { 0x5a60 } else { 0xa590 });
        until(&mut m.bus, start - 1);
        assert_eq!(busy(&mut m.bus), 0);
        until(&mut m.bus, start);
        assert_eq!(busy(&mut m.bus), 1);
        until(&mut m.bus, start + 4224);
        if frame % 2 == 0 {
            words(&mut m.bus, 0xa590, 0x5a60);
        } else {
            words(&mut m.bus, 0x5a60, 0xa590);
        }
    }
}
