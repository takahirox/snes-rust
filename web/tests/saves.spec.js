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
  return page.getByRole('button', { name: /^Select state from/ });
}

function savedHistory(page, buffer = nonBattery, profile = '') {
  return page.evaluate(async scope => {
    const { historyPage } = await import('/saves.js');
    const { entries } = await historyPage(scope);
    return Promise.all(entries.map(async ({ key, thumbnail }) => ({
      key, thumbnail: [...new Uint8Array(await thumbnail.arrayBuffer())],
    })));
  }, { hash: createHash('sha256').update(buffer).digest('hex'), profile });
}

async function fillStorage(page, name = 'localStorage') {
  await page.evaluate(name => {
    const storage = window[name];
    // Find the browser's actual quota, then leave no room for a journal entry.
    let low = 0, high = 6 * 1024 * 1024;
    while (low + 1 < high) {
      const middle = Math.floor((low + high) / 2);
      try { storage.setItem('quota-test', 'x'.repeat(middle)); low = middle; }
      catch (error) {
        if (error.name !== 'QuotaExceededError') throw error;
        high = middle;
      }
    }
    storage.setItem('quota-test', 'x'.repeat(low));
  }, name);
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
  const destination = page.locator('#state-target');
  const selected = await destination.getAttribute('title');
  await page.locator('#load-state').click();
  await expect(page.locator('#error')).toContainText('Could not protect SRAM');
  await expect(destination).toHaveAttribute('title', selected);
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
  await page.locator('#load-state').click();
  await expect(page.locator('#error')).toContainText('Write denied');
  expect(await machineState(page)).toEqual(prior);
  expect(await page.evaluate(key => localStorage.getItem(key), sramKey)).toBe(storedBefore);
  await page.evaluate(() => storageRecovered());
  await page.locator('#load-state').click();
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
  await expect(page.locator('#load-state')).toBeDisabled();
  await page.locator('#choose-state').click();
  await expect(page.locator('#state-message')).toHaveText('No saved states yet.');
  await page.keyboard.press('Escape');
  await page.locator('#save-data').click();
  await page.getByRole('button', { name: 'Save 1', exact: true }).click();
  await expect(page.locator('#save-data-dialog')).toBeHidden();
  expect(await sram(page)).toBe(0x21);
  await page.locator('#choose-state').click();
  await expect(states(page)).toHaveCount(1);
  await expect(states(page).first()).toHaveAttribute('aria-current', 'true');
  const downloaded = page.waitForEvent('download');
  await page.getByRole('button', { name: /^Export state from/ }).click();
  const file = readFileSync(await (await downloaded).path());
  expect(file.equals(Buffer.from(historical))).toBe(true);

  await page.evaluate(() => saveCore.run_frame(0x0100));
  const beforeImport = await machineState(page);
  const bad = Buffer.from(file);
  bad[8] = 42;
  await backupFile(page, '#state-file', 'future.snesstate', bad);
  await expect(page.locator('#state-message')).toContainText('version 42 is unsupported');
  await expect(states(page)).toHaveCount(1);
  await backupFile(page, '#state-file', 'oversize.snesstate', Buffer.alloc(1024 * 1024 + 2048));
  await expect(page.locator('#state-message')).toContainText('exceeds the supported size');
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.getByRole('button', { name: 'Import state', exact: true }).click(),
  ]);
  await chooser.setFiles({ name: 'valid.snesstate', mimeType: 'application/octet-stream', buffer: file });
  await expect(page.locator('#state-message')).toContainText('State imported');
  await expect(states(page)).toHaveCount(2);
  await expect(destination).toHaveAttribute('title', selected);
  await expect(states(page).last()).toHaveAttribute('aria-current', 'true');
  expect(await machineState(page)).toEqual(beforeImport);
  await states(page).first().click();
  await expect(page.locator('#state-picker')).toBeHidden();
  expect(await machineState(page)).toEqual(beforeImport);
  await page.locator('#load-state').click();
  await expect(page.locator('#save-status')).toHaveText('State loaded.');
  expect(await machineState(page)).toEqual(historical);

  await local(page, nonBattery, 'no-battery.sfc');
  await pause(page);
  await expect(page.locator('#save-data')).toBeHidden();
  await page.locator('#choose-state').click();
  await expect(page.locator('#state-message')).toHaveText('No saved states yet.');
  await backupFile(page, '#state-file', 'wrong-rom.snesstate', file);
  await expect(page.locator('#state-message')).toContainText('different ROM revision');
});

test('a failed state load leaves another tab able to autosave to Save 1', async ({ page, context }) => {
  const stored = () => page.evaluate(key => atob(localStorage.getItem(key)).charCodeAt(3), sramKey);
  await ready(page);
  await page.evaluate(([key, data]) => localStorage.setItem(key, data), [sramKey, Buffer.alloc(2048, 0x79).toString('base64')]);
  await local(page);
  await pause(page);
  await sram(page, 0x21);
  await capture(page);
  await sram(page, 0x79);

  const other = await context.newPage();
  await ready(other);
  await local(other);
  await pause(other);
  expect(await sram(other)).toBe(0x79);
  await page.evaluate(key => {
    const set = Storage.prototype.setItem;
    Storage.prototype.setItem = function (record, data) {
      if (record === key) throw new DOMException('Write denied', 'SecurityError');
      return set.call(this, record, data);
    };
  }, sramKey);
  await page.locator('#load-state').click();
  await expect(page.locator('#error')).toContainText('Write denied');
  expect(await stored()).toBe(0x79);
  expect(await sram(page)).toBe(0x79);

  await sram(other, 0x52);
  await other.evaluate(() => window.dispatchEvent(new Event('pagehide')));
  await expect.poll(stored).toBe(0x52);
  await other.locator('#save-data').click();
  await expect(other.getByRole('button', { name: 'Recovery · Another tab', exact: true })).toHaveCount(0);
});

