//! Original test programs, graphics and numerical oracles for Issue #11.
//! Copyright (c) 2026 snes-rust contributors. MIT, like the repository.
use snes_core::{ppu::Ppu, Machine};

fn cgram(p: &mut Ppu, index: usize, word: u16) {
    p.cgram[index * 2..index * 2 + 2].copy_from_slice(&word.to_le_bytes());
}

fn planar(p: &mut Ppu, base: usize, bpp: usize, pixel: u8) {
    for plane in 0..bpp {
        for row in 0..8 {
            p.vram[base + plane / 2 * 16 + plane % 2 + row * 2] =
                if pixel & (1 << plane) != 0 { 255 } else { 0 };
        }
    }
}

fn scene(mode: u8, pixel: u8, palette: u8) -> Ppu {
    let mut p = Ppu::default();
    p.write(0, 15);
    p.write(5, mode);
    p.write(0x2c, 1);
    cgram(&mut p, 0, 0x03e0); // Green backdrop distinguishes transparency.
    cgram(&mut p, pixel as usize, 0x7c00);
    if mode == 7 {
        // Identity matrix; tile 1 avoids overlap with the tilemap at word 0.
        for reg in [0x1b, 0x1e] {
            p.write(reg, 0);
            p.write(reg, 1);
        }
        p.vram[0] = 1;
        for offset in (129..256).step_by(2) {
            p.vram[offset] = pixel;
        }
    } else {
        p.write(7, 0x60); // Tilemap at byte $c000.
        p.write(0x0b, 1); // BG1 characters at byte $2000.
        p.vram[0xc000..0xc002].copy_from_slice(&((palette as u16) << 10).to_le_bytes());
        planar(&mut p, 0x2000, 8, pixel);
    }
    p
}

fn assert_pixel(p: &mut Ppu, x: usize, rgba: [u8; 4]) {
    p.render_line(0);
    assert_eq!(&p.frame[x * 4..x * 4 + 4], &rgba);
}

#[test]
fn direct_color_rendering_selects_channels_and_bypasses_cgram() {
    // Fixed vectors, independently expressed as five-bit R/G/B components.
    for mode in [3, 4, 7] {
        for (pixel, rgb) in [
            (1, [4, 0, 0]),
            (2, [8, 0, 0]),
            (4, [16, 0, 0]),
            (8, [0, 4, 0]),
            (16, [0, 8, 0]),
            (32, [0, 16, 0]),
            (64, [0, 0, 8]),
            (128, [0, 0, 16]),
            (255, [28, 28, 24]),
        ] {
            for palette in 0..8 {
                let mut p = scene(mode, pixel, palette);
                for palette_color in [0x7c00, 0x03e0] {
                    cgram(&mut p, pixel as usize, palette_color);
                    p.write(0x30, 0);
                    assert_pixel(
                        &mut p,
                        0,
                        if palette_color == 0x7c00 {
                            [0, 0, 255, 255]
                        } else {
                            [0, 255, 0, 255]
                        },
                    );
                    let t = if mode == 7 { 0 } else { palette };
                    let components = [
                        rgb[0] + (t & 1) * 2,
                        rgb[1] + ((t / 2) & 1) * 2,
                        rgb[2] + ((t / 4) & 1) * 4,
                    ];
                    let mut expected = [0, 0, 0, 255];
                    for channel in 0..3 {
                        expected[channel] = (components[channel] as u16 * 255 / 31) as u8;
                    }
                    p.write(0x30, 1);
                    assert_pixel(&mut p, 0, expected);
                    // Switching back restores the mutated CGRAM immediately.
                    p.write(0x30, 0);
                    assert_pixel(
                        &mut p,
                        0,
                        if palette_color == 0x7c00 {
                            [0, 0, 255, 255]
                        } else {
                            [0, 255, 0, 255]
                        },
                    );
                }
            }
        }
    }
}

#[test]
fn raw_zero_is_transparent_for_every_tile_palette() {
    for mode in [3, 4, 7] {
        for palette in 0..8 {
            let mut p = scene(mode, 0, palette);
            cgram(&mut p, 0, 0x03e0);
            for direct in [0, 1] {
                p.write(0x30, direct);
                assert_pixel(&mut p, 0, [0, 255, 0, 255]);
            }
        }
    }
}

