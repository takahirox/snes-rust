import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { selectDemo } from './cartridge.js';
const buffer = readFileSync(new URL('../../tests/roms/smoke.bin', import.meta.url));
async function ready(page) {
  await page.goto('/'); await expect(page.locator('#change-cartridge')).toBeEnabled();
}
async function entryActions(page) {
  await expect(page.getByRole('heading', { name: 'snes-rust', exact: true, level: 1 })).toBeVisible();
  const slot = page.getByRole('group', { name: 'Cartridge slot', exact: true });
  await expect(slot).toBeVisible();
  await expect(slot.getByText('Inserted cartridge', { exact: true })).toBeVisible();
  await expect(slot.getByRole('button', { name: 'Insert cartridge', exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: /潮鳴り|Shionari|BUNDLED ADVENTURE/ })).toHaveCount(0);
  await expect(page.locator('#current-game')).toBeVisible();
}
const geometry = page => page.locator('#video, .game-entry, #change-cartridge, #play, #reset, #volume-control, #fullscreen').evaluateAll(nodes => nodes.map(n => {
  const r = n.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height };
}));

test('emulator-first layout exposes display settings and GitHub with optional keyboard help', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await ready(page); await entryActions(page);
  await expect(page.locator('#placeholder')).toHaveText('Insert a cartridge');
  await expect(page.locator('#current-game')).toHaveText('None');
  const areas = await page.locator('.screen, .game-entry').evaluateAll(nodes => nodes.map(n => n.getBoundingClientRect().width));
  expect(areas[0]).toBeGreaterThan(areas[1] * 2);
  await expect(page.locator('details')).toHaveCount(1);
  await expect(page.getByText(/About \/ Legal|All rights reserved|About snes-rust/)).toHaveCount(0);
  const help = page.locator('summary');
  const keyboard = page.getByRole('heading', { name: 'Keyboard' });
  await expect(help).toHaveText('Help');
  await expect(keyboard).toBeHidden();
  const github = page.getByRole('link', { name: 'GitHub', exact: true });
  await expect(github).toHaveAttribute('href', 'https://github.com/takahirox/snes-rust');
  const readme = readFileSync(new URL('../../README.md', import.meta.url), 'utf8');
  expect(readme).toContain('[ROM-specific provenance and distribution notice](demos/NOTICE.md)');
  const notice = readFileSync(new URL('../../demos/NOTICE.md', import.meta.url), 'utf8');
  expect(notice).toContain('**All rights reserved.**');
  expect(notice).toContain('**does not apply to this game ROM or its embedded content**');
  const displaySettings = page.getByRole('group', { name: 'Display settings' });
  const toolbar = page.getByRole('group', { name: 'Player controls' });
  await expect(toolbar.getByRole('group', { name: 'Display settings' })).toBeVisible();
  await expect(page.locator('header').getByRole('combobox')).toHaveCount(0);
  const screen = await page.locator('#video').boundingBox();
  expect((await toolbar.boundingBox()).y).toBeGreaterThanOrEqual(screen.y + screen.height);
  const scale = displaySettings.getByRole('combobox', { name: 'Scale', exact: true });
  const display = displaySettings.getByRole('combobox', { name: 'Display', exact: true });
  await expect(scale).toBeVisible();
  await expect(scale).toHaveValue('3');
  await expect(display).toBeVisible();
  await expect(display).toHaveValue('crt');
  await page.keyboard.press('Tab');
  await expect(help).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(keyboard).toBeVisible();
  await expect(page.locator('details dt')).toHaveText(['Arrow keys', 'Z / X', 'A / S', 'Q / W', 'Enter / Shift']);
  await expect(page.locator('details dd')).toHaveText(['Move', 'B / A', 'Y / X', 'L / R', 'Start / Select']);
  await expect(page.locator('details p')).toHaveText(/touch D-pad for diagonals.*Standard gamepads/);
  await page.keyboard.press('Enter');
  await expect(keyboard).toBeHidden();
  for (const control of [github, page.locator('#video'), page.locator('#fullscreen'),
    page.locator('#mute'), page.locator('#volume'), scale, display, page.locator('#change-cartridge')]) {
    await page.keyboard.press('Tab');
    await expect(control).toBeFocused();
    await expect(control).toHaveCSS('outline-style', 'solid');
    if (control === scale) {
      await page.keyboard.press('1');
      await expect(scale).toHaveValue('1');
      await expect(page.locator('#video')).toHaveCSS('width', '256px');
    }
  }
});