test('a same-byte state load preserves a pending exit save without changing Save 1 on replay or reload', async ({ page }) => {
  const saved = Buffer.alloc(2048, 0x21).toString('base64');
  const stored = () => page.evaluate(key => atob(localStorage.getItem(key)).charCodeAt(3), sramKey);
  const pending = () => page.evaluate(hash => Object.keys(localStorage)
    .filter(key => key.startsWith(`snes-rust:pending-sram:${hash}:`)).length, hash);
  await ready(page);
  await page.evaluate(([key, data]) => localStorage.setItem(key, data), [sramKey, saved]);
  await local(page);
  await pause(page);
  await capture(page);

  const exitSave = Buffer.alloc(2048, 0x79).toString('base64');
  await page.evaluate(([key, exitSave]) => {
    const set = Storage.prototype.setItem;
    Storage.prototype.setItem = function (record, data) {
      if (record === key || (record.startsWith(key + ':') && JSON.parse(data).data === exitSave)) {
        throw new DOMException('Write denied', 'SecurityError');
      }
      return set.call(this, record, data);
    };
    window.storageRecovered = () => { Storage.prototype.setItem = set; };
  }, [sramKey, exitSave]);
  await sram(page, 0x79);
  await page.evaluate(() => window.dispatchEvent(new Event('pagehide')));
  await expect(page.locator('#error')).toContainText('Could not persist SRAM');
  expect(await pending()).toBe(1);
  expect(await stored()).toBe(0x21);

  // The denied exit write is now the only copy of 0x79.
  await sram(page, 0x52);
  await page.locator('#load-state').click();
  await expect(page.locator('#error')).toContainText('Could not protect SRAM');
  expect(await sram(page)).toBe(0x52);
  expect(await pending()).toBe(1);
  expect(await stored()).toBe(0x21);

  await page.evaluate(() => storageRecovered());
  await page.locator('#load-state').click();
  await expect(page.locator('#save-status')).toContainText('Previous SRAM is available');
  await page.locator('#save-data').click();
  await expect(page.getByRole('button', { name: 'Save 1', exact: true })).toHaveAttribute('aria-current', 'true');
  await expect.poll(pending).toBe(0);
  expect(await stored()).toBe(0x21);
  expect(await sram(page)).toBe(0x21);

  await page.reload();
  await expect(page.locator('#status')).toHaveText('Ready');
  await local(page);
  await pause(page);
  await page.locator('#save-data').click();
  await expect(page.getByRole('button', { name: 'Save 1', exact: true })).toHaveAttribute('aria-current', 'true');
  expect(await sram(page)).toBe(0x21);
  await page.getByRole('button', { name: 'Recovery · Pending save', exact: true }).click();
  await expect(page.locator('#save-data-dialog')).toBeHidden();
  expect(await sram(page)).toBe(0x79);
  expect(await stored()).toBe(0x21);
});

test('state loads protect a reused recovery profile from its stale tab', async ({ page, context }) => {
  const stored = () => page.evaluate(key => atob(localStorage.getItem(key)).charCodeAt(3), sramKey);
  await ready(page);
  await page.evaluate(([key, value]) => localStorage.setItem(key, value), [sramKey, Buffer.alloc(2048, 0x21).toString('base64')]);
  await local(page);
  await pause(page);
  await capture(page);

  const stale = await context.newPage();
  await ready(stale);
  await local(stale);
  await pause(stale);
  await sram(page, 0x41);
  await page.evaluate(() => window.dispatchEvent(new Event('pagehide')));
  await expect.poll(stored).toBe(0x41);
  await sram(stale, 0x79);
  await stale.evaluate(() => window.dispatchEvent(new Event('pagehide')));
  await expect(stale.locator('#save-status')).toContainText('This tab’s progress is in Recovery');
  await sram(page, 0x79);
  await page.evaluate(() => window.dispatchEvent(new Event('pagehide')));
  await expect.poll(stored).toBe(0x79);

  // The matching bytes already exist. If protecting their profile fails, the
  // current machine and Save 1 must both survive the attempted load.
  const prior = await machineState(page);
  await page.evaluate(hash => {
    const set = Storage.prototype.setItem;
    Storage.prototype.setItem = function (key, value) {
      if (key === `snes-rust:save-profiles:${hash}` && JSON.parse(value).profiles.some(profile => profile.recovery && profile.epoch)) {
        throw new DOMException('Write denied', 'SecurityError');
      }
      return set.call(this, key, value);
    };
    window.storageRecovered = () => { Storage.prototype.setItem = set; };
  }, hash);
  await page.locator('#load-state').click();
  await expect(page.locator('#error')).toContainText('Could not protect SRAM');
  expect(await machineState(page)).toEqual(prior);
  expect(await stored()).toBe(0x79);

  await page.evaluate(() => storageRecovered());
  await page.locator('#load-state').click();
  await expect(page.locator('#save-status')).toContainText('Previous SRAM is available');
  expect(await stored()).toBe(0x21);
  await page.locator('#save-data').click();
  const copies = page.getByRole('button', { name: /^Recovery · / });
  await expect(copies).toHaveCount(1);
  await page.keyboard.press('Escape');

  await sram(stale, 0x52);
  await stale.evaluate(() => window.dispatchEvent(new Event('pagehide')));
  await stale.evaluate(hash => navigator.locks.request(`snes-rust:saves:${hash}`, () => {}), hash);
  await page.locator('#save-data').click();
  await expect(copies).toHaveCount(2);
  await copies.first().click();
  await expect(page.locator('#save-data-dialog')).toBeHidden();
  expect(await sram(page)).toBe(0x79);
  expect(await stored()).toBe(0x21);
});

for (const contention of [false, true]) test(`navigation preserves dirty SRAM at localStorage quota ${contention ? 'with' : 'without'} lock contention`, async ({ page, context }) => {
  const prior = Buffer.alloc(2048, 0x21).toString('base64');
  const updated = Buffer.alloc(2048, 0x79).toString('base64');
  await ready(page);
  await page.evaluate(([key, prior]) => localStorage.setItem(key, prior), [sramKey, prior]);
  await local(page);
  await pause(page);
  expect(await sram(page)).toBe(0x21);
  await fillStorage(page);
  await page.evaluate(([key, prior, updated]) => {
    // A same-size replacement succeeds. The exit still cannot add a new key.
    localStorage.setItem(key, updated);
    localStorage.setItem(key, prior);
    try { localStorage.setItem('extra-key', 'x'); }
    catch (error) {
      if (error.name === 'QuotaExceededError') return;
      throw error;
    }
    throw new Error('Expected localStorage to reject an additional key');
  }, [sramKey, prior, updated]);
  const holder = contention ? await context.newPage() : null;
  if (holder) {
    await ready(holder);
    await holdSaveLock(holder);
  }
  await sram(page, 0x79);
  await page.goto('/?after-quota-exit');
  if (holder) {
    expect(await holder.evaluate(key => localStorage.getItem(key), sramKey)).toBe(prior);
    await holder.evaluate(() => releaseExitSaveLock());
    await expect(page.locator('#status')).toHaveText('Ready');
    await local(page);
    await expect(page.locator('#status')).toHaveText('Playing');
    expect(await sram(page)).toBe(0x79);
  }
  expect(await page.evaluate(key => localStorage.getItem(key), sramKey)).toBe(updated);
});

