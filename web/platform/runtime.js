import {MAX_FIRMWARE_FILE_BYTES as MAX_FILE, MAX_FIRMWARE_BUNDLE_BYTES as MAX_TOTAL} from './firmware-limits.js';
import {createFat32Image,validateSdImage,MAX_SD_BYTES} from './sd-card-image.js';
import {createSdCardStorage} from './sd-card-storage.js';
// Browser-only adapter for the upstream ESP32 WASM worker. No server or mock device state.
import { loadCatalog, validateScene } from './catalog.js';
import { buildFirmwareIssueUrl } from './watchlist.js';
import { orientationSample } from './imu.js';
import { firmwareCache } from './firmware-cache.js';
import { assembleLocalFirmware } from './local-firmware.js';
import { createDeviceStorage, deviceStorageScope } from './device-storage.js';

export const API_VERSION = '1.0.0';
const MIB = 1024 * 1024;
const KINDS = { rom: 0, bootloader: 1, ptable: 2, app: 3, elf: 4, flash: 5, script: 6, picture: 7 };
const SERIAL_LIMIT = 64 * 1024;
const clone = value => value == null ? value : structuredClone(value);
const error = (code, message, details) => Object.assign(new Error(message), { code, details });
const requireValue = (condition, code, message, details) => { if (!condition) throw error(code, message, details); };
const failure = e => ({ code: e.code || 'RUNTIME_ERROR', message: e.message || String(e), ...(e.details ? { details: e.details } : {}) });
const bytesOf = data => data instanceof ArrayBuffer ? new Uint8Array(data) : new Uint8Array(data.buffer, data.byteOffset, data.byteLength);

export function resolveFirmwareUrl(value, baseUrl) {
  let url;
  try { url = new URL(value, baseUrl); } catch { throw error('INVALID_URL', 'Firmware URL is invalid.'); }
  requireValue(['https:', 'http:'].includes(url.protocol), 'INVALID_URL', 'Firmware URLs must use HTTP or HTTPS.');
  requireValue(!url.username && !url.password, 'INVALID_URL', 'Firmware URLs must not contain credentials.');
  if (new URL(baseUrl).protocol === 'https:') requireValue(url.protocol === 'https:', 'MIXED_CONTENT', 'An HTTPS page cannot load HTTP firmware.');
  return url.href;
}

export async function sha256(data) {
  requireValue(globalThis.crypto?.subtle, 'CRYPTO_UNAVAILABLE', 'SHA-256 requires HTTPS or localhost.');
  return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', data)), b => b.toString(16).padStart(2, '0')).join('');
}

async function checkedHash(data, expected) {
  if (expected != null) requireValue(/^[a-f0-9]{64}$/i.test(expected), 'INVALID_HASH', 'SHA-256 must contain 64 hexadecimal characters.');
  const actual = await sha256(data);
  requireValue(!expected || actual === expected.toLowerCase(), 'HASH_MISMATCH', 'Firmware SHA-256 does not match.', { expected, actual });
  return actual;
}

export async function fetchBytes(url, { fetchImpl = globalThis.fetch, maxBytes = MAX_FILE, signal, onProgress } = {}) {
  const controller = new AbortController();
  const abort = () => controller.abort();
  const checkAbort = () => requireValue(!controller.signal.aborted, 'FETCH_ABORTED', 'Firmware download was cancelled or timed out.', { url });
  signal?.addEventListener('abort', abort, { once: true });
  if (signal?.aborted) controller.abort();
  const timeout = setTimeout(abort, 60000);
  try {
    checkAbort();
    const response = await fetchImpl(url, { mode: 'cors', cache: 'no-store', credentials: 'omit', signal: controller.signal });
    checkAbort();
    requireValue(response.ok, 'HTTP_ERROR', `Firmware request failed: HTTP ${response.status}. Please report this source using the issue link.`, { url, status: response.status, issueUrl: buildFirmwareIssueUrl({ url, status: response.status, reason: 'Firmware request failed.' }) });
    const length = Number(response.headers.get('content-length'));
    if (length > maxBytes) {
      await response.body?.cancel();
      throw error('FILE_TOO_LARGE', `File exceeds ${maxBytes / MIB} MiB limit.`, {url, maxBytes});
    }
    // Fetch exposes decoded bytes. A compressed Content-Length is not a usable
    // denominator for those bytes, and absent/invalid lengths remain unknown.
    const encoding = response.headers.get('content-encoding');
    let totalBytes = Number.isSafeInteger(length) && length > 0 && (!encoding || encoding.toLowerCase() === 'identity') ? length : null;
    const progress = (loadedBytes, done = false) => {
      checkAbort();
      if (totalBytes !== null && (loadedBytes > totalBytes || done && loadedBytes !== totalBytes)) totalBytes = null;
      onProgress?.({ url, loadedBytes, totalBytes, done });
      checkAbort();
    };
    progress(0);
    if (!response.body?.getReader) {
      const bytes = new Uint8Array(await response.arrayBuffer());
      checkAbort();
      requireValue(bytes.byteLength <= maxBytes, 'FILE_TOO_LARGE', `File exceeds ${maxBytes / MIB} MiB limit.`, {url, maxBytes});
      progress(bytes.byteLength, true);
      return bytes;
    }
    const reader = response.body.getReader(), chunks = [];
    const cancelReader = () => { void reader.cancel().catch(() => {}); };
    controller.signal.addEventListener('abort', cancelReader, { once: true });
    let total = 0;
    try {
      while (true) {
        const { done, value } = await reader.read();
        checkAbort();
        if (done) break;
        total += value.byteLength;
        requireValue(total <= maxBytes, 'FILE_TOO_LARGE', `File exceeds ${maxBytes / MIB} MiB limit.`, {url, maxBytes});
        chunks.push(value); progress(total);
      }
      progress(total, true);
    } catch (e) { await reader.cancel().catch(() => {}); throw e; }
    finally { controller.signal.removeEventListener('abort', cancelReader); reader.releaseLock(); }
    const bytes = new Uint8Array(total);
    let offset = 0;
    for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
    return bytes;
  } catch (e) {
    if (e.code) throw e;
    if (e.name === 'AbortError') throw error('FETCH_ABORTED', 'Firmware download was cancelled or timed out.', { url });
    throw error('FETCH_FAILED', 'Firmware download failed. Check the URL, network and Access-Control-Allow-Origin header; a static site cannot bypass CORS.', { url, issueUrl: buildFirmwareIssueUrl({ url, reason: 'Network or CORS failure; source availability is not confirmed.' }) });
  } finally { clearTimeout(timeout); signal?.removeEventListener('abort', abort); }
}

export function decodeFramebuffer(data) {
  const bytes = bytesOf(data);
  requireValue(bytes.length >= 5 && bytes[0] === 1, 'INVALID_FRAME', 'Expected a display framebuffer.');
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const width = view.getUint16(1, true), height = view.getUint16(3, true), count = width * height;
  requireValue(width > 0 && height > 0 && count <= 2048 * 2048 && bytes.length === 5 + count * 2, 'INVALID_FRAME', 'Invalid framebuffer dimensions or payload length.');
  const pixels = new Uint8ClampedArray(count * 4);
  for (let i = 0; i < count; i++) {
    const color = view.getUint16(5 + i * 2, true), at = i * 4;
    pixels[at] = Math.round(((color >> 11) & 31) * 255 / 31);
    pixels[at + 1] = Math.round(((color >> 5) & 63) * 255 / 63);
    pixels[at + 2] = Math.round((color & 31) * 255 / 31);
    pixels[at + 3] = 255;
  }
  return { width, height, pixels };
}

