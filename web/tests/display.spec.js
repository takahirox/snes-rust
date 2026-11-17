import { test, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { selectDemo } from './cartridge.js';

const root = fileURLToPath(new URL('../../', import.meta.url));
execFileSync('cargo', ['run', '--quiet', '--locked', '-p', 'snes-core', '--example', 'hires-fixture', '--', '.cache/hires'], { cwd: root });
const fixture = readFileSync(new URL('../../tests/roms/smoke.bin', import.meta.url));
const evidence = new URL('../../docs/issue-142/', import.meta.url);
if (process.env.UPDATE_CRT_EVIDENCE) mkdirSync(evidence, { recursive: true });

async function boot(page) {
  await page.addInitScript(() => {
    const instantiate = WebAssembly.instantiate;
    window.videoMetrics = { frames: 0, resets: 0, loads: 0, stops: 0, masks: [], audio: 0 };
    WebAssembly.instantiate = async (...args) => {
      const r = await instantiate(...args), core = window.videoCore = r.instance.exports;
      return { instance: { exports: { ...core,
        run_frame(mask) {
          if (window.breakGpu && window.slowWorkload) { const end = performance.now() + 20; while (performance.now() < end) {} }
          videoMetrics.frames++; videoMetrics.masks.push(mask); return core.run_frame(mask); },
        reset() { videoMetrics.resets++; return core.reset(); },
        prepare_rom() { videoMetrics.loads++; return core.prepare_rom(); },
      } } };
    };
    const start = AudioBufferSourceNode.prototype.start, stop = AudioBufferSourceNode.prototype.stop;
    AudioBufferSourceNode.prototype.start = function (...args) { videoMetrics.audio++; return start.apply(this, args); };
    AudioBufferSourceNode.prototype.stop = function (...args) { videoMetrics.stops++; return stop.apply(this, args); };
  });
  const errors = []; page.on('pageerror', e => errors.push(e.message));
  page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
  page.displayErrors = errors;
  await page.goto('/'); await expect(page.locator('#status')).toHaveText('Ready');
}
async function local(page, buffer = fixture) {
  await page.locator('#rom').setInputFiles({ name: 'display.sfc', mimeType: 'application/octet-stream', buffer });
  await expect(page.locator('#status')).toHaveText('Playing');
}
async function choose(page, value) {
  // Native selector remains in Help; change in one task to compare live state.
  await page.evaluate(value => {
    const select = document.getElementById('display'); select.value = value;
    select.dispatchEvent(new Event('change'));
  }, value);
  await expect(page.locator('#display')).toHaveValue(value);
  if (value === 'crt') await expect(page.locator('.crt-display')).toBeVisible();
}
const frameHash = page => page.evaluate(async () => {
  const c = videoCore, data = new Uint8Array(c.memory.buffer, c.framebuffer(), c.framebuffer_len());
  return [...new Uint8Array(await crypto.subtle.digest('SHA-256', data))].join(',');
});
async function geometry(page) {
  await expect.poll(() => page.evaluate(() => {
    const a = document.getElementById('video').getBoundingClientRect(), b = document.querySelector('.crt-display').getBoundingClientRect();
    return Math.max(...['x', 'y', 'width', 'height'].map(k => Math.abs(a[k] - b[k])));
  })).toBeLessThan(.05);
  const rects = await page.evaluate(() => ['#video', '.crt-display'].map(selector => {
    const r = document.querySelector(selector).getBoundingClientRect(); return [r.x, r.y, r.width, r.height];
  }));
  for (let i = 0; i < 4; i++) expect(rects[1][i]).toBeCloseTo(rects[0][i], 1);
  expect(rects[0][2] / rects[0][3]).toBeCloseTo(256 / 224, 3);
}

test('live selection preserves frames, input, audio, SRAM, focus and exact Pixel bytes', async ({ page }) => {
  await boot(page); await local(page);
  await expect(page.locator('.crt-display')).toHaveCount(0);
  await page.getByText('Help / Controls', { exact: true }).click();
  const select = page.getByRole('combobox', { name: 'Display', exact: true });
  await expect(select).toHaveValue('pixel'); await select.focus();
  const before = await page.evaluate(() => {
    new Uint8Array(videoCore.memory.buffer, videoCore.sram_buffer(), videoCore.sram_len())[3] = 0x79;
    return { ...videoMetrics };
  });
  await page.keyboard.press('c');
  await expect(select).toHaveValue('crt'); await expect(select).toBeFocused();
  await expect(page.locator('.crt-display')).toBeVisible();
  // Native select type-ahead groups letters within one second on macOS.
  await page.waitForTimeout(1100); await page.keyboard.press('p'); await expect(select).toHaveValue('pixel');
  await page.getByText('Help / Controls', { exact: true }).click();
  await page.locator('#video').focus(); await page.keyboard.down('ArrowRight');
  for (const value of ['crt', 'pixel', 'crt']) {
    await choose(page, value); await expect(page.locator('#video')).toBeFocused();
  }
  await expect.poll(() => page.evaluate(() => videoMetrics.frames)).toBeGreaterThan(before.frames + 3);
  await expect.poll(() => page.evaluate(() => videoCore.peek(0x11))).toBe(1);
  await page.keyboard.up('ArrowRight');
  const after = await page.evaluate(() => ({ ...videoMetrics, marker: new Uint8Array(videoCore.memory.buffer, videoCore.sram_buffer(), videoCore.sram_len())[3] }));
  expect(after.loads).toBe(before.loads); expect(after.resets).toBe(before.resets);
  expect(after.marker).toBe(0x79); expect(after.audio).toBeGreaterThan(before.audio);
  expect(after.stops).toBe(before.stops);
  await page.locator('#play').click();
  const hash = await frameHash(page);
  for (const mode of ['pixel', 'crt', 'pixel']) { await choose(page, mode); expect(await frameHash(page)).toBe(hash); }
  expect(await page.evaluate(() => {
    const c = videoCore, expected = new Uint8Array(c.memory.buffer, c.framebuffer(), c.framebuffer_len());
    const actual = document.getElementById('video').getContext('2d').getImageData(0, 0, c.framebuffer_width(), c.framebuffer_height()).data;
    return expected.every((v, i) => v === actual[i]);
  })).toBe(true);
  expect(page.displayErrors).toEqual([]);
});

test('CRT handles low/hires transitions, paused resize, non-integer scale, fullscreen and reset', async ({ page }) => {
  await boot(page);
  await local(page, readFileSync(new URL('../../.cache/hires/hires-5-switch.sfc', import.meta.url)));
  await choose(page, 'crt'); await geometry(page);
  await page.locator('#video').focus(); await page.keyboard.down('KeyX');
  await expect.poll(() => page.evaluate(() => videoCore.framebuffer_width())).toBe(512);
  await geometry(page);
  await page.locator('#play').click(); const hiresHash = await frameHash(page);
  for (const mode of ['pixel', 'crt']) {
    await choose(page, mode);
    if (process.env.UPDATE_CRT_EVIDENCE) writeFileSync(new URL(`hires-${mode}.png`, evidence), await page.locator('#video').screenshot());
    expect(await frameHash(page)).toBe(hiresHash);
  }
  await page.locator('#play').click();
  await page.keyboard.up('KeyX');
  await expect.poll(() => page.evaluate(() => videoCore.framebuffer_width())).toBe(256);
  await page.locator('#play').click(); const hash = await frameHash(page);
  await page.setViewportSize({ width: 1051, height: 801 }); await geometry(page);
  expect(await frameHash(page)).toBe(hash);
  await page.locator('#fullscreen').click();
  await expect.poll(() => page.evaluate(() => Boolean(document.fullscreenElement))).toBe(true);
  await geometry(page); await page.evaluate(() => document.exitFullscreen());
  await page.locator('#reset').click(); await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('.crt-display')).toBeVisible();
  await local(page); await expect(page.locator('#display')).toHaveValue('crt');
  await page.evaluate(() => {
    Object.defineProperty(document, 'hidden', { configurable: true, value: true });
    document.dispatchEvent(new Event('visibilitychange'));
  });
  await expect(page.locator('#status')).toHaveText('Paused'); const hiddenHash = await frameHash(page);
  const retained = await page.locator('.crt-display').evaluate(c => c.toDataURL());
  await page.evaluate(() => { delete document.hidden; document.dispatchEvent(new Event('visibilitychange')); });
  await expect(page.locator('#status')).toHaveText('Paused');
  expect(await frameHash(page)).toBe(hiddenHash);
  expect(await page.locator('.crt-display').evaluate(c => c.toDataURL())).toBe(retained);
  await page.locator('#play').click();
  await choose(page, 'pixel'); await expect(page.locator('.crt-display')).toHaveCount(0);
  expect(page.displayErrors).toEqual([]);
});

