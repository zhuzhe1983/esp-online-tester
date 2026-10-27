import {MAX_FIRMWARE_FILE_BYTES as MAX_FILE, MAX_FIRMWARE_BUNDLE_BYTES as MAX_TOTAL} from './firmware-limits.js';
// ESP-IDF flasher_args.json is data, never a command to execute.
// Format: espressif/esp-idf components/esptool_py/flasher_args.json.in (v5.5.1).
const MIB = 1024 * 1024;
const MAX_SEGMENTS = 32, MAX_FLASH = 32 * MIB;
const fail = (code, message, details) => { throw Object.assign(new Error(message), {code, details}); };
const ensure = (ok, code, message, details) => { if (!ok) fail(code, message, details); };
const cancelled = signal => ensure(!signal?.aborted, 'FETCH_ABORTED', 'Local firmware load was cancelled.');
const object = value => value && typeof value === 'object' && !Array.isArray(value);
const basename = path => path.split('/').at(-1);
function relativePath(value) {
  ensure(typeof value === 'string' && value.length > 0 && value.length <= 2048 && !/[\x00-\x1f]/.test(value), 'INVALID_BUILD_PATH', 'Build file path is invalid.');
  const path = value.replace(/\\/g, '/');
  ensure(!path.startsWith('/') && !/^[a-z]:/i.test(path) && !path.split('/').includes('..'), 'INVALID_BUILD_PATH', 'Build files must use relative paths without parent traversal.');
  const normalized = path.split('/').filter(p => p && p !== '.').join('/');
  ensure(normalized.length > 0, 'INVALID_BUILD_PATH', 'Build file path must name a file.');
  return normalized;
}
function offsetValue(value) {
  ensure(typeof value === 'number' || typeof value === 'string' && /^(?:0x[0-9a-f]+|[0-9]+)$/i.test(value), 'INVALID_FLASH_OFFSET', 'Flash offsets must be integers or decimal/0x hexadecimal strings.');
  const offset = Number(value);
  ensure(Number.isSafeInteger(offset) && offset >= 0 && offset % 0x1000 === 0, 'INVALID_FLASH_OFFSET', 'Flash offsets must be nonnegative 4096-byte-aligned integers.');
  return offset;
}
function localFile(file, maxBytes = MAX_FILE) {
  ensure(file && typeof file.arrayBuffer === 'function' && Number.isSafeInteger(file.size) && file.size > 0 && file.size <= maxBytes, 'INVALID_FILE', `Local file must be nonempty and no larger than ${maxBytes} bytes.`);
  return file;
}
async function readFile(file, {signal, onRead} = {}) {
  cancelled(signal); localFile(file); onRead?.({stage:'local-read', name:file.name || 'local.bin', loadedBytes:0, totalBytes:file.size});
  let bytes;
  try { bytes = new Uint8Array(await file.arrayBuffer()); }
  catch (cause) { cancelled(signal); fail('LOCAL_READ_FAILED', 'The selected local file could not be read.', {name:file.name || 'local.bin', reason:cause.message}); }
  cancelled(signal); ensure(bytes.length === file.size, 'LOCAL_READ_FAILED', 'Local file size changed while reading.');
  onRead?.({stage:'local-read', name:file.name || 'local.bin', loadedBytes:bytes.length, totalBytes:bytes.length});
  return bytes;
}

