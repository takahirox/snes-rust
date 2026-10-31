import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';

const buffer = readFileSync(new URL('../../tests/roms/smoke.bin', import.meta.url));
const sram = page => page.evaluate(() => new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3]);
const frame = page => page.evaluate(() => testCore.peek(0x10));
const geometry = page => page.locator('.screen, canvas, .game-entry, #change-cartridge, .toolbar').evaluateAll(nodes => nodes.map(n => {
  const { x, y, width, height } = n.getBoundingClientRect(); return { x, y, width, height };
}));

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const active = new Set();
    window.resetMetrics = { calls: 0, frames: 0, resumes: [], snapshots: [], maxSources: 0, maxLead: 0 };
    const instantiate = WebAssembly.instantiate;
    WebAssembly.instantiate = async (...args) => {
      const result = await instantiate(...args), core = window.testCore = result.instance.exports;
      return { instance: { exports: { ...core,
        reset() {
          resetMetrics.calls++;
          if (window.failReset === 'throw') throw new Error('Injected reset failure');
          if (window.failReset === 'return') return 0;
          const ok = core.reset();
          resetMetrics.snapshots.push({ frame: core.peek(0x10), audio: core.audio_len(), active: active.size });
          return ok;
        },
        run_frame(mask) { resetMetrics.frames++; return core.run_frame(mask); },
      } } };
    };
    const resume = AudioContext.prototype.resume;
    AudioContext.prototype.resume = function () {
      resetMetrics.resumes.push(navigator.userActivation.isActive);
      return resume.call(this);
    };
    const start = AudioBufferSourceNode.prototype.start, stop = AudioBufferSourceNode.prototype.stop;
    AudioBufferSourceNode.prototype.start = function (...args) {
      active.add(this); this.addEventListener('ended', () => active.delete(this));
      resetMetrics.maxSources = Math.max(resetMetrics.maxSources, active.size);
      resetMetrics.maxLead = Math.max(resetMetrics.maxLead, args[0] + this.buffer.duration - this.context.currentTime);
      return start.apply(this, args);
    };
    AudioBufferSourceNode.prototype.stop = function (...args) { active.delete(this); return stop.apply(this, args); };
  });
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
  page.on('requestfailed', r => errors.push(`${r.url()}: ${r.failure()?.errorText}`));
  page.on('response', r => { if (r.status() >= 400) errors.push(`${r.status()} ${r.url()}`); });
  page.testErrors = errors;
  await page.goto('/'); await expect(page.locator('#status')).toHaveText('Ready');
});
test.afterEach(async ({ page }) => { expect(page.testErrors).toEqual([]); });

async function load(page, rom = buffer) {
  await page.locator('#rom').setInputFiles({ name: 'local.sfc', mimeType: 'application/octet-stream', buffer: rom });
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect.poll(() => frame(page)).toBeGreaterThan(3);
}

for (const viewport of [{ width: 1280, height: 900 }, { width: 320, height: 740 }, { width: 667, height: 320 }]) {
  test.describe(`${viewport.width}×${viewport.height}`, () => {
    test.use({ viewport, hasTouch: viewport.width < 1000, isMobile: viewport.width < 1000 });
    test('Reset from playing and paused restarts real Wasm, clears held input and preserves the slot and SRAM', async ({ page }) => {
      await expect(page.locator('#change-cartridge')).toHaveAccessibleName('Insert cartridge');
      await expect(page.locator('#placeholder')).toHaveText('Insert a cartridge');
      await expect(page.locator('#current-game')).toHaveText('None');
      const initial = await geometry(page);
      await load(page);
      await expect(page.locator('#placeholder')).toBeHidden();
      for (const paused of [false, true]) {
        await page.evaluate(() => { new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3] = 0x79; });
        await page.keyboard.down('ArrowRight');
        await expect.poll(() => page.evaluate(() => testCore.peek(0x11))).toBe(1);
        if (paused) await page.locator('#play').click();
        const frames = await page.evaluate(() => resetMetrics.frames);
        await page.locator('#reset').click();
        await expect(page.locator('#status')).toHaveText('Playing');
        await expect(page.getByRole('button', { name: 'Pause', exact: true })).toHaveCount(1);
        await expect(page.locator('canvas')).toBeFocused();
        await expect.poll(() => page.evaluate(() => resetMetrics.frames)).toBeGreaterThan(frames + 3);
        await expect.poll(() => page.evaluate(() => testCore.peek(0x11))).toBe(0);
        await page.keyboard.up('ArrowRight');
        expect(await sram(page)).toBe(0x79);
        expect(await geometry(page)).toEqual(initial);
        await expect(page.locator('#change-cartridge')).toHaveText('Insert cartridge');
        await expect(page.locator('#current-game')).toHaveText('local.sfc');
        await page.keyboard.down('ArrowRight');
        await expect.poll(() => page.evaluate(() => testCore.peek(0x11))).toBe(1);
        await page.keyboard.up('ArrowRight');
      }
      const metrics = await page.evaluate(() => resetMetrics);
      expect(metrics.calls).toBe(2);
      expect(metrics.snapshots).toEqual(Array(2).fill({ frame: 0, audio: 0, active: 0 }));
      expect(metrics.resumes.slice(-2)).toEqual([true, true]);
      expect(metrics.maxSources).toBeGreaterThan(0); expect(metrics.maxSources).toBeLessThanOrEqual(12);
      expect(metrics.maxLead).toBeLessThanOrEqual(.151);
      await expect(page.locator('#error')).toBeEmpty();
    });
  });
}

