import { selectDemo } from './cartridge.js';
import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { setTabHidden } from './playback.js';

test.use({ hasTouch: true, isMobile: true, viewport: { width: 320, height: 740 } });
const masks = { Up: 0x800, Down: 0x400, Left: 0x200, Right: 0x100, A: 0x80, B: 0x8000, X: 0x40, Y: 0x4000, L: 0x20, R: 0x10, Start: 0x1000, Select: 0x2000 };
// Extend the original MIT smoke guest with a test-only subroutine that copies
// BOTH auto-read bytes to WRAM. The shipped fixture and Wasm are unchanged.
const probe = Buffer.from(readFileSync(new URL('../../tests/roms/smoke.bin', import.meta.url)));
const readHigh = Buffer.from([0xad, 0x19, 0x42, 0x85, 0x11]);
const offset = probe.indexOf(readHigh);
if (offset < 0 || probe.indexOf(readHigh, offset + 1) !== -1) throw new Error('Smoke controller probe moved');
probe.set([0x20, 0x00, 0x9e, 0xea, 0xea], offset); // JSR $9e00, NOP, NOP.
probe.set([0xad, 0x18, 0x42, 0x85, 0x12, 0xad, 0x19, 0x42, 0x85, 0x11, 0x60], 0x1e00);
const controller = page => page.evaluate(() => testCore.peek(0x12) | testCore.peek(0x11) << 8);
async function boot(page, diagnostic = true) {
  await page.addInitScript(() => {
    const instantiate = WebAssembly.instantiate;
    WebAssembly.instantiate = async (...args) => { const result = await instantiate(...args); window.testCore = result.instance.exports; return result; };
    Object.defineProperty(navigator, 'getGamepads', { value: () => window.testPads ?? [] });
  });
  await page.goto('/'); await expect(page.locator('#demo')).toBeEnabled();
  if (diagnostic) {
    await page.locator('#rom').setInputFiles({ name: 'controller-probe.sfc', mimeType: 'application/octet-stream', buffer: probe });
    await expect(page.locator('#status')).toHaveText('Playing');
  } else await selectDemo(page);
  await expect(page.locator('#status')).toHaveText('Playing');
}
async function fingers(page) {
  const session = await page.context().newCDPSession(page), points = new Map();
  const send = type => session.send('Input.dispatchTouchEvent', { type, touchPoints: [...points].map(([id, p]) => ({ id, ...p })) });
  return {
    async down(name, id = 1) {
      const r = await page.getByRole('button', { name: `SNES ${name}`, exact: true }).boundingBox();
      points.set(id, { x: r.x + r.width / 2, y: r.y + r.height / 2 }); await send('touchStart');
    },
    async move(id, x, y) { points.set(id, { x, y }); await send('touchMove'); },
    async up(id = 1) { const point = points.get(id); points.delete(id); await session.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [{ id, ...point }] }); },
    async cancel() { points.clear(); await send('touchCancel'); },
  };
}
async function layout(page) {
  const result = await page.evaluate(() => {
    const rect = element => { const r = element.getBoundingClientRect(); return { x: r.x, y: r.y, right: r.right, bottom: r.bottom, width: r.width, height: r.height }; };
    const headerActions = [...document.querySelectorAll('header summary, header a, #scale, #display')];
    const playerActions = [...document.querySelectorAll('#change-cartridge, #current-game, .toolbar button, #volume')];
    const hitTarget = element => {
      const r = rect(element);
      return element.contains(document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2));
    };
    return { width: innerWidth, height: innerHeight, scrollWidth: document.documentElement.scrollWidth, scrollHeight: document.documentElement.scrollHeight,
      screen: rect(document.querySelector('canvas')),
      actions: [...headerActions, ...playerActions].map(action => ({ ...rect(action), label: action.id || action.textContent.trim() })),
      headerHits: headerActions.map(action => ({ label: action.id || action.textContent.trim(), hit: hitTarget(action) })),
      buttons: [...document.querySelectorAll('[data-button]')].map(button => ({ ...rect(button), label: button.ariaLabel, hit: hitTarget(button) })) };
  });
  expect(result.scrollWidth).toBe(result.width);
  expect(result.scrollHeight).toBe(result.height);
  expect(result.buttons).toHaveLength(12);
  for (const r of [result.screen, ...result.actions, ...result.buttons]) {
    expect(r.x, r.label).toBeGreaterThanOrEqual(0); expect(r.y, r.label).toBeGreaterThanOrEqual(0);
    expect(r.right, r.label).toBeLessThanOrEqual(result.width); expect(r.bottom, r.label).toBeLessThanOrEqual(result.height);
  }
  for (const action of result.headerHits) expect(action.hit, action.label).toBe(true);
  for (const button of result.buttons) { expect(button.hit, button.label).toBe(true); expect(button.width).toBeGreaterThanOrEqual(44); expect(button.height).toBeGreaterThanOrEqual(44); }
  expect(result.screen.width/result.screen.height).toBeCloseTo(256/224, 3);
  return result;
}

