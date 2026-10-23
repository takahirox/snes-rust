//! Original MIT live-NTRL scheduler regressions for Issue #37.
//! Copyright (c) 2026 snes-rust contributors.
use super::*;

fn until(b: &mut SystemBus, frame: u64, line: u16, h: u64) {
    let deadline = b.clocks + LINE * 263;
    while (b.frames, b.v, b.h) < (frame, line, h) {
        b.idle();
        assert!(b.clocks < deadline, "scheduler deadline");
    }
    b.events();
}
fn pair(b: &mut SystemBus, channel: u32, offset: u32) -> u16 {
    let a = 0x4300 + channel * 16 + offset;
    u16::from_le_bytes([b.read(a), b.read(a + 1)])
}
fn channel(b: &mut SystemBus, channel: u32, indirect: bool, table: u16, port: u8) {
    for (r, value) in [
        (0, if indirect { 0x40 } else { 0 }),
        (1, port),
        (2, table as u8),
        (3, (table >> 8) as u8),
        (4, 0x7e),
        (5, 0xa6),
        (6, 0x5a),
        (7, 0x7f),
    ] {
        b.write(0x4300 + channel * 16 + r, value);
    }
}
fn configured(indirect: bool, transferring: bool) -> SystemBus {
    configured_with_sibling(indirect, transferring, false)
}
fn configured_with_sibling(indirect: bool, transferring: bool, sibling: bool) -> SystemBus {
    let mut b = SystemBus::new(
        Cartridge::load(include_bytes!("../../../../tests/roms/smoke.bin")).unwrap(),
    );
    until(&mut b, 0, 225, 200);
    channel(&mut b, 0, indirect, 0x100, 0x40);
    let header = if transferring { 0x82 } else { 2 };
    let table: &[u8] = if indirect {
        &[header, 0, 2, 0x83, 0, 3, 0, 0x12]
    } else if transferring {
        &[header, 0x11, 0x22, 0x83, 0x33, 0x44, 0x55, 0]
    } else {
        &[header, 0x11, 0x83, 0x33, 0x44, 0x55, 0]
    };
    for (i, &value) in table.iter().enumerate() {
        b.write(0x7e0100 + i as u32, value);
    }
    for (i, value) in [0x11, 0x22].into_iter().enumerate() {
        b.write(0x7f0200 + i as u32, value);
    }
    for (i, value) in [0x33, 0x44, 0x55].into_iter().enumerate() {
        b.write(0x7f0300 + i as u32, value);
    }
    b.write(0x430a, 0xff); // Initialization must replace a CPU-written counter.
    if sibling {
        channel(&mut b, 2, false, 0x600, 0x42);
        for (i, value) in [0x83, 0x77, 0x88, 0x99, 0].into_iter().enumerate() {
            b.write(0x7e0600 + i as u32, value);
        }
    }
    b.write(0x420c, if sibling { 5 } else { 1 });
    until(&mut b, 1, 0, 200);
    assert_eq!(b.read(0x430a), header);
    assert_eq!(pair(&mut b, 0, 8), if indirect { 0x103 } else { 0x101 });
    until(&mut b, 1, 0, 1200);
    assert_eq!(b.apu.input[0], 0x11);
    assert_eq!(b.read(0x430a), if transferring { 0x81 } else { 1 });
    b
}