test('keyboard picker offers all cartridge actions; Escape, outside click and file cancellation preserve gameplay and held input', async ({ page }) => {
  await page.addInitScript(() => {
    const instantiate = WebAssembly.instantiate;
    WebAssembly.instantiate = async (...args) => { const r = await instantiate(...args); window.testCore = r.instance.exports; return r; };
  });
  await ready(page);
  await page.locator('#rom').setInputFiles({ name: 'local.sfc', mimeType: 'application/octet-stream', buffer });
  await expect(page.locator('#status')).toHaveText('Playing');
  await page.keyboard.down('ArrowRight');
  await expect.poll(() => page.evaluate(() => testCore.peek(0x11))).toBe(1);
  await page.locator('#change-cartridge').focus(); await page.keyboard.press('Enter');
  const picker = page.getByRole('dialog', { name: 'Cartridges' });
  await expect(picker).toBeVisible();
  await expect(picker.getByRole('button')).toHaveCount(5);
  await expect(page.locator('#demo')).toBeFocused();
  await expect(page.locator('#demo')).toHaveAccessibleName('Bundled demo — 潮鳴りの密室');
  await expect(page.locator('#demo-yonaki')).toHaveAccessibleName('Bundled demo — 夜鳴き鐘');
  await expect(page.locator('#open-local')).toHaveAccessibleName('Open local ROM…');
  await expect(page.locator('#status')).toHaveText('Playing');
  expect(await page.evaluate(() => testCore.peek(0x11))).toBe(1);
  await page.keyboard.up('ArrowRight');
  await expect.poll(() => page.evaluate(() => testCore.peek(0x11))).toBe(0);
  await page.keyboard.press('Tab'); await expect(page.locator('#demo-yonaki')).toBeFocused();
  await page.keyboard.press('Tab'); await expect(picker.getByRole('button', { name: 'local.sfc', exact: true })).toBeFocused();
  await page.keyboard.press('Tab'); await expect(picker.getByRole('button', { name: 'Remove local.sfc' })).toBeFocused();
  await page.keyboard.press('Tab'); await expect(page.locator('#open-local')).toBeFocused();
  await page.keyboard.press('Escape'); await expect(picker).toBeHidden();
  await expect(page.locator('#change-cartridge')).toBeFocused();
  await page.locator('#change-cartridge').click(); await page.locator('h1').click();
  await expect(picker).toBeHidden(); await expect(page.locator('#status')).toHaveText('Playing');
  const counter = await page.evaluate(() => testCore.peek(0x10));
  await page.locator('#change-cartridge').click();
  const chooser = page.waitForEvent('filechooser'); await page.locator('#open-local').click(); await chooser;
  // Chromium automation has no native Cancel button; dispatch the platform cancel
  // event with an empty file list, then assert the current machine kept advancing.
  await page.locator('#rom').dispatchEvent('cancel');
  await page.locator('#rom').setInputFiles([]);
  await expect(page.locator('#current-game')).toHaveText('local.sfc');
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect.poll(() => page.evaluate(() => testCore.peek(0x10))).not.toBe(counter);
});

for (const viewport of [{ width: 1280, height: 900 }, { width: 320, height: 740 }, { width: 375, height: 667 }, { width: 360, height: 640 }, { width: 390, height: 844 }, { width: 667, height: 320 }, { width: 844, height: 390 }, { width: 568, height: 320 }, { width: 844, height: 390, safeArea: true }, { width: 568, height: 320, safeArea: true }]) {
  test(`slot and playback geometry stay fixed across every state at ${viewport.width}×${viewport.height}${viewport.safeArea ? ' with safe areas' : ''}`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width: viewport.width, height: viewport.height });
    if (viewport.safeArea) {
      const session = await page.context().newCDPSession(page);
      await session.send('Emulation.setSafeAreaInsetsOverride', { insets: { left: 44, right: 44, bottom: 21, top: 0 } });
    }
    await ready(page); await entryActions(page);
    const initial = await geometry(page);
    let pending; await page.route('**/shionari.sfc', route => { pending = route; });
    await selectDemo(page); await expect.poll(() => Boolean(pending)).toBe(true);
    expect(await geometry(page)).toEqual(initial);
    await pending.fulfill({ status: 200, body: readFileSync(new URL('../../demos/shionari.sfc', import.meta.url)) });
    await expect(page.locator('#status')).toHaveText('Playing');
    expect(await geometry(page)).toEqual(initial);
    if (!viewport.safeArea && [1280, 320, 667].includes(viewport.width)) {
      // Capture actual game pixels after the cartridge's black boot interval.
      await expect.poll(() => page.locator('#video').evaluate(canvas =>
        canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data.some((v, i) => i % 4 !== 3 && v > 0)
      ), { timeout: 10000 }).toBe(true);
      await page.keyboard.press('KeyX', { delay: 100 }); // Skip the opening to the real title scene.
      await page.waitForTimeout(1800);
      await page.screenshot({ path: testInfo.outputPath(`cartridge-${viewport.width}.png`) });
      await page.locator('#change-cartridge').click();
      await page.screenshot({ path: testInfo.outputPath(`picker-${viewport.width}.png`) });
      await page.keyboard.press('Escape');
    }
    await page.locator('#play').click(); await expect(page.locator('#status')).toHaveText('Paused');
    expect(await geometry(page)).toEqual(initial);
    const name = 'A deliberately very long local cartridge filename that must not resize the controls.sfc';
    await page.locator('#rom').setInputFiles({ name, mimeType: 'application/octet-stream', buffer });
    await expect(page.locator('#status')).toHaveText('Playing');
    await expect(page.locator('#current-game')).toHaveText(name);
    expect(await geometry(page)).toEqual(initial);
    await page.locator('#rom').setInputFiles({ name: 'invalid.sfc', mimeType: 'application/octet-stream', buffer: Buffer.alloc(32768) });
    await expect(page.locator('#error')).not.toBeEmpty();
    await expect(page.locator('#current-game')).toHaveText(name);
    expect(await geometry(page)).toEqual(initial);
    // Check player bounds; errors may extend the page below the fixed player.
    const targets = page.locator('h1, header a, #scale, #display, #video, #change-cartridge, #current-game, #play');
    for (const target of await targets.all()) await expect(target).toBeInViewport({ ratio: 1 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(viewport.width);
  });
}

