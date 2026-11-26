import { test, expect } from '@playwright/test';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { selectDemo } from './cartridge.js';
import { setTabHidden, waitForAnimationFrames } from './playback.js';

const rom = readFileSync(new URL('../../tests/roms/smoke.bin', import.meta.url));
const hash = createHash('sha256').update(rom).digest('hex');
const sramKey = `snes-rust:sram:${hash}`;
const nonBattery = Buffer.from(rom);
nonBattery[0x7fd6] = 0;
nonBattery[0x7fd8] = 0;
const local = (page, buffer = rom, name = 'diagnostic.sfc') => page.locator('#rom').setInputFiles({
  name, mimeType: 'application/octet-stream', buffer,
});
const backupFile = (page, id, name, buffer) => page.locator(id).setInputFiles({
  name, mimeType: 'application/octet-stream', buffer,
});

test.beforeEach(async ({ context }) => {
  await context.addInitScript(() => {
    const instantiate = WebAssembly.instantiate;
    WebAssembly.instantiate = async (...args) => {
      const result = await instantiate(...args);
      window.saveCore = result.instance.exports;
      window.saveFrames = 0;
      return { instance: { exports: { ...saveCore, run_frame(mask) {
        saveFrames++;
        return saveCore.run_frame(mask);
      } } } };
    };
  });
});

async function ready(page) {
  await page.goto('/');
  await expect(page.locator('#status')).toHaveText('Ready');
}

async function pause(page) {
  await expect(page.locator('#status')).toHaveText('Playing');
  await page.locator('#play').click();
  await expect(page.locator('#status')).toHaveText('Paused');
}

async function capture(page) {
  await page.locator('#save-state').click();
  await expect(page.locator('#save-status')).toHaveText('State saved.');
  await expect(page.locator('#save-state')).toBeEnabled();
}

function sram(page, value) {
  return page.evaluate(value => {
    const core = saveCore;
    if (value !== undefined) {
      const pointer = core.input_buffer(core.sram_len());
      new Uint8Array(core.memory.buffer, pointer, core.sram_len()).fill(value);
      if (!core.load_sram()) throw new Error('Could not set fixture SRAM');
    }
    return new Uint8Array(core.memory.buffer, core.sram_buffer(), core.sram_len())[3];
  }, value);
}

function machineState(page) {
  return page.evaluate(() => {
    if (!saveCore.save_snapshot()) throw new Error('Could not snapshot fixture');
    return [...new Uint8Array(saveCore.memory.buffer, saveCore.snapshot_buffer(), saveCore.snapshot_len())];
  });
}

function states(page) {
  return page.getByRole('button', { name: /^Load state from/ });
}

async function fillLocalStorage(page) {
  await page.evaluate(() => {
    // Find the browser's actual quota, then leave no room for a journal entry.
    let low = 0, high = 6 * 1024 * 1024;
    while (low + 1 < high) {
      const middle = Math.floor((low + high) / 2);
      try { localStorage.setItem('quota-test', 'x'.repeat(middle)); low = middle; }
      catch (error) {
        if (error.name !== 'QuotaExceededError') throw error;
        high = middle;
      }
    }
    localStorage.setItem('quota-test', 'x'.repeat(low));
  });
}

