//! Original MIT NTSC geometry regressions. Oracles do not use runtime geometry.
//! Copyright (c) 2026 snes-rust contributors.
use super::*;

fn bus() -> SystemBus {
    SystemBus::new(Cartridge::load(include_bytes!("../../../../tests/roms/smoke.bin")).unwrap())
}
fn parity(b: &mut SystemBus) -> u8 {
    b.read_raw(0x213f) >> 7
}
fn held(b: &mut SystemBus) -> [u16; 2] {
    let mut out = [0; 2];
    for (i, port) in [0x213c, 0x213d].into_iter().enumerate() {
        let lo = b.read_raw(port);
        let hi = b.read_raw(port) & 1;
        out[i] = u16::from_le_bytes([lo, hi]);
    }
    out
}
// Literal half-open boundaries from the source reconciliation, including every
// dot after 322. This table intentionally does not invert the runtime formula.
fn late_counter(h: u64, short: bool) -> u16 {
    let starts = if short {
        &[
            1288, 1292, 1296, 1300, 1304, 1308, 1312, 1316, 1320, 1324, 1328, 1332, 1336, 1340,
            1344, 1348, 1352, 1356, 1360,
        ][..]
    } else {
        &[
            1288, 1292, 1298, 1302, 1306, 1310, 1316, 1320, 1324, 1328, 1332, 1336, 1340, 1344,
            1348, 1352, 1356, 1360, 1364,
        ][..]
    };
    322 + starts
        .windows(2)
        .position(|w| w[0] <= h && h < w[1])
        .unwrap() as u16
}

#[test]
fn four_fields_all_line_totals_every_late_clock_and_stat78() {
    for display in [0x80, 0x0f] {
        let mut b = bus();
        b.ppu.write(0, display);
        let mut sum = 0;
        for (field, (length, cumulative)) in [
            (357368, 357368),
            (357364, 714732),
            (357368, 1072100),
            (357364, 1429464),
        ]
        .into_iter()
        .enumerate()
        {
            let start = b.clocks;
            // At H=0 the reported bit still belongs to the preceding field.
            assert_eq!(parity(&mut b), u8::from(field != 0 && field % 2 == 0));
            for v in 0..262 {
                let short = field % 2 == 1 && v == 240;
                let line = if short { 1360 } else { 1364 };
                let begin = b.clocks;
                assert_eq!((b.v, b.h), (v, 0));
                if (239..=241).contains(&v) {
                    b.tick(1288);
                    for h in 1288..line {
                        b.read_raw(0x2137);
                        assert_eq!(
                            held(&mut b),
                            [late_counter(h, short), v],
                            "field={field}, V={v}, clock={h}, display={display}"
                        );
                        b.tick(1);
                    }
                } else {
                    b.tick(line - 1);
                    assert_eq!((b.v, b.h), (v, line - 1));
                    b.tick(1);
                }
                assert_eq!(b.clocks - begin, line);
                assert_eq!(b.h, 0);
                sum += line;
            }
            assert_eq!(b.clocks - start, length);
            assert_eq!(b.clocks, cumulative);
            assert_eq!(sum, cumulative);
            assert_eq!((b.frames, b.v, b.h), (field as u64 + 1, 0, 0));
            assert_eq!(parity(&mut b), (field % 2) as u8);
        }
    }
}

#[test]
fn timed_slhv_and_wrio_capture_both_geometries_and_rollover() {
    for field_start in [0, 357368] {
        for software in [false, true] {
            // Include every clock around both long dots and the last dot/rollover.
            for h in (1288..1321).chain(1354..1367) {
                let mut b = bus();
                let short = field_start != 0;
                let end = if short { 1360 } else { 1364 };
                b.tick(field_start + 327360 + h - 6); // V=240; timed access ends at h.
                b.open = 0xa5;
                if software {
                    assert_eq!(b.read(0x2137), 0xa5); // SLHV leaves CPU open bus.
                } else {
                    b.write(0x4201, 0x7f);
                }
                let expected = if h >= end {
                    [((h - end) / 4) as u16, 241]
                } else {
                    [late_counter(h, short), 240]
                };
                assert_eq!(
                    held(&mut b),
                    expected,
                    "field={short}, software={software}, h={h}"
                );
                assert_eq!((b.v, b.h), (expected[1], h % end));
                // Independent selectors: consume just H low, then capture again.
                b.read(0x213c);
                if !software {
                    b.write(0x4201, 0xff);
                }
                b.read(0x2137);
                let high_h = b.read(0x213c);
                assert_eq!(
                    high_h & 1,
                    u8::from(h + if software { 12 } else { 18 } < end)
                );
                let low_v = b.read(0x213d);
                assert!(low_v == 240 || low_v == 241);
                // Gate low captures once, blocks SLHV and repeated-low writes.
                b.write(0x4201, 0x7f);
                b.read(0x213f); // resets both selectors even while low
                let saved = held(&mut b);
                b.tick(50);
                b.open = 0x5a;
                assert_eq!(b.read(0x2137), 0x5a);
                b.write(0x4201, 0);
                assert_eq!(held(&mut b), saved);
                assert_eq!(b.read(0x213f) & 0xdf, 0x43 | if short { 0x80 } else { 0 });
                b.write(0x4201, 0xff);
                assert_eq!(b.read(0x213f) & 0x40, 0x40);
                assert_eq!(b.read(0x213f) & 0x40, 0);
                // PPU2 bit 5 is held open bus, independently of CPU open bus.
                b.ppu.cgram[0] = 0x20;
                b.ppu.write(0x21, 0);
                b.read(0x213b);
                b.open = 0;
                assert_eq!(b.read(0x213f), 0x23 | if short { 0x80 } else { 0 });
            }
        }
    }
}

