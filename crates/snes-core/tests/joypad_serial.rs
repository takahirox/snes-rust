//! Original MIT manual two-pad vectors and 65C816 guest for Issue #67.
//! Copyright (c) 2026 snes-rust contributors.
use snes_core::{cpu::Bus, Machine};

fn machine() -> Machine {
    Machine::new(include_bytes!("../../../tests/roms/smoke.bin")).unwrap()
}

#[test]
fn released_second_pad_trails_with_ones() {
    let mut m = machine();
    m.bus.write(0x4200, 0);
    m.bus.write(0x4016, 1);
    m.bus.write(0x4016, 0);
    for position in 0..24 {
        assert_eq!(
            m.bus.read(0x4017) & 1,
            u8::from(position >= 16),
            "position {position}"
        );
    }
}

// MSB-first expansion of $A590/$5A60 in the documented B..R,0000 order.
const FIRST: [u8; 16] = [1, 0, 1, 0, 0, 1, 0, 1, 1, 0, 0, 1, 0, 0, 0, 0];
const SECOND: [u8; 16] = [0, 1, 0, 1, 1, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0];

fn latch(m: &mut Machine) {
    m.bus.write(0x4016, 1);
    m.bus.write(0x4016, 0);
}

#[test]
fn every_button_on_each_port_and_unsupported_bits() {
    for port in 0..2 {
        for button in 0..12 {
            let mut m = machine();
            // Independent derivation: button N occupies input bit 15-N and
            // serial position N. Low input bits must never appear as data.
            let input = (1 << (15 - button)) | 0x000f;
            if port == 0 {
                m.set_buttons(input);
                m.set_buttons2(0x000f);
            } else {
                m.set_buttons2(input);
                m.set_buttons(0x000f);
            }
            latch(&mut m);
            for position in 0..24 {
                for read_port in 0..2 {
                    let expected =
                        u8::from(position >= 16 || (port == read_port && position == button));
                    assert_eq!(
                        m.bus.read(0x4016 + read_port) & 1,
                        expected,
                        "port {port}, button {button}, read {read_port}, position {position}"
                    );
                }
            }
        }
    }
}

#[test]
fn complementary_patterns_have_independent_unequal_clocks() {
    for leading_port in 0..2 {
        let mut m = machine();
        m.set_buttons(0xa59f);
        m.set_buttons2(0x5a6f);
        latch(&mut m);
        let sequences = [FIRST, SECOND];
        let mut positions = [0; 2];
        // Exhaust one pad well past its tail while the other is still in
        // its button sequence. Repeat with the port roles reversed.
        for port in [leading_port; 7]
            .into_iter()
            .chain([1 - leading_port, leading_port, leading_port].repeat(9))
            .chain([1 - leading_port; 17])
        {
            let position = positions[port];
            let expected = sequences[port].get(position).copied().unwrap_or(1);
            assert_eq!(
                m.bus.read(0x4016 + port as u32) & 1,
                expected,
                "port {port}, position {position}"
            );
            positions[port] += 1;
        }
    }
}

#[test]
fn shared_level_latch_live_high_capture_and_repeated_low() {
    let mut m = machine();
    m.set_buttons(0xa590);
    m.set_buttons2(0x5a60);
    m.bus.write(0x4016, 0xff); // Only bit zero controls the shared latch.
    for (first, second) in [(0xa590, 0x5a60), (0x5a60, 0xa590), (0xa590, 0x5a60)] {
        m.set_buttons(first);
        m.set_buttons2(second);
        for _ in 0..20 {
            assert_eq!(m.bus.read(0x4016) & 1, (first >> 15) as u8);
            assert_eq!(m.bus.read(0x4017) & 1, (second >> 15) as u8);
        }
        m.bus.write(0x804016, 1); // Repeated high still holds B.
    }
    // Change after the last high write: the falling edge captures these.
    m.set_buttons(0x5a60);
    m.set_buttons2(0xa590);
    m.bus.write(0xbf4016, 0xfe);
    m.set_buttons(0xa590);
    m.set_buttons2(0x5a60);
    for position in 0..24 {
        m.bus.write(0x3f4016, 0x80); // Repeated low must not restart either pad.
        assert_eq!(
            m.bus.read(0x4016) & 1,
            SECOND.get(position).copied().unwrap_or(1)
        );
        assert_eq!(
            m.bus.read(0x4017) & 1,
            FIRST.get(position).copied().unwrap_or(1)
        );
    }
    latch(&mut m); // Both ports relatch the input changes made while low.
    for position in 0..16 {
        assert_eq!(m.bus.read(0x4016) & 1, FIRST[position]);
        assert_eq!(m.bus.read(0x4017) & 1, SECOND[position]);
    }
}

