//! Original numerical tests for Issue #15. No external implementation input.
use super::*;

// Independent bit-stream oracle: b[0..13]=0, b[14]=1 and
// b[k+15]=b[k]+b[k+1] over GF(2). Pack a sliding 15-bit window.
// This constructs a complete sequence without calling the production update.
fn sequence(updates: usize) -> Vec<u16> {
    let mut bits = vec![0u16; updates + 15];
    bits[14] = 1;
    for k in 15..bits.len() {
        bits[k] = (bits[k - 15] + bits[k - 14]) % 2;
    }
    bits.windows(15)
        .map(|w| w.iter().enumerate().map(|(j, b)| b * (1 << j)).sum())
        .collect()
}

#[test]
fn noise_initial_vectors_and_nonzero_maximal_cycle() {
    let literals = [
        0x4000, 0x2000, 0x1000, 0x0800, 0x0400, 0x0200, 0x0100, 0x0080, 0x0040, 0x0020, 0x0010,
        0x0008, 0x0004, 0x0002, 0x4001, 0x6000, 0x3000, 0x1800, 0x0c00, 0x0600, 0x0300, 0x0180,
        0x00c0, 0x0060, 0x0030, 0x0018, 0x000c, 0x0006, 0x4003, 0x2001, 0x5000,
    ];
    assert_eq!(&sequence(30), &literals);
    let mut dsp = Dsp::default();
    let mut ram = vec![0; 65536];
    assert_eq!(dsp.noise, literals[0]);
    dsp.write(0x6c, 0xff); // reset + mute still advances at rate 31
    let mut seen = std::collections::HashSet::new();
    seen.insert(dsp.noise);
    for update in 1..=32767 {
        dsp.sample(&mut ram);
        assert!((1..=0x7fff).contains(&dsp.noise));
        if update < literals.len() {
            assert_eq!(dsp.noise, literals[update]);
        }
        if update < 32767 {
            assert!(seen.insert(dsp.noise), "update {update}");
        }
    }
    assert_eq!(seen.len(), 32767);
    assert_eq!(dsp.noise, 0x4000);
}

#[test]
fn actual_noise_at_all_rates_holds_between_events_over_two_counter_wraps() {
    // Independent first tick/spacing, not production periods or rate_event.
    let vectors = [
        (0, 0),
        (2048, 2048),
        (1040, 1536),
        (536, 1280),
        (1024, 1024),
        (272, 768),
        (536, 640),
        (512, 512),
        (272, 384),
        (216, 320),
        (256, 256),
        (80, 192),
        (56, 160),
        (128, 128),
        (80, 96),
        (56, 80),
        (64, 64),
        (32, 48),
        (16, 40),
        (32, 32),
        (8, 24),
        (16, 20),
        (16, 16),
        (8, 12),
        (6, 10),
        (8, 8),
        (2, 6),
        (1, 5),
        (4, 4),
        (2, 3),
        (2, 2),
        (1, 1),
    ];
    let states = sequence(61441);
    for (rate, (first, spacing)) in vectors.into_iter().enumerate() {
        let mut dsp = Dsp::default();
        let mut ram = vec![0; 65536];
        dsp.write(0x6c, 0xe0 | rate as u8);
        let mut events = Vec::new();
        let mut updates = 0;
        for tick in 1..=61441 {
            if rate != 0 && tick >= first && (tick - first) % spacing == 0 {
                updates += 1;
                events.push(tick);
            }
            dsp.sample(&mut ram);
            assert_eq!(dsp.noise, states[updates], "rate {rate}, tick {tick}");
        }
        match rate {
            0 => assert!(events.is_empty()),
            2 => assert_eq!(&events[..2], &[1040, 2576]),
            3 => assert_eq!(&events[..2], &[536, 1816]),
            29 => assert_eq!(&events[..3], &[2, 5, 8]),
            31 => assert_eq!(events.len(), 61441),
            _ => {}
        }
    }
}

