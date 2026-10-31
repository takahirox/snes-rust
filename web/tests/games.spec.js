// Optional investigation only: the game's full build/license audit is pending.
// Supply local ROMs after reviewing tests/roms/PROVENANCE.md. CI never fetches them.
import { resetAndPause } from './playback.js';
import { test, expect } from '@playwright/test';
import { replay } from './game-replay.js';
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
  await page.goto('/'); await expect(page.locator('#status')).toHaveText('Ready');
  await page.locator('#rom').setInputFiles({ name: 'game.sfc', mimeType: 'application/octet-stream', buffer });
  await expect(page.locator('#status')).toHaveText('Playing');
  await resetAndPause(page); // Fresh frame origin for hardware/replay assertions.
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
