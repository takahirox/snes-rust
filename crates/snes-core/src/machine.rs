use crate::{
    apu::Apu,
    cartridge::Cartridge,
    cpu::{Bus, Cpu},
    ppu::Ppu,
};
const LINE: u64 = 1364;
const HBLANK: u64 = 1096;
// Fullsnes detailed H/V Events: V=225, H=10 dots (4 clocks/dot).
// Distinct from VBlank availability at H=0 and first-line INIDISP writes.
const OAM_RELOAD: u64 = 40;
// Fullsnes H/V Events and Anomie HDMA: activity at H=278 (4 clocks/dot).
// Model this as service eligibility, consumed at the next CPU/DMA boundary.
// No distinct earlier hardware request edge is established by those references;
// startup/alignment/full overhead remain approximate. See Issue #35 validation.
const HDMA_SCANLINE: u64 = 1112;
// Anomie: scanline 0, H approximately 6 dots. This 24-master-clock event is
// a scheduling convention, not verified DMA startup/alignment accuracy.
const HDMA_INIT: u64 = 24;
const DMA_PATTERN: [&[u8]; 8] = [
    &[0],
    &[0, 1],
    &[0, 0],
    &[0, 0, 1, 1],
    &[0, 1, 2, 3],
    &[0, 1, 0, 1],
    &[0, 0],
    &[0, 0, 1, 1],
];
pub struct Machine {
    pub cpu: Cpu,
    pub bus: SystemBus,
}
pub struct SystemBus {
    pub cart: Cartridge,
    pub ppu: Ppu,
    pub apu: Apu,
    pub wram: Vec<u8>,
    pub clocks: u64,
    pub frames: u64,
    io: [u8; 32],
    dma: [[u8; 16]; 8],
    open: u8,
    waddr: usize,
    h: u64,
    v: u16,
    nmi_flag: bool,
    nmi_pending: bool,
    irq: bool,
    pub buttons: u16,
    joy: u16,
    serial: u16,
    strobe: bool,
    auto_until: u64,
    auto_buttons: u16,
    dma_pending: u8,
    hdma_active: u8,
    hdma_line: [u8; 8],
    hdma_transfer: [bool; 8],
    hdma_pending: bool,
    hdma_init: bool,
    math: Option<(u8, u16, u16)>,
}
impl Machine {
    pub fn new(rom: &[u8]) -> Result<Self, String> {
        let cart = Cartridge::load(rom)?;
        let mut m = Self {
            cpu: Cpu::default(),
            bus: SystemBus::new(cart),
        };
        m.cpu.reset(&mut m.bus);
        Ok(m)
    }
    /// Reset preserves cartridge SRAM, clears volatile state, and releases controllers.
    pub fn reset(&mut self) {
        let cart = self.bus.cart.clone();
        self.bus = SystemBus::new(cart);
        self.cpu.reset(&mut self.bus);
    }
    pub fn step(&mut self) {
        self.bus.events();
        if self.bus.nmi_pending {
            self.bus.nmi_pending = false;
            self.cpu.interrupt(&mut self.bus, true);
        } else if self.bus.irq {
            self.cpu.interrupt(&mut self.bus, false);
        }
        self.cpu.step(&mut self.bus);
    }
    pub fn run_frame(&mut self) -> Result<(), String> {
        let end = self.bus.frames + 1;
        let deadline = self.bus.clocks + 400_000;
        while self.bus.frames < end {
            self.step();
            if self.bus.clocks > deadline && self.bus.frames < end {
                return Err(format!(
                    "Frame deadline exceeded at {:02x}:{:04x}",
                    self.cpu.pb, self.cpu.pc
                ));
            }
            if self.cpu.stopped {
                return Err(format!(
                    "CPU STP at {:02x}:{:04x}, frame {}",
                    self.cpu.pb, self.cpu.pc, self.bus.frames
                ));
            }
        }
        if let Some(error) = self.bus.apu.error.take() {
            return Err(error);
        }
        Ok(())
    }
    pub fn set_buttons(&mut self, buttons: u16) {
        self.bus.buttons = buttons & 0xfff0;
    }
    pub fn framebuffer(&self) -> &[u8] {
        &self.bus.ppu.frame
    }
    pub fn take_audio(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.bus.apu.samples)
    }
    pub fn load_sram(&mut self, data: &[u8]) -> Result<(), String> {
        if data.len() != self.bus.cart.sram.len() {
            return Err("Save size does not match cartridge SRAM".into());
        }
        self.bus.cart.sram.copy_from_slice(data);
        Ok(())
    }
}
impl SystemBus {
    fn new(cart: Cartridge) -> Self {
        Self {
            cart,
            ppu: Ppu::default(),
            apu: Apu::default(),
            wram: vec![0; 131072],
            clocks: 0,
            frames: 0,
            io: {
                let mut io = [0; 32];
                io[1] = 0xff; // WRIO power-on value; no falling-edge latch.
                io
            },
            dma: [[0xff; 16]; 8],
            open: 0,
            waddr: 0,
            h: 0,
            v: 0,
            nmi_flag: false,
            nmi_pending: false,
            irq: false,
            buttons: 0,
            joy: 0,
            serial: 0,
            strobe: false,
            auto_until: 0,
            auto_buttons: 0,
            dma_pending: 0,
            hdma_active: 0,
            hdma_line: [0; 8],
            hdma_transfer: [false; 8],
            hdma_pending: false,
            hdma_init: false,
            math: None,
        }
    }
    fn tick(&mut self, n: u64) {
        self.apu.tick(n);
        let mut n = n;
        let htime = (u16::from_le_bytes([self.io[7], self.io[8] & 1]) as u64) * 4;
        let vtime = u16::from_le_bytes([self.io[9], self.io[10] & 1]);
        let mode = self.io[0] & 0x30;
        while n > 0 {
            // Compare within one scanline, then handle H=0 on the new line.
            let old_h = self.h;
            let elapsed = n.min(LINE - self.h);
            self.clocks += elapsed;
            self.h += elapsed;
            n -= elapsed;
            if self.v == 225 && old_h < OAM_RELOAD && self.h >= OAM_RELOAD {
                self.ppu.oam_vblank_reload();
            }
            if self.v == 0 && old_h < HDMA_INIT && self.h >= HDMA_INIT {
                self.hdma_init = true;
            }
            if old_h < HBLANK && self.h >= HBLANK && (1..=224).contains(&self.v) {
                self.ppu.render_line(self.v as usize - 1);
            }
            if old_h < HDMA_SCANLINE && self.h >= HDMA_SCANLINE && self.v <= 224 {
                self.hdma_pending = true;
            }
            if mode & 0x10 != 0
                && old_h < htime
                && self.h >= htime
                && htime < LINE
                && (mode & 0x20 == 0 || self.v == vtime)
            {
                self.irq = true;
            }
            if self.h < LINE {
                break;
            }
            self.h = 0;
            self.v += 1;
            if self.v == 262 {
                self.v = 0;
                self.frames += 1;
                self.nmi_flag = false;
            }
            self.ppu.oam_scanline_start(self.v);
            // V=0 is a valid comparison only after the frame counter wraps.
            if (mode == 0x20 && self.v == vtime)
                || (mode & 0x10 != 0 && htime == 0 && (mode & 0x20 == 0 || self.v == vtime))
            {
                self.irq = true;
            }
            if self.v == 225 {
                self.nmi_flag = true;
                if self.io[0] & 128 != 0 {
                    self.nmi_pending = true;
                }
                if self.io[0] & 1 != 0 {
                    self.auto_buttons = self.buttons;
                    self.auto_until = self.clocks + 4224;
                }
            }
        }
        if self.auto_until != 0 && self.clocks >= self.auto_until {
            self.joy = self.auto_buttons;
            self.auto_until = 0;
        }
        self.ppu.h = self.h as u16;
        self.ppu.v = self.v;
    }
    fn cpu_cycle(&mut self) {
        if let Some((remaining, quotient, product)) = self.math {
            if remaining == 1 {
                self.io[20..24].copy_from_slice(&[
                    quotient as u8,
                    (quotient >> 8) as u8,
                    product as u8,
                    (product >> 8) as u8,
                ]);
                self.math = None;
            } else {
                self.math = Some((remaining - 1, quotient, product));
            }
        }
    }
    fn speed(&self, a: u32) -> u64 {
        let bank = a >> 16;
        let lo = a & 65535;
        if bank & 0x40 != 0 || lo >= 0x8000 {
            if bank & 128 != 0 && self.io[13] & 1 != 0 {
                6
            } else {
                8
            }
        } else if lo < 0x2000 {
            8
        } else if (0x4000..0x4200).contains(&lo) {
            12
        } else {
            6
        }
    }
    pub fn peek(&self, a: u32) -> u8 {
        let bank = a >> 16;
        let lo = (a & 65535) as usize;
        if bank == 0x7e || bank == 0x7f {
            self.wram[(a as usize) & 0x1ffff]
        } else if bank & 0x40 == 0 && lo < 0x2000 {
            self.wram[lo]
        } else {
            self.cart.read(a).unwrap_or(self.open)
        }
    }
    fn read_raw(&mut self, a: u32) -> u8 {
        let a = a & 0xffffff;
        let bank = a >> 16;
        let lo = (a & 65535) as usize;
        if bank == 0x7e || bank == 0x7f {
            return self.wram[a as usize & 0x1ffff];
        }
        if bank & 0x40 == 0 {
            match lo {
                0..=0x1fff => return self.wram[lo],
                0x2100..=0x213f => return self.ppu.read((lo - 0x2100) as u8, self.open),
                0x2140..=0x217f => return self.apu.port_read((lo - 0x2140) & 3),
                0x2180 => {
                    let v = self.wram[self.waddr];
                    self.waddr = (self.waddr + 1) & 0x1ffff;
                    return v;
                }
                0x4016 => {
                    let v = if self.strobe {
                        (self.buttons >> 15) as u8
                    } else {
                        (self.serial >> 15) as u8
                    };
                    if !self.strobe {
                        self.serial = (self.serial << 1) | 1;
                    }
                    return (self.open & 0xfc) | (v & 1);
                }
                0x4017 => return (self.open & 0xe0) | 0x1c,
                0x4210 => {
                    let v = if self.nmi_flag { 128 } else { 0 } | (self.open & 0x70) | 2;
                    self.nmi_flag = false;
                    return v;
                }
                0x4211 => {
                    let v = if self.irq { 128 } else { 0 } | (self.open & 127);
                    self.irq = false;
                    return v;
                }
                0x4212 => {
                    return if self.v >= 225 { 128 } else { 0 }
                        | if self.h >= HBLANK { 64 } else { 0 }
                        | u8::from(self.clocks < self.auto_until)
                        | (self.open & 0x3e)
                }
                0x4213 => return self.io[1],
                0x4214..=0x4217 => return self.io[lo - 0x4200],
                0x4218 => return self.joy as u8,
                0x4219 => return (self.joy >> 8) as u8,
                0x421a..=0x421f => return 0,
                0x4300..=0x437f => return self.dma[(lo >> 4) & 7][lo & 15],
                _ => {}
            }
        }
        self.cart.read(a).unwrap_or(self.open)
    }
    fn write_raw(&mut self, a: u32, v: u8) {
        let a = a & 0xffffff;
        let bank = a >> 16;
        let lo = (a & 65535) as usize;
        if bank == 0x7e || bank == 0x7f {
            self.wram[a as usize & 0x1ffff] = v;
            return;
        }
        if bank & 0x40 == 0 {
            match lo {
                0..=0x1fff => {
                    self.wram[lo] = v;
                    return;
                }
                0x2100..=0x2133 => {
                    self.ppu.write((lo - 0x2100) as u8, v);
                    return;
                }
                0x2140..=0x217f => {
                    self.apu.port_write((lo - 0x2140) & 3, v);
                    return;
                }
                0x2180 => {
                    self.wram[self.waddr] = v;
                    self.waddr = (self.waddr + 1) & 0x1ffff;
                    return;
                }
                0x2181 => {
                    self.waddr = (self.waddr & 0x1ff00) | v as usize;
                    return;
                }
                0x2182 => {
                    self.waddr = (self.waddr & 0x100ff) | ((v as usize) << 8);
                    return;
                }
                0x2183 => {
                    self.waddr = (self.waddr & 65535) | (((v as usize) & 1) << 16);
                    return;
                }
                0x4016 => {
                    let next = v & 1 != 0;
                    if next || self.strobe {
                        self.serial = self.buttons;
                    }
                    self.strobe = next;
                    return;
                }
                0x4200..=0x420d => {
                    let old = self.io[lo - 0x4200];
                    self.io[lo - 0x4200] = v;
                    match lo {
                        0x4200 => {
                            if old & 128 == 0 && v & 128 != 0 && self.nmi_flag {
                                self.nmi_pending = true;
                            }
                            if v & 0x30 == 0 {
                                self.irq = false;
                            }
                        }
                        0x4201 => self.ppu.set_latch_enabled(v & 0x80 != 0),
                        0x4203 => {
                            let r = self.io[2] as u16 * v as u16;
                            self.math = Some((8, v as u16, r));
                        }
                        0x4206 => {
                            let n = u16::from_le_bytes([self.io[4], self.io[5]]);
                            let (q, r) = if v == 0 {
                                (65535, n)
                            } else {
                                (n / v as u16, n % v as u16)
                            };
                            self.math = Some((16, q, r));
                        }
                        0x420b => self.dma_pending = v,
                        _ => {}
                    }
                    return;
                }
                0x4300..=0x437f => {
                    let channel = (lo >> 4) & 7;
                    self.dma[channel][lo & 15] = v;
                    if lo & 15 == 10 {
                        // CPU NTRL writes replace the whole counter, preserving
                        // the prepared transfer flag until after the next event.
                        // Anomie 43xA / HDMA steps 3-5; Issue #37 validation.
                        self.hdma_line[channel] = v;
                    }
                    return;
                }
                _ => {}
            }
        }
        self.cart.write(a, v);
    }
    // A CPU/DMA access can straddle refresh. The 40-clock hold must advance
    // every other device, without consuming another CPU cycle or DMA byte.
    fn bus_tick(&mut self, mut clocks: u64) {
        while clocks > 0 {
            let boundary = if self.h < 538 { 538 } else { LINE };
            let n = clocks.min(boundary - self.h);
            self.tick(n);
            clocks -= n;
            if self.h == 538 {
                self.tick(40);
            }
        }
    }
    // Only the A-bus endpoint is restricted; CPU and B-bus accesses keep their
    // ordinary decoding. Status/result reads remain accessible and keep their
    // register side effects. See docs/issue-23-validation.md for the access rule.
    fn dma_a_blocked(a: u32, write: bool) -> bool {
        a & 0x400000 == 0
            && (matches!(a & 0xffff, 0x2100..=0x21ff | 0x4000..=0x41ff | 0x4200..=0x420f | 0x4300..=0x437f)
                || (write && matches!(a & 0xffff, 0x4210..=0x421f)))
    }
    fn dma_read_a(&mut self, a: u32) -> u8 {
        if Self::dma_a_blocked(a, false) {
            0
        } else {
            self.read_raw(a)
        }
    }
    fn dma_write_a(&mut self, a: u32, v: u8) {
        if !Self::dma_a_blocked(a, true) {
            self.write_raw(a, v);
        }
    }
    fn transfer(&mut self, channel: usize, a: u32, offset: u8) {
        let d = self.dma[channel];
        let b = 0x2100 | d[1].wrapping_add(offset) as u32;
        let wram = matches!(a >> 16, 0x7e | 0x7f) || (a & 0x400000 == 0 && a & 0xffff < 0x2000);
        self.bus_tick(4);
        let v = if d[0] & 128 == 0 {
            self.dma_read_a(a)
        } else if b == 0x2180 && wram {
            // Reverse conflicts still write A, but cannot read/increment WMDATA.
            // Retaining the previous bus byte is a deterministic approximation,
            // not a verified electrical value. See docs/issue-29-validation.md.
            self.open
        } else {
            self.read_raw(b)
        };
        self.bus_tick(4);
        if d[0] & 128 == 0 {
            // WRAM cannot drive A and the WRAM B ports simultaneously.
            // Suppress data/address writes (including WMDATA's increment), but retain
            // this byte's timing and the caller's DMA/HDMA progression.
            // See docs/issue-27-validation.md for the forward contract.
            if !(0x2180..=0x2183).contains(&b) || !wram {
                self.write_raw(b, v);
            }
        } else {
            self.dma_write_a(a, v);
        }
        self.open = v;
    }
    fn events(&mut self) {
        self.hdma_events();
        if self.dma_pending != 0 {
            let pending = std::mem::take(&mut self.dma_pending);
            self.bus_tick(8);
            for i in 0..8 {
                if pending & (1 << i) == 0 {
                    continue;
                }
                let d = self.dma[i];
                let pattern = DMA_PATTERN[(d[0] & 7) as usize];
                let count = u16::from_le_bytes([d[5], d[6]]);
                let count = if count == 0 { 65536 } else { count as usize };
                let mut addr = u16::from_le_bytes([d[2], d[3]]);
                self.bus_tick(8);
                for j in 0..count {
                    // HDMA owns the bus between DMA bytes; do not collapse
                    // multiple HBlanks into one event after a long transfer.
                    self.hdma_events();
                    self.transfer(
                        i,
                        ((d[4] as u32) << 16) | addr as u32,
                        pattern[j % pattern.len()],
                    );
                    if d[0] & 8 == 0 {
                        addr = addr.wrapping_add(if d[0] & 16 != 0 { 65535 } else { 1 });
                    }
                }
                self.dma[i][2] = addr as u8;
                self.dma[i][3] = (addr >> 8) as u8;
                self.dma[i][5] = 0;
                self.dma[i][6] = 0;
                self.hdma_active &= !(1 << i);
            }
            self.io[11] = 0;
        }
        self.hdma_events();
    }
    fn hdma_events(&mut self) {
        if self.hdma_init {
            self.hdma_init = false;
            self.hdma_active = self.io[12];
            for i in 0..8 {
                if self.hdma_active & (1 << i) == 0 {
                    continue;
                }
                self.dma[i][8] = self.dma[i][2];
                self.dma[i][9] = self.dma[i][3];
                self.hdma_reload(i);
            }
        }
        if self.hdma_pending {
            self.hdma_pending = false;
            for i in 0..8 {
                if self.hdma_active & (1 << i) == 0 || self.io[12] & (1 << i) == 0 {
                    continue;
                }
                if self.hdma_transfer[i] {
                    let d = self.dma[i];
                    let pattern = DMA_PATTERN[(d[0] & 7) as usize];
                    for &offset in pattern {
                        let indirect = d[0] & 64 != 0;
                        let pair = if indirect { 5 } else { 8 };
                        let addr = u16::from_le_bytes([self.dma[i][pair], self.dma[i][pair + 1]]);
                        let bank = if indirect { d[7] } else { d[4] };
                        self.transfer(i, ((bank as u32) << 16) | addr as u32, offset);
                        let next = addr.wrapping_add(1);
                        self.dma[i][pair] = next as u8;
                        self.dma[i][pair + 1] = (next >> 8) as u8;
                    }
                }
            }
            // Complete every channel's payload before fetching next-row
            // metadata (Anomie HDMA steps 1-2, then 3-5; see Issue #33 record).
            for i in 0..8 {
                if self.hdma_active & self.io[12] & (1 << i) == 0 {
                    continue;
                }
                self.hdma_line[i] = self.hdma_line[i].wrapping_sub(1);
                self.dma[i][10] = self.hdma_line[i];
                self.hdma_transfer[i] = self.hdma_line[i] & 128 != 0;
                if self.hdma_line[i] & 127 == 0 {
                    self.hdma_reload(i);
                }
            }
        }
    }
    // Both initialization and row-end reload publish metadata through timed
    // A-bus reads, independent of the payload direction. A fresh $80 is never
    // decremented until the next HBlank, so it naturally represents 128 lines.
    fn hdma_reload(&mut self, i: usize) {
        let counter = self.hdma_read(i);
        self.hdma_line[i] = counter;
        self.dma[i][10] = counter;
        self.hdma_transfer[i] = true;
        if self.dma[i][0] & 64 != 0 {
            // Later enabled, unterminated channels still count even if their
            // own row will terminate later in this reload pass (step 5b).
            let later = self.hdma_active & self.io[12] & (0xfeu8 << i) != 0;
            let low = if counter == 0 && !later {
                0
            } else {
                self.hdma_read(i)
            };
            let high = self.hdma_read(i);
            self.dma[i][5] = low;
            self.dma[i][6] = high;
        }
        if counter == 0 {
            self.hdma_active &= !(1 << i);
        }
    }
    fn hdma_read(&mut self, i: usize) -> u8 {
        let addr = u16::from_le_bytes([self.dma[i][8], self.dma[i][9]]);
        let a = ((self.dma[i][4] as u32) << 16) | addr as u32;
        let v = self.dma_read_a(a);
        let next = addr.wrapping_add(1);
        self.dma[i][8] = next as u8;
        self.dma[i][9] = (next >> 8) as u8;
        self.bus_tick(8);
        v
    }
}
impl Bus for SystemBus {
    fn read(&mut self, a: u32) -> u8 {
        self.events();
        self.bus_tick(self.speed(a));
        self.cpu_cycle();
        let v = self.read_raw(a);
        self.open = v;
        v
    }
    fn write(&mut self, a: u32, v: u8) {
        self.events();
        self.bus_tick(self.speed(a));
        self.cpu_cycle();
        self.open = v;
        self.write_raw(a, v);
    }
    fn idle(&mut self) {
        self.events();
        self.bus_tick(6);
        self.cpu_cycle();
    }
}

