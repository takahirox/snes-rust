// Original MIT CPU guest and literal Mode 7 scroll observations for Issue #59.
import { test, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../../', import.meta.url));
execFileSync('cargo', ['run', '--quiet', '--locked', '-p', 'snes-core', '--example', 'mode7-scroll-fixture', '--', '.cache/mode7-scroll'], { cwd: root });
const red = [255,0,0,255], green = [0,255,0,255], blue = [0,0,255,255];
const white = [255,255,255,255], yellow = [255,255,0,255];
const cyan = [0,255,255,255], magenta = [255,0,255,255];
const vectors = [[0,red],[1,blue],[2,white],[3,yellow],[48,green],[49,yellow],[50,cyan],[51,magenta],[255,blue]];
test('Mode 7 scroll: CPU shared latch results and independent offsets in Wasm/canvas', async ({ page }) => {
  const errors = []; page.on('pageerror', e => errors.push(e.message));
  await page.addInitScript(() => {
    const instantiate = WebAssembly.instantiate;
    WebAssembly.instantiate = async (...args) => {
      const r = await instantiate(...args); window.testCore = r.instance.exports; return r;
    };
  });
  await page.goto('/');
  await expect(page.locator('#status')).toContainText('Open a local');
  const buffer = readFileSync(new URL('../../.cache/mode7-scroll/mode7-scroll.sfc', import.meta.url));
  await page.locator('#rom').setInputFiles({ name:'mode7-scroll.sfc', mimeType:'application/octet-stream', buffer });
  await expect(page.locator('#status')).toContainText('loaded');
  await page.locator('#play').click();
  const results = [
    0x34,0x12,0, 0x34,0x12,0, 0x34,0x12,0, 0x34,0x12,0,
    0x34,0x12,0, 0x34,0x12,0, 0x56,0x12,0, 0x34,0x92,0xff,
    0x34,0x12,0, 0x34,0x12,0, 0x34,0x12,0, 0x34,0x12,0,
    0x34,0x12,0, 0x34,0x12,0,
  ];
  // Hosts load, advance and observe only. Polling is bounded to five seconds;
  // the overall test deadline is 30 seconds. No public debug API is added.
  await expect.poll(() => page.evaluate(points => {
    const c = window.testCore, canvas = document.querySelector('canvas');
    const bytes = new Uint8Array(c.memory.buffer, c.framebuffer(), c.framebuffer_len());
    const ctx = canvas.getContext('2d');
    const pixels = [], drawn = [];
    for (const y of [0,24,223]) {
      for (const [x] of points) {
        pixels.push([...bytes.slice((y*256+x)*4,(y*256+x+1)*4)]);
        drawn.push([...ctx.getImageData(x,y,1,1).data]);
      }
    }
    return {
      marker:c.peek(0x30), pass:c.peek(0x31),
      results:Array.from({length:42}, (_,i) => c.peek(0x1800+i)),
      geometry:[c.framebuffer_width(),c.framebuffer_height(),c.framebuffer_len(),canvas.width,canvas.height],
      pixels, canvas:drawn,
    };
  }, vectors), { timeout:5000 }).toEqual({
    marker:0x59, pass:1, results, geometry:[256,224,229376,256,224],
    pixels:[...vectors,...vectors,...vectors].map(([,color]) => color),
    canvas:[...vectors,...vectors,...vectors].map(([,color]) => color),
  });
  await page.locator('#play').click();
  expect(errors).toEqual([]);
  await expect(page.locator('#error')).toBeEmpty();
});
