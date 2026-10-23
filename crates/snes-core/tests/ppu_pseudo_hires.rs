//! Original MIT numerical vectors, graphics and CPU guests for Issue #53.
//! Copyright (c) 2026 snes-rust contributors.
use snes_core::{ppu::Ppu, Machine};
#[path = "common/pseudo_hires.rs"]
mod pseudo_hires;

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const WHITE: [u8; 4] = [255; 4];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const CYAN: [u8; 4] = [0, 255, 255, 255];
const MAGENTA: [u8; 4] = [255, 0, 255, 255];
fn word(p: &mut Ppu, a: usize, n: u16) {
    p.vram[a..a + 2].copy_from_slice(&n.to_le_bytes());
}
fn palette(p: &mut Ppu, i: usize, n: u16) {
    p.cgram[i * 2..i * 2 + 2].copy_from_slice(&n.to_le_bytes());
}
fn scroll(p: &mut Ppu, bg: usize, h: u16, v: u16) {
    for (axis, n) in [h, v].into_iter().enumerate() {
        let r = (13 + bg * 2 + axis) as u8;
        p.write(r, n as u8);
        p.write(r, (n >> 8) as u8);
    }
}
fn character(p: &mut Ppu, base: usize, n: usize, bpp: usize, f: impl Fn(usize, usize) -> u8) {
    for y in 0..8 {
        for plane in 0..bpp {
            let mut byte = 0;
            for x in 0..8 {
                byte |= ((f(x, y) >> plane) & 1) << (7 - x);
            }
            p.vram[base + n * bpp * 8 + y * 2 + plane / 2 * 16 + plane % 2] = byte;
        }
    }
}
fn scene() -> Ppu {
    let mut p = Ppu::default();
    for (r, v) in [
        (5, 1),
        (7, 0x60),
        (8, 0x68),
        (11, 0x21),
        (0x2c, 1),
        (0x2d, 2),
        (0x33, 8),
        (0, 15),
    ] {
        p.write(r, v);
    }
    for (i, c) in [
        (0, 0x7fe0),
        (1, 31),
        (2, 0x3e0),
        (3, 0),
        (17, 0x7c00),
        (18, 0x7fff),
        (19, 0),
    ] {
        palette(&mut p, i, c);
    }
    p.write(0x32, 0xbf); // Fixed magenta; main backdrop cyan.
    character(&mut p, 0x2000, 0, 4, |x, _| [1, 2, 0, 3, 1, 2, 1, 2][x]);
    character(&mut p, 0x4000, 0, 4, |x, _| [1, 2, 0, 3, 2, 1, 2, 1][x]);
    for i in 0..1024 {
        word(&mut p, 0xd000 + i * 2, 0x400);
    }
    p
}
fn at(p: &Ppu, x: usize, y: usize, expected: [u8; 4]) {
    let i = (y * p.framebuffer_width() + x) * 4;
    assert_eq!(&p.frame[i..i + 4], expected, "sample=({x},{y})");
}
fn pair(p: &Ppu, x: usize, y: usize, sub: [u8; 4], main: [u8; 4]) {
    at(p, 2 * x, y, sub);
    at(p, 2 * x + 1, y, main);
}
#[test]
fn pseudo_width_and_literal_phases() {
    let mut p = scene();
    p.render_line(0);
    assert_eq!(
        (p.framebuffer_width(), p.framebuffer_height(), p.frame.len()),
        (512, 224, 512 * 224 * 4)
    );
    for (x, sub, main) in [
        (0, BLUE, RED),
        (1, WHITE, GREEN),
        (2, MAGENTA, CYAN),
        (3, BLACK, BLACK),
        (254, WHITE, RED),
        (255, BLUE, GREEN),
    ] {
        pair(&p, x, 0, sub, main);
    }
}
#[test]
fn setini_preserves_ordinary_hires_character_phases() {
    for mode in [5, 6] {
        let mut p = scene();
        p.write(5, mode);
        p.write(0x2d, 1);
        character(&mut p, 0x2000, 1, 4, |x, _| if x % 2 == 0 { 2 } else { 1 });
        for bits in [0, 8, 0] {
            p.write(0x33, bits);
            p.render_line(0);
            assert_eq!((p.framebuffer_width(), p.frame.len()), (512, 512 * 224 * 4));
            for (x, c) in [
                (0, RED),
                (1, GREEN),
                (8, GREEN),
                (9, RED),
                (16, RED),
                (17, GREEN),
                (510, GREEN),
                (511, RED),
            ] {
                at(&p, x, 0, c);
            }
        }
    }
}
#[test]
fn isolated_low_resolution_control() {
    let mut p = scene();
    p.write(0x33, 0);
    p.render_line(0);
    assert_eq!((p.framebuffer_width(), p.frame.len()), (256, 256 * 224 * 4));
    for (x, c) in [(0, RED), (1, GREEN), (2, CYAN), (3, BLACK), (255, GREEN)] {
        at(&p, x, 0, c);
    }
}
#[test]
fn independent_enables_backdrops_transparency_and_opaque_black() {
    for select in [0, 2] {
        // CGWSEL subscreen-math selection must not route visible output.
        for tm in [0, 1] {
            for ts in [0, 2] {
                let mut p = scene();
                p.write(0x30, select);
                p.write(0x2c, tm);
                p.write(0x2d, ts);
                p.render_line(0);
                pair(
                    &p,
                    0,
                    0,
                    if ts == 0 { MAGENTA } else { BLUE },
                    if tm == 0 { CYAN } else { RED },
                );
                pair(&p, 2, 0, MAGENTA, CYAN);
                pair(
                    &p,
                    3,
                    0,
                    if ts == 0 { MAGENTA } else { BLACK },
                    if tm == 0 { CYAN } else { BLACK },
                );
            }
        }
    }
}
#[test]
fn layer_windows_use_full_dots_and_independent_masks() {
    for tmw in [0, 1] {
        for tsw in [0, 2] {
            let mut p = scene();
            p.write(0x23, 0x22);
            p.write(0x26, 7);
            p.write(0x27, 8);
            p.write(0x2e, tmw);
            p.write(0x2f, tsw);
            p.render_line(0);
            for (x, sub, main) in [
                (0, BLUE, RED),
                (6, WHITE, RED),
                (7, BLUE, GREEN),
                (8, BLUE, RED),
                (9, WHITE, GREEN),
                (255, BLUE, GREEN),
            ] {
                let inside = (7..=8).contains(&x);
                pair(
                    &p,
                    x,
                    0,
                    if inside && tsw != 0 { MAGENTA } else { sub },
                    if inside && tmw != 0 { CYAN } else { main },
                );
            }
        }
    }
}
#[test]
fn bg_and_obj_priority_and_screen_routing() {
    let mut p = scene();
    // OAM object 0 at full dot 5; other objects are off this row.
    for n in 0..128 {
        p.oam[n * 4 + 1] = 240;
    }
    p.oam[..4].copy_from_slice(&[5, 0, 0, 0x30]);
    character(&mut p, 0, 0, 4, |_, _| 1);
    palette(&mut p, 129, 0x3ff); // Yellow object, priority 3.
    for (tm, ts, sub, main) in [
        (1, 18, [255, 255, 0, 255], GREEN),
        (17, 2, BLUE, [255, 255, 0, 255]),
        (17, 18, [255, 255, 0, 255], [255, 255, 0, 255]),
    ] {
        p.write(0x2c, tm);
        p.write(0x2d, ts);
        p.render_line(0);
        pair(&p, 5, 0, sub, main);
        pair(&p, 4, 0, WHITE, RED); // Left of object.
    }
    p.oam[3] = 0; // Low-priority object falls behind both backgrounds.
    p.render_line(0);
    pair(&p, 5, 0, BLUE, GREEN);
    p.write(0x2c, 3);
    p.write(0x2d, 3);
    p.render_line(0);
    pair(&p, 0, 0, RED, RED); // BG1 beats BG2 low.
    word(&mut p, 0xd000, 0x2400); // BG2 high beats BG1 low.
    p.render_line(0);
    pair(&p, 0, 0, BLUE, BLUE);
}