function holdSaveLock(page, romHash = hash) {
  return page.evaluate(hash => new Promise(ready => {
    void navigator.locks.request(`snes-rust:saves:${hash}`, () => new Promise(resolve => {
      window.releaseExitSaveLock = resolve;
      ready();
    }));
  }), romHash);
}

test('closing a dirty tab at quota warns while another tab holds the save lock', async ({ page, context }) => {
  const prior = Buffer.alloc(2048, 0x21).toString('base64');
  const updated = Buffer.alloc(2048, 0x79).toString('base64');
  await ready(page);
  await page.evaluate(([key, data]) => localStorage.setItem(key, data), [sramKey, prior]);
  await local(page);
  await pause(page);
  await fillStorage(page);
  const holder = await context.newPage();
  await ready(holder);
  await holdSaveLock(holder);
  await sram(page, 0x79);

  const warning = page.waitForEvent('dialog', { timeout: 5000 });
  await page.close({ runBeforeUnload: true });
  const dialog = await warning;
  expect(dialog.type()).toBe('beforeunload');
  await dialog.dismiss();
  const stored = () => holder.evaluate(key => localStorage.getItem(key), sramKey);
  expect(await stored()).toBe(prior);
  expect(await sram(page)).toBe(0x79);

  await holder.evaluate(() => releaseExitSaveLock());
  await expect.poll(stored).toBe(updated);
  const closed = page.waitForEvent('close');
  await page.close({ runBeforeUnload: true });
  await closed;
  await local(holder);
  await expect(holder.locator('#status')).toHaveText('Playing');
  expect(await sram(holder)).toBe(0x79);
});

for (const exit of ['close', 'navigate', 'cancel']) test(`durable SRAM survives a session-only replacement at quota (${exit})`, async ({ page, context }) => {
  const largeRom = Buffer.from(rom);
  largeRom[0x7fd8] = 8; // 256 KiB SRAM.
  const largeHash = createHash('sha256').update(largeRom).digest('hex');
  const key = `snes-rust:sram:${largeHash}`;
  const image = value => Buffer.alloc(256 * 1024, value).toString('base64');
  await ready(page);
  await page.evaluate(([key, data]) => localStorage.setItem(key, data), [key, image(0x21)]);
  await local(page, largeRom);
  await pause(page);
  const holder = await context.newPage();
  await ready(holder);
  await holdSaveLock(holder, largeHash);

  await sram(page, 0x50);
  await page.evaluate(() => window.dispatchEvent(new Event('pagehide')));
  const durableJournals = () => holder.evaluate(hash => Object.keys(localStorage)
    .filter(key => key.startsWith(`snes-rust:pending-sram:${hash}:`)).length, largeHash);
  expect(await durableJournals()).toBe(1);
  await fillStorage(holder);
  await sram(page, 0x61);
  await page.evaluate(() => window.dispatchEvent(new Event('pagehide')));
  await sram(page, 0x72);

  const warning = page.waitForEvent('dialog', { timeout: 5000 });
  const exited = exit === 'navigate' ? page.goto('/?after-journal-replacement')
    : page.close({ runBeforeUnload: true });
  const closed = exit === 'close' ? page.waitForEvent('close') : undefined;
  const dialog = await warning;
  expect(dialog.type()).toBe('beforeunload');
  if (exit === 'cancel') await dialog.dismiss();
  else await dialog.accept();
  await exited;
  await closed;

  expect(await holder.evaluate(key => localStorage.getItem(key), key)).toBe(image(0x21));
  await holder.evaluate(() => releaseExitSaveLock());
  const reopened = exit === 'close' ? holder : page;
  if (exit !== 'cancel') {
    await expect(reopened.locator('#status')).toHaveText('Ready');
    await local(reopened, largeRom);
    await expect(reopened.locator('#status')).toHaveText('Playing');
  }
  const expected = exit === 'close' ? 0x50 : 0x72;
  expect(await sram(reopened)).toBe(expected);
  await expect.poll(() => holder.evaluate(key => localStorage.getItem(key), key)).toBe(image(expected));
  expect(await durableJournals()).toBe(0);
});

