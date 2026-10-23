//! Original MIT OAM vectors. Restricted numeric addresses/bytes are policy.
use super::*;

fn address(p: &mut Ppu, word: u16) {
    p.write(2, word as u8);
    p.write(3, (word >> 8) as u8);
}

#[test]
fn blanked_pairs_mirrors_shared_latch_wrap_and_interleaved_accesses() {
    for (force, line) in [(true, 0), (true, 100), (false, 225), (false, 261)] {
        let mut p = Ppu {
            v: line,
            ..Ppu::default()
        };
        p.write(0, if force { 128 } else { 15 });
        address(&mut p, 0);
        p.write(4, 0x81);
        assert_eq!(&p.oam[..4], &[0, 0, 0, 0]);
        assert_eq!(p.oam_port_state(), (1, 1, 0x81));
        assert_eq!(p.read(0x38, 0xff), 0); // Odd read never commits/captures.
        assert_eq!(p.oam_port_state(), (2, 2, 0x81));
        p.read(0x38, 0xff); // Even read also preserves latch.
        p.write(4, 0x23);
        assert_eq!(&p.oam[..4], &[0, 0, 0x81, 0x23]);
        assert_eq!(p.oam_port_state(), (4, 4, 0x81));
        // Independent high-table mirror expectations, both even and odd.
        for (word, physical) in [(0x100, 512), (0x110, 512), (0x1ff, 542)] {
            address(&mut p, word);
            p.write(4, 0x56);
            assert_eq!(p.oam[physical], 0x56);
            assert_eq!(p.olatch, 0x56);
            p.write(4, 0x78);
            assert_eq!(p.oam[physical + 1], 0x78);
            assert_eq!(p.olatch, 0x56); // Odd high byte does not capture.
        }
        assert_eq!(p.oam_port_state(), (0, 0, 0x56)); // Ten-bit wrap.
        p.read(0x38, 0);
        p.write(4, 0x9a); // High-even latch survives wrap and intervening read.
        assert_eq!(&p.oam[..2], &[0x56, 0x9a]);
        address(&mut p, 0x1ff);
        assert_eq!(p.read(0x38, 0), 0x56);
        assert_eq!(p.read(0x38, 0), 0x78);
        assert_eq!(p.oam_port_state(), (0, 0, 0x56));
    }
}

#[test]
fn either_address_register_reloads_whole_programmed_word_and_even_phase() {
    for (force, line, h) in [
        (true, 100, 200),
        (false, 225, 200),
        (false, 100, 200),
        (false, 100, 1200),
        (false, 0, 200),
        (false, 0, 1200),
    ] {
        let mut p = Ppu {
            v: line,
            h,
            ..Ppu::default()
        };
        p.write(0, if force { 128 } else { 15 });
        address(&mut p, 0x104);
        for _ in 0..5 {
            p.read(0x38, 0);
        }
        p.write(3, 1);
        assert_eq!(p.oam_port_state(), (0x208, 0x208, 0));
        p.read(0x38, 0);
        p.write(2, 0xff);
        assert_eq!(p.oam_port_state(), (0x3fe, 0x3fe, 0));
        p.read(0x38, 0);
        p.write(3, 0);
        assert_eq!(p.oam_port_state(), (0x1fe, 0x1fe, 0));
    }
}

#[test]
fn rendering_ownership_policy_preserves_low_sentinels_and_recovers_with_address_write() {
    for (line, h) in [(100, 200), (100, 1200), (0, 200), (0, 1200)] {
        let mut p = Ppu::default();
        address(&mut p, 4);
        p.write(4, 0x12);
        p.write(4, 0x34);
        address(&mut p, 4);
        p.v = line;
        p.h = h;
        p.write(0, 15);
        // Policy isolates this low pair from rendering-owned writes.
        // Actual hardware destinations could include other OAM bytes.
        p.write(4, 0x56);
        p.write(4, 0x78);
        assert_eq!(&p.oam[8..10], &[0x12, 0x34]);
        // Numeric rendering address, latch/phase and returned bits are policy.
        assert_eq!(&p.oam[512..514], &[0x56, 0x78]);
        assert_eq!(p.oam_port_state(), (514, 10, 0x56));
        assert_eq!(p.read(0x38, 0xa1), 0x56);
        assert_eq!(p.read(0x24, 0xee), 0x56); // PPU1 bus, not CPU open bus.
        assert_eq!(p.oam_port_state(), (513, 11, 0x56));
        p.write(0, 128); // No implicit address reload on entering forced blank.
        assert_eq!(p.read(0x38, 0), 0x78);
        assert_eq!(p.oam_port_state(), (514, 12, 0x56));
        p.write(2, 4); // Whole programmed address restored; latch retained.
        assert_eq!(p.oam_port_state(), (8, 8, 0x56));
        assert_eq!(p.read(0x38, 0), 0x12);
        p.write(4, 0x9a);
        assert_eq!(&p.oam[8..10], &[0x56, 0x9a]);
    }
}