// A linear coordinate image: 32x32 repeating dots with independently assigned
// colors. VRAM is merely an encoding of it; the expected crops use this image,
// not production scroll, map, tile or pixel helpers.
fn image(x: usize, y: usize) -> u8 {
    ((x + y * 2 + x / 8 + y / 8 * 3) % 3 + 1) as u8
}
fn coordinate_scene(mode: u8, size: usize) -> Ppu {
    let mut p = scene();
    p.write(5, mode | if size == 16 { 16 } else { 0 });
    p.write(7, 0x61); // 64 columns to expose page crossings.
    p.write(9, 0x70);
    p.write(0x2c, 1);
    p.write(0x2d, 1);
    palette(&mut p, 3, 0x7fff);
    let bpp = match mode {
        0 => 2,
        3 | 4 => 8,
        _ => 4,
    };
    for row in 0..32 {
        for col in 0..64 {
            let c = col % (32 / size);
            let r = row % (32 / size);
            let n = if size == 8 { c + r * 4 } else { c * 2 + r * 32 };
            word(
                &mut p,
                0xc000 + col / 32 * 2048 + row * 64 + col % 32 * 2,
                n as u16,
            );
            for dy in 0..size / 8 {
                for dx in 0..size / 8 {
                    character(&mut p, 0x2000, n + dx + dy * 16, bpp, |x, y| {
                        image(c * size + dx * 8 + x, r * size + dy * 8 + y)
                    });
                }
            }
        }
    }
    p
}
fn image_color(x: usize, y: usize) -> [u8; 4] {
    [RED, GREEN, WHITE][image(x % 32, y % 32) as usize - 1]
}
#[test]
fn planar_scroll_tile_page_boundaries_and_both_edges() {
    for mode in [0, 1, 3, 4] {
        for size in [8, 16] {
            let mut p = coordinate_scene(mode, size);
            for h in [0, 1, 7, 8, 15, 16, 255, 256, 511, 1023] {
                scroll(&mut p, 0, h, 7);
                for y in [0, 8, 223] {
                    p.render_line(y);
                    for x in [0, 1, 6, 7, 8, 15, 16, 254, 255] {
                        let expected = image_color(x + h as usize, y + 8);
                        pair(&p, x, y, expected, expected);
                    }
                }
            }
        }
    }
}
#[test]
fn modes_2_4_offsets_keep_full_dot_boundaries() {
    for mode in [2, 4] {
        let mut p = coordinate_scene(mode, 8);
        scroll(&mut p, 0, 3, 19);
        // First eligible x=5. H=40, next column H=128; V=53 in Mode 2.
        word(&mut p, 0xe000, 0x2028);
        word(&mut p, 0xe002, 0x2080);
        word(&mut p, 0xe040, 0x2035);
        word(&mut p, 0xe042, 0x2035);
        p.render_line(6);
        for (x, sx, sy) in [
            (0, 3, 26),
            (4, 7, 26),
            (5, 48, 60),
            (12, 55, 60),
            (13, 144, 60),
            (20, 151, 60),
            (21, 24, 26),
            (255, 258, 26),
        ] {
            let expected = image_color(sx, if mode == 2 { sy } else { 26 });
            pair(&p, x, 6, expected, expected);
        }
        if mode == 4 {
            word(&mut p, 0xe000, 0xa035); // Axis selector: V=53, H stays ordinary.
            p.render_line(6);
            for (x, sx) in [(4, 7), (5, 8), (12, 15)] {
                let expected = image_color(sx, if x < 5 { 26 } else { 60 });
                pair(&p, x, 6, expected, expected);
            }
        }
    }
}
#[test]
fn identity_mode7_keeps_full_dot_transform_coordinates() {
    let mut p = scene();
    p.write(5, 7);
    p.write(0x2d, 1);
    for r in [0x1b, 0x1e] {
        p.write(r, 0);
        p.write(r, 1);
    }
    for x in 0..128 {
        p.vram[x * 2] = 1;
    }
    for y in 0..8 {
        for x in 0..8 {
            p.vram[128 + y * 16 + x * 2 + 1] = [1, 2, 0, 3, 2, 1, 2, 1][x];
        }
    }
    for y in [0, 1, 223] {
        // Repeat map across every Mode 7 tile row.
        for row in 0..128 {
            for col in 0..128 {
                p.vram[(row * 128 + col) * 2] = 1;
            }
        }
        p.render_line(y);
        for (x, c) in [(0, RED), (1, GREEN), (3, BLACK), (254, GREEN), (255, RED)] {
            pair(&p, x, y, c, c);
        }
        pair(&p, 2, y, MAGENTA, CYAN);
    }
}
#[test]
fn brightness_forced_blank_and_packed_frame_transitions() {
    let mut p = scene();
    for (b, sub, main) in [
        (0, BLACK, BLACK),
        (7, [0, 0, 119, 255], [119, 0, 0, 255]),
        (15, BLUE, RED),
    ] {
        p.write(0, b);
        p.render_line(0);
        pair(&p, 0, 0, sub, main);
    }
    for bits in [0, 1, 2, 4, 0x10, 0x20, 0x40, 0x80, 0xf7] {
        p.write(0x33, bits);
        p.render_line(0);
        assert_eq!(p.framebuffer_width(), 256);
    }
    p.write(0x33, 0);
    p.render_line(0);
    at(&p, 0, 0, RED);
    at(&p, 255, 0, GREEN);
    p.write(0x33, 8);
    p.render_line(1);
    pair(&p, 0, 0, RED, RED);
    pair(&p, 255, 0, GREEN, GREEN);
    pair(&p, 0, 1, BLUE, RED);
    p.write(0x33, 0);
    p.render_line(2);
    pair(&p, 0, 2, RED, RED);
    pair(&p, 255, 2, GREEN, GREEN);
    p.write(0, 0x80);
    p.render_line(223);
    assert!(p.frame[223 * 512 * 4..].chunks_exact(4).all(|c| c == BLACK));
    p.render_line(0);
    assert_eq!((p.framebuffer_width(), p.frame.len()), (256, 256 * 224 * 4));
    for mode in [1, 5, 6] {
        for bits in [0, 8] {
            p.write(5, mode);
            p.write(0x33, bits);
            for y in 0..224 {
                p.render_line(y);
            }
            let w = if mode == 1 && bits == 0 { 256 } else { 512 };
            assert_eq!((p.framebuffer_width(), p.frame.len()), (w, w * 224 * 4));
            assert!(p.frame.chunks_exact(4).all(|c| c == BLACK));
        }
    }
}
fn frames(m: &mut Machine, count: u64) {
    let target = m.bus.frames + count;
    let start = m.cpu.instructions;
    while m.bus.frames < target && m.cpu.instructions - start < 300_000 {
        m.step();
        assert!(!m.cpu.stopped);
    }
    assert_eq!(m.bus.frames, target, "guest deadline");
}
fn guest_frame(m: &Machine, enabled: bool) {
    let w = if enabled { 512 } else { 256 };
    assert_eq!(
        (
            m.framebuffer_width(),
            m.framebuffer_height(),
            m.framebuffer().len()
        ),
        (w, 224, w * 224 * 4)
    );
    assert_eq!(m.bus.peek(0x30), 0x53);
    assert_eq!(m.bus.peek(0x31), if enabled { 8 } else { 0 });
    assert_eq!(m.bus.ppu.regs[5], 1);
    for y in [0, 223] {
        let row = &m.framebuffer()[y * w * 4..(y + 1) * w * 4];
        for (x, c) in if enabled {
            vec![
                (0, BLUE),
                (1, RED),
                (2, WHITE),
                (3, GREEN),
                (510, WHITE),
                (511, GREEN),
            ]
        } else {
            vec![(0, RED), (1, GREEN), (254, RED), (255, GREEN)]
        } {
            assert_eq!(&row[x * 4..x * 4 + 4], c, "guest ({x},{y})");
        }
    }
}
#[test]
fn cpu_static_guests_and_controller_vblank_transitions_reset_replacement() {
    for enabled in [false, true] {
        let mut m = Machine::new(&pseudo_hires::guest_rom(enabled, false)).unwrap();
        frames(&mut m, 5);
        guest_frame(&m, enabled);
    }
    let rom = pseudo_hires::guest_rom(false, true);
    let mut m = Machine::new(&rom).unwrap();
    frames(&mut m, 5);
    guest_frame(&m, false);
    for enabled in [true, false, true] {
        m.set_buttons(if enabled { 0x80 } else { 0 });
        frames(&mut m, 3);
        guest_frame(&m, enabled);
    }
    m.set_buttons(0);
    m.reset();
    assert_eq!(
        (m.framebuffer_width(), m.framebuffer().len()),
        (256, 256 * 224 * 4)
    );
    frames(&mut m, 5);
    guest_frame(&m, false);
    for enabled in [true, false] {
        m = Machine::new(&pseudo_hires::guest_rom(enabled, false)).unwrap();
        frames(&mut m, 5);
        guest_frame(&m, enabled);
    }
}
#[test]
fn runner_ppm_geometry_payload_edges_and_last_row() {
    let dir = std::env::temp_dir().join(format!("snes-pseudo-hires-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for enabled in [false, true] {
        let rom = dir.join("guest.sfc");
        let ppm = dir.join("frame.ppm");
        std::fs::write(&rom, pseudo_hires::guest_rom(enabled, false)).unwrap();
        let r = std::process::Command::new(env!("CARGO_BIN_EXE_snes-run"))
            .arg(rom)
            .args([
                "--frames",
                "5",
                "--timeout",
                "10",
                "--expect",
                "0x30=0x53",
                "--ppm",
            ])
            .arg(&ppm)
            .output()
            .unwrap();
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
        let data = std::fs::read(ppm).unwrap();
        let w = if enabled { 512 } else { 256 };
        let header = format!("P6\n{w} 224\n255\n");
        assert!(data.starts_with(header.as_bytes()));
        assert_eq!(data.len(), header.len() + w * 224 * 3);
        for y in [0, 223] {
            for (x, c) in if enabled {
                vec![(0, BLUE), (1, RED), (510, WHITE), (511, GREEN)]
            } else {
                vec![(0, RED), (1, GREEN), (255, GREEN)]
            } {
                let i = header.len() + (y * w + x) * 3;
                assert_eq!(&data[i..i + 3], &c[..3]);
            }
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}
