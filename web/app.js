const $ = (id) => document.getElementById(id);
const canvas = document.querySelector('canvas');
const context = canvas.getContext('2d', { alpha: false });
let image = context.createImageData(256, 224);
const decoder = new TextDecoder();
let core, loaded = false, playing = false, key = '', audio, nextAudio = 0, lastTime = 0, debt = 0, saveClock = 0;
let keyboard = 0;
let operationError = '', persistenceError = '', restorePending = false;
const sources = new Set();
const frameMilliseconds = 1000 * 1364 * 262 / 21477272;
const audioLead = .04, maxAudioLead = .15, maxSources = 12;
const keys = { ArrowUp: 0x800, ArrowDown: 0x400, ArrowLeft: 0x200, ArrowRight: 0x100, KeyZ: 0x8000, KeyX: 0x80, KeyA: 0x4000, KeyS: 0x40, KeyQ: 0x20, KeyW: 0x10, Enter: 0x1000, ShiftLeft: 0x2000, ShiftRight: 0x2000 };
const held = new Set();
const bytes = (pointer, length) => new Uint8Array(core.memory.buffer, pointer, length);
function check(ok) { if (!ok) throw new Error(decoder.decode(bytes(core.error_buffer(), core.error_len())) || 'Emulator operation failed'); }
function copyInput(data) { const pointer = core.input_buffer(data.length); if (!pointer) throw new Error('ROM exceeds the supported size'); bytes(pointer, data.length).set(data); }
function draw() {
  const width = core.framebuffer_width(), height = core.framebuffer_height();
  if (image.width !== width || image.height !== height) {
    canvas.width = width; canvas.height = height;
    image = context.createImageData(width, height);
  }
  const length = core.framebuffer_len(), pointer = core.framebuffer();
  if (length !== width * height * 4) throw new Error('Invalid framebuffer geometry');
  // Reacquire memory after core calls, including frame growth/reset/replacement.
  image.data.set(bytes(pointer, length)); context.putImageData(image, 0, 0);
}
function showErrors() { $('error').textContent = [operationError, persistenceError].filter(Boolean).join('\n'); }
function report(error) { operationError = error.message || String(error); showErrors(); }
function clearError() { operationError = ''; showErrors(); }
function save() {
  if (!loaded || !key || !core.battery() || !core.sram_len()) return true;
  if (restorePending) return false;
  try {
    const data = bytes(core.sram_buffer(), core.sram_len()); let encoded = ''; for (const v of data) encoded += String.fromCharCode(v); localStorage.setItem(key, btoa(encoded));
    persistenceError = ''; showErrors(); return true;
  }
  catch (e) { persistenceError = `Could not persist SRAM: ${e.message}. Keep this tab open to retain the current save. Cartridge changes are blocked until saving succeeds.`; showErrors(); return false; }
}
function restoreSave() {
  // Block writes until restoration completes, including corrupt/wrong-size saves.
  restorePending = true;
  try {
    const saved = localStorage.getItem(key);
    if (saved !== null) { copyInput(Uint8Array.from(atob(saved), (c) => c.charCodeAt(0))); check(core.load_sram()); }
    restorePending = false; persistenceError = '';
  } catch (e) {
    persistenceError = `Save could not be restored: ${e.message}. Saving and cartridge changes are blocked. Retry restoring the save when storage is available.`;
  }
  $('retry-save').hidden = !restorePending;
  $('play').disabled = restorePending; $('reset').disabled = restorePending;
  showErrors();
}
$('retry-save').addEventListener('click', () => { restoreSave(); if (!restorePending) { draw(); $('status').textContent = 'Save restored — click Play'; } });
function flushAudio() { for (const source of sources) source.stop(); sources.clear(); nextAudio = 0; }
function pause() { playing = false; keyboard = 0; held.clear(); $('play').textContent = 'Play'; flushAudio(); const saved = save(); if (loaded) $('status').textContent = 'Paused'; return saved; }
async function activateAudio() {
  if (!$('sound').checked) return;
  try { audio ??= new AudioContext({ sampleRate: 32000 }); await audio.resume(); }
  catch (e) { report(new Error(`Audio could not start: ${e.message}`)); }
}
function queueAudio() {
  if (!audio || audio.state !== 'running' || !$('sound').checked) return;
  const count = core.audio_len(); if (!count) return;
  const samples = new Float32Array(core.memory.buffer, core.audio_buffer(), count);
  const buffer = audio.createBuffer(2, count / 2, 32000);
  for (let channel = 0; channel < 2; channel++) { const data = buffer.getChannelData(channel); for (let i = 0; i < data.length; i++) data[i] = samples[i * 2 + channel]; }
  const now = audio.currentTime;
  if (nextAudio < now || nextAudio + buffer.duration > now + maxAudioLead || sources.size >= maxSources) { flushAudio(); nextAudio = now + audioLead; }
  const source = audio.createBufferSource(); source.buffer = buffer; source.connect(audio.destination); sources.add(source);
  source.onended = () => sources.delete(source); source.start(nextAudio); nextAudio += buffer.duration;
}
function buttons() {
  let value = keyboard;
  const map = [0x8000, 0x80, 0x4000, 0x40, 0x20, 0x10, 0, 0, 0x2000, 0x1000, 0, 0, 0x800, 0x400, 0x200, 0x100];
  for (const pad of navigator.getGamepads?.() ?? []) {
    if (!pad || pad.mapping !== 'standard') continue;
    for (let i = 0; i < map.length; i++) if (pad.buttons[i]?.pressed) value |= map[i];
    if (pad.axes[0] < -.5) value |= 0x200; if (pad.axes[0] > .5) value |= 0x100;
    if (pad.axes[1] < -.5) value |= 0x800; if (pad.axes[1] > .5) value |= 0x400;
  }
  return value;
}
function animate(time) {
  requestAnimationFrame(animate);
  if (!playing) { lastTime = time; return; }
  // Discard stale wall time after a stall. Catch-up stays within three frames;
  // video and audio restart together instead of accumulating an unbounded debt.
  debt = Math.min(debt + Math.max(time - lastTime, 0), frameMilliseconds * 3); lastTime = time;
  try {
    let frames = 0;
    while (debt >= frameMilliseconds && frames < 3) { check(core.run_frame(buttons())); queueAudio(); debt -= frameMilliseconds; frames++; }
    if (frames) draw();
    if (time - saveClock > 5000) { save(); saveClock = time; }
  } catch (e) { pause(); report(e); }
}
$('rom').addEventListener('change', async (event) => {
  const file = event.target.files[0]; if (!file) return;
  $('rom').disabled = true;
  try {
    if (!pause()) return;
    clearError();
    if (!core) throw new Error('Emulator has not loaded');
    if (!/\.(sfc|smc)$/i.test(file.name)) throw new Error('Choose an .sfc or .smc cartridge image');
    if (file.size > 4 * 1024 * 1024 + 512) throw new Error('Maximum ROM size is 4 MiB plus a copier header');
    let data = new Uint8Array(await file.arrayBuffer()); if (data.length % 1024 === 512) data = data.subarray(512);
    const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', data));
    const newKey = `snes-rust:sram:${Array.from(digest, (b) => b.toString(16).padStart(2, '0')).join('')}`;
    // Reading/hashing is asynchronous: save again at the replacement boundary
    // in case playback or storage availability changed while loading the file.
    if (!pause()) return;
    copyInput(data); check(core.load_rom()); key = newKey; loaded = true;
    $('play').disabled = false; $('reset').disabled = false;
    if (core.battery() && core.sram_len()) restoreSave();
    $('status').textContent = restorePending ? `${file.name} loaded — restore save before playing` : `${file.name} loaded — click Play`; draw();
  } catch (e) { report(e); }
  finally { $('rom').disabled = false; event.target.value = ''; }
});
$('play').addEventListener('click', async () => {
  if (!loaded || restorePending) return;
  if (playing) { pause(); return; }
  clearError(); await activateAudio();
  if (restorePending) return;
  debt = frameMilliseconds; lastTime = performance.now(); playing = true; $('play').textContent = 'Pause'; $('status').textContent = 'Playing';
});
$('reset').addEventListener('click', () => { pause(); try { check(core.reset()); draw(); clearError(); $('status').textContent = 'Reset — SRAM preserved'; } catch (e) { report(e); } });
$('sound').addEventListener('change', () => { flushAudio(); if ($('sound').checked) void activateAudio(); });
$('scale').addEventListener('change', () => { const scale = Number($('scale').value); canvas.style.width = `${256 * scale}px`; });
for (const type of ['keydown', 'keyup']) window.addEventListener(type, (event) => {
  if (!keys[event.code] || !playing) return;
  if (['INPUT', 'SELECT'].includes(event.target.tagName) && !(type === 'keyup' && held.has(event.code))) return;
  event.preventDefault(); if (type === 'keydown') held.add(event.code); else held.delete(event.code);
  keyboard = [...held].reduce((value, code) => value | keys[code], 0);
});
window.addEventListener('blur', () => { keyboard = 0; held.clear(); });
document.addEventListener('visibilitychange', () => { if (document.hidden && playing) pause(); });
window.addEventListener('pagehide', save);
try {
  const response = await fetch('snes_wasm.wasm'); if (!response.ok) throw new Error(`Wasm download failed: HTTP ${response.status}`);
  const result = await WebAssembly.instantiate(await response.arrayBuffer(), {}); core = result.instance.exports;
  $('status').textContent = 'Open a local SNES ROM to begin'; requestAnimationFrame(animate);
} catch (e) { report(e); $('status').textContent = 'Emulator unavailable'; $('rom').disabled = true; }
