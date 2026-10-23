//! Original MIT coordinate graphics, numerical vectors and CPU guests for #49.
//! Copyright (c) 2026 snes-rust contributors.
use snes_core::{ppu::Ppu, Machine};
#[path = "common/hires.rs"]
mod hires;

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const WHITE: [u8; 4] = [255; 4];
fn word(p: &mut Ppu, a: usize, w: u16) {
    p.vram[a & 65535] = w as u8;
    p.vram[(a + 1) & 65535] = (w >> 8) as u8;
}
fn scroll(p: &mut Ppu, bg: usize, h: u16, v: u16) {
    for (axis, n) in [h, v].into_iter().enumerate() {
        let r = (13 + bg * 2 + axis) as u8;
        p.write(r, n as u8);
        p.write(r, (n >> 8) as u8);
    }
}
fn palette(p: &mut Ppu, i: usize, color: u16) {
    p.cgram[i * 2..i * 2 + 2].copy_from_slice(&color.to_le_bytes());
}
fn character(p: &mut Ppu, base: usize, n: usize, bpp: usize, f: impl Fn(usize, usize) -> u8) {
    for y in 0..8 {
        for plane in 0..bpp {
            let mut byte = 0;
            for x in 0..8 {
                byte |= ((f(x, y) >> plane) & 1) << (7 - x);
            }
            p.vram[(base + n * bpp * 8 + y * 2 + plane / 2 * 16 + plane % 2) & 65535] = byte;
        }
    }
}
fn scene(mode: u8, bg: usize, height: usize, layout: u8) -> Ppu {
    let mut p = Ppu::default();
    p.write(5, mode | if height == 16 { 16 << bg } else { 0 });
    p.write(7 + bg as u8, 0x60 | layout);
    p.write(0x0b, 0x11);
    p.write(0x2c, 1 << bg);
    p.write(0x2d, 1 << bg);
    for (i, c) in [0x7c00, 0x001f, 0x03e0, 0x7fff].into_iter().enumerate() {
        palette(&mut p, i, c);
    }
    p.write(0x32, 0x9f); // Sub backdrop blue.
    p.write(0, 15);
    p
}
fn pixel(p: &mut Ppu, x: usize, y: usize, expected: [u8; 4]) {
    p.render_line(y);
    let i = (y * p.framebuffer_width() + x) * 4;
    assert_eq!(&p.frame[i..i + 4], &expected, "x={x},y={y}");
}

#[test]
fn scroll_units_masking_all_layouts_and_both_edges() {
    // Original coordinate texture: color encodes source position modulo 3.
    // The expected row is built independently as a linear source image, then
    // cropped at literal half-dot scroll displacements (no renderer helpers).
    for (mode, bg, bpp) in [(5, 0, 4), (5, 1, 2), (6, 0, 4)] {
        for height in [8, 16] {
            for layout in 0..4 {
                let mut p = scene(mode, bg, height, layout);
                for n in 0..128 {
                    character(&mut p, 0x2000, n, bpp, |x, y| {
                        ((n * 8 + x + y) % 3 + 1) as u8
                    });
                }
                // Map row zero: cell c starts at character 2*c; pages A/B.
                for c in 0..64 {
                    word(
                        &mut p,
                        0xc000 + (c / 32) * 2048 + (c % 32) * 2,
                        (c * 2) as u16,
                    );
                }
                for (h, displacement) in [
                    (0, 0),
                    (1, 2),
                    (7, 14),
                    (8, 16),
                    (255, 510),
                    (256, 512),
                    (511, 1022),
                    (512, 0),
                    (1023, 1022),
                ] {
                    scroll(&mut p, bg, h, 0);
                    p.render_line(0);
                    assert_eq!(
                        (p.framebuffer_width(), p.framebuffer_height(), p.frame.len()),
                        (512, 224, 512 * 224 * 4)
                    );
                    for x in [
                        0, 1, 6, 7, 8, 9, 14, 15, 16, 17, 254, 255, 256, 257, 510, 511,
                    ] {
                        let source = (x + displacement) % if layout & 1 == 0 { 512 } else { 1024 };
                        let expected = [RED, GREEN, WHITE][(source + 1) % 3];
                        pixel(&mut p, x, 0, expected);
                    }
                }
            }
        }
    }
}

