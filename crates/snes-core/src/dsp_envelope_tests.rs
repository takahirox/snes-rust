//! Original numerical vectors; no production arithmetic generates expectations.
use super::*;

fn keyed(gain: u8) -> (Dsp, Vec<u8>) {
    let mut dsp = Dsp::default();
    let mut ram = vec![0; 65536];
    for (r, v) in [
        (0x6c, 0x20),
        (0x3d, 1),
        (0, 64),
        (1, 64),
        (0x0c, 64),
        (0x1c, 64),
        (7, gain),
        (0x4c, 1),
    ] {
        dsp.write(r, v);
    }
    for _ in 0..5 {
        assert_eq!(dsp.sample(&mut ram), (0, 0));
        assert_eq!((dsp.voices[0].env, dsp.voices[0].hidden_env), (0, 0));
    }
    dsp.sample(&mut ram);
    (dsp, ram)
}

fn check(dsp: &mut Dsp, ram: &mut [u8], env: i32, hidden: i32, pcm: i16, outx: u8) {
    assert_eq!(dsp.sample(ram), (pcm, pcm));
    assert_eq!((dsp.voices[0].env, dsp.voices[0].hidden_env), (env, hidden));
    assert_eq!(dsp.regs[8], (env >> 4) as u8);
    assert_eq!(dsp.regs[9], outx);
}

#[test]
fn direct_to_bent_threshold_and_consecutive_holds_are_numerical() {
    // NON=$4000 -> signed source=-32768; post-envelope=-16E;
    // both volumes=64 -> stereo=-4E; OUTX is the signed sample's high byte.
    for (direct, held, env, pcm, outx) in [
        (93, &[1520, 1520, 1520][..], 1520, -6080, 161), // below
        (94, &[1536][..], 1512, -6048, 161),             // exactly at H=$600
        (94, &[1536, 1512][..], 1536, -6144, 160),       // H oscillates, E holds
        (94, &[1536, 1512, 1536][..], 1512, -6048, 161),
        (96, &[1544, 1544][..], 1544, -6176, 159), // at E=$600
        (97, &[1560, 1560][..], 1560, -6240, 158), // above
    ] {
        for rate in [0, 1, 3] {
            // all are holds at these absolute ticks
            let (mut dsp, mut ram) = keyed(direct);
            dsp.write(7, 0xe0 | rate);
            for &hidden in held {
                dsp.sample(&mut ram);
                assert_eq!(dsp.voices[0].env, direct as i32 * 16);
                assert_eq!(dsp.voices[0].hidden_env, hidden);
            }
            dsp.write(7, 0xff);
            check(&mut dsp, &mut ram, env, env, pcm, outx);
        }
    }
}

#[test]
fn preclamp_negative_and_overflow_candidates_feed_bent_gain() {
    for (direct, gain, env, hidden, bent, pcm, outx) in [
        (1, 0x80, 16, -16, 24, -96, 254), // underflow on rate-zero hold
        (1, 0x9f, 0, -16, 8, -32, 255),   // lower clamp, negative H is unsigned
        (126, 0xc0, 2016, 2048, 2024, -8096, 129), // no 11-bit masking
        (127, 0xdf, 2047, 2064, 2047, -8188, 128), // upper saturation
        (96, 0xa0, 1536, 1530, 1568, -6272, 158), // exp decrease -> large step
        (96, 0x80, 1536, 1504, 1568, -6272, 158), // linear decrease -> large step
    ] {
        let (mut dsp, mut ram) = keyed(direct);
        dsp.write(7, gain);
        dsp.sample(&mut ram);
        assert_eq!((dsp.voices[0].env, dsp.voices[0].hidden_env), (env, hidden));
        dsp.write(7, 0xff);
        check(
            &mut dsp,
            &mut ram,
            bent,
            if bent == 2047 { 2055 } else { bent },
            pcm,
            outx,
        );
    }
}

