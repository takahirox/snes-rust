import {
  addStoredProfile, decodeSram, encodeSram, hasPendingSram, hasSessionPendingSram, historyExists, historyPage, lockSaves,
  persistNewProfile, persistSram, readPendingSram, readProfiles, readSramData, readState,
  recoverSram, removeSram, removeState, sameSram, settlePendingSram, sramTimestamp, stageSram,
  writeProfiles, writeSramData, writeState,
} from './saves.js';

const $ = (id) => document.getElementById(id);
const canvas = document.querySelector('canvas');
const context = canvas.getContext('2d', { alpha: false });
let image = context.createImageData(256, 224);
const decoder = new TextDecoder();
let core, loaded = false, playRequested = false, running = false, activeHash = '', activeProfile;
let activeSram;
let saveBusy = false;
let audio, masterGain, nextAudio = 0, lastTime, debt = 0, saveClock = 0;
let keyboard = 0;
const virtualPointers = new Map(), virtualKeys = new Map();
const touchButtons = [...document.querySelectorAll('[data-button]')];
let operationError = '', persistenceError = '', restorePending = false, loading, pendingCartridge;
// Bundled demos by picker button id; each is fetched from the same origin and pinned.
const demos = {
  demo: { file: 'shionari.sfc', name: '潮鳴りの密室 / Shionari no Misshitsu', size: 524288, hash: '1132c59cfba4cc7b0c1a7d7957b4b5de84c6038e0e061ca2e6372f8b8b4a01ca' },
  'demo-yonaki': { file: 'yonaki.sfc', name: '夜鳴き鐘 / Yonaki Kane', size: 3145728, hash: 'aad5e1a3a81b1327efedea01cfd30d6c112f9451c803645e318912b4c17070aa' },
};
const demoButtons = Object.keys(demos).map($);
function enableSources(enabled, pickerEnabled = enabled) {
  for (const button of [$('rom'), $('open-local'), ...demoButtons]) button.disabled = !enabled;
  $('change-cartridge').disabled = !pickerEnabled;
}
const bundledHashes = new Set(Object.values(demos).map(demo => demo.hash));
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
  overBudget(cost, kind = 'cpu', frames = 1) {
    const sum = `${kind}Cost`, samples = `${kind}Samples`;
    this[sum] += cost;
    // The existing NTSC frame budget, averaged over a sustained two seconds
    // of work, rather than a machine-specific benchmark threshold. No gl.finish.
    this[samples] += frames;
    if (this[samples] < 120) return false;
    const slow = this[sum] / this[samples] > frameMilliseconds;
    this[sum] = this[samples] = 0;
    return slow;
  }
  observeWorkload(cost, frames) {
    // Include core catch-up/audio work: a small shader cost can still push an
    // already busy device past its frame budget. A callback that catches up
    // several guest frames has one frame budget for each frame it runs.
    if (this.mode === 'crt' && this.overBudget(cost, 'work', frames)) this.fallback('CRT exceeded the frame budget.');
  }
  present() {
    if (this.mode !== 'crt' || !this.hasFrame) return;
    try {
      const start = performance.now(), gl = this.gl, c = this.surface, s = this.source;
      if (gl.isContextLost()) throw new Error('Context lost');
      const rect = s.getBoundingClientRect();
      if (!rect.width || !rect.height) return;
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
function showErrors() {
  const error = $('error');
  const message = [operationError, persistenceError].filter(Boolean).join('\n');
  if (error.textContent !== message) error.textContent = message;
}
function report(error) { operationError = error.message || String(error); showErrors(); }
function clearError() { operationError = ''; showErrors(); }
function warnBeforeUnload(event) {
  event.preventDefault();
  event.returnValue = true;
}
function saveBeforeUnload(event) {
  void save(event);
  if (hasSessionPendingSram()) warnBeforeUnload(event);
}
function updateUnloadGuard() {
  // A session-only journal can survive navigation and cartridge changes.
  if (activeSram || hasSessionPendingSram()) window.addEventListener('beforeunload', saveBeforeUnload);
  else window.removeEventListener('beforeunload', saveBeforeUnload);
}
updateUnloadGuard();
window.addEventListener('pageshow', updateUnloadGuard);

function trackSram(base, epoch) {
  activeSram = activeProfile ? {
    target: { id: activeProfile.id, base, epoch },
    recoveryId: crypto.randomUUID(),
    saveRevision: 0,
    clean: encodeSram(liveSram()),
  } : undefined;
  updateUnloadGuard();
}

function nextSramRevision(session) {
  // Keep same-size recovery writes possible even at localStorage quota.
  return (++session.saveRevision).toString(16).padStart(14, '0');
}

function adoptSramTarget(session, saved, data) {
  if (saved.id !== session.target.id) {
    session.recoveryId = crypto.randomUUID();
    $('save-status').textContent = 'This tab’s progress is in Recovery under Save data.';
  }
  session.target = saved;
  session.clean = data;
}

function persistActiveSram(data, revision = nextSramRevision(activeSram)) {
  const session = activeSram;
  const journal = session.pending;
  const removed = [journal, journal?.fallback].find(copy => copy?.storage.getItem(copy.key) === null);
  let replayed = session.replayCandidate;
  if (removed && (!replayed || removed.entry.revision > replayed.revision)) replayed = removed.entry;
  if (replayed) {
    // A removed journal may have been replayed or consumed by a state load.
    // Under the ROM lock, adopt its target only if the bytes and epoch still match.
    adoptSramTarget(session, persistSram(activeHash, replayed, { settled: true }), replayed.data);
    if (removed && removed === journal) session.pending = undefined;
    else if (removed) journal.fallback = undefined;
    session.replayCandidate = undefined;
    if (replayed.revision >= revision) return;
  }

  const { target, recoveryId, clean, pending } = session;
  const saved = persistSram(activeHash, { ...target, recoveryId, data, revision }, { dirty: data !== clean });
  adoptSramTarget(session, saved, data);
  const fallback = pending?.fallback;
  if (fallback && fallback.entry.revision < revision && saved.base === data) {
    fallback.storage.removeItem(fallback.key);
    pending.fallback = undefined;
  }
  if (pending && (pending.entry.revision === revision || (pending.entry.revision < revision && saved.base === data))) {
    // A regular autosave can durably replace a delayed lifecycle save.
    pending.storage.removeItem(pending.key);
    session.pending = undefined;
  }
}

function finishSramSave(error) {
  persistenceError = error ? `Could not persist SRAM: ${error.message}. Keep this tab open to retain the current save. Cartridge changes are blocked until saving succeeds.` : '';
  showErrors();
  return !error;
}

async function save(lifecycleEvent) {
  if (!activeSram) return true;
  if (restorePending) return false;
  const session = activeSram, hash = activeHash;
  const data = encodeSram(liveSram());
  const revision = nextSramRevision(session);
  let pending;
  const dirty = data !== session.clean;
  if (lifecycleEvent && (dirty || session.pending)) {
    try {
      if (session.pending?.entry.data !== data) {
        const previous = session.pending;
        const fallback = previous?.durable ? previous : previous?.fallback;
        // A session write can reuse the same key. Check for replay before it
        // replaces a missing session entry, and check shared storage afterward.
        const sessionReplayed = previous && !previous.durable && previous.storage.getItem(previous.key) === null;
        const entry = { ...session.target, recoveryId: session.recoveryId, data, revision };
        const journal = stageSram(hash, entry, previous?.key);
        const retained = journal.durable ? undefined : fallback;
        session.pending = { ...journal, entry, fallback: retained };
        for (const copy of [previous, previous?.fallback]) {
          if (!copy) continue;
          if ((copy === previous && sessionReplayed) || copy.storage.getItem(copy.key) === null) {
            if (!session.replayCandidate || copy.entry.revision > session.replayCandidate.revision) {
              session.replayCandidate = copy.entry;
            }
            if (copy === retained) session.pending.fallback = undefined;
          } else if (copy !== retained && (copy.storage !== journal.storage || copy.key !== journal.key)) {
            copy.storage.removeItem(copy.key);
          }
        }
      }
      pending = session.pending;
    } catch (error) { finishSramSave(error); }
    // Session storage survives navigation and is erased on tab close. Warn
    // if the current bytes have no durable journal yet.
    if (lifecycleEvent.type === 'beforeunload' && dirty && !pending?.durable) {
      warnBeforeUnload(lifecycleEvent);
    }
  }
  try {
    return await lockSaves(hash, () => {
      if (session !== activeSram) return true;
      if (restorePending) return false;
      if (pending && session.pending !== pending) return true;
      persistActiveSram(data, pending?.entry.revision ?? revision);
      return finishSramSave();
    });
  } catch (error) {
    return session === activeSram && !restorePending ? finishSramSave(error) : !restorePending;
  }
}
function restoreSave() {
  // The candidate remains separate until its save has been read and validated.
  // Failed restoration leaves the inserted machine and recoverable SRAM intact.
  restorePending = true;
  const { hash, name, remember } = pendingCartridge;
  try {
    const size = core.prepared_sram_len();
    const catalog = size ? readProfiles(hash, { initialize: true }) : null;
    let pendingError;
    if (size) {
      try { settlePendingSram(hash, size); }
      catch (error) { pendingError = error; }
    }
    const profile = catalog ? requireProfile(catalog, catalog.selected) : null;
    const base = profile ? readSramData(hash, profile.id) : null;
    const saved = decodeSram(base, size);
    if (saved !== null) copyInput(saved);
    check(core.commit_rom(saved !== null ? 1 : 0));
    activeHash = hash; activeProfile = profile; loaded = true;
    trackSram(base, profile?.epoch);
    $('save-status').textContent = pendingError ? `Could not finish saving SRAM: ${pendingError.message}. Open Save data to retry.` : '';
    $('current-game').textContent = name;
    $('current-game').title = name;
    $('placeholder').hidden = true;
    pendingCartridge = undefined; restorePending = false; persistenceError = '';
    if (remember) rememberCartridge(hash, name, remember);
    draw(); setStatus('Paused');
  } catch (e) {
    persistenceError = `Save could not be restored: ${e.message}. Saving and cartridge changes are blocked. Retry restoring the save when storage is available.`;
  }
  $('retry-save').hidden = !restorePending;
  $('play').disabled = !loaded || restorePending; $('reset').disabled = !loaded || restorePending;
  showErrors(); updatePlayback(); updateSaveControls();
}
const retrySave = $('retry-save');
retrySave.addEventListener('click', async () => {
  if (!restorePending || loading || retrySave.disabled) return;
  retrySave.disabled = true;
  void activateAudio();
  const pending = pendingCartridge;
  try {
    await lockSaves(pending.hash, () => {
      if (pending !== pendingCartridge) return;
      restoreSave();
      if (!restorePending) {
        start();
        if (isRunning()) canvas.focus({ preventScroll: true });
      }
    });
  } catch (error) { report(error); }
  finally { retrySave.disabled = false; }
});
function flushAudio() { for (const source of sources) source.stop(); sources.clear(); nextAudio = 0; }
function isRunning() { return running && !document.hidden; }
function playbackStatus() { return isRunning() ? 'Playing' : playRequested ? 'Suspended' : 'Paused'; }
function updatePlayback() {
  $('play').textContent = playRequested ? 'Pause' : 'Play';
  const acceptsInput = isRunning();
  const loadFocus = loading ? document.activeElement : null;
  for (const button of touchButtons) {
    // Suspension and the pause during replacement both gate controller input.
    // Avoid blurring a focused button while the replacement is in progress.
    button.disabled = !playRequested && button !== loadFocus;
    if (!acceptsInput && !button.disabled) button.setAttribute('aria-disabled', 'true');
    else button.removeAttribute('aria-disabled');
  }
}
function clearInput() { keyboard = 0; held.clear(); clearVirtualInput(); }
function syncPlayback(retryAudio = false) {
  const wasRunning = running;
  // An in-flight read can leave the old cartridge running. Once suspended,
  // wait for the load to resolve before resuming either cartridge.
  running = playRequested && !document.hidden && (wasRunning || !loading);
  if (running && !wasRunning) {
    debt = frameMilliseconds;
    lastTime = undefined;
    saveClock = performance.now();
    if (retryAudio && audio && audio.state !== 'running') void activateAudio();
  } else if (wasRunning && !running) {
    clearInput(); flushAudio();
  }
  updatePlayback();
  if (playRequested && !loading && $('status').textContent !== playbackStatus()) setStatus(playbackStatus());
  return wasRunning && !running;
}
function stopPlayback() { playRequested = false; syncPlayback(); if (loaded) setStatus('Paused'); }
function pause() { stopPlayback(); return save(); }
function start(retryAudio = false) {
  if (!loaded || restorePending || loading) return;
  playRequested = true; syncPlayback(retryAudio);
}
function togglePlayback(event) {
  if (!loaded || restorePending || loading) return;
  if (playRequested) { void pause(); return; }
  clearError(); void activateAudio(); start();
  if (isRunning() && event?.detail > 0) canvas.focus({ preventScroll: true });
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
    // For a launch gesture, call resume before any asynchronous ROM read.
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
  if (!isRunning()) return;
  // Discard stale wall time after a stall. Catch-up stays within three frames;
  // video and audio restart together instead of accumulating an unbounded debt.
  debt = Math.min(debt + Math.max(time - (lastTime ?? time), 0), frameMilliseconds * 3); lastTime = time;
  try {
    const workStart = performance.now();
    let frames = 0;
    while (debt >= frameMilliseconds && frames < 3) { check(core.run_frame(buttons())); queueAudio(); debt -= frameMilliseconds; frames++; }
    if (frames) { draw(); display.observeWorkload(performance.now() - workStart, frames); }
    if (time - saveClock > 5000) { void save(); saveClock = time; }
  } catch (e) { void pause(); report(e); }
}
const maxRomBytes = 4 * 1024 * 1024 + 512;
const romSizeError = 'Maximum ROM size is 4 MiB plus a copier header';
function isRomHash(hash) {
  return typeof hash === 'string' && /^[0-9a-f]{64}$/.test(hash);
}
function normalizeRom(data) { return data.length % 1024 === 512 ? data.slice(512) : data; }
async function romHash(data) {
  const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', data));
  return Array.from(digest, (b) => b.toString(16).padStart(2, '0')).join('');
}
function finishCartridgeLoad(playReplacement = false) {
  loading = undefined;
  enableSources(Boolean(core)); updateSaveControls();
  $('play').disabled = !loaded || restorePending; $('reset').disabled = !loaded || restorePending;
  if (playReplacement) start(true);
  else syncPlayback(true);
}
// All sources use the same identity, core validation and battery-save lifecycle.
async function loadCartridge(name, read, { demo, savedHash } = {}) {
  if (loading || restorePending || saveBusy) return;
  const fromLibrary = savedHash !== undefined;
  const load = { fromLibrary };
  let sameCartridge = false;
  let savedLoadFailed = false;
  let focusGame = false;
  let playReplacement = false;
  loading = load;
  updateSaveControls();
  // Keep the native popover toggle available to dismiss a pending saved selection.
  enableSources(false, fromLibrary);
  $('play').disabled = true; $('reset').disabled = true;
  try {
    if (fromLibrary && !isRomHash(savedHash)) throw new Error('Invalid saved ROM hash');
    const expectedHash = demo?.hash ?? savedHash;
    // Check a local file's identity before requiring a save to replace the machine.
    if (expectedHash && expectedHash !== activeHash) {
      const saved = await (demo ? pause() : save());
      if (loading !== load) return;
      if (!saved) {
        if (playRequested) stopPlayback();
        return;
      }
    }
    clearError();
    if (!core) throw new Error('Emulator has not loaded');
    if (expectedHash !== activeHash) setStatus(`Loading ${name}…`);
    let data = await read();
    if (loading !== load) return;
    if (data.length > maxRomBytes) throw new Error(romSizeError);
    if (!demo) data = normalizeRom(data);
    const hash = await romHash(data);
    if (loading !== load) return;
    if (demo && (data.length !== demo.size || hash !== demo.hash)) throw new Error('Demo download does not match the published ROM');
    if (fromLibrary && hash !== savedHash) throw new Error('Saved ROM identity mismatch');
    const remember = !demo && !fromLibrary && !bundledHashes.has(hash) ? data : null;
    const focused = document.activeElement;
    focusGame = !fromLibrary && (focused === document.body || focused === $('change-cartridge'));
    sameCartridge = hash === activeHash;
    if (sameCartridge) {
      if (remember) rememberCartridge(hash, name, remember);
      setStatus(playbackStatus());
    } else {
      // A successful suspension save already captured the outgoing machine.
      // Playback cannot resume while this cartridge read is still pending.
      const savedOnSuspend = await load.savedOnSuspend;
      if (loading !== load) return;
      if (savedOnSuspend) stopPlayback();
      else if (!await pause()) return;
      if (loading !== load) return;
      copyInput(data); check(core.prepare_rom());
      const restore = () => {
        if (loading !== load) return;
        pendingCartridge = { hash, name, remember };
        restoreSave();
      };
      if (core.prepared_sram_len()) await lockSaves(hash, restore);
      else restore();
      if (loading !== load) return;
      if (restorePending) setStatus('Restore save before changing cartridge');
      playReplacement = !restorePending && !operationError && !persistenceError;
    }
  } catch (e) {
    if (loading !== load) return;
    if (!fromLibrary && playRequested) await pause();
    if (loading !== load) return;
    setStatus(loaded ? playbackStatus() : 'Cartridge could not be loaded');
    if (fromLibrary) {
      savedLoadFailed = true;
      if (picker.matches(':popover-open')) showSavedRecovery(savedHash, name);
    } else report(e);
    return;
  } finally {
    if (loading === load) {
      finishCartridgeLoad(playReplacement);
      if (fromLibrary && !savedLoadFailed && picker.matches(':popover-open')) {
        focusGame = picker.contains(document.activeElement);
        closePicker(focusGame);
      }
    }
  }
  if (sameCartridge) {
    if (focusGame && !document.hidden) canvas.focus({ preventScroll: true });
    return;
  }
  if (playReplacement && focusGame && isRunning()) canvas.focus({ preventScroll: true });
}
const picker = $('cartridge-picker');
picker.addEventListener('beforetoggle', event => {
  if (event.newState !== 'closed' || !loading?.fromLibrary) return;
  // The read or digest may still be pending. Release the controls as soon as the picker closes.
  finishCartridgeLoad();
  setStatus(loaded ? playbackStatus() : 'Ready');
  $('change-cartridge').focus({ preventScroll: true });
});
picker.addEventListener('keydown', event => {
  // Removing a row can move focus to another button while Enter is still held.
  if (event.key === 'Enter' && event.repeat) event.preventDefault();
});
let lastRemoveClick;
document.addEventListener('click', event => {
  if (!event.detail || !lastRemoveClick) return;
  const { click, hash, bounds } = lastRemoveClick;
  // A committed removal can move another control under the pointer. Honor the
  // browser's repeat count even outside the old button, and use proximity and
  // timing for taps that each have a click count of one.
  const { clientX: x, clientY: y } = event;
  const inside = x >= bounds.left && x < bounds.right && y >= bounds.top && y < bounds.bottom;
  const nearby = inside || Math.hypot(x - click.clientX, y - click.clientY) <= 6;
  const repeated = event.detail > 1 || (nearby && event.timeStamp - click.timeStamp <= 500);
  if (repeated && !click.target.getClientRects().length) {
    const remove = event.target.closest('.remove-cartridge');
    // A refresh can rebuild the same cartridge's Remove button.
    if (!remove || !sameCartridgeKey(hash, rowKeys.get(remove.parentElement))) {
      event.preventDefault();
      event.stopPropagation();
      return;
    }
  }
  lastRemoveClick = undefined;
}, { capture: true });
function closePicker(restoreFocus = true) {
  picker.hidePopover();
  if (restoreFocus) $('change-cartridge').focus({ preventScroll: true });
}

// Keep metadata separate so listing cartridges never reads their ROM bytes.
let libraryDatabase;
function openLibrary() {
  return libraryDatabase ??= new Promise((resolve, reject) => {
    const request = indexedDB.open('snes-rust:cartridges', 1);
    let blocked = false;
    request.onupgradeneeded = () => {
      request.result.createObjectStore('cartridges', { keyPath: 'hash' });
      request.result.createObjectStore('roms');
    };
    request.onblocked = () => { blocked = true; reject(new Error('Cartridge storage is busy')); };
    request.onerror = () => reject(request.error);
    request.onsuccess = () => {
      const db = request.result;
      if (blocked) { db.close(); return; }
      db.onversionchange = db.onclose = () => { db.close(); libraryDatabase = undefined; };
      resolve(db);
    };
  }).catch(error => { libraryDatabase = undefined; throw error; });
}
async function libraryTransaction(stores, mode, run) {
  const db = await openLibrary();
  return new Promise((resolve, reject) => {
    const transaction = db.transaction(stores, mode);
    transaction.onabort = () => reject(transaction.error ?? new Error('Cartridge storage transaction aborted'));
    let request;
    transaction.oncomplete = () => resolve(request?.result);
    try { request = run(transaction); }
    catch (error) { transaction.abort(); reject(error); }
  });
}

const savedList = $('saved-cartridges');
const libraryStatus = $('library-operation-status');
const savedRecovery = $('library-recovery');
let recoveryCartridge;
const retentionNotice = $('rom-retention-warning');
const retentionFailures = new Map();
let retentionNoticeDismissed = false;
function renderRetentionWarning(target, names) {
  const labels = names.map(name => {
    const label = document.createElement('span');
    label.className = 'rom-retention-name';
    label.textContent = `Could not remember ${name}. `;
    label.title = name;
    return label;
  });
  target.replaceChildren(...labels);
  if (labels.length) target.append('Check browser storage and reopen the original file to retry.');
}
function updateRetentionWarnings() {
  const names = [...retentionFailures.values()].reverse();
  renderRetentionWarning(retentionNotice, retentionNoticeDismissed ? [] : names.slice(0, 1));
  renderRetentionWarning($('library-retention-warning'), names);
}
$('dismiss-rom-warning').addEventListener('click', () => {
  retentionNoticeDismissed = true;
  retentionNotice.textContent = '';
  canvas.focus({ preventScroll: true });
});
const libraryStores = ['cartridges', 'roms'];
let libraryUpdates = Promise.resolve();
const removing = new Set();
const rowKeys = new WeakMap();
let libraryChanges;
try { libraryChanges = new BroadcastChannel('snes-rust:cartridges'); } catch { /* Refresh on picker open if unavailable. */ }

function hasCartridgeName(entry) {
  return typeof entry?.name === 'string' && entry.name.trim() !== '';
}
function sameCartridgeKey(a, b) {
  return a === b || indexedDB.cmp(a, b) === 0;
}
function isRemoving(hash) {
  for (const key of removing) if (sameCartridgeKey(key, hash)) return true;
  return false;
}
function showSavedRecovery(hash, name) {
  recoveryCartridge = { hash, name };
  rowKeys.set(savedRecovery, hash);
  $('remove-broken-cartridge').setAttribute('aria-disabled', String(isRemoving(hash)));
  const message = $('library-load-error');
  message.textContent = `Could not load ${name}.`;
  message.title = name;
  savedRecovery.hidden = false;
  if (picker.contains(document.activeElement)) {
    savedRecovery.scrollIntoView({ block: 'nearest' });
    message.focus({ preventScroll: true });
  }
}
function clearSavedRecovery() {
  const hadFocus = savedRecovery.contains(document.activeElement);
  recoveryCartridge = undefined;
  savedRecovery.hidden = true;
  if (hadFocus && picker.matches(':popover-open')) $('open-local').focus();
}
function renderLibrary(entries) {
  entries = entries.filter(({ hash }) => !bundledHashes.has(hash))
    .map(entry => hasCartridgeName(entry) ? entry : { hash: entry.hash, name: 'Unnamed cartridge' });
  if (recoveryCartridge && !entries.some(({ hash }) => sameCartridgeKey(hash, recoveryCartridge.hash))) {
    clearSavedRecovery();
  }
  entries.sort((a, b) => a.name.localeCompare(b.name) || indexedDB.cmp(a.hash, b.hash));
  const previous = [...savedList.children];
  if (entries.length === previous.length && entries.every(({ hash, name }, i) =>
    sameCartridgeKey(hash, rowKeys.get(previous[i])) && name === previous[i].dataset.name)) return;
  const nameCounts = new Map();
  for (const { name } of entries) nameCounts.set(name, (nameCounts.get(name) ?? 0) + 1);
  const focused = document.activeElement;
  const index = previous.indexOf(focused.parentElement);
  const rows = entries.map(({ hash, name }, i) => {
    const row = document.createElement('li');
    // Keep the original key for lookup and removal, including non-string keys.
    rowKeys.set(row, hash);
    if (typeof hash === 'string') row.dataset.hash = hash;
    row.dataset.name = name;
    const select = document.createElement('button');
    select.textContent = name;
    select.addEventListener('click', () => selectSavedCartridge(hash, name));
    const remove = document.createElement('button');
    remove.className = 'remove-cartridge';
    remove.textContent = 'Remove';
    remove.setAttribute('aria-label', `Remove ${name}`);
    remove.addEventListener('click', event => removeCartridge(hash, name, event));
    if (nameCounts.get(name) > 1) {
      const suffix = document.createElement('small');
      suffix.id = `rom-${i}`;
      suffix.textContent = isRomHash(hash) ? hash.slice(0, 12) : 'Invalid saved entry';
      select.setAttribute('aria-label', name);
      for (const button of [select, remove]) button.setAttribute('aria-describedby', suffix.id);
      select.append(suffix);
    }
    row.append(select, remove);
    return row;
  });
  savedList.replaceChildren(...rows);
  $('my-cartridges').hidden = !rows.length;
  if (index !== -1) {
    const previousKey = rowKeys.get(previous[index]);
    const row = rows.find(row => sameCartridgeKey(rowKeys.get(row), previousKey));
    const next = row?.children[focused === previous[index].lastElementChild ? 1 : 0] ??
      rows[index]?.firstElementChild ?? rows.at(-1)?.firstElementChild ?? $('open-local');
    next.focus();
  }
}
async function refreshLibrary() {
  try {
    const entries = await libraryTransaction('cartridges', 'readonly', tx => tx.objectStore('cartridges').getAll());
    renderLibrary(entries);
    libraryStatus.textContent = '';
  } catch {
    libraryStatus.textContent = 'Could not read My cartridges. Try reopening the picker or choose Open local ROM.';
  }
}
async function isCartridgeRetained(hash) {
  try {
    let entry;
    const data = await libraryTransaction(libraryStores, 'readonly', tx => {
      const metadata = tx.objectStore('cartridges').get(hash);
      metadata.onsuccess = () => { entry = metadata.result; };
      return tx.objectStore('roms').get(hash);
    });
    return Boolean(entry) && data instanceof Uint8Array && data.length <= maxRomBytes &&
      await romHash(normalizeRom(data)) === hash;
  } catch {
    return false;
  }
}
// Order UI refreshes and mutations in this tab. IndexedDB orders read/write
// transactions across tabs and keeps metadata and bytes in sync on abort.
function queueLibrary(update) { libraryUpdates = libraryUpdates.then(update); }
async function refreshOpenLibrary() {
  if (picker.matches(':popover-open')) await refreshLibrary();
}
picker.addEventListener('toggle', event => {
  if (event.newState === 'open') queueLibrary(refreshOpenLibrary);
  else clearSavedRecovery();
});
libraryChanges?.addEventListener('message', event => {
  const hash = event.data?.savedHash;
  queueLibrary(async () => {
    await refreshOpenLibrary();
    if (retentionFailures.has(hash) && await isCartridgeRetained(hash)) {
      retentionFailures.delete(hash);
      updateRetentionWarnings();
    }
  });
});

function rememberCartridge(hash, name, data) {
  queueLibrary(async () => {
    try {
      await libraryTransaction(libraryStores, 'readwrite', tx => {
        const cartridges = tx.objectStore('cartridges');
        const existing = cartridges.get(hash);
        existing.onsuccess = () => {
          if (!hasCartridgeName(existing.result)) cartridges.put({ hash, name });
        };
        // Reimporting also repairs missing or damaged bytes under the same key.
        tx.objectStore('roms').put(data, hash);
      });
    } catch {
      retentionFailures.delete(hash);
      if (!await isCartridgeRetained(hash)) {
        retentionFailures.set(hash, name);
        retentionNoticeDismissed = false;
      }
      updateRetentionWarnings();
      return;
    }
    if (retentionFailures.delete(hash)) updateRetentionWarnings();
    libraryChanges?.postMessage({ savedHash: hash });
    await refreshOpenLibrary();
  });
}
function removeCartridge(hash, name, event) {
  if (isRemoving(hash)) return;
  const button = event.currentTarget;
  if (event.detail > 0) lastRemoveClick = { click: event, hash, bounds: button.getBoundingClientRect() };
  removing.add(hash);
  button.setAttribute('aria-disabled', 'true');
  queueLibrary(async () => {
    try {
      const entries = await libraryTransaction(libraryStores, 'readwrite', tx => {
        const cartridges = tx.objectStore('cartridges');
        cartridges.delete(hash);
        tx.objectStore('roms').delete(hash);
        return cartridges.getAll();
      });
      renderLibrary(entries);
      libraryStatus.textContent = '';
      libraryChanges?.postMessage(null);
    } catch {
      libraryStatus.textContent = `Could not remove ${name}. Try Remove again.`;
    } finally {
      removing.delete(hash);
      button.removeAttribute('aria-disabled');
      if (recoveryCartridge && sameCartridgeKey(hash, recoveryCartridge.hash)) {
        $('remove-broken-cartridge').removeAttribute('aria-disabled');
      }
    }
  });
}
function selectSavedCartridge(hash, name) {
  if (loading || saveBusy || !core || isRemoving(hash)) return;
  if (restorePending) { closePicker(); $('retry-save').focus(); return; }
  void activateAudio();
  void loadCartridge(name, async () => {
    const data = await libraryTransaction('roms', 'readonly', tx => tx.objectStore('roms').get(hash));
    if (!(data instanceof Uint8Array)) throw new Error('Saved ROM bytes are missing or invalid');
    return data;
  }, { savedHash: hash });
}

function openLocalRom() {
  if (loading || saveBusy || !core) return;
  // Unlock before the native file chooser can consume transient activation.
  void activateAudio(); closePicker(); $('rom').click();
}
$('open-local').addEventListener('click', openLocalRom);
$('reopen-original-rom').addEventListener('click', openLocalRom);
$('remove-broken-cartridge').addEventListener('click', event => {
  if (recoveryCartridge) removeCartridge(recoveryCartridge.hash, recoveryCartridge.name, event);
});
$('rom').addEventListener('change', async (event) => {
  const file = event.target.files[0]; if (!file) return;
  void activateAudio();
  try {
    await loadCartridge(file.name, async () => {
      if (!/\.(sfc|smc)$/i.test(file.name)) throw new Error('Choose an .sfc or .smc cartridge image');
      if (file.size > maxRomBytes) throw new Error(romSizeError);
      return new Uint8Array(await file.arrayBuffer());
    });
  } finally { event.target.value = ''; }
});
for (const button of demoButtons) button.addEventListener('click', async () => {
  if (loading || saveBusy || !core) return;
  closePicker();
  if (restorePending) { $('retry-save').focus(); return; }
  void activateAudio();
  const demo = demos[button.id];
  // Reselecting the inserted demo never reloads or changes playback state.
  if (activeHash === demo.hash) {
    if (!document.hidden) canvas.focus({ preventScroll: true });
    return;
  }
  await loadCartridge(demo.name, async () => {
    const response = await fetch(demo.file, { mode: 'same-origin', redirect: 'error' });
    if (!response.ok) throw new Error(`Demo download failed: HTTP ${response.status}`);
    return new Uint8Array(await response.arrayBuffer());
  }, { demo });
});
$('play').addEventListener('click', togglePlayback);
$('reset').addEventListener('click', () => {
  if (!loaded || restorePending || loading || saveBusy) return;
  void pause();
  try {
    check(core.reset()); draw(); clearError();
    void activateAudio(); start();
    if (isRunning()) canvas.focus({ preventScroll: true });
  } catch (e) { report(e); }
});
$('mute').addEventListener('click', () => setVolume(volume ? 0 : lastNonzeroVolume));
$('volume').addEventListener('input', () => setVolume(Number($('volume').value)));
$('scale').addEventListener('change', event => {
  canvas.parentElement.style.setProperty('--video-width', `${256 * Number(event.target.value)}px`);
});
const presentation = $('game-presentation'), fullscreenButton = $('fullscreen');
const fullscreenLabel = fullscreenButton.querySelector('.fullscreen-label');
const fullscreenStatus = $('fullscreen-status');
const fullscreenControls = presentation.querySelector('.fullscreen-controls');
const fullscreenFeedback = presentation.querySelector('.fullscreen-feedback');
const playbackStatusRow = document.querySelector('.playback-status');
const touchControls = document.querySelector('.touch-controls');
const shortLandscape = matchMedia('(orientation: landscape) and (max-height: 600px) and (max-width: 1100px)');
const playerChrome = document.querySelectorAll('header, .toolbar, .game-entry, #retry-save');
let gameplayFullscreen = false;
let fullscreenEnteredWithKeyboard = false;
let fullscreenWarningTimeout;
function clearFullscreenWarning() {
  clearTimeout(fullscreenWarningTimeout);
  fullscreenStatus.textContent = '';
}
function setStatus(text) {
  clearFullscreenWarning();
  $('status').textContent = text;
}
async function toggleFullscreen() {
  clearFullscreenWarning();
  try {
    if (document.fullscreenElement === presentation) {
      // The Exit button can still own focus after fullscreenElement clears and
      // before fullscreenchange runs. Hand gameplay input back before exiting.
      if (!fullscreenEnteredWithKeyboard) canvas.focus({ preventScroll: true });
      await document.exitFullscreen();
    } else {
      if (!document.fullscreenEnabled || !presentation.requestFullscreen) throw new Error();
      await presentation.requestFullscreen();
    }
  } catch {
    if (document.fullscreenElement !== presentation) {
      clearTimeout(fullscreenWarningTimeout);
      fullscreenStatus.textContent = 'Fullscreen unavailable. Try another browser.';
      fullscreenWarningTimeout = setTimeout(clearFullscreenWarning, 4000);
    }
  }
}
fullscreenButton.addEventListener('click', event => {
  fullscreenEnteredWithKeyboard = event.detail === 0;
  void toggleFullscreen();
});
$('exit-fullscreen').addEventListener('click', () => {
  if (document.fullscreenElement === presentation) void toggleFullscreen();
});
function placeTouchControls() {
  // Match the short landscape layout in style.css, where the controller flanks the game.
  const parent = gameplayFullscreen || shortLandscape.matches ? presentation : presentation.parentElement;
  if (touchControls.parentElement !== parent) parent.append(touchControls);
}
shortLandscape.addEventListener('change', placeTouchControls);
placeTouchControls();
document.addEventListener('fullscreenchange', () => {
  const active = document.fullscreenElement === presentation;
  if (active === gameplayFullscreen) return;
  gameplayFullscreen = active;
  const label = active ? 'Exit fullscreen' : 'Fullscreen';
  fullscreenLabel.textContent = label;
  fullscreenButton.ariaLabel = label;
  fullscreenButton.title = label;
  if (active) {
    fullscreenControls.append(stateControls);
    fullscreenFeedback.append($('error'), $('save-status'));
  } else {
    // A native file chooser can leave fullscreen before returning the file.
    if (statePicker.open && !stateView.importing) closeStatePicker();
    fullscreenButton.before(stateControls);
    playbackStatusRow.append($('save-status'));
    $('audio-status').before($('error'));
  }
  // Keep focus and assistive technology inside the fullscreen presentation.
  for (const element of playerChrome) element.inert = active;
  clearVirtualInput();
  placeTouchControls();
  if (statePicker.open) return;
  if (!active && fullscreenEnteredWithKeyboard) fullscreenButton.focus();
  else canvas.focus({ preventScroll: true });
});
const help = $('help');
document.addEventListener('pointerdown', (event) => {
  if (help.open && !help.contains(event.target)) help.open = false;
});
help.addEventListener('focusout', (event) => {
  if (!help.contains(event.relatedTarget)) help.open = false;
});
for (const type of ['keydown', 'keyup']) window.addEventListener(type, (event) => {
  const target = event.target;
  const inHelp = target.closest?.('#help');
  if (type === 'keydown' && event.key === 'Escape' && statePicker.open) {
    event.preventDefault();
    if (removeStateDialog.open) removeStateDialog.close('cancel');
    else if (!event.repeat) closeStatePicker();
    return;
  }
  if (type === 'keydown' && event.key === 'Escape' && pendingStateLoad) {
    pendingStateLoad.cancelled = true;
    event.preventDefault();
    return;
  }
  if (type === 'keydown' && event.key === 'Escape' && document.fullscreenElement === presentation) {
    event.preventDefault();
    if (!event.repeat) void toggleFullscreen();
    return;
  }
  if (type === 'keydown' && event.key === 'Escape' && help.open) {
    help.open = false;
    if (inHelp) {
      event.preventDefault();
      help.querySelector('summary').focus({ preventScroll: true });
    }
    return;
  }
  if (!keys[event.code] || !isRunning()) return;
  const inControl = statePicker.open || (inHelp && help.open) ||
    target.closest?.('#cartridge-picker, .save-dialog, .state-controls, .fullscreen-feedback') ||
    ['INPUT', 'SELECT'].includes(target.tagName) ||
    (['BUTTON', 'A', 'SUMMARY'].includes(target.tagName) && ['Enter', 'Space'].includes(event.code));
  if (inControl && !(type === 'keyup' && held.has(event.code))) return;
  event.preventDefault();
  if (type === 'keydown' && event.repeat) return;
  if (type === 'keydown') held.add(event.code); else held.delete(event.code);
  keyboard = [...held].reduce((value, code) => value | keys[code], 0);
});
window.addEventListener('blur', clearInput);
window.addEventListener('resize', clearVirtualInput);
window.addEventListener('orientationchange', clearVirtualInput);
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
  // Compact fullscreen columns can place a button across the D-pad's thirds.
  const button = document.elementFromPoint(event.clientX, event.clientY)?.closest('[data-button]');
  if (button && element.contains(button)) return Number(button.dataset.button);
  return (x < 1/3 ? 0x200 : x > 2/3 ? 0x100 : 0) | (y < 1/3 ? 0x800 : y > 2/3 ? 0x400 : 0);
}
touchControls.addEventListener('contextmenu', event => event.preventDefault());
touchControls.addEventListener('pointerdown', event => {
  if (!isRunning() || (event.pointerType === 'mouse' && event.button !== 0)) return;
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
    if (!['Space', 'Enter'].includes(event.code)) return;
    event.preventDefault(); event.stopPropagation();
    if (!isRunning() || event.repeat) return;
    virtualKeys.set(button, Number(button.dataset.button)); paintVirtualInput();
  });
  button.addEventListener('keyup', event => {
    if (!['Space', 'Enter'].includes(event.code)) return;
    event.preventDefault(); event.stopPropagation(); virtualKeys.delete(button); paintVirtualInput();
  });
  button.addEventListener('blur', () => { virtualKeys.delete(button); paintVirtualInput(); });
}
document.addEventListener('visibilitychange', event => {
  if (!syncPlayback(true)) return;
  const saved = save(event);
  if (loading) loading.savedOnSuspend = saved;
});
window.addEventListener('pagehide', event => { void save(event); });

