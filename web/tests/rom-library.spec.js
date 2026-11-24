import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { selectDemo } from './cartridge.js';

const smoke = readFileSync(new URL('../../tests/roms/smoke.bin', import.meta.url));
const demo = readFileSync(new URL('../../demos/shionari.sfc', import.meta.url));
const hashOf = buffer => createHash('sha256').update(buffer).digest('hex');
const withHeader = buffer => Buffer.concat([Buffer.alloc(512, 0x5a), buffer]);
const file = (name, buffer) => ({ name, mimeType: 'application/octet-stream', buffer });

// Use the repository's MIT diagnostic header with an original guest. On boot
// it records the restored SRAM byte in WRAM and saves a new byte on SNES B.
function batteryGuest(marker) {
  const rom = Buffer.from(smoke);
  rom.set([
    0x78, 0xd8,             // SEI, CLD
    0xaf, 0x03, 0x00, 0x70, // LDA $700003
    0x85, 0x12,             // STA $12
    0xa9, 0x01,             // poll: LDA #1
    0x8d, 0x16, 0x40,       // STA $4016 (latch controller)
    0x9c, 0x16, 0x40,       // STZ $4016
    0xad, 0x16, 0x40,       // LDA $4016
    0x29, 0x01,             // AND #1 (B)
    0xf0, 0xf1,             // BEQ poll
    0xa9, marker,           // LDA #marker
    0x8f, 0x03, 0x00, 0x70, // STA $700003
    0x80, 0xe9,             // BRA poll
  ], 0);
  // Recompute the LoROM checksum after writing the program.
  rom.fill(0, 0x7fdc, 0x7fe0);
  const checksum = rom.reduce((sum, value) => sum + value, 510) & 0xffff;
  rom.writeUInt16LE(checksum ^ 0xffff, 0x7fdc);
  rom.writeUInt16LE(checksum, 0x7fde);
  return rom;
}
const first = batteryGuest(0x79), second = batteryGuest(0x48);
const firstHash = hashOf(first), secondHash = hashOf(second);
const rows = page => page.locator('#saved-cartridges li');
const saved = (page, hash) => page.locator(`#saved-cartridges li[data-hash="${hash}"]`);
const picker = page => page.getByRole('dialog', { name: 'Cartridges' });
const status = page => page.locator('#status');
const current = page => page.locator('#current-game');
const sram = page => page.evaluate(() => new Uint8Array(libraryCore.memory.buffer, libraryCore.sram_buffer(), libraryCore.sram_len())[3]);
const bootSave = page => page.evaluate(() => libraryCore.peek(0x12));
const commits = page => page.evaluate(() => libraryCommits);

async function instrument(page) {
  await page.addInitScript(() => {
    const instantiate = WebAssembly.instantiate;
    window.libraryCommits = 0;
    WebAssembly.instantiate = async (...args) => {
      const result = await instantiate(...args);
      window.libraryCore = result.instance.exports;
      return { instance: { exports: { ...libraryCore, commit_rom(...args) {
        const ok = libraryCore.commit_rom(...args);
        if (ok) libraryCommits++;
        return ok;
      } } } };
    };
    window.libraryReads = [];
    const transaction = IDBDatabase.prototype.transaction;
    IDBDatabase.prototype.transaction = function (stores, mode, ...args) {
      if (this.name === 'snes-rust:cartridges' && mode === 'readonly') libraryReads.push(stores);
      return transaction.call(this, stores, mode, ...args);
    };
  });
}
async function ready(page) { await expect(status(page)).toHaveText('Ready'); }
async function openPicker(page) { await page.locator('#change-cartridge').click(); await expect(picker(page)).toBeVisible(); }
async function importRom(page, name, buffer) {
  await page.locator('#rom').setInputFiles(file(name, buffer));
  await expect(page.locator('#rom')).toBeEnabled();
}
async function chooseRom(page, name, buffer) {
  await openPicker(page);
  const pending = page.waitForEvent('filechooser');
  await picker(page).getByRole('button', { name: 'Open local ROM…' }).click();
  await (await pending).setFiles(file(name, buffer));
  await expect(page.locator('#rom')).toBeEnabled();
}
async function selectSaved(page, hash, name) {
  await saved(page, hash).getByRole('button', { name, exact: true }).click();
  await expect(page.locator('#rom')).toBeEnabled();
}
async function removeSaved(page, hash, name) {
  await saved(page, hash).getByRole('button', { name: `Remove ${name}`, exact: true }).click();
}
async function changeStoredBytes(page, hash, bytes) {
  await page.evaluate(({ hash, bytes }) => new Promise((resolve, reject) => {
    const request = indexedDB.open('snes-rust:cartridges', 1);
    request.onerror = () => reject(request.error);
    request.onsuccess = () => {
      const db = request.result;
      const tx = db.transaction('roms', 'readwrite');
      tx.oncomplete = () => { db.close(); resolve(); };
      tx.onabort = () => { db.close(); reject(tx.error); };
      const store = tx.objectStore('roms');
      if (bytes) store.put(Uint8Array.from(bytes), hash);
      else store.delete(hash);
    };
  }), { hash, bytes: bytes && [...bytes] });
}

