import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
const rom = readFileSync(new URL('../../demos/shionari.sfc', import.meta.url));
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.launchMetrics = { resumes: [], loads: 0, frames: 0 };
    const instantiate = WebAssembly.instantiate;
    WebAssembly.instantiate = async (...args) => {
      const result = await instantiate(...args), core = result.instance.exports;
      window.testCore = core;
      return { instance: { exports: { ...core,
        load_rom() { launchMetrics.loads++; return core.load_rom(); },
        run_frame(mask) { launchMetrics.frames++; return core.run_frame(mask); },
      } } };
    };
    const resume = AudioContext.prototype.resume;
    AudioContext.prototype.resume = function () {
      launchMetrics.resumes.push({ active: navigator.userActivation.isActive, loads: launchMetrics.loads });
      window.testAudio = this; return resume.call(this);
    };
  });
  await page.goto('/'); await expect(page.locator('#demo')).toBeEnabled();
});

test('one activation unlocks audio before a delayed fetch; duplicate activations cannot double-load', async ({ page }) => {
  let pending, requests = 0;
  await page.route('**/shionari.sfc', route => { requests++; pending = route; });
  expect(await page.evaluate(() => launchMetrics.resumes.length)).toBe(0);
  await expect(page.locator('#demo')).toHaveText('Play demo');
  await page.locator('#demo').click();
  await expect(page.locator('#status')).toContainText('Loading'); await expect(page.locator('#demo')).toBeDisabled();
  expect(await page.evaluate(() => launchMetrics.resumes)).toEqual([{ active: true, loads: 0 }]);
  await page.locator('#demo').dispatchEvent('click'); await page.locator('#play').dispatchEvent('click');
  await expect.poll(() => requests).toBe(1);
  await pending.fulfill({ status: 200, contentType: 'application/octet-stream', body: rom });
  await expect(page.locator('#status')).toHaveText('Playing'); await expect(page.locator('#demo')).toHaveText('Pause demo');
  await expect.poll(() => page.evaluate(() => launchMetrics.frames)).toBeGreaterThan(3);
  await expect.poll(() => page.evaluate(() => testAudio.state)).toBe('running');
  for (let i = 0; i < 3; i++) {
    await page.locator('#demo').click(); await expect(page.locator('#status')).toHaveText('Paused');
    await expect(page.locator('#demo')).toHaveText('Resume demo');
    await page.locator('#demo').click(); await expect(page.locator('#status')).toHaveText('Playing');
  }
  await page.locator('#reset').click(); await expect(page.locator('#status')).toContainText('SRAM preserved');
  await page.locator('#demo').click(); await expect(page.locator('#status')).toHaveText('Playing');
  expect(requests).toBe(1); expect(await page.evaluate(() => launchMetrics.loads)).toBe(1);
});

for (const failure of ['rejected', 'pending']) test(`audio ${failure} does not block gameplay and Sound can recover`, async ({ page }) => {
  await page.evaluate(failure => {
    const resume = AudioContext.prototype.resume;
    window.recoverAudio = () => { AudioContext.prototype.resume = resume; };
    AudioContext.prototype.resume = failure === 'rejected'
      ? () => Promise.reject(new DOMException('Another gesture required', 'NotAllowedError'))
      : () => new Promise(() => {});
    // Keep a real suspended context so this also covers a pending unlock.
    const Original = window.AudioContext;
    window.AudioContext = class extends Original { constructor(...args) { super(...args); void this.suspend(); } };
  }, failure);
  await page.locator('#demo').click(); await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('#audio-status')).toContainText('Sound');
  await expect.poll(() => page.evaluate(() => launchMetrics.frames)).toBeGreaterThan(3);
  await page.evaluate(() => recoverAudio());
  await page.locator('#sound').uncheck(); await page.locator('#sound').check();
  await expect(page.locator('#audio-status')).toBeEmpty();
  await expect.poll(() => page.evaluate(() => testAudio.state)).toBe('running');
});

test('a hidden page finishes loading safely and waits for an intentional resume', async ({ page }) => {
  let pending; await page.route('**/shionari.sfc', route => { pending = route; });
  await page.locator('#demo').click(); await expect.poll(() => Boolean(pending)).toBe(true);
  await page.evaluate(() => { Object.defineProperty(document, 'hidden', { configurable: true, value: true }); document.dispatchEvent(new Event('visibilitychange')); });
  await pending.fulfill({ status: 200, body: rom });
  await expect(page.locator('#demo')).toHaveText('Resume demo');
  expect(await page.evaluate(() => launchMetrics.frames)).toBe(0);
  await page.evaluate(() => { delete document.hidden; }); await page.locator('#demo').click();
  await expect(page.locator('#status')).toHaveText('Playing');
});

test('fullscreen entry and exit preserve playback and SRAM', async ({ page }) => {
  await page.locator('#demo').click(); await expect(page.locator('#status')).toHaveText('Playing');
  await page.evaluate(() => { new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3] = 0x79; });
  await page.locator('#fullscreen').click(); await expect.poll(() => page.evaluate(() => Boolean(document.fullscreenElement))).toBe(true);
  await page.locator('#fullscreen').click(); await expect.poll(() => page.evaluate(() => Boolean(document.fullscreenElement))).toBe(false);
  await expect(page.locator('#status')).toHaveText('Playing');
  expect(await page.evaluate(() => launchMetrics.loads)).toBe(1);
  expect(await page.evaluate(() => new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3])).toBe(0x79);
});

test('a fast double click starts the demo once and leaves it playing', async ({ page }) => {
  await page.locator('#demo').dblclick();
  await expect(page.locator('#status')).toHaveText('Playing');
  expect(await page.evaluate(() => launchMetrics.loads)).toBe(1);
});
