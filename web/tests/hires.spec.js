// Original MIT CPU-built Mode 5/6 fixtures and output checks for Issue #49.
import { test, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../../', import.meta.url));
execFileSync('cargo', ['run', '--quiet', '--locked', '-p', 'snes-core', '--example', 'hires-fixture', '--', '.cache/hires'], { cwd: root });
const fixture = (mode, suffix = '') => readFileSync(new URL(`../../.cache/hires/hires-${mode}${suffix}.sfc`, import.meta.url));
const green = [0,255,0,255], blue = [0,0,255,255];
async function upload(page, mode, suffix = '') {
  await page.locator('#rom').setInputFiles({ name: `hires-${mode}${suffix}.sfc`, mimeType: 'application/octet-stream', buffer: fixture(mode, suffix) });
  await expect(page.locator('#status')).toHaveText('Playing');
  await page.locator('#reset').click(); // Pause at a fresh frame origin for hardware/replay assertions.
}
async function snapshot(page) {
  return page.evaluate(() => {
    const c = window.testCore, canvas = document.querySelector('canvas');
    const width = c.framebuffer_width(), height = c.framebuffer_height(), len = c.framebuffer_len(), ptr = c.framebuffer();
    const bytes = new Uint8Array(c.memory.buffer, ptr, len);
    const ctx = canvas.getContext('2d');
    const size = canvas.getBoundingClientRect();
    return { width, height, len, canvasWidth: canvas.width, canvasHeight: canvas.height,
      shape: size.width / size.height,
      first: [...bytes.slice(0,4)], second: [...bytes.slice(4,8)], last: [...bytes.slice(width*4-4,width*4)],
      bottomLast: [...bytes.slice(-4)],
      canvasFirst: [...ctx.getImageData(0,0,1,1).data],
      canvasSecond: [...ctx.getImageData(1,0,1,1).data],
      canvasLast: [...ctx.getImageData(width-1,0,1,1).data],
      canvasBottomLast: [...ctx.getImageData(width-1,height-1,1,1).data],
      marker: c.peek(0x30) };
  });
}
async function checkFrame(page, width, mode) {
  await expect.poll(async () => (await snapshot(page)).width).toBe(width);
  await expect.poll(async () => (await snapshot(page)).second).toEqual(green);
  const s = await snapshot(page);
  expect(s.height).toBe(224); expect(s.len).toBe(width*224*4);
  expect([s.canvasWidth,s.canvasHeight]).toEqual([width,224]);
  expect(s.marker).toBe(0x49);
  expect(s.shape).toBeCloseTo(256/224, 3);
  expect(s.first).toEqual(width === 512 && mode === 5 ? [255,255,0,255] : [255,0,0,255]);
  expect(s.last).toEqual(width === 512 ? blue : green);
  expect(s.bottomLast).toEqual(s.last);
  expect(s.canvasFirst).toEqual(s.first); expect(s.canvasSecond).toEqual(s.second);
  expect(s.canvasLast).toEqual(s.last); expect(s.canvasBottomLast).toEqual(s.last);
}
for (const mode of [5,6]) test(`Mode ${mode}: real canvas/Wasm 256→512→256, reset and replacement`, async ({ page }) => {
  const errors=[]; page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(() => {
    const instantiate = WebAssembly.instantiate;
    WebAssembly.instantiate = async (...args) => { const r=await instantiate(...args); window.testCore=r.instance.exports; return r; };
  });
  await page.goto('/'); await expect(page.locator('#status')).toHaveText('Ready');
  await upload(page,mode,'-switch'); await page.locator('#play').click();
  await checkFrame(page,256,mode);
  await page.keyboard.down('KeyX'); // A: guest switches BGMODE during VBlank.
  await checkFrame(page,512,mode);
  // Detach old JS views explicitly; subsequent draws must reacquire Wasm memory.
  await page.evaluate(() => testCore.memory.grow(1));
  await checkFrame(page,512,mode);
  await page.getByText('Help / Controls', { exact: true }).click();
  await page.locator('#scale').selectOption('3');
  await expect(page.locator('canvas')).toHaveCSS('width','768px');
  expect((await snapshot(page)).shape).toBeCloseTo(256/224,3);
  await page.keyboard.up('KeyX'); await checkFrame(page,256,mode);
  await page.locator('#scale').selectOption('2');
  await page.getByText('Help / Controls', { exact: true }).click();
  await page.keyboard.down('KeyX'); await checkFrame(page,512,mode);
  await page.locator('#reset').click(); await page.keyboard.up('KeyX');
  let s=await snapshot(page); expect([s.width,s.height,s.len,s.canvasWidth]).toEqual([256,224,256*224*4,256]);
  expect(s.first).toEqual([0,0,0,0]); expect(s.canvasFirst).toEqual([0,0,0,0]);
  await page.locator('#play').click(); await checkFrame(page,256,mode);
  await upload(page,6); await page.locator('#play').click(); await checkFrame(page,512,6);
  await upload(page,1); await page.locator('#play').click(); await checkFrame(page,256,1);
  await page.locator('#play').click();
  expect(errors).toEqual([]); await expect(page.locator('#error')).toBeEmpty();
});
