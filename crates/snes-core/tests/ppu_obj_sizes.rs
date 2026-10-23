//! Original MIT OBJ images, numerical vectors and CPU guests for Issue #55.
//! Copyright (c) 2026 snes-rust contributors.
use snes_core::{ppu::Ppu, Machine};
#[path = "common/obj_sizes.rs"]
mod obj_sizes;

const COLORS: [[u8; 4]; 8] = [
    [0, 0, 0, 255],
    [255, 0, 0, 255],
    [0, 255, 0, 255],
    [0, 0, 255, 255],
    [255, 255, 0, 255],
    [0, 255, 255, 255],
    [255, 0, 255, 255],
    [255, 255, 255, 255],
];
const PALETTE: [u16; 8] = [0, 31, 0x3e0, 0x7c00, 0x3ff, 0x7fe0, 0x7c1f, 0x7fff];

fn scene(selector: u8, large: bool, x: i16, y: u8, flip: u8) -> Ppu {
    let mut p = Ppu::default();
    p.write(0, 0x80);
    p.write(1, selector << 5);
    p.write(0x2c, 16);
    for n in 0..128 {
        p.oam[n * 4 + 1] = 224; // All unused small objects are below the frame.
    }
    p.oam[..4].copy_from_slice(&[x as u8, y, 0, flip]);
    p.oam[512] = u8::from(x < 0) | (u8::from(large) << 1);
    for (i, c) in PALETTE.iter().enumerate() {
        p.cgram[(128 + i) * 2..(129 + i) * 2].copy_from_slice(&c.to_le_bytes());
    }
    p.write(0, 15);
    p
}
fn tile(p: &mut Ppu, address: usize, image: &[u8]) {
    for y in 0..8 {
        for plane in 0..4 {
            let mut byte = 0;
            for x in 0..8 {
                byte |= ((image[y * 8 + x] >> plane) & 1) << (7 - x);
            }
            p.vram[(address + y * 2 + plane / 2 * 16 + plane % 2) % 65536] = byte;
        }
    }
}
// An original linear image; no production addressing/pixel helper is used.
fn image(w: usize, h: usize) -> Vec<u8> {
    (0..h)
        .flat_map(|y| {
            (0..w)
                .map(move |x| ((x * 3 + y * 5 + x / 8 * 2 + y / 8 * 3 + y / 16 * 2) % 7 + 1) as u8)
        })
        .collect()
}
fn encode(p: &mut Ppu, source: &[u8], w: usize) {
    for (ty, rows) in source.chunks(w * 8).enumerate() {
        for tx in 0..w / 8 {
            let crop: Vec<u8> = (0..8)
                .flat_map(|y| rows[y * w + tx * 8..y * w + tx * 8 + 8].iter().copied())
                .collect();
            tile(p, (ty * 16 + tx) * 32, &crop);
        }
    }
}
// Flip an image by reversing rows of each independently cropped square and/or
// pixels of each row. This oracle never computes tile numbers or XOR rows.
fn flipped(source: &[u8], w: usize, flip: u8) -> Vec<u8> {
    source
        .chunks(w * w)
        .flat_map(|square| {
            let mut rows: Vec<Vec<u8>> = square.chunks(w).map(<[u8]>::to_vec).collect();
            if flip & 128 != 0 {
                rows.reverse();
            }
            if flip & 64 != 0 {
                for row in &mut rows {
                    row.reverse();
                }
            }
            rows.concat()
        })
        .collect()
}
fn at(p: &Ppu, x: usize, y: usize, c: u8) {
    let i = (y * 256 + x) * 4;
    assert_eq!(&p.frame[i..i + 4], COLORS[c as usize], "pixel ({x},{y})");
}
fn geometry(selector: u8, large: bool, w: usize, h: usize) {
    for flip in [0, 64, 128, 192] {
        let mut p = scene(selector, large, 24, 24, flip);
        let source = image(w, h);
        encode(&mut p, &source, w);
        let expected = flipped(&source, w, flip);
        for y in 23..=24 + h {
            p.render_line(y);
            for x in 23..=24 + w {
                let c = if (24..24 + w).contains(&x) && (24..24 + h).contains(&y) {
                    expected[(y - 24) * w + x - 24]
                } else {
                    0
                };
                at(&p, x, y, c);
            }
        }
    }
}
#[test]
fn square_selectors_and_all_flips() {
    for (selector, small, large) in [
        (0, 8, 16),
        (1, 8, 32),
        (2, 8, 64),
        (3, 16, 32),
        (4, 16, 64),
        (5, 32, 64),
    ] {
        geometry(selector, false, small, small);
        geometry(selector, true, large, large);
    }
    geometry(7, true, 32, 32);
}
#[test]
fn selector6_small_rectangular_height_and_all_flips() {
    geometry(6, false, 16, 32);
}
#[test]
fn selector6_large_rectangular_height_and_all_flips() {
    geometry(6, true, 32, 64);
}
#[test]
fn selector7_small_rectangular_height_and_all_flips() {
    geometry(7, false, 16, 32);
}
#[test]
fn clipped_rectangles_and_wrapped_y_are_image_crops() {
    for (selector, large, w, h) in [(6, false, 16, 32), (6, true, 32, 64), (7, false, 16, 32)] {
        for flip in [0, 64, 128, 192] {
            for (sx, sy) in [(-5i16, 24u8), (249, 24), (24, 248)] {
                let mut p = scene(selector, large, sx, sy, flip);
                let source = image(w, h);
                encode(&mut p, &source, w);
                let expected = flipped(&source, w, flip);
                for y in 0..100 {
                    p.render_line(y);
                    for x in 0..256 {
                        // Independent signed crop, including the copy 256 rows
                        // above the encoded Y coordinate. No wrapped displacement.
                        let dx = x as i16 - sx;
                        let dy = if sy == 248 {
                            y as i16 + 8
                        } else {
                            y as i16 - 24
                        };
                        let c = if dx >= 0 && dx < w as i16 && dy >= 0 && dy < h as i16 {
                            expected[dy as usize * w + dx as usize]
                        } else {
                            0
                        };
                        at(&p, x, y, c);
                    }
                }
            }
        }
    }
}
#[test]
fn character_nibbles_name_tables_and_wrapping_obsel_addresses() {
    // Start at $FF: both character nibbles wrap without carrying into N.
    // Explicit rows/columns independently encode the expected 32x64 image.
    let names = [
        [0xff, 0xf0, 0xf1, 0xf2],
        [0x0f, 0x00, 0x01, 0x02],
        [0x1f, 0x10, 0x11, 0x12],
        [0x2f, 0x20, 0x21, 0x22],
        [0x3f, 0x30, 0x31, 0x32],
        [0x4f, 0x40, 0x41, 0x42],
        [0x5f, 0x50, 0x51, 0x52],
        [0x6f, 0x60, 0x61, 0x62],
    ];
    for (obsel, bases) in [(0xc0, [0, 0x2000]), (0xdf, [0xc000, 0x4000])] {
        for table in 0..2 {
            for flip in [0, 64, 128, 192] {
                let mut p = scene(6, true, 24, 24, flip | table as u8);
                p.write(1, obsel);
                p.oam[2] = 0xff;
                let mut expected = vec![0; 32 * 64];
                for (t, base) in bases.into_iter().enumerate() {
                    for (ty, row) in names.iter().enumerate() {
                        for (tx, name) in row.iter().enumerate() {
                            let color = ((ty * 4 + tx + t * 3) % 7 + 1) as u8;
                            tile(&mut p, base + name * 32, &[color; 64]);
                            if t == table {
                                for y in ty * 8..ty * 8 + 8 {
                                    expected[y * 32 + tx * 8..y * 32 + tx * 8 + 8].fill(color);
                                }
                            }
                        }
                    }
                }
                let expected = flipped(&expected, 32, flip);
                for y in 23..=88 {
                    p.render_line(y);
                    for x in 23..=56 {
                        let c = if (24..56).contains(&x) && (24..88).contains(&y) {
                            expected[(y - 24) * 32 + x - 24]
                        } else {
                            0
                        };
                        at(&p, x, y, c);
                    }
                }
            }
        }
    }
}
#[test]
fn cpu_cartridges_initialize_and_render_rectangles() {
    for selector in [6, 7] {
        let mut m = Machine::new(&obj_sizes::guest_rom(selector)).unwrap();
        let initial = m.cpu.instructions;
        while m.bus.frames < 5 && m.cpu.instructions - initial < 300_000 {
            m.step();
            assert!(!m.cpu.stopped);
        }
        assert_eq!(m.bus.frames, 5, "guest deadline");
        assert_eq!(m.bus.peek(0x30), 0x55);
        assert_eq!(m.bus.peek(0x31), selector);
        assert_eq!((m.framebuffer_width(), m.framebuffer_height()), (256, 224));
        // Literal vectors shared in meaning with the browser check, including
        // both halves, flip boundaries and the final included/excluded row.
        for (x, y, c) in guest_vectors(selector) {
            let i = (y * 256 + x) * 4;
            assert_eq!(
                &m.framebuffer()[i..i + 4],
                COLORS[c],
                "guest {selector} ({x},{y})"
            );
        }
    }
}
fn guest_vectors(selector: u8) -> Vec<(usize, usize, usize)> {
    let mut v = vec![
        (24, 24, 1),
        (25, 24, 4),
        (31, 31, 1),
        (32, 32, 7),
        (24, 39, 2),
        (24, 40, 5),
        (39, 55, 4),
        (24, 56, 0),
        (40, 40, 0),
        (136, 24, 2),
        (136, 39, 1),
        (136, 40, 6),
        (136, 55, 5),
        (80, 55, 6),
        (176, 24, 6),
        (176, 55, 1),
        (23, 40, 0),
        (79, 40, 0),
        (112, 40, 0),
        (208, 40, 0),
    ];
    if selector == 6 {
        v.extend([
            (80, 56, 2),
            (80, 87, 7),
            (80, 88, 0),
            (176, 56, 7),
            (176, 87, 2),
            (176, 88, 0),
        ]);
    } else {
        v.extend([(80, 56, 0), (176, 56, 0)]);
    }
    v
}