test.describe('retina touch layouts', () => {
  test.use({ deviceScaleFactor: 3, hasTouch: true, isMobile: true });
  for (const viewport of [{ width: 320, height: 740 }, { width: 667, height: 320 }, { width: 568, height: 320 }]) {
    test(`CRT layout and touch at ${viewport.width}x${viewport.height}`, async ({ page }) => {
      await page.setViewportSize(viewport);
      const session = await page.context().newCDPSession(page);
      if (viewport.width === 568) await session.send('Emulation.setSafeAreaInsetsOverride', { insets: { left: 44, right: 44, bottom: 21, top: 0 } });
      await boot(page); await local(page); await choose(page, 'crt');
      await geometry(page);
      for (const button of await page.locator('[data-button]').all()) {
        await expect(button).toBeVisible(); const r = await button.boundingBox();
        expect(r.y + r.height).toBeLessThanOrEqual(viewport.height + 1);
      }
      const c = await page.locator('#video').boundingBox(), slider = await page.locator('#volume').boundingBox();
      expect(slider.y).toBeGreaterThanOrEqual(c.y + c.height);
      await expect(page.locator('#volume')).toBeVisible();
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      const a = await page.getByRole('button', { name: 'SNES A', exact: true }).boundingBox();
      const point = { id: 1, x: a.x + a.width / 2, y: a.y + a.height / 2 };
      await session.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [point] });
      await expect.poll(() => page.evaluate(() => videoMetrics.masks.at(-1) & 0x80)).toBe(0x80);
      await session.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [point] });
      await expect.poll(() => page.evaluate(() => videoMetrics.masks.at(-1) & 0x80)).toBe(0);
      expect(page.displayErrors).toEqual([]);
    });
  }

});

