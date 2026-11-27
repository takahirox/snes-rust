//! Ordinary LoROM/HiROM cartridges and the SPC7110 data-decompression board.
use crate::spc7110::{Spc7110, PROGRAM_SIZE};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mapping {
    LoRom,
    HiRom,
    Spc7110,
}
#[derive(Clone)]
pub struct Cartridge {
    pub rom: Vec<u8>,
    pub sram: Vec<u8>,
    pub title: String,
    pub mapping: Mapping,
    pub battery: bool,
    // ROM offset of each 4 KiB page of the 24-bit address space, or
    // `SLOW_PAGE` where `read` must decode SRAM, open bus or a split page.
    pages: Vec<u32>,
    pub(crate) spc7110: Option<Spc7110>,
}
impl Mapping {
    pub(crate) fn snapshot_id(self) -> u8 {
        match self {
            Self::LoRom => 0,
            Self::HiRom => 1,
            Self::Spc7110 => 2,
        }
    }
}

const SLOW_PAGE: u32 = u32::MAX;
impl Cartridge {
    pub(crate) fn rom_hash(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        Sha256::digest(&self.rom).into()
    }

    pub fn load(image: &[u8]) -> Result<Self, String> {
        let image = if image.len() % 1024 == 512 {
            &image[512..]
        } else {
            image
        };
        if image.len() < 0x8000 || image.len() > 0x400000 || image.len() % 0x8000 != 0 {
            return Err("Expected a 32 KiB–4 MiB LoROM, HiROM or SPC7110 image".into());
        }
        let score = |h: usize, high: bool| -> i32 {
            if h + 64 > image.len() {
                return -100;
            }
            let m = image[h + 21];
            let reset = u16::from_le_bytes([image[h + 60], image[h + 61]]);
            let sum = u16::from_le_bytes([image[h + 28], image[h + 29]]);
            let inv = u16::from_le_bytes([image[h + 30], image[h + 31]]);
            (if (m & 0x2f) == (if high { 0x21 } else { 0x20 }) || (high && m == 0x3a) {
                8
            } else {
                -8
            }) + if reset >= 0x8000 { 4 } else { -8 }
                + if sum ^ inv == 0xffff { 2 } else { 0 }
        };
        let (mapping, h) = if score(0xffc0, true) > score(0x7fc0, false) {
            (Mapping::HiRom, 0xffc0)
        } else {
            (Mapping::LoRom, 0x7fc0)
        };
        if score(h, mapping == Mapping::HiRom) < 0 {
            return Err("No supported SNES cartridge header/reset vector".into());
        }
        let chip = image[h + 22];
        // Map mode $3a with type $f5 is SPC7110 + battery SRAM, without RTC.
        let spc7110 = mapping == Mapping::HiRom && image[h + 21] == 0x3a && chip == 0xf5;
        if chip > 2 && !spc7110 {
            return Err(format!("Unsupported cartridge hardware type ${chip:02x}"));
        }
        if spc7110 && image.len() <= PROGRAM_SIZE {
            return Err("SPC7110 image has no data ROM".into());
        }
        if image[h + 25] >= 2 && image[h + 25] <= 12 {
            return Err("PAL timing is not implemented".into());
        }
        let size = image[h + 24];
        if size > 8 {
            return Err("Unsupported SRAM size".into());
        }
        let mut cart = Self {
            rom: image.to_vec(),
            sram: vec![0; if size == 0 { 0 } else { 1024usize << size }],
            title: String::from_utf8_lossy(&image[h..h + 21])
                .trim()
                .to_string(),
            mapping: if spc7110 { Mapping::Spc7110 } else { mapping },
            battery: chip == 2 || spc7110,
            pages: Vec::new(),
            spc7110: spc7110.then(Spc7110::power),
        };
        cart.blank_sram();
        cart.pages = (0..4096u32)
            .map(|page| {
                let base = page << 12;
                let offset = |a| cart.rom_index(a).filter(|_| cart.sram_index(a).is_none());
                match offset(base) {
                    // Keep the page only if it is one linear ROM run.
                    Some(first)
                        if (0..4096)
                            .step_by(64)
                            .chain([4095])
                            .all(|k| offset(base + k) == Some(first + k as usize)) =>
                    {
                        first as u32
                    }
                    _ => SLOW_PAGE,
                }
            })
            .collect();
        Ok(cart)
    }
    /// Clear SRAM to the state of a new cartridge. SPC7110 games run the
    /// board's factory check program instead of the game until its final
    /// step stores this marker at $00:7ff0, as on shipped cartridges.
    pub(crate) fn blank_sram(&mut self) {
        self.sram.fill(0);
        if self.spc7110.is_some() {
            let i = self.sram_index(0x7ff0).unwrap();
            self.sram[i..i + 16].copy_from_slice(b"SPC7110 CHECK OK");
        }
    }
    /// Restore enhancement-chip registers to their power-on state.
    pub(crate) fn power_on(&mut self) {
        if self.spc7110.is_some() {
            self.spc7110 = Some(Spc7110::power());
        }
    }
    fn sram_index(&self, a: u32) -> Option<usize> {
        let bank = (a >> 16) as usize;
        let lo = (a & 0xffff) as usize;
        if self.sram.is_empty() {
            return None;
        }
        let i = match self.mapping {
            Mapping::LoRom
                if ((0x70..=0x7d).contains(&bank) || (0xf0..=0xff).contains(&bank))
                    && lo < 0x8000 =>
            {
                ((bank & 15) << 15) | lo
            }
            Mapping::HiRom
                if bank & 0x7f >= 0x20 && bank & 0x7f <= 0x3f && (0x6000..0x8000).contains(&lo) =>
            {
                ((bank & 31) << 13) | (lo & 0x1fff)
            }
            Mapping::Spc7110 if bank & 0x40 == 0 && (0x6000..0x8000).contains(&lo) => {
                ((bank & 0x3f) << 13) | (lo & 0x1fff)
            }
            _ => return None,
        };
        Some(i % self.sram.len())
    }
    fn rom_index(&self, a: u32) -> Option<usize> {
        let bank = (a >> 16) as usize;
        let lo = (a & 0xffff) as usize;
        let i = match self.mapping {
            Mapping::LoRom if lo >= 0x8000 || bank & 0x7f >= 0x40 => {
                ((bank & 0x7f) << 15) | (lo & 0x7fff)
            }
            Mapping::HiRom if lo >= 0x8000 || bank & 0x7f >= 0x40 => ((bank & 0x3f) << 16) | lo,
            // Only fixed program ROM is linear; other banks follow $4830-$4834.
            Mapping::Spc7110
                if bank & 0x7f < 0x10 && lo >= 0x8000 || (0xc0..0xd0).contains(&bank) =>
            {
                return Some(mirror(lo | (bank & 0x0f) << 16, PROGRAM_SIZE));
            }
            _ => return None,
        };
        Some(mirror(i, self.rom.len()))
    }
    // The byte at `a` when it lies in a linear ROM page.
    #[inline(always)]
    pub(crate) fn read_rom_page(&self, a: u32) -> Option<u8> {
        let page = self.pages[(a >> 12) as usize & 4095];
        (page != SLOW_PAGE).then(|| self.rom[page as usize + (a & 4095) as usize])
    }
    #[inline]
    pub fn read(&self, a: u32) -> Option<u8> {
        let page = self.pages[(a >> 12) as usize & 4095];
        if page != SLOW_PAGE {
            return Some(self.rom[page as usize + (a & 4095) as usize]);
        }
        if let Some(chip) = &self.spc7110 {
            return self.spc7110_read(chip, a);
        }
        if let Some(i) = self.sram_index(a) {
            return Some(self.sram[i]);
        }
        self.rom_index(a).map(|i| self.rom[i])
    }
    // Side-effect-free SPC7110 memory; chip registers read as open bus.
    fn spc7110_read(&self, chip: &Spc7110, a: u32) -> Option<u8> {
        let bank = (a >> 16) as usize;
        if let Some(i) = self.sram_index(a) {
            return Some(if chip.sram_enabled() { self.sram[i] } else { 0 });
        }
        let (prom, drom) = self.rom.split_at(PROGRAM_SIZE);
        (bank >= 0xc0 || bank & 0x40 == 0 && a & 0x8000 != 0)
            .then(|| chip.rom_read((a & 0x3fffff) as usize, prom, drom))
    }
    /// A CPU or DMA read, including chip-register side effects.
    pub(crate) fn bus_read(&mut self, a: u32) -> Option<u8> {
        if let (Some(chip), Some(index)) = (&mut self.spc7110, Spc7110::register(a)) {
            return chip.read(index, &self.rom[PROGRAM_SIZE..]);
        }
        self.read(a)
    }
    pub fn write(&mut self, a: u32, v: u8) {
        if let Some(chip) = &mut self.spc7110 {
            if let Some(index) = Spc7110::register(a) {
                chip.write(index, v, &self.rom[PROGRAM_SIZE..]);
                return;
            }
            if !chip.sram_enabled() {
                return;
            }
        }
        if let Some(i) = self.sram_index(a) {
            self.sram[i] = v;
        }
    }
}
// Address-line mirroring also handles non-power-of-two ROM sizes.
pub(crate) fn mirror(mut a: usize, mut size: usize) -> usize {
    let mut base = 0;
    let mut mask = 1usize << 23;
    while a >= size {
        while a & mask == 0 {
            mask >>= 1;
        }
        a -= mask;
        if size > mask {
            size -= mask;
            base += mask;
        }
        mask >>= 1;
    }
    base + a
}
