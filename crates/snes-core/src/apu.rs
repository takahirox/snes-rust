//! SPC700 interpreter and provisional BRR mixer. No Nintendo IPL bytes are embedded.
//! The documented IPL transfer protocol is implemented at the port boundary.
use crate::dsp::Dsp;
#[derive(Default, Debug, Clone)]
pub struct Spc {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub p: u8,
    pub pc: u16,
    pub halted: bool,
}
pub struct Apu {
    pub ram: Vec<u8>,
    pub cpu: Spc,
    pub dsp: Dsp,
    pub input: [u8; 4],
    pub output: [u8; 4],
    pub samples: Vec<f32>,
    pub error: Option<String>,
    pub trace_limit: usize,
    pub trace: std::collections::VecDeque<String>,
    pending: Option<(u16, u64)>,
    command: Option<(u8, u64)>,
    boot: bool,
    upload: bool,
    dest: u16,
    count: u8,
    clock: u64,
    cycles: i64,
    total: u64,
    timer_div: [u16; 3],
    timer_stage: [u16; 3],
    timer_out: [u8; 3],
    control: u8,
}
impl Default for Apu {
    fn default() -> Self {
        Self {
            ram: vec![0; 65536],
            cpu: Spc {
                sp: 0xef,
                ..Spc::default()
            },
            dsp: Dsp::default(),
            input: [0; 4],
            output: [0xaa, 0xbb, 0, 0],
            samples: Vec::new(),
            error: None,
            trace_limit: 0,
            trace: std::collections::VecDeque::new(),
            pending: None,
            command: None,
            boot: true,
            upload: false,
            dest: 0,
            count: 0,
            clock: 0,
            cycles: 0,
            total: 0,
            timer_div: [0; 3],
            timer_stage: [0; 3],
            timer_out: [0; 3],
            control: 0x80,
        }
    }
}
impl Apu {
    pub fn port_write(&mut self, port: usize, v: u8) {
        self.input[port] = v;
        if !self.boot || port != 0 {
            return;
        }
        self.command = Some((v, self.total + 4));
    }
    fn boot_command(&mut self, v: u8) {
        if !self.upload {
            if v != 0xcc {
                return;
            }
            self.begin(v);
        } else if v == self.count {
            self.ram[self.dest as usize] = self.input[1];
            self.dest = self.dest.wrapping_add(1);
            self.output[0] = v;
            self.count = self.count.wrapping_add(1);
        } else {
            self.begin(v);
        }
    }
    fn begin(&mut self, v: u8) {
        self.dest = u16::from_le_bytes([self.input[2], self.input[3]]);
        self.output[0] = v;
        if self.input[1] == 0 {
            self.pending = Some((self.dest, self.total + 32));
            self.cpu.p = 0;
            self.cycles = 0;
        } else {
            self.upload = true;
            self.count = 0;
        }
    }
    pub fn port_read(&mut self, port: usize) -> u8 {
        self.output[port]
    }
    pub fn tick(&mut self, master: u64) {
        self.clock += master * 1_024_000;
        let n = self.clock / 21_477_272;
        self.clock %= 21_477_272;
        self.cycles += n as i64;
        while self.cycles > 0 {
            let cycles = if self.boot || self.cpu.halted {
                2
            } else {
                self.step()
            };
            self.cycles -= cycles as i64;
            for _ in 0..cycles {
                self.total += 1;
                if self.boot {
                    if let Some((v, due)) = self.command {
                        if self.total >= due {
                            self.command = None;
                            self.boot_command(v);
                        }
                    }
                }
                if let Some((pc, due)) = self.pending {
                    if self.total >= due {
                        self.pending = None;
                        self.boot = false;
                        self.cpu.pc = pc;
                    }
                }
                for i in 0..3 {
                    if self.control & (1 << i) == 0 {
                        continue;
                    }
                    self.timer_div[i] += 1;
                    if self.timer_div[i] == if i == 2 { 16 } else { 128 } {
                        self.timer_div[i] = 0;
                        self.timer_stage[i] += 1;
                        let target = self.ram[0xfa + i] as u16;
                        let target = if target == 0 { 256 } else { target };
                        if self.timer_stage[i] >= target {
                            self.timer_stage[i] = 0;
                            self.timer_out[i] = (self.timer_out[i] + 1) & 15;
                        }
                    }
                }
                if self.total.is_multiple_of(32) {
                    let (l, r) = self.dsp.sample(&mut self.ram);
                    self.samples.push(l as f32 / 32768.0);
                    self.samples.push(r as f32 / 32768.0);
                }
            }
        }
    }
    pub fn read(&mut self, a: u16) -> u8 {
        match a {
            0xf3 => self.dsp.regs[(self.ram[0xf2] & 127) as usize],
            0xf4..=0xf7 => self.input[(a - 0xf4) as usize],
            0xfd..=0xff => {
                let i = (a - 0xfd) as usize;
                let v = self.timer_out[i];
                self.timer_out[i] = 0;
                v
            }
            _ => self.ram[a as usize],
        }
    }
    pub fn write(&mut self, a: u16, v: u8) {
        match a {
            0xf1 => {
                if let Some((pc, due)) = self.pending {
                    if self.total >= due {
                        self.pending = None;
                        self.boot = false;
                        self.cpu.pc = pc;
                    }
                }
                for i in 0..3 {
                    if v & (1 << i) != 0 && self.control & (1 << i) == 0 {
                        self.timer_div[i] = 0;
                        self.timer_stage[i] = 0;
                        self.timer_out[i] = 0;
                    }
                }
                if v & 16 != 0 {
                    self.input[0] = 0;
                    self.input[1] = 0;
                }
                if v & 32 != 0 {
                    self.input[2] = 0;
                    self.input[3] = 0;
                }
                self.control = v;
            }
            0xf3 => {
                if self.ram[0xf2] & 128 == 0 {
                    self.dsp.write(self.ram[0xf2], v);
                }
            }
            0xf4..=0xf7 => self.output[(a - 0xf4) as usize] = v,
            0xfd..=0xff => return,
            _ => {}
        }
        self.ram[a as usize] = v;
    }
    fn fetch(&mut self) -> u8 {
        let a = self.cpu.pc;
        self.cpu.pc = self.cpu.pc.wrapping_add(1);
        self.read(a)
    }
    fn word(&mut self) -> u16 {
        let l = self.fetch();
        u16::from_le_bytes([l, self.fetch()])
    }
    fn dp(&self, a: u8) -> u16 {
        a as u16 | if self.cpu.p & 32 != 0 { 256 } else { 0 }
    }
    fn pointer(&mut self, a: u8) -> u16 {
        let l = self.read(self.dp(a));
        u16::from_le_bytes([l, self.read(self.dp(a.wrapping_add(1)))])
    }
    fn nz(&mut self, v: u8) {
        self.cpu.p = (self.cpu.p & !0x82) | if v == 0 { 2 } else { 0 } | (v & 128);
    }
    fn nz16(&mut self, v: u16) {
        self.cpu.p = (self.cpu.p & !0x82) | if v == 0 { 2 } else { 0 } | ((v >> 8) as u8 & 128);
    }
    fn flag(&mut self, f: u8, on: bool) {
        if on {
            self.cpu.p |= f;
        } else {
            self.cpu.p &= !f;
        }
    }
    fn push(&mut self, v: u8) {
        self.write(256 | self.cpu.sp as u16, v);
        self.cpu.sp = self.cpu.sp.wrapping_sub(1);
    }
    fn pop(&mut self) -> u8 {
        self.cpu.sp = self.cpu.sp.wrapping_add(1);
        self.read(256 | self.cpu.sp as u16)
    }
    fn call(&mut self, a: u16) {
        self.push((self.cpu.pc >> 8) as u8);
        self.push(self.cpu.pc as u8);
        self.cpu.pc = a;
    }
    fn branch(&mut self, take: bool) -> u32 {
        let d = self.fetch() as i8;
        if take {
            self.cpu.pc = self.cpu.pc.wrapping_add_signed(d as i16);
            2
        } else {
            0
        }
    }
    fn arithmetic(&mut self, kind: u8, a: u8, v: u8) -> u8 {
        let carry = u16::from(self.cpu.p & 1 != 0);
        match kind {
            0 => {
                let r = a | v;
                self.nz(r);
                r
            }
            1 => {
                let r = a & v;
                self.nz(r);
                r
            }
            2 => {
                let r = a ^ v;
                self.nz(r);
                r
            }
            3 => {
                let r = a.wrapping_sub(v);
                self.flag(1, a >= v);
                self.nz(r);
                a
            }
            4 | 5 => {
                let sub = kind == 5;
                let n = if sub { v ^ 255 } else { v };
                let sum = a as u16 + n as u16 + carry;
                self.flag(1, sum > 255);
                self.flag(8, (a & 15) as u16 + (n & 15) as u16 + carry > 15);
                self.flag(64, (!(a ^ n)) & (a ^ sum as u8) & 128 != 0);
                let r = sum as u8;
                self.nz(r);
                r
            }
            _ => unreachable!(),
        }
    }
    fn operand(&mut self, op: u8) -> u16 {
        match op & 31 {
            4 => {
                let a = self.fetch();
                self.dp(a)
            }
            5 => self.word(),
            6 => self.dp(self.cpu.x),
            7 => {
                let a = self.fetch().wrapping_add(self.cpu.x);
                self.pointer(a)
            }
            20 => {
                let a = self.fetch().wrapping_add(self.cpu.x);
                self.dp(a)
            }
            21 => {
                let a = self.word();
                a.wrapping_add(self.cpu.x as u16)
            }
            22 => {
                let a = self.word();
                a.wrapping_add(self.cpu.y as u16)
            }
            23 => {
                let a = self.fetch();
                self.pointer(a).wrapping_add(self.cpu.y as u16)
            }
            _ => unreachable!(),
        }
    }
    /// Execute a single instruction; exposed for deterministic CPU fixtures.
    pub fn step(&mut self) -> u32 {
        let pc = self.cpu.pc;
        if self.control & 128 != 0 && pc == 0xffc0 {
            self.boot = true;
            self.upload = false;
            self.pending = None;
            self.output[0] = 0xaa;
            self.output[1] = 0xbb;
            return 2;
        }
        let op = self.fetch();
        if self.trace_limit > 0 {
            if self.trace.len() >= self.trace_limit {
                self.trace.pop_front();
            }
            self.trace.push_back(format!(
                "{pc:04x} {op:02x} A={:02x} X={:02x} Y={:02x} S={:02x} P={:02x}",
                self.cpu.a, self.cpu.x, self.cpu.y, self.cpu.sp, self.cpu.p
            ));
        }
        let mut cycles = SPC_CYCLES[op as usize] as u32;
        if op & 15 == 1 {
            let vector = 0xffde - (op as u16 >> 4) * 2;
            let l = self.read(vector);
            let h = self.read(vector + 1);
            self.call(u16::from_le_bytes([l, h]));
            return cycles;
        }
        if op & 15 == 2 {
            let a = self.fetch();
            let a = self.dp(a);
            let v = self.read(a);
            let bit = 1 << (op >> 5);
            self.write(a, if op & 16 == 0 { v | bit } else { v & !bit });
            return cycles;
        }
        if op & 15 == 3 {
            let a = self.fetch();
            let v = self.read(self.dp(a));
            let bit = v & (1 << (op >> 5)) != 0;
            cycles += self.branch(bit == (op & 16 == 0));
            return cycles;
        }
        if op & 31 == 16 {
            let take = match op >> 5 {
                0 => self.cpu.p & 128 == 0,
                1 => self.cpu.p & 128 != 0,
                2 => self.cpu.p & 64 == 0,
                3 => self.cpu.p & 64 != 0,
                4 => self.cpu.p & 1 == 0,
                5 => self.cpu.p & 1 != 0,
                6 => self.cpu.p & 2 == 0,
                _ => self.cpu.p & 2 != 0,
            };
            cycles += self.branch(take);
            return cycles;
        }
        // Six ALU families use the same operand encoding.
        if op < 0xc0 && matches!(op&31,4..=9|20..=25) {
            let kind = op >> 5;
            match op & 31 {
                8 => {
                    let v = self.fetch();
                    self.cpu.a = self.arithmetic(kind, self.cpu.a, v);
                }
                9 => {
                    let src = self.fetch();
                    let dst = self.fetch();
                    let v = self.read(self.dp(src));
                    let a = self.read(self.dp(dst));
                    let r = self.arithmetic(kind, a, v);
                    if kind != 3 {
                        self.write(self.dp(dst), r);
                    }
                }
                24 => {
                    let v = self.fetch();
                    let dst = self.fetch();
                    let a = self.read(self.dp(dst));
                    let r = self.arithmetic(kind, a, v);
                    if kind != 3 {
                        self.write(self.dp(dst), r);
                    }
                }
                25 => {
                    let a = self.read(self.dp(self.cpu.x));
                    let v = self.read(self.dp(self.cpu.y));
                    let r = self.arithmetic(kind, a, v);
                    if kind != 3 {
                        self.write(self.dp(self.cpu.x), r);
                    }
                }
                _ => {
                    let address = self.operand(op);
                    let v = self.read(address);
                    self.cpu.a = self.arithmetic(kind, self.cpu.a, v);
                }
            }
            return cycles;
        }
        // Shift/rotate and byte increment/decrement families.
        if op < 0xc0 && matches!(op & 31, 11 | 12 | 27 | 28) {
            let kind = op >> 5;
            let register = op & 31 == 28;
            let address = if register {
                0
            } else if op & 31 == 12 {
                self.word()
            } else {
                let a = self.fetch();
                self.dp(if op & 31 == 27 {
                    a.wrapping_add(self.cpu.x)
                } else {
                    a
                })
            };
            let v = if register {
                self.cpu.a
            } else {
                self.read(address)
            };
            let carry = self.cpu.p & 1;
            let r = match kind {
                0 => {
                    self.flag(1, v & 128 != 0);
                    v << 1
                }
                1 => {
                    self.flag(1, v & 128 != 0);
                    (v << 1) | carry
                }
                2 => {
                    self.flag(1, v & 1 != 0);
                    v >> 1
                }
                3 => {
                    self.flag(1, v & 1 != 0);
                    (v >> 1) | (carry << 7)
                }
                4 => v.wrapping_sub(1),
                _ => v.wrapping_add(1),
            };
            self.nz(r);
            if register {
                self.cpu.a = r;
            } else {
                self.write(address, r);
            }
            return cycles;
        }
        match op {
            0x00 => {}
            0x20 => self.cpu.p &= !32,
            0x40 => self.cpu.p |= 32,
            0x60 => self.cpu.p &= !1,
            0x80 => self.cpu.p |= 1,
            0xa0 => self.cpu.p |= 4,
            0xc0 => self.cpu.p &= !4,
            0xe0 => self.cpu.p &= !0x48,
            0xed => self.cpu.p ^= 1,
            0x0d => self.push(self.cpu.p),
            0x2d => self.push(self.cpu.a),
            0x4d => self.push(self.cpu.x),
            0x6d => self.push(self.cpu.y),
            0x8e => self.cpu.p = self.pop(),
            0xae => {
                self.cpu.a = self.pop();
            }
            0xce => {
                self.cpu.x = self.pop();
            }
            0xee => {
                self.cpu.y = self.pop();
            }
            0x0f => {
                self.call(0);
                self.push(self.cpu.p);
                self.cpu.p = (self.cpu.p | 16) & !4;
                let l = self.read(0xffde);
                self.cpu.pc = u16::from_le_bytes([l, self.read(0xffdf)]);
            }
            0x2f => {
                cycles += self.branch(true) - 2;
            }
            0x3f => {
                let a = self.word();
                self.call(a);
            }
            0x4f => {
                let a = self.fetch();
                self.call(0xff00 | a as u16);
            }
            0x5f => self.cpu.pc = self.word(),
            0x1f => {
                let a = self.word().wrapping_add(self.cpu.x as u16);
                let l = self.read(a);
                self.cpu.pc = u16::from_le_bytes([l, self.read(a.wrapping_add(1))]);
            }
            0x6f => {
                let l = self.pop();
                self.cpu.pc = u16::from_le_bytes([l, self.pop()]);
            }
            0x7f => {
                self.cpu.p = self.pop();
                let l = self.pop();
                self.cpu.pc = u16::from_le_bytes([l, self.pop()]);
            }
            0x2e | 0xde => {
                let a = self.fetch();
                let a = if op == 0xde {
                    a.wrapping_add(self.cpu.x)
                } else {
                    a
                };
                let v = self.read(self.dp(a));
                cycles += self.branch(self.cpu.a != v);
            }
            0x6e => {
                let a = self.fetch();
                let v = self.read(self.dp(a)).wrapping_sub(1);
                self.write(self.dp(a), v);
                cycles += self.branch(v != 0);
            }
            0xfe => {
                self.cpu.y = self.cpu.y.wrapping_sub(1);
                cycles += self.branch(self.cpu.y != 0);
            }
            0x0e | 0x4e => {
                let a = self.word();
                let v = self.read(a);
                self.nz(self.cpu.a.wrapping_sub(v));
                self.write(
                    a,
                    if op == 0x0e {
                        v | self.cpu.a
                    } else {
                        v & !self.cpu.a
                    },
                );
            }
            0x0a | 0x2a | 0x4a | 0x6a | 0x8a | 0xaa | 0xca | 0xea => {
                let code = self.word();
                let a = code & 0x1fff;
                let mask = 1 << (code >> 13);
                let v = self.read(a);
                let bit = v & mask != 0;
                match op {
                    0x0a => self.flag(1, self.cpu.p & 1 != 0 || bit),
                    0x2a => self.flag(1, self.cpu.p & 1 != 0 || !bit),
                    0x4a => self.flag(1, self.cpu.p & 1 != 0 && bit),
                    0x6a => self.flag(1, self.cpu.p & 1 != 0 && !bit),
                    0x8a => self.flag(1, (self.cpu.p & 1 != 0) ^ bit),
                    0xaa => self.flag(1, bit),
                    0xca => self.write(a, (v & !mask) | if self.cpu.p & 1 != 0 { mask } else { 0 }),
                    _ => self.write(a, v ^ mask),
                }
            }
            0x1a | 0x3a | 0x5a | 0x7a | 0x9a | 0xba | 0xda => {
                let a = self.fetch();
                let lo = self.read(self.dp(a));
                let hi = self.read(self.dp(a.wrapping_add(1)));
                let v = u16::from_le_bytes([lo, hi]);
                let ya = u16::from_le_bytes([self.cpu.a, self.cpu.y]);
                match op {
                    0x1a | 0x3a => {
                        let r = v.wrapping_add(if op == 0x1a { 65535 } else { 1 });
                        self.write(self.dp(a), r as u8);
                        self.write(self.dp(a.wrapping_add(1)), (r >> 8) as u8);
                        self.nz16(r);
                    }
                    0x5a => {
                        self.flag(1, ya >= v);
                        self.nz16(ya.wrapping_sub(v));
                    }
                    0x7a | 0x9a => {
                        let sub = op == 0x9a;
                        let n = if sub { v ^ 65535 } else { v };
                        let r = ya as u32 + n as u32 + u32::from(sub);
                        self.flag(1, r > 65535);
                        self.flag(
                            8,
                            (ya & 0xfff) as u32 + (n & 0xfff) as u32 + u32::from(sub) > 0xfff,
                        );
                        self.flag(64, (!(ya ^ n)) & (ya ^ r as u16) & 0x8000 != 0);
                        self.cpu.a = r as u8;
                        self.cpu.y = (r >> 8) as u8;
                        self.nz16(r as u16);
                    }
                    0xba => {
                        self.cpu.a = lo;
                        self.cpu.y = hi;
                        self.nz16(v);
                    }
                    _ => {
                        self.write(self.dp(a), self.cpu.a);
                        self.write(self.dp(a.wrapping_add(1)), self.cpu.y);
                    }
                }
            }
            0x1d | 0x3d => {
                self.cpu.x = self.cpu.x.wrapping_add(if op == 0x1d { 255 } else { 1 });
                self.nz(self.cpu.x);
            }
            0xdc | 0xfc => {
                self.cpu.y = self.cpu.y.wrapping_add(if op == 0xdc { 255 } else { 1 });
                self.nz(self.cpu.y);
            }
            0x5d => {
                self.cpu.x = self.cpu.a;
                self.nz(self.cpu.x);
            }
            0x7d => {
                self.cpu.a = self.cpu.x;
                self.nz(self.cpu.a);
            }
            0x9d => {
                self.cpu.x = self.cpu.sp;
                self.nz(self.cpu.x);
            }
            0xbd => self.cpu.sp = self.cpu.x,
            0xdd => {
                self.cpu.a = self.cpu.y;
                self.nz(self.cpu.a);
            }
            0xfd => {
                self.cpu.y = self.cpu.a;
                self.nz(self.cpu.y);
            }
            0xe8 => {
                self.cpu.a = self.fetch();
                self.nz(self.cpu.a);
            }
            0xcd => {
                self.cpu.x = self.fetch();
                self.nz(self.cpu.x);
            }
            0x8d => {
                self.cpu.y = self.fetch();
                self.nz(self.cpu.y);
            }
            0x8f => {
                let v = self.fetch();
                let a = self.fetch();
                self.write(self.dp(a), v);
            }
            0xfa => {
                let a = self.fetch();
                let v = self.read(self.dp(a));
                let a = self.fetch();
                self.write(self.dp(a), v);
            }
            0xe4..=0xe7 | 0xf4..=0xf7 => {
                let a = self.operand(op);
                self.cpu.a = self.read(a);
                self.nz(self.cpu.a);
            }
            0xc4..=0xc7 | 0xd4..=0xd7 => {
                let a = self.operand(op);
                self.write(a, self.cpu.a);
            }
            0xaf => {
                self.write(self.dp(self.cpu.x), self.cpu.a);
                self.cpu.x = self.cpu.x.wrapping_add(1);
            }
            0xbf => {
                self.cpu.a = self.read(self.dp(self.cpu.x));
                self.cpu.x = self.cpu.x.wrapping_add(1);
                self.nz(self.cpu.a);
            }
            0xf8 | 0xf9 | 0xe9 | 0xeb | 0xec | 0xfb => {
                let a = if op == 0xe9 || op == 0xec {
                    self.word()
                } else {
                    let v = self.fetch();
                    self.dp(v.wrapping_add(if op == 0xf9 {
                        self.cpu.y
                    } else if op == 0xfb {
                        self.cpu.x
                    } else {
                        0
                    }))
                };
                let v = self.read(a);
                if op == 0xf8 || op == 0xf9 || op == 0xe9 {
                    self.cpu.x = v;
                } else {
                    self.cpu.y = v;
                }
                self.nz(v);
            }
            0xd8 | 0xd9 | 0xc9 | 0xcb | 0xcc | 0xdb => {
                let a = if op == 0xc9 || op == 0xcc {
                    self.word()
                } else {
                    let v = self.fetch();
                    self.dp(v.wrapping_add(if op == 0xd9 {
                        self.cpu.y
                    } else if op == 0xdb {
                        self.cpu.x
                    } else {
                        0
                    }))
                };
                let v = if op == 0xd8 || op == 0xd9 || op == 0xc9 {
                    self.cpu.x
                } else {
                    self.cpu.y
                };
                self.write(a, v);
            }
            0xc8 | 0x3e | 0x1e | 0xad | 0x7e | 0x5e => {
                let v = if op == 0xc8 || op == 0xad {
                    self.fetch()
                } else if op == 0x1e || op == 0x5e {
                    let a = self.word();
                    self.read(a)
                } else {
                    let a = self.fetch();
                    self.read(self.dp(a))
                };
                self.arithmetic(
                    3,
                    if op == 0xc8 || op == 0x3e || op == 0x1e {
                        self.cpu.x
                    } else {
                        self.cpu.y
                    },
                    v,
                );
            }
            0xcf => {
                let r = self.cpu.a as u16 * self.cpu.y as u16;
                self.cpu.a = r as u8;
                self.cpu.y = (r >> 8) as u8;
                self.nz(self.cpu.y);
            }
            0x9e => {
                let a = self.cpu.a as u16;
                let y = self.cpu.y as u16;
                let x = self.cpu.x as u16;
                let ya = (y << 8) | a;
                self.flag(8, y & 15 >= x & 15);
                self.flag(64, y >= x);
                if y < x * 2 && x != 0 {
                    self.cpu.a = (ya / x) as u8;
                    self.cpu.y = (ya % x) as u8;
                } else {
                    self.cpu.a = (255 - (ya.wrapping_sub(x << 9)) / (256 - x)) as u8;
                    self.cpu.y = (x + (ya.wrapping_sub(x << 9)) % (256 - x)) as u8;
                }
                self.nz(self.cpu.a);
            }
            0x9f => {
                self.cpu.a = self.cpu.a.rotate_left(4);
                self.nz(self.cpu.a);
            }
            0xdf => {
                let mut a = self.cpu.a as u16;
                if a > 0x99 || self.cpu.p & 1 != 0 {
                    a += 0x60;
                    self.cpu.p |= 1;
                }
                if a & 15 > 9 || self.cpu.p & 8 != 0 {
                    a += 6;
                }
                self.cpu.a = a as u8;
                self.nz(self.cpu.a);
            }
            0xbe => {
                let mut a = self.cpu.a;
                if a > 0x99 || self.cpu.p & 1 == 0 {
                    a = a.wrapping_sub(0x60);
                    self.cpu.p &= !1;
                }
                if a & 15 > 9 || self.cpu.p & 8 == 0 {
                    a = a.wrapping_sub(6);
                }
                self.cpu.a = a;
                self.nz(a);
            }
            0xef | 0xff => self.cpu.halted = true,
            _ => {
                self.error = Some(format!(
                    "Unimplemented SPC700 opcode ${op:02x} at ${pc:04x}"
                ));
                self.cpu.halted = true;
            }
        }
        cycles
    }
}
const SPC_CYCLES: [u8; 256] = [
    2, 8, 4, 5, 3, 4, 3, 6, 2, 6, 5, 4, 5, 4, 6, 8, 2, 8, 4, 5, 4, 5, 5, 6, 5, 5, 6, 5, 2, 2, 4, 6,
    2, 8, 4, 5, 3, 4, 3, 6, 2, 6, 5, 4, 5, 4, 5, 4, 2, 8, 4, 5, 4, 5, 5, 6, 5, 5, 6, 5, 2, 2, 3, 8,
    2, 8, 4, 5, 3, 4, 3, 6, 2, 6, 4, 4, 5, 4, 6, 6, 2, 8, 4, 5, 4, 5, 5, 6, 5, 5, 4, 5, 2, 2, 4, 3,
    2, 8, 4, 5, 3, 4, 3, 6, 2, 6, 4, 4, 5, 4, 5, 5, 2, 8, 4, 5, 4, 5, 5, 6, 5, 5, 5, 5, 2, 2, 3, 6,
    2, 8, 4, 5, 3, 4, 3, 6, 2, 6, 5, 4, 5, 2, 4, 5, 2, 8, 4, 5, 4, 5, 5, 6, 5, 5, 5, 5, 2, 2, 12,
    5, 3, 8, 4, 5, 3, 4, 3, 6, 2, 6, 4, 4, 5, 2, 4, 4, 2, 8, 4, 5, 4, 5, 5, 6, 5, 5, 5, 5, 2, 2, 3,
    4, 3, 8, 4, 5, 4, 5, 4, 7, 2, 5, 6, 4, 5, 2, 4, 9, 2, 8, 4, 5, 5, 6, 6, 7, 4, 5, 5, 5, 2, 2, 6,
    3, 2, 8, 4, 5, 3, 4, 3, 6, 2, 4, 5, 3, 4, 3, 4, 3, 2, 8, 4, 5, 4, 5, 5, 6, 3, 4, 5, 4, 2, 2, 4,
    3,
];

