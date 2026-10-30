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
  await page.getByText('Help / Controls', { exact: true }).click();
  await page.locator('#scale').selectOption('3'); await expect(page.locator('canvas')).toHaveCSS('width', '768px');
  await page.getByText('Help / Controls', { exact: true }).click();
  await page.locator('#reset').click(); await expect(page.locator('#status')).toContainText('SRAM preserved');
  expect(errors).toEqual([]); expect(uploads).toEqual([]); await expect(page.locator('#error')).toBeEmpty();
});
for (const control of ['sound', 'scale']) test(`keyboard releases held buttons after focus moves to ${control}`, async ({ page }) => {
  await load(page); await page.locator('#play').click();
  await page.keyboard.down('ArrowRight');
  await expect.poll(() => page.evaluate(() => testCore.peek(0x11))).toBe(1);
  if (control === 'sound') await page.locator('#sound').click();
  else { await page.getByText('Help / Controls', { exact: true }).click(); await page.locator('#scale').focus(); }
  await expect(page.locator(`#${control}`)).toBeFocused();
  await page.keyboard.up('ArrowRight');
  await expect.poll(() => page.evaluate(() => testCore.peek(0x11))).toBe(0);
  // Controls still receive new key presses without sending them to the game.
  await page.keyboard.down('ArrowRight');
  await page.waitForTimeout(100);
  expect(await page.evaluate(() => testCore.peek(0x11))).toBe(0);
  await page.keyboard.up('ArrowRight');
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
test('failed SRAM writes block cartridge changes until the save can be recovered', async ({ page }) => {
  await load(page); await expect(page.locator('#status')).toContainText('smoke.sfc loaded');
  await page.evaluate(() => {
    new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3] = 0x79;
    window.originalSetItem = Storage.prototype.setItem;
    Storage.prototype.setItem = () => { throw new DOMException('Storage full', 'QuotaExceededError'); };
  });
  const other = Buffer.from(fixture); other[0x7fc0] ^= 1;
  await load(page, other, 'other.sfc');
  await expect(page.locator('#error')).toContainText('Could not persist SRAM');
  await expect(page.locator('#status')).toHaveText('Paused');
  expect(await page.evaluate(() => new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3])).toBe(0x79);
  await expect(page.locator('#rom')).toBeEnabled();
  // Reopening even the same image must not replace its unsaved machine.
  await load(page); await expect(page.locator('#error')).toContainText('Could not persist SRAM');
  expect(await page.evaluate(() => new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3])).toBe(0x79);
  await page.evaluate(() => { Storage.prototype.setItem = window.originalSetItem; });
  await load(page, other, 'other.sfc');
  await expect(page.locator('#status')).toContainText('other.sfc loaded');
  await expect(page.locator('#error')).toBeEmpty();
  expect(await page.evaluate(() => new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3])).toBe(0);
  await page.reload(); await expect(page.locator('#status')).toContainText('Open a local');
  await load(page); await expect(page.locator('#status')).toContainText('smoke.sfc loaded');
  expect(await page.evaluate(() => new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3])).toBe(0x79);
  // Storage can become unavailable during asynchronous file reading/hashing,
  // after the initial save succeeded but before the machine is replaced.
  await page.evaluate(() => {
    const digest = crypto.subtle.digest.bind(crypto.subtle);
    crypto.subtle.digest = async (...args) => { await new Promise((resolve) => { window.releaseHash = resolve; }); return digest(...args); };
    window.originalSetItem = Storage.prototype.setItem;
  });
  await load(page, other, 'other.sfc');
  await expect.poll(() => page.evaluate(() => typeof window.releaseHash)).toBe('function');
  await page.evaluate(() => {
    new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3] = 0x48;
    Storage.prototype.setItem = () => { throw new DOMException('Storage full', 'QuotaExceededError'); };
    window.releaseHash();
  });
  await expect(page.locator('#error')).toContainText('Could not persist SRAM');
  await expect(page.locator('#rom')).toBeEnabled();
  expect(await page.evaluate(() => new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3])).toBe(0x48);
  await page.evaluate(() => { Storage.prototype.setItem = window.originalSetItem; });
  await page.locator('#reset').click(); await expect(page.locator('#error')).toBeEmpty();
  await page.reload(); await expect(page.locator('#status')).toContainText('Open a local');
  await load(page); await expect(page.locator('#status')).toContainText('loaded');
  expect(await page.evaluate(() => new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3])).toBe(0x48);
});
test('reset and Play keep SRAM persistence failures visible and retain the save', async ({ page }) => {
  await load(page); await expect(page.locator('#status')).toContainText('loaded');
  await page.evaluate(() => {
    new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3] = 0x79;
    window.originalSetItem = Storage.prototype.setItem;
    Storage.prototype.setItem = () => { throw new DOMException('Storage unavailable', 'SecurityError'); };
  });
  await page.locator('#reset').click();
  await expect(page.locator('#status')).toContainText('SRAM preserved');
  await expect(page.locator('#error')).toContainText('Could not persist SRAM');
  expect(await page.evaluate(() => new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3])).toBe(0x79);
  await page.locator('#play').click();
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('#error')).toContainText('Could not persist SRAM');
  await page.locator('#play').click();
  await expect(page.locator('#error')).toContainText('Could not persist SRAM');
  await page.evaluate(() => { Storage.prototype.setItem = window.originalSetItem; });
  await page.locator('#reset').click(); await expect(page.locator('#error')).toBeEmpty();
  await page.reload(); await expect(page.locator('#status')).toContainText('Open a local');
  await load(page); await expect(page.locator('#status')).toContainText('loaded');
  expect(await page.evaluate(() => new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3])).toBe(0x79);
});
test('invalid cartridges report errors without crashing the browser', async ({ page }) => {
  await load(page, Buffer.from([1, 2, 3]), 'bad.sfc');
  await expect(page.locator('#error')).toContainText('Expected a 32 KiB');
  await expect(page.locator('#play')).toBeDisabled();
});
for (const failure of ['read', 'base64', 'size']) {
  test(`failed SRAM restoration (${failure}) preserves persisted bytes until retry`, async ({ page }) => {
    await load(page); await expect(page.locator('#status')).toContainText('loaded');
    await page.evaluate(() => { new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3] = 0x79; });
    await page.locator('#reset').click();
    await page.reload(); await expect(page.locator('#status')).toContainText('Open a local');
    await page.evaluate((failure) => {
      window.saveKey = Object.keys(localStorage).find((k) => k.startsWith('snes-rust:sram:'));
      window.validSave = localStorage.getItem(window.saveKey);
      if (failure === 'base64') localStorage.setItem(window.saveKey, '%invalid%');
      if (failure === 'size') localStorage.setItem(window.saveKey, btoa('short'));
      window.persistedSave = localStorage.getItem(window.saveKey);
      window.originalGetItem = Storage.prototype.getItem;
      if (failure === 'read') Storage.prototype.getItem = () => { throw new DOMException('Storage unavailable', 'SecurityError'); };
    }, failure);
    await load(page); await expect(page.locator('#error')).toContainText('Save could not be restored');
    await expect(page.locator('#play')).toBeDisabled(); await expect(page.locator('#reset')).toBeDisabled();
    // Restoring access alone must not enable periodic/page-exit/reset writes.
    await page.evaluate(() => {
      Storage.prototype.getItem = window.originalGetItem;
      document.getElementById('reset').dispatchEvent(new Event('click'));
      window.dispatchEvent(new Event('pagehide'));
    });
    const other = Buffer.from(fixture); other[0x7fc0] ^= 1;
    await load(page, other, 'other.sfc');
    await expect(page.locator('#error')).toContainText('Save could not be restored');
    expect(await page.evaluate(() => localStorage.getItem(window.saveKey))).toBe(await page.evaluate(() => window.persistedSave));
    if (failure !== 'read') {
      await page.locator('#retry-save').click();
      await expect(page.locator('#play')).toBeDisabled();
      await page.evaluate(() => localStorage.setItem(window.saveKey, window.validSave));
    }
    await page.locator('#retry-save').click();
    await expect(page.locator('#error')).toBeEmpty(); await expect(page.locator('#play')).toBeEnabled();
    expect(await page.evaluate(() => new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3])).toBe(0x79);
    await page.locator('#reset').click();
    await page.reload(); await expect(page.locator('#status')).toContainText('Open a local');
    await load(page); await expect(page.locator('#status')).toContainText('loaded');
    expect(await page.evaluate(() => new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3])).toBe(0x79);
  });
}
