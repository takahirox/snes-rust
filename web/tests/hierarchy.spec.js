import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';

async function ready(page) {
  await page.goto('/');
  await expect(page.getByRole('button', { name: 'Try demo', exact: true })).toBeEnabled();
}
async function entryActions(page) {
  await expect(page.getByRole('heading', { name: 'snes-rust', exact: true, level: 1 })).toBeVisible();
  await expect(page.locator('.app-identity')).toContainText('SNES emulator');
  const choices = page.getByRole('group', { name: 'Game selection', exact: true });
  await expect(choices).toBeVisible();
  await expect(choices.getByRole('button', { name: 'Try demo', exact: true })).toBeVisible();
  await expect(choices.getByLabel('Open ROM', { exact: true })).toBeEnabled();
  await expect(choices.locator('.file')).toBeVisible();
  await expect(page.getByRole('heading', { name: /潮鳴り|Shionari|BUNDLED ADVENTURE/ })).toHaveCount(0);
}

test('emulator identity leads a generic player with compact demo metadata and grouped entry actions', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await ready(page); await entryActions(page);
  await expect(page.locator('#placeholder')).toHaveText('Ready when you are');
  await expect(page.locator('#current-game')).toBeHidden();
  await expect(page.locator('#demo-caption')).toBeVisible();
  await expect(page.locator('#demo')).toHaveAccessibleDescription('潮鳴りの密室');
  // Relative emphasis and placement matter; no exact CSS snapshot is required.
  const geometry = await page.evaluate(() => {
    const title = document.querySelector('h1'), caption = document.querySelector('#demo-caption');
    const screen = document.querySelector('.screen').getBoundingClientRect();
    const choices = document.querySelector('.game-entry').getBoundingClientRect();
    return { titleSize: parseFloat(getComputedStyle(title).fontSize), captionSize: parseFloat(getComputedStyle(caption).fontSize),
      screenWidth: screen.width, choicesWidth: choices.width, screenRight: screen.right, choicesLeft: choices.left,
      captionTop: caption.getBoundingClientRect().top, demoBottom: document.querySelector('#demo').getBoundingClientRect().bottom };
  });
  expect(geometry.titleSize).toBeGreaterThan(geometry.captionSize);
  expect(geometry.screenWidth).toBeGreaterThan(geometry.choicesWidth * 2);
  expect(geometry.screenRight).toBeLessThanOrEqual(geometry.choicesLeft);
  expect(geometry.captionTop).toBeGreaterThanOrEqual(geometry.demoBottom);
  await page.getByText('About / Legal', { exact: true }).click();
  await expect(page.getByRole('heading', { name: 'About snes-rust' })).toBeVisible();
  await expect(page.locator('.disclosure').filter({ hasText: 'All rights reserved' })).toBeVisible();
  await expect(page.getByRole('link', { name: 'ROM provenance and distribution notice' })).toHaveAttribute('href', 'https://github.com/takahirox/snes-rust/blob/main/demos/NOTICE.md');
});

test('current cartridge metadata follows demo and local loads without replacing the emulator brand', async ({ page }) => {
  await ready(page);
  await page.getByRole('button', { name: 'Try demo', exact: true }).click();
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('#current-game')).toHaveText('潮鳴りの密室 / Shionari no Misshitsu');
  const buffer = readFileSync(new URL('../../tests/roms/smoke.bin', import.meta.url));
  const name = 'My local diagnostic.sfc';
  await page.getByLabel('Open ROM', { exact: true }).setInputFiles({ name, mimeType: 'application/octet-stream', buffer });
  await expect(page.locator('#current-game')).toHaveText(name);
  await expect(page.locator('#current-game')).toBeVisible();
  await entryActions(page);
  await expect(page.locator('#status')).toContainText(`${name} loaded`);
  await page.locator('#play').click();
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('#current-game')).toHaveText(name);
  await page.getByLabel('Open ROM', { exact: true }).setInputFiles({ name: 'invalid.sfc', mimeType: 'application/octet-stream', buffer: Buffer.alloc(32768) });
  await expect(page.locator('#error')).not.toBeEmpty();
  await expect(page.locator('#current-game')).toHaveText(name);
  await page.getByRole('button', { name: 'Try demo', exact: true }).click();
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('#current-game')).toHaveText('潮鳴りの密室 / Shionari no Misshitsu');
  await expect(page.getByRole('heading', { name: 'snes-rust', exact: true })).toBeVisible();
});

test.describe('mobile first view', () => {
  test.use({ hasTouch: true, isMobile: true });
  for (const viewport of [{ width: 320, height: 740 }, { width: 375, height: 667 }, { width: 360, height: 640 }, { width: 390, height: 844 }, { width: 667, height: 320 }, { width: 844, height: 390 }, { width: 568, height: 320 }, { width: 844, height: 390, safeArea: true }, { width: 568, height: 320, safeArea: true }]) {
    test(`identity, entry actions and all controls fit before loading at ${viewport.width}×${viewport.height}${viewport.safeArea ? ' with safe areas' : ''}`, async ({ page }) => {
      await page.setViewportSize({ width: viewport.width, height: viewport.height });
      if (viewport.safeArea) {
        const session = await page.context().newCDPSession(page);
        await session.send('Emulation.setSafeAreaInsetsOverride', { insets: { left: 44, right: 44, bottom: 21, top: 0 } });
      }
      await ready(page); await entryActions(page);
      await expect(page.locator('#placeholder')).toBeVisible();
      const targets = page.locator('h1, .app-identity p, canvas, #demo, .file, #play, [data-button]');
      expect(await page.locator('[data-button]').count()).toBe(12);
      for (const target of await targets.all()) {
        await expect(target).toBeInViewport({ ratio: 1 });
        if (viewport.safeArea) {
          const r = await target.boundingBox();
          expect(r.x).toBeGreaterThanOrEqual(44);
          expect(r.x + r.width).toBeLessThanOrEqual(viewport.width - 44);
          expect(r.y + r.height).toBeLessThanOrEqual(viewport.height - 21);
        }
      }
      expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(viewport.width);
      expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBe(viewport.height);
      expect(await page.evaluate(() => scrollY)).toBe(0);
    });
  }
});