// A URL ending in .bin does not establish that it contains a bootable flash image.
export function validateMergedImage(data, board, flashBytes, {partitionTableOffset = 0x8000} = {}) {
  const bytes = bytesOf(data), bootOffset = /^(esp32|ttgo-tdisplay)$/.test(board) ? 0x1000 : 0;
  requireValue(Number.isSafeInteger(partitionTableOffset) && partitionTableOffset >= 0x8000 && partitionTableOffset % 0x1000 === 0 && partitionTableOffset + 0x1000 <= flashBytes, 'INVALID_PARTITION_TABLE', 'Partition-table offset is outside flash or not sector aligned.');
  requireValue(bytes.length <= flashBytes, 'FLASH_OVERFLOW', 'Merged firmware exceeds the selected product flash size.');
  requireValue(bytes.length > partitionTableOffset + 32 && bytes[bootOffset] === 0xe9 && bytes[partitionTableOffset] === 0xaa && bytes[partitionTableOffset + 1] === 0x50,
    'INCOMPLETE_IMAGE', 'A raw BIN must be a merged flash image starting at offset 0, containing a bootloader and partition table at 0x8000. Use a manifest for separate app, bootloader and partition images.');
  // ESP-IDF image header chip IDs: ESP32=0, S3=9, C3=5, C6=13, P4=18.
  const chipId = bytes[bootOffset + 12] | bytes[bootOffset + 13] << 8;
  const expected = /^(esp32|ttgo-tdisplay)$/.test(board) ? 0 : board === 'esp32c3' ? 5 : board.includes('c6') ? 13 : board === 'esp32p4' ? 18 : 9;
  requireValue(chipId === expected, 'CHIP_MISMATCH', 'Firmware image targets a different ESP32 chip.', { expected, actual: chipId });
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const completeImage = (at, end) => {
    // Same 24-byte header and 8-byte segment headers as esp-soc/src/image.rs.
    requireValue(at + 24 <= end && bytes[at] === 0xe9 && bytes[at + 1] > 0 && bytes[at + 1] <= 16, 'INCOMPLETE_IMAGE', 'Merged firmware contains an invalid image header.');
    requireValue(view.getUint16(at + 12, true) === expected, 'CHIP_MISMATCH', 'An image inside the merged firmware targets a different chip.');
    let offset = at + 24;
    for (let i = 0; i < bytes[at + 1]; i++) {
      requireValue(offset + 8 <= end, 'INCOMPLETE_IMAGE', 'Merged firmware contains a truncated segment header.');
      offset += 8 + view.getUint32(offset + 4, true);
      requireValue(offset <= end, 'INCOMPLETE_IMAGE', 'Merged firmware contains a truncated image segment.');
    }
  };
  completeImage(bootOffset, Math.min(bytes.length, partitionTableOffset));
  let apps = 0;
  for (let offset = partitionTableOffset; offset + 32 <= Math.min(bytes.length, partitionTableOffset + 0x1000); offset += 32) {
    if (view.getUint16(offset, true) !== 0x50aa) break;
    const at = view.getUint32(offset + 4, true), size = view.getUint32(offset + 8, true);
    requireValue(size > 0 && at >= partitionTableOffset + 0x1000 && at + size <= flashBytes, 'FLASH_OVERFLOW', 'Partition table contains a partition outside product flash.');
    if (bytes[offset + 2] === 0 && at < bytes.length && bytes[at] === 0xe9) { completeImage(at, Math.min(bytes.length, at + size)); apps++; }
  }
  requireValue(apps > 0, 'INCOMPLETE_IMAGE', 'Merged firmware contains no complete app image at a declared partition offset.');
  return true;
}

const romFor = board => board === 'esp32c3' ? 'esp32c3_rev3_rom.elf' : board === 'esp32p4' ? 'esp32p4_rev0_rom.elf' : board.includes('c6') ? 'esp32c6_rev0_rom.elf' : /^(esp32|ttgo-tdisplay)$/.test(board) ? 'esp32_rev300_rom.elf' : 'esp32s3_rev0_rom.elf';

