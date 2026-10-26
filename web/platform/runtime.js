// Browser-only adapter for the upstream ESP32 WASM worker. No server or mock device state.
import { loadCatalog, validateScene } from './catalog.js';

export const API_VERSION = '1.0.0';
const MIB = 1024 * 1024;
const KINDS = { rom: 0, bootloader: 1, ptable: 2, app: 3, elf: 4, flash: 5, script: 6, picture: 7 };
const MAX_FILE = 32 * MIB, MAX_TOTAL = 64 * MIB, MAX_MANIFEST = 256 * 1024;
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

export async function fetchBytes(url, { fetchImpl = globalThis.fetch, maxBytes = MAX_FILE, signal } = {}) {
  const controller = new AbortController();
  const abort = () => controller.abort();
  signal?.addEventListener('abort', abort, { once: true });
  if (signal?.aborted) controller.abort();
  const timeout = setTimeout(abort, 60000);
  try {
    const response = await fetchImpl(url, { mode: 'cors', credentials: 'omit', signal: controller.signal });
    requireValue(response.ok, 'HTTP_ERROR', `Firmware request failed: HTTP ${response.status}.`, { url, status: response.status });
    const length = Number(response.headers.get('content-length'));
    requireValue(!length || length <= maxBytes, 'FILE_TOO_LARGE', `File exceeds ${maxBytes / MIB} MiB limit.`, { url });
    if (!response.body?.getReader) {
      const bytes = new Uint8Array(await response.arrayBuffer());
      requireValue(bytes.byteLength <= maxBytes, 'FILE_TOO_LARGE', 'Downloaded file exceeds its size limit.', { url });
      return bytes;
    }
    const reader = response.body.getReader(), chunks = [];
    let total = 0;
    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        total += value.byteLength;
        requireValue(total <= maxBytes, 'FILE_TOO_LARGE', 'Downloaded file exceeds its size limit.', { url });
        chunks.push(value);
      }
    } catch (e) { await reader.cancel().catch(() => {}); throw e; }
    const bytes = new Uint8Array(total);
    let offset = 0;
    for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
    return bytes;
  } catch (e) {
    if (e.code) throw e;
    if (e.name === 'AbortError') throw error('FETCH_ABORTED', 'Firmware download was cancelled or timed out.', { url });
    throw error('FETCH_FAILED', 'Firmware download failed. Check the URL, network and Access-Control-Allow-Origin header; a static site cannot bypass CORS.', { url });
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
export function validateMergedImage(data, board, flashBytes) {
  const bytes = bytesOf(data), bootOffset = /^(esp32|ttgo-tdisplay)$/.test(board) ? 0x1000 : 0;
  requireValue(bytes.length <= flashBytes, 'FLASH_OVERFLOW', 'Merged firmware exceeds the selected product flash size.');
  requireValue(bytes.length > 0x8020 && bytes[bootOffset] === 0xe9 && bytes[0x8000] === 0xaa && bytes[0x8001] === 0x50,
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
  completeImage(bootOffset, Math.min(bytes.length, 0x8000));
  let apps = 0;
  for (let offset = 0x8000; offset + 32 <= Math.min(bytes.length, 0x9000); offset += 32) {
    if (view.getUint16(offset, true) !== 0x50aa) break;
    const at = view.getUint32(offset + 4, true), size = view.getUint32(offset + 8, true);
    requireValue(size > 0 && at >= 0x9000 && at + size <= flashBytes, 'FLASH_OVERFLOW', 'Partition table contains a partition outside product flash.');
    if (bytes[offset + 2] === 0 && at < bytes.length && bytes[at] === 0xe9) { completeImage(at, Math.min(bytes.length, at + size)); apps++; }
  }
  requireValue(apps > 0, 'INCOMPLETE_IMAGE', 'Merged firmware contains no complete app image at a declared partition offset.');
  return true;
}

const romFor = board => board === 'esp32c3' ? 'esp32c3_rev3_rom.elf' : board === 'esp32p4' ? 'esp32p4_rev0_rom.elf' : board.includes('c6') ? 'esp32c6_rev0_rom.elf' : /^(esp32|ttgo-tdisplay)$/.test(board) ? 'esp32_rev300_rom.elf' : 'esp32s3_rev0_rom.elf';

export function createPlatformRuntime({ catalog: initialCatalog, onEvent, fetchImpl = globalThis.fetch, workerFactory = url => new Worker(url, { type: 'module' }), baseUrl = globalThis.location?.href || new URL('./', import.meta.url).href, assetBaseUrl = new URL('../', import.meta.url).href, requestTimeout = 30000 } = {}) {
  let catalog = initialCatalog, scene = null, product = null, status = 'idle', worker = null, aborter = null;
  let lastError = null, firmware = null, loaded = null, frame = null, frameNumber = 0, serial = '', serialByPort = {}, stats = {}, queue = Promise.resolve();
  let booted = false, inputSequence = 0;
  const pending = new Map(), listeners = new Set(onEvent ? [onEvent] : []);
  const snapshot = () => ({ version: API_VERSION, status, scene: clone(scene), product: clone(product), firmware: clone(firmware), serial, serialByPort: { ...serialByPort }, stats: clone(stats), frame: frame ? { width: frame.width, height: frame.height, frameNumber } : null, lastError: clone(lastError) });
  const emit = event => { for (const listener of listeners) { try { listener(event); } catch (e) { console.error('ESP tester event handler:', e); } } };
  const setStatus = value => { status = value; emit({ type: 'state', state: snapshot() }); };
  const report = e => { lastError = failure(e); emit({ type: 'error', error: clone(lastError) }); };
  const alive = () => requireValue(status !== 'disposed', 'DISPOSED', 'This emulator session has been disposed.');
  const exclusive = fn => {
    const next = queue.then(() => { alive(); return fn(); });
    queue = next.catch(() => {});
    return next.catch(e => { report(e); throw e; });
  };
  const resolve = (key, value) => { const item = pending.get(key); if (!item) return; pending.delete(key); clearTimeout(item.timeout); item.resolve(value); };
  const rejectPending = e => { for (const item of pending.values()) { clearTimeout(item.timeout); item.reject(e); } pending.clear(); };
  const stopWorker = () => { worker?.terminate(); worker = null; rejectPending(error('SESSION_REPLACED', 'The emulator session was replaced.')); booted = false; };
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
    if (message.ready) resolve('ready', message);
    if (message.created !== undefined) resolve('created', message);
    if (message.loaded !== undefined) resolve(`loaded:${message.loaded}`, message);
    if (message.started !== undefined) resolve('started', message);
    if (message.resumed !== undefined) resolve('resumed', message);
    if (message.inputQueued !== undefined) resolve(`input:${message.inputQueued}`, message);
    if (message.stopped !== undefined) {
      const requested = pending.has('stopped');
      resolve('stopped', message);
      if (!requested) {
        if (message.stopped !== 0) { const e = error('EMULATOR_STOPPED', `Emulator stopped with code ${message.stopped}.`); rejectPending(e); report(e); setStatus('error'); }
        else if (status === 'running') setStatus('paused');
      }
    }
  };
  const runtimeConfig = () => {
    const runtime = product?.runtime || {};
    requireValue(runtime.board, 'UNSUPPORTED_PRODUCT', 'This product has catalog metadata but no executable board model yet.');
    return { board: runtime.board, flash_mb: runtime.flashMb ?? 8, psram_mb: runtime.psramMb ?? 2 };
  };
  const readArtifact = async (entry, root, maxBytes = MAX_FILE) => {
    const item = typeof entry === 'string' ? { url: entry } : entry;
    requireValue(item && typeof item.url === 'string', 'INVALID_MANIFEST', 'Each firmware file needs a URL.');
    const url = resolveFirmwareUrl(item.url, root), data = await fetchBytes(url, { fetchImpl, maxBytes, signal: aborter.signal });
    requireValue(data.length > 0, 'EMPTY_FILE', 'Firmware file is empty.', { url });
    return { data, url, sha256: await checkedHash(data, item.sha256) };
  };
  const prepare = async source => {
    requireValue(source && typeof source === 'object', 'FIRMWARE_REQUIRED', 'Select a firmware manifest or merged flash image first.');
    const cfg = runtimeConfig(), files = [], receipts = [];
    let total = 0;
    const add = (kind, artifact, at) => {
      total += artifact.data.length;
      requireValue(total <= MAX_TOTAL, 'BUNDLE_TOO_LARGE', 'Firmware bundle exceeds 64 MiB.');
      files.push({ kind, data: artifact.data, ...(at === undefined ? {} : { at }) });
      receipts.push({ kind, url: artifact.url, sha256: artifact.sha256, bytes: artifact.data.length, ...(at === undefined ? {} : { at }) });
    };
    let manifest = null, manifestUrl = null, manifestHash = null;
    if (source.manifestUrl) {
      manifestUrl = resolveFirmwareUrl(source.manifestUrl, baseUrl);
      const artifact = await readArtifact({ url: manifestUrl, sha256: source.sha256 }, baseUrl, MAX_MANIFEST);
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
          add(kind, await readArtifact(pinned(entry), manifestUrl));
        }
      }
      for (const [offset, entry] of Object.entries(manifest.flash_at || {})) {
        requireValue(/^0x[0-9a-f]+$/i.test(offset), 'INVALID_MANIFEST', 'flash_at offsets must be hexadecimal strings beginning with 0x.');
        const at = Number(offset), artifact = await readArtifact(pinned(entry), manifestUrl);
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
    } else {
      requireValue(!source.format || source.format === 'merged', 'UNSUPPORTED_FORMAT', 'Raw firmware must be a merged flash image.');
      let artifact;
      if (source.file) {
        requireValue(typeof source.file.arrayBuffer === 'function' && source.file.size > 0 && source.file.size <= MAX_FILE, 'INVALID_FILE', 'Choose a nonempty local file no larger than 32 MiB.');
        const data = new Uint8Array(await source.file.arrayBuffer());
        artifact = { data, url: null, sha256: await checkedHash(data, source.sha256) };
      } else artifact = await readArtifact(source, baseUrl);
      validateMergedImage(artifact.data, cfg.board, cfg.flash_mb * MIB);
      add('flash', artifact);
    }
    if (!cfg.appDirect && !files.some(f => f.kind === 'rom')) add('rom', await readArtifact(new URL(`wasm/fw/${romFor(cfg.board)}`, assetBaseUrl).href, baseUrl));
    return { cfg, files, receipt: { manifestUrl, manifestHash, source: source.file ? 'local' : 'remote', artifacts: receipts, appDirect: !!cfg.appDirect, stubs: cfg.stubs || [] } };
  };
  const initialize = async bundle => {
    stopWorker();
    const wasm = await fetchBytes(new URL('wasm/esp32sim.wasm', assetBaseUrl).href, { fetchImpl, maxBytes: MAX_FILE, signal: aborter.signal });
    alive();
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
    for (const spec of bundle.cfg.stubs || []) worker.postMessage({ op: 'stub', spec });
    if (bundle.cfg.wifi) worker.postMessage({ op: 'wifi', spec: bundle.cfg.wifi });
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
    requireValue(status === 'running', 'NOT_RUNNING', 'Start the emulator before sending input.');
    requireValue(event && typeof event === 'object', 'INVALID_INPUT', 'Input must be an object.');
    let message;
    if (event.type === 'button') {
      const control = controls().find(c => c.id === event.id);
      const pin = control?.pin ?? control?.gpio;
      requireValue(Number.isInteger(pin) && pin >= 0 && pin < 64, 'UNSUPPORTED_CONTROL', 'This product does not expose that GPIO button.');
      requireValue(typeof event.pressed === 'boolean', 'INVALID_INPUT', 'Button pressed must be a boolean.');
      message = { t: 'btn', pin, v: event.pressed ? 1 : 0 };
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
    subscribe(listener) { requireValue(typeof listener === 'function', 'INVALID_LISTENER', 'Event listener must be a function.'); listeners.add(listener); return () => listeners.delete(listener); },
    async observe() { return snapshot(); },
    async capabilities() {
      return { version: API_VERSION, backend: 'esp32sim-wasm', static: true, executable: !!product?.runtime?.board,
        lifecycle: ['configure', 'loadFirmware', 'start', 'pause', 'reset', 'dispose'],
        firmware: { manifest: true, mergedImage: true, localFile: true, sha256: true, maxFileBytes: MAX_FILE, corsRequired: true },
        input: { buttons: product?.runtime?.board ? controls().filter(c => Number.isInteger(c.pin ?? c.gpio)).map(c => c.id) : [], touch: !!(product?.runtime?.board && (product?.runtime?.touch || product?.display?.touch)), serialPorts: ['usb', 'uart0', 'uart1'] },
        framebuffer: { available: !!frame, formats: ['rgba', 'png'], source: 'emulator' },
        gps: { mode: 'logical-uart1-nmea', physicalRouting: false, baudTiming: false, hardwareValidated: false },
        unsupported: ['dynamic-peripheral-attachment', 'uart2-input', 'physical-gps-radio', 'arbitrary-firmware-compatibility', 'cycle-exact-host-input'] };
    },
    configure(nextScene) { return exclusive(async () => {
      catalog ||= await loadCatalog();
      const validation = validateScene(nextScene, catalog);
      requireValue(validation.valid, 'INVALID_SCENE', 'Scene configuration is invalid.', validation);
      aborter?.abort(); stopWorker(); loaded = null; firmware = null; frame = null; frameNumber = 0; serial = ''; serialByPort = {}; stats = {}; lastError = null;
      scene = clone(nextScene); product = catalog.products.find(item => item.id === scene.productId);
      setStatus('configured'); return { ...snapshot(), warnings: validation.warnings };
    }); },
    updateSceneMetadata(changes) { return exclusive(async () => {
      requireValue(scene, 'NOT_CONFIGURED', 'Configure a scene before updating its presentation.');
      requireValue(changes && typeof changes === 'object' && Object.keys(changes).every(key => ['keyMapping', 'view'].includes(key)), 'INVALID_SCENE', 'Only keyMapping and view can be changed without reconfiguring the emulator.');
      const next = { ...scene, ...clone(changes) }, validation = validateScene(next, catalog);
      requireValue(validation.valid, 'INVALID_SCENE', 'Scene configuration is invalid.', validation);
      scene = next; emit({ type: 'state', state: snapshot() }); return snapshot();
    }); },
    loadFirmware(source) { return exclusive(async () => {
      requireValue(scene, 'NOT_CONFIGURED', 'Configure a product before loading firmware.');
      stopWorker(); loaded = null; firmware = null; frame = null; frameNumber = 0; serial = ''; serialByPort = {}; stats = {}; lastError = null;
      aborter = new AbortController(); setStatus('loading');
      try {
        const selected = source || scene.firmware, bundle = await prepare(selected);
        await initialize(bundle); loaded = bundle; firmware = bundle.receipt;
        if (!selected.file) scene.firmware = bundle.receipt.manifestUrl
          ? { manifestUrl: bundle.receipt.manifestUrl, sha256: bundle.receipt.manifestHash }
          : { url: bundle.receipt.artifacts[0].url, sha256: bundle.receipt.artifacts[0].sha256 };
        setStatus('ready'); return snapshot();
      }
      catch (e) { stopWorker(); if (status !== 'disposed') setStatus('error'); throw e; }
    }); },
    start() { return exclusive(start); },
    pause() { return exclusive(async () => {
      requireValue(['running', 'paused'].includes(status), 'NOT_RUNNING', 'Only a running emulator can be paused.');
      if (status === 'running') await ask('stopped', { op: 'stop' });
      setStatus('paused'); return snapshot();
    }); },
    reset() { return exclusive(async () => {
      requireValue(loaded, 'NOT_READY', 'Load firmware before resetting.');
      aborter = new AbortController(); serial = ''; serialByPort = {}; stats = {}; frame = null; frameNumber = 0; lastError = null; setStatus('loading');
      try { await initialize(loaded); setStatus('ready'); return await start(); }
      catch (e) { stopWorker(); if (status !== 'disposed') setStatus('error'); throw e; }
    }); },
    input(event) { return exclusive(() => input(event)); },
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
    async dispose() { if (status !== 'disposed') { aborter?.abort(); stopWorker(); loaded = null; setStatus('disposed'); listeners.clear(); } return snapshot(); },
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