test('legacy Save 1, new and duplicate profiles, persisted selection, raw export/import and deletion', async ({ page }) => {
  await ready(page);
  await page.evaluate(([key, value]) => localStorage.setItem(key, value), [sramKey, Buffer.alloc(2048, 0x79).toString('base64')]);
  await local(page);
  await pause(page);
  expect(await sram(page)).toBe(0x79);
  await expect(page.locator('#save-data')).toContainText('Save 1');
  await page.locator('#save-data').click();
  await page.getByRole('button', { name: 'New save', exact: true }).click();
  await expect(page.locator('#save-data')).toContainText('Save 2');
  expect(await sram(page)).toBe(0);
  await sram(page, 0x41);
  const before = await machineState(page);
  await page.locator('#save-data').click();
  await page.getByRole('button', { name: 'Save 2', exact: true }).click();
  expect(await machineState(page)).toEqual(before);

  await page.locator('#save-data').click();
  await page.getByRole('button', { name: 'Duplicate current' }).click();
  await expect(page.locator('#save-data')).toContainText('Save 3');
  expect(await sram(page)).toBe(0x41);
  await sram(page, 0x62);
  await page.locator('#save-data').click();
  const exported = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Export current SRAM' }).click();
  const raw = readFileSync(await (await exported).path());
  expect(raw).toEqual(Buffer.alloc(2048, 0x62));
  await backupFile(page, '#sram-file', 'invalid.srm', raw.subarray(1));
  await expect(page.locator('#save-data-message')).toContainText('exactly 2048 bytes');
  await backupFile(page, '#sram-file', 'copy.srm', raw);
  await expect(page.locator('#save-data-message')).toHaveText('Imported as Save 4. Select it to play.');
  expect(await sram(page)).toBe(0x62);
  await page.getByRole('button', { name: 'Save 4', exact: true }).click();
  await expect(page.locator('#save-data')).toContainText('Save 4');

  await ready(page);
  await local(page, Buffer.concat([Buffer.alloc(512), rom]), 'headered.smc');
  await pause(page);
  expect(await sram(page)).toBe(0x62);
  await expect(page.locator('#save-data')).toContainText('Save 4');
  await page.locator('#save-data').click();
  page.once('dialog', dialog => dialog.accept());
  await page.getByRole('button', { name: 'Delete Save 4', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Save 4', exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Save 1', exact: true })).toHaveAttribute('aria-current', 'true');
  expect(await sram(page)).toBe(0x79);
  await page.getByRole('button', { name: 'Save 2', exact: true }).click();
  await expect(page.locator('#save-data')).toContainText('Save 2');
  expect(await sram(page)).toBe(0x41);
  await page.locator('#reset').click();
  await pause(page);
  expect(await sram(page)).toBe(0x41);
});