for (const failure of ['unavailable', 'compile', 'link', 'draw', 'error', 'budget', 'workload']) {
  test(`GPU ${failure} falls back to playable Pixel and can retry`, async ({ page }) => {
    await page.addInitScript(failure => {
      window.breakGpu = true; window.slowWorkload = failure === 'workload';
      const getContext = HTMLCanvasElement.prototype.getContext;
      HTMLCanvasElement.prototype.getContext = function (kind, ...args) {
        if (kind === 'webgl' && breakGpu && failure === 'unavailable') return null;
        return getContext.call(this, kind, ...args);
      };
      for (const [method, kind] of [['getShaderParameter', 'compile'], ['getProgramParameter', 'link'], ['drawArrays', 'draw'], ['getError', 'error']]) {
        const original = WebGLRenderingContext.prototype[method];
        WebGLRenderingContext.prototype[method] = function (...args) {
          if (breakGpu && failure === kind) {
            if (kind === 'draw') throw new Error('Injected GPU failure');
            return kind === 'error' ? this.OUT_OF_MEMORY : false;
          }
          if (method === 'drawArrays' && breakGpu && failure === 'budget') {
            const end = performance.now() + 20; while (performance.now() < end) {}
          }
          return original.apply(this, args);
        };
      }
    }, failure);
    await boot(page); await local(page);
    await page.evaluate(() => { const s = document.getElementById('display'); s.value = 'crt'; s.dispatchEvent(new Event('change')); });
    await expect(page.locator('#display')).toHaveValue('pixel', { timeout: 15000 });
    await expect(page.locator('#display-status')).toContainText('Using Pixel');
    await expect(page.locator('.crt-display')).toBeHidden();
    const before = await page.evaluate(() => videoMetrics.frames);
    await expect.poll(() => page.evaluate(() => videoMetrics.frames)).toBeGreaterThan(before + 2);
    await expect(page.locator('#status')).toHaveText('Playing');
    await page.evaluate(() => { window.breakGpu = false; });
    await choose(page, 'crt'); await choose(page, 'pixel');
    expect(page.displayErrors).toEqual([]);
  });
}

