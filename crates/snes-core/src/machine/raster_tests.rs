//! Independent literal output/clock oracles for Issue #107 (original MIT tests).
use super::*;

fn bus() -> SystemBus {
    let mut b = SystemBus::new(
        Cartridge::load(include_bytes!("../../../../tests/roms/smoke.bin")).unwrap(),
    );
    b.write_raw(0x2105, 1);
    b.write_raw(0x2131, 0x20); // Backdrop + fixed color, no palette dependency.
    b.write_raw(0x2132, 0x3f);
    b.write_raw(0x2100, 15);
    b.tick(1364); // Enter visible line 1 naturally.
    b
}
fn pixel(b: &SystemBus, x: usize, y: usize) -> [u8; 4] {
    let i = (y * b.ppu.framebuffer_width() + x) * 4;
    b.ppu.frame[i..i + 4].try_into().unwrap()
}
const RED: [u8; 4] = [255, 0, 0, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

#[test]
fn raster_literal_before_at_after_edges_and_consecutive_writes() {
    for (h, split) in [
        (87, 0),
        (88, 0),
        (89, 0),
        (91, 0),
        (92, 1),
        (93, 1),
        (1111, 255),
        (1112, 256),
        (1113, 256),
        (343, 63),
        (344, 64),
        (345, 64),
        (855, 191),
        (856, 192),
        (857, 192),
    ] {
        let mut b = bus();
        b.tick(h);
        // Two commits at the same clock have no intervening pixel.
        b.write_raw(0x2132, 0x20);
        b.write_raw(0x2132, 0x5f);
        b.tick(1113 - h);
        for x in 0..256 {
            assert_eq!(
                pixel(&b, x, 0),
                if x < split { RED } else { [0, 255, 0, 255] },
                "H={h}, x={x}"
            );
        }
    }
}

#[test]
fn raster_multiple_changes_blank_brightness_and_hblank_tail() {
    let mut b = bus();
    b.tick(344); // x=0..63 emitted, next is x=64.
    b.write_raw(0x2100, 7);
    b.tick(128); // H=472, x=96.
    b.write_raw(0x2100, 0x80);
    b.tick(128); // H=600, x=128.
    b.write_raw(0x2100, 15);
    b.write_raw(0x2132, 0x20);
    b.write_raw(0x2132, 0x9f);
    b.tick(496); // H=1096: HBlank flag, but four picture pixels remain!
    assert_eq!(b.read_raw(0x4212) & 64, 64);
    b.write_raw(0x2100, 0);
    b.tick(16);
    for x in 0..256 {
        let expected = match x {
            0..64 => RED,
            64..96 => [119, 0, 0, 255],
            96..128 | 252..256 => BLACK,
            _ => [0, 0, 255, 255],
        };
        assert_eq!(pixel(&b, x, 0), expected, "x={x}");
    }
    // At/after picture end cannot recolor the row; next row uses new state.
    let row = b.ppu.frame[..1024].to_vec();
    b.write_raw(0x2100, 15);
    b.tick(1364);
    assert_eq!(&b.ppu.frame[..1024], row);
    assert_eq!(pixel(&b, 0, 1), [0, 0, 255, 255]);
    // The old whole-line implementation produces a uniform row and fails this.
    assert_ne!(pixel(&b, 0, 0), pixel(&b, 128, 0));
}

fn advance(b: &mut SystemBus, n: u64, partition: &[u64]) {
    let mut left = n;
    let mut i = 0;
    while left > 0 {
        let step = left.min(partition[i % partition.len()]);
        b.tick(step);
        left -= step;
        i += 1;
    }
}
#[test]
fn raster_whole_irregular_single_clock_frames_audio_and_transitions() {
    let mut outputs = vec![];
    for partition in [&[u64::MAX][..], &[1][..], &[3, 127, 2, 1367, 19][..]] {
        let mut b = bus();
        advance(&mut b, 600, partition);
        b.write_raw(0x2100, 0x80);
        // Cross several visible lines in one tick, with forced blank retained.
        advance(&mut b, 3 * 1364, partition);
        b.write_raw(0x2100, 15);
        advance(&mut b, 221 * 1364 - 600, partition); // V=225,H=0.
        assert_eq!((b.v, b.h), (225, 0));
        let frame = b.ppu.frame.clone();
        b.write_raw(0x2132, 0xff);
        advance(&mut b, 37 * 1364 + 1112, partition);
        assert_eq!((b.frames, b.v, b.h), (1, 0, 1112));
        assert_eq!(b.ppu.frame, frame); // VBlank and hidden line never draw.
        advance(&mut b, 1364, partition);
        assert_eq!(pixel(&b, 0, 0), [255, 255, 255, 255]);
        // Include the next odd-field short line and another complete field.
        advance(&mut b, 2 * 262 * 1364 - 4, partition);
        outputs.push((b.ppu.frame, b.apu.samples, b.clocks, b.h, b.v, b.frames));
    }
    assert_eq!(outputs[0], outputs[1]);
    assert_eq!(outputs[0], outputs[2]);
}

#[test]
fn raster_cpu_dma_hdma_commits_preserve_previous_output() {
    let mut b = bus();
    b.tick(338);
    b.write(0x2100, 7); // Six-clock B-bus CPU cycle commits at H=344.
    assert_eq!(b.h, 344);
    b.tick(768);
    assert_eq!(pixel(&b, 63, 0), RED);
    assert_eq!(pixel(&b, 64, 0), [119, 0, 0, 255]);
    let mut b = bus();
    b.dma[0][0] = 0;
    b.dma[0][1] = 0;
    b.wram[0x100] = 0x80;
    b.tick(336);
    b.transfer(0, 0x7e0100, 0); // 4+4 clocks -> H=344; x64 onward blank.
    assert_eq!(b.h, 344);
    b.tick(768);
    assert_eq!(pixel(&b, 63, 0), RED);
    assert_eq!(pixel(&b, 64, 0), BLACK);

    // Natural HDMA service at H>=1112 must leave all 256 pixels intact.
    let mut b = bus();
    for (r, value) in [(0, 0), (1, 0), (2, 0), (3, 1), (4, 0x7e)] {
        b.write_raw(0x4300 + r, value);
    }
    b.wram[0x100..0x105].copy_from_slice(&[1, 15, 1, 0x80, 0]);
    b.write_raw(0x420c, 1);
    // Initialization was pending from hidden line 0; service via events.
    b.events();
    b.tick(1112 - b.h);
    b.events();
    assert_eq!(b.ppu.regs[0], 0x80);
    for x in 0..256 {
        assert_eq!(pixel(&b, x, 0), RED);
    }
    b.tick(1364 - b.h + 1112);
    for x in 0..256 {
        assert_eq!(pixel(&b, x, 1), BLACK);
    }
}

#[test]
fn raster_midline_width_promotion_preserves_prefix_and_next_frame_shrinks() {
    let mut b = bus();
    b.tick(344);
    b.write_raw(0x2133, 8);
    b.write_raw(0x2100, 0x80);
    b.tick(128); // H=472, x=96. Return to low resolution in the same row zero.
    b.write_raw(0x2133, 0);
    b.write_raw(0x2100, 7);
    b.tick(640);
    assert_eq!(b.ppu.framebuffer_width(), 512);
    for x in 0..512 {
        let expected = match x {
            0..128 => RED,
            128..192 => BLACK,
            _ => [119, 0, 0, 255],
        };
        assert_eq!(pixel(&b, x, 0), expected, "promoted x={x}");
    }
    b.write_raw(0x2100, 0x80);
    b.tick(261 * 1364 + 92 - 1112);
    assert_eq!((b.v, b.h), (0, 92));
    b.tick(1364);
    assert_eq!(b.ppu.framebuffer_width(), 256);
    assert_eq!(pixel(&b, 0, 0), BLACK);
}

#[path = "../../tests/common/raster.rs"]
mod guest;
#[test]
fn raster_guest_cpu_and_dma_scheduler_partition_invariance() {
    let mut results = vec![];
    for quantum in [u64::MAX, 1, 3, 17] {
        let mut m = Machine::new(&guest::guest_rom()).unwrap();
        m.bus.raster_quantum = quantum;
        for _ in 0..3 {
            m.run_frame().unwrap();
        }
        results.push((m.bus.ppu.frame, m.bus.apu.samples, m.bus.clocks, m.cpu.pc));
    }
    for result in &results[1..] {
        assert_eq!(&results[0], result);
    }
}

#[test]
fn raster_mode1_planar_bg_palette_math_and_blank_share_compositor() {
    let mut b = bus();
    // BG1 tilemap at byte $4000; tile 0 at $0000, red/green alternating.
    b.write_raw(0x2107, 0x20);
    b.write_raw(0x212c, 1);
    b.write_raw(0x2131, 1); // Color math on BG1, not backdrop.
    b.write_raw(0x2132, 0xe0);
    b.ppu.cgram[2..6].copy_from_slice(&[31, 0, 0xe0, 3]);
    for y in 0..8 {
        b.ppu.vram[y * 2] = 0xaa;
        b.ppu.vram[y * 2 + 1] = 0x55;
    }
    b.tick(344);
    b.write_raw(0x2132, 0x9f); // Add blue to every subsequent BG1 pixel.
    b.tick(256); // H=600, x=128.
    b.write_raw(0x2100, 0x80);
    b.tick(256); // H=856, x=192.
    b.write_raw(0x2100, 7);
    b.tick(256);
    for x in 0..256 {
        let expected = match (x, x % 2) {
            (0..64, 0) => RED,
            (0..64, _) => [0, 255, 0, 255],
            (64..128, 0) => [255, 0, 255, 255],
            (64..128, _) => [0, 255, 255, 255],
            (128..192, _) => BLACK,
            (_, 0) => [119, 0, 119, 255],
            _ => [0, 119, 119, 255],
        };
        assert_eq!(pixel(&b, x, 0), expected, "Mode 1 x={x}");
    }
}

#[test]
fn raster_hdma_natural_cpu_service_partition_invariance() {
    let mut results = vec![];
    for quantum in [u64::MAX, 1, 3, 17] {
        let mut b = bus();
        b.raster_quantum = quantum;
        for (r, value) in [(0, 0), (1, 0), (2, 0), (3, 1), (4, 0x7e)] {
            b.write_raw(0x4300 + r, value);
        }
        b.wram[0x100..0x107].copy_from_slice(&[1, 15, 1, 0x80, 1, 7, 0]);
        b.write_raw(0x420c, 1);
        let deadline = b.clocks + 5 * 1364;
        while b.v < 4 && b.clocks < deadline {
            b.idle();
        }
        assert_eq!(b.v, 4);
        for x in 0..256 {
            assert_eq!(pixel(&b, x, 0), RED);
            assert_eq!(pixel(&b, x, 1), BLACK);
            assert_eq!(pixel(&b, x, 2), [119, 0, 0, 255]);
        }
        results.push((b.ppu.frame, b.apu.samples, b.clocks, b.dma, b.hdma_active));
    }
    for result in &results[1..] {
        assert_eq!(&results[0], result);
    }
}