#[test]
fn direct_color_flows_through_main_sub_math_windows_and_brightness() {
    for mode in [3, 4, 7] {
        let mut p = scene(mode, 0x51, 0); // R=4, G=8, B=8.
        p.write(0x30, 1);
        assert_pixel(&mut p, 0, [32, 65, 65, 255]);
        p.write(0x32, 0x23); // Fixed red=3, green=2, blue=1.
        p.write(0x32, 0x42);
        p.write(0x32, 0x81);
        p.write(0x31, 1); // BG1 + fixed => (7,10,9).
        assert_pixel(&mut p, 0, [57, 82, 74, 255]);
        p.write(0, 7); // floor(component * 255 * 7 / (31 * 15)).
        assert_pixel(&mut p, 0, [26, 38, 34, 255]);
        p.write(0, 15);
        p.write(0x31, 0);
        p.write(0x23, 2); // Mask BG1 with window 1 at x=0 only.
        p.write(0x2e, 1);
        assert_pixel(&mut p, 0, [0, 255, 0, 255]);
        assert_pixel(&mut p, 1, [32, 65, 65, 255]);
        p.write(0x2e, 0);
        p.write(0x25, 0x20); // Math window 1 at x=0.
        p.write(0x30, 0x21); // Prevent math inside, preserve direct bit.
        p.write(0x31, 1);
        assert_pixel(&mut p, 0, [32, 65, 65, 255]);
        assert_pixel(&mut p, 1, [57, 82, 74, 255]);

        // BG1 now supplies the actual subscreen operand, rather than COLDATA.
        p.write(0x2c, 0);
        p.write(0x2d, 1);
        cgram(&mut p, 0, 0x0042); // Main backdrop (2,2,0).
        p.write(0x30, 3);
        p.write(0x31, 0x20); // (2,2,0) + (4,8,8) = (6,10,8).
        assert_pixel(&mut p, 0, [49, 82, 65, 255]);
        p.write(0x31, 0x60); // Opaque direct operand permits halving.
        assert_pixel(&mut p, 0, [24, 41, 32, 255]);
        p.write(0x2f, 1); // Mask subscreen at x=0: use fixed, no halving.
        assert_pixel(&mut p, 0, [41, 32, 8, 255]);
    }
}

#[test]
fn direct_color_preserves_other_color_sources_and_priority() {
    for mode in [3, 4] {
        let mut p = scene(mode, 1, 7);
        p.write(8, 0x64); // BG2 map at $c800, characters at $4000.
        p.write(0x0b, 0x21);
        planar(&mut p, 0x4000, if mode == 3 { 4 } else { 2 }, 2);
        cgram(&mut p, 2, 0x7c00);
        p.write(0x2c, 3);
        // High priority BG2 covers low priority BG1; flipping BG1 priority wins.
        p.vram[0xc801] = 0x20;
        for direct in [0, 1] {
            p.write(0x30, direct);
            assert_pixel(&mut p, 0, [0, 0, 255, 255]);
        }
        p.vram[0xc001] |= 0x20;
        p.write(0x30, 1);
        assert_pixel(&mut p, 0, [49, 16, 32, 255]); // (6,2,4).

        // OBJ at x=0,y=0 uses its own CGRAM entry and priority.
        // OBJ characters at $c000; tile 16 ($c200) avoids the BG1 map word.
        p.write(1, 3);
        p.oam[..4].copy_from_slice(&[0, 0, 16, 0x30]);
        planar(&mut p, 0xc200, 4, 1);
        cgram(&mut p, 129, 0x03ff);
        p.write(0x2c, 16);
        for direct in [0, 1] {
            p.write(0x30, direct);
            assert_pixel(&mut p, 0, [255, 255, 0, 255]);
            p.write(0x2c, 0);
            assert_pixel(&mut p, 0, [0, 255, 0, 255]);
            p.write(0x2c, 16);
        }
    }
    for mode in [0, 1, 2, 5, 6] {
        let mut p = scene(mode, 1, 0);
        for direct in [0, 1] {
            p.write(0x30, direct);
            // Mode 5/6 output even=TS, odd=TM (Anomie Modes 5/6, $2133).
            // Keep the CGRAM negative control on the visible main phase.
            let hires = matches!(mode, 5 | 6);
            assert_pixel(&mut p, usize::from(hires), [0, 0, 255, 255]);
            if hires {
                assert_pixel(&mut p, 0, [0, 0, 0, 255]); // TS disabled, COLDATA=0.
            }
        }
    }
    let mut p = scene(7, 255, 0);
    p.write(0x33, 0x40); // EXTBG is outside the implemented direct-color scope.
    p.write(0x30, 1);
    assert_pixel(&mut p, 0, [0, 0, 255, 255]);
}