async function putStoredCartridge(page, entry, bytes) {
  await page.evaluate(({ entry, bytes }) => new Promise((resolve, reject) => {
    const request = indexedDB.open('snes-rust:cartridges', 1);
    request.onerror = () => reject(request.error);
    request.onsuccess = () => {
      const db = request.result;
      const tx = db.transaction(bytes ? ['cartridges', 'roms'] : 'cartridges', 'readwrite');
      tx.oncomplete = () => { db.close(); resolve(); };
      tx.onabort = () => { db.close(); reject(tx.error); };
      tx.objectStore('cartridges').put(entry);
      if (bytes) tx.objectStore('roms').put(Uint8Array.from(bytes), entry.hash);
    };
  }), { entry, bytes: bytes && [...bytes] });
}

test('remembers normalized ROMs, restores guest progress after reload and revisit, and removes without ejecting', async ({ page, context }) => {
  const requests = [];
  context.on('request', request => requests.push([request.method(), request.url(), request.postData()]));
  await instrument(page);
  await page.goto('/'); await ready(page);
  await chooseRom(page, 'homebrew.smc', withHeader(first));
  await expect(status(page)).toHaveText('Playing');
  await expect(current(page)).toHaveText('homebrew.smc');
  expect(await commits(page)).toBe(1);
  await page.keyboard.down('z');
  await expect.poll(() => sram(page)).toBe(0x79);
  await page.keyboard.up('z');

  await importRom(page, 'alias.sfc', first);
  expect(await commits(page)).toBe(1);
  await expect(current(page)).toHaveText('homebrew.smc');
  await importRom(page, 'homebrew.smc', second);
  await expect(status(page)).toHaveText('Playing');
  expect(await sram(page)).toBe(0);
  await page.keyboard.down('z');
  await expect.poll(() => sram(page)).toBe(0x48);
  await page.keyboard.up('z');
  await importRom(page, 'diagnostic.sfc', smoke);

  await openPicker(page);
  await expect(rows(page)).toHaveCount(3);
  await expect(saved(page, firstHash).getByRole('button', { name: 'homebrew.smc', exact: true })).toBeVisible();
  await expect(saved(page, secondHash).getByRole('button', { name: 'homebrew.smc', exact: true })).toBeVisible();
  for (const hash of [firstHash, secondHash]) {
    await expect(saved(page, hash).getByRole('button', { name: 'homebrew.smc', exact: true })).toHaveAccessibleDescription(hash.slice(0, 12));
  }
  await selectSaved(page, firstHash, 'homebrew.smc');
  await expect.poll(() => bootSave(page)).toBe(0x79);
  await expect(status(page)).toHaveText('Playing');
  await page.evaluate(() => {
    window.writeSram = Storage.prototype.setItem;
    Storage.prototype.setItem = () => { throw new DOMException('Storage full', 'QuotaExceededError'); };
  });
  await openPicker(page);
  await selectSaved(page, firstHash, 'homebrew.smc');
  await expect(status(page)).toHaveText('Playing');
  await page.evaluate(() => { Storage.prototype.setItem = writeSram; });
  await page.locator('#play').click();
  await openPicker(page);
  await selectSaved(page, firstHash, 'homebrew.smc');
  await expect(status(page)).toHaveText('Paused');
  expect(await commits(page)).toBe(4);

  await page.reload(); await ready(page);
  expect(await page.evaluate(() => libraryReads)).toEqual([]);
  await openPicker(page);
  await expect(rows(page)).toHaveCount(3);
  expect(await page.evaluate(() => libraryReads)).toEqual(['cartridges']);
  await selectSaved(page, secondHash, 'homebrew.smc');
  await expect.poll(() => bootSave(page)).toBe(0x48);
  expect(await page.evaluate(() => libraryReads)).toEqual(['cartridges', 'roms']);

  const sameOrigin = page.url();
  await page.close();
  const revisit = await context.newPage();
  await instrument(revisit);
  await revisit.goto(sameOrigin); await ready(revisit);
  await openPicker(revisit);
  await expect(rows(revisit)).toHaveCount(3);
  await selectSaved(revisit, firstHash, 'homebrew.smc');
  await expect.poll(() => bootSave(revisit)).toBe(0x79);
  await openPicker(revisit);
  const unmodifiedSram = await revisit.evaluate(hash => localStorage.getItem(`snes-rust:sram:${hash}`), firstHash);
  await removeSaved(revisit, firstHash, 'homebrew.smc');
  await expect(saved(revisit, firstHash)).toHaveCount(0);
  await expect(status(revisit)).toHaveText('Playing');
  await expect(current(revisit)).toHaveText('homebrew.smc');
  expect(await commits(revisit)).toBe(1);
  expect(await sram(revisit)).toBe(0x79);
  expect(await revisit.evaluate(hash => localStorage.getItem(`snes-rust:sram:${hash}`), firstHash)).toBe(unmodifiedSram);
  await revisit.keyboard.press('Escape');
  await importRom(revisit, 'reimported.sfc', first);
  expect(await commits(revisit)).toBe(1);
  await openPicker(revisit);
  await expect(saved(revisit, firstHash).getByRole('button', { name: 'reimported.sfc', exact: true })).toBeVisible();
  await revisit.reload(); await ready(revisit);
  await openPicker(revisit);
  await selectSaved(revisit, firstHash, 'reimported.sfc');
  await expect.poll(() => bootSave(revisit)).toBe(0x79);

  const origin = new URL(sameOrigin).origin;
  expect(requests.length).toBeGreaterThan(0);
  for (const [method, url, body] of requests) {
    expect(method).toBe('GET');
    expect(body).toBeNull();
    expect(new URL(url).origin).toBe(origin);
    expect(['/', '/app.js', '/style.css', '/snes_wasm.wasm']).toContain(new URL(url).pathname);
    expect(new URL(url).search).toBe('');
  }
});

