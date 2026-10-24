//! Original MIT vectors, calculated in docs/issue-71-validation.md.
use super::Dsp;

fn setup(coefficients: [i8; 8], delay: u8, flags: u8) -> (Dsp, Vec<u8>) {
    let mut dsp = Dsp::default();
    for (r, v) in [
        (0x6c, flags),
        (0x6d, 0x60),
        (0x7d, delay),
        (0x2c, 64),
        (0x3c, 64),
    ] {
        dsp.write(r, v);
    }
    for (k, c) in coefficients.into_iter().enumerate() {
        dsp.write((k * 16 + 15) as u8, c as u8);
    }
    (dsp, vec![0; 65536])
}
fn put(ram: &mut [u8], addr: usize, pair: (i16, i16)) {
    for (offset, byte) in pair
        .0
        .to_le_bytes()
        .into_iter()
        .chain(pair.1.to_le_bytes())
        .enumerate()
    {
        ram[(addr + offset) & 65535] = byte;
    }
}
fn words(ram: &[u8], addr: usize) -> (i16, i16) {
    let word = |a: usize| i16::from_le_bytes([ram[a & 65535], ram[(a + 1) & 65535]]);
    (word(addr), word(addr + 2))
}

#[test]
fn every_tap_has_its_own_age_and_signed_stereo_rounding() {
    for tap in 0..8 {
        let mut coeff = [0; 8];
        coeff[tap] = 127;
        let (mut dsp, mut ram) = setup(coeff, 1, 0x20);
        dsp.write(0x2c, 128);
        dsp.write(0x3c, 128);
        put(&mut ram, 0x6000, (131, -131));
        for n in 0..17 {
            // floor(131/2)=65, floor(-131/2)=-66;
            // floor(*127/64)=128,-131; even FIR=128,-132.
            let expected = if n == 7 - tap { (-128, 132) } else { (0, 0) };
            assert_eq!(dsp.sample(&mut ram), expected, "tap {tap}, sample {n}");
            if n == 0 {
                assert_eq!(dsp.echo_hist[0], [65, -66]);
            }
        }
        assert_eq!(words(&ram, 0x6000), (131, -131));
    }
}

#[test]
fn asymmetric_impulses_overlap_across_history_wrap() {
    let (mut dsp, mut ram) = setup([1, 2, 3, 4, 5, 6, 7, 8], 1, 0x20);
    put(&mut ram, 0x6000, (16384, -8192));
    put(&mut ram, 0x600c, (8192, 4096));
    let left = [512, 448, 384, 576, 480, 384, 288, 192, 96, 64, 32, 0];
    let right = [-256, -224, -192, -32, -16, 0, 16, 32, 48, 32, 16, 0];
    for (l, r) in left.into_iter().zip(right) {
        assert_eq!(dsp.sample(&mut ram), (l, r));
    }
    assert_eq!((dsp.echo_index, dsp.echo_pos), (4, 48));
}

#[test]
fn issue_impulse_reproduction() {
    let (mut dsp, mut ram) = setup([1, 2, 3, 4, 5, 6, 7, 8], 1, 0x20);
    put(&mut ram, 0x6000, (16384, 0));
    for expected in [512, 448, 384, 320, 256, 192, 128, 64, 0] {
        assert_eq!(dsp.sample(&mut ram), (expected, 0));
    }
}

#[test]
fn coefficient_minus_128_and_volume_feedback_endpoint_narrowing() {
    for tap in [0, 7] {
        let mut coeff = [0; 8];
        coeff[tap] = -128;
        let (mut dsp, mut ram) = setup(coeff, 1, 0);
        dsp.write(0x0d, 128);
        dsp.write(0x2c, 128);
        dsp.write(0x3c, 128);
        put(&mut ram, 0x6000, (-32768, 32767));
        for n in 0..8 {
            let expected = if n == 7 - tap {
                (-32768, 32766)
            } else {
                (0, 0)
            };
            assert_eq!(dsp.sample(&mut ram), expected);
            assert_eq!(words(&ram, 0x6000 + n * 4), expected);
        }
    }
}

