//! Public-register discriminator, original MIT numerical expectations.
use snes_core::dsp::Dsp;

#[test]
fn held_bent_candidate_changes_next_committed_envx_and_pcm() {
    let mut dsp = Dsp::default();
    let mut ram = vec![0; 65536];
    for (r, v) in [
        (0x6c, 0x20),
        (0x3d, 1),
        (0, 64),
        (1, 64),
        (0x0c, 64),
        (0x1c, 64),
        (7, 94),
        (0x4c, 1),
    ] {
        dsp.write(r, v);
    }
    for _ in 0..5 {
        assert_eq!(dsp.sample(&mut ram), (0, 0));
    }
    assert_eq!(dsp.sample(&mut ram), (-6016, -6016)); // E=1504
    assert_eq!((dsp.regs[8], dsp.regs[9]), (94, 162));
    dsp.write(7, 0xe0);
    assert_eq!(dsp.sample(&mut ram), (-6016, -6016)); // E holds, H=1536
    dsp.write(7, 0xff);
    assert_eq!(dsp.sample(&mut ram), (-6048, -6048)); // E=1504+8
    assert_eq!((dsp.regs[8], dsp.regs[9]), (94, 161));
}

#[test]
fn hidden_transitions_match_every_sample_and_register_across_apu_partitions_and_wrap() {
    use snes_core::apu::Apu;
    fn configured() -> Apu {
        let mut apu = Apu::default();
        for (r, v) in [
            (0x6c, 0x20),
            (0x3d, 1),
            (0, 64),
            (1, 64),
            (0x0c, 64),
            (0x1c, 64),
            (7, 94),
            (0x4c, 1),
        ] {
            apu.dsp.write(r, v);
        }
        apu
    }
    let mut whole = configured();
    let mut split = configured();
    let mut master = 0;
    let mut previous_tick = 0;
    // Writes occur at the previous checkpoint, identically on both APUs.
    // Each interval has constant E and literal expected stereo PCM=-4E.
    for (tick, writes, env, pcm, outx) in [
        (5, &[][..], 0, 0, 0),
        (6, &[][..], 1504, -6016, 162),
        (7, &[(7, 0xe0)][..], 1504, -6016, 162),
        (8, &[(7, 0xff)][..], 1512, -6048, 161),
        (9, &[(7, 96)][..], 1536, -6144, 160),
        (10, &[(7, 0xa0)][..], 1536, -6144, 160),
        (11, &[(7, 0xff)][..], 1568, -6272, 158),
        (12, &[(7, 127)][..], 2032, -8128, 129),
        (13, &[(7, 0xc0)][..], 2032, -8128, 129),
        (14, &[(7, 0xff)][..], 2040, -8160, 128),
        (15, &[][..], 2047, -8188, 128),
        (16, &[(7, 1)][..], 16, -64, 255),
        (17, &[(7, 0x80)][..], 16, -64, 255),
        (18, &[(7, 0xff)][..], 24, -96, 254),
        (19, &[(0x5c, 1)][..], 16, -64, 255),
        (20, &[][..], 8, -32, 255),
        (21, &[][..], 0, 0, 0),
        (26, &[(0x5c, 0), (7, 0xe0), (0x4c, 1)][..], 0, 0, 0),
        (27, &[][..], 0, 0, 0),
        (28, &[(7, 0xff)][..], 32, -128, 254),
        (29, &[(7, 94)][..], 1504, -6016, 162),
        (30719, &[(7, 0xc0)][..], 1504, -6016, 162),
        (30720, &[(7, 0xff)][..], 1512, -6048, 161),
        (30721, &[(7, 0xc0)][..], 1512, -6048, 161),
        (30722, &[(7, 0xff)][..], 1520, -6080, 161),
        (30724, &[(5, 0x80)][..], 1520, -6080, 161),
        (30725, &[(5, 0), (7, 0xff)][..], 1528, -6112, 160),
    ] {
        for &(reg, value) in writes {
            whole.dsp.write(reg, value);
            split.dsp.write(reg, value);
        }
        let target = (tick * 32u64 * 21_477_272).div_ceil(1_024_000);
        let delta = target - master;
        whole.tick(delta);
        let mut remaining = delta;
        for chunk in [1, 7, 666, 1001, 21, 313].into_iter().cycle() {
            if remaining == 0 {
                break;
            }
            let step = remaining.min(chunk);
            split.tick(step);
            remaining -= step;
        }
        assert_eq!(whole.samples.len(), tick as usize * 2);
        assert_eq!(
            (whole.dsp.regs[8], whole.dsp.regs[9]),
            ((env >> 4) as u8, outx),
            "tick {tick}"
        );
        assert!(
            whole.samples[previous_tick * 2..]
                .iter()
                .all(|&s| s == pcm as f32 / 32768.0),
            "PCM interval through {tick}"
        );
        assert_eq!(whole.dsp.regs, split.dsp.regs, "tick {tick}");
        assert_eq!(whole.samples, split.samples, "tick {tick}");
        master = target;
        previous_tick = tick as usize;
    }
}
