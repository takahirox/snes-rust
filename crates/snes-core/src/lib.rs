pub mod apu;
pub mod cartridge;
pub mod cpu;
pub mod dsp;
mod gaussian;
pub mod machine;
pub mod ppu;
pub use machine::Machine;
/// Stable FNV-1a digest for deterministic frame regression logs (not a security hash).
pub fn frame_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, &v| {
        (h ^ v as u64).wrapping_mul(0x100000001b3)
    })
}