test.describe('large SRAM journal replacement across navigation', () => {
  const largeRom = Buffer.from(rom);
  largeRom[0x7fd8] = 8; // 256 KiB SRAM.
  const largeHash = createHash('sha256').update(largeRom).digest('hex');
  const key = `snes-rust:sram:${largeHash}`;
  const image = value => Buffer.alloc(256 * 1024, value).toString('base64');
  const pagehide = page => page.evaluate(() => window.dispatchEvent(new Event('pagehide')));
  const pending = (page, storage) => page.evaluate(([storage, hash]) => Object.keys(window[storage])
    .filter(key => key.startsWith(`snes-rust:pending-sram:${hash}:`)), [storage, largeHash]);

  test.beforeEach(async ({ page }) => {
    await ready(page);
    await page.evaluate(([key, data]) => localStorage.setItem(key, data), [key, image(0x21)]);
    await local(page, largeRom);
    await pause(page);
    page.on('dialog', dialog => dialog.accept());
  });

  test('the latest session-only journal replaces a same-size entry at quota', async ({ page, context }) => {
    const holder = await context.newPage();
    await ready(holder);
    await holdSaveLock(holder, largeHash);
    await fillStorage(holder);
    await sram(page, 0x50);
    await pagehide(page);
    const staged = await pending(page, 'sessionStorage');
    expect(staged).toHaveLength(1);
    expect(await pending(page, 'localStorage')).toHaveLength(0);
    await fillStorage(page, 'sessionStorage');
    await sram(page, 0x72);
    await page.goto('/?replace-session-journal');
    expect(await pending(page, 'sessionStorage')).toEqual(staged);
    expect(await holder.evaluate(key => localStorage.getItem(key), key)).toBe(image(0x21));

    await holder.evaluate(() => releaseExitSaveLock());
    await local(page, largeRom);
    await pause(page);
    expect(await sram(page)).toBe(0x72);
    expect(await page.evaluate(key => localStorage.getItem(key), key)).toBe(image(0x72));
    expect(await pending(page, 'sessionStorage')).toHaveLength(0);
  });

  for (const intervening of ['none', 'independent save', 'same-byte state restore']) {
    test(`a successor to a durable replay survives navigation at quota (${intervening})`, async ({ page, context }) => {
      // The writer navigates away before its delayed lifecycle callbacks run.
      await page.evaluate(hash => {
        const lock = navigator.locks.request.bind(navigator.locks);
        navigator.locks.request = (name, callback) => name === `snes-rust:saves:${hash}`
          ? new Promise(() => {}) : lock(name, callback);
      }, largeHash);
      await sram(page, 0x50);
      await pagehide(page);
      const durable = await pending(page, 'localStorage');
      expect(durable).toHaveLength(1);
      await fillStorage(page);
      await sram(page, 0x72);
      await pagehide(page);
      expect(await pending(page, 'sessionStorage')).toEqual(durable);

      const other = await context.newPage();
      await ready(other);
      await local(other, largeRom);
      await pause(other);
      expect(await sram(other)).toBe(0x50);
      expect(await pending(other, 'localStorage')).toHaveLength(0);
      const stored = () => other.evaluate(key => localStorage.getItem(key), key);
      if (intervening === 'independent save') {
        // Returning to the exact replayed bytes must not give its writer
        // permission to overwrite this tab's independent save.
        for (const value of [0x51, 0x50]) {
          await sram(other, value);
          await pagehide(other);
          await expect.poll(stored).toBe(image(value));
        }
      } else if (intervening === 'same-byte state restore') {
        await capture(other);
        await other.locator('#load-state').click();
        await expect(other.locator('#save-status')).toHaveText('State loaded.');
      }
      await fillStorage(other);
      await page.goto('/?after-durable-replay');
      await local(page, largeRom);
      await pause(page);

      const expected = intervening === 'none' ? 0x72 : 0x50;
      expect(await sram(page)).toBe(expected);
      expect(await stored()).toBe(image(expected));
      if (intervening !== 'none') {
        expect(await pending(page, 'sessionStorage')).toEqual(durable);
        await other.evaluate(() => localStorage.removeItem('quota-test'));
        await page.locator('#save-data').click();
        await page.getByRole('button', { name: 'Recovery · Another tab', exact: true }).click();
        await expect(page.locator('#save-data-dialog')).toBeHidden();
        expect(await sram(page)).toBe(0x72);
        expect(await stored()).toBe(image(0x50));
      }
      expect(await pending(page, 'sessionStorage')).toHaveLength(0);
    });
  }
});

test('closing after navigation still warns about a session-only SRAM journal at quota', async ({ page, context }) => {
  const prior = Buffer.alloc(2048, 0x21).toString('base64');
  const newer = Buffer.alloc(2048, 0x43).toString('base64');
  await ready(page);
  await page.evaluate(([key, data]) => localStorage.setItem(key, data), [sramKey, prior]);
  await local(page);
  await pause(page);
  const other = await context.newPage();
  await ready(other);
  await local(other);
  await pause(other);
  await sram(other, 0x43);
  await other.evaluate(() => window.dispatchEvent(new Event('pagehide')));
  await expect.poll(() => other.evaluate(key => localStorage.getItem(key), sramKey)).toBe(newer);
  await fillStorage(other);

  await sram(page, 0x79);
  await page.evaluate(() => window.dispatchEvent(new Event('pagehide')));
  await expect(page.locator('#error')).toContainText('Could not persist SRAM');
  const pending = () => page.evaluate(() => Object.keys(sessionStorage)
    .filter(key => key.startsWith('snes-rust:pending-sram:')).length);
  expect(await pending()).toBe(1);
  const acceptNavigation = dialog => dialog.accept();
  page.on('dialog', acceptNavigation);
  await page.goto('/?session-only-recovery');
  page.off('dialog', acceptNavigation);
  await expect(page.locator('#status')).toHaveText('Ready');
  const warnedWithoutCartridge = await page.evaluate(() =>
    !window.dispatchEvent(new Event('beforeunload', { cancelable: true })));

  await local(page);
  await pause(page);
  expect(await sram(page)).toBe(0x43);
  await expect(page.locator('#save-status')).toContainText('Open Save data to retry');
  expect(await pending()).toBe(1);
  const warning = page.waitForEvent('dialog', { timeout: 5000 });
  await page.close({ runBeforeUnload: true });
  const dialog = await warning;
  expect(dialog.type()).toBe('beforeunload');
  await dialog.dismiss();
  expect(warnedWithoutCartridge).toBe(true);
  expect(await pending()).toBe(1);

  await other.evaluate(() => localStorage.removeItem('quota-test'));
  await page.locator('#save-data').click();
  await page.getByRole('button', { name: 'Recovery · Another tab', exact: true }).click();
  await expect(page.locator('#save-data-dialog')).toBeHidden();
  expect(await sram(page)).toBe(0x79);
  expect(await pending()).toBe(0);
  const closed = page.waitForEvent('close');
  await page.close({ runBeforeUnload: true });
  await closed;
});

