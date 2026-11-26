// Save 1 uses the original key. Profile metadata and individual SRAM writes
// are separate so saving one profile never rewrites another profile's bytes.
export const originalProfile = 'legacy';
const catalogKey = hash => `snes-rust:save-profiles:${hash}`;
const sramKey = (hash, id) => `snes-rust:sram:${hash}${id === originalProfile ? '' : `:${id}`}`;
const legacyTimeKey = hash => `snes-rust:sram-time:${hash}`;
const replayKey = (hash, id) => `snes-rust:sram-replay:${hash}:${id}`;
const isProfileUuid = id => typeof id === 'string' && /^[a-f0-9-]{36}$/.test(id);
const validId = id => id === originalProfile || isProfileUuid(id);
const validRevision = revision => typeof revision === 'string' && /^[a-f0-9]{14}$/.test(revision);

export function readProfiles(hash) {
  const raw = localStorage.getItem(catalogKey(hash));
  if (raw === null) return { version: 1, selected: originalProfile, next: 2, profiles: [{ id: originalProfile, name: 'Save 1' }] };
  const catalog = JSON.parse(raw);
  if (catalog?.version !== 1 || !Number.isSafeInteger(catalog.next) || catalog.next < 2 ||
      !Array.isArray(catalog.profiles) ||
      catalog.profiles.some(p => !validId(p?.id) || typeof p.name !== 'string' ||
        (p.recovery !== undefined && p.recovery !== true) ||
        (p.epoch !== undefined && !validId(p.epoch))) ||
      new Set(catalog.profiles.map(p => p.id)).size !== catalog.profiles.length ||
      !catalog.profiles.some(p => p.id === catalog.selected)) {
    throw new Error('Save data profile list is invalid');
  }
  return catalog;
}

export function writeProfiles(hash, catalog) {
  localStorage.setItem(catalogKey(hash), JSON.stringify(catalog));
}

