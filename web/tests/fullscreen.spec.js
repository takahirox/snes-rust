import { test, expect } from '@playwright/test';
import { selectDemo } from './cartridge.js';

async function gameplayLayout(page, { touch, safeArea, fullscreen = true }) {
  const layout = await page.locator(fullscreen ? ':fullscreen' : '.player').evaluate((el, fullscreen) => {
    const rect = node => node.getBoundingClientRect().toJSON();
    const hit = node => {
      const { x, y, width, height } = rect(node);
      return node.contains(document.elementFromPoint(x + width / 2, y + height / 2));
    };
    const video = el.querySelector('#video');
    return {
      viewport: { width: innerWidth, height: innerHeight },
      screen: rect(el.querySelector('.screen')), video: rect(video),
      controls: fullscreen ? [...el.querySelectorAll('.fullscreen-controls button')]
        .map(node => ({ ...rect(node), hit: node.disabled || hit(node), name: node.id })) : [],
      buttons: [...el.querySelectorAll('[data-button]')].filter(node => node.getClientRects().length)
        .map(node => ({ ...rect(node), hit: hit(node), name: node.ariaLabel })),
      scrollWidth: (fullscreen ? el : document.documentElement).scrollWidth,
      scrollHeight: el.scrollHeight, background: getComputedStyle(el).backgroundColor,
    };
  }, fullscreen);
  const { viewport, screen, video, controls, buttons } = layout;
  expect(layout.scrollWidth).toBe(viewport.width);
  if (fullscreen) {
    expect(layout.scrollHeight).toBe(viewport.height);
    expect(layout.background).toBe('rgb(0, 0, 0)');
    expect(video.width).toBeCloseTo(Math.min(screen.width, screen.height * 256 / 224), 1);
    expect(video.x - screen.x).toBeCloseTo((screen.width - video.width) / 2, 1);
    expect(video.y - screen.y).toBeCloseTo((screen.height - video.height) / 2, 1);
    expect(controls.map(control => control.name)).toEqual(['exit-fullscreen', 'choose-state', 'save-state', 'load-state']);
  } else {
    expect(video.width, 'usable normal player image').toBeGreaterThanOrEqual(72);
  }
  expect(video.width / video.height).toBeCloseTo(256 / 224, 3);
  expect(buttons).toHaveLength(touch ? 12 : 0);
  const insets = safeArea ?? { top: 0, right: 0, bottom: 0, left: 0 };
  for (const r of [video, ...buttons, ...controls]) {
    expect(r.x).toBeGreaterThanOrEqual(insets.left);
    expect(r.right).toBeLessThanOrEqual(viewport.width - insets.right);
    expect(r.y).toBeGreaterThanOrEqual(insets.top);
    expect(r.bottom).toBeLessThanOrEqual(viewport.height - insets.bottom);
    if (r !== video) {
      expect(r.hit, r.name).toBe(true);
      expect(r.width).toBeGreaterThanOrEqual(touch ? 44 : 1);
      expect(r.height).toBeGreaterThanOrEqual(touch ? 44 : 1);
    }
  }
  if (viewport.width > viewport.height) {
    for (const button of buttons) {
      if (/ (?:L|Up|Down|Left|Right|Select)$/.test(button.name)) expect(button.right).toBeLessThanOrEqual(video.x);
      else expect(button.x).toBeGreaterThanOrEqual(video.right);
    }
  } else {
    for (const button of buttons) expect(button.y).toBeGreaterThanOrEqual(video.bottom);
  }
  for (const control of controls) {
    expect(control.y).toBeGreaterThanOrEqual(video.bottom);
    for (const button of buttons) expect(control.y, button.name).toBeGreaterThanOrEqual(button.bottom);
  }
}

