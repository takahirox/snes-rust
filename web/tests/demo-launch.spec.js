import { test, expect } from '@playwright/test';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { delayLocalFileRead, selectDemo } from './cartridge.js';
import { setTabHidden, waitForAnimationFrames } from './playback.js';
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
  await expect(page.locator('#video')).toBeFocused();
  await expect(page.locator('#play')).toHaveText('Pause');
  await expect.poll(() => page.evaluate(() => launchMetrics.frames)).toBeGreaterThan(3);
  await expect.poll(() => page.evaluate(() => testAudio.state)).toBe('running');
  const frames = await page.evaluate(() => launchMetrics.frames);
  await selectDemo(page); await expect(page.locator('#status')).toHaveText('Playing');
  await expect.poll(() => page.evaluate(() => launchMetrics.frames)).toBeGreaterThan(frames);
  await page.locator('#play').click(); await expect(page.locator('#play')).toHaveText('Play');
  await selectDemo(page); await expect(page.locator('#status')).toHaveText('Paused');
  await page.locator('#reset').click(); await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('#play')).toHaveText('Pause');
  expect(requests).toBe(1); expect(await page.evaluate(() => launchMetrics.loads)).toBe(1);
  await expect(page.getByRole('button', { name: /Pause demo|Resume demo|Try demo/ })).toHaveCount(0);
});

for (const source of ['demo', 'local']) for (const failure of ['rejected', 'pending']) test(`${source}: audio ${failure} leaves gameplay running and audio controls can recover`, async ({ page }) => {
  const narrowLocalLaunch = source === 'local' && failure === 'rejected';
  if (narrowLocalLaunch) await page.setViewportSize({ width: 375, height: 667 });
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
  const audioStatus = page.locator('#audio-status');
  await expect(audioStatus).toContainText('Sound');
  await expect(audioStatus).toBeVisible();
  await expect(audioStatus).toContainText('Adjust volume or unmute to retry.');
  if (narrowLocalLaunch) {
    await expect(audioStatus).toBeInViewport({ ratio: 1 });
    expect(await page.evaluate(() => scrollY)).toBe(0);
  }
  await expect.poll(() => page.evaluate(() => launchMetrics.frames)).toBeGreaterThan(3);
  await setTabHidden(page, true);
  await expect(page.locator('#status')).toHaveText('Suspended');
  const before = await page.evaluate(() => launchMetrics.frames);
  await setTabHidden(page, false);
  await expect.poll(() => page.evaluate(() => launchMetrics.frames)).toBeGreaterThan(before + 3);
  await expect(audioStatus).toContainText('Adjust volume or unmute to retry.');
  await page.evaluate(() => recoverAudio());
  if (source === 'demo') { await page.locator('#mute').click(); await page.locator('#mute').click(); }
  else { await page.locator('#volume').focus(); await page.keyboard.press('ArrowLeft'); }
  await expect(audioStatus).toBeEmpty();
  await expect.poll(() => page.evaluate(() => testAudio.state)).toBe('running');
});

test('local choice unlocks audio before the picker and delayed file read, then starts on visibility return', async ({ page }) => {
  const releaseRead = await delayLocalFileRead(page);
  await page.locator('#change-cartridge').click();
  const chooserPromise = page.waitForEvent('filechooser');
  await page.locator('#open-local').click();
  expect(await page.evaluate(() => launchMetrics.resumes)).toEqual([{ active: true, loads: 0 }]);
  await (await chooserPromise).setFiles({ name: 'local.smc', mimeType: 'application/octet-stream', buffer: rom });
  await expect(page.locator('#status')).toContainText('Loading');
  await expect(page.locator('#current-game')).toHaveText('None');
  await setTabHidden(page, true);
  await releaseRead();
  await expect(page.locator('#status')).toHaveText('Suspended');
  await expect(page.locator('#current-game')).toHaveText('local.smc');
  await waitForAnimationFrames(page);
  expect(await page.evaluate(() => launchMetrics.frames)).toBe(0);
  await setTabHidden(page, false);
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect.poll(() => page.evaluate(() => launchMetrics.frames)).toBeGreaterThan(3);
});

for (const source of ['demo', 'local']) {
  test(`${source} completing after visibility returns retries audio and preserves focus on Display`, async ({ page }) => {
    let releaseLoad;
    if (source === 'demo') {
      await page.route('**/shionari.sfc', route => {
        releaseLoad = () => route.fulfill({ status: 200, body: rom });
      });
      await selectDemo(page);
      await expect.poll(() => Boolean(releaseLoad)).toBe(true);
    } else {
      releaseLoad = await delayLocalFileRead(page);
      await page.locator('#change-cartridge').click();
      const chooser = page.waitForEvent('filechooser');
      await page.locator('#open-local').click();
      await (await chooser).setFiles({ name: 'local.sfc', mimeType: 'application/octet-stream', buffer: rom });
    }
    await expect(page.locator('#status')).toContainText('Loading');
    const display = page.getByRole('combobox', { name: 'Display', exact: true });
    await display.focus();
    await setTabHidden(page, true);
    const delayAudio = source === 'local';
    await page.evaluate(async delayAudio => {
      const context = testAudio;
      await context.suspend();
      const start = AudioBufferSourceNode.prototype.start;
      window.freshAudioBuffers = 0;
      AudioBufferSourceNode.prototype.start = function (...args) {
        start.apply(this, args);
        if (this.context === context && this.buffer?.getChannelData(0).some(sample => sample !== 0)) freshAudioBuffers++;
      };
      if (delayAudio) {
        const resume = AudioContext.prototype.resume;
        AudioContext.prototype.resume = function () {
          return new Promise(resolve => { window.releaseAudio = () => resolve(resume.call(this)); });
        };
      }
    }, delayAudio);
    await setTabHidden(page, false);
    await expect(display).toBeFocused();
    expect(await page.evaluate(() => launchMetrics.frames)).toBe(0);
    expect(await page.evaluate(() => testAudio.state)).toBe('suspended');

    await releaseLoad();
    await expect(page.locator('#status')).toHaveText('Playing');
    await expect.poll(() => page.evaluate(() => launchMetrics.frames)).toBeGreaterThan(3);
    const audioStatus = page.locator('#audio-status');
    if (delayAudio) {
      await expect(audioStatus).toContainText('Sound blocked. Adjust volume or unmute to retry.');
      expect(await page.evaluate(() => freshAudioBuffers)).toBe(0);
      await page.evaluate(() => releaseAudio());
    }
    await expect.poll(() => page.evaluate(() => testAudio.state)).toBe('running');
    await expect.poll(() => page.evaluate(() => freshAudioBuffers)).toBeGreaterThan(0);
    await expect(audioStatus).toBeEmpty();
    await expect(display).toBeFocused();
  });
}