test.describe('mobile first view', () => {
  test.use({ hasTouch: true, isMobile: true });
  for (const viewport of [{ width: 320, height: 740 }, { width: 360, height: 640 }, { width: 374, height: 640 }, { width: 667, height: 320 }, { width: 568, height: 320, safeArea: true }]) {
    test(`all twelve controls and cartridge slot fit at ${viewport.width}×${viewport.height}`, async ({ page }) => {
      await page.setViewportSize({ width: viewport.width, height: viewport.height });
      if (viewport.safeArea) {
        const session = await page.context().newCDPSession(page);
        await session.send('Emulation.setSafeAreaInsetsOverride', { insets: { left: 44, right: 44, bottom: 21, top: 0 } });
      }
      await ready(page); await entryActions(page);
      const targets = page.locator('header a, summary, #scale, #display, #video, #change-cartridge, #current-game, #play, [data-button]');
      expect(await page.locator('[data-button]').count()).toBe(12);
      for (const target of await targets.all()) {
        await expect(target).toBeInViewport({ ratio: 1 });
        if (viewport.safeArea) {
          const r = await target.boundingBox();
          expect(r.x).toBeGreaterThanOrEqual(44); expect(r.x + r.width).toBeLessThanOrEqual(viewport.width - 44);
          expect(r.y + r.height).toBeLessThanOrEqual(viewport.height - 21);
        }
      }
      expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(viewport.width);
      expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBe(viewport.height);
      await page.locator('#change-cartridge').tap();
      await expect(page.getByRole('dialog', { name: 'Cartridges' })).toBeVisible();
      await page.locator('h1').tap(); await expect(page.locator('#cartridge-picker')).toBeHidden();
      if ([320, 667].includes(viewport.width)) {
        await page.locator('#rom').setInputFiles({ name: 'local.sfc', mimeType: 'application/octet-stream', buffer });
        await expect(page.locator('#status')).toHaveText('Playing');
        await page.locator('summary').tap();
        const reference = page.locator('.controls-reference');
        await expect(reference).toBeInViewport({ ratio: 1 });
        expect(await reference.evaluate(el => el.scrollHeight)).toBeLessThanOrEqual(await reference.evaluate(el => el.clientHeight));
        await page.locator('summary').tap();
        await expect(reference).toBeHidden();
        for (const [id, value] of [['scale', '1'], ['display', 'pixel']]) {
          const select = page.locator(`#${id}`);
          await select.tap();
          await expect(select).toBeFocused();
          // Headless Chromium cannot interact with the platform option popup.
          await page.keyboard.press('Escape');
          await select.selectOption(value);
          await expect(select).toHaveValue(value);
        }
        if (viewport.width === 320) await expect(page.locator('#video')).toHaveCSS('width', '256px');
        await expect(page.locator('.crt-display')).toBeHidden();
        await expect(page.locator('#status')).toHaveText('Playing');
        const repository = 'https://github.com/takahirox/snes-rust';
        await page.route(repository, route => route.fulfill({ body: 'Repository' }));
        await page.getByRole('link', { name: 'GitHub', exact: true }).tap();
        await expect(page).toHaveURL(repository);
      }
    });
  }
});
