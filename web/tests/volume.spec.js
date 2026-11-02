import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { selectDemo } from './cartridge.js';

const fixture = readFileSync(new URL('../../tests/roms/smoke.bin', import.meta.url));
const preferenceKey = 'snes-rust:volume';
const slider = page => page.getByRole('slider', { name: 'Volume', exact: true });
const mute = page => page.locator('#mute');

async function local(page, buffer = fixture) {
  await page.locator('#rom').setInputFiles({ name: 'volume-test.sfc', mimeType: 'application/octet-stream', buffer });
  await expect(page.locator('#status')).toHaveText('Playing');
}

// Observe the real graph without changing the emulator samples or destination.
// Matched analyser taps measure input/output in the same render quantum, so game
// music/effect changes cannot masquerade as output attenuation.
async function instrument(page) {
  await page.addInitScript(() => {
    const instantiate = WebAssembly.instantiate;
    window.volumeMetrics = { frames: 0, starts: 0, invalid: 0, gains: 0, connections: 0, resumes: [] };
    WebAssembly.instantiate = async (...args) => {
      const result = await instantiate(...args), core = window.volumeCore = result.instance.exports;
      return { instance: { exports: { ...core, run_frame(mask) { volumeMetrics.frames++; return core.run_frame(mask); } } } };
    };
    const connect = AudioNode.prototype.connect, createGain = AudioContext.prototype.createGain;
    AudioContext.prototype.createGain = function () {
      volumeMetrics.gains++;
      const gain = window.outputGain = createGain.call(this);
      window.volumeContext = this;
      window.inputMeter = this.createAnalyser(); window.outputMeter = this.createAnalyser();
      inputMeter.fftSize = outputMeter.fftSize = 256;
      connect.call(gain, outputMeter);
      return gain;
    };
    AudioNode.prototype.connect = function (destination, ...args) {
      if (this instanceof AudioBufferSourceNode) {
        volumeMetrics.connections++;
        if (destination !== window.outputGain) volumeMetrics.invalid++;
        connect.call(this, inputMeter);
      }
      return connect.call(this, destination, ...args);
    };
    const active = new Map(), start = AudioBufferSourceNode.prototype.start;
    AudioBufferSourceNode.prototype.start = function (...args) {
      volumeMetrics.starts++;
      if (this.buffer.sampleRate !== 32000 || this.buffer.numberOfChannels !== 2) volumeMetrics.invalid++;
      active.set(this, args[0]); this.addEventListener('ended', () => active.delete(this));
      return start.apply(this, args);
    };
    const resume = AudioContext.prototype.resume;
    AudioContext.prototype.resume = function () { volumeMetrics.resumes.push(navigator.userActivation.isActive); return resume.call(this); };
    window.pendingSound = () => [...active.values()].filter(when => when > volumeContext.currentTime).length;
    window.measureVolume = () => {
      const rms = meter => {
        const data = new Float32Array(meter.fftSize); meter.getFloatTimeDomainData(data);
        return Math.sqrt(data.reduce((sum, value) => sum + value * value, 0) / data.length);
      };
      const input = rms(inputMeter), output = rms(outputMeter);
      return { input, output, ratio: input ? output / input : null, gain: outputGain.gain.value };
    };
  });
}

test.beforeEach(async ({ page }) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
  page.volumeErrors = errors;
  await instrument(page);
});
test.afterEach(async ({ page }) => { expect(page.volumeErrors).toEqual([]); });

async function ready(page) {
  await page.goto('/'); await expect(page.locator('#status')).toHaveText('Ready');
}
async function level(page, value) {
  await slider(page).fill(String(value));
  await expect(slider(page)).toHaveValue(String(value));
  await expect(mute(page)).toHaveAccessibleName(value ? 'Mute' : 'Unmute');
  await expect(mute(page)).toHaveAttribute('aria-pressed', String(!value));
}
async function measuredLevel(page, value) {
  let result;
  await expect.poll(async () => {
    result = await page.evaluate(() => measureVolume());
    return result.input > .00001 && (value === 0
      ? result.output < 1e-8 && result.ratio < 1e-5
      : Math.abs(result.ratio - value / 100) < .005);
  }, { timeout: 12000 }).toBe(true);
  return result;
}