#[cfg(test)]
mod timing_tests {
    use super::*;

    #[test]
    fn sample_clock_is_32khz_and_independent_of_cpu_tick_partition() {
        fn configured() -> Apu {
            let mut a = Apu::default();
            a.ram[0x200..0x204].copy_from_slice(&[0, 3, 0, 3]);
            a.ram[0x300] = 0x93;
            a.ram[0x301..0x309].fill(0x77);
            for (r, v) in [
                (0x6c, 0x20),
                (0x5d, 2),
                (0x0c, 127),
                (0x1c, 127),
                (0, 127),
                (1, 127),
                (3, 0x10),
                (7, 127),
                (0x4c, 1),
            ] {
                a.dsp.write(r, v);
            }
            a.write(0xfa, 125);
            a.write(0xf1, 1);
            a
        }
        let mut whole = configured();
        let mut split = configured();
        whole.tick(21477272);
        for _ in 0..21477 {
            split.tick(1000);
        }
        split.tick(272);
        assert_eq!(whole.samples.len(), 64000);
        assert_eq!(whole.samples, split.samples);
        assert!(whole
            .samples
            .iter()
            .all(|s| s.is_finite() && s.abs() <= 1.0));
        assert_eq!(*whole.samples.last().unwrap(), 3500.0 / 32768.0);
        // 1.024 MHz / (128 * 125) = 64 ticks per second, modulo 16.
        assert_eq!(whole.read(0xfd), 0);
        whole.tick(335582); // Another 1/64 s, within one SPC cycle.
        assert_eq!(whole.read(0xfd), 1);
        assert_eq!(whole.read(0xfd), 0);
    }
}
