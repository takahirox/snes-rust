// Runs and validates each 900-frame window as it completes. A late freeze fails
// within 15 emulated seconds even if title/early gameplay/audio are still valid.
export async function replay(page, game, frames = 18100) {
  return page.evaluate(async ({ game, frames }) => {
    const core = window.testCore; const decoder = new TextDecoder();
    const require = (ok, message, frame) => { if (!ok) throw new Error(`${game.name} frame ${frame}: ${message}`); };
    const word = (a) => core.peek(a) | core.peek(a + 1) << 8;
    const hash = () => { const bytes = new Uint8Array(core.memory.buffer, core.framebuffer(), 256 * 224 * 4); let h = 2166136261; for (const b of bytes) h = Math.imul(h ^ b, 16777619) >>> 0; return h.toString(16).padStart(8, '0'); };
    const newWindow = (start) => ({ start, activeFrames: 0, movementFrames: 0, jumps: 0, transitions: {}, hashes: new Set(), nonzero: 0, count: 0, energy: 0 });
    let sample = newWindow(0), previous, nonzero = 0, energy = 0, count = 0;
    let audioBaseline = 0, maxAudioDrift = 0, minAudio = Infinity, maxAudio = 0;
    const windows = [], transitions = {}; const begin = performance.now();
    for (let frame = 0; frame < frames; frame++) {
      const cycle = frame % 900;
      // Frequent fresh Start edges leave both Gothicvania's Game Over AND title
      // screens. Run, jump and attack recur throughout, including the last window.
      let buttons = frame % 120 >= 60 && frame % 120 < 62 ? 0x1000 : 0;
      if (cycle >= 120 && cycle < 780) buttons |= 0x100;
      if (frame % 90 >= 30 && frame % 90 < 42) buttons |= 0x8000;
      if (frame % 120 >= 85 && frame % 120 < 90) buttons |= 0x4000;
      require(core.run_frame(buttons), decoder.decode(new Uint8Array(core.memory.buffer, core.error_buffer(), core.error_len())), frame);
      const state = core.peek(game.state), x = word(game.x), y = word(game.y);
      const vy = (word(game.vy) << 16) >> 16;
      const ground = game.groundBytes === 1 ? core.peek(game.ground) : word(game.ground);
      if (state === game.playing) {
        sample.activeFrames++;
        if (previous?.state === state) {
          // Count small positive player displacements under Right, excluding
          // initialization, map loads, knockback and respawn teleports.
          if (buttons & 0x100 && x > previous.x && x - previous.x <= 4) sample.movementFrames++;
          // Both sources require grounding and a fresh B edge to initiate a jump.
          if (buttons & 0x8000 && previous.ground && !ground && vy < 0 && y <= previous.y) sample.jumps++;
        }
      }
      if (previous && previous.state !== state) {
        const key = `${previous.state}:${state}`;
        sample.transitions[key] = (sample.transitions[key] ?? 0) + 1;
        transitions[key] = (transitions[key] ?? 0) + 1;
      }
      previous = { state, x, y, ground };
      const audio = new Float32Array(core.memory.buffer, core.audio_buffer(), core.audio_len());
      for (const v of audio) {
        require(Number.isFinite(v), 'non-finite audio', frame);
        if (v) { sample.nonzero++; nonzero++; }
        sample.energy += v * v; energy += v * v;
      }
      count += audio.length; sample.count += audio.length;
      // APU cadence vs alternating NTSC fields, after boot/DMA startup. Bound
      // cumulative drift as well as individual buffers: no growing A/V backlog.
      const fields = frame + 1;
      const elapsedClocks = fields * 357368 - Math.floor(fields / 2) * 4;
      const expectedSamples = elapsedClocks / 21477272 * 64000;
      if (frame === 899) audioBaseline = count - expectedSamples;
      if (frame >= 900) {
        minAudio = Math.min(minAudio, audio.length); maxAudio = Math.max(maxAudio, audio.length);
        maxAudioDrift = Math.max(maxAudioDrift, Math.abs(count - expectedSamples - audioBaseline));
        require(audio.length >= 960 && audio.length <= 1170, `audio buffer length ${audio.length}`, frame);
        require(maxAudioDrift <= 128, `cumulative audio drift ${maxAudioDrift} samples exceeds 2 ms`, frame);
      }
      if (frame % 6 === 0) sample.hashes.add(hash());
      if (cycle === 899) {
        const summary = { ...sample, end: frame, distinctHashes: sample.hashes.size, audioRms: Math.sqrt(sample.energy / sample.count), last: { state, x, y, hash: hash() } };
        delete summary.hashes; delete summary.energy;
        require(sample.movementFrames >= 20, `window ${sample.start}-${frame}: controller-driven movement stopped (${sample.movementFrames} frames)`, frame);
        require(sample.jumps >= 1, `window ${sample.start}-${frame}: no grounded B jump`, frame);
        require(sample.activeFrames >= 90, `window ${sample.start}-${frame}: insufficient active gameplay`, frame);
        require(sample.hashes.size >= 20, `window ${sample.start}-${frame}: video stopped changing`, frame);
        if (game.sound) require(sample.nonzero > 10000 && summary.audioRms > .001, 'game audio went silent', frame);
        windows.push(summary); sample = newWindow(frame + 1);
      }
      if (frame % 120 === 0) await new Promise((resolve) => setTimeout(resolve, 0));
    }
    const image = new ImageData(new Uint8ClampedArray(core.memory.buffer, core.framebuffer(), 256 * 224 * 4).slice(), 256, 224);
    document.querySelector('canvas').getContext('2d').putImageData(image, 0, 0);
    return { name: game.name, frames, emulatedSeconds: (frames * 357368 - Math.floor(frames / 2) * 4) / 21477272, nonzero, audioRms: Math.sqrt(energy / count), windows, transitions,
      audioCadence: { startupFrames: 900, minSamples: minAudio, maxSamples: maxAudio, maxDriftSamples: maxAudioDrift, boundSamples: 128 }, elapsedMs: performance.now() - begin };
  }, { game, frames });
}
