// Original MIT CPU-built SETINI guests and actual Wasm/canvas checks for #53.
import { test, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../../', import.meta.url));
execFileSync('cargo', ['run', '--quiet', '--locked', '-p', 'snes-core', '--example', 'pseudo-hires-fixture', '--', '.cache/pseudo-hires'], { cwd: root });
const red = [255,0,0,255], green = [0,255,0,255], blue = [0,0,255,255], white = [255,255,255,255];
async function upload(page, name) {
  const buffer = readFileSync(new URL(`../../.cache/pseudo-hires/pseudo-hires-${name}.sfc`, import.meta.url));
  await page.locator('#rom').setInputFiles({ name: `pseudo-hires-${name}.sfc`, mimeType: 'application/octet-stream', buffer });
  await expect(page.locator('#status')).toHaveText('Playing');
  await page.locator('#reset').click(); // Pause at a fresh frame origin for hardware/replay assertions.
}
async function snapshot(page) {
  return page.evaluate(() => {
    const c = window.testCore, canvas = document.querySelector('canvas');
    const width = c.framebuffer_width(), height = c.framebuffer_height(), len = c.framebuffer_len();
    // Reacquire both pointer and view after every possible memory/geometry change.
    const bytes = new Uint8Array(c.memory.buffer, c.framebuffer(), len);
    const ctx = canvas.getContext('2d'), size = canvas.getBoundingClientRect();
    const points = [0,1,2,3,width-2,width-1];
    const rows = [0,height-1].map(y => ({
      pixels: points.map(x => [...bytes.slice((y*width+x)*4,(y*width+x+1)*4)]),
      canvas: points.map(x => [...ctx.getImageData(x,y,1,1).data]),
    }));
    return { width, height, len, canvasWidth: canvas.width, canvasHeight: canvas.height,
      shape: size.width / size.height, rows, marker: c.peek(0x30), setini: c.peek(0x31) };
  });
}
async function checkFrame(page, enabled) {
  const width = enabled ? 512 : 256;
  const pixels = enabled ? [blue,red,white,green,white,green] : [red,green,[0,255,255,255],[0,0,0,255],red,green];
  await expect.poll(async () => {
    const s = await snapshot(page);
    return { width:s.width, setini:s.setini, rows:s.rows };
  }).toEqual({ width, setini:enabled ? 8 : 0, rows:[{pixels,canvas:pixels},{pixels,canvas:pixels}] });
  const s = await snapshot(page);
  expect([s.width,s.height,s.len,s.canvasWidth,s.canvasHeight]).toEqual([width,224,width*224*4,width,224]);
  expect(s.marker).toBe(0x53); expect(s.shape).toBeCloseTo(256/224,3);
}
test('SETINI: real canvas/Wasm phases, 256→512→256, reset and replacement', async ({ page }) => {
  const errors = []; page.on('pageerror',e => errors.push(e.message));
  await page.addInitScript(() => {
    const instantiate = WebAssembly.instantiate;
    WebAssembly.instantiate = async (...args) => { const r = await instantiate(...args); window.testCore = r.instance.exports; return r; };
  });
  await page.goto('/'); await expect(page.locator('#status')).toHaveText('Ready');
  await upload(page,'switch'); await page.locator('#play').click(); await checkFrame(page,false);
  await page.keyboard.down('KeyX'); // Guest changes only SETINI, in VBlank after joypad completion.
  await checkFrame(page,true);
  await page.evaluate(() => testCore.memory.grow(1)); await checkFrame(page,true);
  await page.getByText('Help / Controls', { exact: true }).click();
  await page.locator('#scale').selectOption('3');
  await expect(page.locator('canvas')).toHaveCSS('width','768px');
  await page.getByText('Help / Controls', { exact: true }).click();
  expect((await snapshot(page)).shape).toBeCloseTo(256/224,3);
  await page.keyboard.up('KeyX'); await checkFrame(page,false);
  await page.keyboard.down('KeyX'); await checkFrame(page,true);
  await page.locator('#reset').click(); await page.keyboard.up('KeyX');
  const s = await snapshot(page);
  expect([s.width,s.height,s.len,s.canvasWidth,s.canvasHeight]).toEqual([256,224,256*224*4,256,224]);
  expect(s.rows[0].pixels[0]).toEqual([0,0,0,0]); expect(s.rows[0].canvas[0]).toEqual([0,0,0,0]);
  await page.locator('#play').click(); await checkFrame(page,false);
  await upload(page,'on'); await page.locator('#play').click(); await checkFrame(page,true);
  await upload(page,'off'); await page.locator('#play').click(); await checkFrame(page,false);
  await page.locator('#play').click();
  expect(errors).toEqual([]); await expect(page.locator('#error')).toBeEmpty();
});
