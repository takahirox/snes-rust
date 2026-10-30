import { selectDemo } from './cartridge.js';
import { test, expect } from '@playwright/test';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { lstatSync, readFileSync, readdirSync } from 'node:fs';

const dist = new URL('../dist/', import.meta.url);
const assets = { 'index.html': 'text/html', 'app.js': 'text/javascript', 'style.css': 'text/css', 'snes_wasm.wasm': 'application/wasm', 'shionari.sfc': 'application/octet-stream' };
const demoHash = '1132c59cfba4cc7b0c1a7d7957b4b5de84c6038e0e061ca2e6372f8b8b4a01ca';
const saveKey = `snes-rust:sram:${demoHash}`;
const fixture = readFileSync(new URL('../../tests/roms/smoke.bin', import.meta.url));
const prefix = '/snes-rust/';
let server, origin;

test.beforeAll(async () => {
  // Inspect the directory uploaded by Actions, including hidden/unexpected files.
  expect(readdirSync(dist).sort()).toEqual(Object.keys(assets).sort());
  for (const name of Object.keys(assets)) {
    const stat = lstatSync(new URL(name, dist));
    expect(stat.isFile(), name).toBe(true); // Reject symlinks and directories.
    expect(stat.size, name).toBeGreaterThan(0);
  }
  const rom = readFileSync(new URL('shionari.sfc', dist));
  expect(rom.length).toBe(524288);
  expect(createHash('sha256').update(rom).digest('hex')).toBe(demoHash);
  server = createServer((request, response) => {
    const path = new URL(request.url, 'http://localhost').pathname;
    const name = path === prefix ? 'index.html' : path.slice(prefix.length);
    if (request.method !== 'GET' || !path.startsWith(prefix) || !Object.hasOwn(assets, name)) {
      response.writeHead(404).end(); return;
    }
    response.writeHead(200, { 'Content-Type': assets[name] });
    response.end(readFileSync(new URL(name, dist)));
  });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  origin = `http://127.0.0.1:${server.address().port}`;
});
test.afterAll(async () => {
  if (server) await new Promise((resolve) => server.close(resolve));
});

test('Pages artifact boots and plays a local diagnostic under /snes-rust/', async ({ page }) => {
  const errors = [], requests = [], responses = [];
  page.on('pageerror', (error) => errors.push(error.message));
  page.on('console', (message) => { if (message.type() === 'error') errors.push(message.text()); });
  page.on('requestfailed', (request) => errors.push(`${request.url()}: ${request.failure()?.errorText}`));
  page.on('request', (request) => requests.push({ url: request.url(), method: request.method() }));
  page.on('response', (response) => responses.push({ url: response.url(), status: response.status(), type: response.headers()['content-type'] }));
  await page.addInitScript(() => {
    const instantiate = WebAssembly.instantiate;
    WebAssembly.instantiate = async (...args) => {
      const result = await instantiate(...args);
      window.pagesCore = result.instance.exports;
      return result;
    };
  });
  await page.goto(`${origin}${prefix}`);
  await expect(page.locator('#status')).toHaveText('Ready');
  await expect(page.locator('#rom')).toBeEnabled();
  await expect(page.locator('canvas')).toHaveCSS('image-rendering', 'pixelated');
  await loadDemo(page);
  // An original MIT diagnostic is selected locally, never copied into dist.
  await page.locator('#rom').setInputFiles({
    name: 'smoke.sfc', mimeType: 'application/octet-stream',
    buffer: readFileSync(new URL('../../tests/roms/smoke.bin', import.meta.url)),
  });
  await expect(page.locator('#status')).toHaveText('Playing');
  await page.keyboard.down('ArrowRight');
  await expect.poll(() => page.evaluate(() => pagesCore.peek(0x11))).toBe(1);
  await page.keyboard.up('ArrowRight');
  await expect.poll(() => page.evaluate(() => new Uint8Array(pagesCore.memory.buffer, pagesCore.framebuffer(), 256 * 224 * 4).some((value, index) => index % 4 === 0 && value > 0))).toBe(true);
  await page.locator('#play').click();
  await expect(page.locator('#status')).toHaveText('Paused');
  await expect(page.locator('#error')).toBeEmpty();
  expect(errors).toEqual([]);
  const expectedPaths = Object.keys(assets).map((name) => `${origin}${prefix}${name === 'index.html' ? '' : name}`).sort();
  expect(requests.map((request) => request.url).sort()).toEqual(expectedPaths);
  expect(requests.every((request) => request.method === 'GET')).toBe(true);
  expect(responses.map((response) => response.url).sort()).toEqual(expectedPaths);
  for (const response of responses) {
    expect(response.status, response.url).toBe(200);
    const name = response.url.endsWith(prefix) ? 'index.html' : response.url.split('/').pop();
    expect(response.type).toBe(assets[name]);
  }
});