test('holding Enter on Remove does not launch the next saved cartridge', async ({ page }) => {
  await instrument(page);
  await page.goto('/'); await ready(page);
  await importRom(page, 'second.sfc', second);
  await importRom(page, 'first.sfc', first);
  await openPicker(page);
  await expect(rows(page)).toHaveCount(2);
  await saved(page, firstHash).getByRole('button', { name: 'Remove first.sfc' }).focus();
  const next = saved(page, secondHash).getByRole('button', { name: 'second.sfc', exact: true });
  await page.keyboard.down('Enter');
  await expect(saved(page, firstHash)).toHaveCount(0);
  await expect(next).toBeFocused();
  await page.keyboard.down('Enter'); // Auto-repeat from the held removal key.
  await page.keyboard.up('Enter');
  await expect(picker(page)).toBeVisible();
  await expect(next).toBeFocused();
  await expect(current(page)).toHaveText('first.sfc');
  await expect(status(page)).toHaveText('Playing');
  expect(await commits(page)).toBe(2);

  await page.keyboard.press('Enter');
  await expect(picker(page)).toBeHidden();
  await expect(current(page)).toHaveText('second.sfc');
  expect(await commits(page)).toBe(3);
});

const touchTest = test.extend({ hasTouch: true });
for (const { width, height, count, touch, nearEdge } of [
  { width: 320, height: 568, count: 1 },
  { width: 667, height: 320, count: 4 },
  { width: 320, height: 568, count: 1, touch: true },
  { width: 667, height: 320, count: 4, touch: true },
  { width: 320, height: 568, count: 1, touch: true, nearEdge: true },
]) {
  const testPointer = touch ? touchTest : test;
  const gesture = touch ? 'double-tapping' : 'double-clicking';
  const target = nearEdge ? 'the edge of Remove' : 'Remove';
  testPointer(`${gesture} ${target} at ${width}×${height} only removes the intended cartridge`, async ({ page }) => {
    await page.setViewportSize({ width, height });
    await instrument(page);
    await page.goto('/'); await ready(page);
    const otherNames = [];
    for (let i = 1; i < count; i++) {
      const name = `spare-${i}.sfc`;
      await importRom(page, name, batteryGuest(i));
      otherNames.push(name);
    }
    await importRom(page, 'active.sfc', first);
    await openPicker(page);
    await expect(rows(page)).toHaveCount(count);
    const remove = saved(page, firstHash).getByRole('button', { name: 'Remove active.sfc' });
    await remove.scrollIntoViewIfNeeded();
    const box = await remove.boundingBox();
    const x = box.x + (nearEdge ? 2 : box.width / 2), y = box.y + 2;
    if (nearEdge) await page.evaluate(() => {
      document.addEventListener('click', event => { window.removeClickCount = event.detail; }, { capture: true });
    });
    if (touch) await page.touchscreen.tap(x, y);
    else await page.mouse.click(x, y);
    // Let IndexedDB commit and move the controls before sending the second click.
    await expect(remove).toHaveCount(0);
    const next = count === 1 ? page.locator('#demo-yonaki') : picker(page).getByRole('button', { name: 'Remove spare-1.sfc' });
    const nextX = touch ? x - 10 : x;
    if (nearEdge) expect(nextX).toBeLessThan(box.x);
    else expect(nextX).toBeGreaterThan(box.x);
    expect(await next.evaluate((button, { x, y }) => button.contains(document.elementFromPoint(x, y)), { x: nextX, y })).toBe(true);
    if (touch) await page.touchscreen.tap(nextX, y);
    else await page.mouse.click(nextX, y, { clickCount: 2 });
    if (nearEdge) expect(await page.evaluate(() => removeClickCount)).toBe(2);

    await expect(picker(page)).toBeVisible();
    await expect(current(page)).toHaveText('active.sfc');
    await expect(status(page)).toHaveText('Playing');
    expect(await commits(page)).toBe(count);
    await page.reload(); await ready(page);
    await openPicker(page);
    await expect(rows(page)).toHaveCount(count - 1);
    await expect(remove).toHaveCount(0);
    for (const name of otherNames) {
      await expect(picker(page).getByRole('button', { name, exact: true })).toBeVisible();
    }
  });
}

