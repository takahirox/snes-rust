// Original MIT CPU-built OBJ fixtures and actual Wasm/canvas vectors for #55.
import { test, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../../', import.meta.url));
execFileSync('cargo', ['run', '--quiet', '--locked', '-p', 'snes-core', '--example', 'obj-sizes-fixture', '--', '.cache/obj-sizes'], { cwd: root });
const colors = [
  [0,0,0,255], [255,0,0,255], [0,255,0,255], [0,0,255,255],
  [255,255,0,255], [0,255,255,255], [255,0,255,255], [255,255,255,255],
];
for (const selector of [6,7]) {
  test(`OBJ sizes: selector ${selector} lower halves and square-half flip on Wasm/canvas`, async ({ page }) => {
    const errors = []; page.on('pageerror', e => errors.push(e.message));
    await page.addInitScript(() => {
      const instantiate = WebAssembly.instantiate;
      WebAssembly.instantiate = async (...args) => {
        const r = await instantiate(...args); window.testCore = r.instance.exports; return r;
      };
    });
    await page.goto('/');
    await expect(page.locator('#status')).toContainText('Open a local');
    const buffer = readFileSync(new URL(`../../.cache/obj-sizes/obj-sizes-${selector}.sfc`, import.meta.url));
    await page.locator('#rom').setInputFiles({ name:`obj-sizes-${selector}.sfc`, mimeType:'application/octet-stream', buffer });
    await expect(page.locator('#status')).toContainText('loaded');
    // Literal crops of the original cartridge image, never PPU helper output.
    const vectors = [
      [24,24,1], [25,24,4], [31,31,1], [32,32,7],
      [24,39,2], [24,40,5], [39,55,4], [24,56,0], [40,40,0],
      [136,24,2], [136,39,1], [136,40,6], [136,55,5],
      [80,55,6], [176,24,6], [176,55,1],
      [23,40,0], [79,40,0], [112,40,0], [208,40,0],
      ...(selector === 6
        ? [[80,56,2], [80,87,7], [80,88,0], [176,56,7], [176,87,2], [176,88,0]]
        : [[80,56,0], [176,56,0]]),
    ];
    const expected = vectors.map(([,,c]) => colors[c]);
    await page.locator('#play').click();
    await expect.poll(() => page.evaluate(points => {
      const c = window.testCore, canvas = document.querySelector('canvas');
      const bytes = new Uint8Array(c.memory.buffer, c.framebuffer(), c.framebuffer_len());
      const ctx = canvas.getContext('2d');
      return {
        marker:c.peek(0x30), selector:c.peek(0x31),
        geometry:[c.framebuffer_width(),c.framebuffer_height(),c.framebuffer_len(),canvas.width,canvas.height],
        pixels:points.map(([x,y]) => [...bytes.slice((y*256+x)*4,(y*256+x+1)*4)]),
        canvas:points.map(([x,y]) => [...ctx.getImageData(x,y,1,1).data]),
      };
    }, vectors)).toEqual({ marker:0x55, selector, geometry:[256,224,256*224*4,256,224], pixels:expected, canvas:expected });
    await page.locator('#play').click();
    expect(errors).toEqual([]);
    await expect(page.locator('#error')).toBeEmpty();
  });
}
