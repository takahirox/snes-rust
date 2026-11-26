// Save 1 uses the original key. Profile metadata and individual SRAM writes
// are separate so saving one profile never rewrites another profile's bytes.
export const originalProfile = 'legacy';
const catalogKey = hash => `snes-rust:save-profiles:${hash}`;
const sramKey = (hash, id) => `snes-rust:sram:${hash}${id === originalProfile ? '' : `:${id}`}`;
const legacyTimeKey = hash => `snes-rust:sram-time:${hash}`;

export function readProfiles(hash) {
  const raw = localStorage.getItem(catalogKey(hash));
  if (raw === null) return { version: 1, selected: originalProfile, next: 2, profiles: [{ id: originalProfile, name: 'Save 1' }] };
  const catalog = JSON.parse(raw);
  const validId = id => id === originalProfile || (typeof id === 'string' && /^[a-f0-9-]{36}$/.test(id));
  if (catalog?.version !== 1 || !Number.isSafeInteger(catalog.next) || catalog.next < 2 ||
      !Array.isArray(catalog.profiles) ||
      catalog.profiles.some(p => !validId(p?.id) || typeof p.name !== 'string' ||
        (p.recovery !== undefined && p.recovery !== true)) ||
      new Set(catalog.profiles.map(p => p.id)).size !== catalog.profiles.length ||
      !catalog.profiles.some(p => p.id === catalog.selected)) {
    throw new Error('Save data profile list is invalid');
  }
  return catalog;
}

export function writeProfiles(hash, catalog) {
  localStorage.setItem(catalogKey(hash), JSON.stringify(catalog));
}

export function addProfile(catalog, recovery = false) {
  const profile = {
    id: crypto.randomUUID(),
    name: recovery ? `Recovery · ${recovery}` : `Save ${catalog.next++}`,
  };
  if (recovery) profile.recovery = true;
  catalog.profiles.push(profile);
  return profile;
}

function profileRecord(hash, id) {
  const raw = localStorage.getItem(sramKey(hash, id));
  if (id === originalProfile) return { data: raw };
  const record = JSON.parse(raw);
  if (typeof record?.data !== 'string' || !Number.isFinite(record.updated)) throw new Error('Save data is missing or invalid');
  return record;
}

export function readSram(hash, id, size) {
  const { data } = profileRecord(hash, id);
  if (data === null) return null;
  if (data.length !== Math.ceil(size / 3) * 4) throw new Error('Save size does not match cartridge SRAM');
  const decoded = atob(data);
  if (decoded.length !== size) throw new Error('Save size does not match cartridge SRAM');
  return Uint8Array.from(decoded, c => c.charCodeAt(0));
}

export function writeSram(hash, id, bytes) {
  let binary = '';
  for (const value of bytes) binary += String.fromCharCode(value);
  const data = btoa(binary);
  const key = sramKey(hash, id);
  if (id === originalProfile) {
    const changed = localStorage.getItem(key) !== data;
    localStorage.setItem(key, data);
    if (changed) {
      // A timestamp is advisory. The original save has already been written.
      try { localStorage.setItem(legacyTimeKey(hash), String(Date.now())); } catch {}
    }
  } else {
    const previous = JSON.parse(localStorage.getItem(key));
    const updated = previous?.data === data ? previous.updated : Date.now();
    localStorage.setItem(key, JSON.stringify({ data, updated }));
  }
}

export function removeSram(hash, id) {
  localStorage.removeItem(sramKey(hash, id));
  if (id === originalProfile) localStorage.removeItem(legacyTimeKey(hash));
}

export function sramTimestamp(hash, id) {
  if (id !== originalProfile) return profileRecord(hash, id).updated;
  return Number(localStorage.getItem(legacyTimeKey(hash)));
}

export function sameSram(left, right) {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

// History holds small entries and compressed payloads in separate stores.
// The compound key orders each profile by capture time and stable ID.
let database;
function openHistory() {
  if (!globalThis.indexedDB) return Promise.reject(new Error('Browser save state storage is unavailable'));
  return database ??= new Promise((resolve, reject) => {
    const request = indexedDB.open('snes-rust:save-states', 1);
    let blocked = false;
    request.onupgradeneeded = () => {
      request.result.createObjectStore('entries');
      request.result.createObjectStore('snapshots');
    };
    request.onblocked = () => { blocked = true; reject(new Error('Save state storage is busy')); };
    request.onerror = () => reject(request.error);
    request.onsuccess = () => {
      const db = request.result;
      if (blocked) { db.close(); return; }
      db.onversionchange = db.onclose = () => { db.close(); database = undefined; };
      resolve(db);
    };
  }).catch(error => { database = undefined; throw error; });
}

async function transaction(stores, mode, run) {
  const db = await openHistory();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(stores, mode, mode === 'readwrite' ? { durability: 'strict' } : undefined);
    tx.onabort = () => reject(tx.error ?? new Error('Save state transaction aborted'));
    let result;
    tx.oncomplete = () => resolve(result());
    try { result = run(tx); }
    catch (error) { tx.abort(); reject(error); }
  });
}

const scopeRange = ({ hash, profile }, before) =>
  IDBKeyRange.bound([hash, profile], before ?? [hash, profile, []], false, Boolean(before));

export function historyPage(scope, before) {
  return transaction('entries', 'readonly', tx => {
    const cursor = tx.objectStore('entries').openCursor(scopeRange(scope, before), 'prev');
    const entries = [];
    let more = false;
    cursor.onsuccess = () => {
      if (!cursor.result) return;
      if (entries.length === 24) { more = true; return; }
      entries.push({ key: cursor.result.primaryKey, ...cursor.result.value });
      cursor.result.continue();
    };
    return () => ({ entries, more });
  });
}

export function historyExists(scope) {
  return transaction('entries', 'readonly', tx => {
    const request = tx.objectStore('entries').getKey(scopeRange(scope));
    return () => request.result !== undefined;
  });
}

export function writeState({ hash, profile }, time, snapshot, thumbnail) {
  const key = [hash, profile, time, crypto.randomUUID()];
  return transaction(['entries', 'snapshots'], 'readwrite', tx => {
    tx.objectStore('entries').add({ thumbnail }, key);
    tx.objectStore('snapshots').add(snapshot, key);
    return () => key;
  });
}

export function readState(key) {
  return transaction('snapshots', 'readonly', tx => {
    const request = tx.objectStore('snapshots').get(key);
    return () => request.result;
  });
}

export function removeState(key) {
  return transaction(['entries', 'snapshots'], 'readwrite', tx => {
    tx.objectStore('entries').delete(key);
    tx.objectStore('snapshots').delete(key);
    return () => undefined;
  });
}

// Serialize history insertion and profile deletion across tabs. Within a tab
// the player also gates cartridge, profile and snapshot operations.
export function lockSaves(hash, callback) {
  return navigator.locks.request(`snes-rust:saves:${hash}`, callback);
}
