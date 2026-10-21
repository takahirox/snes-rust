// Optional investigation only: the game's full build/license audit is pending.
// Supply local ROMs after reviewing tests/roms/PROVENANCE.md. CI never fetches them.
import { test, expect } from '@playwright/test';
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
const cases = [
  { name: 'Gothicvania', env: 'GOTHICVANIA_ROM', sha: '11d32045c69839309490676fe2dd46e3385806f79c8dad279c2d1f4ffa6027df', sound: true, golden: ['6907ed94', '38a68c59', 'f8778ca6', '2243b3f5'] },
  { name: 'Castle Platformer', env: 'CASTLE_ROM', sha: '8b325cbc3b5eee257b906f11b0a0bced2925c503293e9f242ca0492f23804df8', sound: false, golden: ['ae4abff8', '0e7ff935', 'd889be93', '5966827c'] },
];
for (const game of cases) test(`${game.name}: 301 seconds of recorded inputs`, async ({ page }, testInfo) => {
  test.skip(!process.env[game.env], `Supply ${game.env}; ROM is not distributed or auto-fetched`);
  test.setTimeout(180000);
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
  const result = await page.evaluate(async (name) => {
    const core = window.testCore; const decoder = new TextDecoder();
    const hash = () => { const bytes = new Uint8Array(core.memory.buffer, core.framebuffer(), 256 * 224 * 4); let h = 2166136261; for (const b of bytes) h = Math.imul(h ^ b, 16777619) >>> 0; return h.toString(16).padStart(8, '0'); };
    let nonzero = 0, energy = 0, count = 0; const hashes = new Set(); const observations = [];
    const begin = performance.now();
    for (let frame = 0; frame < 18100; frame++) {
      const cycle = frame % 900;
      // Start/restart, run, jump, attack. Castle starts directly in gameplay.
      let buttons = cycle >= 180 && cycle < 182 ? 0x1000 : 0;
      if (cycle >= 240 && cycle < 600) buttons |= 0x100;
      if ((cycle >= 340 && cycle < 360) || (cycle >= 520 && cycle < 540)) buttons |= 0x8000;
      if (cycle >= 400 && cycle < 410) buttons |= 0x4000;
      if (!core.run_frame(buttons)) throw new Error(decoder.decode(new Uint8Array(core.memory.buffer, core.error_buffer(), core.error_len())));
      const audio = new Float32Array(core.memory.buffer, core.audio_buffer(), core.audio_len());
      for (const sample of audio) { if (!Number.isFinite(sample)) throw new Error('Non-finite audio'); if (sample) nonzero++; energy += sample * sample; count++; }
      if (frame < 900 || frame % 60 === 0) hashes.add(hash());
      if ([179, 299, 359, 599, 899, 18099].includes(frame)) observations.push({ frame, hash: hash() });
      if (frame % 120 === 0) await new Promise((resolve) => setTimeout(resolve, 0));
    }
    // Only the last frame is copied to canvas; emulation/rendering/audio ran for every frame.
    const image = new ImageData(new Uint8ClampedArray(core.memory.buffer, core.framebuffer(), 256 * 224 * 4).slice(), 256, 224);
    document.querySelector('canvas').getContext('2d').putImageData(image, 0, 0);
    return { name, frames: 18100, emulatedSeconds: 18100 / 60.0988, nonzero, audioRms: Math.sqrt(energy / count), distinctHashes: hashes.size, observations, elapsedMs: performance.now() - begin };
  }, game.name);
  expect(result.frames).toBe(18100); expect(result.emulatedSeconds).toBeGreaterThan(300);
  expect(result.distinctHashes).toBeGreaterThan(100);
  expect(result.observations[1].hash).not.toBe(result.observations[0].hash);
  // Reviewed title/play/jump/death snapshots for these exact release images.
  expect([0, 1, 2, 4].map((i) => result.observations[i].hash)).toEqual(game.golden);
  if (game.sound) { expect(result.nonzero).toBeGreaterThan(10000); expect(result.audioRms).toBeGreaterThan(.001); }
  const path = testInfo.outputPath('result.json'); writeFileSync(path, `${JSON.stringify(result, null, 2)}\n`);
  await testInfo.attach('regression-result', { path, contentType: 'application/json' });
  console.log(JSON.stringify(result));
});