#[test]
fn chunking_preserves_raster_events_joypad_and_continuous_audio_phase() {
    let mut whole = bus();
    let mut single = bus();
    let mut irregular = bus();
    for b in [&mut whole, &mut single, &mut irregular] {
        b.write_raw(0x4200, 0xb1);
        b.buttons = 0xa590;
        b.buttons2 = 0x5a60;
    }
    whole.tick(1430001);
    for _ in 0..1430001 {
        single.tick(1);
    }
    let mut i = 0;
    while irregular.clocks < 1430001 {
        let n = [1, 537, 7, 1360, 19, 357365, 256][i % 7];
        irregular.tick(n.min(1430001 - irregular.clocks));
        i += 1;
    }
    assert_eq!((whole.frames, whole.v, whole.h), (4, 0, 537));
    for b in [&mut single, &mut irregular] {
        assert_eq!((b.clocks, b.frames, b.v, b.h), (1430001, 4, 0, 537));
        assert_eq!(
            (
                b.irq,
                b.nmi_pending,
                b.nmi_flag,
                b.hdma_init,
                b.hdma_pending
            ),
            (
                whole.irq,
                whole.nmi_pending,
                whole.nmi_flag,
                whole.hdma_init,
                whole.hdma_pending
            )
        );
        assert_eq!(b.joy, whole.joy);
        assert_eq!(b.serial, whole.serial);
        assert_eq!(b.auto_start, whole.auto_start);
        assert_eq!(b.apu.samples, whole.apu.samples);
        assert_eq!(parity(b), parity(&mut whole));
    }
    // Continue past another audio sample boundary to detect discarded fraction.
    for b in [&mut whole, &mut single, &mut irregular] {
        b.tick(1000);
    }
    assert_eq!(single.apu.samples, whole.apu.samples);
    assert_eq!(irregular.apu.samples, whole.apu.samples);
    assert_eq!(
        whole.apu.samples.len(),
        (1431001u64 * 1024000 / 21477272 / 32 * 2) as usize
    );
}

#[test]
fn refresh_and_dma_keep_every_access_clock_across_short_line() {
    for access in [6, 8, 12] {
        let mut b = bus();
        b.tick(686086); // field 1, V=240, H=1358
        let begin = b.clocks;
        b.read(match access {
            6 => 0x2137,
            8 => 0x7e0000,
            _ => 0x4016,
        });
        assert_eq!(b.clocks - begin, access);
        assert_eq!((b.frames, b.v, b.h), (1, 241, access - 2));
    }
    let mut b = bus();
    b.tick(686086);
    b.dma[0] = [0; 16];
    b.dma[0][1] = 0x18;
    b.wram[0] = 0x5a;
    b.waddr = 0x10000;
    b.transfer(0, 0x7e0000, 0);
    assert_eq!((b.clocks, b.v, b.h), (686094, 241, 6));
    assert_eq!(b.ppu.vram[0], 0x5a);
    b.tick(530); // H=536; the next byte straddles refresh.
    b.transfer(0, 0x7e0000, 0);
    assert_eq!((b.clocks, b.v, b.h), (686672, 241, 584));
    b.transfer(0, 0x7e0000, 0);
    assert_eq!(b.h, 592); // no second hold

    // Two complete fields: 524 holds, including exactly one on the short line.
    for chunks in [false, true] {
        let mut b = bus();
        let mut remaining = 693772; // 714732 - 524*40 access clocks
        while remaining > 0 {
            let n = if chunks { remaining.min(17) } else { remaining };
            b.bus_tick(n);
            remaining -= n;
        }
        assert_eq!((b.clocks, b.frames, b.v, b.h), (714732, 2, 0, 0));
    }
}