#[test]
fn live_noise_rate_changes_keep_the_existing_phase() {
    let mut dsp = Dsp::default();
    let mut ram = vec![0; 65536];
    dsp.write(0x6c, 0x23);
    for _ in 0..537 {
        dsp.sample(&mut ram);
    }
    assert_eq!(dsp.noise, 0x2000); // event at 536
    dsp.write(0x6c, 0x22);
    let mut previous = 537;
    for (tick, expected) in [
        (1039, 0x2000),
        (1040, 0x1000),
        (1816, 0x1000),
        (2575, 0x1000),
        (2576, 0x800),
    ] {
        for _ in previous..tick {
            dsp.sample(&mut ram);
        }
        assert_eq!(dsp.noise, expected, "tick {tick}");
        previous = tick;
    }
    dsp.write(0x6c, 0x20);
    for _ in 0..61440 {
        dsp.sample(&mut ram);
    }
    assert_eq!(dsp.noise, 0x800);
    dsp.write(0x6c, 0x3d); // absolute tick 64016, next event 64019
    dsp.sample(&mut ram);
    assert_eq!(dsp.noise, 0x800);
    dsp.sample(&mut ram);
    assert_eq!(dsp.noise, 0x800);
    dsp.sample(&mut ram);
    assert_eq!(dsp.noise, 0x400);
}

#[test]
fn signed_noise_envelope_outx_stereo_and_echo_vectors() {
    // (state, signed 16-bit source, GAIN, even envelope sample, OUTX, PCM).
    let vectors = [
        (0x4000, -32768, 127, -32512, 129, (-32006, 32258)),
        (0x2000, 16384, 127, 16256, 63, (16002, -16129)),
        (0x0002, 4, 127, 2, 0, (0, -2)),
        (0x4001, -32766, 127, -32512, 129, (-32006, 32258)),
        (0x6000, -16384, 127, -16256, 192, (-16003, 16129)),
        (0x3000, 24576, 127, 24384, 95, (24003, -24194)),
        (0x4003, -32762, 127, -32508, 129, (-32004, 32254)),
        (0x2001, 16386, 127, 16256, 63, (16002, -16129)),
        (0x5000, -24576, 64, -12288, 208, (-12097, 12192)),
        (0x4000, -32768, 0, 0, 0, (0, 0)),
    ];
    for (state, source, gain, sample, outx, pcm) in vectors {
        let mut dsp = Dsp::default();
        let mut ram = vec![0; 65536];
        dsp.noise = state;
        assert_eq!((state << 1) as i16 as i32, source);
        dsp.voices[0].active = true;
        dsp.voices[0].env = gain as i32 * 16; // settled direct GAIN
        for (r, v) in [
            (0x6c, 0),
            (0x3d, 1),
            (7, gain),
            (0, 127),
            (1, 128),
            (0x0c, 127),
            (0x1c, 127),
            (0x4d, 1),
            (0x6d, 0x60),
        ] {
            dsp.write(r, v);
        }
        assert_eq!(dsp.sample(&mut ram), pcm);
        assert_eq!([dsp.regs[8], dsp.regs[9]], [gain, outx]);
        // EON before master volume, zero FIR/EFB. Echo RAM drops bit 0.
        // Fullsnes ESA; Anomie SOUND GENERATION; issue-71-validation.md.
        assert_eq!(
            i16::from_le_bytes([ram[0x6000], ram[0x6001]]),
            (((sample * 127) >> 7) & !1) as i16
        );
        assert_eq!(
            i16::from_le_bytes([ram[0x6002], ram[0x6003]]),
            -sample as i16
        );
        dsp.write(0x4d, 0);
        dsp.sample(&mut ram);
        assert_eq!(&ram[0x6000..0x6004], &[0; 4]);
    }
}