test('remembers an identical local reimport during an SRAM write failure without interrupting play', async ({ page }) => {
  await instrument(page);
  await page.goto('/'); await ready(page);
  await importRom(page, 'active.sfc', first);
  await page.keyboard.down('z');
  await expect.poll(() => sram(page)).toBe(0x79);
  await page.keyboard.up('z');
  await page.evaluate(() => {
    const setItem = Storage.prototype.setItem;
    Storage.prototype.setItem = function (key, value) {
      if (key.startsWith('snes-rust:sram:')) throw new DOMException('Storage full', 'QuotaExceededError');
      return setItem.call(this, key, value);
    };
  });
  await openPicker(page);
  await removeSaved(page, firstHash, 'active.sfc');
  await expect(rows(page)).toHaveCount(0);
  await page.keyboard.press('Escape');

  await chooseRom(page, 'reimported.smc', withHeader(first));
  await expect(status(page)).toHaveText('Playing');
  await expect(current(page)).toHaveText('active.sfc');
  expect(await commits(page)).toBe(1);
  expect(await sram(page)).toBe(0x79);
  await openPicker(page);
  await expect(saved(page, firstHash).getByRole('button', { name: 'reimported.smc', exact: true })).toBeVisible();
  await page.keyboard.press('Escape');

  await chooseRom(page, 'different.sfc', second);
  await expect(page.locator('#error')).toContainText('Could not persist SRAM');
  await expect(status(page)).toHaveText('Paused');
  await expect(current(page)).toHaveText('active.sfc');
  expect(await commits(page)).toBe(1);
  expect(await sram(page)).toBe(0x79);
  await openPicker(page);
  await expect(rows(page)).toHaveCount(1);
  await expect(saved(page, secondHash)).toHaveCount(0);
});

test('rejects invalid imports and shares bundled identity without a saved duplicate', async ({ page }) => {
  await page.goto('/'); await ready(page);
  const unsupported = Buffer.from(first);
  unsupported[0x7fd6] = 0x13;
  for (const [name, buffer, message] of [
    ['broken.sfc', Buffer.alloc(32768), 'header'],
    ['unsupported.sfc', unsupported, 'Unsupported cartridge hardware'],
    ['oversized.smc', Buffer.alloc(4 * 1024 * 1024 + 513), 'Maximum ROM size'],
    ['wrong.txt', first, 'Choose an .sfc or .smc'],
  ]) {
    await importRom(page, name, buffer);
    await expect(page.locator('#error')).toContainText(message, { ignoreCase: true });
    await expect(current(page)).toHaveText('None');
  }
  await openPicker(page);
  await expect(page.getByRole('region', { name: 'My cartridges' })).toHaveCount(0);
  await page.keyboard.press('Escape');
  await importRom(page, 'personal-name.smc', withHeader(demo));
  await expect(status(page)).toHaveText('Playing');
  await page.locator('#play').click();
  await selectDemo(page);
  await expect(status(page)).toHaveText('Paused');
  await expect(current(page)).toHaveText('personal-name.smc');
  await openPicker(page);
  await expect(rows(page)).toHaveCount(0);
});

