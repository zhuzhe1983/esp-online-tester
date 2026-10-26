// Firmware stays at its publisher and in the user's browser, never on our server.
// Only hash-pinned remote artifacts are reusable; callers recheck SHA-256 on hits.
const MAX_BYTES = 64 * 1024 * 1024;
const keyFor = (url, hash) => typeof hash === 'string' && /^[a-f0-9]{64}$/i.test(hash) ? `${url}\n${hash.toLowerCase()}` : null;

export function createFirmwareCache({ indexedDB = globalThis.indexedDB, maxBytes = MAX_BYTES, dbName = 'esp-online-tester-firmware-v1' } = {}) {
  const memory = new Map(); let memoryBytes = 0, connection, opening, epoch = 0;
  function remember(key, data) {
    if (memory.has(key)) { memoryBytes -= memory.get(key).byteLength; memory.delete(key); }
    if (data.byteLength > maxBytes) return;
    memory.set(key, data.slice()); memoryBytes += data.byteLength;
    while (memoryBytes > maxBytes || memory.size > 32) { const oldest = memory.keys().next().value; memoryBytes -= memory.get(oldest).byteLength; memory.delete(oldest); }
  }
  function open() {
    if (connection) return Promise.resolve(connection);
    if (!indexedDB) return Promise.resolve(null);
    if (!opening) opening = new Promise(resolve => {
      let finished = false;
      const done = db => { if (finished) { db?.close(); return; } finished = true; clearTimeout(timer); connection = db; resolve(db); };
      const timer = setTimeout(() => done(null), 2000);
      try {
        const request = indexedDB.open(dbName, 1);
        request.onupgradeneeded = () => request.result.createObjectStore('artifacts', { keyPath: 'key' });
        request.onerror = request.onblocked = () => done(null);
        request.onsuccess = () => { request.result.onversionchange = () => { request.result.close(); connection = null; opening = null; }; done(request.result); };
      } catch { done(null); }
    });
    return opening;
  }
  async function transaction(mode, operation) {
    const db = await open(); if (!db) return { ok: false };
    return new Promise(resolve => {
      let value, tx;
      try {
        tx = db.transaction('artifacts', mode);
        tx.oncomplete = () => resolve({ ok: true, value });
        tx.onerror = tx.onabort = () => resolve({ ok: false });
        operation(tx.objectStore('artifacts'), result => { value = result; });
      } catch { try { tx?.abort(); } catch {} resolve({ ok: false }); }
    });
  }
  return {
    async get(url, hash, limit = maxBytes) {
      const key = keyFor(url, hash); if (!key) return null;
      const generation = epoch;
      if (memory.has(key)) { const data = memory.get(key); if (data.byteLength > limit) return null; remember(key, data); return { data: data.slice(), storage: 'memory' }; }
      const result = await transaction('readonly', (store, done) => { const request = store.get(key); request.onsuccess = () => done(request.result); });
      const row = result.value;
      if (generation !== epoch || !row || !(row.data instanceof ArrayBuffer) || row.data.byteLength > limit || row.data.byteLength > maxBytes) return null;
      const data = new Uint8Array(row.data); remember(key, data);
      await transaction('readwrite', store => { const request = store.get(key); request.onsuccess = () => { if (request.result && generation === epoch) store.put({ ...request.result, lastUsed: Date.now() }); }; });
      return generation === epoch ? { data, storage: 'persistent' } : null;
    },
    async putVerified(url, hash, data) {
      const key = keyFor(url, hash); if (!key || !(data instanceof Uint8Array) || !data.length || data.length > maxBytes) return false;
      const generation = epoch; remember(key, data);
      const copy = data.slice().buffer;
      const result = await transaction('readwrite', store => {
        if (generation !== epoch) return;
        store.put({ key, url, sha256: hash.toLowerCase(), data: copy, bytes: copy.byteLength, lastUsed: Date.now() });
        const request = store.getAll();
        request.onsuccess = () => {
          const rows = request.result.sort((a, b) => b.lastUsed - a.lastUsed); let total = 0, count = 0;
          for (const row of rows) { total += row.data?.byteLength || 0; if (total > maxBytes || ++count > 32) store.delete(row.key); }
        };
      });
      return result.ok;
    },
    async remove(url, hash) {
      const key = keyFor(url, hash); if (!key) return;
      if (memory.has(key)) { memoryBytes -= memory.get(key).byteLength; memory.delete(key); }
      await transaction('readwrite', store => store.delete(key));
    },
    async clear() { epoch++; memory.clear(); memoryBytes = 0; const result = await transaction('readwrite', store => store.clear()); return { cleared: true, persistent: result.ok }; },
  };
}

export const firmwareCache = createFirmwareCache();
