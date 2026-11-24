//! Opt-in headless benchmark of the same frame loop used by the Wasm adapter.
use snes_core::Machine;
use std::{error::Error, fs, time::Instant};

fn digest(bytes: impl IntoIterator<Item = u8>) -> u32 {
    bytes.into_iter().fold(2166136261, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(16777619)
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let [rom, replay, warmup, frames]: [String; 4] = std::env::args()
        .skip(1)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "Usage: benchmark ROM MASKS WARMUP FRAMES")?;
    let masks = fs::read_to_string(replay)?
        .lines()
        .map(str::parse::<u16>)
        .collect::<Result<Vec<_>, _>>()?;
    let warmup = warmup.parse::<usize>()?;
    let frames = frames.parse::<usize>()?;
    let end = warmup
        .checked_add(frames)
        .filter(|&end| frames > 0 && end <= masks.len())
        .ok_or("The replay must cover the warm-up and a nonempty measurement window")?;

    let rom = fs::read(rom)?;
    let start = Instant::now();
    let mut machine = Machine::new(&rom)?;
    let load_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut elapsed = 0.0;
    let mut audio_frames = 0;
    let mut warmup_fields = 0;
    for (index, &buttons) in masks[..end].iter().enumerate() {
        let start = Instant::now();
        machine.set_buttons(buttons);
        machine.run_frame()?;
        let audio = machine.take_audio();
        if index + 1 == warmup {
            warmup_fields = machine.bus.frames;
        }
        if index >= warmup {
            elapsed += start.elapsed().as_secs_f64();
            audio_frames += audio.len() / 2;
        }
        if index >= warmup && ((index - warmup + 1) % 120 == 0 || index + 1 == end) {
            let video = digest(machine.framebuffer().iter().copied());
            let sound = digest(audio.iter().flat_map(|sample| sample.to_le_bytes()));
            let wram = digest((0x7e0000..0x800000).map(|address| machine.bus.peek(address)));
            let sram = digest(machine.bus.cart.sram.iter().copied());
            println!(
                "{{\"frame\":{},\"width\":{},\"height\":{},\"video\":{video},\"sound\":{sound},\"wram\":{wram},\"sram\":{sram}}}",
                index + 1, machine.framebuffer_width(), machine.framebuffer_height()
            );
        }
    }
    println!(
        "{{\"frames\":{frames},\"warmup\":{warmup},\"ms\":{},\"load_ms\":{load_ms},\"audio_frames\":{audio_frames},\"fields\":{},\"mapping\":\"{:?}\",\"clocks\":{},\"instructions\":{}}}",
        elapsed * 1000.0,
        machine.bus.frames - warmup_fields,
        machine.bus.cart.mapping,
        machine.bus.clocks,
        machine.cpu.instructions,
    );
    Ok(())
}