for (const failure of ['return', 'throw']) test(`failed core reset (${failure}) reports an error and leaves playback paused`, async ({ page }) => {
  await load(page);
  await page.evaluate(failure => { window.failReset = failure; }, failure);
  await page.locator('#reset').click();
  await expect(page.locator('#error')).toContainText(failure === 'throw' ? 'Injected reset failure' : 'Emulator operation failed');
  await expect(page.locator('#status')).toHaveText('Paused');
  await expect(page.locator('#play')).toHaveText('Play');
  const frames = await page.evaluate(() => resetMetrics.frames);
  await page.waitForTimeout(100);
  expect(await page.evaluate(() => resetMetrics.frames)).toBe(frames);
  await page.evaluate(() => { window.failReset = false; });
  await page.locator('#reset').click(); await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('#error')).toBeEmpty();
});

test('Reset guards absent, loading and unresolved save restoration without overwriting recoverable bytes', async ({ page }) => {
  await expect(page.locator('#reset')).toBeDisabled();
  await page.locator('#reset').dispatchEvent('click');
  expect(await page.evaluate(() => resetMetrics.calls)).toBe(0);
  await load(page);
  await page.evaluate(() => {
    new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3] = 0x79;
    const digest = crypto.subtle.digest.bind(crypto.subtle);
    crypto.subtle.digest = async (...args) => { await new Promise(resolve => { window.releaseHash = resolve; }); return digest(...args); };
  });
  const other = Buffer.from(buffer); other[0x7fc0] ^= 1;
  await page.locator('#rom').setInputFiles({ name: 'other.sfc', mimeType: 'application/octet-stream', buffer: other });
  await expect.poll(() => page.evaluate(() => typeof releaseHash)).toBe('function');
  await expect(page.locator('#reset')).toBeDisabled();
  await page.locator('#reset').dispatchEvent('click');
  await page.evaluate(() => {
    window.savedBytes = Object.entries(localStorage);
    window.getSave = Storage.prototype.getItem;
    Storage.prototype.getItem = () => { throw new Error('Read unavailable'); };
    releaseHash();
  });
  await expect(page.locator('#error')).toContainText('Save could not be restored');
  await expect(page.locator('#reset')).toBeDisabled();
  await page.locator('#reset').dispatchEvent('click');
  await page.evaluate(() => window.dispatchEvent(new Event('pagehide')));
  expect(await page.evaluate(() => Object.entries(localStorage))).toEqual(await page.evaluate(() => savedBytes));
  expect(await sram(page)).toBe(0x79);
  expect(await page.evaluate(() => resetMetrics.calls)).toBe(0);
  await page.evaluate(() => { Storage.prototype.getItem = getSave; });
  await page.locator('#retry-save').click(); await expect(page.locator('#status')).toHaveText('Playing');
});

test('Reset in a hidden document does not advance frames or auto-play on visibility return', async ({ page }) => {
  await load(page);
  await page.evaluate(() => {
    Object.defineProperty(document, 'hidden', { configurable: true, value: true });
    document.dispatchEvent(new Event('visibilitychange'));
  });
  const frames = await page.evaluate(() => resetMetrics.frames);
  await page.locator('#reset').dispatchEvent('click');
  await page.waitForTimeout(100);
  expect(await page.evaluate(() => resetMetrics.frames)).toBe(frames);
  await expect(page.locator('#play')).toHaveText('Play');
  await expect(page.locator('#status')).toHaveText('Paused');
  await page.evaluate(() => { delete document.hidden; document.dispatchEvent(new Event('visibilitychange')); });
  await page.waitForTimeout(100);
  expect(await page.evaluate(() => resetMetrics.frames)).toBe(frames);
  await page.locator('#reset').click(); await expect(page.locator('#status')).toHaveText('Playing');
});

for (const failure of ['rejected', 'pending', 'unavailable']) test(`Reset retries ${failure} audio in the gesture and starts video without waiting`, async ({ page }) => {
  await page.locator('#sound').uncheck(); await load(page);
  await page.locator('#play').click();
  await page.evaluate(failure => {
    if (failure === 'unavailable') window.AudioContext = class { constructor() { throw new Error('Audio device unavailable'); } };
    else {
      const Original = window.AudioContext;
      window.AudioContext = class extends Original {
        constructor(...args) { super(...args); void this.suspend(); }
        resume() {
          resetMetrics.resumes.push(navigator.userActivation.isActive);
          return failure === 'pending' ? new Promise(() => {}) : Promise.reject(new Error('Sound blocked'));
        }
      };
    }
  }, failure);
  await page.locator('#sound').check();
  const frames = await page.evaluate(() => resetMetrics.frames);
  await page.locator('#reset').click();
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('#play')).toHaveText('Pause');
  await expect(page.locator('#audio-status')).toContainText('Sound');
  await expect.poll(() => page.evaluate(() => resetMetrics.frames)).toBeGreaterThan(frames + 3);
  if (failure !== 'unavailable') expect(await page.evaluate(() => resetMetrics.resumes.at(-1))).toBe(true);
  await page.keyboard.down('ArrowRight');
  await expect.poll(() => page.evaluate(() => testCore.peek(0x11))).toBe(1);
  await page.keyboard.up('ArrowRight');
  await expect(page.locator('#error')).toBeEmpty();
});
