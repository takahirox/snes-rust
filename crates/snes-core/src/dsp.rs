//! 32 kHz BRR voice mixer, authored from S-DSP register documentation.
use crate::gaussian::GAUSSIAN;
#[derive(Default, Clone, Copy)]
struct Voice {
    addr: u16,
    loop_addr: u16,
    header: u8,
    nibble: u8,
    history: [i32; 2],
    decoded: [i32; 4],
    key_delay: u8,
    phase: u32,
    env: i32,
    stage: u8,
    active: bool,
}
pub struct Dsp {
    pub regs: [u8; 128],
    voices: [Voice; 8],
    kon: u8,
    counter: u32,
    echo_pos: usize,
    echo_hist: [[i32; 2]; 8],
    echo_index: usize,
}
impl Default for Dsp {
    fn default() -> Self {
        Self {
            regs: {
                let mut r = [0; 128];
                r[0x6c] = 0xe0;
                r
            },
            voices: [Voice::default(); 8],
            kon: 0,
            counter: 0,
            echo_pos: 0,
            echo_hist: [[0; 2]; 8],
            echo_index: 0,
        }
    }
}
impl Dsp {
    pub fn write(&mut self, r: u8, v: u8) {
        match r {
            0x4c => {
                self.kon = v;
                self.regs[0x4c] = v;
            }
            0x7c => self.regs[0x7c] = 0,
            _ => self.regs[r as usize] = v,
        }
    }
    fn word(ram: &[u8], a: u16) -> u16 {
        u16::from_le_bytes([ram[a as usize], ram[a.wrapping_add(1) as usize]])
    }
    fn decode(v: &mut Voice, ram: &[u8]) -> bool {
        let ended = v.nibble == 16 && v.header & 1 != 0;
        if v.nibble == 16 {
            if v.header & 1 != 0 {
                if v.header & 2 == 0 {
                    v.active = false;
                    v.env = 0;
                    v.decoded = [0; 4];
                    return true;
                }
                v.addr = v.loop_addr;
            }
            v.nibble = 0;
        }
        if v.nibble == 0 {
            v.header = ram[v.addr as usize];
            v.addr = v.addr.wrapping_add(1);
        }
        let byte = ram[v.addr.wrapping_add((v.nibble / 2) as u16) as usize];
        let nibble = if v.nibble & 1 == 0 {
            byte >> 4
        } else {
            byte & 15
        };
        let signed = ((nibble as i8) << 4) >> 4;
        let shift = v.header >> 4;
        let mut sample = if shift <= 12 {
            ((signed as i32) << shift) >> 1
        } else if signed < 0 {
            -2048
        } else {
            0
        };
        let p = v.history[0];
        let q = v.history[1];
        match (v.header >> 2) & 3 {
            1 => sample += p + ((-p) >> 4),
            2 => sample += p * 2 + ((-p * 3) >> 5) - q + (q >> 4),
            3 => sample += p * 2 + ((-p * 13) >> 6) - q + ((q * 3) >> 4),
            _ => {}
        }
        sample = (sample.clamp(-32768, 32767) as i16).wrapping_mul(2) as i32 >> 1;
        v.history[1] = p;
        v.history[0] = sample;
        v.decoded.rotate_left(1);
        v.decoded[3] = sample;
        v.nibble += 1;
        if v.nibble == 16 {
            v.addr = v.addr.wrapping_add(8);
        }
        ended
    }
    fn interpolate(samples: [i32; 4], phase: u32) -> i32 {
        let index = (phase >> 4) as usize;
        // The first three taps wrap at 16 bits; the final sum saturates.
        // BRR history is stored at half amplitude (15 bits).
        let partial = ((GAUSSIAN[255 - index] * samples[0]) >> 10)
            + ((GAUSSIAN[511 - index] * samples[1]) >> 10)
            + ((GAUSSIAN[256 + index] * samples[2]) >> 10);
        ((partial as i16 as i32) + ((GAUSSIAN[index] * samples[3]) >> 10)).clamp(-32768, 32767) & !1
    }
    pub fn sample(&mut self, ram: &mut [u8]) -> (i16, i16) {
        self.counter = self.counter.wrapping_add(1);
        let mut mix = [0i32; 2];
        let mut echo_in = [0i32; 2];
        for i in 0..8 {
            let base = i * 16;
            if self.kon & (1 << i) != 0 {
                let dir = (self.regs[0x5d] as u16) << 8;
                let entry = dir.wrapping_add(self.regs[base + 4] as u16 * 4);
                let start = Self::word(ram, entry);
                let loop_addr = Self::word(ram, entry.wrapping_add(2));
                self.voices[i] = Voice {
                    addr: start,
                    loop_addr,
                    active: true,
                    key_delay: 5,
                    ..Voice::default()
                };
                self.regs[0x7c] &= !(1 << i);
            }
            let v = &mut self.voices[i];
            if self.regs[0x5c] & (1 << i) != 0 {
                v.stage = 3;
            }
            if self.regs[0x6c] & 128 != 0 {
                v.active = false;
                v.env = 0;
            }
            if !v.active {
                self.regs[base + 8] = 0;
                self.regs[base + 9] = 0;
                continue;
            }
            if v.key_delay > 0 {
                v.key_delay -= 1;
                self.regs[base + 8] = 0;
                self.regs[base + 9] = 0;
                continue;
            }
            let adsr = self.regs[base + 5];
            let adsr2 = self.regs[base + 6];
            let gain = self.regs[base + 7];
            if v.stage == 3 {
                v.env -= 8;
            } else if adsr & 128 != 0 {
                match v.stage {
                    0 => {
                        let rate = (adsr & 15) * 2 + 1;
                        if rate == 31 || self.counter.is_multiple_of(RATES[rate as usize]) {
                            v.env += if rate == 31 { 1024 } else { 32 };
                            if v.env >= 2047 {
                                v.env = 2047;
                                v.stage = 1;
                            }
                        }
                    }
                    1 | 2 => {
                        let rate = if v.stage == 1 {
                            ((adsr >> 4) & 7) * 2 + 16
                        } else {
                            adsr2 & 31
                        };
                        if rate != 0 && self.counter.is_multiple_of(RATES[rate as usize]) {
                            v.env -= ((v.env - 1) >> 8) + 1;
                        }
                        if v.stage == 1 && v.env <= (((adsr2 >> 5) as i32 + 1) * 256) {
                            v.stage = 2;
                        }
                    }
                    _ => {}
                }
            } else if gain & 128 == 0 {
                v.env = (gain as i32) * 16;
            } else {
                let rate = gain & 31;
                if rate != 0 && self.counter.is_multiple_of(RATES[rate as usize]) {
                    match (gain >> 5) & 3 {
                        0 => v.env -= 32,
                        1 => v.env -= ((v.env - 1) >> 8) + 1,
                        2 => v.env += 32,
                        _ => v.env += if v.env < 1536 { 32 } else { 8 },
                    }
                }
            }
            v.env = v.env.clamp(0, 2047);
            if v.stage == 3 && v.env == 0 {
                v.active = false;
            }
            let pitch =
                (self.regs[base + 2] as u32 | ((self.regs[base + 3] as u32 & 63) << 8)).min(0x3fff);
            v.phase += pitch;
            while v.phase >= 4096 {
                v.phase -= 4096;
                if Self::decode(v, ram) {
                    self.regs[0x7c] |= 1 << i;
                }
            }
            let sample = ((Self::interpolate(v.decoded, v.phase) * v.env) >> 11) & !1;
            self.regs[base + 8] = (v.env >> 4) as u8;
            self.regs[base + 9] = (sample >> 8) as u8;
            for side in 0..2 {
                let s = (sample * self.regs[base + side] as i8 as i32) >> 7;
                mix[side] += s;
                if self.regs[0x4d] & (1 << i) != 0 {
                    echo_in[side] += s;
                }
            }
        }
        self.kon = 0;
        let ea = ((self.regs[0x6d] as usize) << 8) + self.echo_pos;
        self.echo_hist[self.echo_index] = [
            Self::word(ram, ea as u16) as i16 as i32,
            Self::word(ram, (ea + 2) as u16) as i16 as i32,
        ];
        let mut out = [0i16; 2];
        for side in 0..2 {
            let mut echo = 0;
            for tap in 0..8 {
                echo += (self.echo_hist[(self.echo_index + 8 - tap) & 7][side]
                    * self.regs[tap * 16 + 15] as i8 as i32)
                    >> 7;
            }
            if self.regs[0x6c] & 32 == 0 {
                let feedback = (echo_in[side] + ((echo * self.regs[0x0d] as i8 as i32) >> 7))
                    .clamp(-32768, 32767) as i16;
                let bytes = feedback.to_le_bytes();
                ram[(ea + side * 2) & 65535] = bytes[0];
                ram[(ea + side * 2 + 1) & 65535] = bytes[1];
            }
            let value = ((mix[side] * self.regs[0x0c + side * 16] as i8 as i32) >> 7)
                + ((echo * self.regs[0x2c + side * 16] as i8 as i32) >> 7);
            out[side] = if self.regs[0x6c] & 64 != 0 {
                0
            } else {
                value.clamp(-32768, 32767) as i16
            };
        }
        self.echo_index = (self.echo_index + 1) & 7;
        self.echo_pos += 4;
        let len = ((self.regs[0x7d] & 15) as usize) * 2048;
        if self.echo_pos >= len.max(4) {
            self.echo_pos = 0;
        }
        (out[0], out[1])
    }
}
const RATES: [u32; 32] = [
    u32::MAX,
    2048,
    1536,
    1280,
    1024,
    768,
    640,
    512,
    384,
    320,
    256,
    192,
    160,
    128,
    96,
    80,
    64,
    48,
    40,
    32,
    24,
    20,
    16,
    12,
    10,
    8,
    6,
    5,
    4,
    3,
    2,
    1,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaussian_impulses_and_partial_overflow_match_hardware_coefficients() {
        // Independent vectors at phase zero: taps 370,1305,374,0 / 1024.
        assert_eq!(Dsp::interpolate([1024, 0, 0, 0], 0), 370);
        assert_eq!(Dsp::interpolate([0, 1024, 0, 0], 0), 1304);
        assert_eq!(Dsp::interpolate([0, 0, 1024, 0], 0), 374);
        assert_eq!(Dsp::interpolate([0, 0, 0, 1024], 0), 0);
        // The three-tap wrap is audible for the documented -8, range-12 case.
        assert_eq!(Dsp::interpolate([-16384; 4], 0), 32752);
        assert_eq!(Dsp::interpolate([1024, 0, 0, 0], 4080), 0);
        assert_eq!(Dsp::interpolate([0, 0, 0, 1024], 4080), 370);
    }

    #[test]
    fn brr_filter_zero_sign_range_and_loop_are_numerical() {
        let mut ram = vec![0; 65536];
        ram[0x300] = 0x93; // range 9, filter 0, end+loop
        ram[0x301..0x309].copy_from_slice(&[0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);
        let mut voice = Voice {
            addr: 0x300,
            loop_addr: 0x300,
            active: true,
            ..Voice::default()
        };
        for pass in 0..2 {
            for i in 0..16 {
                assert_eq!(Dsp::decode(&mut voice, &ram), pass == 1 && i == 0);
                let signed = if i < 8 { i } else { i - 16 };
                assert_eq!(voice.decoded[3], signed * 256);
            }
        }
        for shift in 13..=15 {
            ram[0x300] = shift << 4;
            ram[0x301] = 0x78;
            voice = Voice {
                addr: 0x300,
                active: true,
                ..Voice::default()
            };
            Dsp::decode(&mut voice, &ram);
            assert_eq!(voice.decoded[3], 0);
            Dsp::decode(&mut voice, &ram);
            assert_eq!(voice.decoded[3], -2048);
        }
    }

    #[test]
    fn direct_gain_stereo_key_delay_release_and_retrigger_have_expected_levels() {
        let mut ram = vec![0; 65536];
        ram[0x200..0x204].copy_from_slice(&[0, 3, 0, 3]);
        ram[0x300] = 0x93;
        ram[0x301..0x309].fill(0x77);
        let mut dsp = Dsp::default();
        for (r, v) in [
            (0x6c, 0x20),
            (0x5d, 2),
            (0x0c, 127),
            (0x1c, 127),
            (0, 127),
            (1, 0x80),
            (3, 0x10),
            (7, 127),
            (0x4c, 1),
        ] {
            dsp.write(r, v);
        }
        for _ in 0..5 {
            assert_eq!(dsp.sample(&mut ram), (0, 0));
        }
        for _ in 0..20 {
            dsp.sample(&mut ram);
        }
        // Constant decoded sample is 1792. Gaussian output=3584,
        // direct gain=2032/2048, then signed voice/master volumes /128.
        assert_eq!(dsp.sample(&mut ram), (3500, -3529));
        dsp.write(0x5c, 1);
        for i in 1..=254 {
            dsp.sample(&mut ram);
            assert_eq!(dsp.voices[0].env, 2032 - i * 8);
        }
        assert_eq!(dsp.sample(&mut ram), (0, 0));
        dsp.write(0x5c, 0);
        dsp.write(0x4c, 1);
        for _ in 0..5 {
            assert_eq!(dsp.sample(&mut ram), (0, 0));
        }
        for _ in 0..20 {
            dsp.sample(&mut ram);
        }
        assert_eq!(dsp.sample(&mut ram), (3500, -3529));
        dsp.write(0x6c, 0x60);
        assert_eq!(dsp.sample(&mut ram), (0, 0));
    }
}
