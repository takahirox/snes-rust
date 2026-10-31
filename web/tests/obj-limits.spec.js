// Original MIT CPU guests and literal OBJ survivors for Issue #57.
import { resetAndPause } from './playback.js';
import { test, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../../', import.meta.url));
execFileSync('cargo', ['run', '--quiet', '--locked', '-p', 'snes-core', '--example', 'obj-limits-fixture', '--', '.cache/obj-limits'], { cwd: root });
const black = [0,0,0,255], red = [255,0,0,255], green = [0,255,0,255];
const vectors = [
  [[23,black],[24,red],[31,red],[32,black],[200,black]],
  [[0,black],[200,green],[207,green],[208,black],[216,black]],
  [[24,black],[64,red],[71,red],[72,green],[79,green],[80,black],[128,red]],
  [[24,black],[64,red],[200,red],[207,red],[208,green],[215,green],[216,black],[232,black]],
];
for (const mode of [0,1,2,3]) {
  test(`OBJ limits: mode ${mode} CPU STAT77 lifetime and admitted Wasm/canvas tiles`, async ({ page }) => {
    const errors = []; page.on('pageerror', e => errors.push(e.message));
    await page.addInitScript(() => {
      const instantiate = WebAssembly.instantiate;
      WebAssembly.instantiate = async (...args) => {
        const r = await instantiate(...args); window.testCore = r.instance.exports; return r;
      };
    });
    await page.goto('/');
    await expect(page.locator('#status')).toHaveText('Ready');
    const buffer = readFileSync(new URL(`../../.cache/obj-limits/obj-limits-${mode}.sfc`, import.meta.url));
    await page.locator('#rom').setInputFiles({ name:`obj-limits-${mode}.sfc`, mimeType:'application/octet-stream', buffer });
    await expect(page.locator('#status')).toHaveText('Playing');
    await resetAndPause(page); // Fresh frame origin for hardware/replay assertions.
    await page.locator('#play').click();
    const flags = [0,0x40,0x80,0xc0][mode];
    const expected = vectors[mode].map(([,color]) => color);
    // Five-second polling bound and test's 30-second deadline. Guest loops have
    // instruction/frame bounds in Rust; here the host only loads and observes.
    await expect.poll(() => page.evaluate(points => {
      const c = window.testCore, canvas = document.querySelector('canvas');
      const bytes = new Uint8Array(c.memory.buffer, c.framebuffer(), c.framebuffer_len());
      const ctx = canvas.getContext('2d');
      return {
        marker:c.peek(0x30), pass:c.peek(0x31), mode:c.peek(0x32),
        results:Array.from({length:9}, (_,i) => c.peek(0x1800+i)),
        geometry:[c.framebuffer_width(),c.framebuffer_height(),canvas.width,canvas.height],
        pixels:points.map(([x]) => [...bytes.slice((24*256+x)*4,(24*256+x+1)*4)]),
        canvas:points.map(([x]) => [...ctx.getImageData(x,24,1,1).data]),
      };
    }, vectors[mode]), { timeout:5000 }).toEqual({
      marker:0x57, pass:1, mode, results:[flags,flags,flags,flags,flags,flags,flags,0,0],
      geometry:[256,224,256,224], pixels:expected, canvas:expected,
    });
    await page.locator('#play').click();
    expect(errors).toEqual([]);
    await expect(page.locator('#error')).toBeEmpty();
  });
}
