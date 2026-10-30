// Original MIT CPU guests and literal screen-row observations, Issue #65.
import { test, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../../', import.meta.url));
execFileSync('cargo', ['run', '--quiet', '--locked', '-p', 'snes-core', '--example', 'mode7-row-fixture', '--', '.cache/mode7-row'], { cwd: root });
const green = [0,255,0,255];
const points = [
  [6,0,green], [7,0,[255,0,0,255]], [8,0,green],
  [6,9,green], [7,9,[0,0,255,255]], [8,9,green],
  [6,223,green], [7,223,[255,255,0,255]], [8,223,green],
  [7,1,green], [7,8,green], [7,10,green], [7,222,green],
];
for (const variant of ['normal','flipped']) for (const layer of ['bg1','bg2']) {
  test(`Mode 7 row origin: ${variant} ${layer} CPU-built Wasm/framebuffer/canvas`, async ({ page }) => {
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
    await page.addInitScript(() => {
      const instantiate = WebAssembly.instantiate;
      WebAssembly.instantiate = async (...args) => {
        const result = await instantiate(...args); window.testCore = result.instance.exports; return result;
      };
    });
    await page.goto('/');
    await expect(page.locator('#status')).toHaveText('Ready');
    const name = `${variant}-${layer}.sfc`;
    const buffer = readFileSync(new URL(`../../.cache/mode7-row/${name}`, import.meta.url));
    await page.locator('#rom').setInputFiles({ name, mimeType:'application/octet-stream', buffer });
    await expect(page.locator('#status')).toHaveText('Playing');
  await page.locator('#reset').click(); // Pause at a fresh frame origin for hardware/replay assertions.
    await page.locator('#play').click();
    for (let observation = 0; observation < 3; observation++) {
      await expect.poll(() => page.evaluate(points => {
        const c = window.testCore, canvas = document.querySelector('canvas');
        const bytes = new Uint8Array(c.memory.buffer, c.framebuffer(), c.framebuffer_len());
        const ctx = canvas.getContext('2d'), pixels = [], drawn = [];
        for (const [x,y] of points) {
          pixels.push([...bytes.slice((y*256+x)*4,(y*256+x+1)*4)]);
          drawn.push([...ctx.getImageData(x,y,1,1).data]);
        }
        return { marker:c.peek(0x30), coreError:c.error_len(),
          geometry:[c.framebuffer_width(),c.framebuffer_height(),c.framebuffer_len(),canvas.width,canvas.height],
          pixels, canvas:drawn };
      }, points), { timeout:5000 }).toEqual({ marker:0x65, coreError:0, geometry:[256,224,229376,256,224],
        pixels:points.map(([, ,color]) => color), canvas:points.map(([, ,color]) => color) });
      await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    }
    await page.locator('#play').click();
    expect(errors).toEqual([]);
    await expect(page.locator('#error')).toBeEmpty();
  });
}
