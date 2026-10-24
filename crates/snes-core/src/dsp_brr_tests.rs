//! Original MIT specification vectors. See docs/issue-93-validation.md.
use super::{Dsp, Voice};

// Independent full-amplitude model, with mathematical floor divisions and
// modular reduction rather than the decoder's half-amplitude shifts/casts.
// P and R are even signed-16 decoded results; Q=floor(R/2).
fn full(n: i64, range: u8, filter: u8, [p, r]: [i64; 2]) -> i64 {
    let b = if range <= 12 {
        (n * 2i64.pow(range.into())).div_euclid(2)
    } else if n < 0 {
        -2048
    } else {
        0
    };
    let q = r.div_euclid(2);
    let t = b + match filter {
        0 => 0,
        1 => p.div_euclid(2) + (-p).div_euclid(32),
        2 => p - q + q.div_euclid(16) + (-3 * p).div_euclid(64),
        3 => p - q + (-13 * p).div_euclid(128) + (3 * q).div_euclid(16),
        _ => unreachable!(),
    };
    (2 * t.clamp(-32768, 32767) + 32768).rem_euclid(65536) - 32768
}

#[test]
fn worked_full_amplitude_anchors() {
    // filter,n,r,[P,R],full. Worked arithmetic is frozen in the validation doc.
    for (f, n, r, h, expected) in [
        (1, 0, 0, [2, 0], 0),
        (1, 0, 0, [-2, 0], -2),
        (2, 0, 0, [6, -10], 18),
        (2, 0, 0, [-10, 6], -26),
        (3, 0, 0, [6, -10], 18),
        (3, 0, 0, [-10, 6], -24),
        (1, 7, 12, [32766, -32768], -6146),
        (1, -8, 12, [-32768, 32766], 2048),
        (2, 7, 12, [32766, -32768], -2),
        (2, -8, 12, [-32768, 32766], 0),
        (3, 7, 12, [32766, -32768], -2),
        (3, -8, 12, [-32768, 32766], 0),
    ] {
        assert_eq!(full(n, r, f, h), expected);
    }
}

#[test]
fn all_predictor_filters_nibbles_ranges_and_valid_histories() {
    let histories = [
        [0, 0],
        [2, 0],
        [-2, 0],
        [0, 2],
        [0, -2],
        [6, -10],
        [-10, 6],
        [-6, 10],
        [10, -6],
        [1024, 4096],
        [4096, 1024],
        [-1024, -4096],
        [-4096, -1024],
        [32766, -32768],
        [-32768, 32766],
        [32766, 32766],
        [-32768, -32768],
    ];
    let mut ram = vec![0; 65536];
    for filter in 1..=3 {
        for range in 0..=15 {
            ram[0x3000] = range << 4 | filter << 2;
            for nibble in 0..16 {
                ram[0x3001] = nibble << 4 | nibble;
                let n = if nibble < 8 {
                    nibble as i64
                } else {
                    nibble as i64 - 16
                };
                for h in histories {
                    let mut v = Voice {
                        addr: 0x3000,
                        history: h.map(|s| (s / 2) as i32),
                        ..Voice::default()
                    };
                    let mut expected = h;
                    // The second nibble must consume the wrapped first result.
                    for _ in 0..2 {
                        let next = full(n, range, filter, expected);
                        expected = [next, expected[0]];
                        assert!(!Dsp::decode(&mut v, &ram));
                        assert_eq!(
                            v.history.map(|s| i64::from(s) * 2),
                            expected,
                            "filter {filter}, range {range}, nibble {n}, initial {h:?}"
                        );
                        assert_eq!(i64::from(v.decoded[3]) * 2, next);
                    }
                }
            }
        }
    }
}

const DATA: [u8; 8] = [0x4c, 0x2f, 0x61, 0xd3, 0x85, 0xe7, 0x09, 0x2b];
fn block(ram: &mut [u8], addr: usize, header: u8, bytes: [u8; 8]) {
    ram[addr] = header;
    ram[addr + 1..addr + 9].copy_from_slice(&bytes);
}
fn nibble(bytes: [u8; 8], index: usize) -> i64 {
    let n = (bytes[index / 2] >> (4 * (1 - index % 2))) & 15;
    if n < 8 {
        n as i64
    } else {
        n as i64 - 16
    }
}

