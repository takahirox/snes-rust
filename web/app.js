const $ = (id) => document.getElementById(id);
const canvas = document.querySelector('canvas');
const context = canvas.getContext('2d', { alpha: false });
let image = context.createImageData(256, 224);
const decoder = new TextDecoder();
let core, loaded = false, playing = false, key = '', audio, masterGain, nextAudio = 0, lastTime = 0, debt = 0, saveClock = 0;
let keyboard = 0, activeDemo = '';
const virtualPointers = new Map(), virtualKeys = new Map();
const touchButtons = [...document.querySelectorAll('[data-button]')];
let operationError = '', persistenceError = '', restorePending = false, loading = false, pendingCartridge;
// Bundled demos by picker button id; each is fetched from the same origin and pinned.
const demos = {
  demo: { file: 'shionari.sfc', name: '潮鳴りの密室 / Shionari no Misshitsu', size: 524288, hash: '1132c59cfba4cc7b0c1a7d7957b4b5de84c6038e0e061ca2e6372f8b8b4a01ca' },
  'demo-yonaki': { file: 'yonaki.sfc', name: '夜鳴き鐘 / Yonaki Kane', size: 4194304, hash: '53022b7b4154ec7320f057882e8fdd56ec9d47ea6c6d5133b8a3e0c8737f829e' },
};
const demoButtons = Object.keys(demos).map($);
function enableSources(enabled) { for (const button of [$('rom'), $('open-local'), $('change-cartridge'), ...demoButtons]) button.disabled = !enabled; }
const sources = new Set();
// Pace by the confirmed 714732-master-clock NTSC non-interlaced field pair.
const frameMilliseconds = 1000 * 714732 / 2 / 21477272;
const audioLead = .04, maxAudioLead = .15, maxSources = 12;
// Browser-wide preference, independent of cartridge/SRAM identity. Zero is mute;
// retain the last audible level so both slider-zero and the button can restore it.
const volumeKey = 'snes-rust:volume';
let volume = 100, lastNonzeroVolume = 100;
try {
  const saved = JSON.parse(localStorage.getItem(volumeKey));
  if (saved && Number.isInteger(saved.volume) && saved.volume >= 0 && saved.volume <= 100 &&
      Number.isInteger(saved.lastNonzeroVolume) && saved.lastNonzeroVolume > 0 && saved.lastNonzeroVolume <= 100) {
    volume = saved.volume; lastNonzeroVolume = volume || saved.lastNonzeroVolume;
  }
} catch { /* Audio preferences must never block gameplay when storage is unavailable. */ }
function updateVolume() {
  $('volume').value = volume;
  $('volume').setAttribute('aria-valuetext', volume ? `${volume}%` : '0% (muted)');
  $('mute').setAttribute('aria-label', volume ? 'Mute' : 'Unmute');
  $('mute').setAttribute('aria-pressed', String(volume === 0));
  $('volume-control').classList.toggle('muted', volume === 0);
}
function setVolume(value) {
  volume = value;
  if (volume) lastNonzeroVolume = volume;
  updateVolume();
  if (masterGain) {
    // One shared stage affects sources already scheduled as well as future ones.
    masterGain.gain.cancelScheduledValues(audio.currentTime);
    masterGain.gain.setTargetAtTime(volume / 100, audio.currentTime, .005);
  }
  try { localStorage.setItem(volumeKey, JSON.stringify({ volume, lastNonzeroVolume })); } catch { /* Best effort. */ }
  $('audio-status').textContent = '';
  if (volume) void activateAudio();
}
updateVolume();
// Pixel is always drawn first. A separate, non-interactive WebGL surface covers
// only its content box; the original canvas owns geometry, focus and fallback.
// No shader library, CPU pixel filtering or changes to the core framebuffer.
class CrtDisplay {
  constructor(source, select, status) {
    this.source = source; this.select = select; this.status = status;
    this.mode = 'pixel'; this.hasFrame = false;
    this.surface = document.createElement('canvas');
    this.surface.className = 'crt-display'; this.surface.hidden = true;
    this.surface.setAttribute('aria-hidden', 'true');
    this.surface.addEventListener('webglcontextlost', event => {
      event.preventDefault(); this.fallback('CRT graphics context lost.');
    });
    // Restoration never interrupts playback or overrides the current selection.
    // Rebuild all resources on the next explicit CRT choice.
    this.surface.addEventListener('webglcontextrestored', () => { this.status.textContent = 'CRT is available again. Select CRT to retry.'; });
    select.addEventListener('change', () => this.choose(select.value, true));
    new ResizeObserver(() => this.present()).observe(source);
    // Also catch DPR-only changes when moving between screens.
    window.addEventListener('resize', () => this.present());
    let saved;
    try { saved = localStorage.getItem('snes-rust:display'); } catch { /* Best effort. */ }
    // Only an explicit Pixel preference opts out of the default CRT presentation.
    this.choose(saved === 'pixel' ? 'pixel' : 'crt');
  }
  release() {
    const gl = this.gl;
    if (!gl) return;
    for (const query of this.queries ?? []) this.timer.deleteQueryEXT(query);
    this.queries = [];
    if (this.texture) gl.deleteTexture(this.texture);
    if (this.buffer) gl.deleteBuffer(this.buffer);
    if (this.program) gl.deleteProgram(this.program);
    this.texture = this.buffer = this.program = null;
  }
  fallback(reason) {
    this.mode = this.select.value = 'pixel'; this.surface.hidden = true;
    this.release();
    this.status.textContent = `${reason} Using Pixel. Select CRT to retry.`;
  }
  choose(mode, persist = false) {
    this.mode = this.select.value = mode;
    this.status.textContent = '';
    this.surface.hidden = true;
    if (mode === 'pixel') { this.release(); this.surface.remove(); }
    else {
      try { this.initialize(); this.source.after(this.surface); this.present(); }
      catch { this.fallback('CRT is unavailable on this device.'); }
    }
    if (persist) {
      try { localStorage.setItem('snes-rust:display', this.mode); } catch { /* Best effort; independent of SRAM. */ }
    }
  }
  initialize() {
    if (this.program) return;
    const gl = this.gl ??= this.surface.getContext('webgl', {
      alpha: false, antialias: false, depth: false, stencil: false,
      // Keep a paused frame and canvas capture intact between animation frames.
      preserveDrawingBuffer: true,
    });
    if (!gl || gl.isContextLost()) throw new Error('WebGL unavailable');
    const shaders = [];
    try {
      const compile = (type, source) => {
        const shader = gl.createShader(type); shaders.push(shader);
        gl.shaderSource(shader, source); gl.compileShader(shader);
        if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw new Error('Shader compilation failed');
        return shader;
      };
      this.program = gl.createProgram();
      gl.attachShader(this.program, compile(gl.VERTEX_SHADER, `
        attribute vec2 position;
        varying vec2 uv;
        void main() { uv = position * .5 + .5; gl_Position = vec4(position, 0., 1.); }
      `));
      gl.attachShader(this.program, compile(gl.FRAGMENT_SHADER, `
        #ifdef GL_FRAGMENT_PRECISION_HIGH
        precision highp float;
        #else
        precision mediump float;
        #endif
        uniform sampler2D frame;
        uniform vec2 sourceSize;
        uniform float scanStrength;
        varying vec2 uv;
        void main() {
          // Canvas supplies opaque display-encoded RGB. Filter in that same
          // space (no physical signal/colorimetry claim), with clamped edges.
          vec2 p = vec2(uv.x, (floor(uv.y * sourceSize.y) + .5) / sourceSize.y);
          vec2 dotStep = vec2(1. / 256., 0.);
          vec3 center = texture2D(frame, p).rgb;
          vec3 nearColor = (texture2D(frame, p - dotStep * .65).rgb +
                            texture2D(frame, p + dotStep * .65).rgb) * .5;
          vec3 wideColor = (texture2D(frame, p - dotStep * 1.5).rgb +
                            texture2D(frame, p + dotStep * 1.5).rgb) * .5;
          vec3 color = center * .78 + nearColor * .18 + wideColor * .04;
          color += (1. - color) * wideColor * .025;
          // At full strength, row centers remain bright and gaps lose at most
          // 7%. Fade out below 2.5 output samples/row to avoid mobile moire.
          color *= 1. - scanStrength * .035 * (1. + cos(uv.y * sourceSize.y * 6.2831853));
          gl_FragColor = vec4(color, 1.);
        }
      `));
      gl.linkProgram(this.program);
      if (!gl.getProgramParameter(this.program, gl.LINK_STATUS)) throw new Error('Shader link failed');
    } finally { for (const shader of shaders) gl.deleteShader(shader); }
    gl.useProgram(this.program);
    this.buffer = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1, 1,-1, -1,1, -1,1, 1,-1, 1,1]), gl.STATIC_DRAW);
    const position = gl.getAttribLocation(this.program, 'position');
    gl.enableVertexAttribArray(position); gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0);
    this.texture = gl.createTexture(); gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
    gl.pixelStorei(gl.UNPACK_COLORSPACE_CONVERSION_WEBGL, gl.NONE);
    gl.uniform1i(gl.getUniformLocation(this.program, 'frame'), 0);
    this.sizeUniform = gl.getUniformLocation(this.program, 'sourceSize');
    this.scanUniform = gl.getUniformLocation(this.program, 'scanStrength');
    this.maxSize = Math.min(gl.getParameter(gl.MAX_TEXTURE_SIZE), gl.getParameter(gl.MAX_RENDERBUFFER_SIZE));
    this.textureWidth = this.textureHeight = 0;
    this.timer = gl.getExtension('EXT_disjoint_timer_query'); this.queries = [];
    this.cpuCost = this.cpuSamples = this.gpuCost = this.gpuSamples = this.workCost = this.workSamples = 0;
  }
  overBudget(cost, kind = 'cpu') {
    const sum = `${kind}Cost`, samples = `${kind}Samples`;
    this[sum] += cost;
    // The existing NTSC frame budget, averaged over a sustained two seconds
    // of work, rather than a machine-specific benchmark threshold. No gl.finish.
    if (++this[samples] < 120) return false;
    const slow = this[sum] / this[samples] > frameMilliseconds;
    this[sum] = this[samples] = 0;
    return slow;
  }
  observeWorkload(cost) {
    // Include core catch-up/audio work: a small shader cost can still push an
    // already busy device past its presentation budget. Pixel is cheaper.
    if (this.mode === 'crt' && this.overBudget(cost, 'work')) this.fallback('CRT exceeded the frame budget.');
  }
  present() {
    if (this.mode !== 'crt' || !this.hasFrame) return;
    try {
      const start = performance.now(), gl = this.gl, c = this.surface, s = this.source;
      if (gl.isContextLost()) throw new Error('Context lost');
      const rect = s.getBoundingClientRect();
      if (!rect.width || !rect.height) return;
      const parent = s.parentElement.getBoundingClientRect();
      c.style.left = `${rect.left - parent.left}px`; c.style.top = `${rect.top - parent.top}px`;
      c.style.width = `${rect.width}px`; c.style.height = `${rect.height}px`;
      // Bound fill cost on high-DPR phones and very large fullscreen displays.
      const ratio = Math.min(devicePixelRatio || 1, 2, this.maxSize / rect.width, this.maxSize / rect.height);
      const width = Math.max(1, Math.round(rect.width * ratio)), height = Math.max(1, Math.round(rect.height * ratio));
      if (c.width !== width || c.height !== height) { c.width = width; c.height = height; }
      gl.viewport(0, 0, width, height);
      if (this.textureWidth !== s.width || this.textureHeight !== s.height) {
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, s);
        this.textureWidth = s.width; this.textureHeight = s.height;
      } else gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, gl.RGBA, gl.UNSIGNED_BYTE, s);
      gl.uniform2f(this.sizeUniform, s.width, s.height);
      gl.uniform1f(this.scanUniform, Math.max(0, Math.min(1, (height / s.height - 1.5))));
      const timer = this.timer;
      if (timer) {
        const disjoint = gl.getParameter(timer.GPU_DISJOINT_EXT);
        while (this.queries.length && timer.getQueryObjectEXT(this.queries[0], timer.QUERY_RESULT_AVAILABLE_EXT)) {
          const query = this.queries.shift(), ms = timer.getQueryObjectEXT(query, timer.QUERY_RESULT_EXT) / 1e6;
          timer.deleteQueryEXT(query);
          if (!disjoint && this.overBudget(ms, 'gpu')) throw new Error('GPU frame budget exceeded');
        }
      }
      const query = timer && this.queries.length < 4 ? timer.createQueryEXT() : null;
      if (query) timer.beginQueryEXT(timer.TIME_ELAPSED_EXT, query);
      try { gl.drawArrays(gl.TRIANGLES, 0, 6); }
      finally { if (query) { timer.endQueryEXT(timer.TIME_ELAPSED_EXT); this.queries.push(query); } }
      if (gl.getError() !== gl.NO_ERROR) throw new Error('Graphics error');
      if (this.overBudget(performance.now() - start)) throw new Error('Renderer frame budget exceeded');
      c.hidden = false;
    } catch { this.fallback('CRT could not keep rendering.'); }
  }
}
const display = new CrtDisplay(canvas, $('display'), $('display-status'));

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
  display.hasFrame = true; display.present();
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
  if (!volume) return;
  const blocked = () => volume && audio.state !== 'running' ? 'Sound blocked. Adjust volume or unmute to retry.' : '';
  try {
    audio ??= new AudioContext({ sampleRate: 32000 });
    if (!masterGain) {
      const gain = audio.createGain();
      gain.gain.setValueAtTime(volume / 100, audio.currentTime);
      gain.connect(audio.destination); masterGain = gain;
    }
    // Call resume during the activation gesture, before any asynchronous ROM read.
    const resumed = audio.resume();
    $('audio-status').textContent = blocked();
    await resumed;
    $('audio-status').textContent = blocked();
  } catch (e) { $('audio-status').textContent = volume ? `Sound unavailable: ${e.message}. Adjust volume or unmute to retry.` : ''; }
}
function queueAudio() {
  if (!masterGain || audio.state !== 'running') return;
  const count = core.audio_len(); if (!count) return;
  const samples = new Float32Array(core.memory.buffer, core.audio_buffer(), count);
  const buffer = audio.createBuffer(2, count / 2, 32000);
  for (let channel = 0; channel < 2; channel++) { const data = buffer.getChannelData(channel); for (let i = 0; i < data.length; i++) data[i] = samples[i * 2 + channel]; }
  const now = audio.currentTime;
  if (nextAudio < now || nextAudio + buffer.duration > now + maxAudioLead || sources.size >= maxSources) { flushAudio(); nextAudio = now + audioLead; }
  const source = audio.createBufferSource(); source.buffer = buffer; source.connect(masterGain); sources.add(source);
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
    const workStart = performance.now();
    let frames = 0;
    while (debt >= frameMilliseconds && frames < 3) { check(core.run_frame(buttons())); queueAudio(); debt -= frameMilliseconds; frames++; }
    if (frames) { draw(); display.observeWorkload(performance.now() - workStart); }
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
$('mute').addEventListener('click', () => setVolume(volume ? 0 : lastNonzeroVolume));
$('volume').addEventListener('input', () => setVolume(Number($('volume').value)));
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