test('historical restore protects SRAM, refuses unsafe deletion, and reloads the exact paused machine', async ({ page }) => {
  await ready(page);
  await local(page);
  await pause(page);
  await sram(page, 0x21);
  const historical = await machineState(page);
  await capture(page);
  await page.locator('#save-data').click();
  await page.getByRole('button', { name: 'Delete Save 1', exact: true }).click();
  await expect(page.locator('#save-data-message')).toContainText('Remove the saved states');
  await page.keyboard.press('Escape');

  await page.evaluate(() => { saveCore.run_frame(0x0100); saveCore.run_frame(0x0100); });
  await sram(page, 0x79);
  const prior = await machineState(page);
  await page.evaluate(key => {
    const set = Storage.prototype.setItem;
    Storage.prototype.setItem = function (record, data) {
      if (record.startsWith(key + ':')) throw new DOMException('Quota exceeded', 'QuotaExceededError');
      return set.call(this, record, data);
    };
    window.storageRecovered = () => { Storage.prototype.setItem = set; };
  }, sramKey);
  await page.locator('#load-state').click();
  await states(page).first().click();
  await expect(page.locator('#save-state-message')).toContainText('Could not protect SRAM');
  expect(await machineState(page)).toEqual(prior);
  await page.evaluate(() => storageRecovered());
  const storedBefore = await page.evaluate(key => localStorage.getItem(key), sramKey);
  await page.evaluate(key => {
    const set = Storage.prototype.setItem;
    Storage.prototype.setItem = function (record, data) {
      if (record === key) throw new DOMException('Write denied', 'SecurityError');
      return set.call(this, record, data);
    };
    window.storageRecovered = () => { Storage.prototype.setItem = set; };
  }, sramKey);
  await states(page).first().click();
  await expect(page.locator('#save-state-message')).toContainText('Write denied');
  expect(await machineState(page)).toEqual(prior);
  expect(await page.evaluate(key => localStorage.getItem(key), sramKey)).toBe(storedBefore);
  await page.evaluate(() => storageRecovered());
  await states(page).first().click();
  await expect(page.locator('#save-status')).toContainText('Previous SRAM is available');
  expect(await machineState(page)).toEqual(historical);
  await expect(page.locator('#status')).toHaveText('Paused');
  expect(await sram(page)).toBe(0x21);
  await expect(page.locator('#load-state')).toBeFocused();

  await page.locator('#save-data').click();
  await expect(page.getByRole('button', { name: 'Recovery · Last saved', exact: true })).toHaveCount(1);
  await page.getByRole('button', { name: 'Recovery · Current game', exact: true }).click();
  await expect(page.locator('#save-data-dialog')).toBeHidden();
  expect(await sram(page)).toBe(0x79);
  await page.locator('#load-state').click();
  await expect(page.locator('#save-state-message')).toHaveText('No saved states yet.');
  await page.keyboard.press('Escape');
  await page.locator('#save-data').click();
  await page.getByRole('button', { name: 'Save 1', exact: true }).click();
  await expect(page.locator('#save-data-dialog')).toBeHidden();
  expect(await sram(page)).toBe(0x21);
  await page.locator('#load-state').click();
  await expect(states(page)).toHaveCount(1);
  const downloaded = page.waitForEvent('download');
  await page.getByRole('button', { name: /^Export state from/ }).click();
  const file = readFileSync(await (await downloaded).path());
  expect(file.equals(Buffer.from(historical))).toBe(true);

  const bad = Buffer.from(file);
  bad[8] = 42;
  await backupFile(page, '#state-file', 'future.snesstate', bad);
  await expect(page.locator('#save-state-message')).toContainText('version 42 is unsupported');
  await expect(states(page)).toHaveCount(1);
  await backupFile(page, '#state-file', 'oversize.snesstate', Buffer.alloc(1024 * 1024 + 2048));
  await expect(page.locator('#save-state-message')).toContainText('exceeds the supported size');
  await backupFile(page, '#state-file', 'valid.snesstate', file);
  await expect(page.locator('#save-state-message')).toContainText('State imported');
  await expect(states(page)).toHaveCount(2);
  await states(page).first().click();
  await expect(page.locator('#save-state-dialog')).toBeHidden();
  expect(await machineState(page)).toEqual(historical);

  await local(page, nonBattery, 'no-battery.sfc');
  await pause(page);
  await expect(page.locator('#save-data')).toBeHidden();
  await page.locator('#load-state').click();
  await expect(page.locator('#save-state-message')).toHaveText('No saved states yet.');
  await backupFile(page, '#state-file', 'wrong-rom.snesstate', file);
  await expect(page.locator('#save-state-message')).toContainText('different ROM revision');
});

test('navigation persists dirty SRAM when localStorage has room only to replace the save', async ({ page }) => {
  const prior = Buffer.alloc(2048, 0x21).toString('base64');
  const updated = Buffer.alloc(2048, 0x79).toString('base64');
  await ready(page);
  await page.evaluate(([key, prior]) => localStorage.setItem(key, prior), [sramKey, prior]);
  await local(page);
  await pause(page);
  expect(await sram(page)).toBe(0x21);
  await fillLocalStorage(page);
  await page.evaluate(([key, prior, updated]) => {
    // A same-size replacement succeeds. The exit still cannot add a new key.
    localStorage.setItem(key, updated);
    localStorage.setItem(key, prior);
  }, [sramKey, prior, updated]);
  await sram(page, 0x79);
  await page.goto('/?after-quota-exit');
  const persisted = await page.evaluate(([key, updated]) => localStorage.getItem(key) === updated, [sramKey, updated]);
  expect(persisted).toBe(true);
});