test('context loss exposes Pixel immediately; restoration rebuilds resources on retry', async ({ page }) => {
  await boot(page); await local(page); await choose(page, 'crt');
  await page.evaluate(() => {
    window.loseGpu = document.querySelector('.crt-display').getContext('webgl').getExtension('WEBGL_lose_context');
    loseGpu.loseContext();
  });
  await expect(page.locator('#display')).toHaveValue('pixel');
  await expect(page.locator('.crt-display')).toBeHidden();
  await expect(page.locator('#status')).toHaveText('Playing');
  await page.evaluate(() => loseGpu.restoreContext());
  await expect(page.locator('#display-status')).toContainText('available again');
  await choose(page, 'crt'); await geometry(page);
  await page.locator('#play').click();
  const screenshot = await page.locator('.crt-display').screenshot(); expect(screenshot.length).toBeGreaterThan(1000);
  expect(page.displayErrors).toEqual([]);
});

for (const preference of ['crt', 'pixel', 'corrupt', 'denied']) {
  test(`display preference ${preference} is robust and independent of SRAM`, async ({ page }) => {
    await page.addInitScript(preference => {
      if (preference === 'denied') {
        const get = Storage.prototype.getItem, set = Storage.prototype.setItem;
        Storage.prototype.getItem = function (key) { if (key === 'snes-rust:display') throw new Error('Denied'); return get.call(this, key); };
        Storage.prototype.setItem = function (key, value) { if (key === 'snes-rust:display') throw new Error('Denied'); return set.call(this, key, value); };
      } else localStorage.setItem('snes-rust:display', preference);
    }, preference);
    await boot(page); await expect(page.locator('#placeholder')).toBeVisible();
    await expect(page.locator('#display')).toHaveValue(preference === 'crt' ? 'crt' : 'pixel');
    await local(page); await choose(page, 'crt'); await choose(page, 'pixel');
    await expect(page.locator('#error')).toBeEmpty(); expect(page.displayErrors).toEqual([]);
  });
}

test('explicit CRT preference persists across reload', async ({ page }) => {
  await boot(page); await local(page); await choose(page, 'crt');
  await page.reload(); await expect(page.locator('#status')).toHaveText('Ready');
  await expect(page.locator('#display')).toHaveValue('crt'); await expect(page.locator('#placeholder')).toBeVisible();
  await local(page); await expect(page.locator('.crt-display')).toBeVisible();
  await choose(page, 'pixel'); await page.reload();
  await expect(page.locator('#display')).toHaveValue('pixel');
});

// Real Wasm games, paused only to make the comparison byte-identical. Switching
// during gameplay is covered above. No concept images or synthetic game frames.
for (const demo of ['demo', 'demo-yonaki']) {
  test(`${demo}: same-scene Pixel and CRT browser screenshots`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width: 1280, height: 900 }); await boot(page); await selectDemo(page, demo);
    const tap = async code => { await page.keyboard.down(code); await page.waitForTimeout(100); await page.keyboard.up(code); await page.waitForTimeout(500); };
    await page.waitForTimeout(3000);
    if (demo === 'demo') { await tap('KeyX'); await page.waitForTimeout(1500); await tap('KeyX'); await tap('KeyX'); }
    else { await tap('KeyX'); await tap('KeyX'); await tap('Enter'); await tap('Enter'); }
    if (demo === 'demo-yonaki') {
      // The chapter card is grayscale; the prologue photo has colored pixels.
      // Its palette can be only 112 colors in the translucent-figure ROM.
      await expect.poll(() => page.evaluate(() => {
        const c = videoCore, data = new Uint8Array(c.memory.buffer, c.framebuffer(), c.framebuffer_len());
        return data.some((r, i) => i % 4 === 0 && (r !== data[i+1] || r !== data[i+2]));
      }), { timeout: 15000 }).toBe(true);
    }
    await page.waitForTimeout(3500); await page.locator('#play').click();
    const hash = await frameHash(page), name = demo === 'demo' ? 'shionari' : 'yonaki';
    const images = [];
    for (const mode of ['pixel', 'crt']) {
      await choose(page, mode); await geometryIfCrt(page, mode);
      const body = await page.locator('#video').screenshot(); images.push(body);
      await testInfo.attach(`${name}-${mode}`, { body, contentType: 'image/png' });
      if (process.env.UPDATE_CRT_EVIDENCE) writeFileSync(new URL(`${name}-${mode}.png`, evidence), body);
      expect(await frameHash(page)).toBe(hash);
    }
    expect(images[0].equals(images[1])).toBe(false);
    expect(page.displayErrors).toEqual([]);
  });
}
async function geometryIfCrt(page, mode) { if (mode === 'crt') await geometry(page); }

