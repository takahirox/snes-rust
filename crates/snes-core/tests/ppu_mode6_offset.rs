//! Original MIT coordinate graphics, literal vectors and CPU guests for #51.
//! Copyright (c) 2026 snes-rust contributors.
use snes_core::{cpu::Bus, ppu::Ppu, Machine};

fn word(p: &mut Ppu, a: usize, value: u16) {
    for (i, b) in value.to_le_bytes().into_iter().enumerate() {
        p.vram[(a + i) & 65535] = b;
    }
}
fn scroll(p: &mut Ppu, bg: usize, h: u16, v: u16) {
    for (axis, value) in [h, v].into_iter().enumerate() {
        let r = (13 + bg * 2 + axis) as u8;
        p.write(r, value as u8);
        p.write(r, (value >> 8) as u8);
    }
}
fn rgba(i: usize) -> [u8; 4] {
    [
        ((i * 3 % 32) * 255 / 31) as u8,
        ((i * 7 % 32) * 255 / 31) as u8,
        ((i * 11 % 32) * 255 / 31) as u8,
        255,
    ]
}
fn character(p: &mut Ppu, n: usize, label: usize) {
    for y in 0..8 {
        for x in 0..8 {
            let color = 1 + (label + x + 3 * y) % 15;
            for plane in 0..4 {
                let a = (0x2000 + n * 32 + y * 2 + plane / 2 * 16 + plane % 2) & 65535;
                p.vram[a] |= (((color >> plane) & 1) as u8) << (7 - x);
            }
        }
    }
}
fn scene(height: usize, layout: u8) -> Ppu {
    let mut p = Ppu::default();
    for (r, v) in [
        (0, 15),
        (5, 6 | if height == 16 { 16 } else { 0 }),
        (7, 0x60 | layout),
        (9, 0x70),
        (0x0b, 1),
        (0x2c, 1),
        (0x2d, 1),
    ] {
        p.write(r, v);
    }
    for i in 0..128 {
        let c = ((i * 3 % 32) | ((i * 7 % 32) << 5) | ((i * 11 % 32) << 10)) as u16;
        p.cgram[i * 2..i * 2 + 2].copy_from_slice(&c.to_le_bytes());
    }
    for n in 0..256 {
        character(&mut p, n, n);
    }
    for page in 0..4 {
        for r in 0..32 {
            for c in 0..32 {
                let tx = c + page % 2 * 32;
                let ty = r + page / 2 * 32;
                let n = tx % 8 * 2 + ty % 8 * 32;
                let pal = (tx / 8 + ty / 8 * 3) % 8;
                word(
                    &mut p,
                    0xc000 + page * 2048 + r * 64 + c * 2,
                    (n | pal << 10) as u16,
                );
            }
        }
    }
    p
}
// Build a coordinate image without reading production maps or calling address
// helpers. Offset expectations below are literal source coordinates/crops.
fn image(height: usize, layout: u8) -> Vec<[u8; 4]> {
    let width = if layout & 1 != 0 { 1024 } else { 512 };
    let rows = height * if layout & 2 != 0 { 64 } else { 32 };
    let mut result = Vec::with_capacity(width * rows);
    for y in 0..rows {
        for x in 0..width {
            let cx = x / 16;
            let cy = y / height;
            let u = x % 16;
            let v = y % height;
            let label = cx % 8 * 2 + cy % 8 * 32 + u / 8 + v / 8 * 16;
            let pal = (cx / 8 + cy / 8 * 3) % 8;
            result.push(rgba(pal * 16 + 1 + (label + u % 8 + 3 * (v % 8)) % 15));
        }
    }
    result
}
fn check(p: &mut Ppu, source: &[[u8; 4]], x: usize, y: usize, sx: usize, sy: usize) {
    p.render_line(y);
    assert_eq!((p.framebuffer_width(), p.framebuffer_height()), (512, 224));
    let width = if p.regs[7] & 1 != 0 { 1024 } else { 512 };
    let rows = source.len() / width;
    let expected = source[(sy % rows) * width + sx % width];
    let i = (y * 512 + x) * 4;
    assert_eq!(
        &p.frame[i..i + 4],
        &expected,
        "sample ({x},{y}), source ({sx},{sy})"
    );
}
fn rows(p: &mut Ppu, h: u16, v: u16) {
    for c in 0..32 {
        word(p, 0xe000 + c * 2, h);
        word(p, 0xe040 + c * 2, v);
    }
}
#[test]
fn disabled_offsets_retain_ordinary_hires() {
    for height in [8, 16] {
        for layout in 0..4 {
            let source = image(height, layout);
            let mut p = scene(height, layout);
            scroll(&mut p, 0, 35, 19);
            rows(&mut p, 0xdfff, 0xdfff); // BG2 enable and all metadata, no BG1.
            for y in [0, 6, 19, 223] {
                for j in [0, 1, 8, 9, 10, 11, 24, 25, 26, 27, 510, 511] {
                    check(&mut p, &source, j, y, j + 70, y + 20);
                }
            }
        }
    }
}
#[test]
fn independent_axes_enable_masks_and_ignored_metadata() {
    for height in [8, 16] {
        let source = image(height, 3);
        for hm in [0, 0x2000, 0x4000, 0x6000] {
            for vm in [0, 0x2000, 0x4000, 0x6000] {
                for metadata in [0, 0x9c00] {
                    let mut p = scene(height, 3);
                    scroll(&mut p, 0, 35, 19);
                    rows(&mut p, hm | metadata | 47, vm | metadata | 53);
                    for y in [0, 6, 19] {
                        for j in [0, 1, 8, 9, 10, 11, 24, 25, 26, 27, 510, 511] {
                            let sx = j + if j >= 10 && hm & 0x2000 != 0 { 86 } else { 70 };
                            let sy = y + if j >= 10 && vm & 0x2000 != 0 { 54 } else { 20 };
                            check(&mut p, &source, j, y, sx, sy);
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn all_fine_residues_and_successive_full_dot_columns() {
    for height in [8, 16] {
        let source = image(height, 3);
        for large in [false, true] {
            for fine in 0..8 {
                for table_fine in 0..8 {
                    let mut p = scene(height, 3);
                    if large {
                        p.write(5, p.regs[5] | 0x40);
                    }
                    scroll(&mut p, 0, 32 + fine, 19);
                    scroll(&mut p, 2, 24 + table_fine, if large { 33 } else { 17 });
                    // Both sizes select row2 for H/V; large BG3 shares words.
                    for (a, h, v) in [(0xe086, 64, 64), (0xe088, 128, 128), (0xe08a, 192, 192)] {
                        word(&mut p, a, 0x2000 | h);
                        if !large {
                            word(&mut p, a + 64, 0x2000 | v);
                        }
                    }
                    let edge = 2 * (8 - fine as usize);
                    for y in [0, 19] {
                        // (distance from initial edge, source x, source y delta).
                        for (delta, sx, sy) in [
                            (0, 144, 65),
                            (1, 145, 65),
                            (14, 158, 65),
                            (15, 159, 65),
                            (16, 288, 129),
                            (17, 289, 129),
                            (30, 302, 129),
                            (31, 303, 129),
                            (32, 432, 193),
                            (33, 433, 193),
                            (48, 128, 20),
                            (49, 129, 20),
                        ] {
                            check(&mut p, &source, edge + delta, y, sx, y + sy);
                        }
                        check(&mut p, &source, edge - 2, y, 78, y + 20);
                        check(&mut p, &source, edge - 1, y, 79, y + 20);
                    }
                }
            }
        }
    }
}
#[test]
fn bg3_layouts_page_edges_heights_and_64k_wrap() {
    for height in [8, 16] {
        let source = image(height, 3);
        for (layout, addresses) in [
            (0, [0xe7fe, 0xe03e, 0xe7c0, 0xe000]),
            (1, [0xe7fe, 0xe03e, 0xefc0, 0xe800]),
            (2, [0xe7fe, 0xe83e, 0xe7c0, 0xe800]),
            (3, [0xe7fe, 0xf03e, 0xefc0, 0xf800]),
        ] {
            for base in [0x70, 0x7c] {
                for large in [false, true] {
                    let mut p = scene(height, 3);
                    if large {
                        p.write(5, p.regs[5] | 0x40);
                    }
                    p.write(9, base | layout);
                    scroll(&mut p, 2, 248, if large { 504 } else { 248 });
                    let delta = if base == 0x7c { 0x1800 } else { 0 };
                    for (a, w) in addresses.into_iter().zip([0x2040, 0x2035, 0x2080, 0x2049]) {
                        word(&mut p, a + delta, w);
                    }
                    for y in [0, 19] {
                        for (j, sx, sy) in [
                            (14, 14, 1),
                            (15, 15, 1),
                            (16, 144, 54),
                            (17, 145, 54),
                            (30, 158, 54),
                            (31, 159, 54),
                            (32, 288, 74),
                            (33, 289, 74),
                        ] {
                            check(&mut p, &source, j, y, sx, y + sy);
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn large_bg3_shared_words_and_vertical_fine_crossing() {
    for height in [8, 16] {
        let source = image(height, 3);
        for v in 0..16 {
            let mut p = scene(height, 3);
            p.write(5, p.regs[5] | 0x40);
            scroll(&mut p, 2, 31, v);
            word(&mut p, 0xe006, 0x2045);
            word(&mut p, 0xe008, 0x2087);
            word(&mut p, 0xe046, 0x2033);
            word(&mut p, 0xe048, 0x2055);
            for (j, sx, sy) in [
                (16, 144, if v < 8 { 70 } else { 52 }),
                (17, 145, if v < 8 { 70 } else { 52 }),
                (32, 288, if v < 8 { 136 } else { 86 }),
                (33, 289, if v < 8 { 136 } else { 86 }),
            ] {
                check(&mut p, &source, j, 0, sx, sy);
            }
        }
    }
}
#[test]
fn source_and_descriptor_table_wrap_both_phases() {
    let source = image(16, 3);
    for large in [false, true] {
        for h3 in [504, 1016] {
            let mut p = scene(16, 3);
            if large {
                p.write(5, p.regs[5] | 0x40);
            }
            p.write(9, 0x73);
            scroll(&mut p, 0, 1023, 999);
            scroll(&mut p, 2, h3, if large { 1016 } else { 1020 });
            // c63/r63, c63/r0, c0/r63, c0/r0, literal four-screen addresses.
            for (a, w) in [
                (0xfffe, 0x23ff),
                (0xe83e, 0x23fa),
                (0xf7c0, 0x2040),
                (0xe000, 0x2035),
            ] {
                word(&mut p, a, w);
            }
            for (j, y, sx, sy) in [
                (0, 10, 1022, 1010),
                (1, 10, 1023, 1010),
                (2, 10, 0, 5),
                (3, 10, 1, 5),
                (16, 10, 14, 5),
                (17, 10, 15, 5),
                (18, 0, 160, 54),
                (19, 0, 161, 54),
            ] {
                check(&mut p, &source, j, y, sx, sy);
            }
        }
    }
}
#[test]
fn offsets_feed_whole_cell_flips_and_character_number_wrap() {
    for height in [8, 16] {
        for (flip, columns) in [
            (0, [0, 7, 8, 15]),
            (0x4000, [15, 8, 7, 0]),
            (0x8000, [0, 7, 8, 15]),
            (0xc000, [15, 8, 7, 0]),
        ] {
            let mut p = scene(height, 3);
            // Offset source begins at (80,0) -> map column5,row0.
            rows(&mut p, 0x2020, 0x23ff);
            word(&mut p, 0xc00a, 1023 | flip);
            word(&mut p, 0xc04a, 1023 | flip);
            // Wrapped character at $9fe0, then $2000,$21e0,$2200.
            for (n, label) in [(1023, 0), (0, 1), (15, 2), (16, 3)] {
                for i in 0..32 {
                    p.vram[(0x2000 + n * 32 + i) & 65535] = 0;
                }
                character(&mut p, n, label);
            }
            for y in [0, 7, 8, 15] {
                let row = y % height;
                let v = if flip & 0x8000 != 0 {
                    height - 1 - row
                } else {
                    row
                };
                for (j, u) in [16, 23, 24, 31].into_iter().zip(columns) {
                    p.render_line(y);
                    let expected = rgba(1 + (u / 8 + v / 8 * 2 + u % 8 + 3 * (v % 8)) % 15);
                    let i = (y * 512 + j) * 4;
                    assert_eq!(
                        &p.frame[i..i + 4],
                        &expected,
                        "flip={flip:x},height={height},j={j},y={y}"
                    );
                }
            }
        }
    }
}
#[test]
fn bg3_hidden_and_main_sub_phase_controls() {
    let source = image(8, 3);
    let mut p = scene(8, 3);
    rows(&mut p, 0x2040, 0x2035);
    for mask in [1, 7] {
        // Enabling BG2/3 cannot make either visible in Mode6.
        p.write(0x2c, mask);
        p.write(0x2d, mask);
        for (j, sx) in [(16, 144), (17, 145), (510, 638), (511, 639)] {
            check(&mut p, &source, j, 0, sx, 54);
        }
    }
    p.write(0x2c, 4); // Main BG3 alone is backdrop, sub BG1 remains offset.
    check(&mut p, &source, 16, 0, 144, 54);
    p.render_line(0);
    assert_eq!(&p.frame[17 * 4..18 * 4], &rgba(0));
    p.write(0x2c, 1);
    p.write(0x2d, 4);
    check(&mut p, &source, 17, 0, 145, 54);
    p.render_line(0);
    assert_eq!(&p.frame[16 * 4..17 * 4], &[0, 0, 0, 255]);
}

fn guest_rom(h: bool, v: bool, fine: u8) -> Vec<u8> {
    let mut code = vec![0x78, 0xd8];
    let mut write = |r: u16, b: u8| {
        code.extend([0xa9, b, 0x8d, r as u8, (r >> 8) as u8]);
    };
    for (r, b) in [
        (0x2100, 0x80),
        (0x2105, 6),
        (0x2106, 0),
        (0x2107, 0x60),
        (0x2109, 0x70),
        (0x210b, 1),
        (0x212c, 1),
        (0x212d, 1),
        (0x2130, 0),
        (0x2131, 0),
        (0x2133, 0),
        (0x2115, 0x80),
        (0x4200, 0),
        (0x420c, 0),
    ] {
        write(r, b);
    }
    for (r, b) in [(0x210d, fine), (0x210e, 0), (0x2111, 7), (0x2112, 0)] {
        write(r, b);
        write(r, 0);
    }
    write(0x2121, 0);
    for c in [0u16, 0x001f, 0x03e0, 0x7fff, 0x7c00] {
        for b in c.to_le_bytes() {
            write(0x2122, b);
        }
    }
    write(0x2116, 0);
    write(0x2117, 0x10);
    // Red pair, green pair, white pair, blue pair.
    for c in [1u8, 1, 2, 2, 3, 3, 4, 4] {
        for pair in 0..2 {
            for _ in 0..8 {
                for plane in [pair * 2, pair * 2 + 1] {
                    write(
                        if plane % 2 == 0 { 0x2118 } else { 0x2119 },
                        if c & (1 << plane) != 0 { 255 } else { 0 },
                    );
                }
            }
        }
    }
    for (a, entries) in [
        (0x6000u16, [0, 0, 0, 0, 0, 2, 0, 0]),
        (0x6080, [4, 4, 4, 4, 4, 6, 4, 4]),
    ] {
        write(0x2116, a as u8);
        write(0x2117, (a >> 8) as u8);
        for n in entries {
            write(0x2118, n);
            write(0x2119, 0);
        }
    }
    for (a, w) in [
        (0x7000u16, 32u16 | if h { 0x2000 } else { 0 }),
        (0x7020, 32 | if v { 0x2000 } else { 0 }),
    ] {
        write(0x2116, a as u8);
        write(0x2117, (a >> 8) as u8);
        for _ in 0..3 {
            write(0x2118, w as u8);
            write(0x2119, (w >> 8) as u8);
        }
    }
    write(0x0030, 0x51);
    write(0x2100, 15);
    code.extend([0x80, 0xfe]);
    assert!(code.len() < 0x7fc0);
    let mut rom = vec![0; 32768];
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"MODE6 OFFSET ORIGINAL");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&0x8000u16.to_le_bytes());
    rom
}
fn run_guests(enabled: bool) {
    for fine in [0, 3] {
        for (h, v) in if enabled {
            vec![(true, false), (false, true), (true, true)]
        } else {
            vec![(false, false)]
        } {
            let mut m = Machine::new(&guest_rom(h, v, fine)).unwrap();
            let mut instructions = 0;
            while m.bus.frames < 3 && instructions < 150_000 {
                m.step();
                instructions += 1;
                assert!(!m.cpu.stopped);
            }
            assert_eq!(m.bus.frames, 3, "instruction deadline");
            assert_eq!(m.bus.read(0x30), 0x51);
            assert_eq!(
                (
                    m.framebuffer_width(),
                    m.framebuffer_height(),
                    m.framebuffer().len()
                ),
                (512, 224, 512 * 224 * 4)
            );
            let edge = if fine == 0 { 16 } else { 10 };
            let expected = match (h, v) {
                (true, true) => [0, 0, 255, 255],
                (true, false) => [0, 255, 0, 255],
                (false, true) => [255; 4],
                _ => [255, 0, 0, 255],
            };
            for (j, color) in [
                (0, [255, 0, 0, 255]),
                (1, [255, 0, 0, 255]),
                (edge - 2, [255, 0, 0, 255]),
                (edge - 1, [255, 0, 0, 255]),
                (edge, expected),
                (edge + 1, expected),
                (edge + 14, expected),
                (edge + 15, expected),
            ] {
                assert_eq!(
                    &m.framebuffer()[j * 4..j * 4 + 4],
                    &color,
                    "h={h},v={v},fine={fine},j={j}"
                );
            }
        }
    }
}
#[test]
fn source_built_enabled_guests() {
    run_guests(true);
}
#[test]
fn source_built_disabled_controls() {
    run_guests(false);
}