#[test]
fn irq_nmi_are_once_per_line_or_field_at_their_event_offsets() {
    let mut b = bus();
    b.write_raw(0x4200, 0x90); // H-only zero IRQ at H=14 and retained VBlank NMI
    let mut nmis = 0;
    for field in 0..4 {
        for v in 0..262 {
            let length = if field % 2 == 1 && v == 240 {
                1360
            } else {
                1364
            };
            b.tick(13);
            assert_eq!(b.read_raw(0x4211) & 128, 0);
            b.tick(1);
            // Initial V=0 is an unverified reset-created no-event policy.
            let expected = if field == 0 && v == 0 { 0 } else { 128 };
            assert_eq!(b.read_raw(0x4211) & 128, expected);
            assert_eq!(b.read_raw(0x4211) & 128, 0);
            b.tick(length - 15);
            assert_eq!(b.read_raw(0x4211) & 128, 0);
            assert!(!b.nmi_pending);
            b.tick(1);
            assert_eq!(b.read_raw(0x4211) & 128, 0); // no H=0 assertion
            assert_eq!(b.read_raw(0x4211) & 128, 0);
            if v == 224 {
                assert!(std::mem::take(&mut b.nmi_pending));
                assert_eq!(b.read_raw(0x4210) & 128, 128);
                assert_eq!(b.read_raw(0x4210) & 128, 0);
                nmis += 1;
            } else {
                assert!(!b.nmi_pending);
            }
        }
    }
    assert_eq!(nmis, 4);
}

#[test]
fn timed_stat78_reads_across_field_reporting_edge_in_both_parities() {
    for (end, next) in [(357368, 1), (714732, 0), (1072100, 1), (1429464, 0)] {
        // Each six-clock B-bus read ends at one literal clock around V=0/H=1.
        // Starts on the previous line exercise access splitting at rollover.
        for offset in -2i64..=6 {
            let mut b = bus();
            let sample = (end as i64 + offset) as u64;
            b.tick(sample - 6);
            let status = b.read(0x213f);
            assert_eq!(b.clocks, sample);
            assert_eq!(
                status,
                3 | if offset < 4 { 1 - next } else { next } << 7,
                "field end={end}, sample offset={offset}"
            );
            assert_eq!(
                (b.v, b.h),
                if offset < 0 {
                    (261, (1364 + offset) as u64)
                } else {
                    (0, offset as u64)
                }
            );
        }
    }
}

#[test]
fn literal_field_boundaries_and_reset_parity() {
    let mut b = bus();
    for _ in 0..4 {
        assert_eq!(parity(&mut b), 0); // no synthetic toggle on initial V=0
        b.tick(1);
    }
    assert_eq!(parity(&mut b), 0);
    for (end, field) in [(357368, 1), (714732, 0), (1072100, 1), (1429464, 0)] {
        b.tick(end - b.clocks - 1);
        assert_eq!((b.v, b.h), (261, 1363));
        assert_eq!(parity(&mut b), 1 - field);
        b.tick(1);
        assert_eq!((b.v, b.h), (0, 0));
        for h in 0..4 {
            assert_eq!((b.v, b.h), (0, h));
            assert_eq!(parity(&mut b), 1 - field);
            b.tick(1);
        }
        assert_eq!(parity(&mut b), field);
    }
    let mut m = Machine::new(include_bytes!("../../../../tests/roms/smoke.bin")).unwrap();
    m.bus.tick(357372 - m.bus.clocks);
    assert_eq!(parity(&mut m.bus), 1);
    m.reset();
    assert_eq!(parity(&mut m.bus), 0);
    assert_eq!((m.bus.frames, m.bus.v, m.bus.h), (0, 0, 16));
}

#[test]
fn complete_dma_crosses_short_line_then_exactly_one_refresh() {
    let mut b = bus();
    b.tick(686066); // field 1, V=240, H=1338
    b.dma[0] = [0; 16];
    b.dma[0][1] = 0x18; // VRAM low, forced blank, byte address increments
    b.dma[0][4] = 0x7e;
    b.dma[0][5] = 100;
    for (i, v) in b.wram[..100].iter_mut().enumerate() {
        *v = i as u8;
    }
    b.dma_pending = 1;
    b.events();
    // 16 setup + 100*8 payload + one 40-clock hold; first payload crosses V=241.
    assert_eq!((b.clocks, b.frames, b.v, b.h), (686922, 1, 241, 834));
    assert_eq!(&b.dma[0][2..7], &[100, 0, 0x7e, 0, 0]);
    for i in 0..100 {
        assert_eq!(b.ppu.vram[i * 2], i as u8);
    }
}