for (const mode of ['paused', 'changed', 'changed at quota', 'changed after a same-byte restore']) {
  test(`closing a stale tab preserves restored SRAM (${mode})`, async ({ page, context }) => {
    const dirty = mode !== 'paused';
    const sameByteRestore = mode === 'changed after a same-byte restore';
    const journal = mode === 'changed' || sameByteRestore;
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
    if (dirty && !sameByteRestore) await persist(0x79);

    await page.locator('#load-state').click();
    await expect(page.locator('#save-status')).toContainText('State loaded.');
    expect(await stored()).toBe(0x21);

    if (dirty) await sram(stale, 0x52);
    if (mode === 'changed at quota') await fillStorage(page);
    if (journal) {
      // The restored bytes match this tab's original base. Force page exit
      // while the lock is held so only the synchronous journal can save it.
      await holdSaveLock(page);
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

for (const next of ['pause at quota', 'pagehide at quota', 'another write', 'same-byte restore']) {
  test(`a writer reconciles a replay into Save 1 before its next save (${next})`, async ({ page, context }) => {
    const largeRom = Buffer.from(rom);
    largeRom[0x7fd8] = 8; // 256 KiB SRAM.
    const largeHash = createHash('sha256').update(largeRom).digest('hex');
    const key = `snes-rust:sram:${largeHash}`;
    const stored = () => page.evaluate(key => atob(localStorage.getItem(key)).charCodeAt(3), key);
    await ready(page);
    await page.evaluate(([key, prior]) => localStorage.setItem(key, prior),
      [key, Buffer.alloc(256 * 1024, 0x21).toString('base64')]);
    await local(page, largeRom);
    await pause(page);

    // Stage the exit journal, then let the other tab replay it before the
    // originating tab gets its turn to acquire the ROM lock.
    await page.evaluate(hash => {
      const lock = navigator.locks.request.bind(navigator.locks);
      navigator.locks.request = async (name, callback) => {
        if (name === `snes-rust:saves:${hash}`) {
          navigator.locks.request = lock;
          await new Promise(resolve => { window.releaseFirstSave = resolve; });
        }
        return lock(name, callback);
      };
    }, largeHash);
    await sram(page, 0x50);
    await page.evaluate(() => window.dispatchEvent(new Event('pagehide')));
    const other = await context.newPage();
    await ready(other);
    await local(other, largeRom);
    await pause(other);
    expect(await stored()).toBe(0x50);
    expect(await sram(other)).toBe(0x50);
    if (next === 'same-byte restore') {
      await capture(other);
      await other.locator('#load-state').click();
      await expect(other.locator('#save-status')).toHaveText('State loaded.');
    }
    if (next === 'another write') {
      await sram(other, 0x51);
      await other.evaluate(() => window.dispatchEvent(new Event('pagehide')));
      await expect.poll(stored).toBe(0x51);
    }
    const atQuota = next.endsWith('at quota');
    if (atQuota) await fillStorage(other);

    if (next !== 'pagehide at quota') await page.locator('#play').click();
    await sram(page, 0x61);
    if (next === 'pagehide at quota') await page.evaluate(() => window.dispatchEvent(new Event('pagehide')));
    else await pause(page);

    if (atQuota) {
      await expect.poll(stored).toBe(0x61);
      await expect(page.locator('#error')).toBeEmpty();
    } else {
      await expect(page.locator('#save-status')).toContainText('This tab’s progress is in Recovery');
    }
    await page.evaluate(hash => {
      releaseFirstSave();
      return Promise.resolve().then(() => navigator.locks.request(`snes-rust:saves:${hash}`, () => {}));
    }, largeHash);
    expect(await stored()).toBe(atQuota ? 0x61 : next === 'another write' ? 0x51 : 0x50);
    await page.locator('#save-data').click();
    const recovery = page.getByRole('button', { name: 'Recovery · Another tab', exact: true });
    if (atQuota) await expect(recovery).toHaveCount(0);
    else {
      await expect(recovery).toHaveCount(1);
      await recovery.click();
      await expect(page.locator('#save-data-dialog')).toBeHidden();
      expect(await sram(page)).toBe(0x61);
    }
  });
}

for (const replayed of [false, true]) test(`successive stale-tab saves update one recovery profile ${replayed ? 'after journal replay at quota' : 'after the saving tab adopts it'}`, async ({ page, context }) => {
  const largeRom = Buffer.from(rom);
  largeRom[0x7fd8] = 8; // 256 KiB, large enough for repeated copies to exhaust localStorage.
  const largeHash = createHash('sha256').update(largeRom).digest('hex');
  const key = `snes-rust:sram:${largeHash}`;
  const stored = () => page.evaluate(key => atob(localStorage.getItem(key)).charCodeAt(3), key);
  async function persist(value) {
    await sram(page, value);
    await page.evaluate(() => window.dispatchEvent(new Event('pagehide')));
    await expect.poll(stored).toBe(value);
  }
  await ready(page);
  await local(page, largeRom);
  await pause(page);
  await persist(0x21);
  await capture(page);

  const stale = await context.newPage();
  await ready(stale);
  await local(stale, largeRom);
  await pause(stale);
  await persist(0x79);
  await page.locator('#load-state').click();
  await expect(page.locator('#save-status')).toContainText('State loaded.');

  const recoveryProfiles = () => page.evaluate(async hash => {
    // Wait behind any queued lifecycle write before inspecting the catalog and SRAM.
    return navigator.locks.request(`snes-rust:saves:${hash}`, () => {
      const { profiles } = JSON.parse(localStorage.getItem(`snes-rust:save-profiles:${hash}`));
      return profiles.filter(({ name }) => name === 'Recovery · Another tab').map(({ id }) => ({
        id, value: atob(JSON.parse(localStorage.getItem(`snes-rust:sram:${hash}:${id}`)).data).charCodeAt(3),
      }));
    });
  }, largeHash);
  let recoveryId;
  if (replayed) {
    // Delay two lifecycle callbacks before they request the real lock. Replay
    // the first journal in the other tab before the writer can adopt its ID.
    await stale.evaluate(hash => {
      const lock = navigator.locks.request.bind(navigator.locks);
      const delayed = [];
      navigator.locks.request = async (name, callback) => {
        if (name === `snes-rust:saves:${hash}` && delayed.length < 2) {
          await new Promise(resolve => delayed.push(resolve));
        }
        return lock(name, callback);
      };
      window.releaseSaveCallbacks = () => delayed.forEach(resolve => resolve());
    }, largeHash);
    await sram(stale, 0x50);
    await stale.evaluate(() => window.dispatchEvent(new Event('pagehide')));
    await page.locator('#save-data').click();
    await expect(page.getByRole('button', { name: 'Recovery · Another tab', exact: true })).toHaveCount(1);
    await page.keyboard.press('Escape');
    const profiles = await recoveryProfiles();
    recoveryId = profiles[0].id;
    expect(profiles).toEqual([{ id: recoveryId, value: 0x50 }]);

    // A new journal falls back to sessionStorage. Only a same-size update
    // fits in localStorage, and the picker must be able to replay it.
    await fillStorage(page);
    await sram(stale, 0x51);
    await stale.evaluate(() => window.dispatchEvent(new Event('pagehide')));
    await expect.poll(() => stale.evaluate(() => Object.keys(sessionStorage)
      .filter(key => key.startsWith('snes-rust:pending-sram:')).length)).toBe(1);
    await stale.locator('#save-data').click();
    await expect.poll(recoveryProfiles).toEqual([{ id: recoveryId, value: 0x51 }]);
    await expect(stale.locator('#save-data-message')).toBeEmpty();
    await stale.keyboard.press('Escape');
    // Pause triggers an ordinary save before either delayed callback runs.
    await stale.locator('#play').click();
    await sram(stale, 0x61);
    await pause(stale);
    await expect.poll(recoveryProfiles).toEqual([{ id: recoveryId, value: 0x61 }]);
    await stale.evaluate(() => releaseSaveCallbacks());
    await expect(stale.locator('#save-status')).toContainText('This tab’s progress is in Recovery');
    expect(await page.evaluate(hash => JSON.parse(localStorage.getItem(`snes-rust:save-profiles:${hash}`)).selected, largeHash)).toBe('legacy');
    await page.evaluate(() => localStorage.removeItem('quota-test'));
  }
  for (let value = replayed ? 0x52 : 0x50; value <= 0x52; value++) {
    await sram(stale, value);
    await stale.evaluate(() => window.dispatchEvent(new Event('pagehide')));
    const profiles = await recoveryProfiles();
    recoveryId ??= profiles[0]?.id;
    expect(profiles).toEqual([{ id: recoveryId, value }]);
    expect(await stored()).toBe(0x21);
  }
  // Force the last save through the same journal replay used after a tab closes.
  await sram(stale, 0x53);
  await holdSaveLock(page, largeHash);
  await stale.goto('/?closed-stale-game');
  await page.evaluate(() => releaseExitSaveLock());
  await page.locator('#save-data').click();
  const recovery = page.getByRole('button', { name: 'Recovery · Another tab', exact: true });
  await expect(recovery).toHaveCount(1);
  await expect.poll(recoveryProfiles).toEqual([{ id: recoveryId, value: 0x53 }]);
  await recovery.click();
  await expect(page.locator('#save-data-dialog')).toBeHidden();
  expect(await sram(page)).toBe(0x53);
  expect(await stored()).toBe(0x21);
});

test('the shared picker targets saves and loads separately and removes the selected state', async ({ page }) => {
  await ready(page);
  await local(page, nonBattery);
  await pause(page);
  const destination = page.locator('#state-target');
  const picker = page.getByRole('dialog', { name: 'Save states' });
  const selector = page.getByRole('button', { name: /^Save state / });
  const load = page.getByRole('button', { name: 'Load state', exact: true });
  const newState = picker.getByRole('button', { name: 'New state' });
  const options = picker.getByRole('button', { name: /^Select state from/ });
  let time = Date.UTC(2026, 10, 26, 12);
  const advanceTime = () => page.clock.setFixedTime(new Date(time += 1000));

  await expect(destination).toHaveText('New state');
  await expect(load).toBeDisabled();
  await page.evaluate(() => { saveCore.reset(); saveCore.run_frame(0); });
  await advanceTime();
  await capture(page);
  const [first] = await savedHistory(page);
  const firstLabel = await destination.textContent();
  await page.evaluate(() => { for (let i = 0; i < 57; i++) saveCore.run_frame(0); });
  const updatedMachine = await machineState(page);
  await advanceTime();
  await capture(page);
  const history = await savedHistory(page);
  expect(history).toHaveLength(1);
  const [updated] = history;
  expect(updated.key[3]).toBe(first.key[3]);
  expect(updated.key[2]).toBe(time);
  expect(updated.thumbnail).not.toEqual(first.thumbnail);
  await expect(destination).not.toHaveText(firstLabel);

  await selector.press('Enter');
  await expect(newState).toBeFocused();
  await expect(options).toHaveCount(1);
  await expect(options.first()).toHaveAttribute('aria-current', 'true');
  await expect(options.first().locator('.state-check')).toBeVisible();
  expect(await options.first().locator('img').evaluate(async image => {
    await image.decode();
    return [image.naturalWidth, image.naturalHeight];
  })).toEqual([128, 112]);
  await page.keyboard.press('Escape');
  await expect(picker).toBeHidden();
  await expect(selector).toBeFocused();
  await page.evaluate(() => saveCore.run_frame(0x0100));
  await load.press('Enter');
  await expect(page.locator('#save-status')).toHaveText('State loaded.');
  expect(await machineState(page)).toEqual(updatedMachine);
  await selector.press('Space');
  await newState.press('Enter');
  await expect(destination).toHaveText('New state');
  await expect(load).toBeDisabled();
  expect(await savedHistory(page)).toEqual(history);
  expect(await machineState(page)).toEqual(updatedMachine);
  await expect(page.locator('#status')).toHaveText('Paused');

  await page.evaluate(() => saveCore.run_frame(0x0100));
  const olderMachine = await machineState(page);
  await advanceTime();
  await capture(page);
  const [newEntry, formerEntry] = await savedHistory(page);
  expect(newEntry.key[3]).not.toBe(first.key[3]);
  expect(formerEntry).toEqual(updated);
  await selector.click();
  await expect(options).toHaveCount(2);
  await expect(options.first()).toHaveAttribute('aria-current', 'true');
  await options.last().click();
  expect(await savedHistory(page)).toEqual([newEntry, updated]);
  expect(await machineState(page)).toEqual(olderMachine);
  await expect(page.locator('#status')).toHaveText('Paused');

  await page.evaluate(() => saveCore.run_frame(0));
  await advanceTime();
  await capture(page);
  expect((await savedHistory(page)).map(({ key }) => key[3])).toEqual([first.key[3], newEntry.key[3]]);
  await selector.click();
  await options.last().press('Enter');
  await expect(picker).toBeHidden();
  await expect(selector).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(page.locator('#save-state')).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(load).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.locator('#save-status')).toHaveText('State loaded.');
  expect(await machineState(page)).toEqual(olderMachine);
  await advanceTime();
  await capture(page);
  expect((await savedHistory(page)).map(({ key }) => key[3])).toEqual([newEntry.key[3], first.key[3]]);

  await selector.click();
  const remove = page.getByRole('button', { name: /^Remove state from/ }).first();
  const targetTime = await destination.getAttribute('title');
  const removal = page.waitForEvent('dialog');
  const clickRemove = remove.click();
  const confirmation = await removal;
  expect(confirmation.message()).toContain(targetTime);
  await confirmation.dismiss();
  await clickRemove;
  await expect(destination).not.toHaveText('New state');
  page.once('dialog', dialog => dialog.accept());
  await remove.click();
  await expect(states(page)).toHaveCount(1);
  await expect(newState).toHaveAttribute('aria-current', 'true');
  await expect(destination).toHaveText('New state');
  await expect(load).toBeDisabled();
  expect(await machineState(page)).toEqual(olderMachine);
});

test('Tab wraps inside the save-state picker on a narrow screen', async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 568 });
  await ready(page);
  await local(page, nonBattery);
  await pause(page);
  await capture(page);
  const history = await savedHistory(page);
  const picker = page.getByRole('dialog', { name: 'Save states' });
  const selector = page.locator('#choose-state');
  const close = picker.getByRole('button', { name: 'Close', exact: true });
  const importState = picker.getByRole('button', { name: 'Import state', exact: true });

  await selector.press('Enter');
  await expect(picker.getByRole('button', { name: 'New state' })).toBeFocused();
  await expect(states(page)).toHaveCount(1);
  for (const name of [/^Select state from/, /^Export state from/, /^Remove state from/, 'Import state']) {
    await page.keyboard.press('Tab');
    await expect(picker.getByRole('button', { name })).toBeFocused();
  }
  await page.keyboard.press('Tab');
  await expect(close).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await expect(importState).toBeFocused();
  await page.keyboard.press('Tab');
  await page.keyboard.press('Enter');
  await expect(picker).toBeHidden();
  await expect(selector).toBeFocused();
  expect(await savedHistory(page)).toEqual(history);
});