for (const mode of ['paused', 'changed', 'changed at quota']) {
  test(`closing a ${mode} stale tab preserves restored SRAM`, async ({ page, context }) => {
    const dirty = mode !== 'paused';
    const journal = mode === 'changed';
    const stored = () => page.evaluate(key => atob(localStorage.getItem(key)).charCodeAt(3), sramKey);
    async function persist(value) {
      await sram(page, value);
      await page.evaluate(() => window.dispatchEvent(new Event('pagehide')));
      await expect.poll(stored).toBe(value);
    }

    await ready(page);
    await local(page);
    await pause(page);
    await persist(0x21);
    await capture(page);
    if (!dirty) await persist(0x79);

    const stale = await context.newPage();
    await ready(stale);
    await local(stale);
    await pause(stale);
    expect(await sram(stale)).toBe(dirty ? 0x21 : 0x79);
    if (dirty) await persist(0x79);

    await page.locator('#load-state').click();
    await states(page).first().click();
    await expect(page.locator('#save-status')).toContainText('State loaded.');
    expect(await stored()).toBe(0x21);

    if (dirty) await sram(stale, 0x52);
    if (mode === 'changed at quota') await fillLocalStorage(page);
    if (journal) {
      // The restored bytes match this tab's original base. Force page exit
      // while the lock is held so only the synchronous journal can save it.
      await page.evaluate(hash => new Promise(ready => {
        void navigator.locks.request(`snes-rust:saves:${hash}`, () => new Promise(resolve => {
          window.releaseExitSaveLock = resolve;
          ready();
        }));
      }), hash);
    }
    await stale.goto('/?closed-stale-game');
    await stale.close();
    if (journal) {
      await expect.poll(() => page.evaluate(hash => Object.keys(localStorage)
        .filter(key => key.startsWith(`snes-rust:pending-sram:${hash}:`)).length, hash)).toBe(1);
      await page.evaluate(() => releaseExitSaveLock());
    }
    expect(await stored()).toBe(0x21);
    await expect(page.locator('#status')).toHaveText('Paused');
    expect(await sram(page)).toBe(0x21);

    await page.locator('#save-data').click();
    const recovery = page.getByRole('button', { name: 'Recovery · Another tab', exact: true });
    if (journal) {
      await recovery.click();
      await expect(page.locator('#save-data-dialog')).toBeHidden();
      expect(await sram(page)).toBe(0x52);
      expect(await stored()).toBe(0x21);
    } else {
      await expect(recovery).toHaveCount(0);
    }
  });
}