async function loadDemo(page) {
  await selectDemo(page);
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('#play')).toBeEnabled();
  expect(await page.evaluate(() => [pagesCore.battery(), pagesCore.sram_len()])).toEqual([1, 8192]);
}
async function localRom(page, buffer = fixture) {
  await page.locator('#rom').setInputFiles({ name: 'local.sfc', mimeType: 'application/octet-stream', buffer });
  await expect(page.locator('#status')).toHaveText('Playing');
}
async function boot(page) {
  await page.addInitScript(() => {
    const instantiate = WebAssembly.instantiate;
    WebAssembly.instantiate = async (...args) => {
      const result = await instantiate(...args);
      window.pagesCore = result.instance.exports;
      return result;
    };
    const OriginalAudioContext = window.AudioContext;
    window.audioActivations = 0;
    window.AudioContext = class extends OriginalAudioContext {
      constructor(...args) { super(...args); window.audioActivations++; }
    };
  });
  await page.goto(`${origin}${prefix}`);
  await expect(page.locator('#demo')).toBeEnabled();
}
const marker = (page) => page.evaluate(() => new Uint8Array(pagesCore.memory.buffer, pagesCore.sram_buffer(), pagesCore.sram_len())[3]);
const setMarker = (page, value) => page.evaluate((value) => { new Uint8Array(pagesCore.memory.buffer, pagesCore.sram_buffer(), pagesCore.sram_len())[3] = value; }, value);
const scene = (page) => page.evaluate(async () => {
  // Background rows exclude dialogue/typewriter text and its blinking cursor.
  const data = new Uint8Array(pagesCore.memory.buffer, pagesCore.framebuffer() + 16 * 256 * 4, 128 * 256 * 4);
  return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', data)), b => b.toString(16).padStart(2, '0')).join('');
});

const lit = (page) => page.evaluate(() => new Uint8Array(pagesCore.memory.buffer, pagesCore.framebuffer() + 16 * 256 * 4, 128 * 256 * 4).some((value, index) => index % 4 !== 3 && value > 0));
async function tapA(page) {
  await page.keyboard.down('KeyX'); // Existing mapping: X = SNES A.
  await page.waitForTimeout(100);
  await page.keyboard.up('KeyX');
  await page.waitForTimeout(300);
}

test('Shionari skips its opening to the title and keyboard A starts the warehouse story', async ({ page }, testInfo) => {
  const errors = [], requests = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
  page.on('requestfailed', request => errors.push(request.url()));
  page.on('response', response => { if (!response.ok()) errors.push(`${response.status()} ${response.url()}`); });
  page.on('request', request => requests.push(request.url()));
  await boot(page);
  expect(await page.evaluate(() => audioActivations)).toBe(0);
  await loadDemo(page);
  await expect(page.locator('#status')).toHaveText('Playing');
  expect(await page.evaluate(() => audioActivations)).toBe(1);
  await page.locator('canvas').click();
  // After a short black boot the opening prologue fades in; A skips it to the title.
  await expect.poll(() => lit(page), { timeout: 10000 }).toBe(true);
  await testInfo.attach('Shionari opening', { body: await page.locator('canvas').screenshot(), contentType: 'image/png' });
  await tapA(page);
  // Wait for the title background to finish fading in.
  await page.waitForTimeout(1500);
  const title = await scene(page);
  await testInfo.attach('Shionari title', { body: await page.locator('canvas').screenshot(), contentType: 'image/png' });
  await page.waitForTimeout(500);
  expect(await scene(page)).toBe(title); // No idle scene transition.
  await tapA(page); // Opens the title menu with New Game selected.
  await page.keyboard.down('KeyX');
  try { await expect.poll(() => scene(page)).not.toBe(title); }
  finally { await page.keyboard.up('KeyX'); }
  await page.waitForTimeout(1000);
  const warehouse = await scene(page);
  expect(warehouse).not.toBe(title);
  await testInfo.attach('Shionari warehouse story', { body: await page.locator('canvas').screenshot(), contentType: 'image/png' });
  await testInfo.attach('scene hashes', { body: JSON.stringify({ title, warehouse }), contentType: 'application/json' });
  await page.locator('#play').click();
  await expect(page.locator('#status')).toHaveText('Paused');
  const paused = await scene(page);
  await page.waitForTimeout(100);
  expect(await scene(page)).toBe(paused);
  await page.locator('#reset').click();
  await expect(page.locator('#status')).toContainText('SRAM preserved');
  await expect(page.locator('#error')).toBeEmpty();
  expect(errors).toEqual([]);
  expect(requests.every(url => url.startsWith(`${origin}${prefix}`))).toBe(true);
  expect(requests.filter(url => url.endsWith('.sfc'))).toEqual([`${origin}${prefix}shionari.sfc`]);
});