function crtState(page) {
  return page.evaluate(() => {
    const lit = pixels => pixels.some((value, index) => index % 4 !== 3 && value > 0);
    const box = element => {
      const { x, y, width, height } = element.getBoundingClientRect();
      return [x, y, width, height];
    };
    const source = document.getElementById('video');
    const canvas = document.querySelector('.crt-display');
    const gl = canvas?.getContext('webgl');
    const sourceSize = [source.width, source.height];
    const canvasSize = canvas ? [canvas.width, canvas.height] : null;
    const drawingBufferSize = gl ? [gl.drawingBufferWidth, gl.drawingBufferHeight] : null;
    const sourceBox = box(source);
    const canvasBox = canvas ? box(canvas) : null;
    const ratio = Math.min(devicePixelRatio, 2);
    const expectedSize = sourceBox.slice(2).map(size => Math.round(size * ratio));
    const output = new Uint8Array(drawingBufferSize ? drawingBufferSize[0] * drawingBufferSize[1] * 4 : 0);
    if (output.length) gl.readPixels(0, 0, ...drawingBufferSize, gl.RGBA, gl.UNSIGNED_BYTE, output);
    return {
      fullscreen: Boolean(document.fullscreenElement), playback: document.getElementById('status').textContent,
      mode: document.getElementById('display').value, fallback: document.getElementById('display-status').textContent,
      sourceLit: lit(source.getContext('2d').getImageData(0, 0, ...sourceSize).data), outputLit: lit(output),
      sourceSize, expectedSize, canvasSize, drawingBufferSize, sourceBox, canvasBox,
      visible: Boolean(canvasBox?.[2] && canvasBox[3] && canvas.checkVisibility({ visibilityProperty: true })),
      geometryMatches: sourceBox.every((value, index) => value === canvasBox?.[index]),
      sizeMatches: expectedSize.every((size, index) => size === canvasSize?.[index] && size === drawingBufferSize?.[index]),
      contextLost: gl?.isContextLost() ?? null, glError: gl?.getError() ?? null,
    };
  });
}

async function renderedPixels(page, mode, testInfo) {
  if (mode === 'pixel') {
    await expect(page.locator('.crt-display')).toBeHidden();
    await expect.poll(() => page.locator('#video').evaluate(canvas =>
      canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data.some((v, i) => i % 4 !== 3 && v > 0)
    )).toBe(true);
  } else {
    let state;
    try {
      await expect.poll(async () => {
        state = await crtState(page);
        return state;
      }).toMatchObject({
        mode: 'crt', visible: true, geometryMatches: true, sizeMatches: true,
        contextLost: false, glError: 0, sourceLit: true, outputLit: true,
      });
    } catch (error) {
      if (!state) throw error;
      const body = JSON.stringify(state);
      await testInfo.attach('fullscreen-crt-readback', { body, contentType: 'application/json' });
      throw new Error(`CRT readback state ${body}`, { cause: error });
    }
  }
}

