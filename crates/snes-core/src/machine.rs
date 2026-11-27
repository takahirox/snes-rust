use crate::{
    apu::Apu,
    cartridge::Cartridge,
    cpu::{Bus, Cpu},
    ppu::Ppu,
};
#[cfg(test)]
const LINE: u64 = 1364;
const HBLANK: u64 = 1096;
const PICTURE_END: u64 = 1112;
// Fullsnes detailed H/V Events: V=225, H=10 dots (4 clocks/dot).
// Distinct from VBlank availability at H=0 and first-line INIDISP writes.
const OAM_RELOAD: u64 = 40;
// Fullsnes SNES Timing H/V Events: V-only IRQ at H=2.5 dots = 10
// master clocks, independent of HTIME. This is line assertion; Machine::step
// still samples interrupts at instruction boundaries. See Issue #97 validation.
const V_ONLY_IRQ: u64 = 10;
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
// Logical automatic phases; exact electrical edges remain unverified.
// See docs/issue-69-validation.md for sources and event conventions.
struct AutoJoypad {
    next: u64,
    step: u8, // 0: release latch; 1..=16: sample/clock both ports.
}

pub struct Machine {
    pub cpu: Cpu,
    pub bus: SystemBus,
    probe: Option<LoopProbe>,
    snapshot_boundary: bool,
}
// CPU and bus state at an instruction boundary, used to recognize a loop
// iteration that only read ROM/WRAM on the access fast path.
#[derive(Clone, Copy, PartialEq)]
struct LoopProbe {
    words: [u16; 6],
    bytes: [u8; 4],
    emulation: bool,
    v: u16,
    effects: u64,
    clocks: u64,
    instructions: u64,
}
pub struct SystemBus {
    pub cart: Cartridge,
    pub ppu: Ppu,
    pub apu: Apu,
    pub wram: Box<[u8; 0x20000]>,
    pub clocks: u64,
    pub frames: u64,
    #[cfg(test)]
    raster_quantum: u64,
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
    buttons2: u16,
    joy: [u16; 4],
    serial: [u16; 2],
    strobe: bool,
    auto_start: Option<u64>,
    auto_poll: Option<AutoJoypad>,
    auto_latch: bool,
    dma_pending: u8,
    hdma_active: u8,
    hdma_line: [u8; 8],
    hdma_transfer: [bool; 8],
    hdma_pending: bool,
    hdma_init: bool,
    math: Option<(u8, u16, u16)>,
    defer_raster: bool,
    // `tick` may skip its event scan while H stays strictly below this
    // limit on line `fast_v`; zero forces the full scan.
    fast_limit: u64,
    fast_v: u16,
    // A CPU access of c clocks may skip events, `bus_tick` and `tick`
    // entirely while H + c stays below this limit on line `fast_v`: no
    // DMA/HDMA is pending, no refresh or timing event intervenes and the
    // APU is deferred. Zero forces the ordinary path.
    access_limit: u64,
    // Inside run_frame, master clocks owed to the APU. The SPC and DSP see
    // the CPU only through the ports, so catching up before every port
    // access and at the end of the field runs the same SPC instructions.
    defer_apu: bool,
    apu_lag: u64,
    // Counts CPU accesses off the pure fast path (writes, I/O, slow timing).
    effects: u64,
}
impl Machine {
    pub fn new(rom: &[u8]) -> Result<Self, String> {
        Ok(Self::powered_on(Cartridge::load(rom)?))
    }
    fn powered_on(cart: Cartridge) -> Self {
        let mut m = Self {
            cpu: Cpu::default(),
            bus: SystemBus::new(cart),
            probe: None,
            snapshot_boundary: true,
        };
        m.cpu.reset(&mut m.bus);
        m
    }
    /// Reset preserves cartridge SRAM, clears volatile state, and releases controllers.
    pub fn reset(&mut self) {
        let cart = self.bus.cart.clone();
        self.bus = SystemBus::new(cart);
        self.probe = None;
        self.snapshot_boundary = true;
        self.cpu.reset(&mut self.bus);
    }
    fn loop_probe(&self) -> LoopProbe {
        let c = &self.cpu;
        LoopProbe {
            words: [c.a, c.x, c.y, c.s, c.d, c.pc],
            bytes: [c.pb, c.db, c.p, self.bus.open],
            emulation: c.e,
            v: self.bus.v,
            effects: self.bus.effects,
            clocks: self.bus.clocks,
            instructions: c.instructions,
        }
    }
    // Fast-forward a pure polling loop. If the CPU is back at an earlier
    // instruction boundary with identical registers and open bus, and every
    // access since was a fast-path ROM/WRAM read or idle cycle, then memory
    // and pending events are unchanged, so each further iteration repeats
    // the same accesses and clocks. Repeat whole iterations that still end
    // before the next timing event.
    fn skip_pure_loop(&mut self) {
        match self.probe {
            Some(p) if p.words[5] == self.cpu.pc && p.bytes[0] == self.cpu.pb => {
                let now = self.loop_probe();
                let same = LoopProbe {
                    effects: p.effects,
                    clocks: p.clocks,
                    instructions: p.instructions,
                    ..now
                } == p;
                let period = now.clocks - p.clocks;
                if same && now.effects == p.effects && period != 0 {
                    let iterations = self.bus.quick_repeats(period);
                    if iterations != 0 {
                        self.bus.quick_advance(iterations * period);
                        self.cpu.instructions += iterations * (now.instructions - p.instructions);
                    }
                }
                self.probe = Some(self.loop_probe());
            }
            Some(p) if self.cpu.instructions - p.instructions < 8 => {}
            _ => self.probe = Some(self.loop_probe()),
        }
    }
    pub fn step(&mut self) {
        self.snapshot_boundary = false;
        self.step_instruction();
    }
    fn step_instruction(&mut self) {
        if self.bus.access_limit != 0 && !self.cpu.waiting && !self.cpu.stopped {
            self.skip_pure_loop();
        }
        if self.cpu.waiting
            && !self.cpu.stopped
            && !(self.bus.nmi_pending || self.bus.irq)
            && self.bus.idle_run()
        {
            return;
        }
        if self.bus.hdma_init || self.bus.hdma_pending || self.bus.dma_pending != 0 {
            self.bus.events();
        }
        if self.bus.nmi_pending {
            self.bus.nmi_pending = false;
            self.cpu.interrupt(&mut self.bus, true);
        } else if self.bus.irq {
            self.cpu.interrupt(&mut self.bus, false);
        }
        self.cpu.step(&mut self.bus);
    }
    pub fn run_frame(&mut self) -> Result<(), String> {
        self.snapshot_boundary = false;
        self.bus.defer_raster = true;
        self.bus.defer_apu = true;
        self.bus.invalidate_fast();
        let result = self.run_field();
        self.bus.sync_apu();
        self.bus.defer_apu = false;
        self.bus.defer_raster = false;
        self.bus.invalidate_fast();
        let result = result.and_then(|()| match self.bus.apu.error.take() {
            Some(error) => Err(error),
            None => Ok(()),
        });
        if (1..=224).contains(&self.bus.v) {
            self.bus
                .ppu
                .render_until(self.bus.v as usize - 1, self.bus.h);
        }
        self.snapshot_boundary = result.is_ok();
        result
    }
    fn run_field(&mut self) -> Result<(), String> {
        let end = self.bus.frames + 1;
        let deadline = self.bus.clocks + 400_000;
        while self.bus.frames < end {
            self.step_instruction();
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
        Ok(())
    }
    /// Set port 1: bits 15–4 are B, Y, Select, Start, Up, Down, Left,
    /// Right, A, X, L, R. Bits 3–0 are ignored.
    pub fn set_buttons(&mut self, buttons: u16) {
        self.bus.buttons = buttons & 0xfff0;
    }
    /// Set port 2 independently, using the same format as `set_buttons`.
    pub fn set_buttons2(&mut self, buttons: u16) {
        self.bus.buttons2 = buttons & 0xfff0;
    }
    pub fn framebuffer_width(&self) -> usize {
        self.bus.ppu.framebuffer_width()
    }
    pub fn framebuffer_height(&self) -> usize {
        self.bus.ppu.framebuffer_height()
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
    fn new(mut cart: Cartridge) -> Self {
        cart.power_on();
        Self {
            cart,
            ppu: Ppu::default(),
            apu: Apu::default(),
            wram: crate::zeroed(),
            clocks: 0,
            frames: 0,
            #[cfg(test)]
            raster_quantum: u64::MAX,
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
            buttons2: 0,
            joy: [0; 4],
            serial: [0; 2],
            strobe: false,
            auto_start: None,
            auto_poll: None,
            auto_latch: false,
            dma_pending: 0,
            hdma_active: 0,
            hdma_line: [0; 8],
            hdma_transfer: [false; 8],
            hdma_pending: false,
            hdma_init: false,
            math: None,
            defer_raster: false,
            fast_limit: 0,
            fast_v: 0,
            access_limit: 0,
            defer_apu: false,
            apu_lag: 0,
            effects: 0,
        }
    }
    fn sync_apu(&mut self) {
        let lag = std::mem::take(&mut self.apu_lag);
        if lag != 0 {
            self.apu.tick(lag);
        }
    }
    // Run consecutive WAI idle cycles at once while each would take the
    // `bus_tick` and `tick` fast paths: no DMA/HDMA, refresh, raster, IRQ,
    // NMI, joypad or line event can occur, so the CPU keeps waiting.
    // Returns false when the next idle needs the ordinary path.
    fn idle_run(&mut self) -> bool {
        if self.hdma_init
            || self.hdma_pending
            || self.dma_pending != 0
            || self.math.is_some()
            || self.v != self.fast_v
            || self.h == 538
        {
            return false;
        }
        let limit = if self.h < 538 {
            self.fast_limit.min(538)
        } else {
            self.fast_limit
        };
        let idles = limit.saturating_sub(self.h + 1) / 6;
        if idles == 0 {
            return false;
        }
        self.tick(idles * 6);
        true
    }
    fn tick(&mut self, n: u64) {
        if self.defer_apu && !self.apu.chunk_sensitive() {
            self.apu_lag += n;
        } else {
            self.sync_apu();
            self.apu.tick(n);
        }
        if self.h + n < self.fast_limit && self.v == self.fast_v {
            // No raster, IRQ, HDMA, joypad or line event lies in this interval.
            self.clocks += n;
            self.h += n;
            self.ppu.h = self.h as u16;
            return;
        }
        self.tick_events(n);
        self.update_fast_limit();
    }
    // The earliest H on this line at which `tick_events` could act: a
    // superset of its line-specific events. Both horizontal IRQ candidates
    // are included while H-IRQ is enabled, independent of VTIME. Writes to
    // $4200-$420D invalidate this limit.
    fn update_fast_limit(&mut self) {
        let h = self.h;
        let mut limit = self.ppu.line_clocks();
        let mut event = |t: u64| {
            if t > h {
                limit = limit.min(t);
            }
        };
        let mode = self.io[0] & 0x30;
        if self.v == 0 {
            event(4);
            event(HDMA_INIT);
        }
        if self.v == 225 {
            event(OAM_RELOAD);
        }
        if self.v <= 224 {
            event(PICTURE_END);
            event(HDMA_SCANLINE);
        }
        if mode & 0x10 != 0 {
            let htime = (u16::from_le_bytes([self.io[7], self.io[8] & 1]) as u64) * 4;
            event(htime);
            event(htime + 14);
        }
        if mode & 0x20 != 0 {
            event(V_ONLY_IRQ);
        }
        if let Some(next) = self.auto_start {
            limit = limit.min(self.h + (next - self.clocks));
        }
        if let Some(poll) = &self.auto_poll {
            limit = limit.min(self.h + (poll.next - self.clocks));
        }
        if !self.defer_raster && (1..=224).contains(&self.v) {
            limit = 0;
        }
        #[cfg(test)]
        if self.raster_quantum != u64::MAX {
            limit = 0;
        }
        self.fast_limit = limit;
        self.fast_v = self.v;
        self.refresh_access_limit();
    }
    fn invalidate_fast(&mut self) {
        self.fast_limit = 0;
        self.access_limit = 0;
    }
    fn refresh_access_limit(&mut self) {
        self.access_limit = if self.defer_apu
            && !self.apu.chunk_sensitive()
            && !self.hdma_init
            && !self.hdma_pending
            && self.dma_pending == 0
        {
            if self.h < 538 {
                self.fast_limit.min(538)
            } else {
                self.fast_limit
            }
        } else {
            0
        };
    }
    // Advance a CPU bus cycle of `clocks` when no event, DMA/HDMA or math
    // countdown needs attention; otherwise leave all state unchanged.
    #[inline(always)]
    fn quick_cycle(&mut self, clocks: u64) -> bool {
        if self.h + clocks < self.access_limit && self.v == self.fast_v && self.math.is_none() {
            self.quick_advance(clocks);
            true
        } else {
            false
        }
    }
    #[inline(always)]
    fn quick_advance(&mut self, clocks: u64) {
        self.apu_lag += clocks;
        self.clocks += clocks;
        self.h += clocks;
        self.ppu.h = self.h as u16;
    }
    // Whole repeats of a `period`-clock run of quick cycles that would all
    // stay on the fast path from here.
    fn quick_repeats(&self, period: u64) -> u64 {
        if self.v == self.fast_v && self.math.is_none() {
            self.access_limit.saturating_sub(self.h + 1) / period
        } else {
            0
        }
    }
    // One CPU bus cycle of `clocks`: service DMA/HDMA, advance time and
    // count the cycle for the math unit.
    #[inline(never)]
    fn access(&mut self, clocks: u64) {
        self.effects += 1;
        if self.h + clocks < self.access_limit && self.v == self.fast_v {
            self.apu_lag += clocks;
            self.clocks += clocks;
            self.h += clocks;
            self.ppu.h = self.h as u16;
        } else {
            self.events();
            self.bus_tick(clocks);
            self.refresh_access_limit();
        }
        self.cpu_cycle();
    }
    // WRAM index of `a`, including the low-bank mirror.
    #[inline(always)]
    fn wram_index(a: u32) -> Option<usize> {
        let bank = a >> 16;
        if bank & 0xfe == 0x7e {
            Some(a as usize & 0x1ffff)
        } else if bank & 0x40 == 0 && a & 0xffff < 0x2000 {
            Some(a as usize & 0x1fff)
        } else {
            None
        }
    }
    #[inline(never)]
    fn read_slow(&mut self, a: u32) -> u8 {
        self.access(self.speed(a));
        let v = self.read_raw(a);
        self.open = v;
        v
    }
    #[inline(never)]
    fn write_slow(&mut self, a: u32, v: u8) {
        self.access(self.speed(a));
        self.open = v;
        self.write_raw(a, v);
    }
    fn tick_events(&mut self, n: u64) {
        let mut n = n;
        let htime = (u16::from_le_bytes([self.io[7], self.io[8] & 1]) as u64) * 4;
        let vtime = u16::from_le_bytes([self.io[9], self.io[10] & 1]);
        let mode = self.io[0] & 0x30;
        while n > 0 {
            // Compare crossings within one scanline, then handle rollover.
            let old_h = self.h;
            let line = self.ppu.line_clocks();
            // Fullsnes H/V Events: H-only (including zero) and nonzero HV
            // assert 3.5 timer units after comparison: 4*HTIME+14 master
            // clocks. Combined zero uses the separate ten-clock event below.
            // Issue #105 PROVISIONAL / HARDWARE UNVERIFIED: H-only keeps
            // that offset even when outside the line; the crossing guard
            // below suppresses it, with no early fallback or rollover carry.
            // Combined late comparisons retain their approximate fallback.
            let h_irq = if mode == 0x10 || (htime != 0 && htime + 14 < line) {
                htime + 14
            } else {
                htime
            };
            let mut elapsed = n.min(line - self.h);
            #[cfg(test)]
            {
                elapsed = elapsed.min(self.raster_quantum);
            }
            if let Some(next) = self.auto_start {
                elapsed = elapsed.min(next - self.clocks);
            }
            if let Some(poll) = &self.auto_poll {
                elapsed = elapsed.min(poll.next - self.clocks);
            }
            self.clocks += elapsed;
            self.h += elapsed;
            n -= elapsed;
            self.auto_joypad_event();
            // Fullsnes detailed H/V Events: FIELD changes at V=0/H=1,
            // four master clocks after raster rollover. Initial field zero
            // is a reset convention; do not toggle during the initial line.
            if self.frames != 0 && self.v == 0 && old_h < 4 && self.h >= 4 {
                self.ppu.field = !self.ppu.field;
            }
            if self.v == 225 && old_h < OAM_RELOAD && self.h >= OAM_RELOAD {
                self.ppu.oam_vblank_reload();
            }
            if self.v == 0 && old_h < HDMA_INIT && self.h >= HDMA_INIT {
                self.hdma_init = true;
            }
            // During run_frame, compose at relevant PPU accesses or at the
            // picture end. STAT77 starts publishing at HBlank, four pixels
            // before the end of the picture; $213E reads in that tail and
            // every PPU write compose first, so no separate HBlank edge is needed.
            if (1..=224).contains(&self.v)
                && (!self.defer_raster || (old_h < PICTURE_END && self.h >= PICTURE_END))
            {
                self.ppu.render_until(self.v as usize - 1, self.h);
            }
            if old_h < HDMA_SCANLINE && self.h >= HDMA_SCANLINE && self.v <= 224 {
                self.hdma_pending = true;
            }
            if mode & 0x10 != 0
                && old_h < h_irq
                && self.h >= h_irq
                && h_irq < line
                && (mode & 0x20 == 0 || self.v == vtime)
                // Initial reset V=0 was not naturally entered. Preserve its
                // zero-timer no-event policy; physical reset timing is unverified.
                && (htime != 0 || self.v != 0 || self.frames != 0)
            {
                self.irq = true;
            }
            // Fullsnes gives combined HTIME=0 the same 2.5-unit offset as
            // V-only, rather than the nonzero horizontal delay. See Issue #101.
            if (mode == 0x20 || (mode == 0x30 && htime == 0))
                && self.v == vtime
                && old_h < V_ONLY_IRQ
                && self.h >= V_ONLY_IRQ
                // Preserve the startup convention: initial V=0 was not entered
                // by raster rollover. Live configuration/reset timing is unverified.
                && (self.v != 0 || self.frames != 0)
            {
                self.irq = true;
            }
            if self.h < line {
                continue;
            }
            self.h = 0;
            self.v += 1;
            if self.v == 262 {
                self.v = 0;
                self.ppu.obj_frame_start();
                self.frames += 1;
                self.nmi_flag = false;
            }
            self.ppu.v = self.v;
            self.ppu.raster_line_start();
            self.ppu.oam_scanline_start(self.v);
            if self.v == 225 {
                self.nmi_flag = true;
                if self.io[0] & 128 != 0 {
                    self.nmi_pending = true;
                }
                // Fullsnes: first H=74.5 dots; later starts on the same
                // 256-clock lattice within H=32.5..95.5, across both fields.
                let earliest = self.clocks + 130;
                self.auto_start = Some(earliest + (254 + 256 - earliest % 256) % 256);
            }
        }
        self.ppu.h = self.h as u16;
        self.ppu.v = self.v;
    }
    fn joypad_latch(&mut self, manual: bool, automatic: bool) {
        if manual || automatic || self.strobe || self.auto_latch {
            self.serial = [self.buttons & 0xfff0, self.buttons2 & 0xfff0];
        }
        self.strobe = manual;
        self.auto_latch = automatic;
    }
    fn joypad_read(&mut self, port: usize) -> u8 {
        if self.strobe || self.auto_latch {
            ([self.buttons, self.buttons2][port] >> 15) as u8
        } else {
            let bit = (self.serial[port] >> 15) as u8;
            self.serial[port] = (self.serial[port] << 1) | 1;
            bit
        }
    }
    fn auto_joypad_event(&mut self) {
        if self.auto_start == Some(self.clocks) {
            self.auto_start = None;
            if self.io[0] & 1 != 0 {
                self.joy = [0; 4];
                self.joypad_latch(self.strobe, true);
                self.auto_poll = Some(AutoJoypad {
                    next: self.clocks + 256,
                    step: 0,
                });
            }
        }
        if self
            .auto_poll
            .as_ref()
            .is_some_and(|p| p.next == self.clocks)
        {
            let mut poll = self.auto_poll.take().unwrap();
            if poll.step == 0 {
                self.joypad_latch(self.strobe, false);
                poll.next += 128;
            } else {
                for port in 0..2 {
                    let bit = self.joypad_read(port);
                    self.joy[port] = (self.joy[port] << 1) | u16::from(bit);
                    self.joy[port + 2] <<= 1; // Standard-pad Data2 is zero.
                }
                poll.next += 256;
            }
            if poll.step < 16 {
                poll.step += 1;
                self.auto_poll = Some(poll);
            }
        }
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
        // Linear ROM pages never overlap I/O, SRAM or low WRAM mirrors.
        if bank & 0xfe != 0x7e {
            if let Some(v) = self.cart.read_rom_page(a) {
                return v;
            }
        }
        if bank == 0x7e || bank == 0x7f {
            return self.wram[a as usize & 0x1ffff];
        }
        if bank & 0x40 == 0 {
            match lo {
                0..=0x1fff => return self.wram[lo],
                0x2100..=0x213f => {
                    // OAMDATA reads advance the priority rotation address.
                    // STAT77 can first see overflow in the HBlank picture
                    // tail if OBJ evaluation was delayed by forced blank.
                    if self.defer_raster
                        && (lo == 0x2138
                            || (lo == 0x213e && (HBLANK..PICTURE_END).contains(&self.h)))
                        && (1..=224).contains(&self.v)
                    {
                        self.ppu.render_until(self.v as usize - 1, self.h);
                    }
                    return self.ppu.read((lo - 0x2100) as u8, self.open);
                }
                0x2140..=0x217f => {
                    self.sync_apu();
                    return self.apu.port_read((lo - 0x2140) & 3);
                }
                0x2180 => {
                    let v = self.wram[self.waddr];
                    self.waddr = (self.waddr + 1) & 0x1ffff;
                    return v;
                }
                0x4016 | 0x4017 => {
                    let port = lo - 0x4016;
                    let v = self.joypad_read(port);
                    // Standard pads leave Data2 unconnected (CPU bit 1 = 0).
                    // JOYSER1 additionally drives its grounded inputs high.
                    let unused = if port == 0 {
                        self.open & 0xfc
                    } else {
                        (self.open & 0xe0) | 0x1c
                    };
                    return unused | (v & 1);
                }
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
                        | u8::from(self.auto_poll.is_some())
                        | (self.open & 0x3e)
                }
                0x4213 => return self.io[1],
                0x4214..=0x4217 => return self.io[lo - 0x4200],
                0x4218..=0x421f => return (self.joy[(lo - 0x4218) / 2] >> (8 * (lo & 1))) as u8,
                0x4300..=0x437f => return self.dma[(lo >> 4) & 7][lo & 15],
                _ => {}
            }
        }
        self.cart.bus_read(a).unwrap_or(self.open)
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
                    // CPU and DMA/HDMA writes must commit after the elapsed
                    // pixels have sampled the former registers and memory.
                    // VMAIN/VMADD/VMDATA cannot alter them: VRAM writes are
                    // blocked while displaying, and pending forced-blank
                    // pixels are black regardless of VRAM.
                    if (1..=224).contains(&self.v) && !(0x2115..=0x2119).contains(&lo) {
                        self.ppu.render_until(self.v as usize - 1, self.h);
                    }
                    self.ppu.write((lo - 0x2100) as u8, v);
                    return;
                }
                0x2140..=0x217f => {
                    self.sync_apu();
                    self.apu.port_write((lo - 0x2140) & 3, v);
                    self.access_limit = 0;
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
                    self.joypad_latch(v & 1 != 0, self.auto_latch);
                    return;
                }
                0x4200..=0x420d => {
                    let old = self.io[lo - 0x4200];
                    self.io[lo - 0x4200] = v;
                    self.invalidate_fast();
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
        if self.h + clocks < 538 || (self.h > 538 && self.h + clocks < self.ppu.line_clocks()) {
            self.tick(clocks);
            return;
        }
        while clocks > 0 {
            let boundary = if self.h < 538 {
                538
            } else {
                self.ppu.line_clocks()
            };
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
        // Both four-clock halves on the `bus_tick`/`tick` fast paths: no
        // refresh, timing event or pending HDMA, with the APU deferred.
        let quick = self.defer_apu
            && !self.apu.chunk_sensitive()
            && !self.hdma_init
            && !self.hdma_pending
            && self.v == self.fast_v
            && self.h + 8
                < if self.h < 538 {
                    self.fast_limit.min(538)
                } else {
                    self.fast_limit
                };
        if quick {
            self.quick_advance(4);
        } else {
            self.bus_tick(4);
        }
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
        if quick {
            self.quick_advance(4);
        } else {
            self.bus_tick(4);
        }
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
    #[inline]
    fn events(&mut self) {
        if self.hdma_init || self.hdma_pending || self.dma_pending != 0 {
            self.service_events();
        }
    }
    fn service_events(&mut self) {
        self.effects += 1;
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
                    if self.hdma_init || self.hdma_pending {
                        self.hdma_events();
                    }
                    // Pattern lengths are powers of two.
                    self.transfer(
                        i,
                        ((d[4] as u32) << 16) | addr as u32,
                        pattern[j & (pattern.len() - 1)],
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
    // Inline the common ROM/WRAM access on the fast timing path; all other
    // cases take the ordinary out-of-line path.
    #[inline(always)]
    fn read(&mut self, a: u32) -> u8 {
        let a = a & 0xffffff;
        let v = if a >> 17 != 0x3f {
            self.cart.read_rom_page(a)
        } else {
            None
        };
        let v = match v {
            Some(v) => v,
            None => match Self::wram_index(a) {
                Some(i) => self.wram[i],
                None => return self.read_slow(a),
            },
        };
        if !self.quick_cycle(self.speed(a)) {
            return self.read_slow(a);
        }
        self.open = v;
        v
    }
    #[inline(always)]
    fn write(&mut self, a: u32, v: u8) {
        let a = a & 0xffffff;
        match Self::wram_index(a) {
            Some(i) if self.quick_cycle(self.speed(a)) => {
                self.effects += 1;
                self.open = v;
                self.wram[i] = v;
            }
            _ => self.write_slow(a, v),
        }
    }
    #[inline(always)]
    fn idle(&mut self) {
        if !self.quick_cycle(6) {
            self.access(6);
        }
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
        assert_eq!(bus.read_raw(0x4212) & 1, 0);
        bus.tick(298);
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

    pub(super) fn timer_bus(mode: u8, htime: u16, vtime: u16) -> SystemBus {
        let cart = Cartridge::load(include_bytes!("../../../tests/roms/smoke.bin")).unwrap();
        let mut bus = SystemBus::new(cart);
        bus.write_raw(0x4207, htime as u8);
        bus.write_raw(0x4208, (htime >> 8) as u8);
        bus.write_raw(0x4209, vtime as u8);
        bus.write_raw(0x420a, (vtime >> 8) as u8);
        bus.write_raw(0x4200, mode);
        bus
    }

    pub(super) fn timeup(bus: &mut SystemBus) -> bool {
        bus.read_raw(0x4211) & 0x80 != 0
    }

    #[test]
    fn horizontal_zero_irq_fires_once_per_scanline_across_three_natural_fields() {
        let mut bus = timer_bus(0x10, 0, 511);
        bus.tick(357367);
        timeup(&mut bus); // before natural field 1 entry, never at H=0
        bus.tick(1);
        for field in 1..=3 {
            for v in 0..262 {
                assert_eq!((bus.frames, bus.v, bus.h), (field, v, 0));
                assert!(!timeup(&mut bus), "no rollover IRQ at {field}/{v}");
                bus.tick(13);
                assert!(!timeup(&mut bus), "early IRQ at {field}/{v}");
                bus.tick(1);
                assert!(timeup(&mut bus), "missing IRQ at {field}/{v}");
                assert!(!timeup(&mut bus), "TIMEUP acknowledges once");
                let length = if field % 2 == 1 && v == 240 {
                    1360
                } else {
                    1364
                };
                bus.tick(length - 14);
                assert!(!timeup(&mut bus), "no duplicate at {field}/{v}");
            }
        }
        assert_eq!((bus.clocks, bus.frames, bus.v, bus.h), (1429464, 4, 0, 0));
    }

    #[test]
    fn v_only_irq_vertical_zero_fires_at_ten_after_natural_wrap() {
        let mut bus = timer_bus(0x20, 0, 0);
        for (frame, start) in [(1, 357368), (2, 714732), (3, 1072100)] {
            bus.tick(start - bus.clocks);
            assert!(!timeup(&mut bus), "early IRQ in frame {frame}");
            bus.tick(9);
            assert!(!timeup(&mut bus));
            bus.tick(1);
            assert!(timeup(&mut bus), "missing IRQ in frame {frame}");
            assert!(!timeup(&mut bus));
            assert_eq!((bus.frames, bus.v, bus.h), (frame, 0, 10));
        }
    }

    fn check_combined_timer_frames(htime: u16, assertion: u64) {
        for vtime in [0, 1] {
            let mut bus = timer_bus(0x30, htime, vtime);
            // Start after the initial frame's comparison, then verify three
            // complete frames, including the lines that must not trigger.
            bus.tick(LINE * vtime as u64 + assertion + 1);
            timeup(&mut bus);
            for frame in 1..=3 {
                bus.tick(if frame % 2 == 1 { 357368 } else { 357364 } - 2);
                assert!(!timeup(&mut bus), "early IRQ at ({htime}, {vtime})");
                bus.tick(1);
                assert!(timeup(&mut bus), "missing IRQ at ({htime}, {vtime})");
                assert_eq!((bus.frames, bus.v, bus.h), (frame, vtime, assertion));
                bus.tick(1);
                assert!(!timeup(&mut bus), "duplicate IRQ at ({htime}, {vtime})");
            }
        }
    }

    #[test]
    fn combined_zero_irq_timers_assert_at_ten_after_rollover() {
        check_combined_timer_frames(0, 10);
    }

    #[test]
    fn combined_nonzero_timers_assert_after_fourteen_clock_delay() {
        check_combined_timer_frames(1, 18);
    }

    #[test]
    fn horizontal_timer_uses_the_scanline_after_rollover() {
        for vtime in [0, 1, 261] {
            let mut bus = timer_bus(0x30, 1, vtime);
            bus.v = 261;
            bus.h = LINE - 2;
            bus.tick(20); // Cross delayed H=18 on V=0 in a single bus access.
            assert_eq!((bus.frames, bus.v, bus.h), (1, 0, 18));
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

#[cfg(test)]
#[path = "machine/ppu_cgram_tests.rs"]
mod ppu_cgram_tests;

#[cfg(test)]
mod joypad_auto_tests;

#[cfg(test)]
#[path = "machine/ntsc_timing_tests.rs"]
mod ntsc_timing_tests;

#[cfg(test)]
#[path = "machine/v_only_irq_tests.rs"]
mod v_only_irq_tests;

#[cfg(test)]
#[path = "machine/horizontal_irq_tests.rs"]
mod horizontal_irq_tests;

#[cfg(test)]
#[path = "machine/combined_zero_irq_tests.rs"]
mod combined_zero_irq_tests;

#[cfg(test)]
#[path = "machine/horizontal_zero_irq_tests.rs"]
mod horizontal_zero_irq_tests;

#[cfg(test)]
#[path = "machine/late_horizontal_irq_tests.rs"]
mod late_horizontal_irq_tests;

#[cfg(test)]
mod raster_tests;

use crate::snapshot::{self, Reader, Value};

snapshot::snapshot_fields!(AutoJoypad, next, step);
snapshot::snapshot_members!(
    SystemBus,
    clocks,
    frames,
    io,
    dma,
    open,
    waddr,
    h,
    v,
    nmi_flag,
    nmi_pending,
    irq,
    buttons,
    buttons2,
    joy,
    serial,
    strobe,
    auto_start,
    auto_poll,
    auto_latch,
    dma_pending,
    hdma_active,
    hdma_line,
    hdma_transfer,
    hdma_pending,
    hdma_init,
    math,
);

impl Machine {
    /// Build a reset machine for the same cartridge, using fresh or supplied SRAM.
    pub fn restart_with_sram(&self, data: Option<&[u8]>) -> Result<Self, String> {
        let mut cart = self.bus.cart.clone();
        if let Some(data) = data {
            if data.len() != cart.sram.len() {
                return Err("Save size does not match cartridge SRAM".into());
            }
            cart.sram.copy_from_slice(data);
        } else {
            cart.blank_sram();
        }
        Ok(Self::powered_on(cart))
    }

    /// Encode the machine after reset or a successful field, without advancing it.
    /// Audio already produced and host trace output are outside the snapshot.
    pub fn save_snapshot(&self) -> Result<Vec<u8>, String> {
        if !self.snapshot_boundary {
            return Err("Save state requires a completed field or a reset machine".into());
        }
        let bus = &self.bus;
        let cart = &bus.cart;
        let mut body = Vec::with_capacity(
            bus.wram.len() + self.framebuffer().len() + cart.sram.len() + 150_000,
        );
        self.cpu.write(&mut body);
        bus.write_members(&mut body);
        body.extend_from_slice(&bus.wram[..]);
        body.extend_from_slice(&cart.sram);
        bus.ppu.write_snapshot(&mut body);
        bus.apu.write_snapshot(&mut body);
        if let Some(chip) = &cart.spc7110 {
            chip.write(&mut body);
        }
        Ok(snapshot::pack(
            cart.rom_hash(),
            cart.mapping.snapshot_id(),
            cart.battery,
            cart.sram.len(),
            &body,
        ))
    }

    /// Decode a candidate using this machine's cartridge. The original remains
    /// available for validation and any required storage work before replacement.
    pub fn prepare_snapshot(&self, file: &[u8]) -> Result<Self, String> {
        let cart = &self.bus.cart;
        let body = snapshot::unpack(
            file,
            cart.rom_hash(),
            cart.mapping.snapshot_id(),
            cart.battery,
            cart.sram.len(),
        )?;
        let mut input = Reader(&body);
        let cpu = Cpu::read(&mut input)?;
        if !cpu.valid_mode() {
            return Err(snapshot::invalid());
        }
        let mut bus = SystemBus::new(cart.clone());
        bus.read_members(&mut input)?;
        bus.wram.copy_from_slice(input.take(0x20000)?);
        let size = cart.sram.len();
        bus.cart.sram.copy_from_slice(input.take(size)?);
        bus.ppu.read_snapshot(&mut input)?;
        bus.apu.read_snapshot(&mut input)?;
        if bus.cart.spc7110.is_some() {
            let chip = Value::read(&mut input)?;
            if !crate::spc7110::Spc7110::valid(&chip) {
                return Err(snapshot::invalid());
            }
            bus.cart.spc7110 = Some(chip);
        }
        if !input.0.is_empty()
            || cpu.stopped
            || cpu.instructions >= u64::MAX / 2
            || bus.waddr >= bus.wram.len()
            || bus.clocks >= u64::MAX / 2
            || bus.frames > bus.clocks / 350_000
            || bus.v != bus.ppu.v
            || bus.h != u64::from(bus.ppu.h)
            || (bus.buttons | bus.buttons2) & 15 != 0
            || bus
                .auto_start
                .is_some_and(|next| next <= bus.clocks || next - bus.clocks > 385)
            || bus.auto_poll.as_ref().is_some_and(|poll| {
                poll.step > 16 || poll.next <= bus.clocks || poll.next - bus.clocks > 256
            })
            || bus
                .math
                .is_some_and(|(cycles, _, _)| !(1..=16).contains(&cycles))
        {
            return Err(snapshot::invalid());
        }
        // Reconstruct from ordinary slow-path defaults. These caches are
        // pure event bounds or probes for skipping equivalent execution.
        bus.apu.trace_limit = self.bus.apu.trace_limit;
        Ok(Self {
            cpu,
            bus,
            probe: None,
            snapshot_boundary: true,
        })
    }

    /// Validate before replacing any current state.
    pub fn load_snapshot(&mut self, file: &[u8]) -> Result<(), String> {
        *self = self.prepare_snapshot(file)?;
        Ok(())
    }
}