#[test]
fn masks_and_system_bank_mirrors_share_state() {
    let mut m = machine();
    for bank in (0..=0x3f).chain(0x80..=0xbf) {
        for prior in [0x00, 0xff, 0xa5, 0x5a] {
            for swapped in [false, true] {
                let (inputs, sequences) = if swapped {
                    ([0x5a60, 0xa590], [SECOND, FIRST])
                } else {
                    ([0xa590, 0x5a60], [FIRST, SECOND])
                };
                m.set_buttons(inputs[0]);
                m.set_buttons2(inputs[1]);
                m.bus.write((bank << 16) | 0x4016, 1);
                for _ in 0..3 {
                    for (port, sequence) in sequences.iter().enumerate() {
                        m.bus.write(0x1000, prior);
                        assert_eq!(m.bus.read(0x1000), prior);
                        let unused = if port == 0 {
                            prior & 0xfc
                        } else {
                            (prior & 0xe0) | 0x1c
                        };
                        assert_eq!(
                            m.bus.read((bank << 16) | (0x4016 + port as u32)),
                            unused | sequence[0]
                        );
                    }
                }
                m.bus.write(0x4016, 0);
                for position in 0..24 {
                    // Alternate each bank's reads with its opposite mirror.
                    let read_bank = if position % 2 == 0 { bank } else { bank ^ 0x80 };
                    for (port, sequence) in sequences.iter().enumerate() {
                        m.bus.write(0x1000, prior);
                        assert_eq!(m.bus.read(0x1000), prior);
                        let unused = if port == 0 {
                            prior & 0xfc
                        } else {
                            (prior & 0xe0) | 0x1c
                        };
                        let data = sequence.get(position).copied().unwrap_or(1);
                        assert_eq!(
                            m.bus.read((read_bank << 16) | (0x4016 + port as u32)),
                            unused | data
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn outside_system_banks_keep_cartridge_and_wram_decoding() {
    let mut rom = guest_rom(&[0xdb]);
    rom[0x4016..0x4018].copy_from_slice(&[0x36, 0x49]);
    let mut m = Machine::new(&rom).unwrap();
    m.set_buttons(0xa590);
    m.set_buttons2(0x5a60);
    latch(&mut m);
    // An already-consumed B bit makes accidental outside-bank relatching visible.
    assert_eq!(m.bus.read(0x4016) & 1, 1);
    assert_eq!(m.bus.read(0x4017) & 1, 0);
    for bank in (0x40..=0x7d).chain(0xc0..=0xff) {
        for (register, value) in [(0x4016, 0x36), (0x4017, 0x49)] {
            let address = (bank << 16) | register;
            assert_eq!(m.bus.read(address), value);
            m.bus.write(address, 1);
            m.bus.write(address, 0);
            assert_eq!(m.bus.read(address), value);
        }
    }
    for bank in [0x7e, 0x7f] {
        for (register, value) in [(0x4016, 0x63), (0x4017, 0x94)] {
            let address = (bank << 16) | register;
            m.bus.write(address, value);
            assert_eq!(m.bus.read(address), value);
        }
    }
    for position in 1..16 {
        assert_eq!(m.bus.read(0x4016) & 1, FIRST[position]);
        assert_eq!(m.bus.read(0x4017) & 1, SECOND[position]);
    }
}

#[test]
fn reset_releases_both_inputs_and_serial_state() {
    let mut m = machine();
    m.set_buttons(0xffff);
    m.set_buttons2(0xffff);
    latch(&mut m);
    m.bus.read(0x4017);
    m.bus.write(0x4016, 1);
    m.reset();
    for position in 0..24 {
        for port in [0x4016, 0x4017] {
            assert_eq!(m.bus.read(port) & 1, u8::from(position >= 16));
        }
    }
    latch(&mut m);
    for _ in 0..16 {
        assert_eq!(m.bus.read(0x4016) & 1, 0);
        assert_eq!(m.bus.read(0x4017) & 1, 0);
    }
}

fn guest_rom(code: &[u8]) -> Vec<u8> {
    assert!(code.len() < 0x7fc0);
    let mut rom = vec![0; 32768];
    rom[..code.len()].copy_from_slice(code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"TWO PAD ORIGINAL     ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc..0x7fe0].copy_from_slice(&[255, 255, 0, 0]);
    rom[0x7ffc..0x7ffe].copy_from_slice(&[0, 0x80]);
    rom
}

#[test]
fn cpu_guest_records_both_manual_sequences_in_wram() {
    let mut code = vec![0x78]; // SEI, reset's eight-bit accumulator.
    for (address, value) in [(0x4200_u16, 0), (0x4016, 1), (0x4016, 0)] {
        code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
    }
    let mut save = |port: u8, position: u16| {
        let dest = 0x1800 + u16::from(port) * 0x40 + position;
        // LDA long (system-bank mirrors), AND #1, STA absolute.
        code.extend([
            0xaf,
            0x16 + port,
            0x40,
            0x80 + port,
            0x29,
            1,
            0x8d,
            dest as u8,
            (dest >> 8) as u8,
        ]);
    };
    // Deliberately unequal clocks before interleaving the rest.
    for position in 0..7 {
        save(0, position);
    }
    for position in 0..3 {
        save(1, position);
    }
    for position in 3..24 {
        save(1, position);
        if position >= 7 {
            save(0, position);
        }
    }
    code.extend([0xa9, 0xa5, 0x8d, 0xf0, 0x18, 0xdb]); // completion + STP
    let mut m = Machine::new(&guest_rom(&code)).unwrap();
    m.set_buttons(0xa590);
    m.set_buttons2(0x5a60);
    for _ in 0..10_000 {
        m.step();
        assert!(m.bus.frames < 2, "manual guest frame deadline");
        assert!(m.bus.apu.error.is_none());
        if m.cpu.stopped {
            assert_eq!(m.bus.peek(0x18f0), 0xa5);
            for (port, sequence) in [FIRST, SECOND].iter().enumerate() {
                let results: Vec<_> = (0..24)
                    .map(|position| m.bus.peek(0x1800 + port as u32 * 0x40 + position))
                    .collect();
                let expected: Vec<_> = sequence.iter().copied().chain([1; 8]).collect();
                assert_eq!(results, expected, "guest port {port}");
            }
            return;
        }
    }
    panic!("manual guest instruction deadline");
}