const insets = { left: 44, right: 44, top: 8, bottom: 21 };
for (const { viewport, touch, safeArea, rotate, capture } of [
  { viewport: { width: 1280, height: 900 }, touch: false, capture: 'desktop' },
  { viewport: { width: 320, height: 740 }, touch: true, capture: 'portrait' },
  { viewport: { width: 375, height: 667 }, touch: true, rotate: { width: 424, height: 320 } },
  { viewport: { width: 667, height: 320 }, touch: true, safeArea: insets, capture: 'landscape' },
]) {
  test.describe(`${viewport.width}×${viewport.height}`, () => {
    test.use({ viewport, hasTouch: touch, isMobile: touch, deviceScaleFactor: touch ? 2 : 1 });
    test('gameplay fullscreen fits Pixel and CRT, restores the player, and keeps touch targets in bounds', async ({ page }, testInfo) => {
      const errors = [];
      page.on('pageerror', error => errors.push(error.message));
      page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
      const session = await page.context().newCDPSession(page);
      if (safeArea) await session.send('Emulation.setSafeAreaInsetsOverride', { insets: safeArea });
      await page.goto('/');
      await selectDemo(page);
      await expect(page.locator('#status')).toHaveText('Playing');
      await renderedPixels(page, 'crt', testInfo);
      await page.keyboard.press('KeyX', { delay: 100 });
      if (capture) await page.waitForTimeout(1800); // Let Shionari's title animation settle.
      const scale = capture ? '3' : '1';
      await page.locator('#scale').selectOption(scale);
      const video = page.locator('#video'), enter = page.locator('#fullscreen'), exit = page.locator('#exit-fullscreen');
      const label = enter.locator('.fullscreen-label');
      const normal = await video.boundingBox();
      for (const mode of ['crt', 'pixel']) {
        await page.locator('#display').selectOption(mode);
        await renderedPixels(page, mode, testInfo);
        if (capture) await page.screenshot({ path: testInfo.outputPath(`${capture}-${mode}-player.png`), scale: 'css' });
        if (touch) await enter.tap();
        else { await enter.focus(); await page.keyboard.press('Enter'); }
        const presentation = page.locator(':fullscreen');
        await expect(presentation).toHaveAttribute('id', 'game-presentation');
        await expect(presentation.locator('header, .toolbar, .game-entry')).toHaveCount(0);
        for (const chrome of await page.locator('header, .toolbar, .game-entry').all()) {
          await expect(chrome).toHaveJSProperty('inert', true);
        }
        await expect(video).toBeFocused();
        await expect(label).toHaveText('Exit fullscreen');
        await gameplayLayout(page, { touch, safeArea });
        await renderedPixels(page, mode, testInfo);
        if (capture) await page.screenshot({ path: testInfo.outputPath(`${capture}-${mode}-fullscreen.png`), scale: 'css' });
        if (rotate && mode === 'crt') {
          for (const { width, height } of [rotate, viewport]) {
            // A fullscreen Chromium window cannot be resized by Playwright.
            // Rotate the emulated device metrics while keeping the element fullscreen.
            const landscape = width > height;
            await session.send('Emulation.setDeviceMetricsOverride', {
              width, height, screenWidth: width, screenHeight: height, mobile: true, deviceScaleFactor: 2,
              screenOrientation: { type: landscape ? 'landscapePrimary' : 'portraitPrimary', angle: landscape ? 90 : 0 },
            });
            await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([width, height]);
            await expect(presentation).toHaveAttribute('id', 'game-presentation');
            await gameplayLayout(page, { touch });
            await renderedPixels(page, mode, testInfo);
          }
        }
        if (touch) {
          await page.keyboard.press('Tab'); await expect(exit).toBeFocused();
          await expect(exit).toHaveCSS('outline-style', 'solid');
          await exit.tap();
        } else await page.keyboard.press('Escape');
        await expect(presentation).toHaveCount(0);
        await expect(touch ? video : enter).toBeFocused();
        await expect(label).toHaveText('Fullscreen');
        await expect(page.locator('#status')).toHaveText('Playing');
        await expect(page.locator('#scale')).toHaveValue(scale);
        expect(await video.boundingBox()).toEqual(normal);
        await renderedPixels(page, mode, testInfo);
        await expect(page.locator('#display')).toHaveValue(mode);
      }
      expect(errors).toEqual([]);
    });
  });
}

test.describe('320×568 portrait', () => {
  const viewport = { width: 320, height: 568 };
  test.use({ viewport, hasTouch: true, isMobile: true });
  test('touch state controls work in portrait and the compact exit restores the toolbar', async ({ page }) => {
    await page.goto('/');
    await selectDemo(page);
    await expect(page.locator('#status')).toHaveText('Playing');
    const enter = page.getByRole('button', { name: 'Fullscreen', exact: true });
    await enter.tap();
    const presentation = page.locator(':fullscreen');
    await expect(presentation).toHaveAttribute('id', 'game-presentation');
    await presentation.locator('#save-state').tap();
    await expect(presentation.locator('#save-status')).toHaveText('State saved.');
    await presentation.locator('#choose-state').tap();
    await presentation.getByRole('button', { name: 'New state' }).tap();
    await expect(presentation.locator('#load-state')).toBeDisabled();
    await gameplayLayout(page, { touch: true });
    await presentation.getByRole('button', { name: 'Exit fullscreen', exact: true }).tap();
    await expect(presentation).toHaveCount(0);
    await expect(enter).toBeVisible();
    expect(await page.evaluate(() => [innerWidth, document.documentElement.scrollWidth]))
      .toEqual([viewport.width, viewport.width]);
    await expect(enter.locator('.fullscreen-icon')).toBeVisible();
  });
});