#[test]
fn live_02_uses_written_counter_instead_of_stale_81() {
    for indirect in [false, true] {
        let mut b = configured(indirect, true);
        b.write(0x430a, 2);
        assert_eq!(b.read(0x430a), 2);
        until(&mut b, 1, 1, 1200);
        assert_eq!(b.read(0x430a), 1); // No premature reload of $83.
        assert_eq!(b.apu.input[0], 0x22);
        assert_eq!(pair(&mut b, 0, 8), 0x103);
        assert_eq!(pair(&mut b, 0, 5), if indirect { 0x202 } else { 0x5aa6 });
        b.write(0x2140, 0xee);
        until(&mut b, 1, 2, 1200);
        assert_eq!(b.apu.input[0], 0xee); // Clearing repeat affects this later event.
        assert_eq!(b.read(0x430a), 0x83);
        assert_eq!(pair(&mut b, 0, 8), if indirect { 0x106 } else { 0x104 });
        assert_eq!(pair(&mut b, 0, 5), if indirect { 0x300 } else { 0x5aa6 });
        for (line, payload, counter) in [(3, 0x33, 0x82), (4, 0x44, 0x81), (5, 0x55, 0)] {
            until(&mut b, 1, line, 1200);
            assert_eq!((b.apu.input[0], b.read(0x430a)), (payload, counter));
        }
        assert_eq!(pair(&mut b, 0, 8), 0x108);
        assert_eq!(pair(&mut b, 0, 5), if indirect { 0x1200 } else { 0x5aa6 });
        assert_eq!((b.hdma_active, b.io[12]), (0, 1));
        b.write(0x2140, 0xee);
        until(&mut b, 1, 6, 1200);
        assert_eq!((b.apu.input[0], b.read(0x430a)), (0xee, 0));
    }
}

#[test]
fn live_boundaries_preserve_prepared_transfer_then_use_new_repeat() {
    // Derived from decrement-before-repeat/exhaustion, not descriptor-fetch behavior.
    for indirect in [false, true] {
        for transferring in [false, true] {
            for (written, next, following) in [
                (0x00, 0xff, 0xfe),
                (0x01, 0x83, 0x82),
                (0x02, 0x01, 0x83),
                (0x7f, 0x7e, 0x7d),
                (0x80, 0x7f, 0x7e),
                (0x81, 0x83, 0x82),
                (0x82, 0x81, 0x80),
                (0xff, 0xfe, 0xfd),
            ] {
                let mut b = configured(indirect, transferring);
                // A continuing repeat row must have a third payload, then a zero
                // descriptor on the following reload (written $82 -> $81 -> $80).
                if written == 0x82 && !indirect {
                    b.write(if transferring { 0x7e0103 } else { 0x7e0102 }, 0x66);
                    b.write(if transferring { 0x7e0104 } else { 0x7e0103 }, 0);
                }
                b.write(0x7f0202, 0x66);
                b.write(0x2140, 0xee);
                b.write(0x430a, written);
                assert_eq!(b.read(0x430a), written);
                until(&mut b, 1, 1, 1200);
                assert_eq!(b.apu.input[0], if transferring { 0x22 } else { 0xee });
                assert_eq!(b.read(0x430a), next);
                let reloaded = written == 1 || written == 0x81;
                let old_table = if indirect || transferring {
                    0x103
                } else {
                    0x102
                };
                assert_eq!(
                    pair(&mut b, 0, 8),
                    if reloaded {
                        if indirect {
                            0x106
                        } else {
                            old_table + 1
                        }
                    } else {
                        old_table
                    }
                );
                let old_pointer = if transferring { 0x202 } else { 0x201 };
                assert_eq!(
                    pair(&mut b, 0, 5),
                    if indirect {
                        if reloaded {
                            0x300
                        } else {
                            old_pointer
                        }
                    } else {
                        0x5aa6
                    }
                );
                b.write(0x2140, 0xee);
                until(&mut b, 1, 2, 1200);
                let repeats = reloaded || matches!(written, 0 | 0x82 | 0xff);
                let payload = if reloaded {
                    0x33
                } else if indirect {
                    if transferring {
                        0x66
                    } else {
                        0x22
                    }
                } else if written == 0x82 {
                    0x66
                } else {
                    0x83
                };
                assert_eq!(b.apu.input[0], if repeats { payload } else { 0xee });
                // $82 exhausts here: direct fetches zero; indirect fetches $83.
                let expected = if written == 0x82 {
                    if indirect {
                        0x83
                    } else {
                        0
                    }
                } else {
                    following
                };
                assert_eq!(b.read(0x430a), expected);
                assert_eq!(b.read(0x4302), 0);
                assert_eq!(b.read(0x4303), 1);
                assert_eq!((b.read(0x4304), b.read(0x4307)), (0x7e, 0x7f));
                assert_eq!(b.io[12], 1);
            }
        }
    }
}