#[test]
fn adsr_candidates_and_phases_update_before_rate_commitment() {
    let (mut dsp, mut ram) = keyed(94);
    dsp.write(5, 0x80); // slow attack holds E=1504, prepares H=1536
    check(&mut dsp, &mut ram, 1504, 1536, -6016, 162);
    dsp.write(5, 0);
    dsp.write(7, 0xff);
    check(&mut dsp, &mut ram, 1512, 1512, -6048, 161);

    let (mut dsp, mut ram) = keyed(127);
    dsp.write(5, 0x80);
    check(&mut dsp, &mut ram, 2032, 2064, -8128, 129);
    assert_eq!(dsp.voices[0].stage, 1); // attack overflow despite held E
    dsp.write(6, 0xe0); // decay candidate 2024 is in sustain band 7
    check(&mut dsp, &mut ram, 2032, 2024, -8128, 129);
    assert_eq!(dsp.voices[0].stage, 2);
    dsp.write(5, 0);
    dsp.write(7, 0xff);
    check(&mut dsp, &mut ram, 2040, 2040, -8160, 128);

    let (mut dsp, mut ram) = keyed(1);
    dsp.write(7, 0x80); // negative candidate changes Attack -> Decay even in GAIN
    check(&mut dsp, &mut ram, 16, -16, -64, 255);
    assert_eq!(dsp.voices[0].stage, 1);
    dsp.write(7, 0); // direct GAIN supplies sustain level too
    check(&mut dsp, &mut ram, 0, 0, 0, 0);
    assert_eq!(dsp.voices[0].stage, 2);
    dsp.write(5, 0x8f); // returning to ADSR resumes sustain, not fast attack
    check(&mut dsp, &mut ram, 0, 0, 0, 0);
}

#[test]
fn release_retrigger_soft_reset_and_reconstruction_isolate_hidden_state() {
    let (mut dsp, mut ram) = keyed(94);
    dsp.write(7, 0xe0);
    check(&mut dsp, &mut ram, 1504, 1536, -6016, 162);
    dsp.write(0x5c, 1);
    check(&mut dsp, &mut ram, 1496, 1536, -5984, 162);
    dsp.write(7, 127); // release overrides direct GAIN and retains H
    check(&mut dsp, &mut ram, 1488, 1536, -5952, 163);
    dsp.write(0x5c, 0);
    dsp.write(7, 0xff);
    dsp.write(0x4c, 1);
    for _ in 0..5 {
        check(&mut dsp, &mut ram, 0, 0, 0, 0);
    }
    check(&mut dsp, &mut ram, 32, 32, -128, 254);
    dsp.write(0x6c, 0xe0);
    check(&mut dsp, &mut ram, 0, 32, 0, 0);
    dsp.write(0x6c, 0x20);
    check(&mut dsp, &mut ram, 0, 32, 0, 0); // inactive until KON
    dsp.write(0x4c, 1);
    for _ in 0..5 {
        check(&mut dsp, &mut ram, 0, 0, 0, 0);
    }
    check(&mut dsp, &mut ram, 32, 32, -128, 254);
    dsp.write(0x5c, 1);
    for env in [24, 16, 8, 0] {
        dsp.sample(&mut ram);
        assert_eq!(dsp.voices[0].env, env);
        assert_eq!(dsp.voices[0].hidden_env, 32);
    }
    assert!(dsp.voices[0].active); // release silence does not stop BRR processing
    assert_eq!(dsp.voices[0].stage, 3);
    dsp = Dsp::default();
    assert_eq!(dsp.counter, 0);
    assert!(dsp.voices.iter().all(|v| v.env == 0 && v.hidden_env == 0));
}

#[test]
fn bent_nonzero_rate_commits_the_candidate_after_many_holds() {
    let (mut dsp, mut ram) = keyed(94); // tick 6, E=H=1504
    dsp.write(7, 0xe3);
    for tick in 7..=535 {
        dsp.sample(&mut ram);
        assert_eq!(dsp.voices[0].env, 1504);
        assert_eq!(
            dsp.voices[0].hidden_env,
            if tick % 2 == 1 { 1536 } else { 1512 }
        );
    }
    check(&mut dsp, &mut ram, 1512, 1512, -6048, 161); // rate-3 event 536
    check(&mut dsp, &mut ram, 1512, 1544, -6048, 161); // hold, from E not H
}

#[test]
fn attack_phase_uses_overflow_rather_than_fullsnes_7e0_threshold() {
    let (mut dsp, mut ram) = keyed(124); // E=1984
    dsp.write(5, 0x80);
    check(&mut dsp, &mut ram, 1984, 2016, -7936, 132);
    assert_eq!(dsp.voices[0].stage, 0); // candidate $7e0 is not overflow
}
