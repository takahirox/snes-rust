//! 65C816 interpreter, authored from instruction-set documentation.
//! Bus operations are timed by the machine. Internal-cycle accounting is approximate.
pub trait Bus {
    fn read(&mut self, a: u32) -> u8;
    fn write(&mut self, a: u32, v: u8);
    fn idle(&mut self);
}
pub const C: u8 = 1;
pub const Z: u8 = 2;
pub const I: u8 = 4;
pub const D: u8 = 8;
pub const X: u8 = 16;
pub const M: u8 = 32;
pub const V: u8 = 64;
pub const N: u8 = 128;
#[derive(Debug, Clone)]
pub struct Cpu {
    pub a: u16,
    pub x: u16,
    pub y: u16,
    pub s: u16,
    pub d: u16,
    pub pc: u16,
    pub pb: u8,
    pub db: u8,
    pub p: u8,
    pub e: bool,
    pub waiting: bool,
    pub stopped: bool,
    pub instructions: u64,
}
impl Default for Cpu {
    fn default() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            s: 0x1ff,
            d: 0,
            pc: 0,
            pb: 0,
            db: 0,
            p: 0x34,
            e: true,
            waiting: false,
            stopped: false,
            instructions: 0,
        }
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Imp,
    Acc,
    Imm,
    ImmX,
    Dp,
    Dpx,
    Dpy,
    Abs,
    Abx,
    Aby,
    Long,
    Lgx,
    Ind,
    Idx,
    Idy,
    Idl,
    Ily,
    Sr,
    Siy,
    Rel,
    Rll,
}
use Mode::*;
// One entry per opcode. Width-sensitive immediate operands are resolved at execution.
const OPS: [(&str, Mode); 256] = [
    ("BRK", Imm),
    ("ORA", Idx),
    ("COP", Imm),
    ("ORA", Sr),
    ("TSB", Dp),
    ("ORA", Dp),
    ("ASL", Dp),
    ("ORA", Idl),
    ("PHP", Imp),
    ("ORA", Imm),
    ("ASL", Acc),
    ("PHD", Imp),
    ("TSB", Abs),
    ("ORA", Abs),
    ("ASL", Abs),
    ("ORA", Long),
    ("BPL", Rel),
    ("ORA", Idy),
    ("ORA", Ind),
    ("ORA", Siy),
    ("TRB", Dp),
    ("ORA", Dpx),
    ("ASL", Dpx),
    ("ORA", Ily),
    ("CLC", Imp),
    ("ORA", Aby),
    ("INC", Acc),
    ("TCS", Imp),
    ("TRB", Abs),
    ("ORA", Abx),
    ("ASL", Abx),
    ("ORA", Lgx),
    ("JSR", Abs),
    ("AND", Idx),
    ("JSL", Long),
    ("AND", Sr),
    ("BIT", Dp),
    ("AND", Dp),
    ("ROL", Dp),
    ("AND", Idl),
    ("PLP", Imp),
    ("AND", Imm),
    ("ROL", Acc),
    ("PLD", Imp),
    ("BIT", Abs),
    ("AND", Abs),
    ("ROL", Abs),
    ("AND", Long),
    ("BMI", Rel),
    ("AND", Idy),
    ("AND", Ind),
    ("AND", Siy),
    ("BIT", Dpx),
    ("AND", Dpx),
    ("ROL", Dpx),
    ("AND", Ily),
    ("SEC", Imp),
    ("AND", Aby),
    ("DEC", Acc),
    ("TSC", Imp),
    ("BIT", Abx),
    ("AND", Abx),
    ("ROL", Abx),
    ("AND", Lgx),
    ("RTI", Imp),
    ("EOR", Idx),
    ("WDM", Imm),
    ("EOR", Sr),
    ("MVP", Imp),
    ("EOR", Dp),
    ("LSR", Dp),
    ("EOR", Idl),
    ("PHA", Imp),
    ("EOR", Imm),
    ("LSR", Acc),
    ("PHK", Imp),
    ("JMP", Abs),
    ("EOR", Abs),
    ("LSR", Abs),
    ("EOR", Long),
    ("BVC", Rel),
    ("EOR", Idy),
    ("EOR", Ind),
    ("EOR", Siy),
    ("MVN", Imp),
    ("EOR", Dpx),
    ("LSR", Dpx),
    ("EOR", Ily),
    ("CLI", Imp),
    ("EOR", Aby),
    ("PHY", Imp),
    ("TCD", Imp),
    ("JML", Long),
    ("EOR", Abx),
    ("LSR", Abx),
    ("EOR", Lgx),
    ("RTS", Imp),
    ("ADC", Idx),
    ("PER", Rll),
    ("ADC", Sr),
    ("STZ", Dp),
    ("ADC", Dp),
    ("ROR", Dp),
    ("ADC", Idl),
    ("PLA", Imp),
    ("ADC", Imm),
    ("ROR", Acc),
    ("RTL", Imp),
    ("JMI", Abs),
    ("ADC", Abs),
    ("ROR", Abs),
    ("ADC", Long),
    ("BVS", Rel),
    ("ADC", Idy),
    ("ADC", Ind),
    ("ADC", Siy),
    ("STZ", Dpx),
    ("ADC", Dpx),
    ("ROR", Dpx),
    ("ADC", Ily),
    ("SEI", Imp),
    ("ADC", Aby),
    ("PLY", Imp),
    ("TDC", Imp),
    ("JXI", Abx),
    ("ADC", Abx),
    ("ROR", Abx),
    ("ADC", Lgx),
    ("BRA", Rel),
    ("STA", Idx),
    ("BRL", Rll),
    ("STA", Sr),
    ("STY", Dp),
    ("STA", Dp),
    ("STX", Dp),
    ("STA", Idl),
    ("DEY", Imp),
    ("BIT", Imm),
    ("TXA", Imp),
    ("PHB", Imp),
    ("STY", Abs),
    ("STA", Abs),
    ("STX", Abs),
    ("STA", Long),
    ("BCC", Rel),
    ("STA", Idy),
    ("STA", Ind),
    ("STA", Siy),
    ("STY", Dpx),
    ("STA", Dpx),
    ("STX", Dpy),
    ("STA", Ily),
    ("TYA", Imp),
    ("STA", Aby),
    ("TXS", Imp),
    ("TXY", Imp),
    ("STZ", Abs),
    ("STA", Abx),
    ("STZ", Abx),
    ("STA", Lgx),
    ("LDY", ImmX),
    ("LDA", Idx),
    ("LDX", ImmX),
    ("LDA", Sr),
    ("LDY", Dp),
    ("LDA", Dp),
    ("LDX", Dp),
    ("LDA", Idl),
    ("TAY", Imp),
    ("LDA", Imm),
    ("TAX", Imp),
    ("PLB", Imp),
    ("LDY", Abs),
    ("LDA", Abs),
    ("LDX", Abs),
    ("LDA", Long),
    ("BCS", Rel),
    ("LDA", Idy),
    ("LDA", Ind),
    ("LDA", Siy),
    ("LDY", Dpx),
    ("LDA", Dpx),
    ("LDX", Dpy),
    ("LDA", Ily),
    ("CLV", Imp),
    ("LDA", Aby),
    ("TSX", Imp),
    ("TYX", Imp),
    ("LDY", Abx),
    ("LDA", Abx),
    ("LDX", Aby),
    ("LDA", Lgx),
    ("CPY", ImmX),
    ("CMP", Idx),
    ("REP", Imm),
    ("CMP", Sr),
    ("CPY", Dp),
    ("CMP", Dp),
    ("DEC", Dp),
    ("CMP", Idl),
    ("INY", Imp),
    ("CMP", Imm),
    ("DEX", Imp),
    ("WAI", Imp),
    ("CPY", Abs),
    ("CMP", Abs),
    ("DEC", Abs),
    ("CMP", Long),
    ("BNE", Rel),
    ("CMP", Idy),
    ("CMP", Ind),
    ("CMP", Siy),
    ("PEI", Dp),
    ("CMP", Dpx),
    ("DEC", Dpx),
    ("CMP", Ily),
    ("CLD", Imp),
    ("CMP", Aby),
    ("PHX", Imp),
    ("STP", Imp),
    ("JLI", Abs),
    ("CMP", Abx),
    ("DEC", Abx),
    ("CMP", Lgx),
    ("CPX", ImmX),
    ("SBC", Idx),
    ("SEP", Imm),
    ("SBC", Sr),
    ("CPX", Dp),
    ("SBC", Dp),
    ("INC", Dp),
    ("SBC", Idl),
    ("INX", Imp),
    ("SBC", Imm),
    ("NOP", Imp),
    ("XBA", Imp),
    ("CPX", Abs),
    ("SBC", Abs),
    ("INC", Abs),
    ("SBC", Long),
    ("BEQ", Rel),
    ("SBC", Idy),
    ("SBC", Ind),
    ("SBC", Siy),
    ("PEA", Abs),
    ("SBC", Dpx),
    ("INC", Dpx),
    ("SBC", Ily),
    ("SED", Imp),
    ("SBC", Aby),
    ("PLX", Imp),
    ("XCE", Imp),
    ("JSI", Abx),
    ("SBC", Abx),
    ("INC", Abx),
    ("SBC", Lgx),
];
impl Cpu {
    fn flag(&self, f: u8) -> bool {
        self.p & f != 0
    }
    fn set(&mut self, f: u8, on: bool) {
        if on {
            self.p |= f;
        } else {
            self.p &= !f;
        }
    }
    fn nz(&mut self, v: u16, byte: bool) {
        self.set(Z, if byte { v & 255 == 0 } else { v == 0 });
        self.set(N, v & if byte { 128 } else { 32768 } != 0);
    }
    fn status(&mut self, p: u8) {
        self.p = p;
        if self.e {
            self.p |= M | X;
            self.s = 0x100 | (self.s & 255);
        }
        if self.flag(X) {
            self.x &= 255;
            self.y &= 255;
        }
    }
    fn fetch(&mut self, b: &mut impl Bus) -> u8 {
        let v = b.read(((self.pb as u32) << 16) | self.pc as u32);
        self.pc = self.pc.wrapping_add(1);
        v
    }
    fn word(&mut self, b: &mut impl Bus) -> u16 {
        let l = self.fetch(b);
        u16::from_le_bytes([l, self.fetch(b)])
    }
    fn rd16(b: &mut impl Bus, a: u32) -> u16 {
        let l = b.read(a);
        u16::from_le_bytes([l, b.read((a + 1) & 0xffffff)])
    }
    fn rd16_in_bank(b: &mut impl Bus, a: u32) -> u16 {
        let next = (a & 0xff0000) | (a as u16).wrapping_add(1) as u32;
        u16::from_le_bytes([b.read(a), b.read(next)])
    }
    fn next_data_addr(mode: Mode, a: u32) -> u32 {
        // Direct and stack-relative operands stay in bank zero; resolved
        // indirect, absolute and long operands can carry into the next bank.
        if matches!(mode, Dp | Dpx | Dpy | Sr) {
            (a as u16).wrapping_add(1) as u32
        } else {
            (a + 1) & 0xffffff
        }
    }
    fn dpword(&self, b: &mut impl Bus, a: u16) -> u16 {
        let next = if self.e && self.d & 255 == 0 {
            (a & 0xff00) | (a.wrapping_add(1) & 255)
        } else {
            a.wrapping_add(1)
        };
        u16::from_le_bytes([b.read(a as u32), b.read(next as u32)])
    }
    fn push(&mut self, b: &mut impl Bus, v: u8) {
        b.write(self.s as u32, v);
        self.s = self.s.wrapping_sub(1);
        if self.e {
            self.s = 0x100 | (self.s & 255);
        }
    }
    fn pop(&mut self, b: &mut impl Bus) -> u8 {
        self.s = self.s.wrapping_add(1);
        if self.e {
            self.s = 0x100 | (self.s & 255);
        }
        b.read(self.s as u32)
    }
    fn push16(&mut self, b: &mut impl Bus, v: u16) {
        self.push(b, (v >> 8) as u8);
        self.push(b, v as u8);
    }
    fn pop16(&mut self, b: &mut impl Bus) -> u16 {
        let l = self.pop(b);
        u16::from_le_bytes([l, self.pop(b)])
    }
    // Native-only instructions use the full stack pointer even when E is set,
    // then restore page one after the entire instruction.
    fn push16_full(&mut self, b: &mut impl Bus, v: u16) {
        let e = self.e;
        self.e = false;
        self.push16(b, v);
        self.e = e;
        if e {
            self.s = 0x100 | (self.s & 255);
        }
    }
    fn pop16_full(&mut self, b: &mut impl Bus) -> u16 {
        let e = self.e;
        self.e = false;
        let v = self.pop16(b);
        self.e = e;
        if e {
            self.s = 0x100 | (self.s & 255);
        }
        v
    }
    pub fn reset(&mut self, b: &mut impl Bus) {
        *self = Self::default();
        self.pc = Self::rd16(b, 0xfffc);
    }
    pub fn interrupt(&mut self, b: &mut impl Bus, nmi: bool) {
        if self.stopped {
            return;
        }
        self.waiting = false;
        if !nmi && self.flag(I) {
            return;
        }
        // Hardware entry discards an opcode fetch without advancing PC and
        // takes an internal cycle. BRK/COP instead fetch opcode + signature.
        b.read(((self.pb as u32) << 16) | self.pc as u32);
        b.idle();
        self.enter(
            b,
            if nmi {
                if self.e {
                    0xfffa
                } else {
                    0xffea
                }
            } else if self.e {
                0xfffe
            } else {
                0xffee
            },
            false,
        );
    }
    fn enter(&mut self, b: &mut impl Bus, vector: u32, software: bool) {
        if !self.e {
            self.push(b, self.pb);
        }
        self.push16(b, self.pc);
        self.push(
            b,
            if self.e {
                (self.p & !16) | if software { 16 } else { 0 }
            } else {
                self.p
            },
        );
        self.p = (self.p | I) & !D;
        self.pb = 0;
        self.pc = Self::rd16(b, vector);
    }
    fn addr(&mut self, b: &mut impl Bus, m: Mode, write: bool) -> u32 {
        let db = (self.db as u32) << 16;
        match m {
            Dp | Dpx | Dpy | Ind | Idx | Idy | Idl | Ily => {
                let v = self.fetch(b);
                if self.d & 255 != 0 {
                    b.idle();
                }
                let ix = match m {
                    Dpx | Idx => self.x,
                    Dpy => self.y,
                    _ => 0,
                };
                if matches!(m, Dpx | Dpy | Idx) {
                    b.idle();
                }
                let a = if self.e && self.d & 255 == 0 && ix != 0 {
                    (self.d & 0xff00) | (v as u16).wrapping_add(ix) & 255
                } else {
                    self.d.wrapping_add(v as u16).wrapping_add(ix)
                };
                match m {
                    Ind | Idx => {
                        let ptr = if m == Idx && self.e {
                            u16::from_le_bytes([
                                b.read(a as u32),
                                b.read(((a & 0xff00) | (a.wrapping_add(1) & 255)) as u32),
                            ])
                        } else {
                            self.dpword(b, a)
                        };
                        db | ptr as u32
                    }
                    Idy => {
                        let base = self.dpword(b, a);
                        self.indexed_cycle(b, base, self.y, write);
                        (db + base as u32 + self.y as u32) & 0xffffff
                    }
                    Idl | Ily => {
                        // Long pointers do not page-wrap in emulation mode,
                        // but all three pointer bytes still stay in bank zero.
                        let l = u16::from_le_bytes([
                            b.read(a as u32),
                            b.read(a.wrapping_add(1) as u32),
                        ]) as u32;
                        let bank = b.read(a.wrapping_add(2) as u32) as u32;
                        ((bank << 16) + l + if m == Ily { self.y as u32 } else { 0 }) & 0xffffff
                    }
                    _ => a as u32,
                }
            }
            Abs | Abx | Aby => {
                let a = self.word(b);
                let index = match m {
                    Abx => self.x,
                    Aby => self.y,
                    _ => 0,
                };
                if m != Abs {
                    self.indexed_cycle(b, a, index, write);
                }
                (db + a as u32 + index as u32) & 0xffffff
            }
            Long | Lgx => {
                let a = self.word(b) as u32;
                let bank = self.fetch(b) as u32;
                ((bank << 16) + a + if m == Lgx { self.x as u32 } else { 0 }) & 0xffffff
            }
            Sr | Siy => {
                let v = self.fetch(b);
                b.idle();
                let a = self.s.wrapping_add(v as u16);
                if m == Siy {
                    let base =
                        u16::from_le_bytes([b.read(a as u32), b.read(a.wrapping_add(1) as u32)]);
                    b.idle();
                    (db + base as u32 + self.y as u32) & 0xffffff
                } else {
                    a as u32
                }
            }
            _ => 0,
        }
    }
    fn indexed_cycle(&self, b: &mut impl Bus, base: u16, index: u16, write: bool) {
        // W65C816S table 5-7, note 4: (d),Y and absolute indexed reads
        // add one cycle for a page crossing or 16-bit indexes; writes always do.
        // Absolute indexed read-modify-write instructions also always do.
        if write || !self.flag(X) || (base & 255) as u32 + index as u32 > 255 {
            b.idle();
        }
    }
    fn compare(&mut self, a: u16, v: u16, byte: bool) {
        let mask = if byte { 255 } else { 65535 };
        self.set(C, (a & mask) >= (v & mask));
        self.nz(a.wrapping_sub(v) & mask, byte);
    }
    fn alu(&mut self, v: u16, sub: bool, byte: bool) {
        let mask = if byte { 255u32 } else { 65535 };
        let sign = (mask + 1) >> 1;
        let a = self.a as u32 & mask;
        let v = v as u32 & mask;
        let carry = u32::from(self.flag(C));
        let binary = if sub {
            a.wrapping_add(v ^ mask) + carry
        } else {
            a + v + carry
        };
        self.set(
            V,
            if sub {
                (a ^ v) & (a ^ binary) & sign != 0
            } else {
                (!(a ^ v)) & (a ^ binary) & sign != 0
            },
        );
        let mut r = binary;
        if self.flag(D) {
            let mut out = 0u32;
            let mut c = carry as i32;
            for shift in (0..if byte { 8 } else { 16 }).step_by(4) {
                let aa = ((a >> shift) & 15) as i32;
                let vv = ((v >> shift) & 15) as i32;
                let n = if sub { aa - vv + c - 1 } else { aa + vv + c };
                if !sub && shift == if byte { 4 } else { 12 } {
                    let intermediate = (n as u32) << shift;
                    self.set(V, (!(a ^ v)) & (a ^ intermediate) & sign != 0);
                }
                let adjusted = if sub {
                    if n < 0 {
                        n - 6
                    } else {
                        n
                    }
                } else if n > 9 {
                    n + 6
                } else {
                    n
                };
                c = if sub {
                    i32::from(n >= 0)
                } else {
                    i32::from(n > 9)
                };
                out |= ((adjusted as u32) & 15) << shift;
            }
            r = out;
            self.set(C, c != 0);
        } else {
            self.set(C, binary > mask);
        }
        self.load_a((r & mask) as u16, byte);
    }
    fn load_a(&mut self, v: u16, byte: bool) {
        self.a = if byte {
            (self.a & 0xff00) | (v & 255)
        } else {
            v
        };
        self.nz(v, byte);
    }
    pub fn step(&mut self, b: &mut impl Bus) {
        if self.stopped || self.waiting {
            b.idle();
            return;
        }
        let op = self.fetch(b);
        self.instructions += 1;
        let (name, mode) = OPS[op as usize];
        let mb = self.flag(M);
        let xb = self.flag(X);
        match name {
            "BRK" | "COP" => {
                self.fetch(b);
                self.enter(
                    b,
                    match (name, self.e) {
                        ("COP", true) => 0xfff4,
                        ("COP", false) => 0xffe4,
                        (_, true) => 0xfffe,
                        _ => 0xffe6,
                    },
                    true,
                );
                return;
            }
            "REP" | "SEP" => {
                let v = self.fetch(b);
                self.status(if name == "REP" {
                    self.p & !v
                } else {
                    self.p | v
                });
                b.idle();
                return;
            }
            "WDM" => {
                self.fetch(b);
                return;
            }
            "MVP" | "MVN" => {
                let dst = self.fetch(b);
                let src = self.fetch(b);
                let v = b.read(((src as u32) << 16) | self.x as u32);
                b.write(((dst as u32) << 16) | self.y as u32, v);
                self.db = dst;
                let delta = if name == "MVN" { 1 } else { 65535 };
                self.x = self.x.wrapping_add(delta);
                self.y = self.y.wrapping_add(delta);
                if xb {
                    self.x &= 255;
                    self.y &= 255;
                }
                self.a = self.a.wrapping_sub(1);
                if self.a != 65535 {
                    self.pc = self.pc.wrapping_sub(3);
                }
                b.idle();
                b.idle();
                return;
            }
            "JSI" => {
                // Table 5-7: push the return address before fetching the high
                // operand and destination pointer; either may overlap the stack.
                let low = self.fetch(b);
                self.push16_full(b, self.pc);
                let high = self.fetch(b);
                b.idle();
                let pointer = u16::from_le_bytes([low, high]).wrapping_add(self.x);
                self.pc = Self::rd16_in_bank(b, ((self.pb as u32) << 16) | pointer as u32);
                return;
            }
            "JSL" => {
                let a = self.word(b);
                let e = self.e;
                self.e = false;
                self.push(b, self.pb);
                b.idle();
                // The bank operand is fetched after PBR is written to the stack.
                let bank = self.fetch(b);
                self.push16(b, self.pc.wrapping_sub(1));
                self.e = e;
                if e {
                    self.s = 0x100 | (self.s & 255);
                }
                self.pc = a;
                self.pb = bank;
                return;
            }
            "JMP" | "JSR" | "JMI" | "JXI" | "JLI" | "JML" | "PEA" => {
                let mut a = self.word(b);
                let mut bank = self.pb;
                if name == "JML" {
                    bank = self.fetch(b);
                }
                if name == "JMI" {
                    a = Self::rd16_in_bank(b, a as u32);
                }
                if name == "JXI" {
                    b.idle();
                    a = Self::rd16_in_bank(
                        b,
                        ((self.pb as u32) << 16) | a.wrapping_add(self.x) as u32,
                    );
                }
                if name == "JLI" {
                    let pointer = a;
                    a = Self::rd16_in_bank(b, pointer as u32);
                    bank = b.read(pointer.wrapping_add(2) as u32);
                }
                if name == "PEA" {
                    self.push16_full(b, a);
                    return;
                }
                if name == "JSR" {
                    b.idle();
                    self.push16(b, self.pc.wrapping_sub(1));
                }
                self.pc = a;
                self.pb = bank;
                return;
            }
            "PEI" => {
                let a = self.addr(b, Dp, false);
                let v = Self::rd16_in_bank(b, a);
                self.push16_full(b, v);
                return;
            }
            "PER" | "BRL" => {
                let v = self.word(b);
                let target = self.pc.wrapping_add(v);
                b.idle();
                if name == "PER" {
                    self.push16_full(b, target);
                } else {
                    self.pc = target;
                }
                return;
            }
            _ => {}
        }
        if mode == Rel {
            let v = self.fetch(b) as i8;
            let take = match name {
                "BRA" => true,
                "BPL" => !self.flag(N),
                "BMI" => self.flag(N),
                "BVC" => !self.flag(V),
                "BVS" => self.flag(V),
                "BCC" => !self.flag(C),
                "BCS" => self.flag(C),
                "BNE" => !self.flag(Z),
                _ => self.flag(Z),
            };
            if take {
                let prev = self.pc;
                self.pc = self.pc.wrapping_add_signed(v as i16);
                b.idle();
                if self.e && self.pc & 0xff00 != prev & 0xff00 {
                    b.idle();
                }
            }
            return;
        }
        if mode == Imp {
            b.idle();
            match name {
                "CLC" => self.p &= !C,
                "SEC" => self.p |= C,
                "CLI" => self.p &= !I,
                "SEI" => self.p |= I,
                "CLD" => self.p &= !D,
                "SED" => self.p |= D,
                "CLV" => self.p &= !V,
                "XCE" => {
                    let e = self.flag(C);
                    self.set(C, self.e);
                    self.e = e;
                    self.status(self.p);
                }
                "XBA" => {
                    self.a = self.a.rotate_left(8);
                    self.nz(self.a & 255, true);
                    b.idle();
                }
                "TAX" | "TAY" | "TSX" | "TYX" | "TXY" => {
                    let v = match name {
                        "TSX" => self.s,
                        "TYX" => self.y,
                        "TXY" => self.x,
                        _ => self.a,
                    };
                    let v = if xb { v & 255 } else { v };
                    if name == "TAY" || name == "TXY" {
                        self.y = v;
                    } else {
                        self.x = v;
                    }
                    self.nz(v, xb);
                }
                "TXA" => self.load_a(self.x, mb),
                "TYA" => self.load_a(self.y, mb),
                "TSC" => self.load_a(self.s, false),
                "TDC" => self.load_a(self.d, false),
                "TCS" => {
                    self.s = self.a;
                    if self.e {
                        self.s = 0x100 | (self.s & 255);
                    }
                }
                "TXS" => {
                    self.s = self.x;
                    if self.e {
                        self.s = 0x100 | (self.s & 255);
                    }
                }
                "TCD" => {
                    self.d = self.a;
                    self.nz(self.d, false);
                }
                "INX" | "DEX" | "INY" | "DEY" => {
                    let v = if name == "INX" || name == "DEX" {
                        self.x
                    } else {
                        self.y
                    };
                    let v = v.wrapping_add(if name == "INX" || name == "INY" {
                        1
                    } else {
                        65535
                    }) & if xb { 255 } else { 65535 };
                    if name == "INX" || name == "DEX" {
                        self.x = v;
                    } else {
                        self.y = v;
                    }
                    self.nz(v, xb);
                }
                "PHP" => {
                    self.push(b, if self.e { self.p | 0x30 } else { self.p });
                }
                "PHB" => self.push(b, self.db),
                "PHK" => self.push(b, self.pb),
                "PHD" => self.push16_full(b, self.d),
                "PHA" | "PHX" | "PHY" => {
                    let (v, byte) = match name {
                        "PHA" => (self.a, mb),
                        "PHX" => (self.x, xb),
                        _ => (self.y, xb),
                    };
                    if !byte {
                        self.push(b, (v >> 8) as u8);
                    }
                    self.push(b, v as u8);
                }
                "PLP" => {
                    // Table 5-7: pulls and returns idle twice before reading S+1.
                    // The first internal cycle is shared by all implied opcodes.
                    b.idle();
                    let v = self.pop(b);
                    self.status(v);
                }
                "PLB" => {
                    b.idle();
                    let e = self.e;
                    self.e = false;
                    self.db = self.pop(b);
                    self.e = e;
                    if e {
                        self.s = 0x100 | (self.s & 255);
                    }
                    self.nz(self.db as u16, true);
                }
                "PLD" => {
                    b.idle();
                    self.d = self.pop16_full(b);
                    self.nz(self.d, false);
                }
                "PLA" | "PLX" | "PLY" => {
                    b.idle();
                    let byte = if name == "PLA" { mb } else { xb };
                    let v = if byte {
                        self.pop(b) as u16
                    } else {
                        self.pop16(b)
                    };
                    if name == "PLA" {
                        self.load_a(v, byte);
                    } else {
                        if name == "PLX" {
                            self.x = v;
                        } else {
                            self.y = v;
                        }
                        self.nz(v, byte);
                    }
                }
                "RTS" => {
                    b.idle();
                    self.pc = self.pop16(b).wrapping_add(1);
                    b.idle();
                }
                "RTL" => {
                    b.idle();
                    let e = self.e;
                    self.e = false;
                    self.pc = self.pop16(b).wrapping_add(1);
                    self.pb = self.pop(b);
                    self.e = e;
                    if e {
                        self.s = 0x100 | (self.s & 255);
                    }
                }
                "RTI" => {
                    b.idle();
                    let p = self.pop(b);
                    self.status(p);
                    self.pc = self.pop16(b);
                    if !self.e {
                        self.pb = self.pop(b);
                    }
                }
                "WAI" => {
                    self.waiting = true;
                    b.idle();
                }
                "STP" => {
                    self.stopped = true;
                    b.idle();
                }
                "NOP" => {}
                _ => unreachable!("{name}"),
            }
            return;
        }
        let byte = if ["LDX", "LDY", "STX", "STY", "CPX", "CPY"].contains(&name) {
            xb
        } else {
            mb
        };
        let mask = if byte { 255 } else { 65535 };
        let mut a = 0;
        let immediate = mode == Imm || mode == ImmX;
        let store = ["STA", "STX", "STY", "STZ"].contains(&name);
        let v = if mode == Acc {
            self.a & mask
        } else if immediate {
            if byte {
                self.fetch(b) as u16
            } else {
                self.word(b)
            }
        } else {
            let modify = matches!(name, "ASL" | "LSR" | "ROL" | "ROR" | "INC" | "DEC");
            a = self.addr(b, mode, store || modify);
            if store {
                0
            } else if byte {
                b.read(a) as u16
            } else {
                u16::from_le_bytes([b.read(a), b.read(Self::next_data_addr(mode, a))])
            }
        };
        let mut out = None;
        match name {
            "LDA" => self.load_a(v, byte),
            "LDX" => {
                self.x = v;
                self.nz(v, byte);
            }
            "LDY" => {
                self.y = v;
                self.nz(v, byte);
            }
            "STA" => out = Some(self.a),
            "STX" => out = Some(self.x),
            "STY" => out = Some(self.y),
            "STZ" => out = Some(0),
            "ORA" => self.load_a((self.a | v) & mask, byte),
            "AND" => self.load_a(self.a & v, byte),
            "EOR" => self.load_a((self.a ^ v) & mask, byte),
            "ADC" => self.alu(v, false, byte),
            "SBC" => self.alu(v, true, byte),
            "CMP" => self.compare(self.a, v, byte),
            "CPX" => self.compare(self.x, v, byte),
            "CPY" => self.compare(self.y, v, byte),
            "BIT" => {
                self.set(Z, self.a & v & mask == 0);
                if !immediate {
                    self.set(N, v & if byte { 128 } else { 32768 } != 0);
                    self.set(V, v & if byte { 64 } else { 16384 } != 0);
                }
            }
            "TSB" | "TRB" => {
                self.set(Z, self.a & v & mask == 0);
                out = Some(if name == "TSB" {
                    v | self.a
                } else {
                    v & !self.a
                });
                b.idle();
            }
            "INC" | "DEC" | "ASL" | "LSR" | "ROL" | "ROR" => {
                let c = u16::from(self.flag(C));
                let r = match name {
                    "INC" => v.wrapping_add(1),
                    "DEC" => v.wrapping_sub(1),
                    "ASL" | "ROL" => {
                        self.set(C, v & if byte { 128 } else { 32768 } != 0);
                        (v << 1) | if name == "ROL" { c } else { 0 }
                    }
                    _ => {
                        self.set(C, v & 1 != 0);
                        (v >> 1)
                            | if name == "ROR" {
                                c * if byte { 128 } else { 32768 }
                            } else {
                                0
                            }
                    }
                } & mask;
                self.nz(r, byte);
                out = Some(r);
                b.idle();
            }
            _ => unreachable!("{name}"),
        }
        if let Some(v) = out {
            if mode == Acc {
                self.a = (self.a & !mask) | (v & mask);
            } else if !store && !byte {
                // Word read-modify-write instructions write high before low
                // (W65C816S Table 5-7); I/O port side effects observe this order.
                b.write(Self::next_data_addr(mode, a), (v >> 8) as u8);
                b.write(a, v as u8);
            } else {
                b.write(a, v as u8);
                if !byte {
                    b.write(Self::next_data_addr(mode, a), (v >> 8) as u8);
                }
            }
        }
    }
}