#[test]
fn seven_tap_wrap_and_final_tap_saturation_have_both_signs() {
    for (coeff, input, pcm, ram_bytes) in [
        (
            [127, 127, 127, 127, 127, 127, 127, 64],
            (16384, -16384),
            (-4544, 4544),
            [0x40, 0xee, 0xc0, 0x11],
        ),
        (
            [16, 16, 16, 16, 16, 16, 16, 127],
            (32766, -32768),
            (16383, -16384),
            [0xfe, 0x3f, 0, 0xc0],
        ),
    ] {
        let (mut dsp, mut ram) = setup(coeff, 1, 0);
        dsp.write(0x0d, 64);
        for n in 0..8 {
            put(&mut ram, 0x6000 + n * 4, input);
        }
        for _ in 0..7 {
            dsp.sample(&mut ram);
        }
        assert_eq!(dsp.sample(&mut ram), pcm);
        assert_eq!(&ram[0x601c..0x6020], &ram_bytes);
    }
}

#[test]
fn positive_and_negative_feedback_recur_with_even_writeback() {
    for (efb, pcm, stored) in [
        (
            64,
            [(4096, -2049), (1024, -513), (256, -129), (64, -33)],
            [(4096, -2050), (1024, -514), (256, -130), (64, -34)],
        ),
        (
            192,
            [(4096, -2049), (-1024, 512), (256, -128), (-64, 32)],
            [(-4096, 2048), (1024, -512), (-256, 128), (64, -32)],
        ),
    ] {
        let (mut dsp, mut ram) = setup([0, 0, 0, 0, 0, 0, 0, 64], 0, 0);
        dsp.write(0x0d, efb);
        put(&mut ram, 0x6000, (16386, -8194));
        for (output, word) in pcm.into_iter().zip(stored) {
            assert_eq!(dsp.sample(&mut ram), output);
            assert_eq!(words(&ram, 0x6000), word);
            assert_eq!(ram[0x6000] & 1, 0);
            assert_eq!(ram[0x6002] & 1, 0);
            assert_eq!(dsp.echo_pos, 0);
        }
    }
}

// One held noise voice: source -32768, half envelope -16384,
// voice volumes -128/64 -> bounded dry/send 16384/-8192.
fn voice(dsp: &mut Dsp) {
    dsp.voices[0].active = true;
    dsp.voices[0].env = 1024; // settled E_apply, independent of this call

    for (r, v) in [
        (0x3d, 1),
        (7, 64),
        (0, 128),
        (1, 64),
        (0x0c, 64),
        (0x1c, 64),
    ] {
        dsp.write(r, v);
    }
}

#[test]
fn bounded_send_feedback_saturation_dry_composition_and_zero_controls() {
    for (coeff, efb, eon, evol, pcm, stored) in [
        (127, 127, 1, 127, (32767, -32768), (32766, -32768)),
        (127, 127, 0, 127, (32767, -32768), (32256, -32258)),
        (127, 128, 1, 128, (-24318, 28416), (-16126, 24320)),
        (64, 0, 1, 0, (8192, -4096), (16384, -8192)),
        (0, 127, 1, 127, (8192, -4096), (16384, -8192)),
        (64, 0, 0, 64, (16383, -12288), (0, 0)),
    ] {
        let (mut dsp, mut ram) = setup([0, 0, 0, 0, 0, 0, 0, coeff], 0, 0);
        voice(&mut dsp);
        for (r, v) in [(0x0d, efb), (0x4d, eon), (0x2c, evol), (0x3c, evol)] {
            dsp.write(r, v);
        }
        put(&mut ram, 0x6000, (32766, -32768));
        assert_eq!(dsp.sample(&mut ram), pcm);
        assert_eq!(words(&ram, 0x6000), stored);
        assert_eq!([dsp.regs[8], dsp.regs[9]], [64, 192]);
    }
}