for (const viewport of [{ width: 500, height: 250 }, { width: 424, height: 320 }, { width: 360, height: 300 }]) {
  test.describe(`${viewport.width}×${viewport.height} landscape safe area`, () => {
    test.use({ viewport, hasTouch: true, isMobile: true });
    test('keeps the video and twelve 44px targets inside the insets before, during and after fullscreen', async ({ page }) => {
      const session = await page.context().newCDPSession(page);
      await session.send('Emulation.setSafeAreaInsetsOverride', { insets });
      await page.goto('/');
      await selectDemo(page);
      await expect(page.locator('#status')).toHaveText('Playing');
      const geometry = { touch: true, safeArea: insets };
      const presentation = page.locator(':fullscreen');
      await gameplayLayout(page, { ...geometry, fullscreen: false });
      await page.locator('#fullscreen').tap();
      await expect(presentation).toHaveAttribute('id', 'game-presentation');
      await gameplayLayout(page, geometry);
      if (viewport.height === 250) {
        await presentation.locator('#save-state').tap();
        await expect(presentation.locator('#save-status')).toHaveText('State saved.');
        await gameplayLayout(page, geometry);
        await presentation.locator('#choose-state').tap();
        await presentation.getByRole('button', { name: /^Select state from/ }).tap();
        await presentation.locator('#load-state').tap();
        await expect(presentation.locator('#save-status')).toContainText('State loaded.');
      }
      await page.locator('#exit-fullscreen').tap();
      await expect(presentation).toHaveCount(0);
      await gameplayLayout(page, { ...geometry, fullscreen: false });
    });
  });
}

for (const unavailable of ['missing', 'rejected']) {
  test(`fullscreen ${unavailable} leaves the normal player usable`, async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 900 });
    await page.addInitScript(unavailable => {
      Element.prototype.requestFullscreen = unavailable === 'missing' ? undefined :
        () => Promise.reject(new DOMException('Fullscreen denied', 'NotAllowedError'));
    }, unavailable);
    await page.goto('/');
    await selectDemo(page);
    await expect(page.locator('#status')).toHaveText('Playing');
    const video = page.locator('#video'), normal = await video.boundingBox();
    await page.locator('#fullscreen').click();
    const warning = page.locator('#fullscreen-status');
    await expect(warning).toHaveText('Fullscreen unavailable. Try another browser.');
    await expect(warning).toBeInViewport({ ratio: 1 });
    await expect(page.locator(':fullscreen')).toHaveCount(0);
    await expect(video).toBeVisible();
    expect(await video.boundingBox()).toEqual(normal);
    await page.locator('#play').click();
    const status = page.locator('#status');
    await expect(status).toHaveText('Paused');
    expect(await status.isVisible()).toBe(true);
    await expect(warning).toBeEmpty();
    if (unavailable === 'rejected') {
      await page.locator('#fullscreen').click();
      await expect(warning).toBeInViewport({ ratio: 1 });
      await expect(warning).toBeEmpty({ timeout: 6000 });
      await expect(status).toBeVisible();
      await expect(status).toHaveText('Paused');
    }
  });
}

for (const viewport of [{ width: 320, height: 740 }, { width: 424, height: 320 }]) {
  test.describe(`${viewport.width}×${viewport.height} fullscreen fallback`, () => {
    test.use({ viewport, hasTouch: true, isMobile: true });
    test('the warning and all twelve touch targets fit without scrolling', async ({ page }) => {
      await page.addInitScript(() => { Element.prototype.requestFullscreen = undefined; });
      await page.goto('/');
      await selectDemo(page);
      await expect(page.locator('#status')).toHaveText('Playing');
      const enter = page.locator('#fullscreen');
      await expect(enter).toBeInViewport({ ratio: 1 });
      await enter.tap();
      const warning = page.locator('#fullscreen-status');
      await expect(warning).toHaveText('Fullscreen unavailable. Try another browser.');
      await expect(warning).toBeInViewport({ ratio: 1 });
      await expect(page.locator(':fullscreen')).toHaveCount(0);
      const buttons = page.locator('[data-button]');
      await expect(buttons).toHaveCount(12);
      for (const button of await buttons.all()) {
        await expect(button).toBeInViewport({ ratio: 1 });
        await expect(button).toBeEnabled();
        expect(await button.evaluate(el => {
          const { x, y, width, height } = el.getBoundingClientRect();
          return el.contains(document.elementFromPoint(x + width / 2, y + height / 2));
        }), await button.getAttribute('aria-label')).toBe(true);
      }
      expect(await page.evaluate(() => [scrollX, scrollY])).toEqual([0, 0]);
      await expect(page.locator('#play')).toHaveText('Pause');
    });
  });
}

