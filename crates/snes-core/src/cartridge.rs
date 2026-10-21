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
}
impl Cartridge {
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
        Ok(Self {
            rom: image.to_vec(),
            sram: vec![0; if size == 0 { 0 } else { 1024usize << size }],
            title: String::from_utf8_lossy(&image[h..h + 21])
                .trim()
                .to_string(),
            mapping,
            battery: chip == 2,
        })
    }
    fn sram_index(&self, a: u32) -> Option<usize> {
        let bank = (a >> 16) as usize;
        let lo = (a & 0xffff) as usize;
        if self.sram.is_empty() {
            return None;
        }
        let i = match self.mapping {
            Mapping::LoRom if (bank & 0x7f) >= 0x70 && (bank & 0x7f) <= 0x7d && lo < 0x8000 => {
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
    pub fn read(&self, a: u32) -> Option<u8> {
        if let Some(i) = self.sram_index(a) {
            return Some(self.sram[i]);
        }
        let bank = (a >> 16) as usize;
        let lo = (a & 0xffff) as usize;
        let i = match self.mapping {
            Mapping::LoRom if lo >= 0x8000 || bank & 0x7f >= 0x40 => {
                ((bank & 0x7f) << 15) | (lo & 0x7fff)
            }
            Mapping::HiRom if lo >= 0x8000 || bank & 0x7f >= 0x40 => ((bank & 0x3f) << 16) | lo,
            _ => return None,
        };
        Some(self.rom[mirror(i, self.rom.len())])
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
