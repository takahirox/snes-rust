// Optional investigation only: the game's full build/license audit is pending.
// Supply local ROMs after reviewing tests/roms/PROVENANCE.md. CI never fetches them.
import { test, expect } from '@playwright/test';
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
const cases = [
  { name: 'Gothicvania', env: 'GOTHICVANIA_ROM', sha: '11d32045c69839309490676fe2dd46e3385806f79c8dad279c2d1f4ffa6027df', sound: true, playing: 1,
    // Release-source globals; main's permanent `st` local is at $1ffb.
    // See tests/roms/PROVENANCE.md for the source/address derivation.
    state: 0x1ffb, x: 0x7e2041, y: 0x7e2045, vy: 0x7e204a, ground: 0x7f0009, groundBytes: 1,
    transitions: ['0:1', '1:3', '3:0'] },
  { name: 'Castle Platformer', env: 'CASTLE_ROM', sha: '8b325cbc3b5eee257b906f11b0a0bced2925c503293e9f242ca0492f23804df8', sound: false, playing: 0,
    state: 0x15f, x: 0x205, y: 0x208, vy: 0x220, ground: 0x222, groundBytes: 2,
    transitions: ['0:2', '2:0'] },
];
async function loadGame(page, game) {
  const buffer = readFileSync(process.env[game.env]);
  expect(createHash('sha256').update(buffer).digest('hex')).toBe(game.sha);
  // Capture the actual instance without adding a test interface to the product UI.
  await page.addInitScript(() => {
    const original = WebAssembly.instantiate;
    WebAssembly.instantiate = async (...args) => { const result = await original(...args); window.testCore = result.instance.exports; return result; };
  });
  await page.goto('/'); await expect(page.locator('#status')).toContainText('Open a local');
  await page.locator('#rom').setInputFiles({ name: 'game.sfc', mimeType: 'application/octet-stream', buffer });
  await expect(page.locator('#status')).toContainText('loaded');
}
// Runs and validates each 900-frame window as it completes. A late freeze fails
// within 15 emulated seconds even if title/early gameplay/audio are still valid.
async function replay(page, game, frames = 18100) {
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
      // APU cadence vs the fixed NTSC raster, after boot/DMA startup. Bound
      // cumulative drift as well as individual buffers: no growing A/V backlog.
      const expectedSamples = (frame + 1) * 1364 * 262 / 21477272 * 64000;
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
    return { name: game.name, frames, emulatedSeconds: frames * 1364 * 262 / 21477272, nonzero, audioRms: Math.sqrt(energy / count), windows, transitions,
      audioCadence: { startupFrames: 900, minSamples: minAudio, maxSamples: maxAudio, maxDriftSamples: maxAudioDrift, boundSamples: 128 }, elapsedMs: performance.now() - begin };
  }, { game, frames });
}
for (const game of cases) {
  test(`${game.name}: 301 seconds of recorded inputs`, async ({ page }, testInfo) => {
    test.skip(!process.env[game.env], `Supply ${game.env}; ROM is not distributed or auto-fetched`);
    test.setTimeout(180000);
    await loadGame(page, game);
    const result = await replay(page, game);
    expect(result.emulatedSeconds).toBeGreaterThan(300);
    expect(result.windows).toHaveLength(20);
    for (const transition of game.transitions) expect(result.transitions[transition], transition).toBeGreaterThan(0);
    const path = testInfo.outputPath('result.json'); writeFileSync(path, `${JSON.stringify(result, null, 2)}\n`);
    await testInfo.attach('regression-result', { path, contentType: 'application/json' });
    console.log(JSON.stringify({ name: game.name, frames: result.frames, windows: result.windows.length, transitions: result.transitions, audioCadence: result.audioCadence }));
  });
  test(`${game.name}: 301 seconds replay rejects a late gameplay freeze`, async ({ page }) => {
    test.skip(!process.env[game.env], `Supply ${game.env}; ROM is not distributed or auto-fetched`);
    test.setTimeout(60000);
    await loadGame(page, game);
    await page.evaluate((xAddress) => {
      const real = window.testCore; let frame = 0;
      // Keep audio, rendering and successful run_frame returns working while
      // freezing only the observed player X after a full valid 15-second window.
      let frozenX;
      window.testCore = { ...real,
        run_frame(buttons) {
          const ok = real.run_frame(buttons);
          if (++frame === 901) frozenX = [real.peek(xAddress), real.peek(xAddress + 1)];
          return ok;
        },
        peek(a) {
          if (frozenX && (a === xAddress || a === xAddress + 1)) return frozenX[a - xAddress];
          return real.peek(a);
        },
      };
    }, game.x);
    await expect(replay(page, game, 1800)).rejects.toThrow(/window 900-1799: controller-driven movement stopped/);
  });
}
