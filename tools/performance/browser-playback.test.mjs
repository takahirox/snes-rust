import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import { installPlaybackProbe } from './browser-playback.mjs';

const require = createRequire(new URL('../../web/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
const dist = new URL('../../web/dist/', import.meta.url);
const files = { '/': ['index.html', 'text/html'], '/app.js': ['app.js', 'text/javascript'],
  '/style.css': ['style.css', 'text/css'], '/snes_wasm.wasm': ['snes_wasm.wasm', 'application/wasm'] };

test('classifies CRT fallback relative to the measured playback window', async t => {
  const rom = await readFile(new URL('../../demos/yonaki.sfc', import.meta.url));
  const browser = await chromium.launch();
  try {
    for (const when of ['before', 'during', 'after']) {
      await t.test(`context fallback ${when} the window`, async () => {
        const page = await browser.newPage();
        try {
          await page.route('http://127.0.0.1/**', async route => {
            const file = files[new URL(route.request().url()).pathname];
            if (!file) return route.fulfill({ status: 404 });
            const [name, contentType] = file;
            await route.fulfill({ contentType, body: await readFile(new URL(name, dist)) });
          });
          await page.addInitScript(installPlaybackProbe, { masks: [], warmup: 8, frames: 24, mode: 'crt' });
          await page.addInitScript(when => {
            window.loseCrtContext = () => document.querySelector('.crt-display')
              .dispatchEvent(new Event('webglcontextlost', { cancelable: true }));
            if (when === 'before') {
              const getContext = HTMLCanvasElement.prototype.getContext;
              HTMLCanvasElement.prototype.getContext = function (type, ...args) {
                return type === 'webgl' ? null : getContext.call(this, type, ...args);
              };
            }
          }, when);
          await page.goto('http://127.0.0.1/');
          await page.locator('#change-cartridge').click();
          if (when === 'during') {
            await page.evaluate(() => {
              const afterTenFields = () => {
                const fields = bench.frames - bench.warmupFields;
                if (fields >= 10) {
                  window.fallbackFields = fields;
                  window.loseCrtContext();
                } else requestAnimationFrame(afterTenFields);
              };
              requestAnimationFrame(afterTenFields);
            });
          }
          await page.locator('input[type=file]').setInputFiles({
            name: 'benchmark.sfc', mimeType: 'application/octet-stream', buffer: rom,
          });
          await page.waitForFunction(() => window.bench.end !== undefined);
          const result = await page.evaluate(when => {
            if (when === 'after') window.loseCrtContext();
            const { mode, modeAtStart, modeAtEnd, displayChanges, status, start, end, core } = window.bench;
            return { mode, modeAtStart, modeAtEnd, displayChanges, status, elapsed: end - start,
              measured: core.length, fallbackFields: window.fallbackFields,
              liveMode: document.getElementById('display').value,
              liveStatus: document.getElementById('display-status').textContent };
          }, when);
          assert.ok(result.measured >= 24 && result.measured <= 26);
          assert.equal(result.liveMode, 'pixel');
          assert.match(result.liveStatus, /Using Pixel/);
          if (when === 'after') {
            assert.deepEqual([result.mode, result.modeAtStart, result.modeAtEnd], ['crt', 'crt', 'crt']);
            assert.deepEqual(result.displayChanges, []);
            assert.equal(result.status, '');
          } else {
            assert.equal(result.displayChanges.length, 1);
            const [fallback] = result.displayChanges;
            assert.equal(fallback.mode, 'pixel');
            assert.equal(result.modeAtEnd, 'pixel');
            assert.equal(result.status, result.liveStatus);
            if (when === 'before') {
              assert.deepEqual([result.mode, result.modeAtStart], ['pixel', 'pixel']);
              assert.equal(fallback.beforeWindow, true);
              assert.ok(fallback.afterFields <= -8 && fallback.elapsedMs < 0);
            } else {
              assert.deepEqual([result.mode, result.modeAtStart], ['mixed', 'crt']);
              assert.equal(fallback.beforeWindow, false);
              assert.ok(result.fallbackFields >= 10 && result.fallbackFields <= 12);
              assert.equal(fallback.afterFields, result.fallbackFields);
              assert.ok(fallback.elapsedMs >= 0 && fallback.elapsedMs < result.elapsed);
            }
          }
        } finally {
          await page.close();
        }
      });
    }
  } finally {
    await browser.close();
  }
});
