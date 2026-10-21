//! Dependency-free WebAssembly ABI. Buffers are owned by Rust; JavaScript copies
//! into the input buffer and reads output views only until the next core call.
use snes_core::Machine;
use std::cell::RefCell;
#[derive(Default)]
struct State {
    input: Vec<u8>,
    machine: Option<Machine>,
    error: Vec<u8>,
    audio: Vec<f32>,
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
fn perform(f: impl FnOnce(&mut State) -> Result<(), String>) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        match f(&mut s) {
            Ok(()) => {
                s.error.clear();
                1
            }
            Err(e) => {
                s.error = e.into_bytes();
                0
            }
        }
    })
}
#[no_mangle]
pub extern "C" fn input_buffer(len: u32) -> usize {
    if len > 4 * 1024 * 1024 + 512 {
        return 0;
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.input.resize(len as usize, 0);
        s.input.as_mut_ptr() as usize
    })
}
#[no_mangle]
pub extern "C" fn load_rom() -> u32 {
    perform(|s| {
        let m = Machine::new(&s.input)?;
        s.machine = Some(m);
        s.audio.clear();
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn reset() -> u32 {
    perform(|s| {
        s.machine.as_mut().ok_or("Load a cartridge first")?.reset();
        s.audio.clear();
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn run_frame(buttons: u32) -> u32 {
    perform(|s| {
        let m = s.machine.as_mut().ok_or("Load a cartridge first")?;
        m.set_buttons(buttons as u16);
        m.run_frame()?;
        s.audio = m.take_audio();
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn framebuffer() -> usize {
    STATE.with(|s| {
        s.borrow()
            .machine
            .as_ref()
            .map_or(0, |m| m.framebuffer().as_ptr() as usize)
    })
}
#[no_mangle]
pub extern "C" fn audio_buffer() -> usize {
    STATE.with(|s| s.borrow().audio.as_ptr() as usize)
}
#[no_mangle]
pub extern "C" fn audio_len() -> usize {
    STATE.with(|s| s.borrow().audio.len())
}
#[no_mangle]
pub extern "C" fn error_buffer() -> usize {
    STATE.with(|s| s.borrow().error.as_ptr() as usize)
}
#[no_mangle]
pub extern "C" fn error_len() -> usize {
    STATE.with(|s| s.borrow().error.len())
}
#[no_mangle]
pub extern "C" fn sram_buffer() -> usize {
    STATE.with(|s| {
        s.borrow()
            .machine
            .as_ref()
            .map_or(0, |m| m.bus.cart.sram.as_ptr() as usize)
    })
}
#[no_mangle]
pub extern "C" fn sram_len() -> usize {
    STATE.with(|s| {
        s.borrow()
            .machine
            .as_ref()
            .map_or(0, |m| m.bus.cart.sram.len())
    })
}
#[no_mangle]
pub extern "C" fn load_sram() -> u32 {
    perform(|s| {
        s.machine
            .as_mut()
            .ok_or("Load a cartridge first")?
            .load_sram(&s.input)
    })
}
#[no_mangle]
pub extern "C" fn battery() -> u32 {
    STATE.with(|s| {
        u32::from(
            s.borrow()
                .machine
                .as_ref()
                .is_some_and(|m| m.bus.cart.battery),
        )
    })
}

/// Read memory without I/O side effects for deterministic regression markers.
#[no_mangle]
pub extern "C" fn peek(address: u32) -> u32 {
    STATE.with(|s| {
        s.borrow()
            .machine
            .as_ref()
            .map_or(0, |m| m.bus.peek(address & 0xffffff) as u32)
    })
}