const profileDialog = $('save-data-dialog'), profileList = $('save-profiles');
const statePicker = $('state-picker'), stateToggle = $('choose-state'), newState = $('new-state');
const stateControls = stateToggle.parentElement;
const stateClose = $('close-state-picker'), importState = $('import-state'), stateFile = $('state-file');
const stateList = $('state-history'), stateMessage = $('state-message'), moreStates = $('more-states');
const removeStateDialog = $('remove-state-dialog');
for (const controls of [profileDialog, statePicker, stateControls, removeStateDialog]) {
  controls.addEventListener('keydown', event => {
    if (event.key === 'Enter' && event.repeat) event.preventDefault();
  });
}
let stateView, profileView, pendingStateLoad, saveNoticeTimer;
const saveDestinations = new Map();

function requireProfile(catalog, id) {
  const profile = catalog.profiles.find(profile => profile.id === id);
  if (!profile) throw new Error('This profile was removed. Export or duplicate any unsaved SRAM before reloading');
  return profile;
}

function updateSaveControls() {
  const disabled = !loaded || Boolean(loading) || restorePending;
  if (disabled && statePicker.open) closeStatePicker();
  const key = saveDestination(saveScope());
  updateStateTarget(key);
  $('save-data').hidden = !activeProfile;
  $('active-save-name').textContent = activeProfile?.name ?? '';
  for (const button of [$('save-data'), $('save-state'), stateToggle, $('load-state')]) {
    button.disabled = disabled || (button.id === 'load-state' && !key);
    if (saveBusy) button.setAttribute('aria-disabled', 'true');
    else button.removeAttribute('aria-disabled');
  }
}

