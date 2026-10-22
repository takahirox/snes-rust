import { test, expect } from '@playwright/test';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { lstatSync, readFileSync, readdirSync } from 'node:fs';

const dist = new URL('../dist/', import.meta.url);
const assets = { 'index.html': 'text/html', 'app.js': 'text/javascript', 'style.css': 'text/css', 'snes_wasm.wasm': 'application/wasm' };
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
  await expect(page.locator('#status')).toHaveText('Open a local SNES ROM to begin');
  await expect(page.locator('#rom')).toBeEnabled();
  await expect(page.locator('canvas')).toHaveCSS('image-rendering', 'pixelated');
  // An original MIT diagnostic is selected locally, never copied into dist.
  await page.locator('#rom').setInputFiles({
    name: 'smoke.sfc', mimeType: 'application/octet-stream',
    buffer: readFileSync(new URL('../../tests/roms/smoke.bin', import.meta.url)),
  });
  await expect(page.locator('#play')).toBeEnabled();
  await page.locator('#play').click();
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
