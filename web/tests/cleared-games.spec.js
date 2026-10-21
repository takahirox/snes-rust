import { test, expect } from '@playwright/test';
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { replay } from './game-replay.js';

const manifest = JSON.parse(readFileSync(new URL('../../tests/roms/cleared-games.json', import.meta.url)));
async function load(page, name) {
  const buffer = readFileSync(new URL(`../../.cache/cleared-games/${name}/${name}.sfc`, import.meta.url));
  expect(createHash('sha256').update(buffer).digest('hex')).toBe(manifest[name].romSha256);
  await page.addInitScript(() => {
    const original = WebAssembly.instantiate;
    WebAssembly.instantiate = async (...args) => { const result = await original(...args); window.testCore = result.instance.exports; return result; };
    const start = AudioBufferSourceNode.prototype.start;
    AudioBufferSourceNode.prototype.start = function (...args) {
      const samples = this.buffer?.getChannelData(0);
      if (samples?.some((v) => v !== 0)) window.gameAudioBuffers = (window.gameAudioBuffers ?? 0) + 1;
      return start.apply(this, args);
    };
  });
  await page.goto('/'); await expect(page.locator('#status')).toContainText('Open a local');
  await page.locator('#rom').setInputFiles({ name: `${name}.sfc`, mimeType: 'application/octet-stream', buffer });
  await expect(page.locator('#status')).toContainText('loaded');
}
async function evidence(testInfo, result) {
  const path = testInfo.outputPath('result.json');
  writeFileSync(path, `${JSON.stringify(result, null, 2)}\n`);
  await testInfo.attach('cleared-game-result', { path, contentType: 'application/json' });
}
test('cleared Castle Platformer: five-minute gameplay replay', async ({ page }, testInfo) => {
  test.setTimeout(180000);
  await load(page, 'castle');
  // Player layout comes from entity.h; state comes from this build's ld65 labels.
  const result = await replay(page, { name: 'Castle Platformer (modified)', playing: 0, sound: false,
    state: 0x1421, x: 0x205, y: 0x208, vy: 0x220, ground: 0x222, groundBytes: 2 });
  expect(result.windows).toHaveLength(20);
  expect(result.transitions['0:2']).toBeGreaterThan(0);
  expect(result.transitions['2:0']).toBeGreaterThan(0);
  await evidence(testInfo, result);
});
test('cleared Memory Game: five-minute playthroughs using controller inputs', async ({ page }, testInfo) => {
  test.setTimeout(180000);
  await load(page, 'memory');
  const result = await page.evaluate(async () => {
    const core = window.testCore;
    const word = (a) => core.peek(a) | core.peek(a + 1) << 8;
    const require = (ok, message, frame) => { if (!ok) throw new Error(`Memory Game frame ${frame}: ${message}`); };
    const hash = () => {
      const data = new Uint8Array(core.memory.buffer, core.framebuffer(), 256 * 224 * 4);
      let h = 2166136261; for (const b of data) h = Math.imul(h ^ b, 16777619) >>> 0;
      return h >>> 0;
    };
    // Symbols from the pinned build's labels.txt and GameState in gameloop.s.
    const stateAddress = 0x7e2c1e, cursorAddress = 0x7e2c20, guessesAddress = 0x7e2c28;
    let previousState = 2, previousCursor = 0, titleFrames = 0, wins = 0;
    let movements = 0, selections = 0, active = 0, hashes = new Set();
    let nonzero = 0, energy = 0, sampleCount = 0;
    const windows = [], transitions = {};
    for (let frame = 0; frame < 18100; frame++) {
      const state = word(stateAddress), cursor = word(cursorAddress);
      titleFrames = state === 2 ? titleFrames + 1 : 0;
      let buttons = 0;
      if (frame % 2 === 0) {
        if (state === 2 && titleFrames > 30) buttons = 0x1000;
        if (state === 6) buttons = 0x8000; // Skip the five-second memorization display.
        if (state === 10 || state === 12) {
          // Read the game's shuffled grid to choose a matching card, then move
          // and select solely with emulated joypad presses. Never write RAM.
          const opened = (i) => core.peek(0x7e2c00 + i);
          const card = (i) => word(0x102 + i * 2);
          const first = word(0x7e2c22);
          const target = Array.from({ length: 30 }, (_, i) => i)
            .find((i) => !opened(i) && (state === 10 || card(i) === card(first)));
          require(target !== undefined, 'no selectable matching card', frame);
          if (Math.floor(cursor / 6) !== Math.floor(target / 6)) buttons = cursor < target ? 0x400 : 0x800;
          else if (cursor !== target) buttons = cursor < target ? 0x100 : 0x200;
          else buttons = 0x8000;
        }
      }
      require(core.run_frame(buttons), 'core execution failed', frame);
      const audio = new Float32Array(core.memory.buffer, core.audio_buffer(), core.audio_len());
      for (const v of audio) {
        require(Number.isFinite(v) && Math.abs(v) <= 1, 'invalid audio sample', frame);
        if (v !== 0) nonzero++;
        energy += v * v;
      }
      sampleCount += audio.length;
      const nextState = word(stateAddress), nextCursor = word(cursorAddress);
      if (nextState !== previousState) {
        const key = `${previousState}:${nextState}`;
        transitions[key] = (transitions[key] ?? 0) + 1;
        if (nextState === 12 || nextState === 14) selections++;
        if (nextState === 2 && core.peek(guessesAddress) === 15) wins++;
      }
      if (buttons & 0xf00 && nextCursor !== previousCursor) movements++;
      if (nextState >= 10) active++;
      previousState = nextState; previousCursor = nextCursor;
      if (frame % 6 === 0) hashes.add(hash());
      if (frame % 900 === 899) {
        require(movements >= 20 && selections >= 2 && active >= 100 && hashes.size >= 10,
          `gameplay stalled: movement=${movements}, selections=${selections}, active=${active}, hashes=${hashes.size}`, frame);
        const audioRms = Math.sqrt(energy / sampleCount);
        require(nonzero > 10000 && audioRms > .001, 'game sound effects went silent', frame);
        windows.push({ end: frame, movements, selections, active, hashes: hashes.size, nonzero, audioRms });
        movements = 0; selections = 0; active = 0; hashes = new Set();
        nonzero = 0; energy = 0; sampleCount = 0;
      }
      if (frame % 120 === 0) await new Promise((resolve) => setTimeout(resolve, 0));
    }
    return { frames: 18100, emulatedSeconds: 18100 * 1364 * 262 / 21477272, wins, windows, transitions };
  });
  expect(result.wins).toBeGreaterThan(1);
  expect(result.windows).toHaveLength(20);
  await evidence(testInfo, result);
});