for (const source of ['local', 'demo', 'demo-yonaki']) {
  test(`${source}: real queued stereo output attenuates at 0/25/50/100 without interrupting frames`, async ({ page }, testInfo) => {
    await ready(page);
    if (source === 'local') await local(page); else await selectDemo(page, source);
    await expect(page.locator('#status')).toHaveText('Playing');
    const evidence = [];
    for (const value of [100, 50, 25, 0, 100]) {
      const before = await page.evaluate(() => volumeMetrics.frames);
      await level(page, value);
      evidence.push({ value, ...await measuredLevel(page, value) });
      await expect.poll(() => page.evaluate(() => volumeMetrics.frames)).toBeGreaterThan(before + 2);
      await expect(page.locator('#status')).toHaveText('Playing');
    }
    const metrics = await page.evaluate(() => volumeMetrics);
    expect(metrics.gains).toBe(1); expect(metrics.invalid).toBe(0);
    expect(metrics.connections).toBe(metrics.starts); expect(metrics.starts).toBeGreaterThan(10);
    await testInfo.attach('live-output-rms', { body: JSON.stringify({ evidence, metrics }), contentType: 'application/json' });
  });
}

test('gain changes reach already queued sound while no new frame or source can run', async ({ page }, testInfo) => {
  await ready(page); await local(page); await measuredLevel(page, 100);
  await expect.poll(() => page.evaluate(() => pendingSound())).toBeGreaterThan(0);
  const result = await page.evaluate(() => {
    const pending = pendingSound(), frames = volumeMetrics.frames, starts = volumeMetrics.starts;
    const input = document.getElementById('volume'); input.value = '25'; input.dispatchEvent(new Event('input'));
    // Audio renders on its own thread; prevent rAF from generating new buffers.
    const until = performance.now() + 40; while (performance.now() < until) {}
    return { pending, frames: volumeMetrics.frames - frames, starts: volumeMetrics.starts - starts, ...measureVolume() };
  });
  expect(result.pending).toBeGreaterThan(0); expect(result.frames).toBe(0); expect(result.starts).toBe(0);
  expect(result.input).toBeGreaterThan(.00001); expect(result.ratio).toBeCloseTo(.25, 2);
  await testInfo.attach('already-queued-output', { body: JSON.stringify(result), contentType: 'application/json' });
});

test('native keyboard, mute restore, pause/reset/cartridge/fullscreen and reload share one preference', async ({ page }) => {
  await ready(page);
  await expect(page.locator('#sound, input[type=checkbox]')).toHaveCount(0);
  await expect(slider(page)).toHaveValue('100'); await expect(mute(page)).toHaveAccessibleName('Mute');
  expect(await page.evaluate(() => volumeMetrics.resumes)).toEqual([]); // No autoplay unlock.
  await slider(page).focus(); await page.keyboard.press('Home');
  await expect(slider(page)).toHaveValue('0'); await expect(slider(page)).toHaveAttribute('aria-valuetext', '0% (muted)');
  await page.keyboard.press('ArrowRight'); await expect(slider(page)).toHaveValue('1');
  await page.keyboard.press('End'); await expect(slider(page)).toHaveValue('100');
  await level(page, 50);
  await mute(page).focus(); await page.keyboard.press('Space');
  await expect(mute(page)).toBeFocused();
  expect(await mute(page).evaluate(el => getComputedStyle(el).outlineStyle)).toBe('solid');
  await expect(slider(page)).toHaveValue('0'); await expect(mute(page)).toHaveAccessibleName('Unmute');
  await page.reload(); await expect(page.locator('#status')).toHaveText('Ready');
  await expect(slider(page)).toHaveValue('0');
  expect(await page.evaluate(() => volumeMetrics.gains)).toBe(0);
  await mute(page).click(); await expect(slider(page)).toHaveValue('50');
  await local(page); await measuredLevel(page, 50);
  await mute(page).click(); await level(page, 25); await measuredLevel(page, 25); // Slider unmutes.
  await page.locator('#play').click(); await level(page, 50);
  await expect(page.locator('#status')).toHaveText('Paused');
  await page.locator('#play').click(); await measuredLevel(page, 50);
  await mute(page).click();
  for (const id of ['demo', 'demo-yonaki']) {
    await selectDemo(page, id); await page.locator('#reset').click();
    await expect(slider(page)).toHaveValue('0'); await expect(mute(page)).toHaveAccessibleName('Unmute');
  }
  await local(page);
  await page.locator('#fullscreen').click();
  await expect.poll(() => page.evaluate(() => Boolean(document.fullscreenElement))).toBe(true);
  await expect(slider(page)).toBeVisible(); await expect(slider(page)).toHaveValue('0');
  await page.locator('#fullscreen').click();
  await expect.poll(() => page.evaluate(() => Boolean(document.fullscreenElement))).toBe(false);
  await mute(page).click(); await expect(slider(page)).toHaveValue('50'); await measuredLevel(page, 50);
  await page.reload(); await expect(slider(page)).toHaveValue('50');
  await local(page); await measuredLevel(page, 50);
  expect(await page.evaluate(() => volumeMetrics.gains)).toBe(1);
});

