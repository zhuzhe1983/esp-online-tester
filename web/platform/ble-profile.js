// Audited virtual-controller metadata, selected by exact firmware bytes, never by filename.
// The guest NimBLE host still executes; this does not connect the browser to physical BLE.
const FACTORY_SHA = '411578a2ebca2cfe3541fdc32aeddbc4703cf912a5daa7e1253f69306d6e1d87';
const PROFILES = [{id:'stackchan-factory-1.5.1',board:'stackchan',sha256:FACTORY_SHA,bytes:12783789,profileSha256:'d9fb0e726cc36fc1357dfad3fcb272da639716db7ea3a8546ed35cc506943437',url:'./assets/firmware-profiles/stackchan-factory-1.5.1/profile.json'}];
const hashPattern = /^[a-f0-9]{64}$/;
const reject = message => { throw Object.assign(new Error(message), {code:'BLE_PROFILE_MISMATCH'}); };
const requireValue = (condition, message) => { if (!condition) reject(message); };
const validRange = (range, limit) => Number.isSafeInteger(range?.offset) && range.offset >= 0 && Number.isSafeInteger(range.bytes) && range.bytes > 0 && range.offset + range.bytes <= limit;

export function selectBleProfile(board, files, artifacts) {
  // A second app/flash segment could overwrite the audited image during loading.
  if (files.length !== artifacts.length || files.filter(file => file.kind !== 'rom').length !== 1) return null;
  const profile = PROFILES.find(item => item.board === board && artifacts.some((artifact, index) => artifact.kind === 'flash' && (artifact.at ?? 0) === 0 && artifact.sha256 === item.sha256 && artifact.bytes === item.bytes && files[index]?.kind === 'flash' && (files[index].at ?? 0) === 0));
  if (!profile) return null;
  const index = artifacts.findIndex(artifact => artifact.kind === 'flash' && (artifact.at ?? 0) === 0 && artifact.sha256 === profile.sha256 && artifact.bytes === profile.bytes);
  return {...profile, image:files[index].data};
}

export async function verifyBleProfile(profile, selected, symbols, digest) {
  requireValue(profile?.id === selected.id && profile.board === selected.board && profile.format === 'merged' && profile.firmwareSha256 === selected.sha256 && profile.firmwareBytes === selected.bytes, 'The BLE profile does not describe the selected firmware.');
  requireValue(selected.image.byteLength === selected.bytes && await digest(selected.image) === selected.sha256, 'Firmware bytes changed before BLE profile validation.');
  requireValue(profile.symbols && /^[a-zA-Z0-9][a-zA-Z0-9._-]*\.elf$/.test(profile.symbols.file) && hashPattern.test(profile.symbols.sha256) && Number.isSafeInteger(profile.symbols.bytes) && profile.symbols.bytes > 0 && profile.symbols.bytes <= 65536, 'Invalid BLE symbol metadata.');
  requireValue(symbols.byteLength === profile.symbols.bytes && await digest(symbols) === profile.symbols.sha256, 'BLE symbol metadata failed its checksum.');
  requireValue(Array.isArray(profile.windows) && profile.windows.length > 0 && profile.windows.length <= 64, 'Missing BLE symbol fingerprints.');
  for (const window of profile.windows) {
    requireValue(validRange(window, selected.bytes) && window.bytes <= 65536 && hashPattern.test(window.sha256), 'Invalid BLE symbol fingerprint range.');
    requireValue(await digest(selected.image.subarray(window.offset, window.offset + window.bytes)) === window.sha256, `BLE symbol fingerprint mismatch: ${window.symbol || window.offset}.`);
  }
  requireValue(Array.isArray(profile.mutableFlashRanges) && profile.mutableFlashRanges.length <= 32, 'Missing BLE persistent-data boundaries.');
  for (const range of profile.mutableFlashRanges) {
    requireValue(validRange(range, 16 * 1024 * 1024), 'Invalid BLE persistent-data boundary.');
    requireValue(!profile.windows.some(window => range.offset < window.offset + window.bytes && range.offset + range.bytes > window.offset), 'BLE persistent data overlaps a controller hook.');
  }
}

export function verifyBleStoredPages(profile, baseline, pages) {
  for (const [offset, data] of pages) {
    const bytes = new Uint8Array(data);
    for (let index = 0; index < bytes.length; index++) {
      const at = offset + index;
      if (bytes[index] === baseline[at]) continue;
      requireValue(profile.mutableFlashRanges.some(range => at >= range.offset && at < range.offset + range.bytes), 'Saved Flash changed executable firmware or boot selection; the fixed BLE profile cannot be applied. Device storage has been preserved.');
    }
  }
}

export async function loadBleProfile(selected, {read, digest}) {
  if (!selected) return null;
  const url = new URL(selected.url, import.meta.url);
  const bytes = await read(url.href, 65536);
  requireValue(hashPattern.test(selected.profileSha256) && await digest(bytes) === selected.profileSha256, 'BLE profile metadata failed its pinned checksum.');
  let profile;
  try { profile = JSON.parse(new TextDecoder().decode(bytes)); } catch { reject('Invalid BLE profile JSON.'); }
  requireValue(typeof profile?.symbols?.file === 'string' && /^[a-zA-Z0-9][a-zA-Z0-9._-]*\.elf$/.test(profile.symbols.file), 'BLE symbol metadata must be a local file.');
  const symbols = await read(new URL(profile.symbols.file, url).href, 65536);
  await verifyBleProfile(profile, selected, symbols, digest);
  return {profile, symbols, receipt:{id:profile.id, model:'virtual-vhci', physicalBluetooth:false, firmwareSha256:profile.firmwareSha256, symbolsSha256:profile.symbols.sha256}};
}