test('demo battery SRAM survives reset/reload, shares local identity and stays isolated from another ROM', async ({ page }) => {
  await boot(page); await loadDemo(page);
  await setMarker(page, 0x79);
  await page.locator('#reset').click(); expect(await marker(page)).toBe(0x79);
  await page.reload(); await expect(page.locator('#demo')).toBeEnabled(); await loadDemo(page);
  expect(await marker(page)).toBe(0x79);
  // A local copy with a copier header is the same game/save, not a second key.
  await localRom(page, Buffer.concat([Buffer.alloc(512), readFileSync(new URL('shionari.sfc', dist))]));
  expect(await marker(page)).toBe(0x79);
  await setMarker(page, 0x48);
  await loadDemo(page); expect(await marker(page)).toBe(0x48);
  await loadDemo(page); expect(await marker(page)).toBe(0x48); // Same-demo pause/resume retains SRAM.
  await localRom(page); expect(await marker(page)).toBe(0);
  await setMarker(page, 0x26);
  await loadDemo(page); expect(await marker(page)).toBe(0x48);
  await localRom(page); expect(await marker(page)).toBe(0x26);
  const keys = await page.evaluate(() => Object.keys(localStorage).filter(k => k.startsWith('snes-rust:sram:')).sort());
  expect(keys).toEqual([saveKey, `snes-rust:sram:${createHash('sha256').update(fixture).digest('hex')}`].sort());
});

test('failed saves protect both demo and local cartridges, including same-demo reload', async ({ page }) => {
  await boot(page); await loadDemo(page); await setMarker(page, 0x79);
  await page.evaluate(() => {
    window.originalSetItem = Storage.prototype.setItem;
    Storage.prototype.setItem = () => { throw new DOMException('Storage full', 'QuotaExceededError'); };
  });
  await page.locator('#play').click();
  for (const source of ['demo', 'local']) {
    if (source === 'demo') await selectDemo(page);
    else await page.locator('#rom').setInputFiles({ name: 'local.sfc', mimeType: 'application/octet-stream', buffer: fixture });
    await expect(page.locator('#error')).toContainText('Could not persist SRAM');
    expect(await marker(page)).toBe(0x79);
    expect(await page.evaluate(() => pagesCore.sram_len())).toBe(8192);
  }
  await page.evaluate(() => { Storage.prototype.setItem = window.originalSetItem; });
  await localRom(page); await setMarker(page, 0x26);
  await page.evaluate(() => { Storage.prototype.setItem = () => { throw new DOMException('Storage full', 'QuotaExceededError'); }; });
  await selectDemo(page);
  await expect(page.locator('#error')).toContainText('Could not persist SRAM');
  expect(await marker(page)).toBe(0x26);
  await page.evaluate(() => { Storage.prototype.setItem = window.originalSetItem; });
  await loadDemo(page); expect(await marker(page)).toBe(0x79);
});