#[test]
fn full_cell_flips_height_and_character_wrap() {
    for (mode, bg, bpp) in [(5, 0, 4), (5, 1, 2), (6, 0, 4)] {
        for height in [8, 16] {
            for (flip, columns, rows) in [
                (0, [0, 7, 8, 15], [0, 7, 8, 15]),
                (0x4000, [15, 8, 7, 0], [0, 7, 8, 15]),
                (0x8000, [0, 7, 8, 15], [15, 8, 7, 0]),
                (0xc000, [15, 8, 7, 0], [15, 8, 7, 0]),
            ] {
                let mut p = scene(mode, bg, height, 0);
                word(&mut p, 0xc000, 1023 | flip);
                // N=1023: upper left; N+1=0; N+16=15; N+17=16.
                for (n, label) in [(1023, 0), (0, 1), (15, 2), (16, 3)] {
                    character(&mut p, 0x2000, n, bpp, |x, y| {
                        ((label + x + 2 * y) % 3 + 1) as u8
                    });
                }
                for (xi, x) in [0, 7, 8, 15].into_iter().enumerate() {
                    for (yi, sy) in [0, 7, 8, 15].into_iter().enumerate() {
                        if sy >= height {
                            continue;
                        }
                        // Place row zero exactly at local sy via V=(sy-1) modulo map height.
                        scroll(
                            &mut p,
                            bg,
                            0,
                            ((sy + height * 32 - 1) % (height * 32)) as u16,
                        );
                        let u = columns[xi];
                        let v = if height == 8 && flip & 0x8000 != 0 {
                            7 - sy
                        } else {
                            rows[yi]
                        };
                        let label = usize::from(u >= 8) + usize::from(v >= 8) * 2;
                        let expected = [RED, GREEN, WHITE][(label + u % 8 + 2 * (v % 8)) % 3];
                        pixel(&mut p, x, 0, expected);
                    }
                }
            }
        }
    }
}

#[test]
fn map_pages_and_vram_byte_wrap() {
    // Addresses for (cell31,row31), (cell32,row31), (cell31,row32),
    // (cell32,row32). These literals cover A/B/C/D and both mirror layouts.
    for (layout, addresses) in [
        (0, [0xc7fe, 0xc7c0, 0xc03e, 0xc000]),
        (1, [0xc7fe, 0xcfc0, 0xc03e, 0xc800]),
        (2, [0xc7fe, 0xc7c0, 0xc83e, 0xc800]),
        (3, [0xc7fe, 0xcfc0, 0xd03e, 0xd800]),
    ] {
        for height in [8, 16] {
            for base in [0x60, 0xfc] {
                let mut p = scene(5, 0, height, layout);
                p.write(7, base | layout);
                p.write(0x0b, 7); // Character base $e000; N=768 wraps to byte $4000.
                for (n, color) in [(768, 1), (770, 2), (772, 3), (774, 1)] {
                    character(&mut p, 0xe000, n, 4, |_, _| color);
                }
                let delta = if base == 0xfc { 0x13800 } else { 0 };
                for (a, n) in addresses.into_iter().zip([768, 770, 772, 774]) {
                    word(&mut p, a + delta, n);
                }
                scroll(&mut p, 0, 248, (height * 31 - 1) as u16);
                pixel(&mut p, 0, 0, RED);
                pixel(&mut p, 16, 0, GREEN);
                scroll(&mut p, 0, 248, (height * 32 - 1) as u16);
                pixel(&mut p, 0, 0, WHITE);
                pixel(&mut p, 16, 0, RED);
            }
        }
    }
}

#[test]
fn phases_transparency_palettes_priority_and_mode6_hidden_layers() {
    let mut p = scene(5, 0, 8, 0);
    p.write(8, 0x68);
    p.write(0x0b, 0x21);
    palette(&mut p, 0, 0x7fff);
    palette(&mut p, 17, 0x001f);
    palette(&mut p, 18, 0x03e0);
    palette(&mut p, 5, 0x7c00);
    palette(&mut p, 6, 0x03ff);
    character(&mut p, 0x2000, 0, 4, |x, _| {
        if x == 4 {
            0
        } else {
            1 + x as u8 % 2
        }
    });
    character(&mut p, 0x4000, 0, 2, |x, _| {
        if x == 4 {
            0
        } else {
            1 + x as u8 % 2
        }
    });
    word(&mut p, 0xc000, 0x0400);
    word(&mut p, 0xd000, 0x0400);
    p.write(0x32, 0x3f); // Sub backdrop magenta, distinct from all visible phase colors.
    let magenta = [255, 0, 255, 255];
    for (tm, ts, expected) in [
        (1, 0, [magenta, GREEN]),
        (0, 2, [BLUE, WHITE]),
        (1, 2, [BLUE, GREEN]),
        (2, 1, [RED, [255, 255, 0, 255]]),
    ] {
        p.write(0x2c, tm);
        p.write(0x2d, ts);
        pixel(&mut p, 0, 0, expected[0]);
        pixel(&mut p, 1, 0, expected[1]);
        pixel(&mut p, 4, 0, magenta); // transparent sub => fixed color
    }
    p.write(0x2c, 3);
    p.write(0x2d, 3);
    pixel(&mut p, 0, 0, RED); // BG1 low above BG2 low
    word(&mut p, 0xd000, 0x2400);
    pixel(&mut p, 0, 0, BLUE); // BG2 high above BG1 low
    word(&mut p, 0xc000, 0x2400);
    pixel(&mut p, 1, 0, GREEN); // BG1 high above BG2 high
    pixel(&mut p, 5, 0, GREEN);
    p.write(5, 6);
    p.write(0x2c, 14);
    p.write(0x2d, 14);
    pixel(&mut p, 0, 0, magenta);
    pixel(&mut p, 1, 0, WHITE);
    // Transparent main pixel with a deliberately empty character.
    p.write(0x2c, 1);
    word(&mut p, 0xc000, 2);
    pixel(&mut p, 1, 0, WHITE);
}