test('105 non-battery captures persist, page thumbnails, and restore arbitrary entries', async ({ page }) => {
  test.setTimeout(60000);
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await ready(page);
  await local(page, nonBattery, 'no-battery.sfc');
  await pause(page);
  await expect(page.locator('#save-data')).toBeHidden();
  const expected = await page.evaluate(async () => {
    // Pause can land on any frame. Start the fixture's palette cycle at reset.
    if (!saveCore.reset()) throw new Error('Could not reset capture fixture');
    const saved = [];
    const button = document.getElementById('save-state');
    const status = document.getElementById('save-status');
    for (let i = 0; i < 105; i++) {
      if (!saveCore.run_frame(i % 4 === 0 ? 0x0100 : 0)) throw new Error('Fixture failed');
      if (!saveCore.save_snapshot()) throw new Error('Capture probe failed');
      const snapshot = new Uint8Array(saveCore.memory.buffer, saveCore.snapshot_buffer(), saveCore.snapshot_len());
      const digest = await crypto.subtle.digest('SHA-256', snapshot);
      const start = performance.now();
      button.click();
      while (button.hasAttribute('aria-disabled')) {
        await new Promise(resolve => setTimeout(resolve, 0));
      }
      if (status.textContent !== 'State saved.') throw new Error(document.getElementById('error').textContent);
      saved.push({ digest: [...new Uint8Array(digest)], milliseconds: performance.now() - start });
    }
    return saved;
  });
  expect(new Set(expected.map(({ digest }) => digest.join(','))).size).toBe(105);
  await ready(page);
  await page.locator('#change-cartridge').click();
  await page.getByRole('button', { name: 'no-battery.sfc', exact: true }).click();
  await pause(page);
  await page.locator('#load-state').click();
  await expect(states(page)).toHaveCount(24);
  for (let pageNumber = 2; pageNumber <= 5; pageNumber++) {
    await page.getByRole('button', { name: 'Show older states' }).click();
    await expect(states(page)).toHaveCount(Math.min(24 * pageNumber, 105));
  }
  await expect(page.getByRole('button', { name: 'Show older states' })).toBeHidden();
  const selected = 57; // Frame 58 after reset is in the bright red part of the palette cycle.
  const image = states(page).nth(104 - selected).locator('img');
  const preview = await image.evaluate(async image => {
    await image.decode();
    const c = document.createElement('canvas');
    c.width = image.naturalWidth;
    c.height = image.naturalHeight;
    const context = c.getContext('2d');
    context.drawImage(image, 0, 0);
    return [...context.getImageData(0, 0, c.width, c.height).data];
  });
  const started = Date.now();
  await states(page).nth(104 - selected).click();
  await expect(page.locator('#save-status')).toHaveText('State loaded.');
  const latency = Date.now() - started;
  const snapshot = Buffer.from(await machineState(page));
  expect([...createHash('sha256').update(snapshot).digest()]).toEqual(expected[selected].digest);
  const raw = await page.evaluate(() => {
    const source = document.querySelector('canvas');
    const resized = document.createElement('canvas');
    resized.width = 128;
    resized.height = 112;
    const ctx = resized.getContext('2d', { alpha: false });
    ctx.drawImage(source, 0, 0, 128, 112);
    return [...ctx.getImageData(0, 0, 128, 112).data];
  });
  const meanError = raw.reduce((sum, value, i) => sum + Math.abs(value - preview[i]), 0) / raw.length;
  expect(raw.some((value, i) => i % 4 !== 3 && value > 100)).toBe(true);
  expect(meanError).toBeLessThan(12);
  await page.locator('#load-state').click();
  await expect(states(page)).toHaveCount(24);
  page.once('dialog', dialog => dialog.accept());
  await page.getByRole('button', { name: /^Remove state from/ }).first().click();
  await expect(states(page)).toHaveCount(23);
  await page.keyboard.press('Escape');
  await page.locator('#change-cartridge').click();
  await page.getByRole('button', { name: 'Remove no-battery.sfc', exact: true }).click();
  await expect(page.getByRole('button', { name: 'no-battery.sfc', exact: true })).toHaveCount(0);
  await ready(page);
  await local(page, nonBattery, 'reimported.sfc');
  await pause(page);
  await page.locator('#load-state').click();
  await expect(states(page)).toHaveCount(24);
  expect(errors).toEqual([]);
  const stats = expected.map(({ milliseconds }) => milliseconds).sort((a, b) => a - b);
  console.log(`105 captures: median ${stats[52].toFixed(1)} ms, p95 ${stats[99].toFixed(1)} ms; one UI load ${latency} ms; selected compressed file ${snapshot.length} bytes`);
});

test('aborted IndexedDB write leaves no orphan entry, and a closed dialog cancels a pending restore', async ({ page }) => {
  await ready(page);
  await local(page, nonBattery, 'no-battery.sfc');
  await pause(page);
  await page.evaluate(() => {
    const add = IDBObjectStore.prototype.add;
    IDBObjectStore.prototype.add = function (...args) {
      const request = add.apply(this, args);
      if (this.name === 'snapshots') {
        request.addEventListener('success', () => this.transaction.abort(), { once: true });
      }
      return request;
    };
    window.restoreIdb = () => { IDBObjectStore.prototype.add = add; };
  });
  await page.locator('#save-state').click();
  await expect(page.locator('#error')).toContainText('transaction aborted');
  await page.locator('#load-state').click();
  await expect(page.locator('#save-state-message')).toHaveText('No saved states yet.');
  await page.keyboard.press('Escape');
  await page.evaluate(() => restoreIdb());
  await capture(page);
  await page.evaluate(() => saveCore.run_frame(0x0100));
  const newer = await machineState(page);
  await page.locator('#load-state').click();
  await page.evaluate(() => {
    const request = LockManager.prototype.request;
    LockManager.prototype.request = async function (...args) {
      await new Promise(resolve => { window.releaseSaveLock = resolve; });
      return request.apply(this, args);
    };
  });
  await states(page).first().click();
  await expect.poll(() => page.evaluate(() => typeof releaseSaveLock)).toBe('function');
  await page.keyboard.press('Escape');
  await page.evaluate(() => releaseSaveLock());
  await expect(page.locator('#load-state')).toBeEnabled();
  expect(await machineState(page)).toEqual(newer);
});

