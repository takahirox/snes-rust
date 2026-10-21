import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
const fixture = readFileSync(new URL('../../tests/roms/smoke.bin', import.meta.url));
async function load(page, buffer = fixture, name = 'smoke.sfc') {
  await page.locator('#rom').setInputFiles({ name, mimeType: 'application/octet-stream', buffer });
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const instantiate = WebAssembly.instantiate;
    WebAssembly.instantiate = async (...args) => { const result = await instantiate(...args); window.testCore = result.instance.exports; return result; };
    const original = AudioBufferSourceNode.prototype.start;
    AudioBufferSourceNode.prototype.start = function (...args) {
      const data = this.buffer?.getChannelData(0);
      if (data?.some((v) => v !== 0)) window.nonSilentBuffers = (window.nonSilentBuffers ?? 0) + 1;
      return original.apply(this, args);
    };
    Object.defineProperty(navigator, 'getGamepads', { value: () => window.testPads ?? [] });
  });
  await page.goto('/');
  await expect(page.locator('#status')).toContainText('Open a local');
});
test('real wasm renders frames, reads keyboard/gamepad, produces audio and pauses/resets', async ({ page }) => {
  const errors = []; page.on('pageerror', (e) => errors.push(e.message));
  const uploads = []; page.on('request', (r) => { if (r.method() !== 'GET') uploads.push(r.url()); });
  await load(page); await page.locator('#play').click();
  const marker = () => page.evaluate(() => new Uint8Array(testCore.memory.buffer, testCore.framebuffer(), 256 * 224 * 4).some((v, i) => i % 4 === 0 && v > 0));
  await expect.poll(marker).toBe(true);
  await page.keyboard.down('ArrowRight');
  await expect.poll(() => page.evaluate(() => testCore.peek(0x11))).toBe(1);
  const before = await page.locator('canvas').evaluate((c) => c.toDataURL());
  await expect.poll(() => page.locator('canvas').evaluate((c) => c.toDataURL())).not.toBe(before);
  await page.keyboard.up('ArrowRight');
  await expect.poll(() => page.evaluate(() => testCore.peek(0x11))).toBe(0);
  await expect.poll(() => page.evaluate(() => window.nonSilentBuffers ?? 0)).toBeGreaterThan(0);
  await page.evaluate(() => { window.testPads = [{ mapping: 'standard', buttons: Array.from({ length: 16 }, (_, i) => ({ pressed: i === 0 || i === 15 })), axes: [1, 0] }]; });
  await expect.poll(() => page.evaluate(() => testCore.peek(0x11))).toBe(0x81);
  await page.locator('#play').click(); await expect(page.locator('#status')).toHaveText('Paused');
  const paused = await page.locator('canvas').evaluate((c) => c.toDataURL());
  await page.waitForTimeout(100); expect(await page.locator('canvas').evaluate((c) => c.toDataURL())).toBe(paused);
  await page.locator('#scale').selectOption('3'); await expect(page.locator('canvas')).toHaveCSS('width', '768px');
  await page.locator('#reset').click(); await expect(page.locator('#status')).toContainText('SRAM preserved');
  expect(errors).toEqual([]); expect(uploads).toEqual([]); await expect(page.locator('#error')).toBeEmpty();
});
test('SRAM survives reset, page reload, and copier-header ROM identity', async ({ page }) => {
  await load(page);
  await page.evaluate(() => { new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3] = 0x79; });
  await page.locator('#reset').click();
  expect(await page.evaluate(() => new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3])).toBe(0x79);
  await page.reload(); await expect(page.locator('#status')).toContainText('Open a local');
  await load(page, Buffer.concat([Buffer.alloc(512), fixture]), 'with-header.smc');
  expect(await page.evaluate(() => new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3])).toBe(0x79);
});
test('invalid cartridges report errors without crashing the browser', async ({ page }) => {
  await load(page, Buffer.from([1, 2, 3]), 'bad.sfc');
  await expect(page.locator('#error')).toContainText('Expected a 32 KiB');
  await expect(page.locator('#play')).toBeDisabled();
});