test('all twelve touch buttons reach real guest auto-read registers with their SNES bits', async ({ page }) => {
  await boot(page); const touch = await fingers(page); await layout(page);
  await page.locator('#change-cartridge').tap(); await page.locator('h1').tap();
  await expect(page.locator('#cartridge-picker')).toBeHidden();
  await expect(page.locator('#status')).toHaveText('Playing');
  for (const [name, mask] of Object.entries(masks)) {
    await touch.down(name); await expect.poll(() => controller(page), { message: name }).toBe(mask);
    await touch.up(); await expect.poll(() => controller(page)).toBe(0);
  }
});

test('Reset releases captured touch and keyboard input, then accepts all twelve buttons without Play', async ({ page }) => {
  await boot(page); const touch = await fingers(page);
  await page.keyboard.down('ArrowRight'); await touch.down('B');
  await expect.poll(() => controller(page)).toBe(0x100 | 0x8000);
  await page.locator('#reset').click();
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('#play')).toHaveText('Pause');
  await expect(page.locator('[data-button].pressed')).toHaveCount(0);
  await expect.poll(() => controller(page)).toBe(0);
  await touch.up(); await page.keyboard.up('ArrowRight');
  for (const [name, mask] of Object.entries(masks)) {
    await touch.down(name); await expect.poll(() => controller(page), { message: name }).toBe(mask);
    await touch.up(); await expect.poll(() => controller(page)).toBe(0);
  }
  await page.evaluate(() => { window.testPads = [{ mapping: 'standard', axes: [1, 0], buttons: [] }]; });
  await expect.poll(() => controller(page)).toBe(0x100);
  await page.evaluate(() => { window.testPads = []; });
  await expect.poll(() => controller(page)).toBe(0);
});

test('multi-touch diagonals, shoulders and independent keyboard/gamepad overlap', async ({ page }) => {
  await boot(page); const touch = await fingers(page);
  await page.locator('#video').tap(); await page.keyboard.down('ArrowRight');
  await touch.down('Right'); await touch.down('A', 2); await touch.down('L', 3);
  await expect.poll(() => controller(page)).toBe(0x100 | 0x80 | 0x20);
  await touch.up(); await expect.poll(() => controller(page)).toBe(0x100 | 0x80 | 0x20); // Keyboard still holds Right.
  await page.keyboard.up('ArrowRight'); await expect.poll(() => controller(page)).toBe(0x80 | 0x20);
  await page.evaluate(() => { window.testPads = [{ mapping: 'standard', axes: [0, 0], buttons: Array.from({ length: 16 }, (_, i) => ({ pressed: i === 1 || i === 5 })) }]; });
  await touch.up(2); await touch.up(3); await expect.poll(() => controller(page)).toBe(0x80 | 0x10);
  await page.evaluate(() => { window.testPads = []; });
  await touch.down('Up');
  const dpad = await page.locator('.dpad').boundingBox();
  await touch.move(1, dpad.x + 10, dpad.y + 10); await expect.poll(() => controller(page)).toBe(0x800 | 0x200);
  await touch.down('B', 2); await expect.poll(() => controller(page)).toBe(0x800 | 0x200 | 0x8000);
  await touch.cancel(); await expect.poll(() => controller(page)).toBe(0);
  // Two pointers can also hold the same bit; releasing one retains the other.
  await touch.down('A'); await touch.down('A', 2); await touch.up();
  await expect.poll(() => controller(page)).toBe(0x80); await touch.up(2);
  await expect.poll(() => controller(page)).toBe(0);
});