test('measure Pixel and CRT submission/frame workload with normal and throttled CPU', async ({ page }, testInfo) => {
  test.setTimeout(60000);
  await page.addInitScript(() => {
    window.renderTimes = []; window.rafTimes = []; window.measureMode = 'pixel';
    const put = CanvasRenderingContext2D.prototype.putImageData, error = WebGLRenderingContext.prototype.getError;
    let renderStart;
    CanvasRenderingContext2D.prototype.putImageData = function (...args) {
      renderStart = performance.now(); const result = put.apply(this, args);
      if (measureMode === 'pixel') renderTimes.push(performance.now() - renderStart);
      return result;
    };
    WebGLRenderingContext.prototype.getError = function (...args) {
      const result = error.apply(this, args);
      if (renderStart !== undefined && measureMode === 'crt') renderTimes.push(performance.now() - renderStart);
      renderStart = undefined; return result;
    };
    const raf = window.requestAnimationFrame;
    window.requestAnimationFrame = callback => raf(time => {
      const start = performance.now(); callback(time); rafTimes.push({ at: time, cost: performance.now() - start });
    });
  });
  await page.setViewportSize({ width: 1280, height: 900 }); await boot(page); await selectDemo(page, 'demo-yonaki');
  await page.waitForTimeout(3000);
  const cdp = await page.context().newCDPSession(page), results = [];
  for (const rate of [1, 2, 4]) {
    await cdp.send('Emulation.setCPUThrottlingRate', { rate });
    for (const mode of ['pixel', 'crt']) {
      await choose(page, mode); await page.waitForTimeout(250);
      await page.evaluate(mode => { measureMode = mode; renderTimes = []; rafTimes = []; videoMetrics.frames = 0; }, mode);
      await page.waitForTimeout(3000);
      const result = await page.evaluate(() => {
        const summarize = values => {
          const v = values.toSorted((a,b) => a-b);
          return { count: v.length, mean: v.reduce((a,b) => a+b, 0) / v.length, p95: v[Math.floor(v.length * .95)] };
        };
        return { renderer: summarize(renderTimes), callback: summarize(rafTimes.map(v => v.cost)),
          interval: summarize(rafTimes.slice(1).map((v,i) => v.at - rafTimes[i].at)), frames: videoMetrics.frames,
          actualMode: document.getElementById('display').value,
          gpu: document.querySelector('.crt-display')?.getContext('webgl').getParameter(WebGLRenderingContext.RENDERER) };
      });
      results.push({ cpuThrottle: rate, mode, ...result });
      await expect(page.locator('#status')).toHaveText('Playing');
      if (mode === 'crt' && result.actualMode === 'pixel') await expect(page.locator('#display-status')).toContainText('Using Pixel');
      else await expect(page.locator('#display')).toHaveValue(mode);
    }
  }
  await cdp.send('Emulation.setCPUThrottlingRate', { rate: 1 });
  const data = JSON.stringify({ browser: await page.context().browser().version(), platform: process.platform, results }, null, 2);
  await testInfo.attach('render-workload', { body: data, contentType: 'application/json' });
  if (process.env.UPDATE_CRT_EVIDENCE) writeFileSync(new URL('render-workload.json', evidence), data + '\n');
  expect(page.displayErrors).toEqual([]);
});

