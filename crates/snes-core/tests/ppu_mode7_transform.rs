//! Original MIT public-API static coordinate/pixel vectors for Issues #63/#65.
//! All source coordinates are literal, independently derived in the validation record.
use snes_core::{ppu::Ppu, Machine};
#[path = "common/mode7_transform.rs"]
mod mode7_transform;
const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];
fn pair(p: &mut Ppu, r: u8, value: i16) {
    for b in value.to_le_bytes() {
        p.write(r, b);
    }
}
// A sparse original full-map scene: all map entries select green character 1,
// except the marked source cell selecting character 2, with one red pixel.
// This distinguishes remote columns/rows 0, 1, 511, 512, 1023, not a repeating strip.
fn scene(registers: [i16; 8], source: (i32, i32), raw: u8) -> Ppu {
    let mut p = Ppu::default();
    p.write(0, 128);
    p.write(5, 7);
    p.write(0x33, 0x40);
    for (i, color) in [
        (0, 0u16),
        (1, 0x001f),
        (2, 0x03e0),
        (3, 0x7c00),
        (128, 0x7c00),
        (129, 0x001f),
        (130, 0x03e0),
        (131, 0x7c00),
    ] {
        p.cgram[i * 2..i * 2 + 2].copy_from_slice(&color.to_le_bytes());
    }
    for word in 0..16384 {
        p.vram[word * 2] = 1;
    }
    for tile in [1, 2] {
        for pixel in 0..64 {
            p.vram[tile * 128 + pixel * 2 + 1] = 0x82;
        }
    }
    // Character zero is asymmetric and blue except (0,0)=green, (7,0)=red,
    // (0,7)=green. It must never be confused with the marked map character.
    for pixel in 0..64 {
        p.vram[pixel * 2 + 1] = 0x83;
    }
    p.vram[1] = 0x82;
    p.vram[15] = 0x81;
    p.vram[113] = 0x82;
    let (x, y) = ((source.0 & 1023) as usize, (source.1 & 1023) as usize);
    p.vram[((y / 8) * 128 + x / 8) * 2] = 2;
    p.vram[256 + (y & 7) * 16 + (x & 7) * 2 + 1] = raw;
    for (r, v) in [0x1b, 0x1c, 0x1d, 0x1e, 0x0d, 0x0e, 0x1f, 0x20]
        .into_iter()
        .zip(registers)
    {
        pair(&mut p, r, v);
    }
    p.write(0, 15);
    p
}
fn pixel(p: &mut Ppu, x: usize, y: usize, expected: [u8; 4]) {
    p.render_line(y);
    let a = (y * 256 + x) * 4;
    assert_eq!(&p.frame[a..a + 4], expected, "display ({x},{y})");
}
fn sample(regs: [i16; 8], input: (usize, usize), source: (i32, i32), sel: u8) {
    for layer in [1, 2] {
        let mut p = scene(regs, source, 0x81);
        p.write(0x2c, layer);
        p.write(0x1a, sel);
        pixel(&mut p, input.0, input.1, RED);
    }
}
#[test]
fn identity_controls() {
    for (input, source) in [((0, 0), (0, 1)), ((7, 8), (7, 9)), ((255, 223), (255, 224))] {
        sample([256, 0, 0, 256, 0, 0, 0, 0], input, source, 0);
    }
}
#[test]
fn fractional_x_origin_and_signed_rounding() {
    // T(32)=0; 7*32=224. T(-32)=-64; -64-224=-288.
    sample([32, 0, 0, 0, 1, 0, 0, 0], (7, 0), (0, 0), 0);
    sample([-32, 0, 0, 0, 1, 0, 0, 0], (7, 0), (-2, 0), 0);
}
#[test]
fn fractional_y_origin_and_signed_rounding() {
    sample([0, 0, 32, 0, 1, 0, 0, 0], (7, 0), (0, 0), 0);
    sample([0, 0, -32, 0, 1, 0, 0, 0], (7, 0), (0, -2), 0);
}
#[test]
fn vertical_origin_products_and_row_products_are_separately_truncated() {
    // Framebuffer row 6 is screen Y=7. T(32)+T(7*32)=192, not 256.
    // Negative counterpart is -64-256=-320.
    for (b, d, source) in [
        (32, 0, (0, 0)),
        (0, 32, (0, 0)),
        (-32, 0, (-2, 0)),
        (0, -32, (0, -2)),
    ] {
        sample([0, b, 0, d, 0, 1, 0, 0], (0, 6), source, 0);
    }
    // Row product alone: 48 + T(224) = 240, not 272.
    sample([16, 32, 16, 32, 0, 0, 0, 0], (3, 6), (0, 0), 0);
}
#[test]
fn fractional_horizontal_steps_and_cross_terms_with_signed_centers() {
    for (x, source) in [
        (0, (0, 0)),
        (7, (0, 0)),
        (8, (1, 1)),
        (15, (1, 1)),
        (16, (2, 2)),
    ] {
        sample([32, 0, 32, 0, 0, 0, 0, 0], (x, 0), source, 0);
    }
    sample([128, 64, -64, 128, 5, -3, 2, -2], (7, 9), (9, 0), 0);
    sample([-96, 160, 192, -48, -4, 7, -3, 4], (5, 3), (-1, 5), 0);
}
#[test]
fn clipped_displacement_x_counterexamples() {
    sample([128, 0, 0, 0, 1024, 0, 0, 0], (0, 0), (0, 0), 0);
}
#[test]
fn negative_clipped_displacement_x_counterexample() {
    sample([128, 0, 0, 0, -1025, 0, 0, 0], (0, 0), (-1, 0), 0);
}
#[test]
fn clipped_displacement_y_counterexamples() {
    sample([0, 0, 128, 0, 1024, 0, 0, 0], (0, 0), (0, 0), 0);
}
#[test]
fn negative_clipped_displacement_y_counterexample() {
    sample([0, 0, 128, 0, -1025, 0, 0, 0], (0, 0), (0, -1), 0);
}
#[test]
fn displacement_boundaries_on_both_inputs_and_both_outputs() {
    // H-CX or V-CY; center contribution is included literally below.
    // Difference extremes are +/-8191, using representable input endpoints.
    for (offset, center, expected, vertical_expected) in [
        (1023, 0, 511, 512),
        (1024, 0, 0, 0),
        (1025, 0, 0, 1),
        (-1023, 0, -512, -511),
        (-1024, 0, -512, -512),
        (-1025, 0, -1, 0),
        (4095, -4096, -3585, -3584),
        (-4096, 4095, 3583, 3584),
        (0, -1023, -512, -511),
        (0, -1024, -1024, -1024),
        (0, -1025, -1025, -1024),
        (0, 1023, 511, 512),
        (0, 1024, 512, 512),
        (0, 1025, 1024, 1025),
    ] {
        // A,C consume clipped H; B,D consume clipped V. Keeping the other
        // center equal to the first makes both output equations observable.
        for vertical in [false, true] {
            let mut r = [0; 8];
            r[if vertical { 1 } else { 0 }] = 128;
            r[if vertical { 3 } else { 2 }] = 128;
            r[if vertical { 5 } else { 4 }] = offset;
            r[6] = center;
            r[7] = center;
            // Unused displacement terms are multiplied by zero.
            let source = if vertical {
                vertical_expected
            } else {
                expected
            };
            sample(r, (0, 0), (source, source), 0);
        }
    }
}
#[test]
fn upper_offset_and_center_bits_are_ignored() {
    for upper in [0, 0x2000, 0x6000, 0xe000u16] {
        let mut r = [128, 64, -64, 128, 5, -3, 2, -2];
        for value in &mut r[4..] {
            *value = ((*value as u16 & 0x1fff) | upper) as i16;
        }
        sample(r, (7, 9), (9, 0), 0);
    }
}
#[test]
fn all_flips_nonidentity_asymmetric_source_and_nonzero_centers() {
    for (sel, source) in [(0, (9, 0)), (1, (129, -61)), (2, (68, 117)), (3, (188, 57))] {
        sample([128, 64, -64, 128, 5, -3, 2, -2], (7, 9), source, sel);
    }
}
#[test]
fn screen_over_all_encodings_both_axes_before_wrapping_and_character_zero_fine_bits() {
    // Zero matrices make the source exactly the nonzero center, independent
    // of row convention. Every boundary is checked on both axes and layers.
    for (source, colors) in [
        ((-1, 0), [RED, RED, BLACK, RED]),
        ((0, 0), [RED, RED, RED, RED]),
        ((1023, 0), [RED, RED, RED, RED]),
        ((1024, 0), [RED, RED, BLACK, GREEN]),
        ((0, -1), [RED, RED, BLACK, GREEN]),
        ((0, 1023), [RED, RED, RED, RED]),
        ((0, 1024), [RED, RED, BLACK, GREEN]),
        ((-1, -1), [RED, RED, BLACK, BLUE]),
    ] {
        for (sel, color) in [0, 0x40, 0x80, 0xc0].into_iter().zip(colors) {
            for layer in [1, 2] {
                let mut p = scene(
                    [0, 0, 0, 0, 0, 0, source.0 as i16, source.1 as i16],
                    source,
                    0x81,
                );
                p.write(0x2c, layer);
                p.write(0x1a, sel);
                pixel(&mut p, 0, 0, color);
            }
        }
    }
    // Corrected clipped displacement reaches X=-1, fine X=7, Y=1; character
    // zero there is blue, while map wrap is red and transparent-over is black.
    // The second vector independently tests the Y equation at (1,-1).
    for (sel, color) in [(0, RED), (0x40, RED), (0x80, BLACK), (0xc0, BLUE)] {
        for layer in [1, 2] {
            for (registers, source) in [
                ([128, 0, 0, 0, -1025, 0, 0, 1], (-1, 1)),
                ([0, 0, 128, 0, -1024, 0, 1, 0], (1, -1)),
            ] {
                let mut p = scene(registers, source, 0x81);
                p.write(0x2c, layer);
                p.write(0x1a, sel);
                pixel(&mut p, 0, 0, color);
            }
        }
    }
}
#[test]
fn shared_corrected_sample_keeps_raw_decoding_and_bg1_direct_color() {
    for (raw, bg1, bg2, direct) in [
        (0x00, BLACK, BLACK, BLACK),
        (0x80, BLUE, BLACK, [0, 0, 131, 255]),
        (0x81, RED, RED, [32, 0, 131, 255]),
    ] {
        for (layer, color) in [(1, bg1), (2, bg2)] {
            let mut p = scene([32, 0, 0, 0, 1, 0, 0, 0], (0, 0), raw);
            p.write(0x2c, layer);
            pixel(&mut p, 7, 0, color);
            p.write(0x30, 1);
            pixel(&mut p, 7, 0, if layer == 1 { direct } else { bg2 });
        }
    }
}
#[test]
fn cpu_built_fractional_and_clipping_variants_three_complete_stable_frames() {
    for clipped in [false, true] {
        for bg2 in [false, true] {
            let mut m = Machine::new(&mode7_transform::guest_rom(clipped, bg2)).unwrap();
            while m.bus.peek(0x30) != 0x63 && m.cpu.instructions < 300_000 {
                m.step();
                assert!(!m.cpu.stopped);
                assert!(m.bus.apu.error.is_none());
                assert!(m.bus.frames < 8, "startup frame deadline");
            }
            assert_eq!(m.bus.peek(0x30), 0x63, "startup instruction deadline");
            m.run_frame().unwrap(); // discard the partial setup frame
            for _ in 0..3 {
                m.run_frame().unwrap();
                assert_eq!(
                    (
                        m.framebuffer_width(),
                        m.framebuffer_height(),
                        m.framebuffer().len()
                    ),
                    (256, 224, 229376)
                );
                for y in [0, 24, 223] {
                    for (x, color) in if clipped {
                        [(0, RED), (1, RED), (2, GREEN), (7, GREEN)]
                    } else {
                        [(0, RED), (7, RED), (8, GREEN), (15, GREEN)]
                    } {
                        let a = (y * 256 + x) * 4;
                        assert_eq!(
                            &m.framebuffer()[a..a + 4],
                            color,
                            "clipped={clipped},bg2={bg2},({x},{y})"
                        );
                    }
                }
                assert_eq!(m.bus.peek(0x30), 0x63);
            }
        }
    }
}