test('outside release, cancellation, lost capture, blur, visibility and focus release input', async ({ page }) => {
  await boot(page); const touch = await fingers(page);
  await touch.down('B'); await touch.move(1, 2, 2); await touch.up(); await expect.poll(() => controller(page)).toBe(0);
  await touch.down('L'); await touch.cancel(); await expect.poll(() => controller(page)).toBe(0);
  await page.keyboard.down('ArrowRight'); await touch.down('A');
  await expect.poll(() => controller(page)).toBe(masks.Right | masks.A);
  await page.evaluate(() => window.dispatchEvent(new Event('blur')));
  await expect.poll(() => controller(page)).toBe(0); await touch.up(); await page.keyboard.up('ArrowRight');
  await touch.down('R'); await page.evaluate(() => {
    const b = document.querySelector('[data-button="16"]');
    // Releasing real capture exercises the platform-generated lost event.
    for (let id = 1; id < 30; id++) if (b.hasPointerCapture(id)) b.releasePointerCapture(id);
  });
  await expect.poll(() => controller(page)).toBe(0); await touch.up();
  await page.getByRole('button', { name: 'SNES Start', exact: true }).focus(); await page.keyboard.down('Space');
  await expect.poll(() => controller(page)).toBe(0x1000);
  await page.locator('#video').tap(); await page.keyboard.up('Space'); await expect.poll(() => controller(page)).toBe(0);
  await page.keyboard.down('ArrowLeft'); await touch.down('Select');
  await expect.poll(() => controller(page)).toBe(masks.Left | masks.Select);
  await setTabHidden(page, true);
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('#play')).toHaveText('Pause');
  await expect(page.locator('[data-button].pressed')).toHaveCount(0);
  await setTabHidden(page, false);
  // The guest sees both releases before the original physical inputs are released.
  await expect.poll(() => controller(page)).toBe(0);
  await touch.up(); await page.keyboard.up('ArrowLeft');
  await page.keyboard.down('ArrowRight'); await touch.down('B');
  await page.evaluate(() => {
    window.testPads = [{ mapping: 'standard', axes: [0, 0], buttons: Array.from({ length: 16 }, (_, i) => ({ pressed: i === 5 })) }];
  });
  await expect.poll(() => controller(page)).toBe(masks.Right | masks.B | masks.R);
  await page.evaluate(() => { window.testPads = []; }); await touch.up(); await page.keyboard.up('ArrowRight');
  await expect.poll(() => controller(page)).toBe(0);
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('#play')).toHaveText('Pause');
});

for (const viewport of [{ width: 320, height: 740 }, { width: 375, height: 667 }, { width: 360, height: 640 }, { width: 390, height: 844 }, { width: 667, height: 320 }, { width: 844, height: 390 }, { width: 568, height: 320 }, { width: 424, height: 320 }, { width: 400, height: 320 }]) {
  test(`touch layout and live resize preserve geometry/SRAM at ${viewport.width}×${viewport.height}`, async ({ page }) => {
    await page.setViewportSize(viewport); await boot(page); const touch = await fingers(page);
    let result = await layout(page);
    if (viewport.width > viewport.height) {
      const left = result.buttons.find(b => b.label === 'SNES Left'), right = result.buttons.find(b => b.label === 'SNES A');
      expect(left.right).toBeLessThanOrEqual(result.screen.x); expect(right.x).toBeGreaterThanOrEqual(result.screen.right);
    }
    await page.evaluate(() => { new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3] = 0x79; });
    await touch.down('A'); await expect.poll(() => controller(page)).toBe(0x80);
    const counter = await page.evaluate(() => testCore.peek(0x10));
    await page.setViewportSize(viewport.width > viewport.height ? { width: 320, height: 740 } : { width: 667, height: 320 });
    await expect.poll(() => controller(page)).toBe(0); await touch.up();
    await expect(page.locator('#status')).toHaveText('Playing'); await layout(page);
    expect(await page.evaluate(() => new Uint8Array(testCore.memory.buffer, testCore.sram_buffer(), testCore.sram_len())[3])).toBe(0x79);
    await expect.poll(() => page.evaluate(() => testCore.peek(0x10))).not.toBe(counter);
    await page.locator('#play').tap(); await expect(page.locator('#status')).toHaveText('Paused'); await layout(page);
    await page.locator('#play').tap(); await expect(page.locator('#status')).toHaveText('Playing');
  });
}