export function createPlatformRuntime({ catalog: initialCatalog, onEvent, fetchImpl = globalThis.fetch, workerFactory = url => new Worker(url, { type: 'module' }), baseUrl = globalThis.location?.href || new URL('./', import.meta.url).href, assetBaseUrl = new URL('../', import.meta.url).href, requestTimeout = 30000, deviceStorage = createDeviceStorage(), sdStorage = createSdCardStorage() } = {}) {
  let catalog = initialCatalog, scene = null, product = null, status = 'idle', worker = null, aborter = null;
  let lastError = null, firmware = null, loaded = null, frame = null, frameNumber = 0, serial = '', serialByPort = {}, stats = {}, queue = Promise.resolve();
  const emptyStorage = () => ({status:'unloaded', scope:null, backend:null, restored:false, pristine:true, revision:0, epoch:0, lastSavedAt:null, changedPages:0, error:null});
  const emptySd=()=>({supported:false,present:false,bytes:0,format:null,status:'unloaded',backend:null,restored:false,pristine:true,changedPages:0,error:null});
  let sdCard=emptySd(),sdSession=null,leds=[];
  let storage = emptyStorage(), storageSession = null, storageSequence = 0;
  let booted = false, inputSequence = 0, loadCancelled = false, progressGeneration = 0, disposing = false;
  const imuModel = () => ({stopwatch:'bmi270-register-subset',atoms3:'mpu6886-register-subset',atoms3r:'bmi270-register-subset'}[product?.runtime?.board] || null);
  const defaultImu = () => ({enabled: true, source: product?.runtime?.board === 'stopwatch' ? 'scene' : 'agent', ...orientationSample([0,0,0,1]), delivery: 'configured', guestTime: null});
  let imu = defaultImu(), imuPreviousAt = null, imuGeneration = 0;
  const pending = new Map(), listeners = new Set(onEvent ? [onEvent] : []);
  const snapshot = () => ({ version: API_VERSION, status, scene: clone(scene), product: clone(product), firmware: clone(firmware), serial, serialByPort: { ...serialByPort }, stats: clone(stats), storage: clone(storage), sdCard:clone(sdCard),leds:clone(leds), imu: imuModel() ? clone(imu) : null, frame: frame ? { width: frame.width, height: frame.height, frameNumber } : null, lastError: clone(lastError) });
  const emit = event => { for (const listener of listeners) { try { listener(event); } catch (e) { console.error('ESP tester event handler:', e); } } };
  const loadReporter = (loadId, signal) => {
    let fileIndex = 0;
    const send = event => {
      if (loadId === progressGeneration && status === 'loading' && !signal.aborted) emit({ type: 'load-progress', loadId, ...event });
    };
    return {
      file(kind, url) {
        const index = ++fileIndex;
        return progress => send({ phase: 'download', kind, fileIndex: index, ...progress, url });
      },
      prepare(stage, kind = null, url = null) { send({ phase: 'prepare', stage, kind, url, fileIndex: null, loadedBytes: null, totalBytes: null, done: false }); },
      cache(kind, url, bytes, storage) { send({ phase: 'cache', kind, url, storage, fileIndex: ++fileIndex, loadedBytes: bytes, totalBytes: bytes, done: true }); },
    };
  };
  const setStatus = value => { status = value; emit({ type: 'state', state: snapshot() }); };
  const report = e => { lastError = failure(e); emit({ type: 'error', error: clone(lastError) }); };
  const alive = () => requireValue(!disposing && status !== 'disposed', 'DISPOSED', 'This emulator session has been disposed.');
  const exclusive = fn => {
    const next = queue.then(() => { alive(); return fn(); });
    queue = next.catch(() => {});
    return next.catch(e => { report(e); throw e; });
  };
  const resolve = (key, value) => { const item = pending.get(key); if (!item) return; pending.delete(key); clearTimeout(item.timeout); item.resolve(value); };
  const rejectPending = e => { for (const item of pending.values()) { clearTimeout(item.timeout); item.reject(e); } pending.clear(); };
  const stopWorker = () => { worker?.terminate(); worker = null; rejectPending(error('SESSION_REPLACED', 'The emulator session was replaced.')); booted = false; imuPreviousAt = null; imu.gyroDps = [0,0,0]; imu.delivery = 'configured'; imu.guestTime = null; };
  const ask = (key, message, transfer = []) => new Promise((resolveRequest, reject) => {
    requireValue(worker, 'NOT_READY', 'The emulator worker is not ready.');
    requireValue(!pending.has(key), 'BUSY', 'A worker command is already pending.');
    const timeout = setTimeout(() => { pending.delete(key); reject(error('WORKER_TIMEOUT', `Emulator did not acknowledge ${message.op}.`)); }, requestTimeout);
    pending.set(key, { resolve: resolveRequest, reject, timeout });
    worker.postMessage(message, transfer);
  });
  const appendSerial = (source, data) => {
    serial = (serial + data).slice(-SERIAL_LIMIT);
    serialByPort[source] = ((serialByPort[source] || '') + data).slice(-SERIAL_LIMIT);
    emit({ type: 'serial', source, data });
  };
  const receive = message => {
    if (message.text !== undefined) {
      let text;
      try { text = JSON.parse(message.text); } catch { appendSerial('emulator', String(message.text)); return; }
      if (text.t === 'serial') appendSerial(text.src || 'usb', text.data || '');
      else if (text.t === 'emu') appendSerial('emulator', `${text.msg}\n`);
      else if (text.t === 'stat') { stats = { ...stats, ...text }; emit({ type: 'stats', stats: clone(stats) }); }
      else if (text.t === 'ring') {leds=clone(text.leds);emit({type:'leds',leds:clone(leds)});}
      else if (text.t === 'board') emit({ type: 'board', board: text });
    }
    if (message.bin !== undefined) {
      try {
        if (new Uint8Array(message.bin)[0] === 1) {
          frame = decodeFramebuffer(message.bin); frameNumber++;
          emit({ type: 'frame', ...frame, frameNumber, source: 'emulator' });
        }
      } catch (e) { report(e); }
      finally { if (message.ack) worker?.postMessage({ op: 'frame-ack' }); }
    }
    if (message.log !== undefined) appendSerial('emulator', `${message.log}\n`);
    if (message.pace) { stats = { ...stats, ...message.pace }; emit({ type: 'stats', stats: clone(stats) }); }
    if (message.flashReply !== undefined) resolve(`flash:${message.flashReply}`, message);
    if (message.flashDelta) { try { receiveFlash(message.flashDelta); } catch(e) { storageFailure(storageSession,e); } }
    if (message.ready) resolve('ready', message);
    if (message.sdDelta) {try{receiveSd(message.sdDelta);}catch(e){sdFailure(e);}}
    if (message.sdReply !== undefined) resolve(`sd:${message.sdReply}`,message);
    if (message.created !== undefined) resolve('created', message);
    if (message.loaded !== undefined) resolve(`loaded:${message.loaded}`, message);
    if (message.started !== undefined) resolve('started', message);
    if (message.resumed !== undefined) resolve('resumed', message);
    if (message.inputQueued !== undefined) resolve(`input:${message.inputQueued}`, message);
    if (message.imuApplied !== undefined) resolve(`imu:${message.imuApplied}`, message);
    if (message.stopped !== undefined) {
      const requested = pending.has('stopped');
      resolve('stopped', message);
      if (!requested) {
        if (message.stopped !== 0) { const e = error('EMULATOR_STOPPED', `Emulator stopped with code ${message.stopped}.`); rejectPending(e); report(e); setStatus('error'); }
        else if (status === 'running') setStatus('paused');
      }
    }
  };
  const storageEvent = () => emit({type:'storage', storage:clone(storage)});
  const detachStorage = () => { if (storageSession) storageSession.closed = true; storageSession = null; storage = emptyStorage(); storageEvent(); };
  const flashAsk = async (op, fields = {}, transfer = []) => {
    const requestId = ++storageSequence, result = await ask(`flash:${requestId}`, {op, requestId, ...fields}, transfer);
    requireValue(result.ok !== false, 'STORAGE_WORKER_ERROR', result.error || 'Cannot access emulator Flash.');
    return {...result, requestId};
  };
  const storageFailure = (session, e) => {
    if (session !== storageSession || session.closed) return;
    storage.status = e.code === 'STORAGE_CONFLICT' ? 'conflict' : 'error'; storage.error = failure(e); storageEvent(); report(e);
    if (['STORAGE_CONFLICT','STORAGE_WORKER_ERROR'].includes(e.code)) { session.closed=true; stopWorker(); setStatus('error'); }
  };
  const persistFlash = session => {
    if (session.saving) return session.saving;
    session.saving = (async () => {
      while (!session.closed && session.savedVersion < session.version) {
        const version = session.version;
        storage.status = 'saving'; storageEvent();
        const result = await deviceStorage.save(session.scope, {epoch:storage.epoch, revision:storage.revision, pages:[...session.pages].map(([offset,data])=>({offset,data}))});
        if (session.closed || session !== storageSession) return;
        session.savedVersion = version;
        Object.assign(storage, {status:result.persistent?'saved':'memory-only', backend:result.backend, epoch:result.row.epoch, revision:result.row.revision, lastSavedAt:result.row.savedAt, error:result.warning?{code:'STORAGE_MEMORY_ONLY',message:result.warning}:null});
        storageEvent();
      }
    })().catch(e=>{storageFailure(session,e); throw e;}).finally(()=>{session.saving=null;});
    // Automatic saves are observable through storage/error events; callers of flush
    // also receive the rejection. No unhandled promise hides a failed transaction.
    session.saving.catch(()=>{});
    return session.saving;
  };
  const receiveFlash = ({watchId,pages}) => {
    const session = storageSession;
    if (!session || session.closed || session.watchId !== watchId) return;
    for (const {offset,data} of pages) {
      requireValue(Number.isInteger(offset) && offset>=0 && offset%4096===0 && offset+4096<=session.baseline.length && data instanceof ArrayBuffer && data.byteLength===4096, 'STORAGE_WORKER_ERROR', 'Worker sent invalid Flash page bounds.');
      const bytes = new Uint8Array(data); let differs = false;
      for (let i=0;i<4096;i++) if (bytes[i]!==session.baseline[offset+i]) {differs=true;break;}
      if (differs) session.pages.set(offset,data); else session.pages.delete(offset);
    }
    session.version++;
    storage.pristine = session.pages.size===0; storage.changedPages = session.pages.size;
    void persistFlash(session);
  };
  const flushStorage = async () => {
    const session = storageSession;
    requireValue(session && storage.scope, 'STORAGE_NOT_LOADED', 'Load firmware before accessing its device storage.');
    requireValue(storage.status !== 'conflict', 'STORAGE_CONFLICT', storage.error?.message || 'Device storage changed in another tab.');
    if (worker) await flashAsk('flash-flush');
    await persistFlash(session);
    return clone(storage);
  };
  const preserveStorage = async () => {
    await Promise.all([storageSession&&storage.status!=='conflict'?flushStorage():null,sdSession&&!sdSession.closed&&sdCard.status!=='conflict'?flushSd():null]);
  };
  const initializeStorage = async (bundle, supported, progress) => {
    detachStorage();
    if (!supported) { storage.status='error'; storage.pristine=null; storage.error={code:'STORAGE_UNSUPPORTED',message:'This emulator build does not expose writable Flash snapshots.'}; storageEvent(); return; }
    storage.status='loading'; storageEvent(); progress.prepare('device-storage');
    const snapshot = await flashAsk('flash-snapshot'), baseline = new Uint8Array(snapshot.data);
    requireValue(baseline.length===bundle.cfg.flash_mb*MIB, 'STORAGE_WORKER_ERROR', 'Emulator Flash capacity does not match the device.');
    const scope = deviceStorageScope({productId:product.id, board:bundle.cfg.board, chip:product.chip || 'esp32s3', flashBytes:baseline.length, firmwareSha256:await sha256(baseline)});
    const result = await deviceStorage.read(scope);
    requireValue(!aborter.signal.aborted, 'FETCH_ABORTED', 'Firmware load was cancelled.');
    const pages = new Map(result.row.pages.map(page=>[page.offset,page.data]));
    const session = {scope,baseline,pages,version:0,savedVersion:0,saving:null,closed:false,watchId:null};
    storageSession=session;
    storage={status:result.persistent?'clean':'memory-only', scope, backend:result.backend, restored:pages.size>0, pristine:pages.size===0, revision:result.row.revision, epoch:result.row.epoch, lastSavedAt:result.row.savedAt, changedPages:pages.size, error:result.warning?{code:'STORAGE_MEMORY_ONLY',message:result.warning}:null};
    if (pages.size) {
      const restore=[...pages].map(([offset,data])=>({offset,data:data.slice(0)}));
      await flashAsk('flash-restore',{pages:restore},restore.map(page=>page.data));
    }
    const watching=await flashAsk('flash-watch'); session.watchId=watching.requestId;
    storageEvent();
  };
  const unsubscribeStorage = deviceStorage.subscribe(change => {
    const session=storageSession;
    if (!session || session.closed || change.key!==session.scope.key || change.epoch<storage.epoch || change.epoch===storage.epoch && change.revision<=storage.revision) return;
    storageFailure(session,error('STORAGE_CONFLICT','Another tab saved or cleared this device storage. Reload the same firmware to restore its latest state.'));
  });
  const sdEvent=()=>emit({type:'sd-card',sdCard:clone(sdCard)});
  const sdAsk=async(op,fields={},transfer=[])=>{const requestId=++storageSequence;const result=await ask(`sd:${requestId}`,{op,requestId,...fields},transfer);requireValue(result.ok!==false,'STORAGE_WORKER_ERROR',result.error||'Cannot access virtual SD card.');return {...result,requestId};};
  const sdFailure=(e,session=sdSession)=>{if(!session||session.closed||session!==sdSession||disposing||status==='disposed')return;sdCard.status=e.code==='STORAGE_CONFLICT'?'conflict':'error';sdCard.error=failure(e);sdEvent();report(e);if(e.code==='STORAGE_CONFLICT'){if(sdSession)sdSession.closed=true;stopWorker();setStatus('error');}};
  const persistSd=session=>{
    if(session.saving)return session.saving;
    session.saving=(async()=>{while(!session.closed&&session.savedVersion<session.version){const version=session.version;
      const r=await sdStorage.save(session.key,{epoch:session.epoch,revision:session.revision,pages:[...session.pages].map(([offset,data])=>({offset,data}))});
      if(session.closed||session!==sdSession)return;session.epoch=r.row.epoch;session.revision=r.row.revision;session.savedVersion=version;
      sdCard.backend=r.backend;sdCard.status=r.persistent?'saved':'memory-only';sdEvent();}
    })().catch(e=>{sdFailure(e,session);throw e;}).finally(()=>{session.saving=null;});session.saving.catch(()=>{});return session.saving;
  };
  const receiveSd=({watchId,pages})=>{const session=sdSession;if(!session||session.closed||session.watchId!==watchId)return;
    for(const {offset,data} of pages){requireValue(Number.isSafeInteger(offset)&&offset>=0&&offset%4096===0&&offset+4096<=session.baseline.length&&data instanceof ArrayBuffer&&data.byteLength===4096,'STORAGE_WORKER_ERROR','Invalid SD page bounds.');
      const bytes=new Uint8Array(data);if(bytes.some((b,i)=>b!==session.baseline[offset+i]))session.pages.set(offset,data);else session.pages.delete(offset);}
    session.version++;sdCard.changedPages=session.pages.size;sdCard.pristine=session.isDefault&&!session.pages.size;void persistSd(session);
  };
  const flushSd=async()=>{requireValue(sdSession&&!sdSession.closed,'SD_NOT_LOADED','Load a PaperMono firmware before using its SD card.');if(worker)await sdAsk('sd-flush');await persistSd(sdSession);return clone(sdCard);};
  const initializeSd=async bundle=>{
    alive();
    if(sdSession)sdSession.closed=true;sdSession=null;sdCard=emptySd();leds=[];
    if(bundle.cfg.board!=='papermono'){sdEvent();return;}
    requireValue(storage.scope,'SD_STORAGE_SCOPE','SD requires a loaded firmware identity.');
    const key=`v1:${product.id}:${bundle.cfg.board}:${storage.scope.firmwareSha256}`;
    // Subscribe to this identity before reading or hashing its base image. A clear
    // from another tab must invalidate even a read-only firmware's pending mount.
    const session={key,baseline:null,pages:new Map(),epoch:-1,revision:-1,version:0,savedVersion:0,saving:null,closed:false,isDefault:true};sdSession=session;
    const current=worker,signal=aborter.signal;
    const active=()=>{alive();requireValue(!signal.aborted,'FETCH_ABORTED','Firmware load was cancelled.');requireValue(session===sdSession&&!session.closed&&worker===current,'STORAGE_CONFLICT','The SD card changed while it was being loaded. Reload the firmware.');};
    sdCard={...emptySd(),supported:true,status:'loading'};sdEvent();
    const r=await sdStorage.read(key);active();requireValue(!!r.image===!!r.row.media,'STORAGE_CORRUPT','Saved SD base image is missing.');
    session.epoch=r.row.epoch;session.revision=r.row.revision;session.isDefault=!r.row.media;
    const baseline=r.image?validateSdImage(r.image):createFat32Image(),pages=session.pages,bytes=baseline.slice();session.baseline=baseline;
    if(r.image){const hash=await sha256(baseline);active();requireValue(r.row.media.bytes===baseline.length&&r.row.media.sha256===hash,'STORAGE_CORRUPT','Saved SD base image failed its checksum.');}
    for(const {offset,data} of r.row.pages){requireValue(Number.isSafeInteger(offset)&&offset>=0&&offset%4096===0&&offset+4096<=bytes.length&&data instanceof ArrayBuffer&&data.byteLength===4096&&!pages.has(offset),'STORAGE_CORRUPT','Saved SD image has invalid page bounds.');pages.set(offset,data);bytes.set(new Uint8Array(data),offset);}
    await sdAsk('sd-attach',{data:bytes.buffer},[bytes.buffer]);active();
    const watching=await sdAsk('sd-watch');active();session.watchId=watching.requestId;
    sdCard={supported:true,present:true,bytes:baseline.length,format:r.row.media?.format||'fat32',status:r.persistent?'clean':'memory-only',backend:r.backend,restored:pages.size>0||!!r.row.media,pristine:session.isDefault&&!pages.size,changedPages:pages.size,error:null};sdEvent();
  };
  const unsubscribeSd=sdStorage.subscribe(change=>{const session=sdSession;if(!session||session.closed||change.key!==session.key||change.epoch<session.epoch||change.epoch===session.epoch&&change.revision<=session.revision)return;sdFailure(error('STORAGE_CONFLICT','Another tab replaced or saved this SD card. Reload the firmware.'));});
  const replaceSd=async bytes=>{
    requireValue(loaded&&sdSession&&!sdSession.closed,'SD_NOT_LOADED','Load a PaperMono firmware before replacing its SD card.');
    const session=sdSession,bundle=loaded,signal=aborter.signal;
    const active=()=>{alive();requireValue(!signal.aborted,'FETCH_ABORTED','SD replacement was cancelled.');requireValue(session===sdSession&&!session.closed,'STORAGE_CONFLICT','The SD card changed while it was being replaced. Reload the firmware.');};
    if(worker&&status==='running'){await ask('stopped',{op:'stop'});active();setStatus('paused');}
    await preserveStorage();active();
    const image=bytes?.slice().buffer||null,hash=image?await sha256(new Uint8Array(image)):null;active();
    await sdStorage.replace(session.key,{image,media:image?{format:'image',sha256:hash,bytes:image.byteLength}:null});active();session.closed=true;
    loadCancelled=false;aborter=new AbortController();serial='';serialByPort={};stats={};frame=null;frameNumber=0;lastError=null;setStatus('loading');
    try{await initialize(bundle,loadReporter(++progressGeneration,aborter.signal));alive();setStatus('ready');return snapshot();}catch(e){stopWorker();if(!disposing&&status!=='disposed')setStatus('error');throw e;}
  };
  const pageHide=()=>{if(worker)void preserveStorage().catch(report);};
  globalThis.addEventListener?.('pagehide',pageHide);
  const visibilitySave=()=>{if(globalThis.document?.visibilityState==='hidden')pageHide();};
  globalThis.document?.addEventListener('visibilitychange',visibilitySave);
  const runtimeConfig = () => {
    const runtime = product?.runtime || {};
    requireValue(runtime.board, 'UNSUPPORTED_PRODUCT', 'This product has catalog metadata but no executable board model yet.');
    // Official StopWatch V0.5 enters a non-responsive JIT path in its IMU page.
    // The same unmodified firmware passes through the WASM CPU interpreter.
    return { board: runtime.board, flash_mb: runtime.flashMb ?? 8, psram_mb: runtime.psramMb ?? 2, ...(runtime.board === 'stopwatch' ? {jit: false} : {}) };
  };
  const readArtifact = async (entry, root, progress, kind, maxBytes = MAX_FILE) => {
    const item = typeof entry === 'string' ? { url: entry } : entry;
    requireValue(item && typeof item.url === 'string', 'INVALID_MANIFEST', 'Each firmware file needs a URL.');
    const url = resolveFirmwareUrl(item.url, root);
    let cached = await firmwareCache.get(url, item.sha256, maxBytes);
    requireValue(!aborter.signal.aborted, 'FETCH_ABORTED', 'Firmware load was cancelled.');
    if (cached) {
      try { await checkedHash(cached.data, item.sha256); }
      catch { await firmwareCache.remove(url, item.sha256); cached = null; }
    }
    const data = cached?.data || await fetchBytes(url, { fetchImpl, maxBytes, signal: aborter.signal, onProgress: progress.file(kind, url) });
    if (cached) progress.cache(kind, url, data.byteLength, cached.storage);
    progress.prepare('verify', kind, url);
    requireValue(data.length > 0, 'EMPTY_FILE', 'Firmware file is empty.', { url });
    const digest = await checkedHash(data, item.sha256);
    requireValue(!aborter.signal.aborted, 'FETCH_ABORTED', 'Firmware load was cancelled.');
    if (!cached && item.sha256) await firmwareCache.putVerified(url, digest, data);
    return { data, url, sha256: digest };
  };
  const prepare = async (source, progress) => {
    requireValue(source && typeof source === 'object', 'FIRMWARE_REQUIRED', 'Select a firmware manifest or merged flash image first.');
    const cfg = runtimeConfig(), files = [], receipts = [];
    let total = 0;
    const add = (kind, artifact, at) => {
      total += artifact.data.length;
      requireValue(total <= MAX_TOTAL, 'BUNDLE_TOO_LARGE', 'Firmware bundle exceeds 256 MiB.');
      files.push({ kind, data: artifact.data, ...(at === undefined ? {} : { at }) });
      receipts.push({ kind, url: artifact.url, sha256: artifact.sha256, bytes: artifact.data.length, ...(at === undefined ? {} : { at }) });
    };
    let manifest = null, manifestUrl = null, manifestHash = null, localLayout = null;
    if (source.manifestUrl) {
      manifestUrl = resolveFirmwareUrl(source.manifestUrl, baseUrl);
      const artifact = await readArtifact({ url: manifestUrl, sha256: source.sha256 }, baseUrl, progress, 'manifest');
      manifestHash = artifact.sha256;
      try { manifest = JSON.parse(new TextDecoder().decode(artifact.data)); } catch { throw error('INVALID_MANIFEST', 'Firmware manifest is not valid JSON.'); }
      requireValue(manifest && !Array.isArray(manifest) && typeof manifest === 'object', 'INVALID_MANIFEST', 'Firmware manifest must be an object.');
      requireValue(!manifest.nodes, 'UNSUPPORTED_MANIFEST', 'Multi-node network manifests are not supported by this product session.');
      requireValue(!manifest.board || manifest.board === cfg.board, 'BOARD_MISMATCH', 'Manifest board does not match the selected product.', { selected: cfg.board, manifest: manifest.board });
      for (const [key, expected] of [['flash_mb', cfg.flash_mb], ['psram_mb', cfg.psram_mb]]) if (manifest[key] != null) requireValue(manifest[key] === expected, 'MEMORY_MISMATCH', `Manifest ${key} does not match this product.`, { expected, actual: manifest[key] });
      requireValue(manifest.files && typeof manifest.files === 'object' && !Array.isArray(manifest.files), 'INVALID_MANIFEST', 'Manifest must list its firmware files.');
      const pinned = entry => typeof entry === 'string' ? { url: entry, sha256: manifest.artifact_sha256?.[entry] } : entry;
      for (const [kind, entries] of Object.entries(manifest.files)) {
        requireValue(Object.hasOwn(KINDS, kind), 'INVALID_MANIFEST', `Unsupported firmware file kind: ${kind}.`);
        for (const entry of [].concat(entries)) {
          requireValue(files.length < 32, 'INVALID_MANIFEST', 'A manifest may contain at most 32 files.');
          add(kind, await readArtifact(pinned(entry), manifestUrl, progress, kind));
        }
      }
      for (const [offset, entry] of Object.entries(manifest.flash_at || {})) {
        requireValue(/^0x[0-9a-f]+$/i.test(offset), 'INVALID_MANIFEST', 'flash_at offsets must be hexadecimal strings beginning with 0x.');
        const at = Number(offset), artifact = await readArtifact(pinned(entry), manifestUrl, progress, 'flash_at');
        requireValue(Number.isSafeInteger(at) && at >= 0 && at + artifact.data.length <= cfg.flash_mb * MIB, 'FLASH_OVERFLOW', 'Flash segment is outside product flash.');
        requireValue(files.length < 32, 'INVALID_MANIFEST', 'A manifest may contain at most 32 files.');
        add('flash', artifact, at);
      }
      requireValue(files.some(f => f.kind === 'app' || f.kind === 'flash'), 'INCOMPLETE_IMAGE', 'Manifest contains no app or flash image.');
      cfg.appDirect = manifest.app_direct === true;
      if (!cfg.appDirect && !files.some(f => f.kind === 'flash')) requireValue(files.some(f => f.kind === 'bootloader') && files.some(f => f.kind === 'ptable'), 'INCOMPLETE_IMAGE', 'ROM boot needs both a bootloader and a partition table.');
      cfg.stubs = manifest.stubs || [];
      requireValue(Array.isArray(cfg.stubs) && cfg.stubs.length <= 64 && cfg.stubs.every(s => typeof s === 'string' && s.length < 256), 'INVALID_MANIFEST', 'Invalid manifest stub list.');
      cfg.stubs = cfg.stubs.map(spec => { const [name] = spec.split('='); return String(manifest.symbols?.[name] || name) + spec.slice(name.length); });
      cfg.wifi = manifest.wifi || '';
      requireValue(typeof cfg.wifi === 'string' && cfg.wifi.length <= 1024, 'INVALID_MANIFEST', 'Invalid WiFi emulation configuration.');
    } else if (['build','segments'].includes(source.format)) {
      const local = await assembleLocalFirmware(source, {board:cfg.board, flashBytes:cfg.flash_mb * MIB, signal:aborter.signal,
        onRead:event => progress.prepare(event.stage, event.name)});
      progress.prepare('verify', 'local-layout');
      validateMergedImage(local.data, cfg.board, cfg.flash_mb * MIB, {partitionTableOffset:local.partitionTableOffset});
      const segments = [];
      for (const entry of local.segments) segments.push({name:entry.path.split('/').at(-1), offset:entry.offset, bytes:entry.data.length, sha256:await checkedHash(entry.data)});
      requireValue(!aborter.signal.aborted, 'FETCH_ABORTED', 'Local firmware load was cancelled.');
      localLayout = {format:source.format, partitionTableOffset:local.partitionTableOffset, segments};
      add('flash', {data:local.data, url:null, sha256:await checkedHash(local.data, source.sha256)});
    } else {
      requireValue(!source.format || source.format === 'merged', 'UNSUPPORTED_FORMAT', 'Raw firmware must be a merged flash image.');
      let artifact;
      if (source.file) {
        requireValue(typeof source.file.arrayBuffer === 'function' && source.file.size > 0 && source.file.size <= MAX_FILE, 'INVALID_FILE', 'Choose a nonempty local file no larger than 256 MiB.');
        progress.prepare('local', 'flash');
        const data = new Uint8Array(await source.file.arrayBuffer());
        progress.prepare('verify', 'flash');
        artifact = { data, url: null, sha256: await checkedHash(data, source.sha256) };
      } else artifact = await readArtifact(source, baseUrl, progress, 'flash');
      validateMergedImage(artifact.data, cfg.board, cfg.flash_mb * MIB);
      add('flash', artifact);
    }
    if (!cfg.appDirect && !files.some(f => f.kind === 'rom')) add('rom', await readArtifact(new URL(`wasm/fw/${romFor(cfg.board)}`, assetBaseUrl).href, baseUrl, progress, 'rom'));
    return { cfg, files, receipt: { manifestUrl, manifestHash, source: source.file || localLayout ? 'local' : 'remote', ...(localLayout ? {localLayout} : {}), artifacts: receipts, appDirect: !!cfg.appDirect, stubs: cfg.stubs || [], executionMode: cfg.jit === false ? 'wasm-interpreter' : 'wasm-jit' } };
  };
  const initialize = async (bundle, progress) => {
    stopWorker();
    const wasmUrl = new URL('wasm/esp32sim.wasm', assetBaseUrl).href;
    const wasm = await fetchBytes(wasmUrl, { fetchImpl, maxBytes: MAX_FILE, signal: aborter.signal, onProgress: progress.file('wasm', wasmUrl) });
    progress.prepare('worker');
    alive();
    requireValue(!aborter.signal.aborted, 'FETCH_ABORTED', 'Firmware load was cancelled.');
    const current = workerFactory(new URL('wasm/worker.js', assetBaseUrl)); worker = current;
    current.onmessage = event => { if (worker === current) receive(event.data); };
    current.onerror = event => { const e = error('WORKER_ERROR', event.message || 'WebAssembly worker failed.'); rejectPending(e); report(e); setStatus('error'); };
    const wasmBuffer = wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength);
    await ask('ready', { op: 'init', wasm: wasmBuffer, frameAck: true }, [wasmBuffer]);
    const result = await ask('created', { op: 'create', ...bundle.cfg });
    requireValue(result.created, 'CREATE_FAILED', 'Cannot create the selected emulator board.');
    for (const file of bundle.files) {
      const data = file.data.slice().buffer;
      const result = await ask(`loaded:${file.at === undefined ? KINDS[file.kind] : `at${file.at}`}`, { op: 'load', ...(file.at === undefined ? { kind: KINDS[file.kind] } : { at: file.at }), data }, [data]);
      requireValue(result.ok, 'LOAD_FAILED', `Emulator rejected ${file.kind}.`);
    }
    await initializeStorage(bundle, result.flashStorage, progress);
    await initializeSd(bundle);
    for (const spec of bundle.cfg.stubs || []) worker.postMessage({ op: 'stub', spec });
    if (bundle.cfg.wifi) worker.postMessage({ op: 'wifi', spec: bundle.cfg.wifi });
    if (imuModel()) await applyImu();
  };
  const applyImu = async () => {
    if (!worker) return;
    const inputId = ++inputSequence;
    const receipt = await ask(`imu:${inputId}`, {op: 'imu', inputId, accelG: imu.accelG, gyroDps: imu.gyroDps});
    requireValue(receipt.ok && Number.isFinite(receipt.time), 'IMU_UNSUPPORTED', 'The loaded emulator does not support this IMU input.');
    imu.delivery = 'register-state'; imu.guestTime = receipt.time;
  };
  const start = async () => {
    requireValue(loaded && ['ready', 'paused', 'running'].includes(status), 'NOT_READY', 'Load firmware before starting the emulator.');
    if (status === 'running') return snapshot();
    const resume = booted, result = await ask(resume ? 'resumed' : 'started', { op: resume ? 'resume' : 'start', appDirect: !!loaded.cfg.appDirect });
    requireValue(resume ? result.resumed : result.started, 'BOOT_FAILED', result.error || 'The emulator could not boot this firmware.');
    booted = true; setStatus('running'); return snapshot();
  };
  const controls = () => Array.isArray(product?.controls) ? product.controls : product?.controls?.buttons || [];
  const input = async event => {
    const releaseWhilePaused = status === 'paused' && ((['button','matrix-key'].includes(event?.type) && event.pressed === false) || (event?.type === 'touch' && event.down === false));
    requireValue(status === 'running' || releaseWhilePaused, 'NOT_RUNNING', 'Start the emulator before sending input.');
    requireValue(event && typeof event === 'object', 'INVALID_INPUT', 'Input must be an object.');
    let message;
    if (event.type === 'button') {
      const control = controls().find(c => c.id === event.id);
      const pin = control?.pin ?? control?.gpio;
      requireValue(Number.isInteger(pin) && pin >= 0 && pin < 64, 'UNSUPPORTED_CONTROL', 'This product does not expose that GPIO button.');
      requireValue(typeof event.pressed === 'boolean', 'INVALID_INPUT', 'Button pressed must be a boolean.');
      message = { t: 'btn', pin, v: event.pressed ? 1 : 0 };
    } else if (event.type === 'matrix-key') {
      requireValue(['cardputer','cardputer-adv'].includes(product.runtime?.board), 'UNSUPPORTED_CONTROL', 'This product does not expose a matrix keyboard.');
      requireValue(Number.isInteger(event.row) && event.row >= 0 && event.row < 4 && Number.isInteger(event.column) && event.column >= 0 && event.column < 14 && typeof event.pressed === 'boolean', 'INVALID_INPUT', 'Keyboard row/column must be within the physical 4×14 layout and pressed must be a boolean.');
      message = { t:'matrix-key', row:event.row, column:event.column, pressed:event.pressed?1:0 };
    } else if (event.type === 'touch') {
      const display = product.display || product.hardware?.display;
      requireValue(product.runtime?.touch === true || display?.touch === true || display?.touch?.supported === true, 'UNSUPPORTED_CONTROL', 'Touch input is not supported for this product.');
      const width = frame?.width || display?.framebufferWidth || display?.width, height = frame?.height || display?.framebufferHeight || display?.height;
      requireValue(Number.isInteger(event.x) && Number.isInteger(event.y) && event.x >= 0 && event.y >= 0 && event.x < width && event.y < height && typeof event.down === 'boolean', 'INVALID_INPUT', 'Touch coordinates must be integer native display pixels within the panel.');
      message = { t: 'touch', x: event.x, y: event.y, down: event.down ? 1 : 0 };
    } else if (event.type === 'serial') {
      requireValue(['usb', 'uart0', 'uart1'].includes(event.port), 'UNSUPPORTED_PORT', 'Supported input ports are usb, uart0 and uart1; UART2 input is not exposed by upstream.');
      const maxBytes = event.port === 'usb' ? 4096 : 128;
      requireValue(typeof event.data === 'string' && new TextEncoder().encode(event.data).length <= maxBytes, 'INVALID_INPUT', `Serial input must be a string of at most ${maxBytes} UTF-8 bytes; UART RX has a 128-byte FIFO.`);
      message = { t: 'key', src: event.port, data: event.data };
    } else throw error('UNSUPPORTED_INPUT', 'Unsupported input type.');
    const inputId = ++inputSequence;
    const receipt = await ask(`input:${inputId}`, { op: 'text', inputId, data: JSON.stringify(message) });
    requireValue(Number.isFinite(receipt.time) && receipt.time >= 0, 'INVALID_INPUT_RECEIPT', 'Worker did not return a valid guest input clock.');
    emit({ type: 'input', event: clone(event), receipt: { delivery: 'queued', guestTime: receipt.time, frame: { width: frame?.width, height: frame?.height } } });
    return { accepted: true, delivery: 'queued', message, guestTime: receipt.time };
  };
  const advance = fromTime => new Promise((resolveAdvance, reject) => {
    // Let the guest drain its 128-byte RX FIFO before the next NMEA chunk. This is
    // deliberately separate from electrical baud timing: upstream injects bytes directly.
    // Use the matching worker receipt, never a possibly stale main-thread stat.
    const targetTime = fromTime + 0.04;
    const finish = e => { clearTimeout(timer); listeners.delete(listener); e ? reject(e) : resolveAdvance(); };
    const listener = event => {
      if (event.type === 'stats' && Number(event.stats.time) >= targetTime) finish();
      if (event.type === 'state' && event.state.status !== 'running') finish(error('NOT_RUNNING', 'GPS injection was interrupted.'));
    };
    const timer = setTimeout(() => finish(error('INPUT_TIMEOUT', 'The emulator did not advance while sending GPS data.')), 10000);
    listeners.add(listener);
    if (Number(stats.time) >= targetTime) finish();
  });
  const api = {
    version: API_VERSION,
    attachSdCard({bytes,format='fat32'}={}) {return exclusive(async()=>{requireValue(bytes!==undefined||format==='fat32','INVALID_SD_IMAGE','Only FAT32 formatting is available.');return replaceSd(bytes===undefined?null:validateSdImage(bytes));});},
    clearSdCard() {return exclusive(()=>replaceSd(null));},
    exportSdCard() {return exclusive(async()=>{await flushSd();const result=await sdAsk('sd-export');return {bytes:new Uint8Array(result.data),filename:`${product.id}-sd.img`};});},
    flushSdCard() {return exclusive(flushSd);},
    async clearFirmwareCache() { return firmwareCache.clear(); },
    flushDeviceStorage() { return exclusive(flushStorage); },
    clearDeviceStorage() { const loadId=++progressGeneration; return exclusive(async () => {
      requireValue(loaded && storageSession, 'STORAGE_NOT_LOADED', 'Load firmware before clearing its device storage.');
      const bundle=loaded, session=storageSession, scope=session.scope;
      // Freeze first; pending writes finish before the tombstone transaction. A new
      // epoch invalidates every other tab, including tabs suspended during this clear.
      if(sdSession&&!sdSession.closed)await flushSd();
      stopWorker(); session.closed=true;
      await session.saving?.catch(()=>{});
      try { await deviceStorage.clear(scope); }
      catch(e) { session.closed=false; storageFailure(session,e); setStatus('error'); throw e; }
      loadCancelled=false; aborter=new AbortController(); serial=''; serialByPort={}; stats={}; frame=null; frameNumber=0; lastError=null; setStatus('loading');
      try { await initialize(bundle,loadReporter(loadId,aborter.signal)); setStatus('ready'); return snapshot(); }
      catch(e) { stopWorker(); if(status!=='disposed')setStatus('error'); throw e; }
    }); },
    subscribe(listener) { requireValue(typeof listener === 'function', 'INVALID_LISTENER', 'Event listener must be a function.'); listeners.add(listener); return () => listeners.delete(listener); },
    async observe() { return snapshot(); },
    async capabilities() {
      return { version: API_VERSION, backend: 'esp32sim-wasm', static: true, executable: !!product?.runtime?.board,
        execution: {mode: product?.runtime?.board === 'stopwatch' ? 'wasm-interpreter' : 'wasm-jit', reason: product?.runtime?.board === 'stopwatch' ? 'Official V0.5 IMU JIT non-response; interpreter verified.' : null},
        sdCard:{available:product?.runtime?.board==='papermono',defaultBytes:64*MIB,maxBytes:MAX_SD_BYTES,formats:['fat32','raw-image'],scope:'product-board-canonical-flash-sha256',autoSaveIntervalMs:1500,crashSafe:false},
        lifecycle: ['configure', 'loadFirmware', 'start', 'pause', 'reset', 'dispose'],
        deviceStorage: {available:!!storage.scope, backend:storage.backend, persistent:storage.backend==='indexeddb', scope:'product-board-chip-canonical-flash-sha256', autoSaveIntervalMs:500, crashSafe:false},
        firmware: { manifest: true, mergedImage: true, localFile: true, localSegments: true, localBuild: true, sha256: true, maxFileBytes: MAX_FILE, maxBundleBytes: MAX_TOTAL, corsRequired: true },
        input: { buttons: product?.runtime?.board ? controls().filter(c => Number.isInteger(c.pin ?? c.gpio)).map(c => c.id) : [], touch: !!(product?.runtime?.board && (product?.runtime?.touch || product?.display?.touch)), keyboard:['cardputer','cardputer-adv'].includes(product?.runtime?.board)?{rows:4,columns:14,model:product.runtime.board==='cardputer'?'gpio-matrix-74hc138':'tca8418',layout:product.runtime.board}:null, serialPorts: ['usb', 'uart0', 'uart1'] },
        framebuffer: { available: !!frame, formats: ['rgba', 'png'], source: 'emulator' },
        gps: { mode: 'logical-uart1-nmea', physicalRouting: false, baudTiming: false, hardwareValidated: false },
        imu: { available: !!imuModel(), model: imuModel(), sceneOrientation: product?.runtime?.board === 'stopwatch', orientation: 'quaternion-xyzw', accelerationUnit: 'g', angularVelocityUnit: 'deg/s', maxSceneHz: 30, hardwareValidated: false, sampleTiming: 'host-input', physicalMountingVerified: false },
        unsupported: ['dynamic-peripheral-attachment', 'uart2-input', 'physical-gps-radio', 'arbitrary-firmware-compatibility', 'cycle-exact-host-input'] };
    },
    configure(nextScene) { progressGeneration++; imuGeneration++; return exclusive(async () => {
      catalog ||= await loadCatalog();
      const validation = validateScene(nextScene, catalog);
      requireValue(validation.valid, 'INVALID_SCENE', 'Scene configuration is invalid.', validation);
      if (worker && status==='running') await ask('stopped',{op:'stop'});
      await preserveStorage(); alive(); aborter?.abort(); stopWorker(); detachStorage(); loaded = null; firmware = null; frame = null; frameNumber = 0; serial = ''; serialByPort = {}; stats = {}; lastError = null;
      if(sdSession)sdSession.closed=true;sdSession=null;sdCard=emptySd();leds=[];
      scene = clone(nextScene); product = catalog.products.find(item => item.id === scene.productId);sdCard.supported=product.runtime?.board==='papermono';sdEvent();emit({type:'leds',leds:[]});
      imu = defaultImu(); imuPreviousAt = null;
      setStatus('configured'); return { ...snapshot(), warnings: validation.warnings };
    }); },
    updateSceneMetadata(changes) { return exclusive(async () => {
      requireValue(scene, 'NOT_CONFIGURED', 'Configure a scene before updating its presentation.');
      requireValue(changes && typeof changes === 'object' && Object.keys(changes).every(key => ['keyMapping', 'view'].includes(key)), 'INVALID_SCENE', 'Only keyMapping and view can be changed without reconfiguring the emulator.');
      const next = { ...scene, ...clone(changes) }, validation = validateScene(next, catalog);
      requireValue(validation.valid, 'INVALID_SCENE', 'Scene configuration is invalid.', validation);
      scene = next; emit({ type: 'state', state: snapshot() }); return snapshot();
    }); },
    loadFirmware(source) { const loadId = ++progressGeneration; return exclusive(async () => {
      requireValue(scene, 'NOT_CONFIGURED', 'Configure a product before loading firmware.');
      if (worker && status==='running') await ask('stopped',{op:'stop'});
      await preserveStorage(); alive(); stopWorker(); detachStorage();if(sdSession)sdSession.closed=true;sdSession=null;sdCard={...emptySd(),supported:product?.runtime?.board==='papermono'};leds=[]; loaded = null; firmware = null; frame = null; frameNumber = 0; serial = ''; serialByPort = {}; stats = {}; lastError = null;
      loadCancelled = false; aborter = new AbortController(); setStatus('loading');
      const progress = loadReporter(loadId, aborter.signal);
      try {
        const selected = source || scene.firmware, bundle = await prepare(selected, progress);
        await initialize(bundle, progress); loaded = bundle; firmware = bundle.receipt;
        if (bundle.receipt.source === 'remote') scene.firmware = bundle.receipt.manifestUrl
          ? { manifestUrl: bundle.receipt.manifestUrl, sha256: bundle.receipt.manifestHash }
          : { url: bundle.receipt.artifacts[0].url, sha256: bundle.receipt.artifacts[0].sha256 };
        setStatus('ready'); return snapshot();
      }
      catch (e) { stopWorker(); if (status !== 'disposed') setStatus(loadCancelled ? 'configured' : 'error'); if (loadCancelled) throw error('FETCH_ABORTED', 'Firmware load was cancelled.'); throw e; }
    }); },
    async cancelPendingLoad() {
      if (status !== 'loading') return snapshot();
      progressGeneration++;
      loadCancelled = true; aborter?.abort(); stopWorker(); detachStorage(); loaded = null; firmware = null;
      await queue;
      if (status !== 'disposed') setStatus('configured');
      return snapshot();
    },
    start() { return exclusive(start); },
    pause() { return exclusive(async () => {
      requireValue(['running', 'paused'].includes(status), 'NOT_RUNNING', 'Only a running emulator can be paused.');
      if (imuModel()) { imu.gyroDps = [0,0,0]; imuPreviousAt = null; await applyImu(); }
      if (status === 'running') await ask('stopped', { op: 'stop' });
      await preserveStorage(); setStatus('paused'); return snapshot();
    }); },
    reset() { const loadId = ++progressGeneration; return exclusive(async () => {
      requireValue(loaded, 'NOT_READY', 'Load firmware before resetting.');
      if (worker && status==='running') await ask('stopped',{op:'stop'});
      await preserveStorage();
      loadCancelled = false; aborter = new AbortController(); serial = ''; serialByPort = {}; stats = {}; frame = null; frameNumber = 0; lastError = null; setStatus('loading');
      try { await initialize(loaded, loadReporter(loadId, aborter.signal)); setStatus('ready'); return await start(); }
      catch (e) { stopWorker(); if (status !== 'disposed') setStatus(loadCancelled ? 'configured' : 'error'); if (loadCancelled) throw error('FETCH_ABORTED', 'Firmware load was cancelled.'); throw e; }
    }); },
    input(event) { return exclusive(() => input(event)); },
    setImu(options = {}) { const generation = imuGeneration, requestedAt = performance.now(); return exclusive(async () => {
      requireValue(imuModel(), 'IMU_UNSUPPORTED', 'This product does not have a host-fed IMU model.');
      requireValue(generation === imuGeneration, 'SESSION_REPLACED', 'The device changed before its orientation was applied.');
      requireValue(options && typeof options === 'object' && !Array.isArray(options) && Object.keys(options).every(key => ['enabled','quaternion','source','angularVelocityDps'].includes(key)), 'INVALID_IMU', 'IMU options contain unknown fields.');
      requireValue(options.enabled === undefined || typeof options.enabled === 'boolean', 'INVALID_IMU', 'IMU enabled must be a boolean.');
      const source = options.source || 'agent';
      requireValue(['agent', 'scene'].includes(source), 'INVALID_IMU', 'IMU source must be agent or scene.');
      requireValue(source !== 'scene' || product.runtime.board === 'stopwatch', 'IMU_SCENE_UNSUPPORTED', 'This product has no scene-to-sensor orientation mapping; use agent input with explicit virtual axes.');
      if (source === 'scene' && !imu.enabled && options.enabled !== true) return clone(imu);
      const now = requestedAt;
      const sample = orientationSample(options.quaternion || imu.quaternion, {previous: imu.quaternion,
        elapsedSeconds: imuPreviousAt === null ? 0 : (now - imuPreviousAt) / 1000,
        angularVelocityDps: options.enabled === false || status !== 'running' || source === 'agent' && options.angularVelocityDps === undefined ? [0,0,0] : options.angularVelocityDps});
      imu = {...imu, ...sample, enabled: options.enabled ?? imu.enabled, source}; imuPreviousAt = now;
      await applyImu(); emit({type: 'imu', imu: clone(imu)}); return clone(imu);
    }); },
    setGpsFix(options) { return exclusive(async () => {
      requireValue(options && typeof options === 'object' && Object.keys(options).every(key => ['accessoryId','latitude','longitude','altitude','speedKnots','course','satellites','positionSource'].includes(key)), 'INVALID_INPUT', 'GPS fix contains unknown fields.');
      const attached = scene?.accessories.find(item => item.id === options?.accessoryId);
      const definition = scene?.localAccessories?.find(item => item.id === attached?.catalogId) || catalog?.accessories.find(item => item.id === attached?.catalogId);
      requireValue(attached && definition?.model?.id === 'uart-nmea', 'ACCESSORY_NOT_ATTACHED', 'Attach a GPS accessory before setting its fix.');
      const parameters = { ...definition.parameters, ...attached.parameters };
      for (const key of ['latitude', 'longitude', 'altitude', 'speedKnots', 'course', 'satellites']) if (options[key] !== undefined) parameters[key] = options[key];
      createGpsNmea(parameters);
      requireValue(parameters.altitude === undefined || parameters.altitude >= -10000 && parameters.altitude <= 100000, 'INVALID_GPS_FIX', 'Altitude must be -10000…100000 meters.');
      requireValue(parameters.speedKnots === undefined || parameters.speedKnots <= 10000, 'INVALID_GPS_FIX', 'Speed must be 0…10000 knots.');
      if (options.positionSource !== undefined) {
        const source = options.positionSource;
        requireValue(source && ['manual', 'ip-estimate'].includes(source.kind), 'INVALID_INPUT', 'Position source must be manual or ip-estimate.');
        requireValue(Object.keys(source).every(key => ['kind','provider','fetchedAt','city','region','country'].includes(key)), 'INVALID_INPUT', 'Position source contains unknown fields; IP addresses are not stored.');
        parameters.positionSource = { kind: source.kind };
        for (const key of ['provider', 'fetchedAt', 'city', 'region', 'country']) if (source[key] !== undefined) {
          requireValue(typeof source[key] === 'string' && source[key].length <= 160, 'INVALID_INPUT', `Position source ${key} must be a short string.`);
          parameters.positionSource[key] = source[key];
        }
        requireValue(source.fetchedAt === undefined || /^\d{4}-\d{2}-\d{2}T.*Z$/.test(source.fetchedAt) && Number.isFinite(Date.parse(source.fetchedAt)), 'INVALID_INPUT', 'Position source fetchedAt must be an ISO UTC date-time ending in Z.');
      }
      const next = clone(scene); next.accessories.find(item => item.id === attached.id).parameters = parameters;
      const validation = validateScene(next, catalog);
      requireValue(validation.valid, 'INVALID_GPS_FIX', 'GPS fix would create an invalid scene.', validation); scene = next;
      emit({ type: 'state', state: snapshot() });
      return { accessoryId: attached.id, parameters: clone(parameters) };
    }); },
    injectGps(options) { return exclusive(async () => {
      const attached = scene?.accessories.find(item => item.id === options?.accessoryId);
      const definition = scene?.localAccessories?.find(item => item.id === attached?.catalogId) || catalog?.accessories.find(item => item.id === attached?.catalogId);
      requireValue(attached && definition?.model?.id === 'uart-nmea', 'ACCESSORY_NOT_ATTACHED', 'Attach a GPS accessory using the UART NMEA model before injecting a fix.');
      requireValue(product?.runtime?.board === 'stopwatch', 'UNSUPPORTED_WIRING', 'This release provides the documented GPS test path only for StopWatch.');
      requireValue(attached.wiring?.mode === 'logical' && attached.wiring?.port === 'uart1', 'UNSUPPORTED_WIRING', 'GPS injection requires explicit logical UART1 wiring; physical routing is not simulated.');
      const sentences = createGpsNmea({ ...definition.parameters, ...attached.parameters, ...options });
      for (let offset = 0; offset < sentences.length; offset += 64) {
        const receipt = await input({ type: 'serial', port: 'uart1', data: sentences.slice(offset, offset + 64) });
        if (offset + 64 < sentences.length) await advance(receipt.guestTime);
      }
      return { accepted: true, port: 'uart1', data: sentences, delivery: 'logical-uart-rx', limitations: ['GPIO routing and baud rate are bypassed.', 'Firmware must read UART1; output has not been compared with physical GPS hardware.'] };
    }); },
    async capture({ format = 'png' } = {}) {
      alive(); requireValue(frame, 'NO_FRAME', 'No real framebuffer has been received from the firmware.');
      const captured = frame, number = frameNumber;
      if (format === 'rgba') return { format, ...captured, pixels: captured.pixels.slice(), frameNumber: number };
      requireValue(format === 'png', 'UNSUPPORTED_FORMAT', 'Capture format must be png or rgba.');
      requireValue(globalThis.document, 'CANVAS_UNAVAILABLE', 'PNG capture requires a browser canvas.');
      const canvas = document.createElement('canvas'); canvas.width = captured.width; canvas.height = captured.height;
      canvas.getContext('2d').putImageData(new ImageData(captured.pixels, captured.width, captured.height), 0, 0);
      return { format, width: captured.width, height: captured.height, frameNumber: number, dataUrl: canvas.toDataURL('image/png') };
    },
    async dispose() {
      if (status !== 'disposed') {
        disposing=true; progressGeneration++; imuGeneration++; aborter?.abort();
        try { if (storageSession && status!=='loading') { if (worker && status==='running') await ask('stopped',{op:'stop'}); await preserveStorage(); } }
        catch(e) { report(e); }
        finally { stopWorker(); if(storageSession)storageSession.closed=true; loaded=null; unsubscribeStorage(); deviceStorage.close();if(sdSession)sdSession.closed=true;unsubscribeSd();sdStorage.close(); globalThis.removeEventListener?.('pagehide',pageHide);globalThis.document?.removeEventListener('visibilitychange',visibilitySave); setStatus('disposed'); listeners.clear(); }
      }
      return snapshot();
    },
  };
  return api;
}