test('bundled cartridge snapshots respect visibility and paused intent on narrow and short touch layouts', async ({ browser }) => {
  const context = await browser.newContext({ hasTouch: true, viewport: { width: 375, height: 667 } });
  try {
    const page = await context.newPage();
    await ready(page);
    await selectDemo(page);
    await expect(page.locator('#status')).toHaveText('Playing');
    await page.locator('#save-state').tap();
    await expect(page.locator('#save-status')).toHaveText('State saved.');
    await page.locator('#load-state').tap();
    await expect(states(page)).toHaveCount(1);
    await setTabHidden(page, true);
    await states(page).first().tap();
    await expect(page.locator('#status')).toHaveText('Suspended');
    await setTabHidden(page, false);
    await expect(page.locator('#status')).toHaveText('Playing');
    await page.setViewportSize({ width: 667, height: 375 });
    await pause(page);
    await page.locator('#load-state').tap();
    await expect(page.locator('#save-state-dialog')).toBeInViewport({ ratio: 1 });
    await states(page).first().tap();
    await expect(page.locator('#status')).toHaveText('Paused');
    await waitForAnimationFrames(page);
    await expect(page.locator('#status')).toHaveText('Paused');
    await expect(page.locator('[data-button]')).toHaveCount(12);
  } finally {
    await context.close();
  }
});

test('production Wasm replays both demos deterministically and clears delivered audio on restore', async ({ page }) => {
  await ready(page);
  for (const [id, inputs] of [
    ['demo', [[70, 76, 128], [184, 190, 128], [212, 218, 128]]],
    ['demo-yonaki', [[146, 152, 128], [182, 188, 4096], [215, 221, 4096]]],
  ]) {
    await selectDemo(page, id);
    await pause(page);
    const result = await page.evaluate(inputs => {
      const core = saveCore;
      if (!core.reset()) throw new Error('Reset failed');
      const bytes = (ptr, len) => new Uint8Array(core.memory.buffer, ptr, len).slice();
      function field(mask) {
        if (!core.run_frame(mask)) throw new Error('Demo execution failed');
      }
      function snapshot() {
        if (!core.save_snapshot()) throw new Error('Save failed');
        return bytes(core.snapshot_buffer(), core.snapshot_len());
      }
      function prepare(data) {
        const pointer = core.input_buffer(data.length);
        new Uint8Array(core.memory.buffer, pointer, data.length).set(data);
        const start = performance.now();
        if (!core.prepare_snapshot()) throw new Error(new TextDecoder().decode(bytes(core.error_buffer(), core.error_len())));
        return performance.now() - start;
      }
      for (let i = 0; i < 245; i++) {
        field(inputs.find(([start, end]) => i >= start && i < end)?.[2] ?? 0);
      }
      const snapshotStart = performance.now();
      const saved = snapshot();
      const encode = performance.now() - snapshotStart;
      const maskSequence = [128, 128, 0, 4096];
      function replay() {
        return maskSequence.map(mask => {
          field(mask);
          return {
            audio: [...new Float32Array(core.memory.buffer, core.audio_buffer(), core.audio_len())],
            state: [...snapshot()],
          };
        });
      }
      const expected = replay();
      const decode = prepare(saved);
      if (!core.commit_prepared_state() || core.audio_len()) throw new Error('Audio was not cleared at commit');
      return {
        exact: JSON.stringify(replay()) === JSON.stringify(expected),
        audible: expected.some(frame => frame.audio.some(value => value !== 0)),
        size: saved.length, encode, decode,
      };
    }, inputs);
    expect(result).toMatchObject({ exact: true, audible: true });
    console.log(`${id}: ${result.size} bytes; Wasm encode ${result.encode.toFixed(1)} ms, decode ${result.decode.toFixed(1)} ms`);
  }
});

