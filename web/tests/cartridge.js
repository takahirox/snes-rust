export async function selectDemo(page, id = 'demo') {
  await page.locator('#change-cartridge').click();
  await page.locator(`#${id}`).click();
}

export async function delayLocalFileRead(page) {
  await page.evaluate(() => {
    const read = File.prototype.arrayBuffer;
    File.prototype.arrayBuffer = async function () {
      await new Promise(resolve => { window.releaseLocalFileRead = resolve; });
      return read.call(this);
    };
  });
  return () => page.evaluate(() => releaseLocalFileRead());
}

const fixtureProfile = '00000000-0000-4000-8000-000000000001';
export const sramKey = (hash, id) => `snes-rust:sram:${hash}:${id}`;

export function selectedProfile(page, hash) {
  return page.evaluate(hash => JSON.parse(localStorage.getItem(`snes-rust:save-profiles:${hash}`))?.selected, hash);
}

export async function seedSram(page, hash, data) {
  const key = sramKey(hash, fixtureProfile);
  await page.evaluate(({ hash, id, key, data }) => {
    localStorage.setItem(`snes-rust:save-profiles:${hash}`, JSON.stringify({
      version: 1, selected: id, next: 2, profiles: [{ id, name: 'Save 1' }],
    }));
    localStorage.setItem(key, JSON.stringify({ data, updated: Date.now() }));
  }, { hash, id: fixtureProfile, key, data });
  return key;
}

export function savedSramData(page, key) {
  return page.evaluate(key => JSON.parse(localStorage.getItem(key))?.data ?? null, key);
}

export function savedSramByte(page, key) {
  return page.evaluate(key => {
    const record = JSON.parse(localStorage.getItem(key));
    return record ? atob(record.data).charCodeAt(3) : null;
  }, key);
}
