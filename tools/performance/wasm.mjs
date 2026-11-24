// Node runs the production Wasm ABI without rendering or Web Audio overhead.
import { readFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';

const [kind, binary, romPath, replay, warm, count] = process.argv.slice(2);
const warmup = Number(warm), frames = Number(count);
const [rom, text] = await Promise.all([readFile(romPath), readFile(replay, 'utf8')]);
const masks = text.trim().split('\n').map(Number);
if (frames <= 0 || masks.length < warmup + frames) throw new Error('Invalid frame window');

function digest(bytes) {
  let hash = 2166136261;
  for (const byte of bytes) hash = Math.imul(hash ^ byte, 16777619);
  return hash >>> 0;
}

const compileStart = performance.now();
let load, step, evidence, audioCount, finish, videoFrames;
if (kind === 'rust') {
  const { instance: { exports: core } } = await WebAssembly.instantiate(await readFile(binary), {});
  load = () => {
    const address = core.input_buffer(rom.length);
    if (!address) throw new Error('ROM buffer unavailable');
    new Uint8Array(core.memory.buffer, address, rom.length).set(rom);
    if (!core.load_rom()) throw new Error('Unable to load cartridge');
  };
  step = mask => {
    if (!core.run_frame(mask)) throw new Error('Guest frame failed');
  };
  audioCount = () => core.audio_len() / 2;
  evidence = () => ({
    width: core.framebuffer_width(), height: core.framebuffer_height(),
    video: digest(new Uint8Array(core.memory.buffer, core.framebuffer(), core.framebuffer_len())),
    sound: digest(new Uint8Array(core.memory.buffer, core.audio_buffer(), core.audio_len() * 4)),
    wram: digest((function* () {
      for (let address = 0x7e0000; address < 0x800000; address++) yield core.peek(address);
    })()),
    sram: digest(new Uint8Array(core.memory.buffer, core.sram_buffer(), core.sram_len())),
  });
} else if (kind === 'snes9x') {
  const { default: instantiate } = await import(pathToFileURL(binary));
  const ref = await instantiate();
  load = () => {
    const ptr = ref._malloc(rom.length);
    ref.HEAPU8.set(rom, ptr);
    try {
      if (!ref._bench_load(ptr, rom.length)) throw new Error('Reference rejected NTSC cartridge');
    } finally {
      ref._free(ptr);
    }
  };
  step = (mask, checkpoint) => ref._bench_frame(mask, Number(checkpoint));
  let previous = 0;
  audioCount = () => {
    const frames = ref._bench_audio_frames();
    const delta = frames - previous;
    previous = frames;
    return delta;
  };
  evidence = () => ({
    width: ref._bench_width(), height: ref._bench_height(),
    video: ref._bench_video() >>> 0, wram: ref._bench_wram() >>> 0,
    audio_energy: ref._bench_audio_energy(),
  });
  finish = () => {
    videoFrames = ref._bench_video_frames();
    ref._bench_unload();
  };
} else {
  throw new Error('Expected rust or snes9x');
}
const compileMs = performance.now() - compileStart;
const loadStart = performance.now();
load();
const loadMs = performance.now() - loadStart;
let ms = 0, audioFrames = 0;
for (let index = 0; index < warmup + frames; index++) {
  const measured = index >= warmup;
  const checkpoint = measured && ((index - warmup + 1) % 120 === 0 || index + 1 === warmup + frames);
  const start = performance.now();
  step(masks[index], checkpoint);
  const elapsed = performance.now() - start;
  const audio = audioCount();
  if (measured) {
    ms += elapsed;
    audioFrames += audio;
    if (checkpoint) {
      console.log(JSON.stringify({ frame: index + 1, ...evidence() }));
    }
  }
}
finish?.();
console.log(JSON.stringify({
  frames, warmup, ms, audio_frames: audioFrames, video_frames: videoFrames,
  load_ms: loadMs, compile_ms: compileMs, rss_bytes: process.memoryUsage().rss,
  sha256: createHash('sha256').update(rom).digest('hex'),
}));
