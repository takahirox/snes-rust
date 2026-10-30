// Original MIT CPU/DMA guest. Literal edges and hash shared with native snapshot.
import { test, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../../', import.meta.url));
execFileSync('cargo', ['run', '--quiet', '--locked', '-p', 'snes-core', '--example', 'raster-fixture', '--', '.cache/raster'], { cwd: root });
const colors = [[255,0,0,255], [0,0,0,255], [0,255,0,255], [0,119,0,255],
  [0,0,0,255], [0,255,0,255], [0,0,0,255], [0,0,255,255]];
const rows = [[0,[0,40,51,77,102,138,149,167]], [1,[0,47,59,84,110,145,157,174]],
  [100,[0,42,53,79,104,140,151,169]], [223,[0,46,58,83,109,144,156,173]]];
test('raster CPU and DMA: native/Wasm frame hash, exact edges, live canvas and reset', async ({ page }, testInfo) => {
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  await page.addInitScript(() => {
    const instantiate = WebAssembly.instantiate;
    WebAssembly.instantiate = async (...args) => {
      const result = await instantiate(...args); window.testCore = result.instance.exports; return result;
    };
  });
  await page.goto('/');
  await expect(page.locator('#status')).toHaveText('Ready');
  const buffer = readFileSync(new URL('../../.cache/raster/raster.sfc', import.meta.url));
  await page.locator('#rom').setInputFiles({ name:'raster.sfc', mimeType:'application/octet-stream', buffer });
  await expect(page.locator('#status')).toHaveText('Playing');
  await page.locator('#reset').click(); // Pause at a fresh frame origin for hardware/replay assertions.
  // Actual Wasm ABI runs exactly three frames, without host register mutation.
  for (let attempt = 0; attempt < 2; attempt++) {
    const snapshot = await page.evaluate(rows => {
      const c = window.testCore;
      if (!c.reset()) throw new Error('reset failed');
      for (let i=0; i<3; i++) if (!c.run_frame(0)) throw new Error('guest deadline');
      const bytes = new Uint8Array(c.memory.buffer, c.framebuffer(), c.framebuffer_len());
      let hash = 2166136261;
      for (const b of bytes) hash = Math.imul(hash ^ b, 16777619) >>> 0;
      const pixels = rows.map(([y]) => Array.from({ length:256 }, (_, x) => [...bytes.slice((y*256+x)*4,(y*256+x+1)*4)]));
      return { hash, pixels, marker:c.peek(0x30), error:c.error_len(),
        geometry:[c.framebuffer_width(), c.framebuffer_height(), c.framebuffer_len()] };
    }, rows);
    expect(snapshot).toEqual({ hash:0x6041bd2d, marker:0x6b, error:0, geometry:[256,224,229376],
      pixels:rows.map(([,edges]) => Array.from({ length:256 }, (_, x) => colors[edges.findLastIndex(edge => edge <= x)])) });
    await testInfo.attach(`frame3-${attempt}.json`, { body:JSON.stringify(snapshot), contentType:'application/json' });
  }
  // UI playback draws real output. Poll phase drifts across fields, so these
  // points are safely inside each independently bounded color band.
  await page.locator('#play').click();
  await expect.poll(() => page.evaluate(() => {
    const ctx = document.querySelector('canvas').getContext('2d');
    return [0,70,95,125,200,255].map(x => [...ctx.getImageData(x,100,1,1).data]);
  })).toEqual([colors[0], colors[2], colors[3], colors[4], colors[7], colors[7]]);
  await page.locator('#play').click();
  await page.locator('canvas').screenshot({ path:testInfo.outputPath('raster.png') });
  expect(errors).toEqual([]);
  await expect(page.locator('#error')).toBeEmpty();
});