test('pausing on a rejected local file saves active guest SRAM and reports write failures', async ({ page }) => {
  await instrument(page);
  await page.goto('/'); await ready(page);
  await importRom(page, 'active.sfc', first);
  await expect(status(page)).toHaveText('Playing');
  await page.keyboard.down('z');
  await expect.poll(() => sram(page)).toBe(0x79);
  await page.keyboard.up('z');

  const key = `snes-rust:sram:${firstHash}`;
  await page.evaluate(key => {
    const setItem = Storage.prototype.setItem;
    window.pausedSaveWrites = [];
    Storage.prototype.setItem = function (name, value) {
      if (name === key && document.getElementById('status').textContent === 'Paused') {
        pausedSaveWrites.push(atob(value).charCodeAt(3));
        if (window.failPausedSave) throw new DOMException('Storage full', 'QuotaExceededError');
      }
      return setItem.call(this, name, value);
    };
  }, key);
  const error = page.locator('#error');
  await chooseRom(page, 'wrong.txt', second);
  await expect(status(page)).toHaveText('Paused');
  await expect(error).toContainText('Choose an .sfc or .smc');
  expect(await page.evaluate(() => pausedSaveWrites)).toEqual([0x79]);
  expect(await page.evaluate(key => atob(localStorage.getItem(key)).charCodeAt(3), key)).toBe(0x79);

  await page.locator('#play').click();
  await expect(status(page)).toHaveText('Playing');
  await page.evaluate(() => { window.failPausedSave = true; });
  await chooseRom(page, 'wrong.txt', second);
  await expect(status(page)).toHaveText('Paused');
  await expect(error).toContainText('Choose an .sfc or .smc');
  await expect(error).toContainText('Could not persist SRAM');
  await expect(current(page)).toHaveText('active.sfc');
  expect(await commits(page)).toBe(1);
  expect(await sram(page)).toBe(0x79);
});

test('a missing stored name leaves healthy cartridges visible and can be repaired or removed', async ({ page }) => {
  await page.goto('/'); await ready(page);
  await importRom(page, 'healthy.sfc', first);
  await importRom(page, 'original.sfc', second);
  await openPicker(page);
  await expect(rows(page)).toHaveCount(2);
  await putStoredCartridge(page, { hash: secondHash });

  await page.reload(); await ready(page);
  await openPicker(page);
  const healthy = saved(page, firstHash).getByRole('button', { name: 'healthy.sfc', exact: true });
  const unnamed = saved(page, secondHash).getByRole('button', { name: 'Unnamed cartridge', exact: true });
  await expect(healthy).toBeVisible();
  await expect(unnamed).toBeVisible();
  await expect(picker(page).getByRole('status')).toBeEmpty();
  await page.keyboard.press('Escape');
  await chooseRom(page, 'repaired.sfc', second);
  await expect(status(page)).toHaveText('Playing');
  await openPicker(page);
  const repaired = saved(page, secondHash).getByRole('button', { name: 'repaired.sfc', exact: true });
  await expect(repaired).toBeVisible();

  await page.reload(); await ready(page);
  await openPicker(page);
  await expect(repaired).toBeVisible();
  await expect(healthy).toBeVisible();
  await putStoredCartridge(page, { hash: secondHash });
  await page.keyboard.press('Escape');
  await openPicker(page);
  await expect(unnamed).toBeVisible();
  await removeSaved(page, secondHash, 'Unnamed cartridge');
  await expect(saved(page, secondHash)).toHaveCount(0);
  await expect(healthy).toBeVisible();
});

test('rejects a saved cartridge with an empty hash before reading its valid ROM bytes', async ({ page }) => {
  await instrument(page);
  await page.goto('/'); await ready(page);
  await importRom(page, 'active.sfc', second);
  await openPicker(page);
  await expect(rows(page)).toHaveCount(1);
  await putStoredCartridge(page, { hash: '', name: 'broken.sfc' }, first);
  await page.keyboard.press('Escape');
  await openPicker(page);
  await selectSaved(page, '', 'broken.sfc');
  await expect(page.locator('#error')).toContainText('Reopen the original file or remove the saved entry');
  await expect(current(page)).toHaveText('active.sfc');
  await expect(status(page)).toHaveText('Playing');
  expect(await commits(page)).toBe(1);
  expect(await page.evaluate(() => libraryReads)).not.toContain('roms');

  await openPicker(page);
  await removeSaved(page, '', 'broken.sfc');
  await expect(rows(page)).toHaveCount(1);
  await expect(saved(page, secondHash)).toBeVisible();
});

