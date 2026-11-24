// Install before the app so the probe can follow the production field loop.
export function installPlaybackProbe({ masks, warmup, frames, mode }) {
  localStorage.setItem('snes-rust:display', mode);
  const probe = window.bench = {
    frames: 0, timings: [], core: [], arrivals: [], audio: 0, gaps: 0,
    late: 0, gapSeconds: 0, cancelledAudio: 0, lastFrame: 0,
  };
  const withinWindow = () => probe.start !== undefined && probe.end === undefined;
  const displayChanges = [];
  let activeMode = mode;
  const select = HTMLSelectElement.prototype;
  const value = Object.getOwnPropertyDescriptor(select, 'value');
  Object.defineProperty(select, 'value', { ...value, set(next) {
    value.set.call(this, next);
    // The app sets this control when changing renderers, including in a
    // render callback or a context-loss event between callbacks.
    if (this.id !== 'display' || probe.end !== undefined) return;
    const selected = value.get.call(this);
    if (selected === activeMode) return;
    activeMode = selected;
    displayChanges.push({ mode: selected, beforeWindow: !withinWindow(),
      fields: probe.frames, time: performance.now() });
  } });

  const instantiate = WebAssembly.instantiate;
  WebAssembly.instantiate = async (...args) => {
    const result = await instantiate(...args);
    const exports = result.instance.exports;
    return { ...result, instance: { exports: {
      ...exports,
      run_frame(buttons) {
        if (probe.end !== undefined) return exports.run_frame(buttons);
        const start = performance.now();
        const ok = exports.run_frame(masks[probe.frames] ?? 0);
        if (!ok) return ok;
        ++probe.frames;
        if (withinWindow()) probe.core.push(performance.now() - start);
        return ok;
      },
    } } };
  };
  const raf = window.requestAnimationFrame;
  window.requestAnimationFrame = callback => raf.call(window, time => {
    const before = probe.frames;
    const start = performance.now();
    callback(time);
    if (probe.frames === before) return;
    const end = performance.now();
    // Catch-up callbacks schedule audio per field but present only once.
    // Use whole callbacks at both boundaries to measure the same work.
    if (probe.start === undefined) {
      if (probe.frames >= warmup) {
        probe.start = end;
        probe.warmupFields = probe.frames;
        probe.lastFrame = time;
        probe.modeAtStart = activeMode;
      }
      return;
    }
    probe.timings.push(end - start);
    probe.arrivals.push(time - probe.lastFrame);
    probe.lastFrame = time;
    if (probe.frames - probe.warmupFields >= frames) {
      probe.end = end;
      probe.modeAtEnd = activeMode;
      probe.mode = displayChanges.some(change => !change.beforeWindow) ? 'mixed' : probe.modeAtStart;
      probe.displayChanges = displayChanges.map(({ mode, beforeWindow, fields, time }) => ({
        mode, beforeWindow, afterFields: fields - probe.warmupFields, elapsedMs: time - probe.start,
      }));
      probe.status = document.getElementById('display-status').textContent;
    }
  });
}