#[cfg(test)]
#[path = "machine/dma_tests.rs"]
mod dma_tests;

#[cfg(test)]
#[path = "machine/hdma_init_tests.rs"]
mod hdma_init_tests;

#[cfg(test)]
#[path = "machine/hdma_reload_tests.rs"]
mod hdma_reload_tests;

#[cfg(test)]
#[path = "machine/hdma_phase_tests.rs"]
mod hdma_phase_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::{M, X};

    fn blank_bus() -> SystemBus {
        SystemBus::new(Cartridge::load(include_bytes!("../../../tests/roms/smoke.bin")).unwrap())
    }

    #[test]
    fn refresh_stalls_one_bus_access_but_keeps_audio_and_raster_running() {
        let mut bus = blank_bus();
        bus.tick(534);
        bus.read(0x7e0000); // 8 clocks plus the 40-clock refresh hold.
        assert_eq!(bus.clocks, 582);
        assert_eq!(bus.h, 582);
        let before = bus.clocks;
        bus.read(0x7e0000);
        assert_eq!(bus.clocks - before, 8);
        // Enough complete scanlines to observe the independent 32 kHz clock.
        for _ in 0..1000 {
            bus.idle();
        }
        let stereo_frames = bus.apu.samples.len() as f64 / 2.0;
        assert!((stereo_frames - bus.clocks as f64 * 32000.0 / 21477272.0).abs() <= 1.0);
    }

    #[test]
    fn math_results_wait_cpu_cycles_in_fast_and_slow_regions() {
        for address in [0x008000, 0x808000, 0x7e0000, 0x004016] {
            let mut bus = blank_bus();
            bus.write_raw(0x420d, 1);
            bus.write(0x4202, 13);
            bus.write(0x4203, 17);
            for _ in 0..7 {
                bus.read(address);
            }
            assert_eq!(bus.io[22], 0, "multiply completed early");
            bus.read(address);
            assert_eq!(&bus.io[20..24], &[17, 0, 221, 0]);
            bus.write(0x4204, 0xe8);
            bus.write(0x4205, 3);
            bus.write(0x4206, 7);
            for _ in 0..15 {
                bus.read(address);
            }
            assert_eq!(&bus.io[20..24], &[17, 0, 221, 0], "divide completed early");
            bus.read(address);
            assert_eq!(&bus.io[20..24], &[142, 0, 6, 0]);
            bus.write(0x4206, 0);
            for _ in 0..16 {
                bus.idle();
            }
            assert_eq!(&bus.io[20..24], &[255, 255, 0xe8, 3]);
        }
    }

    #[test]
    fn hdma_preempts_long_dma_instead_of_losing_scanlines() {
        for start_line in [0, 261] {
            let mut bus = blank_bus();
            // Original HDMA brightness table: 3 rows, then end. DMA on another
            // channel runs across all three HBlanks. White backdrop, no BGs.
            bus.wram[..5].copy_from_slice(&[0x83, 15, 8, 1, 0]);
            bus.ppu.cgram[..2].copy_from_slice(&[0xff, 0x7f]);
            bus.dma[0] = [0; 16];
            bus.dma[0][4] = 0x7e;
            bus.io[12] = 1;
            if start_line == 261 {
                bus.io[12] = 0;
                while bus.v < 261 {
                    bus.idle();
                }
                bus.io[12] = 1;
            }
            bus.dma[1] = [0; 16];
            bus.dma[1][1] = 0x80;
            bus.dma[1][4] = 0x40;
            let bytes = if start_line == 0 { 600u16 } else { 800u16 };
            bus.dma[1][5..7].copy_from_slice(&bytes.to_le_bytes());
            bus.ppu.write(0, 15);
            bus.waddr = 0x10000; // DMA destination must not overwrite the HDMA table.
            while bus.h < 1000 {
                bus.idle();
            }
            bus.dma_pending = 2;
            bus.events();
            assert!(bus.clocks > LINE * 4);
            // Lines 1,2,3 use data transferred at preceding HBlanks 0,1,2.
            for (y, shade) in [(0, 255), (1, 136), (2, 17)] {
                assert_eq!(
                    &bus.ppu.frame[y * 256 * 4..y * 256 * 4 + 4],
                    &[shade, shade, shade, 255]
                );
            }
            assert_eq!(bus.hdma_active, 0);
            assert_eq!(bus.dma[1][5..7], [0, 0]);
            assert_eq!(bus.frames, u64::from(start_line == 261));
        }
    }

    #[test]
    fn dma_holds_cpu_and_nmi_until_completion_without_stopping_audio() {
        let mut rom = include_bytes!("../../../tests/roms/smoke.bin").to_vec();
        rom[0x100] = 0xea; // NMI handler NOP; isolate the interrupt's stack use.
        rom[0x7ffa..0x7ffc].copy_from_slice(&[0, 0x81]);
        let mut m = Machine::new(&rom).unwrap();
        m.cpu.pc = 0x8000;
        m.cpu.waiting = true;
        m.bus.io[0] = 0x80;
        m.bus.tick(LINE * 225 - m.bus.clocks - 40);
        m.bus.dma[0] = [0; 16];
        m.bus.dma[0][1] = 0x80;
        m.bus.dma[0][4] = 0x40;
        m.bus.dma[0][5] = 64;
        m.bus.dma_pending = 1;
        let pc = m.cpu.pc;
        let samples = m.bus.apu.samples.len();
        m.bus.events();
        assert_eq!(m.cpu.pc, pc);
        assert!(m.cpu.waiting);
        assert!(m.bus.nmi_pending);
        assert!(m.bus.apu.samples.len() > samples);
        assert_eq!(m.bus.io[11], 0);
        m.step();
        assert!(!m.cpu.waiting);
        assert!(!m.bus.nmi_pending);
        assert_eq!(m.cpu.s, 0x1fc); // Exactly one hardware interrupt frame.
    }

    #[test]
    fn dma_samples_source_before_ppu_write_and_commits_at_the_access_boundary() {
        for (line, expected) in [(224, 0x5a), (261, 0)] {
            let mut bus = blank_bus();
            bus.ppu.write(0, 15);
            bus.ppu.write(0x15, 0x80);
            bus.dma[0][0] = 0;
            bus.dma[0][1] = 0x18;
            bus.wram[0] = 0x5a;
            bus.v = line;
            bus.h = LINE - 6;
            bus.ppu.v = line;
            bus.ppu.h = (LINE - 6) as u16;
            // Read occurs at +4 in the old line; write at +8 in the new
            // line. Only the transition into VBlank can commit to VRAM.
            bus.transfer(0, 0x7e0000, 0);
            assert_eq!(bus.clocks, 8);
            assert_eq!(bus.ppu.vram[0], expected);
        }
    }

    #[test]
    fn nmi_enable_edge_status_ack_and_autojoy_completion_are_ordered() {
        let mut bus = blank_bus();
        bus.buttons = 0x9180;
        bus.write_raw(0x4200, 1);
        bus.tick(LINE * 225);
        assert!(bus.nmi_flag);
        assert!(!bus.nmi_pending);
        assert_ne!(bus.read_raw(0x4212) & 1, 0);
        bus.write_raw(0x4200, 0x81);
        assert!(std::mem::take(&mut bus.nmi_pending));
        bus.write_raw(0x4200, 0x81);
        assert!(!bus.nmi_pending, "a level is not a second NMI edge");
        assert_ne!(bus.read_raw(0x4210) & 128, 0);
        assert_eq!(bus.read_raw(0x4210) & 128, 0);
        bus.tick(4223);
        assert_ne!(bus.read_raw(0x4212) & 1, 0);
        bus.tick(1);
        assert_eq!(bus.read_raw(0x4212) & 1, 0);
        assert_eq!([bus.read_raw(0x4218), bus.read_raw(0x4219)], [0x80, 0x91]);
    }

    #[test]
    fn stack_reads_sample_nmi_status_at_the_vblank_boundary() {
        // At clock 306876 a byte PLA must read $4210 at 306902, after
        // VBlank begins at 306900. Reading before the second idle returns $62.
        for p in [0, M, X, M | X] {
            for op in [0x28, 0xab, 0x2b, 0x68, 0xfa, 0x7a, 0x40, 0x60, 0x6b] {
                for before_vblank in [27, 26, 25, 21, 20] {
                    let mut rom = include_bytes!("../../../tests/roms/smoke.bin").to_vec();
                    rom[0] = op;
                    let mut bus = SystemBus::new(Cartridge::load(&rom).unwrap());
                    let start = LINE * 225 - before_vblank;
                    bus.tick(start);
                    let mut cpu = Cpu {
                        pc: 0x8000,
                        e: false,
                        p,
                        s: 0x420f,
                        ..Cpu::default()
                    };
                    cpu.step(&mut bus);
                    let low = match op {
                        0x28 | 0x40 => cpu.p,
                        0xab => cpu.db,
                        0x2b => cpu.d as u8,
                        0x68 => cpu.a as u8,
                        0xfa => cpu.x as u8,
                        0x7a => cpu.y as u8,
                        _ => cpu.pc.wrapping_sub(1) as u8,
                    };
                    let expected = (op & 0x70) | 2 | if before_vblank <= 26 { 0x80 } else { 0 };
                    assert_eq!(low, expected, "opcode {op:02x}, P={p:02x}, start={start}");
                    let bytes = match op {
                        0x40 => 4,
                        0x6b => 3,
                        0x2b | 0x60 => 2,
                        0x68 if p & M == 0 => 2,
                        0xfa | 0x7a if p & X == 0 => 2,
                        _ => 1,
                    };
                    assert_eq!(cpu.s, 0x420f + bytes);
                    assert_eq!(
                        bus.clocks,
                        start + 20 + 6 * u64::from(bytes) + if op == 0x60 { 6 } else { 0 }
                    );
                    // An early read leaves the later VBlank flag pending; an
                    // on-time read consumes it exactly once.
                    assert_eq!(bus.nmi_flag, before_vblank > 26 && bytes > 1);
                }
            }
        }
    }

    fn timer_bus(mode: u8, htime: u16, vtime: u16) -> SystemBus {
        let cart = Cartridge::load(include_bytes!("../../../tests/roms/smoke.bin")).unwrap();
        let mut bus = SystemBus::new(cart);
        bus.write_raw(0x4207, htime as u8);
        bus.write_raw(0x4208, (htime >> 8) as u8);
        bus.write_raw(0x4209, vtime as u8);
        bus.write_raw(0x420a, (vtime >> 8) as u8);
        bus.write_raw(0x4200, mode);
        bus
    }

    fn timeup(bus: &mut SystemBus) -> bool {
        bus.read_raw(0x4211) & 0x80 != 0
    }

    #[test]
    fn horizontal_zero_timer_fires_once_per_scanline_across_frames() {
        let mut bus = timer_bus(0x10, 0, 0);
        for line in 1..=3 * 262 {
            bus.tick(LINE - 1);
            assert!(!timeup(&mut bus), "early IRQ on line {line}");
            bus.tick(1);
            assert!(timeup(&mut bus), "missing IRQ on line {line}");
            assert!(!timeup(&mut bus), "TIMEUP must acknowledge IRQ");
        }
        assert_eq!(bus.frames, 3);
    }

    #[test]
    fn vertical_zero_timer_fires_once_per_frame_after_wrap() {
        let mut bus = timer_bus(0x20, 0, 0);
        for frame in 1..=3 {
            bus.tick(LINE * 262 - 1);
            assert!(!timeup(&mut bus), "early IRQ in frame {frame}");
            bus.tick(1);
            assert!(timeup(&mut bus), "missing IRQ in frame {frame}");
            assert!(!timeup(&mut bus));
            assert_eq!((bus.frames, bus.v, bus.h), (frame, 0, 0));
        }
    }

    #[test]
    fn combined_timers_match_zero_and_nonzero_coordinates() {
        for (htime, vtime) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            let mut bus = timer_bus(0x30, htime, vtime);
            // Start after the initial frame's comparison, then verify three
            // complete frames, including the lines that must not trigger.
            bus.tick(LINE * vtime as u64 + 4 * htime as u64 + 1);
            timeup(&mut bus);
            for frame in 1..=3 {
                bus.tick(LINE * 262 - 2);
                assert!(!timeup(&mut bus), "early IRQ at ({htime}, {vtime})");
                bus.tick(1);
                assert!(timeup(&mut bus), "missing IRQ at ({htime}, {vtime})");
                assert_eq!((bus.frames, bus.v, bus.h), (frame, vtime, 4 * htime as u64));
                bus.tick(1);
                assert!(!timeup(&mut bus), "duplicate IRQ at ({htime}, {vtime})");
            }
        }
    }

    #[test]
    fn horizontal_timer_uses_the_scanline_after_rollover() {
        for vtime in [0, 1, 261] {
            let mut bus = timer_bus(0x30, 1, vtime);
            bus.v = 261;
            bus.h = LINE - 2;
            bus.tick(8); // Cross H=1 on V=0 in a single bus access.
            assert_eq!((bus.frames, bus.v, bus.h), (1, 0, 6));
            assert_eq!(timeup(&mut bus), vtime == 0, "VTIME={vtime}");
        }
        let mut bus = timer_bus(0x10, 341, 0);
        bus.tick(LINE + 8);
        assert!(!timeup(&mut bus), "out-of-range HTIME must not match");
        let mut bus = timer_bus(0x20, 0, 262);
        bus.tick(LINE * 262);
        assert!(!timeup(&mut bus), "out-of-range VTIME must not match");
    }
}

#[cfg(test)]
#[path = "machine/hdma_ntrl_tests.rs"]
mod hdma_ntrl_tests;

#[cfg(test)]
#[path = "machine/ppu_counter_tests.rs"]
mod ppu_counter_tests;

#[cfg(test)]
#[path = "machine/ppu_vram_tests.rs"]
mod ppu_vram_tests;

#[cfg(test)]
#[path = "machine/ppu_oam_tests.rs"]
mod ppu_oam_tests;