test('unavailable save-state storage reports the failure and keeps the cartridge playable', async ({ page }) => {
  await page.addInitScript(() => Object.defineProperty(window, 'indexedDB', { value: undefined }));
  await ready(page);
  await local(page);
  await expect(page.locator('#status')).toHaveText('Playing');
  await page.locator('#save-state').click();
  await expect(page.locator('#error')).toContainText('Browser save state storage is unavailable');
  await expect(page.locator('#save-status')).toBeEmpty();
  await expect(page.locator('#load-state')).toBeEnabled();
  await page.locator('#load-state').click();
  await expect(page.locator('#save-state-message')).toContainText('Browser save state storage is unavailable');
  await page.keyboard.press('Escape');
  await pause(page);
  await page.locator('#save-data').click();
  await page.getByRole('button', { name: 'New save', exact: true }).click();
  await expect(page.locator('#save-data')).toContainText('Save 2');
});

test('a playing restore flushes old Web Audio and releases held input while keeping player settings', async ({ page }) => {
  await page.addInitScript(() => {
    const instantiate = WebAssembly.instantiate;
    const start = AudioBufferSourceNode.prototype.start;
    const stop = AudioBufferSourceNode.prototype.stop;
    const active = new Set();
    window.audioRestore = { active: 0, stopped: 0, committed: false, buttons: 0 };
    AudioBufferSourceNode.prototype.start = function (...args) {
      const result = start.apply(this, args);
      active.add(this);
      audioRestore.active = active.size;
      this.addEventListener('ended', () => {
        active.delete(this);
        audioRestore.active = active.size;
      }, { once: true });
      return result;
    };
    AudioBufferSourceNode.prototype.stop = function (...args) {
      if (audioRestore.committed && active.has(this)) audioRestore.stopped++;
      return stop.apply(this, args);
    };
    WebAssembly.instantiate = async (...args) => {
      const result = await instantiate(...args);
      const core = result.instance.exports;
      return { instance: { exports: { ...core,
        run_frame(mask) {
          audioRestore.buttons = mask;
          return core.run_frame(mask);
        },
        commit_prepared_state() {
          const committed = core.commit_prepared_state();
          audioRestore.committed = Boolean(committed);
          return committed;
        },
      } } };
    };
  });
  await ready(page);
  await local(page);
  await pause(page);
  await capture(page);
  await page.locator('#display').selectOption('pixel');
  const volume = page.locator('#volume');
  await volume.press('ArrowLeft');
  const level = await volume.inputValue();
  await page.locator('#play').click();
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect.poll(() => page.evaluate(() => audioRestore.active)).toBeGreaterThan(0);
  await page.locator('#video').focus();
  await page.keyboard.down('ArrowRight');
  await expect.poll(() => page.evaluate(() => audioRestore.buttons)).toBe(0x0100);
  await page.locator('#load-state').click();
  await states(page).first().click();
  await expect(page.locator('#save-state-dialog')).toBeHidden();
  await expect.poll(() => page.evaluate(() => audioRestore.buttons)).toBe(0);
  expect(await page.evaluate(() => audioRestore.stopped)).toBeGreaterThan(0);
  await page.keyboard.up('ArrowRight');
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('#display')).toHaveValue('pixel');
  await expect(volume).toHaveValue(level);
});