#[test]
fn packed_frame_transitions_and_mixed_rows_preserve_previous_pixels() {
    let mut p = scene(1, 0, 8, 0);
    character(&mut p, 0x2000, 0, 4, |x, _| 1 + x as u8 % 2);
    p.render_line(0);
    assert_eq!(p.frame.len(), 256 * 224 * 4);
    let before = p.frame[..256 * 4].to_vec();
    p.write(5, 5);
    p.render_line(1);
    assert_eq!(p.frame.len(), 512 * 224 * 4);
    for x in 0..256 {
        assert_eq!(
            &p.frame[x * 8..x * 8 + 8],
            [&before[x * 4..x * 4 + 4], &before[x * 4..x * 4 + 4]].concat()
        );
    }
    p.write(5, 1);
    pixel(&mut p, 510, 2, GREEN);
    pixel(&mut p, 511, 2, GREEN);
    p.render_line(0);
    assert_eq!(p.frame.len(), 256 * 224 * 4);
    p.write(5, 6);
    p.render_line(0);
    assert_eq!(p.frame.len(), 512 * 224 * 4);
    p.write(5, 1);
    p.render_line(0);
    assert_eq!(p.frame.len(), 256 * 224 * 4);
}

fn bounded_frames(m: &mut Machine, count: u64) {
    let target = m.bus.frames + count;
    let start = m.cpu.instructions;
    while m.bus.frames < target && m.cpu.instructions - start < 300_000 {
        m.step();
        assert!(!m.cpu.stopped);
    }
    assert_eq!(m.bus.frames, target, "guest instruction deadline");
}
fn machine_pixel(m: &Machine, x: usize, expected: [u8; 4]) {
    assert_eq!(&m.framebuffer()[x * 4..x * 4 + 4], expected, "guest x={x}");
}
#[test]
fn source_built_cpu_guests_modes_5_6_and_low_resolution_control() {
    for mode in [1, 5, 6] {
        let mut m = Machine::new(&hires::guest_rom(mode, false)).unwrap();
        bounded_frames(&mut m, 5);
        assert_eq!(m.bus.peek(0x30), 0x49);
        assert_eq!(m.bus.peek(0x31), mode);
        let width = if mode == 1 { 256 } else { 512 };
        assert_eq!(
            (
                m.framebuffer_width(),
                m.framebuffer_height(),
                m.framebuffer().len()
            ),
            (width, 224, width * 224 * 4)
        );
        let even = if mode == 5 { [255, 255, 0, 255] } else { RED };
        for x in [0, 6, width - 8, width - 2] {
            let expected = if mode == 1 {
                RED
            } else if x % 16 >= 8 {
                if mode == 5 {
                    [0, 255, 255, 255]
                } else {
                    WHITE
                }
            } else {
                even
            };
            machine_pixel(&m, x, expected);
        }
        machine_pixel(&m, 1, GREEN);
        machine_pixel(&m, width - 1, if mode == 1 { GREEN } else { BLUE });
    }
}
#[test]
fn complete_frame_transitions_reset_and_cartridge_replacement() {
    for mode in [5, 6] {
        let rom = hires::guest_rom(mode, true);
        let mut m = Machine::new(&rom).unwrap();
        bounded_frames(&mut m, 5);
        for (buttons, width) in [(0, 256), (0x80, 512), (0, 256)] {
            m.set_buttons(buttons);
            bounded_frames(&mut m, 3);
            assert_eq!(
                (m.framebuffer_width(), m.framebuffer().len()),
                (width, width * 224 * 4)
            );
            machine_pixel(&m, 1, GREEN);
            machine_pixel(&m, width - 1, if width == 256 { GREEN } else { BLUE });
        }
        m.set_buttons(0x80);
        bounded_frames(&mut m, 3);
        m.reset();
        assert_eq!(
            (m.framebuffer_width(), m.framebuffer().len()),
            (256, 256 * 224 * 4)
        );
        bounded_frames(&mut m, 5);
        machine_pixel(&m, 255, GREEN);
        m = Machine::new(&hires::guest_rom(6, false)).unwrap();
        bounded_frames(&mut m, 5);
        machine_pixel(&m, 511, BLUE);
        m = Machine::new(&hires::guest_rom(1, false)).unwrap();
        bounded_frames(&mut m, 5);
        machine_pixel(&m, 255, GREEN);
    }
}