#[test]
fn blanking_reload_events_preserve_latch_and_reset_phase_as_documented_policy() {
    for force in [false, true] {
        let mut p = Ppu::default();
        address(&mut p, 4);
        p.write(4, 0x65);
        p.v = 224;
        p.write(0, if force { 128 } else { 15 });
        let old = p.oam_port_state();
        p.oam_scanline_start(225);
        assert_eq!(p.oam_port_state(), if force { old } else { (8, 8, 0x65) });
    }
    for line in [100, 225, 226, 261] {
        for value in [15, 128] {
            let mut p = Ppu::default();
            address(&mut p, 4);
            p.write(4, 0x87);
            p.v = line;
            p.write(0, value);
            // First-VBlank old-F rule uses Anomie's specific INIDISP entry.
            let expected = if line == 225 {
                (8, 8, 0x87)
            } else if line == 100 && value == 15 {
                (513, 9, 0x87)
            } else {
                (9, 9, 0x87)
            };
            assert_eq!(p.oam_port_state(), expected);
        }
    }
}

#[test]
fn sprite_rotation_uses_advancing_cursor_not_rendering_substitute() {
    let mut p = Ppu::default();
    // Two overlapping original solid sprites, distinct palette colors.
    for i in 0..128 {
        p.oam[i * 4 + 1] = 128;
    }
    p.oam[..8].copy_from_slice(&[0, 0, 0, 0, 0, 0, 0, 2]);
    p.vram[..16].fill(0xff);
    p.cgram[262..264].copy_from_slice(&0x001fu16.to_le_bytes()); // palette 0, pixel 3
    p.cgram[294..296].copy_from_slice(&0x03e0u16.to_le_bytes()); // palette 1, pixel 3
    p.write(0x2c, 16);
    p.write(2, 2);
    p.write(3, 128); // Sprite 1 first.
    p.v = 10;
    p.write(0, 15); // Rendering substitute must not rotate to sprite 0.
    p.render_line(0);
    assert_eq!(&p.frame[..4], &[0, 255, 0, 255]);
    p.oam_scanline_start(225);
    p.v = 225;
    p.read(0x38, 0);
    p.read(0x38, 0);
    p.read(0x38, 0);
    p.read(0x38, 0); // Advance to sprite 2, wraps through sprites 0 then 1.
    p.render_line(0);
    assert_eq!(&p.frame[..4], &[255, 0, 0, 255]);
    p.oam_scanline_start(225); // Reload puts sprite 1 first again.
    p.render_line(0);
    assert_eq!(&p.frame[..4], &[0, 255, 0, 255]);
    p.write(3, 0); // Rotation disabled.
    p.render_line(0);
    assert_eq!(&p.frame[..4], &[255, 0, 0, 255]);
}

#[test]
fn high_even_write_captures_latch_for_later_low_odd_commit() {
    let mut p = Ppu::default();
    p.write(4, 0x11); // Seed a distinguishable low-table latch.
    p.write(3, 1); // High table, even byte.
    p.write(4, 0x65); // Fullsnes: both direct commit AND latch capture.
    p.write(3, 0); // Reload even low address, retaining the latch.
    p.read(0x38, 0); // Reach odd phase without changing the latch.
    p.write(4, 0x87);
    assert_eq!(&p.oam[..2], &[0x65, 0x87]);
    assert_eq!(p.oam[512], 0x65);
    assert_eq!(p.olatch, 0x65);
}
