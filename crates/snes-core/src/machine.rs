use crate::{
    apu::Apu,
    cartridge::Cartridge,
    cpu::{Bus, Cpu},
    ppu::Ppu,
};
const LINE: u64 = 1364;
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
    dma_pending: u8,
    hdma_active: u8,
    hdma_line: [u8; 8],
    hdma_transfer: [bool; 8],
    hdma_pending: bool,
    hdma_init: bool,
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
            io: [0; 32],
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
            dma_pending: 0,
            hdma_active: 0,
            hdma_line: [0; 8],
            hdma_transfer: [false; 8],
            hdma_pending: false,
            hdma_init: false,
        }
    }
    fn tick(&mut self, mut n: u64) {
        self.apu.tick(n);
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
            if old_h < 1096 && self.h >= 1096 && self.v <= 224 {
                if self.v > 0 {
                    self.ppu.render_line(self.v as usize - 1);
                }
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
                self.hdma_init = true;
            }
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
                    self.joy = self.buttons;
                    self.auto_until = self.clocks + 4224;
                }
            }
        }
        self.ppu.h = self.h as u16;
        self.ppu.v = self.v;
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
                        | if self.h >= 1096 { 64 } else { 0 }
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
                        0x4203 => {
                            let r = self.io[2] as u16 * v as u16;
                            self.io[22] = r as u8;
                            self.io[23] = (r >> 8) as u8;
                        }
                        0x4206 => {
                            let n = u16::from_le_bytes([self.io[4], self.io[5]]);
                            let (q, r) = if v == 0 {
                                (65535, n)
                            } else {
                                (n / v as u16, n % v as u16)
                            };
                            self.io[20] = q as u8;
                            self.io[21] = (q >> 8) as u8;
                            self.io[22] = r as u8;
                            self.io[23] = (r >> 8) as u8;
                        }
                        0x420b => self.dma_pending = v,
                        _ => {}
                    }
                    return;
                }
                0x4300..=0x437f => {
                    self.dma[(lo >> 4) & 7][lo & 15] = v;
                    return;
                }
                _ => {}
            }
        }
        self.cart.write(a, v);
    }
    fn transfer(&mut self, channel: usize, a: u32, offset: u8) {
        let d = self.dma[channel];
        let b = 0x2100 | d[1].wrapping_add(offset) as u32;
        if d[0] & 128 == 0 {
            let v = self.read_raw(a);
            self.write_raw(b, v);
        } else {
            let v = self.read_raw(b);
            self.write_raw(a, v);
        }
        self.tick(8);
    }
    fn events(&mut self) {
        if self.hdma_init {
            self.hdma_init = false;
            self.hdma_active = self.io[12];
            for i in 0..8 {
                self.dma[i][8] = self.dma[i][2];
                self.dma[i][9] = self.dma[i][3];
                self.hdma_line[i] = 0;
                self.hdma_transfer[i] = true;
            }
        }
        if self.dma_pending != 0 {
            let pending = std::mem::take(&mut self.dma_pending);
            self.tick(8);
            for i in 0..8 {
                if pending & (1 << i) == 0 {
                    continue;
                }
                let d = self.dma[i];
                let pattern = DMA_PATTERN[(d[0] & 7) as usize];
                let count = u16::from_le_bytes([d[5], d[6]]);
                let count = if count == 0 { 65536 } else { count as usize };
                let mut addr = u16::from_le_bytes([d[2], d[3]]);
                self.tick(8);
                for j in 0..count {
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
        if self.hdma_pending {
            self.hdma_pending = false;
            for i in 0..8 {
                if self.hdma_active & (1 << i) == 0 || self.io[12] & (1 << i) == 0 {
                    continue;
                }
                if self.hdma_line[i] & 127 == 0 {
                    let counter = self.hdma_read(i);
                    self.hdma_line[i] = counter;
                    self.hdma_transfer[i] = true;
                    if counter == 0 {
                        self.hdma_active &= !(1 << i);
                        continue;
                    }
                    if self.dma[i][0] & 64 != 0 {
                        let l = self.hdma_read(i);
                        let h = self.hdma_read(i);
                        self.dma[i][5] = l;
                        self.dma[i][6] = h;
                    }
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
                self.hdma_line[i] = self.hdma_line[i].wrapping_sub(1);
                self.dma[i][10] = self.hdma_line[i];
                self.hdma_transfer[i] = self.hdma_line[i] & 128 != 0;
            }
        }
    }
    fn hdma_read(&mut self, i: usize) -> u8 {
        let addr = u16::from_le_bytes([self.dma[i][8], self.dma[i][9]]);
        let a = ((self.dma[i][4] as u32) << 16) | addr as u32;
        let v = self.read_raw(a);
        let next = addr.wrapping_add(1);
        self.dma[i][8] = next as u8;
        self.dma[i][9] = (next >> 8) as u8;
        self.tick(8);
        v
    }
}
impl Bus for SystemBus {
    fn read(&mut self, a: u32) -> u8 {
        self.tick(self.speed(a));
        let v = self.read_raw(a);
        self.open = v;
        v
    }
    fn write(&mut self, a: u32, v: u8) {
        self.tick(self.speed(a));
        self.open = v;
        self.write_raw(a, v);
    }
    fn idle(&mut self) {
        self.tick(6);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
