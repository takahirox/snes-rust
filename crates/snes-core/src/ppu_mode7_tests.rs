//! Original MIT register vectors for Issue #59. No public inspection API.
use super::Ppu;

#[test]
fn each_write_immediately_captures_shared_previous_byte_in_every_mode() {
    for mode in 0..8 {
        let mut p = Ppu::default();
        p.write(5, mode);
        // Seed explicitly, rather than asserting physical reset retention.
        p.write(0x20, 0x31);
        for (r, byte, expected) in [
            (0x0d, 0x42, 0x4231),
            (0x0e, 0x53, 0x5342),
            (0x1b, 0x64, 0x6453),
            (0x1c, 0x75, 0x7564),
            (0x1d, 0x86, 0x8675),
            (0x1e, 0x97, 0x9786),
            (0x1f, 0xa8, 0xa897),
            (0x20, 0xb9, 0xb9a8),
            (0x0e, 0xca, 0xcab9),
            (0x0d, 0xdb, 0xdbca),
            (0x0d, 0xec, 0xecdb),
        ] {
            let old_matrix = p.m7;
            let old_scroll = p.m7_scroll;
            p.write(r, byte);
            if r <= 0x0e {
                let axis = (r - 0x0d) as usize;
                assert_eq!(p.m7_scroll[axis], expected);
                assert_eq!(p.m7_scroll[axis ^ 1], old_scroll[axis ^ 1]);
                assert_eq!(p.m7, old_matrix);
            } else {
                assert_eq!(p.m7[(r - 0x1b) as usize] as u16, expected);
                for (i, old) in old_matrix.into_iter().enumerate() {
                    if i != (r - 0x1b) as usize {
                        assert_eq!(p.m7[i], old);
                    }
                }
                assert_eq!(p.m7_scroll, old_scroll);
            }
            assert_eq!(p.m7_latch, byte);
        }
        p.write(5, (mode + 1) & 7);
        assert_eq!(p.m7_scroll, [0xecdb, 0xcab9]);
    }
}

#[test]
fn planar_and_mode7_latches_are_independent_with_horizontal_fine_merge() {
    let mut p = Ppu::default();
    p.write(0x0d, 0x15);
    p.write(0x0f, 0x42);
    p.write(0x0d, 0);
    assert_eq!(p.m7_scroll[0], 0x0015);
    assert_eq!(p.scroll[0][0], 0x0045);
    for r in 0x0f..=0x14 {
        p.write(r, 0x65);
        assert_eq!(p.m7_latch, 0);
    }
    for r in 0x1b..=0x20 {
        p.write(r, 0x76);
        assert_eq!(p.scroll_latch, 0x65);
    }
    p.write(0x0e, 0x12);
    assert_eq!(p.m7_scroll[1], 0x1276);
    assert_eq!(p.scroll[0][1], 0x1265);
    // Another single H write preserves previous H high-byte low three bits.
    p.write(0x0d, 3);
    p.write(0x1f, 0xff);
    p.write(0x0f, 0x2a);
    p.write(0x0d, 0);
    assert_eq!(p.scroll[0][0], 0x002b);
    assert_eq!(p.m7_scroll[0], 0x00ff);
}