for (const failure of ['read', 'base64', 'size']) test(`failed demo restore (${failure}) blocks writes and both sources until retry`, async ({ page }) => {
  await boot(page); await loadDemo(page); await setMarker(page, 0x79);
  await page.locator('#reset').click(); await page.reload();
  await expect(page.locator('#demo')).toBeEnabled();
  await page.evaluate(({ saveKey, failure }) => {
    window.validSave = localStorage.getItem(saveKey);
    if (failure === 'base64') localStorage.setItem(saveKey, '%invalid%');
    if (failure === 'size') localStorage.setItem(saveKey, btoa('short'));
    window.persistedSave = localStorage.getItem(saveKey);
    window.originalGetItem = Storage.prototype.getItem;
    if (failure === 'read') Storage.prototype.getItem = () => { throw new DOMException('Storage unavailable', 'SecurityError'); };
  }, { saveKey, failure });
  await selectDemo(page);
  await expect(page.locator('#error')).toContainText('Save could not be restored');
  await expect(page.locator('#play')).toBeDisabled(); await expect(page.locator('#reset')).toBeDisabled();
  await page.evaluate(() => {
    Storage.prototype.getItem = window.originalGetItem;
    document.getElementById('reset').dispatchEvent(new Event('click'));
    window.dispatchEvent(new Event('pagehide'));
  });
  await selectDemo(page);
  await page.locator('#rom').setInputFiles({ name: 'local.sfc', mimeType: 'application/octet-stream', buffer: fixture });
  await expect(page.locator('#error')).toContainText('Save could not be restored');
  expect(await page.evaluate(saveKey => localStorage.getItem(saveKey), saveKey)).toBe(await page.evaluate(() => persistedSave));
  await page.evaluate(saveKey => localStorage.setItem(saveKey, validSave), saveKey);
  await page.locator('#retry-save').click();
  await expect(page.locator('#error')).toBeEmpty(); expect(await marker(page)).toBe(0x79);
  await expect(page.locator('#play')).toBeEnabled();
  await localRom(page); await loadDemo(page); expect(await marker(page)).toBe(0x79);
});

test('demo download errors and corrupt bytes preserve the previous playable cartridge', async ({ page }) => {
  await boot(page); await localRom(page); await setMarker(page, 0x79);
  for (const response of [
    { status: 404, body: 'Missing', message: 'Demo download failed: HTTP 404' },
    { status: 200, body: fixture, message: 'Demo download does not match the published ROM' },
  ]) {
    await page.route('**/shionari.sfc', route => route.fulfill({ status: response.status, contentType: 'application/octet-stream', body: response.body }));
    await selectDemo(page);
    await expect(page.locator('#error')).toContainText(response.message);
    await expect(page.locator('#status')).toHaveText('Paused');
    await expect(page.locator('#play')).toBeEnabled(); expect(await marker(page)).toBe(0x79);
    await page.unroute('**/shionari.sfc');
  }
  // A valid-sized image with an invalid header is rejected by the real core
  // through the same loader, also preserving the old machine and SRAM.
  await page.locator('#rom').setInputFiles({ name: 'bad.sfc', mimeType: 'application/octet-stream', buffer: Buffer.alloc(32768) });
  await expect(page.locator('#error')).not.toBeEmpty(); expect(await marker(page)).toBe(0x79);
  await page.locator('#play').click(); await expect(page.locator('#status')).toHaveText('Playing');
  await page.keyboard.down('ArrowRight');
  await expect.poll(() => page.evaluate(() => pagesCore.peek(0x11))).toBe(1);
  await page.keyboard.up('ArrowRight');
  await page.locator('#play').click(); await loadDemo(page);
});

test('Pages server rejects unpublished paths and CSP rejects remote ROM connections', async ({ page, request }) => {
  for (const path of ['/shionari.sfc', '/snes-rust/smoke.bin', '/snes-rust/castle.sfc', '/snes-rust/memory.sfc', '/snes-rust/.cache/save.srm', '/snes-rust/demos/shionari.sfc']) {
    expect((await request.get(`${origin}${path}`)).status(), path).toBe(404);
  }
  await boot(page);
  const remote = [];
  page.on('request', request => { if (new URL(request.url()).origin !== origin) remote.push(request.url()); });
  const blocked = await page.evaluate(async () => {
    try { await fetch('https://example.invalid/shionari.sfc'); return false; }
    catch { return true; }
  });
  expect(blocked).toBe(true); expect(remote).toEqual([]);
});