export function createGpsNmea({ latitude, longitude, altitude = 0, speedKnots = 0, course = 0, satellites = 8, timestamp = new Date().toISOString() } = {}) {
  requireValue(Number.isFinite(latitude) && Math.abs(latitude) <= 90 && Number.isFinite(longitude) && Math.abs(longitude) <= 180, 'INVALID_GPS_FIX', 'Latitude must be -90…90 and longitude -180…180.');
  requireValue([altitude, speedKnots, course].every(Number.isFinite) && speedKnots >= 0 && course >= 0 && course < 360 && Number.isInteger(satellites) && satellites >= 1 && satellites <= 99, 'INVALID_GPS_FIX', 'Invalid GPS altitude, speed, course or satellite count.');
  const date = new Date(timestamp); requireValue(Number.isFinite(date.getTime()), 'INVALID_GPS_FIX', 'Invalid GPS timestamp.');
  const pad = n => String(n).padStart(2, '0'), time = `${pad(date.getUTCHours())}${pad(date.getUTCMinutes())}${pad(date.getUTCSeconds())}.00`;
  const day = `${pad(date.getUTCDate())}${pad(date.getUTCMonth() + 1)}${pad(date.getUTCFullYear() % 100)}`;
  const coordinate = (value, digits) => { const units = Math.round(Math.abs(value) * 60 * 1e5), degrees = Math.floor(units / 6e6), minutes = units % 6e6 / 1e5; return `${String(degrees).padStart(digits, '0')}${minutes.toFixed(5).padStart(8, '0')}`; };
  const lat = `${coordinate(latitude, 2)},${latitude < 0 ? 'S' : 'N'}`, lon = `${coordinate(longitude, 3)},${longitude < 0 ? 'W' : 'E'}`;
  const sentence = value => { let checksum = 0; for (const char of value) checksum ^= char.charCodeAt(0); return `$${value}*${checksum.toString(16).toUpperCase().padStart(2, '0')}\r\n`; };
  return sentence(`GPRMC,${time},A,${lat},${lon},${speedKnots.toFixed(2)},${course.toFixed(2)},${day},,,A`) + sentence(`GPGGA,${time},${lat},${lon},1,${pad(satellites)},1.0,${altitude.toFixed(1)},M,0.0,M,,`);
}