/** Inspect only JSON and selected-file metadata. No binaries are read here. */
export async function inspectLocalBuild(selectedFiles, {signal} = {}) {
  const files = Array.from(selectedFiles || []);
  ensure(files.length > 0 && files.length <= 4096, 'LOCAL_FILE_COUNT', 'Choose 1–4096 build files, including flasher_args.json.');
  const paths = new Map(), flat = !files.some(file => file.webkitRelativePath);
  for (const file of files) {
    const path = relativePath(file.webkitRelativePath || file.name);
    ensure(!paths.has(path), 'DUPLICATE_BUILD_PATH', 'Two selected files have the same path; select their build directory to preserve paths.', {path});
    paths.set(path, file);
  }
  const configs = [...paths].filter(([path]) => basename(path) === 'flasher_args.json');
  ensure(configs.length === 1, 'BUILD_CONFIG_REQUIRED', 'Select exactly one flasher_args.json with its referenced binaries.', {found:configs.length});
  const [layoutFile, configFile] = configs[0]; localFile(configFile);
  let config;
  try { config = JSON.parse(new TextDecoder().decode(await readFile(configFile, {signal}))); }
  catch (e) { if (e.code) throw e; fail('INVALID_BUILD_CONFIG', 'flasher_args.json is not valid JSON.'); }
  ensure(object(config) && object(config.flash_files), 'INVALID_BUILD_CONFIG', 'flasher_args.json must contain a flash_files offset-to-path map.');
  const entries = Object.entries(config.flash_files);
  ensure(entries.length > 0 && entries.length <= MAX_SEGMENTS, 'LOCAL_SEGMENT_COUNT', 'A local build must contain 1–32 flash segments.');
  ensure(config.write_flash_args == null || Array.isArray(config.write_flash_args) && config.write_flash_args.every(v => typeof v === 'string'), 'INVALID_BUILD_CONFIG', 'write_flash_args must be a string array.');
  ensure(!Object.values(config).some(v => object(v) && (v.encrypted === true || v.encrypted === 'true')) && !(config.write_flash_args || []).some(v => v.startsWith('--encrypt')), 'ENCRYPTED_IMAGE_UNSUPPORTED', 'Encrypted flash images are not supported by this emulator.');
  const prefix = layoutFile.includes('/') ? layoutFile.slice(0, layoutFile.lastIndexOf('/') + 1) : '';
  const missingFiles = [], segments = [], offsets = new Set();
  for (const [rawOffset, rawPath] of entries) {
    const offset = offsetValue(rawOffset), path = relativePath(rawPath);
    ensure(!offsets.has(offset), 'DUPLICATE_FLASH_OFFSET', 'Build layout declares the same flash offset more than once.', {offset}); offsets.add(offset);
    let file = paths.get(prefix + path);
    if (!file && flat) {
      const matches = [...paths].filter(([p]) => basename(p) === basename(path));
      ensure(matches.length <= 1, 'AMBIGUOUS_BUILD_FILE', 'A build filename matches multiple selected files; choose the build directory.', {path});
      file = matches[0]?.[1];
    }
    if (!file) missingFiles.push(path);
    else { localFile(file); segments.push({file, offset, path}); }
  }
  const declaredSize = config.flash_settings?.flash_size;
  ensure(declaredSize == null || typeof declaredSize === 'string' && (/^\d+MB$/i.test(declaredSize) || ['detect','keep'].includes(declaredSize)), 'INVALID_BUILD_CONFIG', 'Unsupported declared flash_size.');
  const flashSizeBytes = /^\d+MB$/i.test(declaredSize || '') ? Number.parseInt(declaredSize,10) * MIB : null;
  const chip = config.extra_esptool_args?.chip ?? null;
  ensure(chip === null || typeof chip === 'string' && /^esp32[a-z0-9]*$/.test(chip), 'INVALID_BUILD_CONFIG', 'Invalid build target chip.');
  return {layoutFile, chip, flashSizeBytes, segments:segments.sort((a,b)=>a.offset-b.offset), missingFiles, warnings:[]};
}