for (const viewport of [{ width: 375, height: 667 }, { width: 360, height: 640 }]) {
  test(`demo selection keeps every touch control in view at ${viewport.width}×${viewport.height}`, async ({ page }) => {
    await page.setViewportSize(viewport); await boot(page, false); await layout(page);
    await page.locator('#play').tap(); await expect(page.locator('#status')).toHaveText('Paused'); await layout(page);
    await page.locator('#play').tap(); await expect(page.locator('#status')).toHaveText('Playing'); await layout(page);
    expect(await page.evaluate(() => scrollY)).toBe(0);
  });
}

test('CRT fallback guidance fits the initial 375×667 portrait viewport', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 667 });
  await page.addInitScript(() => {
    const getContext = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function (kind, ...args) {
      return kind === 'webgl' ? null : getContext.call(this, kind, ...args);
    };
  });
  await boot(page);
  await expect(page.locator('#display')).toHaveValue('pixel');
  const guidance = page.getByText(/Using Pixel\. Select CRT to retry\./);
  await expect(guidance).toBeInViewport({ ratio: 1 });
  expect(await page.evaluate(() => scrollY)).toBe(0);
  await layout(page);
});

test('combined CRT and sound failures keep 360×640 portrait playback usable', async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 640 });
  await page.addInitScript(() => {
    const getContext = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function (kind, ...args) {
      return kind === 'webgl' ? null : getContext.call(this, kind, ...args);
    };
    AudioContext.prototype.resume = () => Promise.reject(new DOMException('Activation denied', 'NotAllowedError'));
  });
  await boot(page, false);
  await expect(page.locator('#display')).toHaveValue('pixel');
  const canvas = page.locator('#video');
  const audioGuidance = page.locator('#audio-status');
  const displayGuidance = page.locator('#display-status');
  await expect(audioGuidance).toHaveText(/Sound unavailable.*Adjust volume or unmute to retry/);
  await expect(displayGuidance).toHaveText(/Using Pixel\. Select CRT to retry\./);
  await expect(canvas).toBeInViewport({ ratio: 1 });
  const { width, height } = await canvas.boundingBox();
  expect(Math.round(height)).toBeGreaterThanOrEqual(120);
  expect(width / height).toBeCloseTo(256 / 224, 3);
  for (const button of await page.locator('[data-button]').all()) {
    await expect(button).toBeInViewport({ ratio: 1 });
  }
  await displayGuidance.scrollIntoViewIfNeeded();
  await expect(audioGuidance).toBeInViewport({ ratio: 1 });
  await expect(displayGuidance).toBeInViewport({ ratio: 1 });
  await expect(page.locator('#status')).toHaveText('Playing');
});

test('save restore retry fits the initial 412×915 portrait viewport', async ({ page }) => {
  await page.setViewportSize({ width: 412, height: 915 });
  await page.addInitScript(() => {
    const getItem = Storage.prototype.getItem;
    Storage.prototype.getItem = function (key) {
      if (!window.allowSaveRead && key.startsWith('snes-rust:sram:')) {
        throw new DOMException('Storage unavailable', 'SecurityError');
      }
      return getItem.call(this, key);
    };
  });
  await page.goto('/');
  await expect(page.locator('#status')).toHaveText('Ready');
  await page.locator('#rom').setInputFiles({ name: 'controller-probe.sfc', mimeType: 'application/octet-stream', buffer: probe });
  const warning = page.getByRole('alert');
  await expect(warning).toHaveText(/Save could not be restored.*Retry restoring the save/);
  await expect(warning).toBeInViewport({ ratio: 1 });
  const retry = page.getByRole('button', { name: 'Retry restoring save' });
  await expect(retry).toBeInViewport({ ratio: 1 });
  expect(await page.evaluate(() => scrollY)).toBe(0);
  await layout(page);
  await page.evaluate(() => { window.allowSaveRead = true; });
  await retry.tap();
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect(retry).toBeHidden();
});