test('save destinations are remembered separately for each ROM and SRAM profile', async ({ page }) => {
  await ready(page);
  await local(page);
  await pause(page);
  const destination = page.locator('#state-target');
  const saveData = page.locator('#save-data');
  await capture(page);
  const save1 = await destination.getAttribute('title');
  const [original] = await savedHistory(page, rom, 'legacy');
  await saveData.click();
  await page.getByRole('button', { name: 'New save', exact: true }).click();
  await expect(saveData).toContainText('Save 2');
  await expect(destination).toHaveText('New state');
  await capture(page);
  const save2 = await destination.getAttribute('title');
  expect(await savedHistory(page, rom, 'legacy')).toEqual([original]);

  await local(page, nonBattery);
  await pause(page);
  await expect(destination).toHaveText('New state');
  await capture(page);
  const separateRom = await savedHistory(page);
  await local(page);
  await pause(page);
  await expect(saveData).toContainText('Save 2');
  await expect(destination).toHaveAttribute('title', save2);
  await saveData.click();
  await page.getByRole('button', { name: 'Save 1', exact: true }).click();
  await expect(saveData).toContainText('Save 1');
  await expect(destination).toHaveAttribute('title', save1);
  await capture(page);
  expect((await savedHistory(page, rom, 'legacy')).map(({ key }) => key[3])).toEqual([original.key[3]]);
  expect(await savedHistory(page)).toEqual(separateRom);
});

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
    const picker = document.getElementById('state-picker');
    const newState = document.getElementById('new-state');
    for (let i = 0; i < 105; i++) {
      if (i) { picker.showPopover(); newState.click(); }
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
  await expect(page.locator('#state-target')).toHaveText('New state');
  await expect(page.locator('#load-state')).toBeDisabled();
  await page.locator('#choose-state').click();
  await expect(states(page)).toHaveCount(24);
  for (let pageNumber = 2; pageNumber <= 5; pageNumber++) {
    await page.getByRole('button', { name: 'Show older states' }).click();
    await expect(states(page)).toHaveCount(Math.min(24 * pageNumber, 105));
    await expect(states(page).nth(24 * (pageNumber - 1))).toBeFocused();
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
  await states(page).nth(104 - selected).click();
  const started = Date.now();
  await page.locator('#load-state').click();
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
  const selectedLabel = await page.locator('#state-target').getAttribute('title');
  await page.locator('#choose-state').click();
  await expect(states(page)).toHaveCount(24);
  page.once('dialog', dialog => dialog.accept());
  await page.getByRole('button', { name: /^Remove state from/ }).first().click();
  await expect(states(page)).toHaveCount(23);
  await expect(page.locator('#state-target')).toHaveAttribute('title', selectedLabel);
  expect(await machineState(page)).toEqual([...snapshot]);
  await page.keyboard.press('Escape');
  await page.locator('#change-cartridge').click();
  await page.getByRole('button', { name: 'Remove no-battery.sfc', exact: true }).click();
  await expect(page.getByRole('button', { name: 'no-battery.sfc', exact: true })).toHaveCount(0);
  await ready(page);
  await local(page, nonBattery, 'reimported.sfc');
  await pause(page);
  await page.locator('#choose-state').click();
  await expect(states(page)).toHaveCount(24);
  expect(errors).toEqual([]);
  const stats = expected.map(({ milliseconds }) => milliseconds).sort((a, b) => a - b);
  console.log(`105 captures: median ${stats[52].toFixed(1)} ms, p95 ${stats[99].toFixed(1)} ms; one UI load ${latency} ms; selected compressed file ${snapshot.length} bytes`);
});

