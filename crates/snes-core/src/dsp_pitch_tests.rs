//! Original MIT numerical PMON vectors. Expectations are independently derived.
use super::*;

fn setup() -> (Dsp, Vec<u8>) {
    let mut d = Dsp::default();
    d.write(0x6c, 0x20);
    let mut ram = vec![0; 65536];
    ram[0x300] = 0x93;
    ram[0x301..0x309].copy_from_slice(&[0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);
    for i in 0..8 {
        d.voices[i] = Voice {
            active: true,
            addr: 0x300,
            loop_addr: 0x300,
            ..Voice::default()
        };
        d.write(i as u8 * 16 + 7, 64);
    }
    (d, ram)
}

#[test]
fn signed_arithmetic_anchors_preserve_both_rounding_steps() {
    for (p, previous, expected) in [
        (0x1000, 0, 0x1000),
        (0x1000, -32768, 0),
        (0x1000, -32512, 0x20),
        (0x1000, 32766, 0x1ffc),
        (0x1001, -2, 0xffc),
        (0x3fff, 32766, 0x7fee),
        (0, 32766, 0),
        (0, -32768, 0),
        (0x1001, 34, 0x1005),
        (0x1001, -34, 0xff8),
    ] {
        assert_eq!(modulated_pitch(p, previous), expected, "{p:x}, {previous}");
    }
}

#[test]
fn every_pmon_bit_and_no_wrap_use_only_the_immediate_predecessor() {
    for selected in 0..8 {
        let (mut d, mut ram) = setup();
        // At fractional zero: constant decoded 1024 -> source 2048,
        // half envelope -> 1024, modulation at pitch $1000 adds $80.
        for i in 0..8 {
            d.voices[i].decoded = [1024; 4];
            d.write(i as u8 * 16 + 2, 0);
            d.write(i as u8 * 16 + 3, 0x10);
        }
        d.write(0x2d, 1 << selected);
        // Keep decoded windows constant when consuming the first nibble.
        ram[0x301..0x309].fill(0x44);
        d.sample(&mut ram);
        for i in 0..8 {
            assert_eq!(
                d.voices[i].phase,
                if i == selected && i > 0 {
                    0x1080
                } else {
                    0x1000
                }
            );
            assert_eq!(d.voices[i].nibble, 1);
        }
        assert_eq!(d.regs[0x2d], 1 << selected);
    }
}

#[test]
fn chains_use_current_sample_and_zero_voices_break_the_chain() {
    let (mut d, mut ram) = setup();
    d.write(0x2d, 0xfe);
    // Voice0's new BRR nibble shifts [0,1024,0,0] to [1024,0,0,0]:
    // source 370, half envelope 184; voice1 increment $1014 (184>>5=5).
    d.voices[0].decoded = [0, 1024, 0, 0];
    d.voices[0].addr = 0x400;
    d.write(3, 0x10);
    for i in 1..8 {
        d.write(i * 16 + 3, 0x10);
    }
    d.voices[1].decoded = [1024; 4];
    ram[0x301] = 0x44;
    // Voice1's source at index1: coefficients 366,1305,378,0,
    // sum for constant1024=2049 -> even2048 -> envelope1024.
    // Voice2 then receives +1024, despite its zero output breaking voice3.
    d.sample(&mut ram);
    assert_eq!(d.voices[1].phase, 0x1014);
    assert_eq!(d.voices[2].phase, 0x1080);
    assert_eq!(d.voices[3].phase, 0x1000);
    // Voice0 shifts the impulse out of the interpolation window:
    // new source zero, so voice1 advances by exactly $1000 (not old $1014).
    d.sample(&mut ram);
    assert_eq!(d.voices[1].phase, 0x2014);
}

#[test]
fn full_resolution_even_output_precedes_voice_volumes_and_final_mute() {
    // Both predecessors publish OUTX=4, but low bits cross a >>5 boundary.
    // Gaussian at zero sums 2049 for constant1024, then truncates even.
    for (decoded, increment) in [(1024, 0x1080), (1058, 0x1084)] {
        for (volume, flag) in [(0, 0x20), (127, 0x20), (128, 0x60)] {
            let (mut d, mut ram) = setup();
            d.voices[0].decoded = [decoded; 4];
            d.write(0, volume);
            d.write(1, volume);
            d.write(0x6c, flag);
            d.write(0x2d, 2);
            d.write(0x13, 0x10);
            d.sample(&mut ram);
            assert_eq!(d.regs[9], 4);
            assert_eq!(d.voices[1].phase, increment);
        }
    }
}

#[test]
fn inactive_delay_zero_envelope_release_and_retrigger_supply_correct_output() {
    for state in 0..5 {
        let (mut d, mut ram) = setup();
        d.write(0x2d, 6);
        d.write(0x13, 0x10);
        d.write(0x23, 0x10);
        d.voices[0].decoded = [1024; 4];
        d.voices[1].decoded = [1024; 4];
        match state {
            0 => d.voices[1].active = false,
            1 => d.voices[1].key_delay = 1,
            2 => d.write(0x17, 0),
            3 => {
                d.voices[1].env = 1024;
                d.write(0x5c, 2);
            }
            _ => {
                d.write(0x4c, 2);
            }
        }
        d.sample(&mut ram);
        // Release: env1016; window after zero nibble [1024,1024,1024,0],
        // phase $80 index8 -> source2048, envelope1016 -> step $107C.
        assert_eq!(d.voices[2].phase, if state == 3 { 0x107c } else { 0x1000 });
        if state != 3 {
            assert_eq!(d.regs[25], 0);
        }
    }
}

#[test]
fn bounded_position_saturates_and_scalar_decode_consumes_only_new_steps() {
    let (mut d, mut ram) = setup();
    d.write(0x3d, 1);
    d.noise = 0x3fff; // source32766, held dynamic gain env2047
    d.voices[0].env = 2047;
    d.write(7, 0x80);
    d.write(0x2d, 2);
    d.write(0x12, 0xff);
    d.write(0x13, 0xff);
    // Actual predecessor32750 (after env/even) still >>5=1023, step$7FEE.
    d.write(0x10, 64);
    d.write(0x11, 128);
    d.write(0x0c, 64);
    d.write(0x1c, 64);
    d.voices[1].phase = 0x3fff;
    assert_eq!(d.sample(&mut ram), (127, -255));
    assert_eq!(d.regs[25], 1);
    assert_eq!(d.voices[1].phase, 0x7fff);
    assert_eq!(d.voices[1].nibble, 4); // seven minus three, NOT eleven samples
    assert_eq!(d.voices[1].decoded, [0, 256, 512, 768]);
    assert_eq!([d.regs[0x12], d.regs[0x13]], [255, 255]);
    assert_eq!(d.sample(&mut ram), (383, -767));
    assert_eq!(d.regs[25], 5);
    assert_eq!(d.voices[1].phase, 0x7fff);
    assert_eq!(d.voices[1].nibble, 8);
    assert_eq!(d.voices[1].decoded, [1024, 1280, 1536, 1792]);
    d.write(0x12, 0);
    d.write(0x13, 0xc0); // zero masked pitch; no decode
    d.sample(&mut ram);
    assert_eq!(d.voices[1].phase, 0x3fff);
    assert_eq!(d.voices[1].nibble, 8);
}

#[test]
fn live_enable_disable_preserves_phase_envelope_history_and_shared_clock() {
    let (mut d, mut ram) = setup();
    d.voices[0].decoded = [1024; 4];
    d.write(0x13, 0x10);
    for (pmon, phase, nibble) in [
        (0, 0x1000, 1),
        (2, 0x2080, 2),
        (0, 0x3080, 3),
        (2, 0x4100, 4),
    ] {
        let counter = d.counter;
        let noise = d.noise;
        d.write(0x2d, pmon);
        assert_eq!(d.counter, counter);
        assert_eq!(d.noise, noise);
        d.sample(&mut ram);
        assert_eq!(d.voices[1].phase, phase);
        assert_eq!(d.voices[1].nibble, nibble);
        assert_eq!(d.voices[1].env, 1024);
        assert_eq!(d.voices[1].key_delay, 0);
        assert_eq!(d.counter, 30720 - nibble as u32);
    }
}

#[test]
fn noise_predecessor_and_target_keep_shared_noise_and_brr_end_processing() {
    for looping in [false, true] {
        let (mut d, mut ram) = setup();
        ram[0x300] = if looping { 0x93 } else { 0x91 };
        d.write(0x3d, 3);
        d.noise = 0x2000; // source16384 -> envelope8192
        d.write(0x2d, 2);
        d.write(0x13, 0x10); // increment$1400
        for (tick, phase, nibble, end) in [
            (1, 0x1400, 1, 0),
            (4, 0x5000, 5, 0),
            (12, 0x3000, 15, 0),
            (13, 0x4400, 16, 0),
            (14, 0x1800, if looping { 1 } else { 16 }, 2),
        ] {
            let already = (30720 - d.counter) % 30720;
            for _ in already..tick {
                d.sample(&mut ram);
            }
            assert_eq!(d.voices[1].phase, phase);
            assert_eq!(d.voices[1].nibble, nibble);
            assert_eq!(d.regs[0x7c], end);
            assert_eq!(d.regs[25], if !looping && tick == 14 { 0 } else { 32 });
            assert_eq!(d.noise, 0x2000);
        }
    }
}

#[test]
fn pmon_does_not_change_advancing_shared_noise_or_its_audible_target() {
    let (mut on, mut a) = setup();
    let (mut off, mut b) = setup();
    for d in [&mut on, &mut off] {
        d.write(0x3d, 3);
        d.write(0x6c, 0x3f);
        d.write(0x13, 0x10);
        d.write(0x10, 64);
        d.write(0x11, 128);
        d.write(0x0c, 64);
        d.write(0x1c, 64);
    }
    let states = [
        0x2000, 0x1000, 0x0800, 0x0400, 0x0200, 0x0100, 0x0080, 0x0040, 0x0020, 0x0010, 0x0008,
        0x0004, 0x0002, 0x4001, 0x6000, 0x3000,
    ];
    for (index, state) in states.into_iter().enumerate() {
        // Enable, disable and reenable while the shared generator keeps moving.
        on.write(0x2d, if (5..9).contains(&index) { 0 } else { 2 });
        assert_eq!(on.sample(&mut a), off.sample(&mut b));
        assert_eq!(on.noise, state);
        assert_eq!(off.noise, state);
        assert_eq!(on.counter, 30719 - index as u32);
        assert_eq!(on.counter, off.counter);
        assert_eq!(on.regs[25], off.regs[25]);
        if index == 0 {
            // Noise +16384, half envelope8192, target increment$1400.
            assert_eq!(on.voices[1].phase, 0x1400);
            assert_eq!(off.voices[1].phase, 0x1000);
            assert_eq!(on.regs[25], 32);
        }
    }
}
