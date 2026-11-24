import { expect } from '@playwright/test';

export function setTabHidden(page, hidden) {
  return page.evaluate(hidden => {
    Object.defineProperty(document, 'hidden', { configurable: true, value: hidden });
    document.dispatchEvent(new Event('visibilitychange'));
  }, hidden);
}

export function waitForAnimationFrames(page) {
  return page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
}

// Hardware/replay checks drive frames directly and need the exact reset origin.
// Pause in the same task as Reset so no animation frame can run in between.
export async function resetAndPause(page) {
  const afterReset = await page.evaluate(() => {
    document.getElementById('reset').click();
    const state = document.getElementById('status').textContent;
    document.getElementById('play').click();
    return state;
  });
  expect(afterReset).toBe('Playing');
  await expect(page.locator('#status')).toHaveText('Paused');
}