for (const saved of ['not JSON', 'null', '{"volume":101,"lastNonzeroVolume":50}', '{"volume":25,"lastNonzeroVolume":0}', '{"volume":"50","lastNonzeroVolume":50}']) {
  test(`invalid stored preference falls back safely: ${saved}`, async ({ page }) => {
    await page.addInitScript(({ key, saved }) => localStorage.setItem(key, saved), { key: preferenceKey, saved });
    await ready(page); await expect(slider(page)).toHaveValue('100');
    await local(page); await measuredLevel(page, 100);
  });
}
for (const denial of ['access', 'read', 'write']) {
  test(`storage ${denial} denial does not block volume or a non-battery local ROM`, async ({ page }) => {
    await page.addInitScript(denial => {
      if (denial === 'access') Object.defineProperty(window, 'localStorage', { get() { throw new DOMException('Denied', 'SecurityError'); } });
      else Storage.prototype[denial === 'read' ? 'getItem' : 'setItem'] = () => { throw new DOMException('Denied', 'SecurityError'); };
    }, denial);
    await ready(page); await level(page, 25);
    // SRAM must retain its existing protection on denied storage; use the
    // source-built smoke guest's non-battery header variant for this audio check.
    const noBattery = Buffer.from(fixture); noBattery[0x7fd6] = 0;
    await local(page, noBattery); await measuredLevel(page, 25);
    await mute(page).click(); await measuredLevel(page, 0);
    await mute(page).click(); await measuredLevel(page, 25);
    await expect(page.locator('#error')).toBeEmpty();
  });
}

for (const viewport of [{ width: 1280, height: 900 }, { width: 320, height: 740 }, { width: 667, height: 320 }, { width: 568, height: 320, safeArea: true }]) {
  test.describe(`${viewport.width}x${viewport.height}${viewport.safeArea ? ' safe area' : ''}`, () => {
    test.use({ viewport, hasTouch: viewport.width < 1000, isMobile: viewport.width < 1000 });
    test('visible audio group keeps its geometry and supports native pointer dragging', async ({ page }, testInfo) => {
      if (viewport.safeArea) {
        const session = await page.context().newCDPSession(page);
        await session.send('Emulation.setSafeAreaInsetsOverride', { insets: { left: 44, right: 44, bottom: 21, top: 0 } });
      }
      await ready(page);
      const geometry = () => page.locator('#mute, #volume').evaluateAll(nodes => nodes.map(n => {
        const { x, y, width, height } = n.getBoundingClientRect(); return { x, y, width, height };
      }));
      const initial = await geometry();
      for (const target of await page.locator('#mute, #volume, canvas, [data-button]').all()) {
        // Desktop hides virtual buttons.
        if (await target.isVisible()) await expect(target).toBeInViewport({ ratio: 1 });
      }
      expect(initial[0].height).toBeGreaterThanOrEqual(44); expect(initial[0].width).toBeGreaterThanOrEqual(44);
      expect(initial[1].width).toBeGreaterThanOrEqual(58); expect(initial[1].height).toBeGreaterThanOrEqual(44);
      const bounds = await page.evaluate(() => ({ width: innerWidth, scroll: document.documentElement.scrollWidth, height: innerHeight, scrollHeight: document.documentElement.scrollHeight }));
      expect(bounds.scroll).toBe(bounds.width);
      if (viewport.width < 1000) expect(bounds.scrollHeight).toBe(bounds.height);
      await page.screenshot({ path: testInfo.outputPath('before-cartridge.png') });
      await local(page); expect(await geometry()).toEqual(initial);
      const r = await slider(page).boundingBox(), y = r.y + r.height / 2;
      if (viewport.width < 1000) {
        const session = await page.context().newCDPSession(page);
        await session.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ id: 1, x: r.x + r.width - 8, y }] });
        for (let i = 1; i <= 5; i++) await session.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ id: 1, x: r.x + r.width - 8 - (r.width - 16) * i / 5, y }] });
        await session.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
      } else {
        await page.mouse.move(r.x + r.width - 8, y); await page.mouse.down();
        await page.mouse.move(r.x + 8, y, { steps: 5 }); await page.mouse.up();
      }
      await expect(slider(page)).toHaveValue('0'); await measuredLevel(page, 0);
      await mute(page).click();
      expect(Number(await slider(page).inputValue())).toBeGreaterThan(0);
      await expect(page.locator('#status')).toHaveText('Playing');
      const overlaps = await page.evaluate(() => {
        const audio = [...document.querySelectorAll('#mute, #volume')].map(n => n.getBoundingClientRect());
        return [...document.querySelectorAll('canvas, [data-button]')].filter(n => n.getClientRects().length).some(n => {
          const r = n.getBoundingClientRect();
          return audio.some(a => a.left < r.right && a.right > r.left && a.top < r.bottom && a.bottom > r.top);
        });
      });
      expect(overlaps).toBe(false);
      await page.screenshot({ path: testInfo.outputPath('after-cartridge.png') });
    });
  });
}