#[test]
fn every_non_bit_selects_only_its_voice_and_all_selected_share_one_sample() {
    for selected in 0..8 {
        let mut dsp = Dsp::default();
        let mut ram = vec![0; 65536];
        dsp.write(0x6c, 0x20);
        dsp.write(0x3d, 1 << selected);
        dsp.write(0x0c, 64);
        dsp.write(0x1c, 64);
        for i in 0..8 {
            dsp.voices[i].active = true;
            dsp.voices[i].env = 1024; // settled half envelope
            dsp.voices[i].decoded = [1024; 4];
            dsp.write(i as u8 * 16 + 7, 64);
        }
        dsp.write(selected * 16, 64);
        dsp.write(selected * 16 + 1, 0xe0);
        assert_eq!(dsp.sample(&mut ram), (-4096, 2048));
        for i in 0..8 {
            assert_eq!(
                dsp.regs[i * 16 + 9],
                if i == selected as usize { 192 } else { 4 }
            );
        }
        dsp.write(0x3d, 0); // same decoded data now supplies the selected voice
        assert_eq!(dsp.sample(&mut ram), (256, -128));
        dsp.write(0x3d, 255);
        dsp.write(0x6c, 0x3f);
        assert_eq!(dsp.sample(&mut ram), (2048, -1024)); // one update -> $2000
        for i in 0..8 {
            assert_eq!(dsp.regs[i * 16 + 9], 32);
        }
        assert_eq!(dsp.noise, 0x2000);
    }
}

fn keyed(looping: bool, pitch: u16, non: bool) -> (Dsp, Vec<u8>) {
    let mut dsp = Dsp::default();
    let mut ram = vec![0; 65536];
    ram[0x200..0x204].copy_from_slice(&[0, 3, 0, 3]);
    ram[0x300] = if looping { 3 } else { 1 };
    for (r, v) in [
        (0x6c, 0x3f),
        (0x5d, 2),
        (7, 127),
        (0x3d, u8::from(non)),
        (2, pitch as u8),
        (3, (pitch >> 8) as u8),
        (0, 64),
        (1, 64),
        (0x0c, 64),
        (0x1c, 64),
        (0x4c, 1),
    ] {
        dsp.write(r, v);
    }
    (dsp, ram)
}

#[test]
fn noise_pitch_is_independent_but_brr_decoding_loops_and_endx_continue() {
    let (mut low, mut lr) = keyed(true, 0x1000, true);
    let (mut high, mut hr) = keyed(true, 0x3000, true);
    let (mut brr, mut br) = keyed(true, 0x1000, false);
    for tick in 1..=100 {
        assert_eq!(low.sample(&mut lr), high.sample(&mut hr), "tick {tick}");
        assert_eq!(low.regs[9], high.regs[9]);
        brr.sample(&mut br);
        let a = low.voices[0];
        let b = brr.voices[0];
        assert_eq!(
            (a.addr, a.nibble, a.phase, a.header, a.active),
            (b.addr, b.nibble, b.phase, b.header, b.active)
        );
        assert_eq!(a.decoded, b.decoded);
        assert_eq!(low.regs[0x7c], brr.regs[0x7c]);
        if tick == 18 {
            assert_eq!(low.regs[0x7c], 0);
            assert_eq!(a.nibble, 16);
        }
        if tick == 19 {
            assert_eq!(low.regs[0x7c], 1);
            assert_eq!(a.addr, 0x301);
            assert_eq!(a.nibble, 1);
        }
    }
    let (mut end, mut ram) = keyed(false, 0x1000, true);
    for _ in 0..18 {
        assert_eq!(end.sample(&mut ram), (0, 0));
        assert_eq!([end.regs[8], end.regs[9], end.regs[0x7c]], [0; 3]);
    }
    assert!(end.voices[0].active);
    assert_eq!(end.sample(&mut ram), (0, 0)); // terminal completion reports ENDX after header mute
    assert_eq!([end.regs[8], end.regs[9], end.regs[0x7c]], [0, 0, 1]);
    assert!(end.voices[0].active);
    assert_eq!((end.voices[0].stage, end.voices[0].nibble), (3, 1));
    end.write(0x7c, 0);
    for _ in 0..16 {
        assert_eq!(end.sample(&mut ram), (0, 0));
    }
    assert_eq!(end.regs[0x7c], 1);
    let state = end.noise;
    end.sample(&mut ram);
    assert_ne!(end.noise, state);
}

