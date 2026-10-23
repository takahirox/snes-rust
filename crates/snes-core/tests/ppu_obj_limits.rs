//! Original MIT geometric budgets and literal survivor vectors for Issue #57.
//! Copyright (c) 2026 snes-rust contributors.
use snes_core::{cpu::Bus, ppu::Ppu, Machine};
#[path = "common/obj_limits.rs"]
mod obj_limits;

const BLACK: [u8; 4] = [0, 0, 0, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
fn scene(selector: u8) -> Ppu {
    let mut p = Ppu::default();
    p.write(1, selector << 5);
    p.write(0x2c, 16);
    for n in 0..128 {
        object(&mut p, n, -128, 100, false, 15, 0);
    }
    for (i, c) in [31u16, 0x3e0, 0x7c00].into_iter().enumerate() {
        p.cgram[(129 + i) * 2..(130 + i) * 2].copy_from_slice(&c.to_le_bytes());
    }
    // Every tile row has red/green/blue columns 0/1/2. Tile 15 is transparent.
    for row in 0..8 {
        for (col, color) in [(0, 1u8), (1, 2), (2, 3)] {
            for y in 0..8 {
                for plane in 0..4 {
                    p.vram[(row * 16 + col) * 32 + y * 2 + plane / 2 * 16 + plane % 2] =
                        if color & (1 << plane) != 0 { 255 } else { 0 };
                }
            }
        }
    }
    p.write(0, 15);
    p
}
fn object(p: &mut Ppu, i: usize, x: i16, y: u8, large: bool, name: u8, attr: u8) {
    p.oam[i * 4..i * 4 + 4].copy_from_slice(&[x as u8, y, name, attr]);
    let shift = (i % 4) * 2;
    p.oam[512 + i / 4] = (p.oam[512 + i / 4] & !(3 << shift))
        | ((u8::from(x < 0) | (u8::from(large) << 1)) << shift);
}
fn at(p: &Ppu, x: usize, y: usize, color: [u8; 4]) {
    let a = (y * 256 + x) * 4;
    assert_eq!(&p.frame[a..a + 4], color, "({x},{y})");
}
fn status(p: &mut Ppu, expected: u8) {
    assert_eq!(p.read(0x3e, 0) & 0xc0, expected);
    assert_eq!(p.read(0x3e, 0) & 0xc0, expected, "read must not clear");
}
#[test]
fn independent_32_and_33_object_boundaries() {
    for count in [32, 33] {
        let mut p = scene(0);
        for i in 0..31 {
            object(&mut p, i, 0, 24, false, 15, 0); // transparent and overlapping
        }
        object(&mut p, 31, 200, 24, false, 1, 0);
        if count == 33 {
            object(&mut p, 32, 216, 24, false, 2, 0);
        }
        p.render_line(24);
        status(&mut p, if count == 33 { 0x40 } else { 0 });
        at(&p, 200, 24, GREEN);
        at(&p, 216, 24, BLACK);
    }
}
#[test]
fn independent_34_and_35_tile_boundaries_reverse_object_order() {
    for extra in [false, true] {
        let mut p = scene(0);
        // Seventeen 16-wide objects: exactly 34 candidates, below range limit.
        for i in 1..18 {
            object(&mut p, i, if i == 1 { 64 } else { 128 }, 24, true, 0, 0);
        }
        if extra {
            object(&mut p, 0, 24, 24, false, 2, 0);
        }
        p.render_line(24);
        at(&p, 24, 24, BLACK); // earliest selected, fetched last, rejected
        at(&p, 64, 24, RED);
        at(&p, 72, 24, GREEN);
        at(&p, 128, 24, RED);
        status(&mut p, if extra { 0x80 } else { 0 });
    }
}
#[test]
fn tile_exhaustion_inside_object_is_left_to_right_after_flip() {
    for flip in [0, 64] {
        let mut p = scene(0);
        object(&mut p, 0, 24, 24, true, 0, flip);
        // Reverse fetching spends 33 slots before reaching object zero.
        object(&mut p, 1, 0, 24, false, 15, 0);
        for i in 2..18 {
            object(&mut p, i, 128, 24, true, 0, 0);
        }
        p.render_line(24);
        at(&p, 24, 24, if flip == 0 { RED } else { GREEN });
        at(&p, 31, 24, if flip == 0 { RED } else { GREEN });
        at(&p, 32, 24, BLACK);
        at(&p, 39, 24, BLACK);
        status(&mut p, 0x80);
    }
}
#[test]
fn horizontal_selection_excludes_fully_offscreen_but_includes_minus256() {
    for (x, expected, visible) in [
        (-8, 0, false),
        (-9, 0, false),
        (-255, 0, false),
        (-256, 0x40, false),
        (-7, 0x40, true),
        (255, 0x40, true),
    ] {
        let mut p = scene(0);
        for i in 0..32 {
            object(&mut p, i, x, 24, false, 15, 0);
        }
        object(&mut p, 32, 200, 24, false, 1, 0);
        p.render_line(24);
        status(&mut p, expected);
        at(&p, 200, 24, if expected == 0 { GREEN } else { BLACK });
        if !visible {
            at(&p, 0, 24, BLACK);
        }
    }
}
#[test]
fn clipped_aligned_and_unaligned_tiles_and_minus256_use_geometric_budget() {
    // 33 late candidates plus the probe. Literal counts for a 16-wide probe:
    // -16:0, -15:1, -8:1, -7/-1:2, 240/241/247:2,
    // 248/249/255:1, -256:2.
    for (x, over, left, right) in [
        (-16, false, None, None),
        (-15, false, Some((0, GREEN)), None),
        (-8, false, Some((0, GREEN)), None),
        (-7, true, Some((0, RED)), Some((1, BLACK))),
        (-1, true, Some((0, RED)), Some((7, BLACK))),
        (240, true, Some((240, RED)), Some((248, BLACK))),
        (241, true, Some((241, RED)), Some((249, BLACK))),
        (247, true, Some((247, RED)), Some((255, BLACK))),
        (248, false, Some((248, RED)), None),
        (249, false, Some((249, RED)), None),
        (255, false, Some((255, RED)), None),
        (-256, true, Some((0, BLACK)), None),
    ] {
        let mut p = scene(0);
        object(&mut p, 0, x, 24, true, 0, 0);
        object(&mut p, 1, 128, 24, false, 15, 0);
        for i in 2..18 {
            object(&mut p, i, 128, 24, true, 15, 0);
        }
        p.render_line(24);
        for (x, color) in left.into_iter().chain(right) {
            at(&p, x, 24, color);
        }
        status(&mut p, if over { 0x80 } else { 0 });
    }
}
#[test]
fn all_dimension_pairs_budget_width_and_select_lower_height() {
    // Literal dimensions from OBSEL table; row is deliberately in lower half.
    for (selector, large, width, height) in [
        (0, false, 8, 8),
        (0, true, 16, 16),
        (1, false, 8, 8),
        (1, true, 32, 32),
        (2, false, 8, 8),
        (2, true, 64, 64),
        (3, false, 16, 16),
        (3, true, 32, 32),
        (4, false, 16, 16),
        (4, true, 64, 64),
        (5, false, 32, 32),
        (5, true, 64, 64),
        (6, false, 16, 32),
        (6, true, 32, 64),
        (7, false, 16, 32),
        (7, true, 32, 32),
    ] {
        // Explicit candidate totals 32,34,36,39 (last 64-wide object clips one
        // tile at X=256); no production helper oracle.
        let (count, over) = match width {
            8 => (32, 0),
            16 => (17, 0),
            32 => (9, 0x80),
            64 => (5, 0x80),
            _ => unreachable!(),
        };
        let mut p = scene(selector);
        for i in 0..count {
            object(
                &mut p,
                i,
                if i == count - 1 { 200 } else { 64 },
                24,
                large,
                0,
                0,
            );
        }
        let y = 24 + height - 1;
        p.render_line(y);
        status(&mut p, over);
        at(&p, 200, y, RED);
        // A fresh scene just below the last included row has no candidates.
        let mut p = scene(selector);
        for i in 0..33 {
            object(&mut p, i, 64, 24, large, 0, 0);
        }
        p.render_line(24 + height);
        status(&mut p, 0);
        at(&p, 64, 24 + height, BLACK);
    }
}
#[test]
fn y_wrap_and_both_flags() {
    let mut p = scene(6);
    for i in 0..33 {
        object(&mut p, i, if i == 31 { 200 } else { 64 }, 248, false, 0, 0);
    }
    p.render_line(16); // Local row 24: lower half of 16x32.
    status(&mut p, 0xc0);
    at(&p, 200, 16, RED);
    at(&p, 208, 16, GREEN);
    at(&p, 64, 16, RED);
    p.render_line(24); // No objects here, flags persist.
    status(&mut p, 0xc0);
    at(&p, 200, 24, BLACK);
}
#[test]
fn routing_windows_transparency_and_overlap_do_not_save_budget() {
    for mode in 0..4 {
        let mut p = scene(0);
        for i in 0..33 {
            object(&mut p, i, 64, 24, true, if mode == 0 { 12 } else { 0 }, 0);
        }
        if mode == 2 {
            p.write(0x2c, 0);
            p.write(0x2d, 0);
        }
        if mode == 3 {
            p.write(0x25, 2);
            p.write(0x26, 0);
            p.write(0x27, 255);
            p.write(0x2e, 16);
            p.write(0x2f, 16);
        }
        p.render_line(24);
        status(&mut p, 0xc0);
        at(&p, 64, 24, if mode == 1 { RED } else { BLACK });
    }
}
#[test]
fn stable_rotation_wraps_selection_and_preserves_pixel_priority() {
    let mut p = scene(0);
    // Start 120: selected 120..127,0..23; index 24 is the rejected 33rd.
    for i in 0..128 {
        object(&mut p, i, 0, 100, false, 15, 0);
    }
    for i in (120..128).chain(0..25) {
        object(&mut p, i, 64, 24, false, 0, 0x30);
    }
    object(&mut p, 120, 64, 24, false, 1, 0); // earlier beats higher BG priority
    object(&mut p, 23, 200, 24, false, 2, 0);
    object(&mut p, 24, 216, 24, false, 2, 0);
    p.write(2, 240);
    p.write(3, 128);
    p.render_line(24);
    status(&mut p, 0x40);
    at(&p, 64, 24, GREEN);
    at(&p, 200, 24, BLUE);
    at(&p, 216, 24, BLACK);
    // Reverse fetch must also wrap: 17 large objects are 34 tiles; first
    // selected small index 120 loses to the later large objects across zero.
    let mut p = scene(0);
    object(&mut p, 120, 24, 24, false, 2, 0);
    for i in (121..128).chain(0..10) {
        object(&mut p, i, 128, 24, true, 0, 0);
    }
    object(&mut p, 121, 64, 24, true, 0, 0);
    p.write(2, 240);
    p.write(3, 128);
    p.render_line(24);
    status(&mut p, 0x80);
    at(&p, 24, 24, BLACK);
    at(&p, 64, 24, RED);
    at(&p, 72, 24, GREEN);
}
#[test]
fn stat77_preserves_open_bus_version_and_read_bus_update() {
    let mut p = scene(0);
    // M7A * high byte M7B -> $000010, a known PPU1 bus bit 4.
    p.write(0x1b, 16);
    p.write(0x1b, 0);
    p.write(0x1c, 0);
    p.write(0x1c, 1);
    assert_eq!(p.read(0x34, 0), 16);
    assert_eq!(p.read(0x3e, 0xff), 0x11);
    assert_eq!(p.read(0x04, 0), 0x11); // write-only PPU1 open bus
    for i in 0..33 {
        object(&mut p, i, 64, 24, true, 0, 0);
    }
    p.render_line(24);
    assert_eq!(p.read(0x3e, 0), 0xd1);
    assert_eq!(p.read(0x04, 0), 0xd1);
    assert_eq!(p.read(0x3e, 0), 0xd1);
}

#[test]
fn cpu_guests_and_natural_scheduler_lifetime() {
    for mode in 0..4 {
        let mut m = Machine::new(&obj_limits::guest_rom(mode)).unwrap();
        let start = m.cpu.instructions;
        while m.bus.peek(0x30) != 0x57 && m.cpu.instructions - start < 300_000 {
            m.step();
            assert!(!m.cpu.stopped);
            assert!(m.bus.frames < 8, "frame deadline");
        }
        assert_eq!(m.bus.peek(0x30), 0x57, "instruction deadline mode {mode}");
        let flags = [0, 0x40, 0x80, 0xc0][mode as usize];
        let expected = [flags, flags, flags, flags, flags, flags, flags, 0, 0];
        for (i, expected) in expected.into_iter().enumerate() {
            assert_eq!(
                m.bus.peek(0x1800 + i as u32),
                expected,
                "mode {mode} result {i}"
            );
        }
        assert_eq!(m.bus.peek(0x31), 1, "CPU's independent comparisons");
        // Restore original scene for stable final pixels, then observe complete frames.
        for _ in 0..3 {
            m.run_frame().unwrap();
        }
        for (x, color) in obj_limits::vectors(mode) {
            let a = (24 * 256 + x) * 4;
            assert_eq!(&m.framebuffer()[a..a + 4], color, "mode {mode} x={x}");
        }
    }
}

#[test]
fn scheduler_row_zero_convention_and_forced_blank_suppression() {
    // Only ordinary timed bus accesses advance raster state; no injected H/V.
    let mut m = Machine::new(&obj_limits::guest_rom(0)).unwrap();
    m.bus.write(0x2100, 128);
    m.bus.write(0x2102, 0);
    m.bus.write(0x2103, 0);
    for i in 0..128 {
        for b in [0, if i < 33 { 0 } else { 100 }, 0, 0] {
            m.bus.write(0x2104, b);
        }
    }
    for _ in 0..32 {
        m.bus.write(0x2104, 0);
    }
    while m.bus.frames < 1 {
        m.bus.read(0x4212);
    }
    // Forced blank across frame boundary: no evaluation of row zero.
    while m.bus.clocks % (262 * 1364) < 2 * 1364 {
        m.bus.read(0x4212);
    }
    assert_eq!(m.bus.read(0x213e) & 0xc0, 0);
    m.bus.write(0x2100, 15);
    while m.bus.frames < 2 {
        m.bus.read(0x4212);
    }
    assert_eq!(
        m.bus.read(0x213e) & 0xc0,
        0,
        "cleared at V=0 before drawing"
    );
    while m.bus.ppu.v == 0 {
        m.bus.read(0x4212);
    }
    assert_eq!(m.bus.read(0x213e) & 0xc0, 0, "row zero not yet rendered");
    while m.bus.ppu.v == 1 && m.bus.ppu.h < 1000 {
        m.bus.read(0x4212);
    }
    assert_eq!(m.bus.read(0x213e) & 0xc0, 0);
    while m.bus.ppu.v == 1 && m.bus.ppu.h < 1150 {
        m.bus.read(0x4212);
    }
    assert_eq!(
        m.bus.read(0x213e) & 0xc0,
        0x40,
        "row zero evaluated at V=1 HBlank"
    );
}

#[test]
fn rectangular_horizontal_selection_uses_width_in_lower_halves() {
    for (selector, large, width, row) in [
        (6, false, 16, 31),
        (6, true, 32, 63),
        (7, false, 16, 31),
        (7, true, 32, 31),
    ] {
        for (x, flags) in [(-width, 0), (-width - 1, 0), (1 - width, 0x40)] {
            let mut p = scene(selector);
            for i in 0..32 {
                object(&mut p, i, x, 24, large, 0, 0);
            }
            object(&mut p, 32, 200, 24 + row, false, 1, 0);
            let y = (24 + row) as usize;
            p.render_line(y);
            at(&p, 200, y, if flags == 0 { GREEN } else { BLACK });
            status(&mut p, flags);
        }
    }
}
#[test]
fn flags_accumulate_independently_and_transparent_front_reveals_back() {
    let mut p = scene(0);
    for i in 0..33 {
        object(&mut p, i, 0, 24, false, 15, 0);
    }
    p.render_line(24);
    status(&mut p, 0x40);
    // Change the next scene, without altering result state. Only TIME OVER
    // occurs on this line; RANGE OVER from the earlier line remains set.
    for i in 0..128 {
        object(&mut p, i, -128, 100, false, 15, 0);
    }
    for i in 0..18 {
        object(&mut p, i, 64, 50, true, 12, 0);
    }
    object(&mut p, 17, 64, 50, true, 0, 0x30);
    p.render_line(50);
    status(&mut p, 0xc0);
    at(&p, 64, 50, RED);
    at(&p, 72, 50, GREEN);
    p.write(0, 128); // Forced blank itself never acknowledges flags.
    p.render_line(60);
    status(&mut p, 0xc0);
    at(&p, 64, 60, BLACK);
}

#[test]
fn range_rejected_object_does_not_enter_tile_budget() {
    let mut p = scene(2); // 8x8 / 64x64
    for i in 0..31 {
        object(&mut p, i, 0, 24, false, 15, 0);
    }
    object(&mut p, 31, 200, 24, false, 1, 0);
    object(&mut p, 32, 216, 24, true, 2, 0); // Five visible tiles, none admitted.
    p.render_line(24);
    status(&mut p, 0x40); // 32 selected tiles; the 33rd object's tiles don't count.
    at(&p, 200, 24, GREEN);
    at(&p, 216, 24, BLACK);
}
