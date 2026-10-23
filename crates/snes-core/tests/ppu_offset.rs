//! Original MIT test graphics, coordinate vectors and 65C816 programs for #19.
//! Copyright (c) 2026 snes-rust contributors.
use snes_core::{ppu::Ppu, Machine};

fn word(p: &mut Ppu, address: usize, value: u16) {
    for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
        p.vram[(address + i) & 65535] = byte;
    }
}

fn scroll(p: &mut Ppu, bg: usize, h: u16, v: u16) {
    for (axis, value) in [h, v].into_iter().enumerate() {
        let reg = (0x0d + bg * 2 + axis) as u8;
        p.write(reg, value as u8);
        p.write(reg, (value >> 8) as u8);
    }
}

fn depth(mode: u8, bg: usize) -> usize {
    match (mode, bg) {
        (4, 0) => 8,
        (4, 1) => 2,
        _ => 4,
    }
}

// A coordinate-labelled texture, unrelated to the offset algorithm. Map words
// select 8x8 groups of characters and palettes. Each character has a nonuniform
// pixel pattern, so fine scroll, row selection and flips remain observable.
fn scene(mode: u8, bg: usize, size: usize) -> Ppu {
    let mut p = Ppu::default();
    p.write(0, 15);
    p.write(5, mode | if size == 16 { 16 << bg } else { 0 });
    p.write(7 + bg as u8, 0x63); // Four pages at byte $c000.
    p.write(9, 0x70); // Offset table at $e000, BG3 8x8 by default.
    p.write(0x0b, if bg == 0 { 1 } else { 0x10 });
    p.write(0x2c, 1 << bg); // BG3 disabled on both screens in every scene.
    let bpp = depth(mode, bg);
    for i in 0..256 {
        let color = color_word(i);
        p.cgram[i * 2..i * 2 + 2].copy_from_slice(&color.to_le_bytes());
        for y in 0..8 {
            for x in 0..8 {
                let pixel = 1 + (i + x + y * 3) % ((1 << bpp) - 1);
                for plane in 0..bpp {
                    p.vram[0x2000 + i * bpp * 8 + plane / 2 * 16 + plane % 2 + y * 2] |=
                        (((pixel >> plane) & 1) as u8) << (7 - x);
                }
            }
        }
    }
    for page in 0..4 {
        for row in 0..32 {
            for col in 0..32 {
                let tx = col + (page % 2) * 32;
                let ty = row + (page / 2) * 32;
                let tile = (tx % 8) * 2 + (ty % 8) * 32;
                let palette = (tx / 8 + ty / 8 * 3) % 8;
                word(
                    &mut p,
                    0xc000 + page * 2048 + row * 64 + col * 2,
                    (tile | (palette << 10)) as u16,
                );
            }
        }
    }
    p
}

fn color_word(i: usize) -> u16 {
    ((i * 3 % 32) | ((i * 7 % 32) << 5) | ((i * 11 % 32) << 10)) as u16
}

fn rgba(color: u16) -> [u8; 4] {
    [
        ((color & 31) * 255 / 31) as u8,
        (((color >> 5) & 31) * 255 / 31) as u8,
        (((color >> 10) & 31) * 255 / 31) as u8,
        255,
    ]
}

fn source_color(bpp: usize, size: usize, sx: usize, sy: usize, flips: u16) -> u16 {
    let tx = sx / size % 64;
    let ty = sy / size % 64;
    let mut px = sx % size;
    let mut py = sy % size;
    if flips & 0x4000 != 0 {
        px = size - 1 - px;
    }
    if flips & 0x8000 != 0 {
        py = size - 1 - py;
    }
    let tile = (tx % 8) * 2 + (ty % 8) * 32 + px / 8 + py / 8 * 16;
    let pixel = 1 + (tile + px % 8 + py % 8 * 3) % ((1 << bpp) - 1);
    color_word(
        pixel
            + if bpp == 8 {
                0
            } else {
                (tx / 8 + ty / 8 * 3) % 8 * (1 << bpp)
            },
    )
}