#[test]
fn runner_ppm_header_payload_and_edges() {
    let directory = std::env::temp_dir().join(format!("snes-hires-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    for mode in [1, 5, 6] {
        let rom = directory.join(format!("{mode}.sfc"));
        let ppm = directory.join(format!("{mode}.ppm"));
        std::fs::write(&rom, hires::guest_rom(mode, false)).unwrap();
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_snes-run"))
            .arg(rom)
            .args([
                "--frames",
                "5",
                "--timeout",
                "10",
                "--expect",
                "0x30=0x49",
                "--ppm",
            ])
            .arg(&ppm)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let data = std::fs::read(ppm).unwrap();
        let width = if mode == 1 { 256 } else { 512 };
        let header = format!("P6\n{width} 224\n255\n");
        assert!(data.starts_with(header.as_bytes()));
        assert_eq!(data.len(), header.len() + width * 224 * 3);
        let pixels = &data[header.len()..];
        assert_eq!(
            &pixels[..3],
            if mode == 5 {
                &[255, 255, 0]
            } else {
                &[255, 0, 0]
            }
        );
        assert_eq!(&pixels[3..6], &[0, 255, 0]);
        assert_eq!(
            &pixels[pixels.len() - 3..],
            if mode == 1 {
                &[0, 255, 0]
            } else {
                &[0, 0, 255]
            }
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn isolated_low_resolution_control() {
    let mut m = Machine::new(&hires::guest_rom(1, false)).unwrap();
    bounded_frames(&mut m, 5);
    assert_eq!(m.bus.peek(0x30), 0x49);
    assert_eq!(
        (
            m.framebuffer_width(),
            m.framebuffer_height(),
            m.framebuffer().len()
        ),
        (256, 224, 256 * 224 * 4)
    );
    machine_pixel(&m, 0, RED);
    machine_pixel(&m, 1, GREEN);
    machine_pixel(&m, 255, GREEN);
}

#[test]
fn forced_blank_uses_the_current_packed_stride() {
    let mut p = scene(1, 0, 8, 0);
    for mode in [1, 5, 6, 1] {
        p.write(5, mode);
        p.write(0, 0x80);
        for y in 0..224 {
            p.render_line(y);
        }
        let width = if mode == 1 { 256 } else { 512 };
        assert_eq!(
            (p.framebuffer_width(), p.frame.len()),
            (width, width * 224 * 4)
        );
        assert!(p.frame.chunks_exact(4).all(|rgba| rgba == [0, 0, 0, 255]));
    }
}

#[test]
fn both_modes_independent_main_sub_enables_and_backdrops() {
    for mode in [5, 6] {
        let mut p = scene(mode, 0, 8, 0);
        character(&mut p, 0x2000, 0, 4, |x, _| {
            if x >= 4 {
                0
            } else {
                1 + x as u8 % 2
            }
        });
        p.write(0x32, 0x3f); // Fixed magenta versus CGRAM blue.
        let magenta = [255, 0, 255, 255];
        for (tm, ts, pair) in [
            (0, 0, [magenta, BLUE]),
            (1, 0, [magenta, GREEN]),
            (0, 1, [RED, BLUE]),
            (1, 1, [RED, GREEN]),
        ] {
            p.write(0x2c, tm);
            p.write(0x2d, ts);
            pixel(&mut p, 0, 0, pair[0]);
            pixel(&mut p, 1, 0, pair[1]);
            pixel(&mut p, 4, 0, magenta);
            pixel(&mut p, 5, 0, BLUE);
        }
    }
}