#[test]
fn write_disable_and_output_mute_preserve_history_and_address_progression() {
    let (mut audible, mut a) = setup([1, 2, 3, 4, 5, 6, 7, 8], 1, 0x20);
    let (mut muted, mut b) = setup([1, 2, 3, 4, 5, 6, 7, 8], 1, 0x60);
    put(&mut a, 0x6000, (16384, -8192));
    b.copy_from_slice(&a);
    for expected in [(512, -256), (448, -224), (384, -192)] {
        assert_eq!(audible.sample(&mut a), expected);
        assert_eq!(muted.sample(&mut b), (0, 0));
        assert_eq!(audible.echo_hist, muted.echo_hist);
        assert_eq!(
            (audible.echo_pos, audible.echo_index),
            (muted.echo_pos, muted.echo_index)
        );
        assert_eq!(a, b);
    }
    muted.write(0x6c, 0x20);
    assert_eq!(muted.sample(&mut b), (320, -160));
    // Mute with writes enabled still updates RAM and feeds the next read.
    let (mut dsp, mut ram) = setup([0, 0, 0, 0, 0, 0, 0, 64], 0, 0x40);
    dsp.write(0x0d, 64);
    put(&mut ram, 0x6000, (16386, -8194));
    assert_eq!(dsp.sample(&mut ram), (0, 0));
    assert_eq!(words(&ram, 0x6000), (4096, -2050));
    dsp.write(0x6c, 0);
    assert_eq!(dsp.sample(&mut ram), (1024, -513));
}

#[test]
fn fixed_nonzero_delay_and_16_bit_ram_address_wrap() {
    let (mut dsp, mut ram) = setup([0, 0, 0, 0, 0, 0, 0, 64], 1, 0);
    dsp.write(0x6d, 0xff);
    dsp.write(0x0d, 64);
    put(&mut ram, 0xff00, (16384, -8192));
    put(&mut ram, 0, (4096, -2048));
    for n in 0..1025 {
        let expected = match n {
            0 => (4096, -2048),
            64 => (1024, -512),
            512 => (1024, -512),
            576 => (256, -128),
            1024 => (256, -128),
            _ => (0, 0),
        };
        assert_eq!(dsp.sample(&mut ram), expected, "sample {n}");
        if n == 63 {
            assert_eq!(dsp.echo_pos, 256);
        }
        if n == 64 {
            assert_eq!(words(&ram, 0), (1024, -512));
        }
        if n == 511 {
            assert_eq!(dsp.echo_pos, 0);
        }
    }
    assert_eq!(words(&ram, 0xff00), (256, -128));
}

#[test]
fn write_disable_keeps_nonzero_feedback_history_until_writes_resume() {
    let (mut dsp, mut ram) = setup([64, 0, 0, 0, 0, 0, 0, 64], 0, 0x20);
    dsp.write(0x0d, 64);
    put(&mut ram, 0x6000, (16384, -8192));
    for _ in 0..7 {
        assert_eq!(dsp.sample(&mut ram), (4096, -2048));
        assert_eq!(words(&ram, 0x6000), (16384, -8192));
    }
    assert_eq!(dsp.sample(&mut ram), (8192, -4096));
    assert_eq!(words(&ram, 0x6000), (16384, -8192));
    dsp.write(0x6c, 0);
    assert_eq!(dsp.sample(&mut ram), (8192, -4096));
    assert_eq!(words(&ram, 0x6000), (8192, -4096));
    assert_eq!(dsp.sample(&mut ram), (6144, -3072));
    assert_eq!(words(&ram, 0x6000), (6144, -3072));
}

#[test]
fn bounded_send_feedback_saturates_over_multiple_recurrences() {
    let (mut dsp, mut ram) = setup([0, 0, 0, 0, 0, 0, 0, 127], 0, 0);
    voice(&mut dsp);
    for (r, v) in [(0x4d, 1), (0x0d, 127), (0x2c, 127), (0x3c, 127)] {
        dsp.write(r, v);
    }
    put(&mut ram, 0x6000, (32766, -32768));
    for _ in 0..4 {
        assert_eq!(dsp.sample(&mut ram), (32767, -32768));
        assert_eq!(&ram[0x6000..0x6004], &[0xfe, 0x7f, 0, 0x80]);
    }
}