// Issue #65: screen Y is framebuffer row + 1, before M7SEL mirroring.
// Sparse markers distinguish both adjacent rows and distant map cells.
#[test]
fn row_origin_d_only_endpoints() {
    for (r, sy) in [(0, 1), (9, 10), (223, 224)] {
        sample([256, 0, 0, 256, 0, 0, 0, 0], (7, r), (7, sy), 0);
    }
}
#[test]
fn row_origin_d_only_flipped_endpoints() {
    for (r, sy) in [(0, 254), (9, 245), (223, 31)] {
        sample([256, 0, 0, 256, 0, 0, 0, 0], (7, r), (7, sy), 2);
    }
}
#[test]
fn row_origin_b_only_endpoints() {
    for (r, sx) in [(0, 1), (9, 15), (223, 336)] {
        sample([0, 384, 0, 0, 0, 0, 0, 0], (7, r), (sx, 0), 0);
    }
}
#[test]
fn row_origin_b_only_flipped_endpoints() {
    for (r, sx) in [(0, 381), (9, 367), (223, 46)] {
        sample([0, 384, 0, 0, 0, 0, 0, 0], (7, r), (sx, 0), 2);
    }
}
#[test]
fn row_origin_negative_fractional_b_and_d() {
    for (sel, points) in [
        (0, [(0, -2), (9, -15), (223, -336)]),
        (2, [(0, -381), (9, -368), (223, -47)]),
    ] {
        for (r, source) in points {
            sample([0, -384, 0, 0, 0, 0, 0, 0], (7, r), (source, 0), sel);
            sample([0, 0, 0, -384, 0, 0, 0, 0], (7, r), (0, source), sel);
        }
    }
}
#[test]
fn row_origin_positive_fractional_d() {
    for (sel, points) in [
        (0, [(0, 1), (9, 15), (223, 336)]),
        (2, [(0, 381), (9, 367), (223, 46)]),
    ] {
        for (r, sy) in points {
            sample([0, 0, 0, 384, 0, 0, 0, 0], (7, r), (0, sy), sel);
        }
    }
}
#[test]
fn row_origin_signed_centers_and_all_flips() {
    for (sel, source) in [(0, (9, 0)), (1, (129, -61)), (2, (68, 117)), (3, (188, 57))] {
        sample([128, 64, -64, 128, 5, -3, 2, -2], (7, 9), source, sel);
    }
    sample([-96, 160, 192, -48, -4, 7, -3, 4], (5, 3), (-1, 5), 0);
}

#[test]
fn row_origin_last_d_normal() {
    sample([256, 0, 0, 256, 0, 0, 0, 0], (7, 223), (7, 224), 0);
}
#[test]
fn row_origin_last_d_flipped() {
    sample([256, 0, 0, 256, 0, 0, 0, 0], (7, 223), (7, 31), 2);
}
#[test]
fn row_origin_last_b_normal() {
    sample([0, 384, 0, 0, 0, 0, 0, 0], (7, 223), (336, 0), 0);
}
#[test]
fn row_origin_last_b_flipped() {
    sample([0, 384, 0, 0, 0, 0, 0, 0], (7, 223), (46, 0), 2);
}