test('repeated Pixel/CRT choices release GPU resources and reuse one context', async ({ page }) => {
  await page.addInitScript(() => {
    window.resources = {};
    for (const type of ['Shader', 'Program', 'Texture', 'Buffer']) {
      resources[type] = 0;
      for (const [action, delta] of [['create', 1], ['delete', -1]]) {
        const name = action + type, original = WebGLRenderingContext.prototype[name];
        WebGLRenderingContext.prototype[name] = function (...args) {
          const result = original.apply(this, args);
          if (action === 'create' ? result : args[0]) resources[type] += delta;
          return result;
        };
      }
    }
    window.contexts = new Set();
    const getContext = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function (kind, ...args) {
      const result = getContext.call(this, kind, ...args); if (kind === 'webgl') contexts.add(result); return result;
    };
  });
  await boot(page); await local(page);
  for (let i = 0; i < 10; i++) { await choose(page, 'crt'); await choose(page, 'pixel'); }
  expect(await page.evaluate(() => resources)).toEqual({ Shader: 0, Program: 0, Texture: 0, Buffer: 0 });
  expect(await page.evaluate(() => contexts.size)).toBe(1);
  expect(page.displayErrors).toEqual([]);
});

test('CRT pixels retain source orientation, brightness, opacity and canvas capture', async ({ page }) => {
  await boot(page); await local(page);
  await expect.poll(() => page.evaluate(() => videoMetrics.frames)).toBeGreaterThan(10);
  await page.locator('#play').click(); await choose(page, 'crt');
  const pixels = await page.evaluate(async () => {
    const source = document.getElementById('video'), c = document.querySelector('.crt-display'), gl = c.getContext('webgl');
    const original = source.getContext('2d').getImageData(0, 0, source.width, source.height).data;
    const data = new Uint8Array(c.width * c.height * 4); gl.readPixels(0, 0, c.width, c.height, gl.RGBA, gl.UNSIGNED_BYTE, data);
    let error = 0, inputSum = 0, outputSum = 0, opaque = true;
    for (let y = 0; y < c.height; y++) for (let x = 0; x < c.width; x++) {
      const i = (y * c.width + x) * 4, j = ((source.height - 1 - Math.floor(y * source.height / c.height)) * source.width + Math.floor(x * source.width / c.width)) * 4;
      opaque &&= data[i+3] === 255;
      for (let k = 0; k < 3; k++) { error += Math.abs(data[i+k] - original[j+k]); inputSum += original[j+k]; outputSum += data[i+k]; }
    }
    // Capture the actual GPU surface while paused. No CPU recreation of CRT.
    const stream = c.captureStream(), video = document.createElement('video'); video.srcObject = stream; video.muted = true;
    const started = video.play();
    // Request a fresh presentation after starting capture, without running the
    // paused core; browsers only deliver canvas-stream frames after a paint.
    window.dispatchEvent(new Event('resize')); await started;
    const capture = document.createElement('canvas'); capture.width = c.width; capture.height = c.height;
    const ctx = capture.getContext('2d'); ctx.drawImage(video, 0, 0);
    const captured = ctx.getImageData(0, 0, c.width, c.height).data;
    stream.getTracks().forEach(track => track.stop()); video.srcObject = null;
    return { error: error / (c.width * c.height * 3), ratio: outputSum / inputSum, opaque,
      lit: outputSum > 0, captured: captured.some((v,i) => i % 4 !== 3 && v > 0) };
  });
  expect(pixels.lit).toBe(true); expect(pixels.opaque).toBe(true); expect(pixels.captured).toBe(true);
  expect(pixels.ratio).toBeGreaterThan(.9); expect(pixels.ratio).toBeLessThan(1.1);
  expect(pixels.error).toBeGreaterThan(0); expect(pixels.error).toBeLessThan(255 * .1);
  expect(page.displayErrors).toEqual([]);
});
