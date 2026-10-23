//! Original MIT vectors. Restricted addresses and retention test model policy.
use super::*;

#[test]
fn cgram_shared_phase_latch_readdress_wrap_and_independent_read_buses() {
    let mut p = Ppu::default();
    p.cgram[8..12].copy_from_slice(&[0x92, 0x34, 0x56, 0x78]);
    p.cgram[510..512].copy_from_slice(&[0xab, 0x6d]);
    p.open1 = 0x51;
    p.open2 = 0xe2;
    p.write(0x21, 4);
    p.write(0x22, 0x65); // Only latch, odd shared phase.
    assert_eq!(&p.cgram[8..10], &[0x92, 0x34]);
    assert_eq!((p.caddr, p.clatch), (9, 0x65));
    assert_eq!(p.read(0x3b, 0x03), 0xb4); // Old high + PPU2 bit 7.
    assert_eq!(
        (p.caddr, p.clatch, p.open1, p.open2),
        (10, 0x65, 0x51, 0xb4)
    );
    assert_eq!(p.read(0x3b, 0xff), 0x56); // Low read clears PPU2 bit 7.
    p.write(0x22, 0xfe); // Read-low/write-high uses retained latch.
    assert_eq!(&p.cgram[10..12], &[0x65, 0x7e]);
    assert_eq!((p.caddr, p.clatch), (12, 0x65));
    p.write(0x22, 0xd3);
    p.write(0x21, 0xff); // Readdress resets phase, retains pending latch (policy).
    assert_eq!((p.caddr, p.clatch), (510, 0xd3));
    assert_eq!(p.read(0x3b, 0), 0xab);
    p.write(0x22, 0xff);
    assert_eq!(&p.cgram[510..512], &[0xd3, 0x7f]);
    assert_eq!((p.caddr, p.clatch), (0, 0xd3));
    p.write(0x22, 0x21);
    p.write(0x22, 0x87);
    assert_eq!(&p.cgram[..2], &[0x21, 7]);
    p.write(0x21, 0xff);
    assert_eq!(p.read(0x3b, 0x04), 0xd3);
    assert_eq!(p.read(0x3b, 0x04), 0xff);
    assert_eq!(p.caddr, 0);
    // Reverse seeds distinguish CPU, PPU1 and PPU2 sources on high reads.
    p.open1 = 0xf1;
    p.open2 = 0x02;
    p.caddr = 511;
    assert_eq!(p.read(0x3b, 0x80), 0x7f);
    assert_eq!(p.read(0x24, 0), 0xf1);
}

#[test]
fn cgram_visible_ownership_routes_memory_but_preserves_software_phase() {
    for (force, v, h, software) in [
        (true, 100, 300, true),
        (false, 225, 300, true),
        (false, 261, 1200, true),
        (false, 100, 1200, true),
        (false, 0, 1200, true),
        (false, 100, 300, false),
        (false, 0, 300, false),
    ] {
        let mut p = Ppu::default();
        p.cgram[..2].copy_from_slice(&[0xa1, 0x32]);
        p.cgram[8..10].copy_from_slice(&[0x12, 0x34]);
        p.v = v;
        p.h = h;
        p.write(0, if force { 128 } else { 15 });
        p.write(0x21, 4);
        assert_eq!(p.read(0x3b, 0x77), if software { 0x12 } else { 0xa1 });
        assert_eq!(p.read(0x3b, 0), if software { 0x34 } else { 0xb2 });
        p.write(0x21, 4);
        p.write(0x22, 0x65);
        assert_eq!(&p.cgram[8..10], &[0x12, 0x34]);
        p.write(0x22, 0xfe);
        assert_eq!(
            &p.cgram[8..10],
            if software {
                &[0x65, 0x7e]
            } else {
                &[0x12, 0x34]
            }
        );
        assert_eq!(
            &p.cgram[..2],
            if software {
                &[0xa1, 0x32]
            } else {
                &[0x65, 0x7e]
            }
        );
        assert_eq!((p.caddr, p.clatch), (10, 0x65));
        p.write(0, 128);
        p.write(0x21, 4);
        assert_eq!(p.read(0x3b, 0), if software { 0x65 } else { 0x12 });
    }
}