fn check(p: &mut Ppu, bg: usize, size: usize, x: usize, y: usize, source: (usize, usize)) {
    p.render_line(y);
    let offset = (y * 256 + x) * 4;
    let expected = source_color(depth(p.regs[5] & 7, bg), size, source.0, source.1, 0);
    assert_eq!(
        &p.frame[offset..offset + 4],
        &rgba(expected),
        "BG{}, size={size}, screen=({x},{y}), source={source:?}",
        bg + 1
    );
}

fn rows(p: &mut Ppu, h: u16, v: u16) {
    for col in 0..32 {
        word(p, 0xe000 + col * 2, h);
        word(p, 0xe040 + col * 2, v);
    }
}

#[test]
fn disabled_offsets_retain_ordinary_scroll() {
    for mode in [2, 4] {
        for bg in 0..2 {
            for size in [8, 16] {
                let mut p = scene(mode, bg, size);
                scroll(&mut p, bg, 35, 19);
                rows(&mut p, 0x9fff, 0x9fed); // All unused bits, no enables.
                for y in [0, 6, 17] {
                    for x in [0, 4, 5, 12, 13, 21, 50, 255] {
                        check(&mut p, bg, size, x, y, (x + 35, y + 20));
                    }
                }
            }
        }
    }
}

#[test]
fn mode2_independent_axes_and_all_enable_masks() {
    for bg in 0..2 {
        for size in [8, 16] {
            for hm in [0, 0x2000, 0x4000, 0x6000] {
                for vm in [0, 0x2000, 0x4000, 0x6000] {
                    let mut p = scene(2, bg, size);
                    scroll(&mut p, bg, 35, 19);
                    // H=40 (low three bits ignored), V=53. Bit 15 and
                    // bits 10..12 have no meaning in Mode 2.
                    rows(&mut p, hm | 0x9c2f, vm | 0x9c35);
                    for y in [0, 6, 17] {
                        for x in [0, 4, 5, 6, 12, 13, 20, 21, 50] {
                            let sx = if x >= 5 && hm & (0x2000 << bg) != 0 {
                                x + 43
                            } else {
                                x + 35
                            };
                            let sy = if x >= 5 && vm & (0x2000 << bg) != 0 {
                                y + 54
                            } else {
                                y + 20
                            };
                            check(&mut p, bg, size, x, y, (sx, sy));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn mode4_axis_selector_and_ignored_following_row() {
    for bg in 0..2 {
        for size in [8, 16] {
            for mask in [0, 0x2000, 0x4000, 0x6000] {
                for vertical in [false, true] {
                    let mut p = scene(4, bg, size);
                    scroll(&mut p, bg, 35, 19);
                    rows(
                        &mut p,
                        mask | 0x1c35 | if vertical { 0x8000 } else { 0 },
                        0x63f1,
                    );
                    for y in [0, 6, 17] {
                        for x in [0, 4, 5, 12, 13, 21, 50] {
                            let enabled = x >= 5 && mask & (0x2000 << bg) != 0;
                            let sx = if enabled && !vertical { x + 51 } else { x + 35 };
                            let sy = if enabled && vertical { y + 54 } else { y + 20 };
                            check(&mut p, bg, size, x, y, (sx, sy));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn all_fine_scrolls_first_boundary_successive_columns_and_bg3_scroll() {
    for mode in [2, 4] {
        for bg in 0..2 {
            for size in [8, 16] {
                for fine in 0..8 {
                    for table_fine in 0..8 {
                        let mut p = scene(mode, bg, size);
                        scroll(&mut p, bg, 32 + fine, 19);
                        scroll(&mut p, 2, 24 + table_fine, 17);
                        // BG3 row 2, columns 3,4,5. Other words remain disabled.
                        for (col, h) in [(3, 64), (4, 128), (5, 192)] {
                            word(&mut p, 0xe080 + col * 2, 0x6000 | h);
                        }
                        let edge = 8 - fine as usize;
                        for y in [0, 19] {
                            for (x, sx) in [
                                (edge - 1, 39),
                                (edge, 72),
                                (edge + 7, 79),
                                (edge + 8, 144),
                                (edge + 15, 151),
                                (edge + 16, 216),
                                (edge + 24, 64),
                            ] {
                                check(&mut p, bg, size, x, y, (sx, y + 20));
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn ten_bit_source_wrap_and_bg3_row_wrap() {
    for bg in 0..2 {
        for size in [8, 16] {
            let mut p = scene(2, bg, size);
            scroll(&mut p, bg, 1023, 999);
            scroll(&mut p, 2, 0, 1020);
            // BG3 32x32 repeats row 127 as row 31; +8 wraps to row 0.
            word(&mut p, 0xe7c0, 0x63ff); // H coarse=1016.
            word(&mut p, 0xe000, 0x63fa); // V=1018.
            for (x, y, source) in [
                (0, 10, (1023, 1010)),
                (1, 10, (0, 5)),
                (2, 0, (1, 1019)),
                (8, 10, (7, 5)),
            ] {
                check(&mut p, bg, size, x, y, source);
            }
        }
    }
}

#[test]
fn bg3_screen_layout_page_boundaries_and_vram_wrap() {
    // Literal byte addresses: base $e000, H at pixel (248,248), V at
    // (248,256), next column H at (256,248), V at (256,256).
    for (layout, addresses) in [
        (0, [0xe7fe, 0xe03e, 0xe7c0, 0xe000]),
        (1, [0xe7fe, 0xe03e, 0xefc0, 0xe800]),
        (2, [0xe7fe, 0xe83e, 0xe7c0, 0xe800]),
        (3, [0xe7fe, 0xf03e, 0xefc0, 0xf800]),
    ] {
        for base in [0x70, 0xfc] {
            let mut p = scene(2, 0, 8);
            p.write(9, base | layout);
            scroll(&mut p, 2, 248, 248);
            // Relocate the same vectors from $e000 to $1f800; vw wraps.
            let delta = if base == 0xfc { 0x11800 } else { 0 };
            for (address, value) in addresses.into_iter().zip([0x2040, 0x2035, 0x2080, 0x2049]) {
                word(&mut p, address + delta, value);
            }
            check(&mut p, 0, 8, 8, 0, (72, 54));
            check(&mut p, 0, 8, 16, 0, (144, 74));
        }
    }
}

#[test]
fn bg3_large_tiles_repeat_descriptors_without_character_interpretation() {
    for mode in [2, 4] {
        for bg in 0..2 {
            for size in [8, 16] {
                let mut p = scene(mode, bg, size);
                p.write(5, mode | 0x40 | if size == 16 { 16 << bg } else { 0 });
                // BG3 y=0 and y=8 select the very same descriptor in Mode 2.
                word(&mut p, 0xe000, 0x6045);
                word(&mut p, 0xe002, 0x6087);
                for (x, source) in [
                    (8, (72, if mode == 2 { 70 } else { 1 })),
                    (16, (80, if mode == 2 { 70 } else { 1 })),
                    (24, (152, if mode == 2 { 136 } else { 1 })),
                ] {
                    check(&mut p, bg, size, x, 0, source);
                }
                // y=8 crosses to the next 16-pixel row only for V (y+8=16).
                scroll(&mut p, 2, 0, 8);
                word(&mut p, 0xe040, 0x6033);
                check(&mut p, bg, size, 8, 0, (72, if mode == 2 { 52 } else { 1 }));
                // BG3 16x16 page crossing at x=512,y=512, not 256.
                p.write(9, 0x73);
                scroll(&mut p, 2, 504, 504);
                for (a, w) in [
                    (0xe7fe, 0x6040),
                    (0xf03e, 0x6035),
                    (0xefc0, 0x6080),
                    (0xf800, 0x6049),
                ] {
                    word(&mut p, a, w);
                }
                check(&mut p, bg, size, 8, 0, (72, if mode == 2 { 54 } else { 1 }));
                check(
                    &mut p,
                    bg,
                    size,
                    16,
                    0,
                    (144, if mode == 2 { 74 } else { 1 }),
                );
            }
        }
    }
}

#[test]
fn offset_coordinates_feed_character_flips_for_both_tile_sizes() {
    for mode in [2, 4] {
        for bg in 0..2 {
            for size in [8, 16] {
                for flips in [0, 0x4000, 0x8000, 0xc000] {
                    let mut p = scene(mode, bg, size);
                    rows(&mut p, 0x6040, 0x6030);
                    for a in (0xc000..0xe000).step_by(2) {
                        let entry = u16::from_le_bytes([p.vram[a], p.vram[a + 1]]);
                        word(&mut p, a, entry | flips);
                    }
                    for y in [0, 6, 9] {
                        p.render_line(y);
                        for x in [8, 9, 15, 16] {
                            let sy = y + if mode == 2 { 49 } else { 1 };
                            let expected = source_color(depth(mode, bg), size, x + 64, sy, flips);
                            let offset = (y * 256 + x) * 4;
                            assert_eq!(&p.frame[offset..offset + 4], &rgba(expected));
                        }
                    }
                }
            }
        }
    }
}

fn solid(p: &mut Ppu, address: usize, bpp: usize, pixel: u8) {
    for plane in 0..bpp {
        for row in 0..8 {
            p.vram[address + plane / 2 * 16 + plane % 2 + row * 2] =
                if pixel & (1 << plane) == 0 { 0 } else { 255 };
        }
    }
}

fn pixel(p: &mut Ppu, x: usize, expected: [u8; 4]) {
    p.render_line(0);
    assert_eq!(&p.frame[x * 4..x * 4 + 4], &expected, "x={x}");
}

#[test]
fn offset_composition_priority_transparency_and_mode4_direct_color() {
    let mut p = Ppu::default();
    for (r, v) in [
        (0, 15),
        (5, 4),
        (7, 0x60),
        (8, 0x68),
        (9, 0x70),
        (0x0b, 0x41),
        (0x2c, 3),
        (0x30, 1),
    ] {
        p.write(r, v);
    }
    // BG1 source x=40 -> tile 1, palette 7: direct (6,10,12).
    // BG2 source x=40 -> tile 1, raw 3: CGRAM blue.
    solid(&mut p, 0x2040, 8, 0x51);
    solid(&mut p, 0x8010, 2, 3);
    p.cgram[6..8].copy_from_slice(&0x7c00u16.to_le_bytes());
    word(&mut p, 0xc00a, 0x1c01);
    word(&mut p, 0xd00a, 0x2001); // High BG2 beats low BG1.
    word(&mut p, 0xe000, 0x6020);
    pixel(&mut p, 8, [0, 0, 255, 255]);
    word(&mut p, 0xc00a, 0x3c01); // High BG1 beats high BG2.
    pixel(&mut p, 8, [49, 82, 98, 255]);
    solid(&mut p, 0x2040, 8, 0); // Palette 7 raw zero stays transparent.
    pixel(&mut p, 8, [0, 0, 255, 255]);
    solid(&mut p, 0x2040, 8, 0x51);
    p.write(0x2c, 0);
    p.write(0x2d, 1); // Offset BG1 is the subscreen operand; BG3 still off.
    p.write(0x30, 3);
    p.write(0x31, 0x20); // Black main backdrop plus direct BG1.
    pixel(&mut p, 8, [49, 82, 98, 255]);
    pixel(&mut p, 7, [0, 0, 0, 255]); // Initial unaffected transparent region.
    p.write(0x2d, 2);
    pixel(&mut p, 8, [0, 0, 255, 255]);
    p.write(0x2d, 0);
    pixel(&mut p, 8, [0, 0, 0, 255]);
}

// A bounded, original LoROM generator. LDA #byte; STA $address writes every
// register, VRAM word and CGRAM color through the emulated CPU during forced
// blank. No fixture binary, assembler, firmware or host PPU patch is used.
fn guest_rom(mode: u8, enabled: bool, vertical: bool) -> Vec<u8> {
    let mut code = vec![0x78]; // SEI; reset supplies emulation/8-bit A.
    let mut write = |address: u16, value: u8| {
        code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
    };
    for (reg, value) in [
        (0x2100, 0x80),
        (0x2105, mode),
        (0x2107, 0x60),
        (0x2109, 0x70),
        (0x210b, 1),
        (0x212c, 1),
        (0x212d, 0),
        (0x2130, 0),
        (0x2131, 0),
        (0x2115, 0x80),
    ] {
        write(reg, value);
    }
    // Ordinary scroll (3,0); first eligible x=5. BG3 fine H=7 is ignored.
    for (reg, value) in [(0x210d, 3u16), (0x210e, 0), (0x2111, 7), (0x2112, 0)] {
        write(reg, value as u8);
        write(reg, (value >> 8) as u8);
    }
    write(0x2121, 0);
    for color in [0x03e0u16, 0x001f, 0x7c00, 0x7fff] {
        // green, red, blue, white
        write(0x2122, color as u8);
        write(0x2122, (color >> 8) as u8);
    }
    // Characters 0,1,2 are solid red, blue and white, respectively.
    write(0x2116, 0);
    write(0x2117, 0x10);
    let bpp = if mode == 4 { 8 } else { 4 };
    for value in [1u8, 2, 3] {
        for pair in 0..bpp / 2 {
            for _ in 0..8 {
                write(
                    0x2118,
                    if value & (1 << (pair * 2)) != 0 {
                        255
                    } else {
                        0
                    },
                );
                write(
                    0x2119,
                    if value & (2 << (pair * 2)) != 0 {
                        255
                    } else {
                        0
                    },
                );
            }
        }
    }
    // Map row0 columns 0..7 are red except col5=blue. Row4 is white.
    for (address, entries) in [(0x6000u16, [0u16, 0, 0, 0, 0, 1, 0, 0]), (0x6080, [2; 8])] {
        write(0x2116, address as u8);
        write(0x2117, (address >> 8) as u8);
        for entry in entries {
            write(0x2118, entry as u8);
            write(0x2119, (entry >> 8) as u8);
        }
    }
    // Mode2 chooses H and V independently; Mode4 selects one via bit15.
    let mask = if enabled { 0x2000 } else { 0 };
    let h = mask | 32 | if mode == 4 && vertical { 0x8000 } else { 0 };
    for (address, value) in [
        (0x7000u16, if mode == 2 && vertical { 0 } else { h }),
        (
            0x7020,
            if mode == 2 {
                if vertical {
                    mask | 32
                } else {
                    0
                }
            } else {
                0x23f0
            },
        ),
    ] {
        write(0x2116, address as u8);
        write(0x2117, (address >> 8) as u8);
        for _ in 0..3 {
            write(0x2118, value as u8);
            write(0x2119, (value >> 8) as u8);
        }
    }
    write(0x2100, 15);
    code.extend([0x80, 0xfe]); // BRA forever, bounded by the host test.
    assert!(code.len() < 0x7fc0);
    let mut rom = vec![0; 0x8000];
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"OFFSET TABLE ORIGINAL");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&0x8000u16.to_le_bytes());
    rom
}

fn run_guest(mode: u8, enabled: bool) {
    for vertical in [false, true] {
        let mut m = Machine::new(&guest_rom(mode, enabled, vertical)).unwrap();
        let mut instructions = 0;
        while m.bus.frames < 3 && instructions < 150_000 {
            m.step();
            instructions += 1;
            assert!(!m.cpu.stopped);
        }
        assert_eq!(m.bus.frames, 3, "guest instruction deadline");
        // x=4: ordinary source x=7 (red). x=5: replacement source x=40
        // (blue) or y=33 (white); disabled: ordinary source x=8 (red).
        for (x, expected) in [
            (4, [255, 0, 0, 255]),
            (
                5,
                if !enabled {
                    [255, 0, 0, 255]
                } else if vertical {
                    [255; 4]
                } else {
                    [0, 0, 255, 255]
                },
            ),
            (
                12,
                if !enabled {
                    [255, 0, 0, 255]
                } else if vertical {
                    [255; 4]
                } else {
                    [0, 0, 255, 255]
                },
            ),
        ] {
            assert_eq!(
                &m.framebuffer()[x * 4..x * 4 + 4],
                &expected,
                "mode={mode}, enabled={enabled}, vertical={vertical}, x={x}"
            );
        }
    }
}

#[test]
fn source_built_mode2_enabled() {
    run_guest(2, true);
}
#[test]
fn source_built_mode4_enabled() {
    run_guest(4, true);
}
#[test]
fn source_built_guests_disabled_controls() {
    run_guest(2, false);
    run_guest(4, false);
}
