import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';

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
    await expect(page.locator('#status')).toContainText('loaded');
    await page.locator('#play').tap();
  } else await page.locator('#demo').tap();
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
    return { width: innerWidth, height: innerHeight, scrollWidth: document.documentElement.scrollWidth,
      screen: rect(document.querySelector('canvas')), buttons: [...document.querySelectorAll('[data-button]')].map(button => {
        const r = rect(button), hit = document.elementFromPoint(r.x + r.width/2, r.y + r.height/2);
        return { ...r, label: button.ariaLabel, hit: hit === button };
      }) };
  });
  expect(result.scrollWidth).toBe(result.width);
  expect(result.buttons).toHaveLength(12);
  for (const r of [result.screen, ...result.buttons]) {
    expect(r.x, r.label).toBeGreaterThanOrEqual(0); expect(r.y, r.label).toBeGreaterThanOrEqual(0);
    expect(r.right, r.label).toBeLessThanOrEqual(result.width); expect(r.bottom, r.label).toBeLessThanOrEqual(result.height);
  }
  for (const button of result.buttons) { expect(button.hit, button.label).toBe(true); expect(button.width).toBeGreaterThanOrEqual(44); expect(button.height).toBeGreaterThanOrEqual(44); }
  expect(result.screen.width/result.screen.height).toBeCloseTo(256/224, 3);
  return result;
}

test('all twelve touch buttons reach real guest auto-read registers with their SNES bits', async ({ page }) => {
  await boot(page); const touch = await fingers(page); await layout(page);
  for (const [name, mask] of Object.entries(masks)) {
    await touch.down(name); await expect.poll(() => controller(page), { message: name }).toBe(mask);
    await touch.up(); await expect.poll(() => controller(page)).toBe(0);
  }
});

test('multi-touch diagonals, shoulders and independent keyboard/gamepad overlap', async ({ page }) => {
  await boot(page); const touch = await fingers(page);
  await page.locator('canvas').tap(); await page.keyboard.down('ArrowRight');
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
  await touch.down('A'); await page.evaluate(() => window.dispatchEvent(new Event('blur')));
  await expect.poll(() => controller(page)).toBe(0); await touch.up();
  await touch.down('R'); await page.evaluate(() => {
    const b = document.querySelector('[data-button="16"]');
    // Releasing real capture exercises the platform-generated lost event.
    for (let id = 1; id < 30; id++) if (b.hasPointerCapture(id)) b.releasePointerCapture(id);
  });
  await expect.poll(() => controller(page)).toBe(0); await touch.up();
  await page.getByRole('button', { name: 'SNES Start', exact: true }).focus(); await page.keyboard.down('Space');
  await expect.poll(() => controller(page)).toBe(0x1000);
  await page.locator('canvas').tap(); await page.keyboard.up('Space'); await expect.poll(() => controller(page)).toBe(0);
  await touch.down('Select'); await page.evaluate(() => {
    Object.defineProperty(document, 'hidden', { configurable: true, value: true }); document.dispatchEvent(new Event('visibilitychange'));
  });
  await expect(page.locator('#status')).toHaveText('Paused'); await touch.up();
  await page.evaluate(() => { delete document.hidden; }); await page.locator('#play').tap();
  await expect.poll(() => controller(page)).toBe(0);
});

for (const viewport of [{ width: 320, height: 740 }, { width: 390, height: 844 }, { width: 667, height: 320 }, { width: 844, height: 390 }, { width: 568, height: 320 }]) {
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
  await page.locator('#demo').tap(); await expect(page.locator('#status')).toHaveText('Paused');
  await page.locator('#demo').tap(); await expect(page.locator('#status')).toHaveText('Playing');
});


test('safe-area insets keep landscape controls clear of notches and home indicators', async ({ page }) => {
  await page.setViewportSize({ width: 844, height: 390 });
  const session = await page.context().newCDPSession(page);
  await session.send('Emulation.setSafeAreaInsetsOverride', { insets: { left: 44, right: 44, bottom: 21, top: 0 } });
  await boot(page); const result = await layout(page);
  for (const r of [result.screen, ...result.buttons]) {
    expect(r.x).toBeGreaterThanOrEqual(44); expect(r.right).toBeLessThanOrEqual(800);
    expect(r.bottom).toBeLessThanOrEqual(369);
  }
  expect(await page.locator('meta[name="viewport"]').getAttribute('content')).toContain('viewport-fit=cover');
});

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