#[test]
fn silence_non_mute_key_delay_release_and_voice_reset_do_not_reseed() {
    let states = sequence(400);
    let (mut dsp, mut ram) = keyed(true, 0x1000, true);
    for (tick, &expected) in states.iter().enumerate().take(301).skip(1) {
        match tick {
            8 => dsp.write(0x3d, 0),
            10 => dsp.write(0x3d, 1),
            12 => dsp.write(0x6c, 0x7f), // mute, keep voice processing
            14 => dsp.write(0x6c, 0x3f),
            16 => dsp.write(0x5c, 1), // 2032 / 8 = 254 release samples
            275 => dsp.write(0x5c, 0),
            276 => dsp.write(0x4c, 1),
            285 => dsp.write(0x6c, 0xbf), // voice reset, rate still 31
            290 => {
                dsp.write(0x6c, 0x3f);
                dsp.write(0x4c, 1);
            }
            _ => {}
        }
        let pcm = dsp.sample(&mut ram);
        assert_eq!(dsp.noise, expected, "tick {tick}");
        if (1..=5).contains(&tick) || (270..=280).contains(&tick) || (285..=294).contains(&tick) {
            assert_eq!([dsp.regs[8], dsp.regs[9]], [0, 0], "tick {tick}");
            assert_eq!(pcm, (0, 0));
        }
        if tick == 269 {
            // Independent bit-stream recurrence gives state $5801, signed
            // source -20478. E_apply=8 -> O=-80, half/half volumes -> -20.
            assert_eq!(expected, 0x5801);
            assert_eq!([dsp.regs[8], dsp.regs[9]], [0, 255]);
            assert_eq!(pcm, (-20, -20));
        }
        if tick == 12 || tick == 13 {
            assert_eq!(pcm, (0, 0));
            assert_eq!(dsp.voices[0].env, 2032);
        }
        if (16..=269).contains(&tick) {
            assert_eq!(dsp.voices[0].env, 2032 - (tick as i32 - 15) * 8);
        }
    }
    assert_eq!(Dsp::default().noise, 0x4000);
    assert_eq!(Dsp::default().counter, 0);
}

#[test]
fn mixed_brr_and_noise_mix_numerically_and_eon_selects_each_source() {
    let mut dsp = Dsp::default();
    let mut ram = vec![0; 65536];
    dsp.noise = 0x2000; // signed source +16384
    ram[0x200..0x204].copy_from_slice(&[0, 3, 0, 3]);
    ram[0x300] = 0x93; // original range-9, filter-0, end+loop block
    ram[0x301..0x309].fill(0x44); // constant decoded +1024
    for i in 0..2 {
        for (r, v) in [(0, 64), (1, 0xe0), (3, 0x10), (7, 64)] {
            dsp.write(i as u8 * 16 + r, v);
        }
    }
    for (r, v) in [
        (0x6c, 0),
        (0x3d, 1),
        (0x0c, 64),
        (0x1c, 64),
        (0x4d, 1),
        (0x6d, 0x60),
        (0x5d, 2),
        (0x4c, 3),
    ] {
        dsp.write(r, v);
    }
    for _ in 0..5 {
        assert_eq!(dsp.sample(&mut ram), (0, 0));
    }
    for _ in 0..20 {
        dsp.sample(&mut ram);
    }
    // Half envelopes -> +8192 noise, +1024 BRR; both contribute to PCM.
    assert_eq!(dsp.sample(&mut ram), (2304, -1152));
    assert_eq!([dsp.regs[9], dsp.regs[25]], [32, 4]);
    assert_eq!(&ram[0x6000..0x6004], &[0, 16, 0, 248]); // +4096/-2048 noise only
    dsp.write(0x4d, 2);
    assert_eq!(dsp.sample(&mut ram), (2304, -1152));
    assert_eq!(&ram[0x6000..0x6004], &[0, 2, 0, 255]); // +512/-256 BRR only
}