function addProfile(catalog, recovery = false, id = crypto.randomUUID()) {
  const profile = {
    id,
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

export function decodeSram(data, size) {
  if (data === null) return null;
  if (data.length !== Math.ceil(size / 3) * 4) throw new Error('Save size does not match cartridge SRAM');
  const decoded = atob(data);
  if (decoded.length !== size) throw new Error('Save size does not match cartridge SRAM');
  return Uint8Array.from(decoded, c => c.charCodeAt(0));
}

export function readSramData(hash, id) {
  return profileRecord(hash, id).data;
}

function readSram(hash, id, size) {
  return decodeSram(readSramData(hash, id), size);
}

export function encodeSram(bytes) {
  let binary = '';
  for (const value of bytes) binary += String.fromCharCode(value);
  return btoa(binary);
}

function writeSram(hash, id, bytes) {
  writeSramData(hash, id, encodeSram(bytes));
}

export function writeSramData(hash, id, data, recoveryRevision) {
  const key = sramKey(hash, id);
  const legacy = id === originalProfile;
  const raw = localStorage.getItem(key);
  const previous = legacy ? { data: raw } : JSON.parse(raw);
  const changed = previous?.data !== data;
  const updated = changed ? Date.now() : previous?.updated;
  const revision = recoveryRevision ?? (changed ? undefined : previous?.recoveryRevision);
  const value = legacy ? data : JSON.stringify({ data, updated, recoveryRevision: revision });
  // Revoke the replay claim before changing the bytes. A failed write leaves
  // the previous SRAM intact, so it is safe to reinstate its claim.
  const receiptKey = replayKey(hash, id);
  const receipt = changed ? localStorage.getItem(receiptKey) : null;
  if (receipt !== null) localStorage.removeItem(receiptKey);
  try {
    localStorage.setItem(key, value);
  } catch (error) {
    if (receipt !== null) {
      try { localStorage.setItem(receiptKey, receipt); } catch {}
    }
    throw error;
  }
  if (legacy && changed) {
    // A timestamp is advisory. The original save has already been written.
    try { localStorage.setItem(legacyTimeKey(hash), String(updated)); } catch {}
  }
}

export function removeSram(hash, id) {
  localStorage.removeItem(replayKey(hash, id));
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

export function addStoredProfile(hash, catalog, data, recovery = false) {
  const profile = addProfile(catalog, recovery);
  writeSram(hash, profile.id, data);
  return profile;
}

export function persistNewProfile(hash, catalog, profile) {
  try {
    writeProfiles(hash, catalog);
  } catch (error) {
    try { removeSram(hash, profile.id); } catch {}
    throw error;
  }
}

export function recoverSram(hash, id, data, source) {
  const catalog = readProfiles(hash);
  const existing = catalog.profiles.find(profile => {
    if (!profile.recovery || profile.id === id) return false;
    try { return sameSram(data, readSram(hash, profile.id, data.length)); }
    catch { return false; }
  });
  if (existing) {
    // Reusing these bytes as a restore backup revokes earlier write claims,
    // including an unadopted journal replay targeting this recovery profile.
    existing.epoch = crypto.randomUUID();
    writeProfiles(hash, catalog);
  } else {
    const recovery = addStoredProfile(hash, catalog, data, source);
    persistNewProfile(hash, catalog, recovery);
  }
}

// Call while holding the ROM's save lock. The returned target is safe to
// update only as long as its stored bytes and epoch still match.
export function persistSram(hash, { id, base, epoch, data, recoveryId, revision }, { dirty = true, settled = false, replaying = false } = {}) {
  const catalog = readProfiles(hash);
  const profile = catalog.profiles.find(profile => profile.id === id);
  const stored = profile ? readSramData(hash, id) : undefined;
  if (profile && epoch === profile.epoch) {
    // The saved bytes may already be a replay from this writer. Preserve
    // their ordering even when the originating document has navigated away.
    const writer = replaying && isProfileUuid(recoveryId) && validRevision(revision) ? `${recoveryId}:` : null;
    const receipt = writer && localStorage.getItem(replayKey(hash, id));
    const predecessor = receipt?.startsWith(writer) ? receipt.slice(writer.length) : null;
    const sameWriter = validRevision(predecessor);
    if (sameWriter && revision <= predecessor) {
      return data === stored ? { id, base: data, epoch } : { id, epoch };
    }
    if ((settled ? data : base) === stored || sameWriter) {
      if (!settled) writeSramData(hash, id, data);
      const saved = { id, base: data, epoch };
      if (writer && (data !== stored || sameWriter)) saved.receipt = `${writer}${revision}`;
      return saved;
    }
  }
  if (!dirty || data === stored) return { id, epoch };

  // The reserved ID belongs to this writer. Its revision is stored atomically
  // with the replayed bytes and cleared by an independent change to those bytes.
  const recovery = catalog.profiles.find(profile => profile.id === recoveryId);
  if (recovery?.recovery && recovery.epoch === undefined) {
    const saved = profileRecord(hash, recovery.id);
    if (saved.data === data && (revision === undefined || revision === saved.recoveryRevision)) {
      return { id: recovery.id, base: data };
    }
    if (!settled && validRevision(revision) && validRevision(saved.recoveryRevision)) {
      if (revision < saved.recoveryRevision) return { id, epoch };
      if (revision > saved.recoveryRevision) {
        writeSramData(hash, recovery.id, data, revision);
        return { id: recovery.id, base: data };
      }
    }
  }
  if (settled) return { id, epoch };

  const fallback = addProfile(catalog, 'Another tab', !recovery && isProfileUuid(recoveryId) ? recoveryId : undefined);
  writeSramData(hash, fallback.id, data, revision);
  persistNewProfile(hash, catalog, fallback);
  return { id: fallback.id, base: data };
}

// A closing/frozen tab might not get another turn to acquire the Web Lock.
// Stage its dirty bytes under a unique key before yielding to the browser.
const pendingPrefix = 'snes-rust:pending-sram:';

export function stageSram(hash, entry, fallbackKey) {
  const key = `${pendingPrefix}${hash}:${crypto.randomUUID()}`;
  const value = JSON.stringify(entry);
  try {
    localStorage.setItem(key, value);
    return { storage: localStorage, key, durable: true };
  } catch (error) {
    if (error.name !== 'QuotaExceededError') throw error;
    // The tab's session storage survives navigation and has its own quota.
    // Replaying the entry still requires the ROM lock and the same stale-save checks.
    // Replace this tab's prior entry in place at quota. If it has a durable
    // copy, sharing the key lets this tab prefer its newer session entry.
    const tabKey = fallbackKey ?? key;
    sessionStorage.setItem(tabKey, value);
    return { storage: sessionStorage, key: tabKey, durable: false };
  }
}

function* pendingSramEntries(hash, includeFallback = true) {
  const prefix = `${pendingPrefix}${hash}:`;
  let tabStorage;
  try { tabStorage = sessionStorage; } catch { /* The tab journal may be unavailable. */ }
  const tabKeys = Object.keys(tabStorage ?? {}).filter(key => key.startsWith(prefix));
  for (const key of Object.keys(localStorage)) {
    if (key.startsWith(prefix) && (includeFallback || !tabKeys.includes(key))) yield { storage: localStorage, key };
  }
  for (const key of tabKeys) yield { storage: tabStorage, key };
}

export function hasPendingSram(hash) {
  return !pendingSramEntries(hash).next().done;
}

export function hasSessionPendingSram() {
  try { return Object.keys(sessionStorage).some(key => key.startsWith(pendingPrefix)); }
  catch { return false; }
}

export function* readPendingSram(hash, size, { profileId, includeFallback = true } = {}) {
  for (const { storage, key } of pendingSramEntries(hash, includeFallback)) {
    const raw = storage.getItem(key);
    if (raw === null) continue;
    const entry = JSON.parse(raw);
    if (typeof entry?.data !== 'string') throw new Error('Pending save data is invalid');
    if (profileId !== undefined && entry.id !== profileId) continue;
    yield { storage, key, entry, data: decodeSram(entry.data, size) };
  }
}

export function settlePendingSram(hash, size) {
  for (const { storage, key, entry } of readPendingSram(hash, size, { includeFallback: false })) {
    const saved = persistSram(hash, entry, { replaying: true });
    if (storage !== localStorage && saved.base === entry.data) localStorage.removeItem(key);
    storage.removeItem(key);
    if (saved.receipt) {
      // Reclaim the journal's space first. A session-only save can still
      // succeed at quota even when localStorage has no room for a receipt.
      try { localStorage.setItem(replayKey(hash, saved.id), saved.receipt); } catch {}
    }
  }
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

// Serialize SRAM writes, profile changes and history mutations across tabs.
export function lockSaves(hash, callback) {
  return navigator.locks.request(`snes-rust:saves:${hash}`, callback);
}