const chipForBoard = board => /^(esp32|ttgo-tdisplay|atom-lite)$/.test(board) ? 'esp32' : board === 'esp32c3' ? 'esp32c3' : board.includes('c6') ? 'esp32c6' : board === 'esp32p4' ? 'esp32p4' : 'esp32s3';
/** Return a temporary merged buffer with erased 0xff gaps, never app-direct. */
export async function assembleLocalFirmware(source, {board, flashBytes, signal, onRead} = {}) {
  ensure(object(source) && ['build','segments'].includes(source.format), 'UNSUPPORTED_FORMAT', 'Choose a build directory or explicit local flash segments.');
  ensure(!source.file && !source.url && !source.manifestUrl, 'INVALID_LOCAL_SOURCE', 'Do not mix local segments with a remote or merged source.');
  cancelled(signal);
  let layout;
  if (source.format === 'build') {
    layout = await inspectLocalBuild(source.files, {signal});
    ensure(layout.missingFiles.length === 0, 'LOCAL_FILES_MISSING', 'Some binaries named by flasher_args.json are missing.', {missingFiles:layout.missingFiles});
    ensure(!layout.chip || layout.chip === chipForBoard(board), 'CHIP_MISMATCH', 'Local build targets a different ESP32 chip.', {expected:chipForBoard(board), actual:layout.chip});
    ensure(!layout.flashSizeBytes || layout.flashSizeBytes <= flashBytes, 'FLASH_OVERFLOW', 'Local build expects more flash than this product provides.');
  } else {
    ensure(Array.isArray(source.files) && source.files.length > 0 && source.files.length <= MAX_SEGMENTS, 'LOCAL_SEGMENT_COUNT', 'Choose 1–32 local flash segments.');
    layout = {layoutFile:null, chip:null, flashSizeBytes:null, segments:Array.from(source.files).map(entry => {
      ensure(object(entry), 'INVALID_LOCAL_SOURCE', 'Every local segment needs file and offset.');
      return {file:localFile(entry.file), offset:offsetValue(entry.offset), path:basename(relativePath(entry.file.name || 'local.bin'))};
    }).sort((a,b)=>a.offset-b.offset)};
  }
  ensure(Number.isSafeInteger(flashBytes) && flashBytes > 0 && flashBytes <= MAX_FLASH, 'FLASH_OVERFLOW', 'Invalid selected flash capacity.');
  let total = 0, end = 0;
  const sameFiles = new Set();
  for (const entry of layout.segments) {
    ensure(!sameFiles.has(entry.file), 'DUPLICATE_LOCAL_FILE', 'The same local file is assigned more than once.'); sameFiles.add(entry.file);
    ensure(entry.offset >= end, 'FLASH_SEGMENT_OVERLAP', 'Local flash segments overlap.', {offset:entry.offset, previousEnd:end});
    total += entry.file.size; end = entry.offset + entry.file.size;
    ensure(total <= MAX_TOTAL && end <= flashBytes, 'FLASH_OVERFLOW', 'Local segments exceed product flash capacity or the bundle limit.');
  }
  const segments = [];
  for (const entry of layout.segments) segments.push({...entry, data:await readFile(entry.file, {signal,onRead})});
  const tables = segments.filter(s => s.data.length >= 32 && s.data[0] === 0xaa && s.data[1] === 0x50);
  ensure(tables.length === 1, 'PARTITION_TABLE_REQUIRED', 'Select exactly one binary partition table with its actual flash offset. An app.bin alone is not bootable.');
  const table = tables[0], partitionTableOffset = table.offset;
  ensure(partitionTableOffset >= 0x8000 && table.data.length <= 0x1000, 'INVALID_PARTITION_TABLE', 'Partition table must occupy at most one 4096-byte sector at offset 0x8000 or above.');
  const bootOffset = chipForBoard(board) === 'esp32' ? 0x1000 : 0;
  const boot = segments.find(s => s.offset === bootOffset);
  ensure(boot && boot.data[0] === 0xe9 && boot.offset + boot.data.length <= partitionTableOffset, 'BOOTLOADER_REQUIRED', `A complete matching bootloader at 0x${bootOffset.toString(16)} is required; app-direct boot is not used.`);
  const partitions = [], dv = new DataView(table.data.buffer, table.data.byteOffset, table.data.byteLength);
  for (let at = 0; at + 32 <= table.data.length && dv.getUint16(at,true) === 0x50aa; at += 32) {
    const offset = dv.getUint32(at+4,true), size = dv.getUint32(at+8,true), type = table.data[at+2];
    ensure(offset >= partitionTableOffset + 0x1000 && offset % 0x1000 === 0 && size > 0 && offset + size <= flashBytes, 'INVALID_PARTITION_TABLE', 'A partition lies outside valid product flash.', {offset,size});
    ensure(!partitions.some(p => offset < p.offset+p.size && p.offset < offset+size), 'PARTITION_OVERLAP', 'Binary partition table contains overlapping partitions.');
    partitions.push({offset,size,type});
  }
  let apps = 0;
  for (const s of segments) {
    if (s === boot || s === table) continue;
    const partition = partitions.find(p => p.offset === s.offset);
    ensure(partition && s.data.length <= partition.size, 'SEGMENT_NOT_IN_PARTITION', 'A local image does not start at a declared partition or exceeds its size.', {offset:s.offset, bytes:s.data.length});
    if (partition.type === 0) { ensure(s.data[0] === 0xe9, 'INCOMPLETE_IMAGE', 'App partition does not contain an ESP-IDF image.'); apps++; }
  }
  ensure(apps > 0, 'APP_IMAGE_REQUIRED', 'No app binary was provided at an app partition offset.');
  cancelled(signal);
  const data = new Uint8Array(end).fill(0xff);
  for (const s of segments) data.set(s.data,s.offset);
  return {data, partitionTableOffset, layoutFile:layout.layoutFile, segments:segments.map(({file,...entry})=>entry)};
}
