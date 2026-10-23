//! Original MIT register/readback and independent identity pixel vectors, Issue #59.
use snes_core::{ppu::Ppu, Machine};
#[path = "common/mode7_scroll.rs"]
mod mode7_scroll;
const BLACK: [u8; 4] = [0, 0, 0, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const YELLOW: [u8; 4] = [255, 255, 0, 255];
const CYAN: [u8; 4] = [0, 255, 255, 255];
const MAGENTA: [u8; 4] = [255, 0, 255, 255];
fn pair(p: &mut Ppu, r: u8, v: u16) {
    for b in v.to_le_bytes() {
        p.write(r, b);
    }
}
fn product(p: &mut Ppu) -> [u8; 3] {
    [p.read(0x34, 0), p.read(0x35, 0), p.read(0x36, 0)]
}
fn at(p: &Ppu, x: usize, y: usize, expected: [u8; 4]) {
    let a = (y * 256 + x) * 4;
    assert_eq!(&p.frame[a..a + 4], expected, "pixel ({x},{y})");
}
fn scene(vertical: bool) -> Ppu {
    let mut p = Ppu::default();
    p.write(5, 1); // capture outside Mode 7
    p.write(0x2c, 1);
    for (r, v) in [
        (0x1b, 256),
        (0x1c, 0),
        (0x1d, 0),
        (0x1e, 256),
        (0x1f, 0),
        (0x20, 0),
        (0x0d, 0),
        (0x0e, 0),
    ] {
        pair(&mut p, r, v);
    }
    for (i, color) in mode7_scroll::PALETTE.into_iter().enumerate() {
        p.cgram[i * 2..i * 2 + 2].copy_from_slice(&color.to_le_bytes());
    }
    let mut strip = [8; 128];
    for (x, color) in mode7_scroll::STRIPES {
        strip[x] = color;
    }
    for row in 0..128 {
        for col in 0..128 {
            p.vram[(row * 128 + col) * 2] = (if vertical { row } else { col } % 16 + 1) as u8;
        }
    }
    for tile in 0..16 {
        for y in 0..8 {
            for x in 0..8 {
                p.vram[(tile + 1) * 128 + y * 16 + x * 2 + 1] =
                    strip[tile * 8 + if vertical { y } else { x }];
            }
        }
    }
    p.write(0, 15);
    p
}
fn supplies_next_matrix_low_byte(ports: &[u8]) {
    for mode in 0..8 {
        for &r in ports {
            let mut p = Ppu::default();
            p.write(5, mode);
            pair(&mut p, 0x1c, 0x0100);
            p.write(r, 0x34);
            p.write(0x1b, 0x12);
            assert_eq!(product(&mut p), [0x34, 0x12, 0], "mode {mode}, port {r:x}");
        }
    }
}
#[test]
fn horizontal_offset_supplies_next_matrix_low_byte_immediately() {
    supplies_next_matrix_low_byte(&[0x0d]);
}
#[test]
fn vertical_offset_supplies_next_matrix_low_byte_immediately() {
    supplies_next_matrix_low_byte(&[0x0e]);
}
#[test]
fn matrix_and_centers_supply_next_matrix_low_byte_immediately() {
    supplies_next_matrix_low_byte(&[0x1d, 0x1e, 0x1f, 0x20]);
}
#[test]
fn hv_interleaving_supplies_latest_byte_to_matrix() {
    let mut p = Ppu::default();
    pair(&mut p, 0x1c, 0x0100);
    p.write(0x0d, 0x34);
    p.write(0x0e, 0x56);
    p.write(0x1b, 0x12);
    assert_eq!(product(&mut p), [0x56, 0x12, 0]);
}
#[test]
fn signed_16bit_matrix_multiplication_control() {
    let mut p = Ppu::default();
    pair(&mut p, 0x1c, 0x0100);
    pair(&mut p, 0x1b, 0x9234);
    assert_eq!(product(&mut p), [0x34, 0x92, 0xff]);
    pair(&mut p, 0x1b, 0xff34);
    pair(&mut p, 0x1c, 0xfe00);
    assert_eq!(product(&mut p), [0x98, 1, 0]); // -204 * -2 = 408
}
#[test]
fn all_planar_only_ports_preserve_mode7_previous_byte() {
    for r in 0x0f..=0x14 {
        let mut p = Ppu::default();
        pair(&mut p, 0x1c, 0x0100);
        p.write(0x20, 0x34);
        p.write(r, 0x77);
        p.write(0x1b, 0x12);
        assert_eq!(product(&mut p), [0x34, 0x12, 0]);
    }
}
#[test]
fn independent_horizontal_capture_survives_mode_switches() {
    let mut p = scene(false);
    p.write(0x0d, 0x15);
    p.write(0x0f, 0x42);
    p.write(0x0d, 0);
    for mode in [7, 0, 3, 7] {
        p.write(5, mode);
        if mode != 7 {
            continue;
        }
        for y in [0, 24, 223] {
            p.render_line(y);
            // Source columns 21,22,23,24,69,70,71,72,276(mod128=20).
            for (x, color) in [
                (0, RED),
                (1, BLUE),
                (2, WHITE),
                (3, YELLOW),
                (48, GREEN),
                (49, YELLOW),
                (50, CYAN),
                (51, MAGENTA),
                (255, BLUE),
            ] {
                at(&p, x, y, color);
            }
        }
    }
}
#[test]
fn hv_interleaving_changes_vertical_capture_without_planar_interference() {
    let mut p = scene(true);
    p.write(0x0d, 0x15);
    p.write(0x10, 0x42); // planar previous byte differs from Mode 7 previous byte
    p.write(0x0e, 0); // M7VOFS=21, BG1VOFS=66
    p.write(0x0d, 0); // restore H to zero, V is already independently captured
    p.write(5, 7);
    for (y, color) in [
        (0, RED),
        (1, BLUE),
        (2, WHITE),
        (3, YELLOW),
        (48, GREEN),
        (49, YELLOW),
        (50, CYAN),
        (51, MAGENTA),
        (223, BLACK),
    ] {
        p.render_line(y);
        for x in [0, 127, 255] {
            at(&p, x, y, color);
        }
    }
}
#[test]
fn positive_negative_offsets_fine_tile_boundaries_and_ignored_upper_bits() {
    // Literal input displacement and source artwork colors; no production
    // register/coordinate helpers are used to calculate expected pixels.
    for (offset, samples) in [
        (
            1,
            vec![
                (0, GREEN),
                (1, BLUE),
                (5, WHITE),
                (6, YELLOW),
                (7, CYAN),
                (14, RED),
                (15, GREEN),
            ],
        ),
        (
            7,
            vec![(0, YELLOW), (1, CYAN), (7, MAGENTA), (8, RED), (9, GREEN)],
        ),
        (8, vec![(0, CYAN), (6, MAGENTA), (7, RED), (8, GREEN)]),
        (
            0x1fff,
            vec![
                (1, RED),
                (2, GREEN),
                (3, BLUE),
                (8, YELLOW),
                (9, CYAN),
                (16, RED),
                (17, GREEN),
            ],
        ),
        (
            0x1ff8,
            vec![
                (8, RED),
                (9, GREEN),
                (15, YELLOW),
                (16, CYAN),
                (23, RED),
                (24, GREEN),
            ],
        ),
    ] {
        for upper in [0, 0x2000, 0x6000, 0xe000] {
            for vertical in [false, true] {
                let mut p = scene(vertical);
                pair(&mut p, if vertical { 0x0e } else { 0x0d }, offset | upper);
                p.write(5, 7);
                p.write(0x1a, 0x80); // transparent outside map, avoids wrap masking sign bugs
                for &(position, color) in &samples {
                    let (x, y) = if vertical {
                        (24, position)
                    } else {
                        (position, 24)
                    };
                    p.render_line(y);
                    at(&p, x, y, color);
                }
                if offset == 0x1fff || offset == 0x1ff8 {
                    let (x, y) = if vertical { (24, 0) } else { (0, 24) };
                    p.render_line(y);
                    at(&p, x, y, BLACK);
                }
            }
        }
    }
}
#[test]
fn cpu_guest_results_completion_and_distinguishing_scroll_scene() {
    let mut m = Machine::new(&mode7_scroll::guest_rom()).unwrap();
    while m.bus.peek(0x30) != 0x59 && m.cpu.instructions < 300_000 {
        m.step();
        assert!(!m.cpu.stopped);
        assert!(m.bus.frames < 8, "frame deadline");
    }
    assert_eq!(m.bus.peek(0x30), 0x59, "instruction deadline");
    let expected = [
        0x34, 0x12, 0, 0x34, 0x12, 0, 0x34, 0x12, 0, 0x34, 0x12, 0, 0x34, 0x12, 0, 0x34, 0x12, 0,
        0x56, 0x12, 0, 0x34, 0x92, 0xff, 0x34, 0x12, 0, 0x34, 0x12, 0, 0x34, 0x12, 0, 0x34, 0x12,
        0, 0x34, 0x12, 0, 0x34, 0x12, 0,
    ];
    for (i, expected) in expected.into_iter().enumerate() {
        assert_eq!(m.bus.peek(0x1800 + i as u32), expected, "guest result {i}");
    }
    assert_eq!(m.bus.peek(0x31), 1, "sticky guest pass");
    for _ in 0..3 {
        m.run_frame().unwrap();
    }
    assert_eq!(
        (
            m.framebuffer_width(),
            m.framebuffer_height(),
            m.framebuffer().len()
        ),
        (256, 224, 256 * 224 * 4)
    );
    for y in [0, 24, 223] {
        for (x, color) in [
            (0, RED),
            (1, BLUE),
            (2, WHITE),
            (3, YELLOW),
            (48, GREEN),
            (49, YELLOW),
            (50, CYAN),
            (51, MAGENTA),
            (255, BLUE),
        ] {
            let a = (y * 256 + x) * 4;
            assert_eq!(&m.framebuffer()[a..a + 4], color, "guest ({x},{y})");
        }
    }
    assert_eq!(m.bus.peek(0x30), 0x59);
    assert_eq!(m.bus.peek(0x31), 1);
}
