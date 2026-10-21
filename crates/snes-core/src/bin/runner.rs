use snes_core::{frame_hash, Machine};
use std::{
    env, fs, process,
    time::{Duration, Instant},
};
fn number(s: &str) -> Result<u32, String> {
    if let Some(s) = s.strip_prefix("0x") {
        u32::from_str_radix(s, 16)
    } else {
        s.parse()
    }
    .map_err(|_| format!("Invalid number: {s}"))
}
fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let path=args.next().ok_or("Usage: snes-run ROM [--frames N] [--timeout SECONDS] [--replay CSV] [--ppm FILE] [--expect ADDRESS=BYTE] [--expect-pc ADDRESS] [--dump ADDRESS,LENGTH] [--sram FILE] [--trace N]")?;
    let mut frames = 60;
    let mut timeout = 30;
    let mut replay = Vec::new();
    let mut ppm = None;
    let mut expects = Vec::new();
    let mut save = None;
    let mut trace = 0;
    let mut dump = None;
    let mut expect_pc = None;
    while let Some(arg) = args.next() {
        let v = args
            .next()
            .ok_or_else(|| format!("Missing value for {arg}"))?;
        match arg.as_str() {
            "--dump" => {
                let (a, n) = v.split_once(',').ok_or("Expected ADDRESS,LENGTH")?;
                let a = number(a)?;
                let n = number(n)?;
                if a > 0xffffff || n > 256 {
                    return Err("Dump limited to 256 bytes in SNES address space".into());
                }
                dump = Some((a, n));
            }
            "--expect-pc" => expect_pc = Some(number(&v)?),
            "--frames" => frames = number(&v)?,
            "--timeout" => timeout = number(&v)?,
            "--trace" => trace = number(&v)?.min(1000),
            "--ppm" => ppm = Some(v),
            "--sram" => save = Some(v),
            "--replay" => {
                let s = fs::read_to_string(v).map_err(|e| e.to_string())?;
                for (line, row) in s.lines().enumerate() {
                    let row = row.split('#').next().unwrap().trim();
                    if row.is_empty() {
                        continue;
                    }
                    let (a, b) = row.split_once(',').ok_or_else(|| {
                        format!("Replay line {}: expected frame,buttons", line + 1)
                    })?;
                    let frame = number(a.trim())?;
                    let buttons = number(b.trim())?;
                    if buttons > 65535 {
                        return Err("Buttons exceed 16 bits".into());
                    }
                    if replay.last().is_some_and(|(n, _)| *n >= frame) {
                        return Err("Replay frames must increase strictly".into());
                    }
                    replay.push((frame, buttons as u16));
                }
            }
            "--expect" => {
                let (a, b) = v.split_once('=').ok_or("Expected ADDRESS=BYTE")?;
                let a = number(a)?;
                let b = number(b)?;
                if a > 0xffffff || b > 255 {
                    return Err("Expectation outside SNES address/byte range".into());
                }
                expects.push((a, b as u8));
            }
            _ => return Err(format!("Unknown option: {arg}")),
        }
    }
    if frames == 0 || frames > 1_000_000 || timeout == 0 || timeout > 86400 {
        return Err("Frames must be 1..1000000 and timeout 1..86400 seconds".into());
    }
    let rom = fs::read(path).map_err(|e| e.to_string())?;
    let mut m = Machine::new(&rom)?;
    m.bus.apu.trace_limit = trace as usize;
    if let Some(path) = &save {
        match fs::read(path) {
            Ok(s) => m.load_sram(&s)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    let start = Instant::now();
    let mut input = 0;
    let mut hashes = std::collections::BTreeSet::new();
    let mut energy = 0f64;
    let mut samples = 0u64;
    let mut nonzero = 0u64;
    let mut history = std::collections::VecDeque::new();
    for frame in 0..frames {
        if input < replay.len() && replay[input].0 == frame {
            m.set_buttons(replay[input].1);
            input += 1;
        }
        let target = m.bus.frames + 1;
        let deadline = m.bus.clocks + 400_000;
        while m.bus.frames < target {
            if trace > 0 {
                if history.len() >= trace as usize {
                    history.pop_front();
                }
                history.push_back(format!(
                    "{:02x}:{:04x} A={:04x} X={:04x} Y={:04x} S={:04x} D={:04x} P={:02x} E={} t={}",
                    m.cpu.pb,
                    m.cpu.pc,
                    m.cpu.a,
                    m.cpu.x,
                    m.cpu.y,
                    m.cpu.s,
                    m.cpu.d,
                    m.cpu.p,
                    m.cpu.e,
                    m.bus.clocks
                ));
            }
            m.step();
            if m.cpu.stopped
                || (m.bus.clocks > deadline && m.bus.frames < target)
                || m.bus.apu.error.is_some()
            {
                for row in &history {
                    eprintln!("{row}");
                }
                return Err(format!(
                    "Execution stopped at frame {frame}: CPU {:?}; SPC {:?}; {}",
                    m.cpu,
                    m.bus.apu.cpu,
                    m.bus.apu.error.as_deref().unwrap_or("STP/frame deadline")
                ));
            }
            if m.cpu.instructions % 16384 == 0
                && start.elapsed() > Duration::from_secs(timeout as u64)
            {
                return Err(format!("Wall timeout at frame {frame}, CPU {:?}", m.cpu));
            }
        }
        if start.elapsed() > Duration::from_secs(timeout as u64) {
            return Err(format!("Wall timeout at frame {frame}"));
        }
        let hash = frame_hash(m.framebuffer());
        hashes.insert(hash);
        for v in m.take_audio() {
            samples += 1;
            energy += (v as f64).powi(2);
            if v != 0.0 {
                nonzero += 1;
            }
        }
        if frame % 60 == 0 || frame + 1 == frames {
            println!("{{\"frame\":{frame},\"hash\":\"{hash:016x}\",\"pc\":\"{:02x}:{:04x}\",\"clocks\":{}}}",m.cpu.pb,m.cpu.pc,m.bus.clocks);
        }
    }
    if let Some((a, n)) = dump {
        let values: Vec<_> = (0..n).map(|i| m.bus.peek((a + i) & 0xffffff)).collect();
        println!("{{\"dump_address\":{a},\"bytes\":{values:?}}}");
    }
    if trace > 0 {
        for row in &history {
            eprintln!("{row}");
        }
        for row in &m.bus.apu.trace {
            eprintln!("SPC {row}");
        }
        eprintln!(
            "SPC {:?}; ports in={:?} out={:?}",
            m.bus.apu.cpu, m.bus.apu.input, m.bus.apu.output
        );
    }
    if let Some(expected) = expect_pc {
        let observed = ((m.cpu.pb as u32) << 16) | m.cpu.pc as u32;
        if observed != expected {
            return Err(format!(
                "Terminal PC: expected ${expected:06x}, observed ${observed:06x}"
            ));
        }
    }
    for (a, v) in expects {
        let actual = m.bus.peek(a);
        if actual != v {
            return Err(format!(
                "Marker ${a:06x}: expected ${v:02x}, observed ${actual:02x}; CPU {:?}",
                m.cpu
            ));
        }
    }
    if let Some(path) = ppm {
        let mut data = b"P6\n256 224\n255\n".to_vec();
        for p in m.framebuffer().as_chunks::<4>().0 {
            data.extend_from_slice(&p[..3]);
        }
        fs::write(path, data).map_err(|e| e.to_string())?;
    }
    if let Some(path) = save {
        fs::write(path, &m.bus.cart.sram).map_err(|e| e.to_string())?;
    }
    println!("{{\"frames\":{frames},\"distinct_hashes\":{},\"audio_samples\":{samples},\"nonzero_samples\":{nonzero},\"audio_rms\":{},\"elapsed_ms\":{}}}",hashes.len(),if samples==0{0.0}else{(energy/samples as f64).sqrt()},start.elapsed().as_millis());
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("snes-run: {error}");
        process::exit(1);
    }
}
