// Original MIT guests and literal source-confirmed observations, Issue #63.
import { test, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../../', import.meta.url));
execFileSync('cargo', ['run', '--quiet', '--locked', '-p', 'snes-core', '--example', 'mode7-transform-fixture', '--', '.cache/mode7-transform'], { cwd: root });
const red = [255,0,0,255], green = [0,255,0,255];
for (const variant of ['fractional','clipped']) for (const layer of ['bg1','bg2']) {
  test(`Mode 7 transform: ${variant} ${layer} CPU-built Wasm/framebuffer/canvas`, async ({ page }) => {
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
    await expect(page.locator('#status')).toContainText('Open a local');
    const name = `${variant}-${layer}.sfc`;
    const buffer = readFileSync(new URL(`../../.cache/mode7-transform/${name}`, import.meta.url));
    await page.locator('#rom').setInputFiles({ name, mimeType:'application/octet-stream', buffer });
    await expect(page.locator('#status')).toContainText('loaded');
    await page.locator('#play').click();
    const points = variant === 'fractional' ? [[0,red],[7,red],[8,green],[15,green]] : [[0,red],[1,red],[2,green],[7,green]];
    for (let observation = 0; observation < 3; observation++) {
      await expect.poll(() => page.evaluate(points => {
        const c = window.testCore, canvas = document.querySelector('canvas');
        const bytes = new Uint8Array(c.memory.buffer, c.framebuffer(), c.framebuffer_len());
        const ctx = canvas.getContext('2d'), pixels = [], drawn = [];
        for (const y of [0,24,223]) for (const [x] of points) {
          pixels.push([...bytes.slice((y*256+x)*4,(y*256+x+1)*4)]);
          drawn.push([...ctx.getImageData(x,y,1,1).data]);
        }
        return { marker:c.peek(0x30),
          geometry:[c.framebuffer_width(),c.framebuffer_height(),c.framebuffer_len(),canvas.width,canvas.height],
          pixels, canvas:drawn };
      }, points), { timeout:5000 }).toEqual({ marker:0x63, geometry:[256,224,229376,256,224],
        pixels:[...points,...points,...points].map(([,color]) => color),
        canvas:[...points,...points,...points].map(([,color]) => color) });
      await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    }
    await page.locator('#play').click();
    expect(errors).toEqual([]);
    await expect(page.locator('#error')).toBeEmpty();
  });
}