// Tiny source-built LoROM: LDA #byte / STA $address, then BRA forever.
// Every register and video-memory write is executed by the emulated CPU.
fn guest_rom(mode: u8, direct: bool, palette_color: u16) -> Vec<u8> {
    let mut code = vec![0x78]; // SEI, emulation mode with 8-bit accumulator.
    let mut write = |address: u16, value: u8| {
        code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
    };
    write(0x2100, 0x80);
    write(0x2105, mode);
    write(0x212c, 1);
    write(0x2130, u8::from(direct));
    write(0x2131, 0);
    write(0x2121, 0);
    write(0x2122, 0xe0); // Green backdrop.
    write(0x2122, 3);
    write(0x2121, 255);
    write(0x2122, palette_color as u8);
    write(0x2122, (palette_color >> 8) as u8);
    write(0x2115, 0x80); // Increment VRAM word after high-byte write.
    if mode == 7 {
        for reg in [0x211b, 0x211e] {
            write(reg, 0);
            write(reg, 1);
        }
        write(0x2116, 0);
        write(0x2117, 0);
        write(0x2118, 1); // Tile 1 at map origin.
        write(0x2119, 0);
        write(0x2116, 64); // Tile 1 pixel data in high bytes of words 64..127.
        write(0x2117, 0);
        for _ in 0..64 {
            write(0x2119, 255);
        }
    } else {
        write(0x2107, 0x60);
        write(0x210b, 1);
        write(0x2116, 0);
        write(0x2117, 0x10); // Byte $2000, word $1000.
        for _ in 0..32 {
            write(0x2118, 255); // All eight planes set, pixel $ff.
            write(0x2119, 255);
        }
        write(0x2116, 0);
        write(0x2117, 0x60);
        for palette in 0..8 {
            write(0x2118, 0);
            write(0x2119, palette * 4);
        }
        write(0x2118, 1); // Tile 1 is raw zero: transparency after palette 7.
        write(0x2119, 28);
    }
    write(0x2100, 15);
    code.extend([0x80, 0xfe]);
    assert!(code.len() < 0x7fc0);
    let mut rom = vec![0; 0x8000];
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"DIRECT COLOR ORIGINAL");
    rom[0x7fd5] = 0x20; // Ordinary NTSC LoROM, no SRAM or coprocessor.
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[0xff, 0xff, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&0x8000u16.to_le_bytes());
    rom
}

fn assert_source_built_guest(mode: u8) {
    for direct in [false, true] {
        for palette_color in [0x7c00, 0x001f] {
            let mut machine = Machine::new(&guest_rom(mode, direct, palette_color)).unwrap();
            // Explicit instruction and frame bounds include initialization and
            // a complete subsequent frame; never patch Machine's PPU state.
            let mut instructions = 0;
            while machine.bus.frames < 2 && instructions < 100_000 {
                machine.step();
                instructions += 1;
                assert!(!machine.cpu.stopped);
            }
            assert_eq!(machine.bus.frames, 2, "guest instruction deadline");
            for palette in 0..if mode == 7 { 1 } else { 8 } {
                // $ff gives R=28,G=28,B=24; palette adds 2,2,4.
                let expected = if direct {
                    [
                        if palette & 1 != 0 { 246 } else { 230 },
                        if palette & 2 != 0 { 246 } else { 230 },
                        if palette & 4 != 0 { 230 } else { 197 },
                        255,
                    ]
                } else if palette_color == 0x7c00 {
                    [0, 0, 255, 255]
                } else {
                    [255, 0, 0, 255]
                };
                let offset = palette * 8 * 4;
                assert_eq!(
                    &machine.framebuffer()[offset..offset + 4],
                    &expected,
                    "mode={mode}, direct={direct}, palette={palette}"
                );
            }
            let transparent_x = if mode == 7 { 8 } else { 64 };
            assert_eq!(
                &machine.framebuffer()[transparent_x * 4..transparent_x * 4 + 4],
                &[0, 255, 0, 255]
            );
        }
    }
}

#[test]
fn source_built_guest_mode3() {
    assert_source_built_guest(3);
}

#[test]
fn source_built_guest_mode4() {
    assert_source_built_guest(4);
}

#[test]
fn source_built_guest_mode7() {
    assert_source_built_guest(7);
}