test('a numeric storage key sharing a filename leaves the healthy ROM playable and the broken entry removable', async ({ page }) => {
  await page.goto('/'); await ready(page);
  await importRom(page, 'shared.sfc', first);
  await openPicker(page);
  await expect(rows(page)).toHaveCount(1);
  await putStoredCartridge(page, { hash: 7, name: 'shared.sfc' }, second);

  await page.reload(); await ready(page);
  await openPicker(page);
  await expect(rows(page)).toHaveCount(2);
  const healthy = saved(page, firstHash);
  await expect(healthy.getByRole('button', { name: 'shared.sfc', exact: true })).toHaveAccessibleDescription(firstHash.slice(0, 12));
  await selectSaved(page, firstHash, 'shared.sfc');
  await expect(status(page)).toHaveText('Playing');
  await expect(current(page)).toHaveText('shared.sfc');
  await openPicker(page);
  const brokenRemove = picker(page).getByRole('button', { name: 'Remove shared.sfc', description: 'Invalid saved entry' });
  await brokenRemove.focus();
  await page.keyboard.press('Enter');
  await expect(rows(page)).toHaveCount(1);
  await expect(healthy).toBeVisible();
  await expect(status(page)).toHaveText('Playing');
  await expect(healthy.getByRole('button', { name: 'shared.sfc', exact: true })).toBeFocused();

  await page.reload(); await ready(page);
  await openPicker(page);
  await expect(rows(page)).toHaveCount(1);
  await expect(healthy).toBeVisible();
});

test('reports missing and mismatched bytes, preserves the active guest, and repairs on reimport', async ({ page }) => {
  await instrument(page);
  await page.goto('/'); await ready(page);
  await importRom(page, 'target.sfc', first);
  await importRom(page, 'active.sfc', second);
  await openPicker(page);
  await expect(rows(page)).toHaveCount(2);
  for (const badBytes of [null, second]) {
    await changeStoredBytes(page, firstHash, badBytes);
    await selectSaved(page, firstHash, 'target.sfc');
    await expect(page.locator('#error')).toContainText('Reopen the original file or remove the saved entry');
    await expect(current(page)).toHaveText('active.sfc');
    await expect(status(page)).toHaveText('Playing');
    expect(await commits(page)).toBe(2);
    await openPicker(page);
  }
  await page.keyboard.press('Escape');
  await chooseRom(page, 'target-renamed.sfc', first);
  await expect(status(page)).toHaveText('Playing');
  await openPicker(page);
  await selectSaved(page, secondHash, 'active.sfc');
  await openPicker(page);
  await selectSaved(page, firstHash, 'target.sfc');
  await expect(current(page)).toHaveText('target.sfc');
  await expect(page.locator('#error')).toBeEmpty();
  await openPicker(page);
  await changeStoredBytes(page, firstHash, null);
  await removeSaved(page, firstHash, 'target.sfc');
  await expect(saved(page, firstHash)).toHaveCount(0);
  await expect(status(page)).toHaveText('Playing');
});

test('saved ROMs with copier headers must match the normalized SRAM identity', async ({ page }) => {
  await instrument(page);
  await page.goto('/'); await ready(page);
  await importRom(page, 'original.sfc', first);
  await page.keyboard.down('z');
  await expect.poll(() => sram(page)).toBe(0x79);
  await page.keyboard.up('z');
  await importRom(page, 'active.sfc', second);
  await openPicker(page);
  await expect(rows(page)).toHaveCount(2);

  const headered = withHeader(first);
  const rawHash = hashOf(headered);
  await putStoredCartridge(page, { hash: rawHash, name: 'wrong-key.smc' }, headered);
  await changeStoredBytes(page, firstHash, headered);
  await page.keyboard.press('Escape');
  await openPicker(page);
  await selectSaved(page, rawHash, 'wrong-key.smc');
  await expect(page.locator('#error')).toContainText('Reopen the original file or remove the saved entry');
  await expect(current(page)).toHaveText('active.sfc');
  await expect(status(page)).toHaveText('Playing');
  expect(await commits(page)).toBe(2);

  await openPicker(page);
  await selectSaved(page, firstHash, 'original.sfc');
  await expect.poll(() => bootSave(page)).toBe(0x79);
  await expect(status(page)).toHaveText('Playing');
  expect(await commits(page)).toBe(3);
  expect(await page.evaluate(hash => localStorage.getItem(`snes-rust:sram:${hash}`), rawHash)).toBeNull();
});

