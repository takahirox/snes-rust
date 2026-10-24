//! Original MIT public-API vectors for Issue #61. See the specification record.
use snes_core::{ppu::Ppu, Machine};
#[path = "common/mode7_extbg.rs"]
mod mode7_extbg;
const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const YELLOW: [u8; 4] = [255, 255, 0, 255];
const CYAN: [u8; 4] = [0, 255, 255, 255];
const GRAY: [u8; 4] = [131, 131, 131, 255];
const MAGENTA: [u8; 4] = [255, 0, 255, 255];
fn pair(p: &mut Ppu, r: u8, value: u16) {
    for b in value.to_le_bytes() {
        p.write(r, b);
    }
}
fn palette(p: &mut Ppu, index: usize, value: u16) {
    p.cgram[index * 2..index * 2 + 2].copy_from_slice(&value.to_le_bytes());
}
fn raw(p: &mut Ppu, value: u8) {
    for a in (129..256).step_by(2) {
        p.vram[a] = value;
    }
}
fn scene(value: u8) -> Ppu {
    let mut p = Ppu::default();
    p.write(0, 128);
    p.write(5, 7);
    p.write(0x33, 0x40);
    p.write(0x2c, 3);
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
    for a in (0..32768).step_by(2) {
        p.vram[a] = 1;
    }
    raw(&mut p, value);
    for (i, v) in [
        (0, 0x4210),
        (1, 0x001f),
        (127, 0x03e0),
        (128, 0x7c00),
        (129, 0x03ff),
        (255, 0x7fe0),
    ] {
        palette(&mut p, i, v);
    }
    p.write(0, 15);
    p
}
fn at(p: &mut Ppu, x: usize, y: usize, expected: [u8; 4]) {
    p.render_line(y);
    let a = (y * 256 + x) * 4;
    assert_eq!(&p.frame[a..a + 4], expected, "pixel ({x},{y})");
}
#[test]
fn every_raw_byte_interpretation_on_each_and_both_layers() {
    for (value, bg1, bg2, both) in [
        (0x00, GRAY, GRAY, GRAY),
        (0x01, RED, RED, RED),
        (0x7f, GREEN, GREEN, GREEN),
        (0x80, BLUE, GRAY, BLUE),
        (0x81, YELLOW, RED, RED),
        (0xff, CYAN, GREEN, GREEN),
    ] {
        for (enable, color) in [(1, bg1), (2, bg2), (3, both)] {
            let mut p = scene(value);
            p.write(0x2c, enable);
            for y in [0, 24, 223] {
                at(&mut p, 24, y, color);
            }
            // A second backdrop prevents accidental black/zero coincidences.
            palette(&mut p, 0, 0x7c1f);
            at(&mut p, 24, 24, if color == GRAY { MAGENTA } else { color });
        }
    }
}
#[test]
fn extbg_disabled_and_disabled_layers_cannot_leak_pixels_or_math() {
    for value in [1, 0x7f, 0x80, 0x81, 0xff] {
        let mut p = scene(value);
        p.write(0x33, 0);
        p.write(0x2c, 2);
        p.write(0x2d, 2);
        p.write(0x30, 2);
        p.write(0x31, 2); // no visible BG2 to select math
        at(&mut p, 24, 24, GRAY);
        p.write(0x33, 0x40);
        p.write(0x2c, 0);
        p.write(0x2d, 0);
        at(&mut p, 24, 24, GRAY);
    }
    let mut p = scene(0x81);
    for ext in [0, 0x40] {
        p.write(0x33, ext);
        p.write(0x2c, 1);
        at(&mut p, 24, 24, YELLOW);
    }
}
fn object(p: &mut Ppu, priority: u8) {
    p.write(1, 3); // OBJ at byte $c000, tile 0. No Mode 7 character overlap.
    for i in 0..128 {
        p.oam[i * 4 + 1] = 240;
    }
    p.oam[..4].copy_from_slice(&[24, 24, 0, priority << 4]);
    for y in 0..8 {
        p.vram[0xc000 + y * 2] = 255;
    }
    palette(p, 129, 0x7c1f);
}
#[test]
fn both_bg2_priorities_against_all_obj_priorities_with_each_screen_enable() {
    for value in [1, 0x81] {
        for enable in [1, 2, 3] {
            for priority in 0..4 {
                let mut p = scene(value);
                object(&mut p, priority);
                p.write(0x30, 1); // discriminates BG1 from BG2 even for raw $01
                p.write(0x2c, enable | 16);
                let bg1 = if value == 1 {
                    [32, 0, 0, 255]
                } else {
                    [32, 0, 131, 255]
                };
                // Explicit source ordering, independent of renderer rank numbers.
                let expected = match (value, enable, priority) {
                    (_, 1, 0) | (1, 3, 0) => bg1,
                    (0x81, 2 | 3, 0 | 1) => RED,
                    _ => MAGENTA,
                };
                at(&mut p, 24, 24, expected);
            }
        }
    }
}
#[test]
fn independent_main_windows_include_both_boundaries() {
    for (enable, selector, mask, color) in [(1, 2, 1, YELLOW), (2, 0x20, 2, RED)] {
        let mut p = scene(0x81);
        p.write(0x2c, enable);
        p.write(0x23, selector);
        p.write(0x26, 20);
        p.write(0x27, 23);
        p.write(0x2e, mask);
        for (x, c) in [(19, color), (20, GRAY), (23, GRAY), (24, color)] {
            at(&mut p, x, 24, c);
        }
        p.write(0x2e, mask ^ 3); // other layer mask cannot hide this one
        at(&mut p, 20, 24, color);
        p.write(0x2e, mask);
        p.write(0x23, selector ^ 0x22); // other layer selector cannot hide this one
        at(&mut p, 20, 24, color);
        p.write(0x23, selector);
        p.write(0x26, 24);
        p.write(0x27, 23); // empty window
        at(&mut p, 23, 24, color);
    }
    let mut p = scene(0x81);
    p.write(0x23, 0x20);
    p.write(0x26, 20);
    p.write(0x27, 23);
    p.write(0x2e, 2);
    for (x, c) in [(19, RED), (20, YELLOW), (23, YELLOW), (24, RED)] {
        at(&mut p, x, 24, c);
    }
    raw(&mut p, 0x80); // transparent BG2 must reveal independently masked BG1
    p.write(0x23, 2);
    p.write(0x2e, 1);
    at(&mut p, 20, 24, GRAY);
    at(&mut p, 24, 24, BLUE);
}
#[test]
fn main_layer_identity_selects_fixed_color_math() {
    for (value, enable, layer_bit, plain, added) in [
        (0x81, 3, 2, [32, 0, 0, 255], [57, 16, 8, 255]),
        (0x01, 3, 1, [32, 0, 0, 255], [57, 16, 8, 255]),
        (0x80, 3, 1, [0, 0, 32, 255], [24, 16, 41, 255]),
    ] {
        let mut p = scene(value);
        palette(&mut p, 1, 0x0004);
        palette(&mut p, 128, 0x1000);
        p.write(0x2c, enable);
        p.write(0x32, 0x23);
        p.write(0x32, 0x42);
        p.write(0x32, 0x81); // + (3,2,1)
        p.write(0x31, layer_bit);
        at(&mut p, 24, 24, added);
        p.write(0x31, layer_bit ^ 3);
        at(&mut p, 24, 24, plain);
    }
}
#[test]
fn independent_subscreen_routing_windows_and_bg2_math_operand() {
    for (sub, selector, mask, added) in [
        (1, 2, 1, [16, 49, 0, 255]),    // backdrop (2,2,0) + BG1 (0,4,0)
        (2, 0x20, 2, [49, 16, 0, 255]), // + BG2 (4,0,0)
    ] {
        let mut p = scene(0x81);
        palette(&mut p, 0, 0x0042);
        palette(&mut p, 129, 0x0080);
        palette(&mut p, 1, 0x0004);
        p.write(0x2c, 0);
        p.write(0x2d, sub);
        p.write(0x30, 2);
        p.write(0x31, 0x20);
        p.write(0x23, selector);
        p.write(0x26, 20);
        p.write(0x27, 23);
        p.write(0x2e, mask); // main masking cannot affect sub operand
        at(&mut p, 20, 24, added);
        p.write(0x2f, mask);
        for (x, c) in [
            (19, added),
            (20, [16, 16, 0, 255]),
            (23, [16, 16, 0, 255]),
            (24, added),
        ] {
            at(&mut p, x, 24, c);
        }
        p.write(0x2f, mask ^ 3);
        at(&mut p, 20, 24, added);
        p.write(0x2d, 0);
        at(&mut p, 24, 24, [16, 16, 0, 255]);
    }
    let mut p = scene(0x81);
    palette(&mut p, 129, 0x0080);
    palette(&mut p, 1, 0x0004);
    p.write(0x2c, 1);
    p.write(0x2d, 2);
    p.write(0x30, 2);
    p.write(0x31, 1);
    at(&mut p, 24, 24, [32, 32, 0, 255]);
    p.write(0x31, 2);
    at(&mut p, 24, 24, [0, 32, 0, 255]);
    raw(&mut p, 0x80);
    palette(&mut p, 128, 0x0080);
    p.write(0x31, 1);
    at(&mut p, 24, 24, [0, 32, 0, 255]); // $80 is transparent on sub
}
#[test]
fn bg1_direct_color_with_extbg_is_independent_of_cgram_and_bg2_display() {
    for enable in [1, 2, 3] {
        let mut p = scene(0);
        p.write(0x30, 1);
        p.write(0x2c, enable);
        at(&mut p, 24, 24, GRAY);
    }
    for (value, direct) in [
        (1, [32, 0, 0, 255]),
        (0x80, [0, 0, 131, 255]),
        (0x81, [32, 0, 131, 255]),
        (0xff, [230, 230, 197, 255]),
    ] {
        for enable in [1, 3] {
            let mut p = scene(value);
            p.write(0x2c, enable);
            p.write(0x30, 1);
            for (pal, c) in [(0x001f, RED), (0x03e0, GREEN)] {
                palette(&mut p, value as usize, pal);
                if value & 127 != 0 {
                    palette(&mut p, (value & 127) as usize, pal);
                }
                let bg2_visible = enable == 3 && value & 128 != 0 && value != 0x80;
                at(&mut p, 24, 24, if bg2_visible { c } else { direct });
                p.write(0x2c, 1);
                at(&mut p, 24, 24, direct);
                p.write(0x30, 0);
                at(&mut p, 24, 24, c);
                p.write(0x30, 1);
                p.write(0x2c, enable);
            }
        }
        let mut p = scene(value);
        p.write(0x2c, 2);
        p.write(0x30, 1);
        for (pal, c) in [(0x001f, RED), (0x03e0, GREEN)] {
            if value & 127 != 0 {
                palette(&mut p, (value & 127) as usize, pal);
            }
            at(&mut p, 24, 24, if value == 0x80 { GRAY } else { c });
        }
    }
}
#[test]
fn shared_source_with_small_offset_ignores_bg2_planar_settings() {
    let mut p = scene(0);
    // Interior identity source: at x=24..27, H=3 selects columns 27..30.
    // Only tile row 2 contains these bytes: V=2 selects row 26, also row 2.
    for (x, v) in [(3, 0x80), (4, 0x81), (5, 0xff), (6, 0x7f)] {
        p.vram[129 + 2 * 16 + x * 2] = v;
    }
    pair(&mut p, 0x0d, 3);
    pair(&mut p, 0x0e, 2);
    for (r, v) in [(8, 0xff), (0x0b, 0xf0), (5, 0xf7)] {
        p.write(r, v);
    }
    pair(&mut p, 0x0f, 90);
    pair(&mut p, 0x10, 74);
    for (enable, colors) in [
        (1, [BLUE, YELLOW, CYAN, GREEN]),
        (2, [GRAY, RED, GREEN, GREEN]),
        (3, [BLUE, RED, GREEN, GREEN]),
    ] {
        p.write(0x2c, enable);
        for (x, c) in (24..28).zip(colors) {
            at(&mut p, x, 24, c);
        }
    }
}
#[test]
fn extbg_bit_preserves_other_bgmodes() {
    for mode in 0..7 {
        let mut p = scene(0x81);
        p.write(5, mode);
        for y in [0, 24] {
            p.write(0x33, 0);
            p.render_line(y);
            let before = p.frame.clone();
            p.write(0x33, 0x40);
            p.render_line(y);
            assert_eq!(p.frame, before, "mode {mode}");
        }
    }
}
#[test]
fn original_cpu_guest_completes_and_renders_stable_extbg_scene() {
    let mut m = Machine::new(&mode7_extbg::guest_rom()).unwrap();
    while m.bus.peek(0x30) != 0x61 && m.cpu.instructions < 300_000 {
        m.step();
        assert!(!m.cpu.stopped);
        assert!(m.bus.apu.error.is_none());
        assert!(m.bus.frames < 8, "startup frame deadline");
    }
    assert_eq!(m.bus.peek(0x30), 0x61, "startup instruction deadline");
    m.run_frame().unwrap(); // discard the frame containing setup/forced blank
    for _ in 0..3 {
        let instructions = m.cpu.instructions;
        m.run_frame().unwrap(); // rejects STP/APU errors; bounded to 400,000 clocks
        assert!(
            m.cpu.instructions - instructions < 100_000,
            "stable instruction deadline"
        );
        assert_eq!(
            (
                m.framebuffer_width(),
                m.framebuffer_height(),
                m.framebuffer().len()
            ),
            (256, 224, 229376)
        );
        for y in [24, 80, 160, 223] {
            for (x, c) in [
                (0, GRAY),
                (1, RED),
                (2, GREEN),
                (3, [0, 0, 131, 255]),
                (4, RED),
                (5, GREEN),
                (6, [32, 0, 0, 255]),
                (7, [230, 230, 197, 255]),
            ] {
                let a = (y * 256 + x) * 4;
                assert_eq!(&m.framebuffer()[a..a + 4], c, "guest ({x},{y})");
            }
        }
        assert_eq!(m.bus.peek(0x30), 0x61);
    }
}
