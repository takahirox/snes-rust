import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { selectDemo } from './cartridge.js';
const rom = readFileSync(new URL('../../demos/shionari.sfc', import.meta.url));
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.launchMetrics = { resumes: [], loads: 0, frames: 0 };
    const instantiate = WebAssembly.instantiate;
    WebAssembly.instantiate = async (...args) => {
      const result = await instantiate(...args), core = result.instance.exports;
      window.testCore = core;
      return { instance: { exports: { ...core,
        prepare_rom() { launchMetrics.loads++; return core.prepare_rom(); },
        run_frame(mask) { launchMetrics.frames++; return core.run_frame(mask); },
      } } };
    };
    const resume = AudioContext.prototype.resume;
    AudioContext.prototype.resume = function () {
      launchMetrics.resumes.push({ active: navigator.userActivation.isActive, loads: launchMetrics.loads });
      window.testAudio = this; return resume.call(this);
    };
  });
  await page.goto('/'); await expect(page.locator('#change-cartridge')).toBeEnabled();
});

test('gesture unlocks audio before delayed fetch; concurrent and repeated demo choices cannot reload', async ({ page }) => {
  let pending, requests = 0;
  await page.route('**/shionari.sfc', route => { requests++; pending = route; });
  expect(await page.evaluate(() => launchMetrics.resumes.length)).toBe(0);
  await selectDemo(page);
  await expect(page.locator('#status')).toContainText('Loading');
  for (const id of ['change-cartridge', 'demo', 'rom', 'play', 'reset']) await expect(page.locator(`#${id}`)).toBeDisabled();
  expect(await page.evaluate(() => launchMetrics.resumes)).toEqual([{ active: true, loads: 0 }]);
  await page.locator('#demo').dispatchEvent('click'); await page.locator('#play').dispatchEvent('click');
  await page.locator('#rom').setInputFiles({ name: 'duplicate.sfc', mimeType: 'application/octet-stream', buffer: rom });
  await expect.poll(() => requests).toBe(1);
  await pending.fulfill({ status: 200, contentType: 'application/octet-stream', body: rom });
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('#play')).toHaveText('Pause');
  await expect.poll(() => page.evaluate(() => launchMetrics.frames)).toBeGreaterThan(3);
  await expect.poll(() => page.evaluate(() => testAudio.state)).toBe('running');
  const frames = await page.evaluate(() => launchMetrics.frames);
  await selectDemo(page); await expect(page.locator('#status')).toHaveText('Playing');
  await expect.poll(() => page.evaluate(() => launchMetrics.frames)).toBeGreaterThan(frames);
  await page.locator('#play').click(); await expect(page.locator('#play')).toHaveText('Play');
  await selectDemo(page); await expect(page.locator('#status')).toHaveText('Paused');
  await page.locator('#reset').click(); await expect(page.locator('#status')).toContainText('SRAM preserved');
  await page.locator('#play').click(); await expect(page.locator('#status')).toHaveText('Playing');
  expect(requests).toBe(1); expect(await page.evaluate(() => launchMetrics.loads)).toBe(1);
  await expect(page.getByRole('button', { name: /Pause demo|Resume demo|Try demo/ })).toHaveCount(0);
});

for (const source of ['demo', 'local']) for (const failure of ['rejected', 'pending']) test(`${source}: audio ${failure} leaves gameplay running and Sound can recover`, async ({ page }) => {
  await page.evaluate(failure => {
    const resume = AudioContext.prototype.resume;
    window.recoverAudio = () => { AudioContext.prototype.resume = resume; };
    AudioContext.prototype.resume = failure === 'rejected'
      ? () => Promise.reject(new DOMException('Another gesture required', 'NotAllowedError'))
      : () => new Promise(() => {});
    const Original = window.AudioContext;
    window.AudioContext = class extends Original { constructor(...args) { super(...args); void this.suspend(); } };
  }, failure);
  if (source === 'demo') await selectDemo(page);
  else {
    await page.locator('#change-cartridge').click();
    const chooserPromise = page.waitForEvent('filechooser');
    await page.locator('#open-local').click();
    await (await chooserPromise).setFiles({ name: 'local.sfc', mimeType: 'application/octet-stream', buffer: rom });
  }
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('#audio-status')).toContainText('Sound');
  await expect.poll(() => page.evaluate(() => launchMetrics.frames)).toBeGreaterThan(3);
  await page.evaluate(() => recoverAudio());
  await page.locator('#sound').uncheck(); await page.locator('#sound').check();
  await expect(page.locator('#audio-status')).toBeEmpty();
  await expect.poll(() => page.evaluate(() => testAudio.state)).toBe('running');
});

test('local choice unlocks audio before the picker and delayed file read, then auto-starts', async ({ page }) => {
  await page.evaluate(() => {
    const read = File.prototype.arrayBuffer;
    File.prototype.arrayBuffer = async function () { await new Promise(resolve => { window.releaseRead = resolve; }); return read.call(this); };
  });
  await page.locator('#change-cartridge').click();
  const chooserPromise = page.waitForEvent('filechooser');
  await page.locator('#open-local').click();
  expect(await page.evaluate(() => launchMetrics.resumes)).toEqual([{ active: true, loads: 0 }]);
  await (await chooserPromise).setFiles({ name: 'local.smc', mimeType: 'application/octet-stream', buffer: rom });
  await expect(page.locator('#status')).toContainText('Loading');
  await expect(page.locator('#current-game')).toHaveText('None');
  await page.evaluate(() => releaseRead());
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('#current-game')).toHaveText('local.smc');
  await expect.poll(() => page.evaluate(() => launchMetrics.frames)).toBeGreaterThan(3);
});

test('hidden page finishes loading safely and waits for intentional playback', async ({ page }) => {
  let pending; await page.route('**/shionari.sfc', route => { pending = route; });
  await selectDemo(page); await expect.poll(() => Boolean(pending)).toBe(true);
  await page.evaluate(() => { Object.defineProperty(document, 'hidden', { configurable: true, value: true }); document.dispatchEvent(new Event('visibilitychange')); });
  await pending.fulfill({ status: 200, body: rom });
  await expect(page.locator('#play')).toBeEnabled(); await expect(page.locator('#play')).toHaveText('Play');
  expect(await page.evaluate(() => launchMetrics.frames)).toBe(0);
  await page.evaluate(() => { delete document.hidden; }); await page.locator('#play').click();
  await expect(page.locator('#status')).toHaveText('Playing');
});

test('fullscreen entry and exit preserve playback and SRAM', async ({ page }) => {
  await selectDemo(page); await expect(page.locator('#status')).toHaveText('Playing');
  await page.evaluate(() => { new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3] = 0x79; });
  await page.locator('#fullscreen').click(); await expect.poll(() => page.evaluate(() => Boolean(document.fullscreenElement))).toBe(true);
  await page.locator('#fullscreen').click(); await expect.poll(() => page.evaluate(() => Boolean(document.fullscreenElement))).toBe(false);
  await expect(page.locator('#status')).toHaveText('Playing');
  expect(await page.evaluate(() => launchMetrics.loads)).toBe(1);
  expect(await page.evaluate(() => new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3])).toBe(0x79);
});