test('writes a ROM only after restoring SRAM, including on retry', async ({ page }) => {
  await page.goto('/'); await ready(page);
  await page.evaluate(hash => localStorage.setItem(`snes-rust:sram:${hash}`, '%invalid%'), firstHash);
  await importRom(page, 'retry.sfc', first);
  await expect(page.locator('#error')).toContainText('Save could not be restored');
  await openPicker(page);
  await expect(rows(page)).toHaveCount(0);
  await page.keyboard.press('Escape');
  await page.evaluate(hash => {
    const sram = new Uint8Array(2048);
    sram[3] = 0x79;
    localStorage.setItem(`snes-rust:sram:${hash}`, btoa(String.fromCharCode(...sram)));
  }, firstHash);
  await page.locator('#retry-save').click();
  await expect(status(page)).toHaveText('Playing');
  await openPicker(page);
  await expect(saved(page, firstHash)).toBeVisible();
});

test('aborted read, write and delete transactions preserve the committed library and allow retry', async ({ page }) => {
  await page.goto('/'); await ready(page);
  await page.evaluate(() => {
    window.abortLibraryRequest = (method, store) => {
      const original = IDBObjectStore.prototype[method];
      IDBObjectStore.prototype[method] = function (...args) {
        const request = original.apply(this, args);
        if (this.name === store) {
          request.addEventListener('success', () => this.transaction.abort(), { once: true });
          IDBObjectStore.prototype[method] = original;
        }
        return request;
      };
    };
    abortLibraryRequest('put', 'roms');
  });
  await importRom(page, 'abort.sfc', first);
  await expect(status(page)).toHaveText('Playing');
  await expect(page.locator('#error')).toContainText('Could not remember abort.sfc');
  await openPicker(page);
  await expect(rows(page)).toHaveCount(0);
  await page.keyboard.press('Escape');
  await importRom(page, 'abort.sfc', first);
  await expect(page.locator('#error')).toBeEmpty();
  await openPicker(page);
  await page.evaluate(() => abortLibraryRequest('get', 'roms'));
  await selectSaved(page, firstHash, 'abort.sfc');
  await expect(page.locator('#error')).toContainText('Could not load the saved ROM');
  await expect(status(page)).toHaveText('Playing');
  await openPicker(page);
  await page.evaluate(() => abortLibraryRequest('delete', 'roms'));
  await removeSaved(page, firstHash, 'abort.sfc');
  await expect(picker(page).getByRole('status')).toContainText('Could not remove abort.sfc');
  await expect(saved(page, firstHash)).toBeVisible();
  await expect(status(page)).toHaveText('Playing');
  await removeSaved(page, firstHash, 'abort.sfc');
  await expect(saved(page, firstHash)).toHaveCount(0);
  await expect(page.locator('#open-local')).toBeFocused();
});

test('quota exhaustion leaves playback and file import usable', async ({ page, context }) => {
  await page.goto('/'); await ready(page);
  const origin = new URL(page.url()).origin;
  const cdp = await context.newCDPSession(page);
  await cdp.send('Storage.overrideQuotaForOrigin', { origin, quotaSize: 1 });
  await chooseRom(page, 'quota.sfc', first);
  await expect(status(page)).toHaveText('Playing');
  await expect(page.locator('#error')).toContainText('Could not remember quota.sfc');
  await openPicker(page);
  await expect(rows(page)).toHaveCount(0);
  await page.keyboard.press('Escape');
  await cdp.send('Storage.overrideQuotaForOrigin', { origin });
  await chooseRom(page, 'quota.sfc', first);
  await expect(page.locator('#error')).toBeEmpty();
  await openPicker(page);
  await expect(saved(page, firstHash)).toBeVisible();
});