test('aborted IndexedDB writes roll back creation and overwrite, and cancelled restores keep their destination', async ({ page }) => {
  await ready(page);
  await local(page, nonBattery, 'no-battery.sfc');
  await pause(page);
  await page.evaluate(() => {
    const add = IDBObjectStore.prototype.add;
    window.abortSnapshotWrite = true;
    IDBObjectStore.prototype.add = function (...args) {
      const request = add.apply(this, args);
      if (this.name === 'snapshots' && window.abortSnapshotWrite) {
        request.addEventListener('success', () => this.transaction.abort(), { once: true });
      }
      return request;
    };
  });
  await page.locator('#save-state').click();
  await expect(page.locator('#error')).toContainText('transaction aborted');
  await expect(page.locator('#load-state')).toBeDisabled();
  await page.locator('#choose-state').click();
  await expect(page.locator('#state-message')).toHaveText('No saved states yet.');
  await page.keyboard.press('Escape');
  await page.evaluate(() => { window.abortSnapshotWrite = false; });
  const originalMachine = await machineState(page);
  await capture(page);
  const original = await savedHistory(page);
  const destination = page.locator('#state-target');
  const originalLabel = await destination.getAttribute('title');
  await page.evaluate(() => saveCore.run_frame(0x0100));
  const newer = await machineState(page);
  expect(newer).not.toEqual(originalMachine);
  await page.evaluate(() => { window.abortSnapshotWrite = true; });
  await page.locator('#save-state').click();
  await expect(page.locator('#error')).toContainText('transaction aborted');
  await expect(page.locator('#save-status')).toBeEmpty();
  await expect(destination).toHaveAttribute('title', originalLabel);
  expect(await savedHistory(page)).toEqual(original);
  expect(await machineState(page)).toEqual(newer);

  await page.locator('#load-state').click();
  await expect(page.locator('#save-status')).toHaveText('State loaded.');
  expect(await machineState(page)).toEqual(originalMachine);
  await page.evaluate(() => saveCore.run_frame(0x0100));
  await page.evaluate(() => {
    const request = LockManager.prototype.request;
    LockManager.prototype.request = async function (...args) {
      await new Promise(resolve => { window.releaseSaveLock = resolve; });
      return request.apply(this, args);
    };
  });
  await page.locator('#load-state').click();
  await expect.poll(() => page.evaluate(() => typeof releaseSaveLock)).toBe('function');
  await page.keyboard.press('Escape');
  await page.evaluate(() => releaseSaveLock());
  await expect(page.locator('#load-state')).toBeEnabled();
  expect(await machineState(page)).toEqual(newer);
  await expect(destination).toHaveAttribute('title', originalLabel);
});