test('Escape returns keyboard focus to the scrolled fullscreen button', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 700 });
  await page.goto('/');
  const enter = page.locator('#fullscreen');
  await expect(enter).not.toBeInViewport();
  await enter.focus();
  await expect(enter).toBeInViewport({ ratio: 1 });
  expect(await page.evaluate(() => scrollY)).toBeGreaterThan(0);
  await page.keyboard.press('Enter');
  await expect(page.locator(':fullscreen')).toHaveAttribute('id', 'game-presentation');
  await page.keyboard.press('Escape');
  await expect(page.locator(':fullscreen')).toHaveCount(0);
  await expect(enter).toBeFocused();
  await expect(enter).toBeInViewport({ ratio: 1 });
  await expect(enter).toHaveCSS('outline-style', 'solid');
});

test.describe('normal portrait keyboard focus', () => {
  test.use({ viewport: { width: 375, height: 667 }, hasTouch: true, isMobile: true });
  test('Tab moves from the game through the toolbar and cartridge before the touch controller', async ({ page }) => {
    await page.goto('/');
    await selectDemo(page);
    await expect(page.locator('#status')).toHaveText('Playing');
    await expect(page.locator('#video')).toBeFocused();
    await expect(page.locator('#load-state')).toBeDisabled();
    for (const id of [
      'play', 'reset', 'choose-state', 'save-state', 'fullscreen',
      'mute', 'volume', 'scale', 'display', 'save-data', 'change-cartridge',
    ]) {
      await page.keyboard.press('Tab');
      await expect(page.locator(`#${id}`)).toBeFocused();
    }
    const touchButtons = page.locator('.touch-controls button');
    await expect(touchButtons).toHaveCount(12);
    for (const button of await touchButtons.all()) {
      await page.keyboard.press('Tab');
      await expect(button).toBeFocused();
    }
    await page.locator('#fullscreen').tap();
    await expect(page.locator(':fullscreen')).toHaveAttribute('id', 'game-presentation');
    await page.locator('#exit-fullscreen').tap();
    await expect(page.locator(':fullscreen')).toHaveCount(0);
    // Canvas focus is restored before the toolbar leaves fullscreen's inert state.
    await expect(page.getByRole('button', { name: 'Fullscreen', exact: true })).toBeVisible();
    await expect(page.locator('#video')).toBeFocused();
    await page.keyboard.press('Tab');
    await expect(page.locator('#play')).toBeFocused();
  });

  test('keyboard activation of Exit returns focus to the Fullscreen button', async ({ page }) => {
    await page.goto('/');
    const enter = page.getByRole('button', { name: 'Fullscreen', exact: true });
    const presentation = page.locator(':fullscreen');
    await enter.focus();
    await page.keyboard.press('Enter');
    await expect(presentation).toHaveAttribute('id', 'game-presentation');
    await expect(page.locator('#video')).toBeFocused();
    await page.keyboard.press('Tab');
    await expect(presentation.getByRole('button', { name: 'Exit fullscreen', exact: true })).toBeFocused();
    await page.keyboard.press('Enter');
    await expect(presentation).toHaveCount(0);
    await expect(enter).toBeFocused();
  });
});

test.describe('normal short landscape keyboard focus', () => {
  test.use({ viewport: { width: 424, height: 320 }, hasTouch: true, isMobile: true });
  test('the canvas focus ring is rendered over Pixel and CRT', async ({ page }) => {
    await page.goto('/');
    await selectDemo(page);
    await renderedPixels(page, 'crt');
    await page.locator('#play').tap();
    await expect(page.locator('#status')).toHaveText('Paused');
    const video = page.locator('#video');
    const link = page.getByRole('link', { name: 'GitHub' });
    const edgePixel = async () => {
      const screenshot = await video.screenshot({ scale: 'css' });
      return page.evaluate(async png => {
        const bytes = Uint8Array.from(atob(png), byte => byte.charCodeAt(0));
        const image = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
        const context = new OffscreenCanvas(image.width, image.height).getContext('2d');
        context.drawImage(image, 0, 0);
        return [...context.getImageData(Math.floor(image.width / 2), 1, 1, 1).data];
      }, screenshot.toString('base64'));
    };
    const focusColor = [0x30, 0x6e, 0x59, 0xff];
    for (const mode of ['pixel', 'crt']) {
      await page.locator('#display').selectOption(mode);
      await renderedPixels(page, mode);
      await link.focus();
      expect(await edgePixel(), `${mode} unfocused edge`).not.toEqual(focusColor);
      await page.keyboard.press('Tab');
      await expect(video).toBeFocused();
      expect(await edgePixel(), `${mode} focused edge`).toEqual(focusColor);
    }
  });
});
