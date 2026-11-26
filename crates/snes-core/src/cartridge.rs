//! Ordinary LoROM/HiROM cartridges. No enhancement-chip implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mapping {
    LoRom,
    HiRom,
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
}
impl Mapping {
    pub(crate) fn snapshot_id(self) -> u8 {
        match self {
            Self::LoRom => 0,
            Self::HiRom => 1,
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
            return Err("Expected a 32 KiB–4 MiB ordinary LoROM/HiROM image".into());
        }
        let score = |h: usize, high: bool| -> i32 {
            if h + 64 > image.len() {
                return -100;
            }
            let m = image[h + 21];
            let reset = u16::from_le_bytes([image[h + 60], image[h + 61]]);
            let sum = u16::from_le_bytes([image[h + 28], image[h + 29]]);
            let inv = u16::from_le_bytes([image[h + 30], image[h + 31]]);
            (if (m & 0x2f) == (if high { 0x21 } else { 0x20 }) {
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
        if chip > 2 {
            return Err(format!("Unsupported cartridge hardware type ${chip:02x}"));
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
            mapping,
            battery: chip == 2,
            pages: Vec::new(),
        };
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
        if let Some(i) = self.sram_index(a) {
            return Some(self.sram[i]);
        }
        self.rom_index(a).map(|i| self.rom[i])
    }
    pub fn write(&mut self, a: u32, v: u8) {
        if let Some(i) = self.sram_index(a) {
            self.sram[i] = v;
        }
    }
}
// Address-line mirroring also handles non-power-of-two ROM sizes.
fn mirror(mut a: usize, mut size: usize) -> usize {
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