#[test]
fn sequential_and_two_loop_traversals_keep_both_histories_per_voice() {
    let mut ram = vec![0; 65536];
    // A -> B -> C -> D -> C -> D -> C, changing both filter and range.
    block(&mut ram, 0x3000, 0x94, DATA);
    block(&mut ram, 0x3009, 0x7b, [0x6e; 8]);
    block(&mut ram, 0x3100, 0xac, DATA);
    block(&mut ram, 0x3109, 0xd7, [0x8f; 8]);
    let mut voices = [
        Voice {
            addr: 0x3000,
            loop_addr: 0x3100,
            history: [3, -5],
            ..Voice::default()
        },
        Voice {
            addr: 0x3100,
            loop_addr: 0x3100,
            history: [-7, 11],
            ..Voice::default()
        },
    ];
    let mut histories = [[6, -10], [-14, 22]];
    let mut stale = histories;
    // Independently frozen full-amplitude stream boundary pairs, newest first.
    let anchors = [
        (0, [2052, 6]),
        (15, [-1612, 1012]),
        (16, [-3256, -1612]),
        (31, [10928, 9496]),
        (32, [16016, 10928]),
        (47, [12404, -16340]),
        (48, [7532, 12404]),
        (63, [19804, 25494]),
        (64, [18966, 19804]),
        (79, [5250, 11898]),
        (80, [824, 5250]),
        (95, [10286, 15342]),
        (96, [10112, 10286]),
        (111, [3986, 10366]),
    ];
    for step in 0..112 {
        for i in 0..2 {
            let group = step / 16;
            let (header, data) = if i == 0 && group < 2 {
                if group == 0 {
                    (0x94, DATA)
                } else {
                    (0x7b, [0x6e; 8])
                }
            } else if (group - if i == 0 { 2 } else { 0 }) % 2 == 0 {
                (0xac, DATA)
            } else {
                (0xd7, [0x8f; 8])
            };
            let n = nibble(data, step % 16);
            let next = full(n, header >> 4, (header >> 2) & 3, histories[i]);
            if step % 16 == 0 && step > 0 {
                // Every transition distinguishes reset, reversed and stale
                // histories; the other voice is an additional wrong-history control.
                for wrong in [
                    [0, 0],
                    [histories[i][1], histories[i][0]],
                    stale[i],
                    histories[1 - i],
                ] {
                    assert_ne!(next, full(n, header >> 4, (header >> 2) & 3, wrong));
                }
                stale[i] = histories[i];
            }
            let other = voices[1 - i].history;
            let ended = Dsp::decode(&mut voices[i], &ram);
            let expected_end = step % 16 == 0
                && step > 0
                && (if i == 0 {
                    group == 2 || (group > 2 && group % 2 == 0)
                } else {
                    group % 2 == 0
                });
            assert_eq!(ended, expected_end, "voice {i}, step {step}");
            histories[i] = [next, histories[i][0]];
            if i == 0 {
                if let Some((_, pair)) = anchors.iter().find(|(at, _)| *at == step) {
                    assert_eq!(histories[i], *pair, "frozen stream step {step}");
                }
            }
            assert_eq!(
                voices[i].history.map(|s| i64::from(s) * 2),
                histories[i],
                "voice {i}, step {step}"
            );
            assert_eq!(i64::from(voices[i].decoded[3]) * 2, next);
            assert_eq!(voices[1 - i].history, other);
        }
    }
}

#[test]
fn predictor_history_stays_ahead_of_held_preload_window_then_continues_once() {
    let first_blocks = [
        [
            1024, -64, 452, 167, 1692, 1842, 958, 1666, -487, 823, 259, 2034, 1906, -6, 506, -806,
        ],
        [
            1024, 928, 1321, 1392, 2950, 4574, 5185, 6362, 5218, 5261, 4624, 5673, 6479, 5239,
            4423, 2239,
        ],
        [
            1024, 816, 1146, 1140, 2652, 4094, 4433, 5406, 4063, 4187, 3709, 5054, 6067, 5002,
            4569, 2864,
        ],
    ];
    for filter in 1..=3 {
        let mut ram = vec![0; 65536];
        ram[0x2000..0x2004].copy_from_slice(&[0, 0x30, 0, 0x30]);
        block(&mut ram, 0x3000, 0x93 | filter << 2, DATA);
        let mut expected = Vec::new();
        let mut history = [0, 0];
        for step in 0..64 {
            let next = full(nibble(DATA, step % 16), 9, filter, history);
            history = [next, history[0]];
            expected.push((next / 2) as i32);
        }
        assert_eq!(expected[..16], first_blocks[filter as usize - 1]);
        let mut d = Dsp::default();
        for (r, v) in [
            (0x6c, 0x20),
            (0x5d, 0x20),
            (0x0c, 64),
            (0, 64),
            (7, 64),
            (0x4c, 1),
        ] {
            d.write(r, v);
        }
        for call in 1..=5 {
            assert_eq!(d.sample(&mut ram), (0, 0));
            assert_eq!([d.regs[8], d.regs[9]], [0, 0]);
            let count = match call {
                1 => 0,
                2 => 4,
                3 => 8,
                _ => 12,
            };
            if count != 0 {
                assert_eq!(
                    d.voices[0].history,
                    [expected[count - 1], expected[count - 2]]
                );
            }
        }
        assert_eq!(d.voices[0].preload, expected[..12]);
        assert_eq!(d.voices[0].decoded, expected[..4]);
        for _ in 0..32 {
            d.sample(&mut ram);
            assert_eq!(d.voices[0].decoded, expected[..4]);
            assert_eq!(d.voices[0].history, [expected[11], expected[10]]);
            assert_eq!(d.voices[0].nibble, 12);
        }
        d.write(3, 0x10);
        for position in 1..=48 {
            d.sample(&mut ram);
            assert_eq!(d.voices[0].decoded, expected[position..position + 4]);
            let newest = (position + 3).max(11);
            assert_eq!(
                d.voices[0].history,
                [expected[newest], expected[newest - 1]],
                "filter {filter}, position {position}"
            );
            assert_eq!(d.voices[0].nibble as usize, newest % 16 + 1);
            assert_eq!(d.voices[0].preload_next as usize, (position + 4).min(12));
            if position == 8 || position == 9 {
                d.write(3, 0);
                let held = d.voices[0];
                for _ in 0..16 {
                    d.sample(&mut ram);
                    assert_eq!(d.voices[0].history, held.history);
                    assert_eq!(d.voices[0].decoded, held.decoded);
                    assert_eq!(d.voices[0].preload_next, held.preload_next);
                    assert_eq!(d.voices[0].addr, held.addr);
                }
                d.write(3, 0x10);
            }
        }
    }
}
