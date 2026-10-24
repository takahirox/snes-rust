// Original MIT CPU guest and literal EXTBG observations for Issue #61.
import { test, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../../', import.meta.url));
execFileSync('cargo', ['run', '--quiet', '--locked', '-p', 'snes-core', '--example', 'mode7-extbg-fixture', '--', '.cache/mode7-extbg'], { cwd: root });
const vectors = [
  [0,[131,131,131,255]], [1,[255,0,0,255]], [2,[0,255,0,255]],
  [3,[0,0,131,255]], [4,[255,0,0,255]], [5,[0,255,0,255]],
  [6,[32,0,0,255]], [7,[230,230,197,255]],
];
test('Mode 7 EXTBG: CPU-built BG2, priority, transparency and BG1 direct color in Wasm/canvas', async ({ page }) => {
  const errors = []; page.on('pageerror', e => errors.push(e.message));
  page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
  await page.addInitScript(() => {
    const instantiate = WebAssembly.instantiate;
    WebAssembly.instantiate = async (...args) => {
      const result = await instantiate(...args); window.testCore = result.instance.exports; return result;
    };
  });
  await page.goto('/');
  await expect(page.locator('#status')).toContainText('Open a local');
  const buffer = readFileSync(new URL('../../.cache/mode7-extbg/mode7-extbg.sfc', import.meta.url));
  await page.locator('#rom').setInputFiles({ name:'mode7-extbg.sfc', mimeType:'application/octet-stream', buffer });
  await expect(page.locator('#status')).toContainText('loaded');
  await page.locator('#play').click();
  // Poll real framebuffer/canvas observations; hosts never write emulator state.
  // Three observations separated by animation callbacks check stable displayed output.
  for (let observation = 0; observation < 3; observation++) {
    await expect.poll(() => page.evaluate(points => {
      const c = window.testCore, canvas = document.querySelector('canvas');
      const bytes = new Uint8Array(c.memory.buffer, c.framebuffer(), c.framebuffer_len());
      const ctx = canvas.getContext('2d');
      const pixels = [], drawn = [];
      for (const y of [24,80,160,222,223]) for (const [x] of points) {
        pixels.push([...bytes.slice((y*256+x)*4,(y*256+x+1)*4)]);
        drawn.push([...ctx.getImageData(x,y,1,1).data]);
      }
      return { marker:c.peek(0x30),
        geometry:[c.framebuffer_width(),c.framebuffer_height(),c.framebuffer_len(),canvas.width,canvas.height],
        pixels, canvas:drawn,
      };
    }, vectors), { timeout:5000 }).toEqual({
      marker:0x61, geometry:[256,224,229376,256,224],
      pixels:[...vectors,...vectors,...vectors,...vectors].map(([,color]) => color).concat(vectors.map(() => [131,131,131,255])),
      canvas:[...vectors,...vectors,...vectors,...vectors].map(([,color]) => color).concat(vectors.map(() => [131,131,131,255])),
    });
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  }
  await page.locator('#play').click();
  expect(errors).toEqual([]);
  await expect(page.locator('#error')).toBeEmpty();
});