test('a vanished destination fails safely and returns to New state', async ({ page }) => {
  await ready(page);
  await local(page, nonBattery);
  await pause(page);
  await capture(page);
  const [vanished] = await savedHistory(page);
  await page.evaluate(async key => {
    const { removeState } = await import('/saves.js');
    await removeState(key);
  }, vanished.key);
  await page.locator('#save-state').click();
  await expect(page.locator('#error')).toContainText('The save destination no longer exists.');
  await expect(page.locator('#save-status')).toBeEmpty();
  await expect(page.locator('#state-target')).toHaveText('New state');
  expect(await savedHistory(page)).toEqual([]);
  await capture(page);
  expect((await savedHistory(page))[0].key[3]).not.toBe(vanished.key[3]);
});

test.describe('touch save states', () => {
  test.use({ hasTouch: true, isMobile: true, viewport: { width: 320, height: 568 } });
  test('bundled cartridge snapshots respect visibility and paused intent on narrow and short touch layouts', async ({ page }) => {
    await ready(page);
    await selectDemo(page);
    await expect(page.locator('#status')).toHaveText('Playing');
    await page.locator('#save-state').tap();
    await expect(page.locator('#save-status')).toHaveText('State saved.');
    await page.locator('#choose-state').tap();
    const picker = page.getByRole('dialog', { name: 'Save states' });
    await expect(picker).toBeInViewport({ ratio: 1 });
    const chosen = states(page).first();
    await expect(chosen.locator('.state-check')).toBeVisible();
    expect(await chosen.evaluate(option => option.scrollWidth === option.clientWidth)).toBe(true);
    await picker.getByRole('button', { name: 'New state' }).tap();
    await expect(page.locator('#state-target')).toHaveText('New state');
    await expect(page.locator('#status')).toHaveText('Playing');
    await expect(page.locator('#load-state')).toBeDisabled();
    await page.locator('#choose-state').tap();
    await expect(states(page)).toHaveCount(1);
    await states(page).first().tap();
    await expect(picker).toBeHidden();
    await expect(page.locator('#status')).toHaveText('Playing');
    await setTabHidden(page, true);
    await page.locator('#load-state').tap();
    await expect(page.locator('#save-status')).toHaveText('State loaded.');
    await expect(page.locator('#status')).toHaveText('Suspended');
    await setTabHidden(page, false);
    await expect(page.locator('#status')).toHaveText('Playing');
    await page.setViewportSize({ width: 667, height: 375 });
    await pause(page);
    const beforeSelection = await machineState(page);
    await page.locator('#choose-state').tap();
    await expect(picker).toBeInViewport({ ratio: 1 });
    await states(page).first().tap();
    await expect(picker).toBeHidden();
    expect(await machineState(page)).toEqual(beforeSelection);
    await page.locator('#load-state').tap();
    await expect(page.locator('#save-status')).toHaveText('State loaded.');
    await expect(page.locator('#status')).toHaveText('Paused');
    await waitForAnimationFrames(page);
    await expect(page.locator('#status')).toHaveText('Paused');
    await expect(page.locator('[data-button]')).toHaveCount(12);
  });
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
  await expect(page.locator('#load-state')).toBeDisabled();
  await page.locator('#choose-state').click();
  await expect(page.locator('#state-message')).toContainText('Browser save state storage is unavailable');
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
  await expect(page.locator('#state-picker')).toBeHidden();
  await expect.poll(() => page.evaluate(() => audioRestore.buttons)).toBe(0);
  expect(await page.evaluate(() => audioRestore.stopped)).toBeGreaterThan(0);
  await page.keyboard.up('ArrowRight');
  await expect(page.locator('#status')).toHaveText('Playing');
  await expect(page.locator('#display')).toHaveValue('pixel');
  await expect(volume).toHaveValue(level);
});
