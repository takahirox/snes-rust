//! Original MIT vectors for Issue #41. Invalid-fetch retention is emulator policy.
use super::*;

// Independent bit-permutation oracle: rotate each source bit within the low
// 8/9/10-bit field, then discard the unconnected physical address bit 15.
fn word(address: u16, remap: u8) -> u16 {
    let width = if remap == 0 { 0 } else { remap + 7 };
    let mut physical = 0u16;
    for bit in 0..16 {
        if address & (1 << bit) != 0 {
            let target = if bit < width { (bit + 3) % width } else { bit };
            physical |= 1 << target;
        }
    }
    let a = physical & 0x7fff;
    u16::from_le_bytes([(a as u8).wrapping_mul(13) ^ 0x5b, (a >> 7) as u8 ^ 0xa6])
}
fn populated() -> Ppu {
    let mut p = Ppu::default();
    for a in 0..32768u16 {
        p.vram[a as usize * 2..a as usize * 2 + 2].copy_from_slice(&word(a, 0).to_le_bytes());
    }
    p
}

#[test]
fn vram_read_operation_matrix_steps_remaps_wrap_and_recovery() {
    // force, line, horizontal phase, available. HBlank never grants access.
    for (force, v, h, available) in [
        (true, 100, 200, true),
        (true, 0, 1200, true),
        (false, 225, 200, true),
        (false, 261, 1200, true),
        (false, 100, 200, false),
        (false, 100, 1200, false),
        (false, 0, 200, false),
        (false, 0, 1200, false),
    ] {
        for remap in 0..4 {
            for (step_bits, step) in [(0, 1u16), (1, 32), (2, 128), (3, 128)] {
                for high in [false, true] {
                    let mut p = populated();
                    p.write(0x15, (u8::from(high) << 7) | (remap << 2) | step_bits);
                    p.write(0x16, 0x47);
                    p.write(0x17, 0x12); // Independently seed buffer under forced blank.
                    let mut buffer = word(0x1247, remap);
                    assert_eq!(p.vbuffer, buffer);
                    p.v = v;
                    p.h = h;
                    p.write(0, if force { 0x80 } else { 15 });
                    // Both byte ports must attempt prefetch after address replacement.
                    p.write(0x16, 0xff);
                    if available {
                        buffer = word(0x12ff, remap);
                    }
                    assert_eq!((p.vaddr, p.vbuffer), (0x12ff, buffer));
                    p.write(0x17, 0xff);
                    if available {
                        buffer = word(0xffff, remap);
                    }
                    assert_eq!((p.vaddr, p.vbuffer), (0xffff, buffer));
                    let selected = if high { 0x3a } else { 0x39 };
                    let other = if high { 0x39 } else { 0x3a };
                    let shift = if high { 8 } else { 0 };
                    let mut address = 0xffffu16;
                    for _ in 0..5 {
                        for _ in 0..2 {
                            assert_eq!(p.read(other, 0xe7), (buffer >> (8 - shift)) as u8);
                            assert_eq!((p.vaddr, p.vbuffer), (address, buffer));
                        }
                        let old_byte = (buffer >> shift) as u8;
                        assert_eq!(p.read(selected, 0xd2), old_byte);
                        if available {
                            buffer = word(address, remap);
                        }
                        address = address.wrapping_add(step);
                        assert_eq!((p.vaddr, p.vbuffer), (address, buffer));
                        // PPU1 read bus receives the returned byte, not CPU open bus.
                        assert_eq!(p.read(0x24, 0x19), old_byte);
                        // Refresh read bus deterministically for the next iteration.
                        let returned = p.read(other, 0);
                        assert_eq!(p.read(0x24, 0xff), returned);
                    }
                    p.write(0, 0x80);
                    // Return is still old buffer; only the next read sees recovery.
                    assert_eq!(p.read(selected, 0), (buffer >> shift) as u8);
                    buffer = word(address, remap);
                    address = address.wrapping_add(step);
                    assert_eq!((p.vaddr, p.vbuffer), (address, buffer));
                    assert_eq!(p.read(other, 0), (buffer >> (8 - shift)) as u8);
                }
            }
        }
    }
}

#[test]
fn vram_writes_do_not_refill_and_partial_address_writes_replace_live_bytes() {
    let mut p = populated();
    p.write(0x15, 0x80);
    p.write(0x16, 0xff);
    p.write(0x17, 0x12);
    let seeded = p.vbuffer;
    p.write(0x19, 0x3d); // Increment through low-byte carry, no refill.
    assert_eq!((p.vaddr, p.vbuffer), (0x1300, seeded));
    p.write(0x16, 0x27);
    assert_eq!((p.vaddr, p.vbuffer), (0x1327, word(0x1327, 0)));
    p.read(0x3a, 0); // $1328
    p.write(0x17, 0x24);
    assert_eq!((p.vaddr, p.vbuffer), (0x2428, word(0x2428, 0)));
}