for (const mode of ['denied', 'unavailable']) {
  test(`IndexedDB ${mode} leaves the chooser and bundled demo usable`, async ({ page }) => {
    await page.addInitScript(mode => {
      if (mode === 'unavailable') Object.defineProperty(window, 'indexedDB', { value: undefined });
      else IDBFactory.prototype.open = () => { throw new DOMException('Storage denied', 'SecurityError'); };
    }, mode);
    await page.goto('/'); await ready(page);
    await openPicker(page);
    await expect(picker(page).getByRole('status')).toContainText('Could not read My cartridges');
    await page.keyboard.press('Escape');
    await chooseRom(page, 'offline.sfc', first);
    await expect(status(page)).toHaveText('Playing');
    await expect(page.locator('#error')).toContainText('Could not remember offline.sfc');
    await selectDemo(page);
    await expect(current(page)).toContainText('Shionari no Misshitsu');
    await expect(status(page)).toHaveText('Playing');
  });
}

test('concurrent imports and removal converge across open pickers', async ({ page, context }) => {
  const other = await context.newPage();
  await Promise.all([page.goto('/'), other.goto('/')]);
  await Promise.all([ready(page), ready(other)]);
  await Promise.all([importRom(page, 'shared.sfc', first), importRom(other, 'shared.sfc', withHeader(first))]);
  await Promise.all([openPicker(page), openPicker(other)]);
  await expect(rows(page)).toHaveCount(1);
  await expect(rows(other)).toHaveCount(1);

  await other.keyboard.press('Escape');
  await other.evaluate(() => {
    const read = File.prototype.arrayBuffer;
    File.prototype.arrayBuffer = async function () {
      await new Promise(resolve => { window.finishReadingRom = resolve; });
      return read.call(this);
    };
  });
  await other.locator('#rom').setInputFiles(file('again.sfc', first));
  await expect.poll(() => other.evaluate(() => typeof window.finishReadingRom)).toBe('function');
  await removeSaved(page, firstHash, 'shared.sfc');
  await expect(rows(page)).toHaveCount(0);
  await other.evaluate(() => finishReadingRom());
  await expect(other.locator('#rom')).toBeEnabled();
  // The first tab refreshes while its picker stays open.
  await expect(saved(page, firstHash).getByRole('button', { name: 'again.sfc', exact: true })).toBeVisible();
  await openPicker(other);
  await expect(rows(other)).toHaveCount(1);
  await removeSaved(page, firstHash, 'again.sfc');
  await expect(rows(other)).toHaveCount(0);
  await expect(status(page)).toHaveText('Playing');
  await expect(status(other)).toHaveText('Playing');
});

test.describe('scrolling personal cartridges', () => {
  test.use({ hasTouch: true });
  test('long names and touch targets fit on desktop, narrow portrait and short landscape', async ({ page }) => {
    await page.goto('/'); await ready(page);
    const longName = `Z${'long-cartridge-name'.repeat(10)}.sfc`;
    await importRom(page, longName, first);
    for (let i = 0; i < 8; i++) await importRom(page, `Cartridge ${i}.sfc`, batteryGuest(i + 1));
    await page.locator('#play').click();

    for (const [width, height] of [[1100, 850], [320, 568], [667, 320]]) {
      await page.setViewportSize({ width, height });
      await page.locator('#change-cartridge').tap();
      await expect(rows(page)).toHaveCount(9);
      const dialog = picker(page);
      const name = saved(page, firstHash).getByRole('button', { name: longName, exact: true });
      const remove = saved(page, firstHash).getByRole('button', { name: `Remove ${longName}`, exact: true });
      const box = await dialog.boundingBox();
      expect(box.x).toBeGreaterThanOrEqual(16);
      expect(box.y).toBeGreaterThanOrEqual(16);
      expect(box.x + box.width).toBeLessThanOrEqual(width - 16);
      expect(box.y + box.height).toBeLessThanOrEqual(height - 16);
      expect(await dialog.evaluate(el => el.scrollWidth)).toBe(await dialog.evaluate(el => el.clientWidth));
      expect(await name.evaluate(el => el.scrollWidth)).toBe(await name.evaluate(el => el.clientWidth));
      expect((await remove.boundingBox()).height).toBeGreaterThanOrEqual(44);
      if (height < 600) {
        expect(await dialog.evaluate(el => el.scrollHeight > el.clientHeight)).toBe(true);
        await expect(page.locator('#open-local')).not.toBeInViewport();
      }
      await page.locator('#open-local').scrollIntoViewIfNeeded();
      await expect(page.locator('#open-local')).toBeInViewport();
      if (height === 320) {
        await remove.tap();
        await expect(rows(page)).toHaveCount(8);
        await expect(status(page)).toHaveText('Paused');
      }
      await page.keyboard.press('Escape');
      await expect(dialog).toBeHidden();
    }
  });
});