#[test]
fn live_boundaries_reach_exhaustion_and_only_fetched_zero_terminates() {
    for indirect in [false, true] {
        for transferring in [false, true] {
            for (written, events, repeat) in [
                (0x00u8, 128, true),
                (0x01, 1, false),
                (0x02, 2, false),
                (0x7f, 127, false),
                (0x80, 128, false),
                (0x81, 1, true),
                (0x82, 2, true),
                (0xff, 127, true),
            ] {
                let mut b = configured(indirect, transferring);
                // All remaining data plus the final zero descriptor are original
                // zero bytes. Nonzero sentinel detects a zero-valued transfer.
                if indirect {
                    for a in 0x7e0103..0x7e0108 {
                        b.write(a, 0);
                    }
                    for a in 0x7f0201..0x7f0282 {
                        b.write(a, 0);
                    }
                } else {
                    for a in 0x7e0102..0x7e0184 {
                        b.write(a, 0);
                    }
                }
                b.write(0x430a, written);
                for event in 1..=events {
                    b.write(0x2140, 0xee);
                    until(&mut b, 1, event, 1200);
                    let transfer = if event == 1 { transferring } else { repeat };
                    assert_eq!(b.apu.input[0], if transfer { 0 } else { 0xee });
                    let expected = if event == events {
                        0
                    } else {
                        written.wrapping_sub(event as u8)
                    };
                    assert_eq!(b.read(0x430a), expected);
                    assert_eq!((b.hdma_active, b.io[12]), (u8::from(event != events), 1));
                }
                let consumed = u16::from(transferring) + if repeat { events - 1 } else { 0 };
                assert_eq!(
                    pair(&mut b, 0, 8),
                    if indirect { 0x105 } else { 0x103 + consumed }
                );
                assert_eq!(pair(&mut b, 0, 5), if indirect { 0 } else { 0x5aa6 });
                b.write(0x2140, 0xee);
                until(&mut b, 1, events + 1, 1200);
                assert_eq!(b.apu.input[0], 0xee);
            }
        }
    }
}

#[test]
fn live_writes_are_channel_local_and_do_not_schedule_inactive_work() {
    for indirect in [false, true] {
        let mut b = configured_with_sibling(indirect, true, true);
        assert_eq!((b.read(0x432a), b.apu.input[2]), (0x82, 0x77));
        assert_eq!(pair(&mut b, 2, 8), 0x602);
        // Channel 1 was not enabled at frame initialization. Merely writing its
        // counter leaves it outside the scheduled set (no enabling experiment).
        channel(&mut b, 1, indirect, 0x500, 0x41);
        b.write(0x431a, 0x7f);
        b.write(0x430a, 2);
        until(&mut b, 1, 1, 1200);
        assert_eq!((b.read(0x430a), b.read(0x431a)), (1, 0x7f));
        assert_eq!((pair(&mut b, 1, 8), pair(&mut b, 1, 5)), (0xffff, 0x5aa6));
        assert_eq!(b.apu.input[1], 0);
        assert_eq!((b.read(0x432a), b.apu.input[2]), (0x81, 0x88));
        assert_eq!(pair(&mut b, 2, 8), 0x603);
        assert_eq!(pair(&mut b, 2, 5), 0x5aa6);
        // Let channel 0 reload and terminate normally.
        until(&mut b, 1, 5, 1200);
        assert_eq!((b.hdma_active, b.io[12]), (0, 5));
        let table = pair(&mut b, 0, 8);
        let pointer = pair(&mut b, 0, 5);
        b.write(0x430a, 0x82);
        b.write(0x2140, 0xee);
        until(&mut b, 1, 6, 1200);
        assert_eq!((b.apu.input[0], b.read(0x430a)), (0xee, 0x82));
        assert_eq!((pair(&mut b, 0, 8), pair(&mut b, 0, 5)), (table, pointer));
        assert_eq!((b.hdma_active, b.io[12]), (0, 5));
    }
}
