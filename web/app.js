const $ = (id) => document.getElementById(id);
const canvas = document.querySelector('canvas');
const context = canvas.getContext('2d', { alpha: false });
let image = context.createImageData(256, 224);
const decoder = new TextDecoder();
let core, loaded = false, playing = false, key = '', audio, nextAudio = 0, lastTime = 0, debt = 0, saveClock = 0;
let keyboard = 0, activeDemo = '';
const virtualPointers = new Map(), virtualKeys = new Map();
const touchButtons = [...document.querySelectorAll('[data-button]')];
let operationError = '', persistenceError = '', restorePending = false, loading = false, pendingCartridge;
// Bundled demos by picker button id; each is fetched from the same origin and pinned.
const demos = {
  demo: { file: 'shionari.sfc', name: '潮鳴りの密室 / Shionari no Misshitsu', size: 524288, hash: '1132c59cfba4cc7b0c1a7d7957b4b5de84c6038e0e061ca2e6372f8b8b4a01ca' },
  'demo-yonaki': { file: 'yonaki.sfc', name: '夜鳴き鐘 / Yonaki Kane', size: 4194304, hash: 'e8782a44c18a3cd8487d4a42687e5aa975dbbc4b750dbef9cf240e4236bd5925' },
};
const demoButtons = Object.keys(demos).map($);
function enableSources(enabled) { for (const button of [$('rom'), $('open-local'), $('change-cartridge'), ...demoButtons]) button.disabled = !enabled; }
const sources = new Set();
// Pace by the confirmed 714732-master-clock NTSC non-interlaced field pair.
const frameMilliseconds = 1000 * 714732 / 2 / 21477272;
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
  // The candidate remains separate until its save has been read and validated.
  // Failed restoration leaves the inserted machine and recoverable SRAM intact.
  restorePending = true;
  try {
    const saved = core.prepared_sram_len() ? localStorage.getItem(pendingCartridge.key) : null;
    if (saved !== null) copyInput(Uint8Array.from(atob(saved), (c) => c.charCodeAt(0)));
    check(core.commit_rom(saved !== null ? 1 : 0));
    ({ key, activeDemo } = pendingCartridge); loaded = true;
    $('current-game').textContent = pendingCartridge.name;
    $('current-game').title = pendingCartridge.name;
    $('placeholder').hidden = true;
    pendingCartridge = undefined; restorePending = false; persistenceError = '';
    draw(); $('status').textContent = 'Paused';
  } catch (e) {
    persistenceError = `Save could not be restored: ${e.message}. Saving and cartridge changes are blocked. Retry restoring the save when storage is available.`;
  }
  $('retry-save').hidden = !restorePending;
  $('play').disabled = !loaded || restorePending; $('reset').disabled = !loaded || restorePending;
  showErrors(); updatePlayback();
}
$('retry-save').addEventListener('click', () => {
  if (!restorePending || loading) return;
  void activateAudio(); restoreSave();
  if (!restorePending) { start(); canvas.focus({ preventScroll: true }); }
});
function flushAudio() { for (const source of sources) source.stop(); sources.clear(); nextAudio = 0; }
function updatePlayback() {
  $('play').textContent = playing ? 'Pause' : 'Play';
  for (const button of touchButtons) button.disabled = !playing;
}
function clearInput() { keyboard = 0; held.clear(); clearVirtualInput(); }
function pause() { playing = false; clearInput(); updatePlayback(); flushAudio(); const saved = save(); if (loaded) $('status').textContent = 'Paused'; return saved; }
function start() {
  if (!loaded || restorePending || loading || document.hidden) return;
  debt = frameMilliseconds; lastTime = performance.now(); playing = true; updatePlayback(); $('status').textContent = 'Playing';
}
function togglePlayback(event) {
  if (!loaded || restorePending || loading) return;
  if (playing) { pause(); return; }
  clearError(); void activateAudio(); start();
  if (playing && event?.detail > 0) canvas.focus({ preventScroll: true });
}
async function activateAudio() {
  if (!$('sound').checked) return;
  try {
    audio ??= new AudioContext({ sampleRate: 32000 });
    // Call resume during the activation gesture, before any asynchronous ROM read.
    const resumed = audio.resume();
    $('audio-status').textContent = audio.state === 'running' ? '' : 'Sound blocked. Toggle Sound to retry or mute.';
    await resumed;
    $('audio-status').textContent = audio.state === 'running' ? '' : 'Sound blocked. Toggle Sound to retry or mute.';
  } catch (e) { $('audio-status').textContent = `Sound unavailable: ${e.message}. Toggle Sound to retry or mute.`; }
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
  let value = keyboard | virtualButtons();
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
// Both sources use the same identity, core validation and battery-save lifecycle.
async function loadCartridge(name, read, demo) {
  if (loading || restorePending) return;
  loading = true;
  enableSources(false);
  $('play').disabled = true; $('reset').disabled = true;
  try {
    if (!pause()) return;
    clearError();
    if (!core) throw new Error('Emulator has not loaded');
    $('status').textContent = `Loading ${name}…`;
    let data = new Uint8Array(await read());
    if (data.length > 4 * 1024 * 1024 + 512) throw new Error('Maximum ROM size is 4 MiB plus a copier header');
    if (!demo && data.length % 1024 === 512) data = data.subarray(512);
    const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', data));
    const hash = Array.from(digest, (b) => b.toString(16).padStart(2, '0')).join('');
    if (demo && (data.length !== demo.size || hash !== demo.hash)) throw new Error('Demo download does not match the published ROM');
    // Reading/hashing is asynchronous: save again at the replacement boundary.
    if (!pause()) return;
    copyInput(data); check(core.prepare_rom());
    pendingCartridge = { key: `snes-rust:sram:${hash}`, name, activeDemo: demo ? demo.file : '' };
    restoreSave();
    if (restorePending) $('status').textContent = 'Restore save before changing cartridge';
  } catch (e) {
    $('status').textContent = loaded ? 'Paused' : 'Cartridge could not be loaded';
    report(e);
  } finally {
    loading = false;
    enableSources(Boolean(core));
    $('play').disabled = !loaded || restorePending; $('reset').disabled = !loaded || restorePending;
    updatePlayback();
  }
  if (pendingCartridge || restorePending || operationError || persistenceError) return;
  start();
  if (playing) canvas.focus({ preventScroll: true });
}
const picker = $('cartridge-picker');
function closePicker() { picker.hidePopover(); $('change-cartridge').focus({ preventScroll: true }); }
$('open-local').addEventListener('click', () => {
  if (loading || !core) return;
  // Unlock before the native file chooser can consume transient activation.
  void activateAudio(); closePicker(); $('rom').click();
});
$('rom').addEventListener('change', async (event) => {
  const file = event.target.files[0]; if (!file) return;
  void activateAudio();
  try {
    await loadCartridge(file.name, () => {
      if (!/\.(sfc|smc)$/i.test(file.name)) throw new Error('Choose an .sfc or .smc cartridge image');
      if (file.size > 4 * 1024 * 1024 + 512) throw new Error('Maximum ROM size is 4 MiB plus a copier header');
      return file.arrayBuffer();
    });
  } finally { event.target.value = ''; }
});
for (const button of demoButtons) button.addEventListener('click', async () => {
  if (loading || !core) return;
  closePicker();
  if (restorePending) { $('retry-save').focus(); return; }
  void activateAudio();
  const demo = demos[button.id];
  // Reselecting the inserted demo never reloads or changes playback state.
  if (activeDemo === demo.file) { canvas.focus({ preventScroll: true }); return; }
  await loadCartridge(demo.name, async () => {
    const response = await fetch(demo.file, { mode: 'same-origin', redirect: 'error' });
    if (!response.ok) throw new Error(`Demo download failed: HTTP ${response.status}`);
    return response.arrayBuffer();
  }, demo);
});
$('play').addEventListener('click', togglePlayback);
$('reset').addEventListener('click', () => {
  if (!loaded || restorePending || loading) return;
  pause();
  try {
    check(core.reset()); draw(); clearError();
    void activateAudio(); start();
    if (playing) canvas.focus({ preventScroll: true });
  } catch (e) { report(e); }
});
$('sound').addEventListener('change', () => { flushAudio(); $('audio-status').textContent = ''; if ($('sound').checked) void activateAudio(); });
$('scale').addEventListener('change', () => { const scale = Number($('scale').value); canvas.style.width = `${256 * scale}px`; });
for (const type of ['keydown', 'keyup']) window.addEventListener(type, (event) => {
  if (!keys[event.code] || !playing) return;
  if ((event.target.closest?.('#cartridge-picker') || ['INPUT', 'SELECT'].includes(event.target.tagName) || (['BUTTON', 'SUMMARY', 'A'].includes(event.target.tagName) && ['Enter', 'Space'].includes(event.code))) && !(type === 'keyup' && held.has(event.code))) return;
  event.preventDefault(); if (type === 'keydown') held.add(event.code); else held.delete(event.code);
  keyboard = [...held].reduce((value, code) => value | keys[code], 0);
});
window.addEventListener('blur', clearInput);
window.addEventListener('resize', clearVirtualInput);
window.addEventListener('orientationchange', clearVirtualInput);
document.addEventListener('fullscreenchange', clearVirtualInput);
$('fullscreen').addEventListener('click', async () => {
  try {
    if (document.fullscreenElement) await document.exitFullscreen();
    else await document.documentElement.requestFullscreen();
  } catch (e) { report(new Error(`Fullscreen unavailable: ${e.message}`)); }
});
// Each pointer owns its own mask. Releasing it never clears another pointer,
// a keyboard key or a physical gamepad button.
function virtualButtons() {
  let value = 0;
  for (const pointer of virtualPointers.values()) value |= pointer.bits;
  for (const bits of virtualKeys.values()) value |= bits;
  return value;
}
function paintVirtualInput() {
  const value = virtualButtons();
  for (const button of touchButtons) {
    const pressed = Boolean(value & Number(button.dataset.button));
    button.classList.toggle('pressed', pressed);
  }
}
function clearVirtualInput() {
  for (const [id, pointer] of virtualPointers) {
    if (pointer.element.hasPointerCapture(id)) pointer.element.releasePointerCapture(id);
  }
  virtualPointers.clear(); virtualKeys.clear(); paintVirtualInput();
}
function directionAt(event, element) {
  const rect = element.getBoundingClientRect();
  const x = (event.clientX - rect.left) / rect.width, y = (event.clientY - rect.top) / rect.height;
  if (x < 0 || x > 1 || y < 0 || y > 1) return 0;
  return (x < 1/3 ? 0x200 : x > 2/3 ? 0x100 : 0) | (y < 1/3 ? 0x800 : y > 2/3 ? 0x400 : 0);
}
const touchControls = document.querySelector('.touch-controls');
touchControls.addEventListener('contextmenu', event => event.preventDefault());
touchControls.addEventListener('pointerdown', event => {
  if (!playing || (event.pointerType === 'mouse' && event.button !== 0)) return;
  const dpad = event.target.closest('.dpad');
  const button = event.target.closest('[data-button]');
  const element = dpad || button;
  if (!element) return;
  event.preventDefault(); element.setPointerCapture(event.pointerId);
  virtualPointers.set(event.pointerId, { element, dpad: Boolean(dpad), bits: dpad ? directionAt(event, dpad) : Number(button.dataset.button) });
  paintVirtualInput();
});
touchControls.addEventListener('pointermove', event => {
  const pointer = virtualPointers.get(event.pointerId); if (!pointer) return;
  event.preventDefault();
  if (pointer.dpad) pointer.bits = directionAt(event, pointer.element);
  paintVirtualInput();
});
for (const type of ['pointerup', 'pointercancel', 'lostpointercapture']) touchControls.addEventListener(type, event => {
  virtualPointers.delete(event.pointerId); paintVirtualInput();
});
for (const button of touchButtons) {
  button.addEventListener('keydown', event => {
    if (!playing || !['Space', 'Enter'].includes(event.code)) return;
    event.preventDefault(); event.stopPropagation(); virtualKeys.set(button, Number(button.dataset.button)); paintVirtualInput();
  });
  button.addEventListener('keyup', event => {
    if (!['Space', 'Enter'].includes(event.code)) return;
    event.preventDefault(); event.stopPropagation(); virtualKeys.delete(button); paintVirtualInput();
  });
  button.addEventListener('blur', () => { virtualKeys.delete(button); paintVirtualInput(); });
}
document.addEventListener('visibilitychange', () => { if (document.hidden) { clearInput(); if (playing) pause(); } });
window.addEventListener('pagehide', save);
try {
  const response = await fetch('snes_wasm.wasm'); if (!response.ok) throw new Error(`Wasm download failed: HTTP ${response.status}`);
  const result = await WebAssembly.instantiate(await response.arrayBuffer(), {}); core = result.instance.exports;
  $('status').textContent = 'Ready'; enableSources(true); requestAnimationFrame(animate);
} catch (e) { report(e); $('status').textContent = 'Emulator unavailable'; enableSources(false); }