test('cleared Memory Game plays finite card sound effects through gesture-activated Web Audio', async ({ page }) => {
  await load(page, 'memory');
  const state = () => page.evaluate(() => testCore.peek(0x7e2c1e));
  const audible = () => page.evaluate(() => window.gameAudioBuffers ?? 0);
  await page.locator('#play').click();
  await expect.poll(state).toBe(2);
  await page.waitForTimeout(300);
  expect(await audible()).toBe(0); // No boot/title tone posing as gameplay sound.
  await page.keyboard.press('Enter', { delay: 80 });
  await expect.poll(state).toBe(6);
  await page.keyboard.press('KeyZ', { delay: 80 });
  await expect.poll(state).toBe(10);
  expect(await audible()).toBe(0);
  await page.keyboard.press('KeyZ', { delay: 80 });
  await expect.poll(state).toBe(12);
  await expect.poll(audible).toBeGreaterThan(0);
  // The effect must end while the player remains idle on the same card.
  await page.waitForTimeout(400);
  const afterEffect = await audible();
  await page.waitForTimeout(250);
  expect(await audible()).toBe(afterEffect);
  await page.locator('#play').click();
  await expect(page.locator('#status')).toHaveText('Paused');
  const pathCount = await audible();
  await page.waitForTimeout(100);
  expect(await audible()).toBe(pathCount);
  await page.locator('#reset').click();
  await page.locator('#play').click();
  await expect.poll(state).toBe(2);
  await page.waitForTimeout(300);
  expect(await audible()).toBe(pathCount);
  await page.locator('#play').click();
  await expect(page.locator('#error')).toBeEmpty();
});