test('a demo selected before hiding restores SRAM and starts without another Play click', async ({ page }) => {
  const saveKey = `snes-rust:sram:${createHash('sha256').update(rom).digest('hex')}`;
  const save = Buffer.alloc(8192);
  save[3] = 0x79;
  await page.evaluate(([key, data]) => localStorage.setItem(key, data), [saveKey, save.toString('base64')]);
  let pending; await page.route('**/shionari.sfc', route => { pending = route; });
  await selectDemo(page); await expect.poll(() => Boolean(pending)).toBe(true);
  await setTabHidden(page, true);
  await pending.fulfill({ status: 200, body: rom });
  await expect(page.locator('#status')).toHaveText('Suspended');
  await expect(page.locator('#play')).toBeEnabled();
  await expect(page.locator('#play')).toHaveText('Pause');
  await expect.poll(() => page.evaluate(() => testAudio.state)).toBe('running');
  const restored = await page.evaluate(() => {
    const sram = new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len());
    const restored = sram[3];
    sram[3] = 0x48;
    window.dispatchEvent(new Event('pagehide'));
    return restored;
  });
  expect(restored).toBe(0x79);
  await expect.poll(() => page.evaluate(key => atob(localStorage.getItem(key)).charCodeAt(3), saveKey)).toBe(0x48);
  await waitForAnimationFrames(page);
  expect(await page.evaluate(() => launchMetrics.frames)).toBe(0);
  await setTabHidden(page, false);
  await expect.poll(() => page.evaluate(() => launchMetrics.frames)).toBeGreaterThan(3);
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('#play')).toHaveText('Pause');
  expect(await page.evaluate(() => launchMetrics.resumes)).toEqual([{ active: true, loads: 0 }]);
  await expect(page.locator('#error')).toBeEmpty();
});

test('an empty file choice retains an explicit Pause through hiding and returning', async ({ page }) => {
  await selectDemo(page);
  await expect.poll(() => page.evaluate(() => launchMetrics.frames)).toBeGreaterThan(3);
  await page.locator('#play').click();
  const frames = await page.evaluate(() => launchMetrics.frames);
  await page.locator('#change-cartridge').click();
  const chooser = page.waitForEvent('filechooser');
  await page.locator('#open-local').click();
  await setTabHidden(page, true);
  await (await chooser).setFiles([]);
  await setTabHidden(page, false);
  await waitForAnimationFrames(page);
  await expect(page.locator('#status')).toHaveText('Paused');
  expect(await page.evaluate(() => [launchMetrics.frames, launchMetrics.loads])).toEqual([frames, 1]);
  await expect(page.locator('#error')).toBeEmpty();
});

test('fullscreen entry and exit preserve playback and SRAM', async ({ page }) => {
  await selectDemo(page); await expect(page.locator('#status')).toHaveText('Playing');
  const before = await page.evaluate(() => {
    new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3] = 0x79;
    const store = Storage.prototype.setItem;
    window.sramWrites = 0;
    Storage.prototype.setItem = function (key, value) {
      if (key.startsWith('snes-rust:sram:')) sramWrites++;
      return store.call(this, key, value);
    };
    return launchMetrics.frames;
  });
  await page.locator('#fullscreen').click(); await expect(page.locator(':fullscreen')).toHaveAttribute('id', 'game-presentation');
  await expect.poll(() => page.evaluate(() => launchMetrics.frames)).toBeGreaterThan(before + 2);
  const fullscreenFrames = await page.evaluate(() => launchMetrics.frames);
  await page.keyboard.press('Escape'); await expect(page.locator(':fullscreen')).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => launchMetrics.frames)).toBeGreaterThan(fullscreenFrames + 2);
  await expect(page.locator('#status')).toHaveText('Playing');
  expect(await page.evaluate(() => launchMetrics.loads)).toBe(1);
  expect(await page.evaluate(() => sramWrites)).toBe(0);
  expect(await page.evaluate(() => new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3])).toBe(0x79);
  await page.locator('#display').focus();
  await setTabHidden(page, true);
  await expect(page.locator('#status')).toHaveText('Suspended');
  const suspended = await page.evaluate(() => launchMetrics.frames);
  await setTabHidden(page, false);
  await expect.poll(() => page.evaluate(() => launchMetrics.frames)).toBeGreaterThan(suspended + 2);
  await expect(page.locator(':fullscreen')).toHaveCount(0);
  await expect(page.locator('#display')).toBeFocused();
});