async function saveAction(dialog, action) {
  if (!loaded || loading || restorePending || saveBusy) return;
  const message = dialog === profileDialog ? $('save-data-message') : dialog === statePicker ? stateMessage : $('save-status');
  saveBusy = true;
  updateSaveControls();
  clearError();
  message.textContent = '';
  try {
    await action();
  } catch (error) {
    if (dialog?.open) message.textContent = error.message;
    else report(error);
  } finally {
    core.discard_prepared_state();
    saveBusy = false;
    updateSaveControls();
  }
}

function finishSaveRestore(base, epoch) {
  check(core.commit_prepared_state());
  trackSram(base, epoch);
  clearInput();
  flushAudio();
  persistenceError = '';
  showErrors();
  lastTime = undefined;
  debt = frameMilliseconds;
  saveClock = performance.now();
  draw();
  setStatus(playbackStatus());
  updateSaveControls();
}

function button(label, click, accessibleName) {
  const control = document.createElement('button');
  control.type = 'button';
  control.textContent = label;
  if (accessibleName) control.setAttribute('aria-label', accessibleName);
  control.addEventListener('click', click);
  return control;
}

function downloadSave(data, name) {
  const url = URL.createObjectURL(new Blob([data], { type: 'application/octet-stream' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = name;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

function liveSram() {
  return bytes(core.sram_buffer(), core.sram_len()).slice();
}

function preparedSram() {
  return bytes(core.prepared_state_sram_buffer(), core.sram_len()).slice();
}

function prepareProfile(data) {
  if (data !== null) copyInput(data);
  check(core.prepare_profile(data !== null ? 1 : 0));
}

function changeProfile(id, mode, data) {
  const view = profileView;
  return saveAction(profileDialog, () => lockSaves(activeHash, () => {
    if (!profileDialog.open || view !== profileView) return;
    if (id === activeProfile.id) { profileDialog.close(); return; }
    let catalog = readProfiles(activeHash);
    const retained = catalog.profiles.some(profile => profile.id === activeProfile.id);
    if (retained) {
      try { persistActiveSram(encodeSram(liveSram())); }
      catch (error) { finishSramSave(error); throw error; }
      catalog = readProfiles(activeHash);
    }
    if (!retained && mode !== 'duplicate') requireProfile(catalog, activeProfile.id);
    let profile, base;
    if (id) {
      profile = requireProfile(catalog, id);
      base = readSramData(activeHash, id);
      prepareProfile(decodeSram(base, core.sram_len()));
      catalog.selected = id;
      writeProfiles(activeHash, catalog);
    } else {
      prepareProfile(mode === 'duplicate' ? liveSram() : data ?? null);
      profile = addStoredProfile(activeHash, catalog, preparedSram());
      catalog.selected = profile.id;
      persistNewProfile(activeHash, catalog, profile);
      base = readSramData(activeHash, profile.id);
    }
    activeProfile = profile;
    finishSaveRestore(base, profile.epoch);
    profileDialog.close();
  }));
}

function deleteProfile(id) {
  return saveAction(profileDialog, () => lockSaves(activeHash, async () => {
    const hash = activeHash;
    const view = profileView;
    const inUse = await historyExists({ hash, profile: id });
    if (!profileDialog.open || view !== profileView) return;
    const catalog = readProfiles(hash);
    const profile = requireProfile(catalog, id);
    requireProfile(catalog, activeProfile.id);
    if (inUse) throw new Error(`Remove the saved states for ${profile.name} before deleting its save data.`);
    if (!confirm(`Delete ${profile.name} and its game save data?`)) return;
    const isActive = activeProfile.id === id;
    catalog.profiles = catalog.profiles.filter(item => item.id !== id);
    if (isActive) {
      const replacement = catalog.profiles.find(item => item.id === catalog.selected) ?? catalog.profiles[0];
      const base = replacement ? readSramData(hash, replacement.id) : null;
      prepareProfile(decodeSram(base, core.sram_len()));
      activeProfile = replacement ?? addStoredProfile(hash, catalog, preparedSram());
      catalog.selected = activeProfile.id;
      try {
        if (replacement) writeProfiles(hash, catalog);
        else persistNewProfile(hash, catalog, activeProfile);
      } catch (error) {
        activeProfile = profile;
        throw error;
      }
      finishSaveRestore(replacement ? base : readSramData(hash, activeProfile.id), activeProfile.epoch);
    } else {
      if (catalog.selected === id) catalog.selected = activeProfile.id;
      writeProfiles(hash, catalog);
    }
    removeSram(hash, id);
    renderProfiles();
    profileDialog.querySelector('[autofocus]').focus();
  }));
}

function renderProfiles() {
  const catalog = readProfiles(activeHash);
  profileList.replaceChildren();
  for (const profile of catalog.profiles) {
    const row = document.createElement('li');
    const selected = profile.id === activeProfile.id;
    const select = button(profile.name, () => void changeProfile(profile.id));
    if (selected) select.setAttribute('aria-current', 'true');
    row.append(select);
    const time = document.createElement('span');
    time.className = 'save-time';
    try {
      const timestamp = sramTimestamp(activeHash, profile.id);
      time.textContent = timestamp ? new Date(timestamp).toLocaleString() : 'No recorded save time';
    } catch { time.textContent = 'Save data unavailable'; }
    row.append(time, button('Delete', () => void deleteProfile(profile.id), `Delete ${profile.name}`));
    profileList.append(row);
  }
}

$('save-data').addEventListener('click', () => {
  if (!activeProfile || loading || saveBusy || restorePending) return;
  profileView = {};
  $('save-data-message').textContent = '';
  profileDialog.showModal();
  try {
    renderProfiles();
    if (hasPendingSram(activeHash)) {
      const view = profileView;
      void saveAction(profileDialog, () => lockSaves(activeHash, () => {
        if (!profileDialog.open || view !== profileView) return;
        settlePendingSram(activeHash, core.sram_len());
        renderProfiles();
      }));
    }
  }
  catch (error) { $('save-data-message').textContent = error.message; }
});
$('new-save-profile').addEventListener('click', () => void changeProfile());
$('duplicate-save-profile').addEventListener('click', () => void changeProfile(undefined, 'duplicate'));
$('export-sram').addEventListener('click', () => void saveAction(profileDialog, () => {
  downloadSave(liveSram(), `${activeHash.slice(0, 12)}-${activeProfile.id}.srm`);
}));
$('import-sram').addEventListener('click', () => { if (!saveBusy) $('sram-file').click(); });
$('sram-file').addEventListener('change', async event => {
  const file = event.target.files[0];
  event.target.value = '';
  if (!file || !profileDialog.open) return;
  const view = profileView;
  await saveAction(profileDialog, async () => {
    if (file.size !== core.sram_len()) throw new Error(`Choose raw SRAM of exactly ${core.sram_len()} bytes for this cartridge.`);
    const data = new Uint8Array(await file.arrayBuffer());
    await lockSaves(activeHash, () => {
      if (!profileDialog.open || view !== profileView) return;
      const catalog = readProfiles(activeHash);
      const profile = addStoredProfile(activeHash, catalog, data);
      persistNewProfile(activeHash, catalog, profile);
      renderProfiles();
      $('save-data-message').textContent = `Imported as ${profile.name}. Select it to play.`;
    });
  });
});

function saveScope() {
  return { hash: activeHash, profile: activeProfile?.id ?? '' };
}

const destinationScope = ({ hash, profile }) => `${hash}:${profile}`;
const saveDestination = scope => saveDestinations.get(destinationScope(scope));
const stateTime = (key, short = false) => new Date(key[2]).toLocaleString(undefined, {
  year: short ? undefined : 'numeric', month: 'numeric', day: 'numeric',
  hour: '2-digit', minute: '2-digit', second: '2-digit', hourCycle: 'h23',
});

function updateStateTarget(key) {
  const label = $('state-target');
  label.textContent = key ? stateTime(key, true) : 'New state';
  label.title = key ? stateTime(key) : '';
}

function selectSaveDestination(scope, key) {
  if (key) saveDestinations.set(destinationScope(scope), key);
  else saveDestinations.delete(destinationScope(scope));
  updateSaveControls();
}

function ensureScope(scope) {
  if (scope.hash !== activeHash || scope.profile !== (activeProfile?.id ?? '')) {
    throw new Error('The cartridge or save data profile changed. Try again.');
  }
  if (scope.profile) requireProfile(readProfiles(scope.hash), scope.profile);
}

function thumbnail(pointer, width) {
  const raw = document.createElement('canvas');
  raw.width = width;
  raw.height = 224;
  const rawContext = raw.getContext('2d');
  const pixelData = rawContext.createImageData(width, raw.height);
  pixelData.data.set(bytes(pointer, pixelData.data.length));
  rawContext.putImageData(pixelData, 0, 0);
  const preview = document.createElement('canvas');
  preview.width = 128;
  preview.height = 112;
  preview.getContext('2d', { alpha: false }).drawImage(raw, 0, 0, preview.width, preview.height);
  return new Promise((resolve, reject) => preview.toBlob(blob => {
    if (blob) resolve(blob);
    else reject(new Error('Could not create a screenshot'));
  }, 'image/jpeg', .78));
}

$('save-state').addEventListener('click', () => void saveAction(null, async () => {
  if (statePicker.open) closeStatePicker();
  const scope = saveScope();
  const destination = saveDestination(scope);
  const time = Date.now();
  check(core.save_snapshot());
  const snapshot = bytes(core.snapshot_buffer(), core.snapshot_len()).slice();
  const preview = await thumbnail(core.framebuffer(), core.framebuffer_width());
  const key = await lockSaves(scope.hash, () => {
    ensureScope(scope);
    return writeState(scope, time, snapshot, preview, destination);
  });
  selectSaveDestination(scope, key);
  if (!key) throw new Error('The save destination no longer exists. Save again to create a new state.');
  const message = $('save-status');
  message.textContent = 'State saved.';
  clearTimeout(saveNoticeTimer);
  saveNoticeTimer = setTimeout(() => {
    if (message.textContent === 'State saved.') message.textContent = '';
  }, 3000);
}));

function clearHistory(view) {
  if (!view) return;
  for (const url of view.previews) URL.revokeObjectURL(url);
  view.previews.clear();
  stateList.replaceChildren();
  moreStates.hidden = true;
  view.before = undefined;
}

function isHistoryOpen(view) {
  return view && view === stateView && statePicker.open;
}

async function showHistory(view, append = false) {
  const { entries, more } = await historyPage(view.scope, append ? view.before : undefined);
  if (!isHistoryOpen(view)) return;
  if (!append) clearHistory(view);
  stateMessage.textContent = stateList.children.length || entries.length ? '' : 'No saved states yet.';
  const selected = saveDestination(view.scope);
  newState.setAttribute('aria-current', String(!selected));
  let first;
  for (const { key, thumbnail } of entries) {
    view.before = key;
    const row = document.createElement('li');
    const time = stateTime(key);
    const action = button('', () => chooseState(view, key), `Select state from ${time}`);
    action.className = 'state-option';
    if (key[3] === selected?.[3]) action.setAttribute('aria-current', 'true');
    const image = document.createElement('img');
    image.alt = '';
    image.width = 128;
    image.height = 112;
    const url = URL.createObjectURL(thumbnail);
    image.src = url;
    view.previews.add(url);
    const timestamp = document.createElement('time');
    timestamp.dateTime = new Date(key[2]).toISOString();
    timestamp.textContent = time;
    const mark = document.createElement('span');
    mark.className = 'state-check';
    mark.setAttribute('aria-hidden', 'true');
    mark.textContent = '✓';
    action.append(image, timestamp, mark);
    const remove = button('Remove', () => {
      if (saveBusy) return;
      $('remove-state-message').textContent = `Saved state from ${time}`;
      removeStateDialog.returnValue = '';
      removeStateDialog.addEventListener('close', () => {
        if (removeStateDialog.returnValue === 'remove' && isHistoryOpen(view)) {
          void deleteHistoryState(view, key, row, url);
        }
      }, { once: true });
      remove.focus({ preventScroll: true });
      // Browser confirmation prompts exit element fullscreen.
      removeStateDialog.showModal();
    }, `Remove state from ${time}`);
    row.append(
      action,
      button('Export', () => void exportHistoryState(view, key), `Export state from ${time}`),
      remove);
    stateList.append(row);
    first ??= action;
  }
  moreStates.hidden = !more;
  if (append) first?.focus();
}

function chooseState(view, key) {
  if (!isHistoryOpen(view) || saveBusy) return;
  selectSaveDestination(view.scope, key);
  closeStatePicker();
}

function closeStatePicker() {
  if (removeStateDialog.open) removeStateDialog.close('cancel');
  statePicker.close();
  clearHistory(stateView);
  stateView = undefined;
  stateToggle.focus({ preventScroll: true });
}

stateToggle.addEventListener('click', () => {
  if (!loaded || loading || saveBusy || restorePending || statePicker.open) return;
  clearInput();
  const view = stateView = { scope: saveScope(), previews: new Set() };
  newState.setAttribute('aria-current', String(!saveDestination(view.scope)));
  stateMessage.textContent = 'Loading history…';
  statePicker.showModal();
  void showHistory(view).catch(error => {
    if (isHistoryOpen(view)) stateMessage.textContent = error.message;
  });
});
statePicker.addEventListener('cancel', event => {
  event.preventDefault();
  closeStatePicker();
});
statePicker.addEventListener('keydown', event => {
  if (event.key !== 'Tab') return;
  const last = moreStates.hidden ? importState : moreStates;
  if (event.target !== statePicker && event.target !== (event.shiftKey ? stateClose : last)) return;
  event.preventDefault();
  (event.shiftKey ? last : stateClose).focus();
});
stateClose.addEventListener('click', closeStatePicker);
$('state-picker-backdrop').addEventListener('click', closeStatePicker);
newState.addEventListener('click', () => chooseState(stateView));
moreStates.addEventListener('click', () => {
  if (isHistoryOpen(stateView)) void saveAction(statePicker, () => showHistory(stateView, true));
});

function prepareSnapshot(snapshot) {
  if (!(snapshot instanceof Uint8Array)) throw new Error('The saved state is missing or invalid.');
  if (snapshot.length > core.snapshot_max_len()) throw new Error('Save state exceeds the supported size.');
  copyInput(snapshot);
  check(core.prepare_snapshot());
}

function preserveSram(scope) {
  const { hash, profile } = scope;
  if (!profile) return;
  const current = liveSram(), incoming = preparedSram();
  const base = readSramData(hash, profile);
  const persisted = decodeSram(base, current.length);
  const priorImages = [{ data: current, source: 'Current game' }];
  if (persisted && !sameSram(current, persisted)) priorImages.push({ data: persisted, source: 'Last saved' });
  // The ROM lock serializes replay with this restore. Preserve journal images
  // alongside live and saved SRAM so replay cannot replace the selected bytes.
  const pending = [...readPendingSram(hash, current.length, { profileId: profile })];
  for (const { data } of pending) {
    if (!priorImages.some(image => sameSram(data, image.data))) priorImages.push({ data, source: 'Pending save' });
  }
  const recover = priorImages.filter(({ data }) => !sameSram(data, incoming));
  for (const { data, source } of recover) recoverSram(hash, profile, data, source);
  // Any displaced image is now durable and visible. If this write fails,
  // both the inserted machine and the prior persisted bytes remain intact.
  const data = encodeSram(incoming);
  const catalog = readProfiles(hash);
  const record = requireProfile(catalog, profile);
  const previousEpoch = record.epoch;
  // Revoke old write claims even when the bytes match. A tab can stage an
  // exit save outside the lock or in session storage this tab cannot read.
  record.epoch = crypto.randomUUID();
  writeProfiles(hash, catalog);
  if (data !== base) {
    try {
      writeSramData(hash, profile, data);
    } catch (error) {
      // The SRAM write was rejected. Restore the prior epoch under the same lock
      // so tabs with the unchanged SRAM can still save to this profile.
      record.epoch = previousEpoch;
      writeProfiles(hash, catalog);
      throw error;
    }
  }
  for (const { storage, key } of pending) {
    // Each entry is now recoverable. If cleanup fails, the new epoch still
    // prevents the retained entry from overwriting the restored SRAM.
    try { storage.removeItem(key); } catch {}
  }
  return { data, epoch: record.epoch, recovered: Boolean(recover.length) };
}

$('load-state').addEventListener('click', () => void saveAction(null, async () => {
  const scope = saveScope();
  const key = saveDestination(scope);
  if (!key) return;
  if (statePicker.open) closeStatePicker();
  const load = pendingStateLoad = { cancelled: false };
  try {
    const snapshot = await readState(key);
    if (load.cancelled) return;
    await lockSaves(scope.hash, () => {
      if (load.cancelled) return;
      ensureScope(scope);
      if (snapshot === undefined) selectSaveDestination(scope);
      prepareSnapshot(snapshot);
      let protectedSram;
      try { protectedSram = preserveSram(scope); }
      catch (error) { throw new Error(`Could not protect SRAM before restoring: ${error.message}. Retry when storage is available.`); }
      finishSaveRestore(protectedSram?.data, protectedSram?.epoch);
      $('save-status').textContent = protectedSram?.recovered ? 'State loaded. Previous SRAM is available in Save data.' : 'State loaded.';
    });
  } finally {
    pendingStateLoad = undefined;
  }
}));

function exportHistoryState(view, key) {
  return saveAction(statePicker, async () => {
    const snapshot = await readState(key);
    if (!isHistoryOpen(view)) return;
    prepareSnapshot(snapshot);
    downloadSave(snapshot, `${key[0].slice(0, 12)}-${key[2]}-${key[3]}.snesstate`);
  });
}

function deleteHistoryState(view, key, row, url) {
  return saveAction(statePicker, async () => {
    await lockSaves(view.scope.hash, () => removeState(key));
    if (saveDestination(view.scope)?.[3] === key[3]) selectSaveDestination(view.scope);
    if (!isHistoryOpen(view)) return;
    newState.setAttribute('aria-current', String(!saveDestination(view.scope)));
    const next = row.nextElementSibling ?? row.previousElementSibling;
    row.remove();
    URL.revokeObjectURL(url);
    view.previews.delete(url);
    if (!stateList.children.length && moreStates.hidden) stateMessage.textContent = 'No saved states yet.';
    (next?.querySelector('button') ?? newState).focus();
  });
}

importState.addEventListener('click', () => {
  if (saveBusy || !isHistoryOpen(stateView)) return;
  stateView.importing = true;
  stateFile.click();
});
stateFile.addEventListener('cancel', () => {
  if (stateView) stateView.importing = false;
});
stateFile.addEventListener('change', async event => {
  const file = event.target.files[0];
  event.target.value = '';
  const view = stateView;
  if (!file || !isHistoryOpen(view)) return;
  await saveAction(statePicker, async () => {
    if (file.size > core.snapshot_max_len()) throw new Error('Save state exceeds the supported size.');
    const time = Date.now();
    const snapshot = new Uint8Array(await file.arrayBuffer());
    if (!isHistoryOpen(view)) return;
    ensureScope(view.scope);
    prepareSnapshot(snapshot);
    const preview = await thumbnail(core.prepared_state_framebuffer(), core.prepared_state_framebuffer_width());
    await lockSaves(view.scope.hash, () => {
      if (!isHistoryOpen(view)) return;
      ensureScope(view.scope);
      return writeState(view.scope, time, snapshot, preview);
    });
    await showHistory(view);
    if (isHistoryOpen(view)) stateMessage.textContent = 'State imported.';
  }).finally(() => {
    view.importing = false;
  });
});

try {
  const response = await fetch('snes_wasm.wasm'); if (!response.ok) throw new Error(`Wasm download failed: HTTP ${response.status}`);
  const result = await WebAssembly.instantiate(await response.arrayBuffer(), {}); core = result.instance.exports;
  setStatus('Ready'); enableSources(true); requestAnimationFrame(animate);
} catch (e) { report(e); setStatus('Emulator unavailable'); enableSources(false); }