test('tapping a landscape controller button dismisses Help and reaches the guest', async ({ page }) => {
  await page.setViewportSize({ width: 667, height: 320 });
  await boot(page);
  const help = page.locator('summary');
  const reference = page.locator('.controls-reference');
  await help.tap();
  await expect(reference).toBeVisible();
  await layout(page);
  const touch = await fingers(page);
  await touch.down('A');
  await expect(reference).toBeHidden();
  await expect.poll(() => controller(page)).toBe(masks.A);
  await touch.up();
  await expect.poll(() => controller(page)).toBe(0);
  await expect(page.locator('#status')).toHaveText('Playing');
  await layout(page);
});

test('short landscape Help scrolls by keyboard during playback and Escape returns focus', async ({ page }) => {
  await page.setViewportSize({ width: 568, height: 280 });
  await boot(page);
  const help = page.locator('summary');
  const reference = page.locator('.controls-reference');
  const hints = reference.getByText(/Standard gamepads work/);
  await help.focus();
  await page.keyboard.press('Enter');
  await expect(reference).toBeVisible();
  await expect(hints).not.toBeInViewport({ ratio: 1 });
  await page.keyboard.press('Tab');
  await expect(reference).toBeFocused();
  await page.keyboard.down('ArrowDown');
  await expect.poll(() => reference.evaluate(el => el.scrollTop)).toBeGreaterThan(0);
  expect(await controller(page)).toBe(0);
  await page.keyboard.up('ArrowDown');
  await page.keyboard.press('End');
  await expect(hints).toBeInViewport({ ratio: 1 });
  await page.keyboard.press('Escape');
  await expect(reference).toBeHidden();
  await expect(help).toBeFocused();
  await page.locator('#video').tap();
  await page.keyboard.down('ArrowDown');
  await expect.poll(() => controller(page)).toBe(masks.Down);
  await page.keyboard.up('ArrowDown');
  await expect.poll(() => controller(page)).toBe(0);
});

test('Shionari title advances into its real story with mobile A touch', async ({ page }) => {
  await boot(page, false); const touch = await fingers(page);
  const scene = () => page.evaluate(async () => {
    const bytes = new Uint8Array(testCore.memory.buffer, testCore.framebuffer() + 16*256*4, 128*256*4);
    return [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].join(',');
  });
  await page.waitForTimeout(1200); const title = await scene();
  await touch.down('A'); await expect.poll(scene).not.toBe(title); await touch.up();
  await page.waitForTimeout(1000); expect(await scene()).not.toBe(title);
  await page.setViewportSize({ width: 667, height: 320 }); await layout(page);
  await page.locator('#play').tap(); await expect(page.locator('#status')).toHaveText('Paused');
  await page.locator('#play').tap(); await expect(page.locator('#status')).toHaveText('Playing');
});


for (const viewport of [{ width: 844, height: 390 }, { width: 568, height: 320 }]) {
  test(`safe-area insets preserve landscape geometry and controls at ${viewport.width}×${viewport.height}`, async ({ page }) => {
    await page.setViewportSize(viewport);
    const session = await page.context().newCDPSession(page);
    await session.send('Emulation.setSafeAreaInsetsOverride', { insets: { left: 44, right: 44, bottom: 21, top: 0 } });
    await boot(page); const result = await layout(page);
    for (const r of [result.screen, ...result.actions, ...result.buttons]) {
      expect(r.x).toBeGreaterThanOrEqual(44); expect(r.right).toBeLessThanOrEqual(viewport.width - 44);
      expect(r.bottom).toBeLessThanOrEqual(viewport.height - 21);
    }
    expect(await page.locator('meta[name="viewport"]').getAttribute('content')).toContain('viewport-fit=cover');
  });
}

test('fullscreen entry/exit release virtual input and preserve running guest geometry', async ({ page }) => {
  await boot(page); const touch = await fingers(page);
  await touch.down('B'); await expect.poll(() => controller(page)).toBe(0x8000);
  await page.locator('#fullscreen').click();
  await expect.poll(() => page.evaluate(() => Boolean(document.fullscreenElement))).toBe(true);
  await expect.poll(() => controller(page)).toBe(0); await touch.up();
  await touch.down('A'); await expect.poll(() => controller(page)).toBe(0x80);
  await page.locator('#fullscreen').click();
  await expect.poll(() => page.evaluate(() => Boolean(document.fullscreenElement))).toBe(false);
  await expect.poll(() => controller(page)).toBe(0); await touch.up();
  await expect(page.locator('#status')).toHaveText('Playing'); await layout(page);
});