test('demo loading reports progress, accepts binary MIME types and rechecks saving before replacement', async ({ page }) => {
  await boot(page); await localRom(page); await setMarker(page, 0x79);
  let pending;
  await page.route('**/shionari.sfc', route => { pending = route; });
  await selectDemo(page);
  await expect(page.locator('#status')).toHaveText('Loading 潮鳴りの密室 / Shionari no Misshitsu…');
  for (const id of ['demo', 'rom', 'play', 'reset']) await expect(page.locator(`#${id}`)).toBeDisabled();
  await expect.poll(() => Boolean(pending)).toBe(true);
  await page.evaluate(() => {
    new Uint8Array(pagesCore.memory.buffer, pagesCore.sram_buffer(), pagesCore.sram_len())[3] = 0x48;
    window.originalSetItem = Storage.prototype.setItem;
    Storage.prototype.setItem = () => { throw new DOMException('Storage full', 'QuotaExceededError'); };
  });
  await pending.fulfill({ status: 200, contentType: 'application/octet-stream', body: readFileSync(new URL('shionari.sfc', dist)) });
  await expect(page.locator('#error')).toContainText('Could not persist SRAM');
  expect(await marker(page)).toBe(0x48);
  await expect(page.locator('#demo')).toBeEnabled(); await expect(page.locator('#play')).toBeEnabled();
  await page.evaluate(() => { Storage.prototype.setItem = window.originalSetItem; });
  await page.unroute('**/shionari.sfc');
  // Static hosts may advertise either generic binary or the SNES-specific type;
  // arrayBuffer loading must not depend on a JavaScript/Wasm MIME workaround.
  for (const contentType of ['application/octet-stream', 'application/vnd.nintendo.snes']) {
    await page.route('**/shionari.sfc', route => route.fulfill({ status: 200, contentType, body: readFileSync(new URL('shionari.sfc', dist)) }));
    await loadDemo(page);
    await page.unroute('**/shionari.sfc');
  }
  await localRom(page); expect(await marker(page)).toBe(0x48);
});

for (const destination of ['demo', 'local']) for (const failure of ['read', 'base64', 'size']) {
  test(`switching to ${destination}: failed ${failure} restore retains the inserted machine and both saves until retry`, async ({ page }) => {
    await boot(page);
    const localKey = `snes-rust:sram:${createHash('sha256').update(fixture).digest('hex')}`;
    const targetKey = destination === 'demo' ? saveKey : localKey;
    const oldKey = destination === 'demo' ? localKey : saveKey;
    if (destination === 'demo') await localRom(page); else await loadDemo(page);
    await setMarker(page, 0x79);
    const oldName = await page.locator('#current-game').textContent();
    const valid = Buffer.alloc(destination === 'demo' ? 8192 : 1024 << fixture[0x7fd8]);
    valid[3] = 0x48;
    await page.evaluate(({ targetKey, valid, failure }) => {
      window.targetSave = valid;
      localStorage.setItem(targetKey, failure === 'base64' ? '%invalid%' : failure === 'size' ? btoa('short') : valid);
      window.targetBefore = localStorage.getItem(targetKey);
      window.originalGetItem = Storage.prototype.getItem;
      if (failure === 'read') Storage.prototype.getItem = function (key) {
        if (key === targetKey) throw new DOMException('Storage unavailable', 'SecurityError');
        return window.originalGetItem.call(this, key);
      };
    }, { targetKey, valid: valid.toString('base64'), failure });
    if (destination === 'demo') await selectDemo(page);
    else await page.locator('#rom').setInputFiles({ name: 'local.sfc', mimeType: 'application/octet-stream', buffer: fixture });
    await expect(page.locator('#error')).toContainText('Save could not be restored');
    await expect(page.locator('#current-game')).toHaveText(oldName);
    expect(await marker(page)).toBe(0x79);
    await expect(page.locator('#play')).toBeDisabled();
    await page.evaluate(() => { Storage.prototype.getItem = originalGetItem; window.dispatchEvent(new Event('pagehide')); });
    expect(await page.evaluate(targetKey => localStorage.getItem(targetKey), targetKey)).toBe(await page.evaluate(() => targetBefore));
    expect(await page.evaluate(oldKey => atob(localStorage.getItem(oldKey)).charCodeAt(3), oldKey)).toBe(0x79);
    await page.evaluate(targetKey => localStorage.setItem(targetKey, targetSave), targetKey);
    await page.locator('#retry-save').click();
    await expect(page.locator('#status')).toHaveText('Playing');
    await expect(page.locator('#error')).toBeEmpty();
    expect(await marker(page)).toBe(0x48);
    if (destination === 'demo') await localRom(page); else await loadDemo(page);
    expect(await marker(page)).toBe(0x79);
  });
}
