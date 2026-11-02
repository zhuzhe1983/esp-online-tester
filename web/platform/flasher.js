import {firmwareBoardMatches} from './board-compatibility.js';
import {MAX_FIRMWARE_FILE_BYTES as MAX_FILE, MAX_FIRMWARE_BUNDLE_BYTES as MAX_TOTAL} from './firmware-limits.js';
// Hardware writes are available only from a user's Web Serial chooser gesture.
import { fetchBytes, resolveFirmwareUrl, sha256, validateMergedImage } from './runtime.js';
import { validateScene } from './catalog.js';
import { loadWatchlist } from './watchlist.js';
import { firmwareCache } from './firmware-cache.js';
import { assembleLocalFirmware, inspectLocalBuild } from './local-firmware.js';
import {firmwareFlashPolicy} from './firmware-policy.js';
import {validateAppImage} from './app-image.js';
import {createFlashHardReset,resetFlashedDevice} from './flash-reset.js';
import {createFlashTransport,waitForFlashClose} from './flash-transport.js';

const MIB = 1024 * 1024, MAX_FLASH = 32 * MIB;
const problem = (code, message, details) => Object.assign(new Error(message), { code, details });
const check = (condition, code, message, details) => { if (!condition) throw problem(code, message, details); };
const normalizedChip = value => String(value || '').toLowerCase().replace(/[^a-z0-9]/g, '');
const clone = value => structuredClone(value);
const cancelled = () => problem('FLASH_CANCELLED', 'Flashing was cancelled. If writing had begun, the device may contain a partial image; reconnect in bootloader mode and flash again.');
// Bytes and port grants remain private to a prepared batch. Public receipts are
// copies, so changing the scene or a receipt cannot change an in-progress batch.
const preparedSessions = new WeakMap(), occupiedPorts = new WeakSet();
const gesture = nav => check(nav?.userActivation?.isActive === true, 'USER_GESTURE_REQUIRED', 'Click the flash button to authorize this operation.');

export async function prepareFlashSession(input, {navigator:nav=globalThis.navigator,fetchImpl=globalThis.fetch,baseUrl=globalThis.location?.href,signal,loadWatchlistData=options=>loadWatchlist({fetchImpl,...options})}={}) {
  const source=input.source || input.scene?.firmware;
  const policy=firmwareFlashPolicy(selectedProduct(input.scene,input.catalog),source);
  check(policy.canFlash,policy.reason,policy.message);
  const fixed={scene:clone(input.scene),catalog:clone(input.catalog),source:{...source},watchEntry:input.watchEntry && clone(input.watchEntry)};
  if(source?.files)fixed.source.files=source.files.map(file=>file.file?{...file}:file);
  const watchlist=await loadWatchlistData({signal});
  const plan=await prepareFlashPlan({...fixed,watchlist,fetchImpl,baseUrl,signal});
  if(signal?.aborted)throw cancelled();
  const session=Object.freeze({product:clone(plan.product),receipt:clone(plan.receipt)});
  preparedSessions.set(session,{plan,ports:new WeakSet(),active:false,nav});
  return session;
}

export async function chooseFlashPort(session,{navigator:nav=globalThis.navigator,document:doc=globalThis.document,secureContext=globalThis.isSecureContext}={}) {
  const record=preparedSessions.get(session);
  check(record && record.nav===nav,'FLASH_SESSION_INVALID','Prepare a firmware batch first.');
  const capability=flasherCapabilities({navigator:nav,document:doc,secureContext});
  check(capability.supported,'WEB_SERIAL_UNAVAILABLE',capability.reason);gesture(nav);
  // No asynchronous work may precede requestPort: the chooser needs this click.
  const port=await nav.serial.requestPort();
  check(preparedSessions.has(session),'FLASH_SESSION_INVALID','This firmware batch has been closed.');
  record.ports.add(port);return port;
}

export function activateFlashSession(session,active,{navigator:nav=globalThis.navigator}={}) {
  const record=preparedSessions.get(session);
  check(record && record.nav===nav,'FLASH_SESSION_INVALID','Prepare a firmware batch first.');
  if(active)gesture(nav);record.active=!!active;
}
export function releaseFlashSession(session) { if(session)preparedSessions.delete(session); }

export function flasherCapabilities({ navigator: nav = globalThis.navigator, document: doc = globalThis.document, secureContext = globalThis.isSecureContext } = {}) {
  const policy = doc?.permissionsPolicy || doc?.featurePolicy;
  const allowed = !policy?.allowsFeature || policy.allowsFeature('serial');
  const supported = secureContext === true && !!nav?.serial?.requestPort && allowed;
  return { supported, requiresUserGesture: true, iframePolicy: 'serial', reason: secureContext !== true ? 'HTTPS 或 localhost 才能使用 Web Serial。' : !nav?.serial?.requestPort ? '此浏览器没有 Web Serial；请在支持该接口的桌面浏览器中打开。' : !allowed ? '嵌入页未获 serial 权限；宿主页需允许 serial Permissions Policy，iframe 需 allow="serial"。' : '', eraseAll: false, persistentFirmwareCache: true };
}

function selectedProduct(scene, catalog) {
  const validation = validateScene(scene, catalog);
  check(validation.valid, 'INVALID_SCENE', 'Cannot flash an invalid scene.', validation);
  const product = catalog.products.find(item => item.id === scene.productId);
  check(product && ['esp32s3', 'esp32c3', 'esp32c6'].includes(normalizedChip(product.chip)), 'FLASH_UNSUPPORTED_CHIP', 'This flasher currently accepts ESP32-S3, C3 and C6 product definitions with security-info support.');
  return product;
}

function artifactTarget(url, product, catalog, watched) {
  const asset = watched?.assets?.find(item => item.url === url || item.upstreamUrl === url);
  const urls = [url, asset?.url, asset?.upstreamUrl].filter(Boolean);
  const matches = catalog.firmwares.filter(item => item.source && [item.source.url, item.source.manifestUrl].some(value => value && urls.includes(value)));
  const firmware = matches.find(item => item.productIds?.includes(product.id));
  check(!matches.length || firmware, 'BOARD_MISMATCH', 'Catalog firmware does not target the selected product.');
  check(!firmware || ['available', 'trial'].includes(firmware.status), 'REFERENCE_ONLY', 'This catalog entry has no flashable artifact.');
  // A precise catalog artifact/target is independent of simulation verification.
  // Broad watchlist project membership alone does not identify a device image.
  check(!watched || asset && firmware || watched.compatibility?.status === 'product-verified' && watched.compatibility.productIds?.includes(product.id), 'REFERENCE_ONLY', 'This watchlist asset has no confirmed firmware mapping for the selected product.');
  return {asset, firmware};
}

async function flashBuild(source, product, signal) {
  const board=normalizedChip(product.chip),flashBytes=product.runtime.flashMb*MIB;
  check(!source.file && !source.url && !source.manifestUrl,'INVALID_LOCAL_SOURCE','Do not mix local segments with a remote or merged source.');
  const layout=source.format==='build'?await inspectLocalBuild(source.files,{signal}):{segments:source.files};
  // Some flasher_args.json files contain a single merged image at offset zero.
  // Try that format first; otherwise preserve each declared partition offset.
  if(Array.isArray(layout.segments) && layout.segments.length===1 && /^(?:0|0x0+)$/i.test(String(layout.segments[0]?.offset)) && !layout.missingFiles?.length) {
    const file=layout.segments[0].file;
    check(file?.size>0 && file.size<=MAX_FILE && typeof file.arrayBuffer==='function','INVALID_FILE','Choose a nonempty firmware file no larger than 256 MiB.');
    const data=new Uint8Array(await file.arrayBuffer());if(signal?.aborted)throw cancelled();
    check(data.length===file.size,'INVALID_FILE','Firmware file size changed while reading.');
    try {
      validateMergedImage(data,board,flashBytes);
      check(!layout.chip || layout.chip===board,'CHIP_MISMATCH','Build metadata targets a different chip.');
      check(!layout.flashSizeBytes || layout.flashSizeBytes<=flashBytes,'FLASH_OVERFLOW','Build metadata exceeds the product flash capacity.');
      return {data,partitionTableOffset:0x8000,segments:[{data,offset:0}],format:'merged'};
    } catch(error) {if(error.code!=='INCOMPLETE_IMAGE')throw error;}
  }
  return {...await assembleLocalFirmware(source,{board,flashBytes,signal}),format:'segments'};
}

function appPartition(table, flashBytes) {
  const view = new DataView(table.buffer, table.byteOffset, table.byteLength), apps = [];
  for (let at = 0; at + 32 <= table.length; at += 32) {
    if (view.getUint16(at, true) !== 0x50aa) break;
    const offset = view.getUint32(at + 4, true), size = view.getUint32(at + 8, true);
    check(size > 0 && offset >= 0x9000 && offset + size <= flashBytes, 'FLASH_LAYOUT', 'Partition lies outside the selected product flash.');
    if (table[at + 2] === 0) apps.push({ offset, size, subtype: table[at + 3] });
  }
  const factory = apps.filter(item => item.subtype === 0);
  check(factory.length === 1 || apps.length === 1, 'FLASH_LAYOUT', 'The app address is ambiguous. Use explicit flash_at entries for OTA layouts.');
  return factory[0] || apps[0];
}

export function validateFlashLayout(files, product, {partitionTableOffset = 0x8000} = {}) {
  const flashBytes = product.runtime.flashMb * MIB;
  check(Number.isSafeInteger(flashBytes) && flashBytes > 0 && flashBytes <= MAX_FLASH, 'FLASH_LAYOUT', 'Invalid product flash capacity.');
  check(files.length > 0 && files.length <= 32, 'FLASH_LAYOUT', 'Flash plan needs 1–32 images.');
  const sorted = [...files].sort((a, b) => a.address - b.address);
  let previousEnd = 0;
  for (const file of sorted) {
    check(file.data instanceof Uint8Array && file.data.length > 0 && Number.isSafeInteger(file.address) && file.address >= 0 && file.address % 4096 === 0, 'FLASH_LAYOUT', 'Flash image addresses must be nonnegative 4 KiB sector boundaries.');
    check(file.address >= previousEnd && file.address + file.data.length <= flashBytes, 'FLASH_LAYOUT', 'Flash images overlap erase sectors or exceed product flash.');
    previousEnd = Math.ceil((file.address + file.data.length) / 4096) * 4096;
  }
  const length = Math.max(...sorted.map(file => file.address + file.data.length));
  const merged = new Uint8Array(length).fill(255);
  for (const file of sorted) merged.set(file.data, file.address);
  // Hardware image IDs follow the chip, independent of an emulator board alias.
  validateMergedImage(merged, normalizedChip(product.chip), flashBytes, {partitionTableOffset});
  return sorted;
}

// The runtime's bounded, no-store fetch/SHA/image validators are reused. Only bytes
// actually destined for flash are loaded: no emulator ROM, ELF, script or picture.
export async function prepareFlashPlan({ scene, catalog, source = scene?.firmware, watchEntry, watchlist, fetchImpl = globalThis.fetch, signal, baseUrl = globalThis.location?.href } = {}) {
  const product = selectedProduct(scene, catalog);
  if (watchEntry) artifactTarget(source?.url || source?.manifestUrl, product, catalog, watchEntry);
  check(source && typeof source === 'object', 'FIRMWARE_REQUIRED', 'Choose a complete firmware image or manifest first.');
  check(!source.archive,'FLASH_LAYOUT','ZIP 固件请先解压，并使用完整 BIN 或烧录清单刷写实体设备。');
  check(source.bootMode!=='app-direct','FLASH_LAYOUT','应用直启不是完整刷机镜像；请提供 bootloader、分区表和应用文件的清单。');
  if (source.url || source.manifestUrl) artifactTarget(resolveFirmwareUrl(source.url || source.manifestUrl, baseUrl), product, catalog);
  let total = 0;
  const read = async (entry, base, maxBytes = MAX_FILE) => {
    const value = typeof entry === 'string' ? { url: entry } : entry;
    check(value && typeof value.url === 'string', 'INVALID_MANIFEST', 'Firmware file requires a URL.');
    const url = resolveFirmwareUrl(value.url, base);
    const watched = watchlist?.entries.find(entry => entry.assets.some(asset => asset.url === url || asset.upstreamUrl === url));
    const {asset: watchedAsset, firmware: catalogArtifact} = artifactTarget(url, product, catalog, watched);
    const pin = value.sha256 || watchedAsset?.sha256 || catalogArtifact?.source.sha256;
    let cached = await firmwareCache.get(url, pin, maxBytes);
    if (cached && await sha256(cached.data) !== pin.toLowerCase()) { await firmwareCache.remove(url, pin); cached = null; }
    if (signal?.aborted) throw cancelled();
    const data = cached?.data || await fetchBytes(url, { fetchImpl, signal, maxBytes, sha256:pin });
    total += data.length; check(data.length > 0 && total <= MAX_TOTAL, 'FLASH_SIZE', 'Firmware bundle is empty or exceeds 256 MiB.');
    const actual = await sha256(data);
    // A reviewed catalog/watchlist URL remains pinned even when a caller omits
    // or substitutes its hash. Updating a remote release requires a new review.
    for (const expected of [value.sha256, watchedAsset?.sha256, catalogArtifact?.source.sha256].filter(Boolean)) {
      check(/^[a-f0-9]{64}$/i.test(expected) && actual === expected.toLowerCase(), 'HASH_MISMATCH', 'Firmware SHA-256 does not match.', { url, actual, expected });
    }
    if (!cached && pin && !signal?.aborted) await firmwareCache.putVerified(url, actual, data);
    return { data, url, sha256: actual };
  };
  const files = [], receipt = [];
  const add = (artifact, address, kind) => { files.push({ data: artifact.data, address }); receipt.push({ kind, address, bytes: artifact.data.length, url: artifact.url || null, sha256: artifact.sha256 }); };
  let manifestHash = null, partitionTableOffset = 0x8000, format = 'merged';
  if (source.manifestUrl) {
    const url = resolveFirmwareUrl(source.manifestUrl, baseUrl), artifact = await read({ url, sha256: source.sha256 }, baseUrl);
    manifestHash = artifact.sha256;
    let manifest;
    try { manifest = JSON.parse(new TextDecoder().decode(artifact.data)); } catch { throw problem('INVALID_MANIFEST', 'Firmware manifest is not valid JSON.'); }
    check(manifest && typeof manifest.files === 'object' && !Array.isArray(manifest.files), 'INVALID_MANIFEST', 'Firmware manifest must list files.');
    check(!manifest.nodes && !manifest.app_direct, 'FLASH_LAYOUT', 'Network and app-direct emulator manifests are not hardware flash plans.');
    check(firmwareBoardMatches(manifest.board,product.runtime.board || product.id) || manifest.productId === product.id, 'BOARD_MISMATCH', 'The manifest must explicitly target the selected product board.');
    check(manifest.flash_mb === product.runtime.flashMb, 'FLASH_SIZE', 'Manifest and product flash capacities differ.');
    const pin = entry => typeof entry === 'string' ? { url: entry, sha256: manifest.artifact_sha256?.[entry] } : entry;
    const values = {};
    for (const [kind, entry] of Object.entries(manifest.files)) {
      check(['rom', 'elf', 'script', 'picture', 'flash', 'bootloader', 'ptable', 'app'].includes(kind), 'INVALID_MANIFEST', `Unknown firmware kind: ${kind}.`);
      if (['rom', 'elf', 'script', 'picture'].includes(kind)) continue;
      check(!Array.isArray(entry), 'FLASH_LAYOUT', 'A hardware file kind must contain one image. Use flash_at for additional segments.');
      values[kind] = await read(pin(entry), url);
    }
    if (values.flash) {
      check(!values.bootloader && !values.ptable && !values.app && !Object.keys(manifest.flash_at || {}).length, 'FLASH_LAYOUT', 'Merged flash cannot be combined with overlapping segments.');
      add(values.flash, 0, 'flash');
    } else {
      format = 'segments';
      check(values.bootloader && values.ptable, 'FLASH_LAYOUT', 'Hardware flashing requires a bootloader and partition table.');
      add(values.bootloader, normalizedChip(product.chip) === 'esp32' ? 0x1000 : 0, 'bootloader');
      add(values.ptable, 0x8000, 'ptable');
      if (values.app) {
        const partition = appPartition(values.ptable.data, product.runtime.flashMb * MIB);
        check(values.app.data.length <= partition.size, 'FLASH_LAYOUT', 'The app is larger than its declared partition.');
        add(values.app, partition.offset, 'app');
      }
      for (const [offset, entry] of Object.entries(manifest.flash_at || {})) {
        check(/^0x[0-9a-f]+$/i.test(offset) && files.length < 32, 'FLASH_LAYOUT', 'Explicit flash offsets must be hexadecimal addresses; at most 32 images are accepted.');
        add(await read(pin(entry), url), Number(offset), 'flash_at');
      }
    }
  } else if (['build','segments'].includes(source.format)) {
    const local = await flashBuild(source, product, signal);format=local.format;
    partitionTableOffset = local.partitionTableOffset;
    if (source.sha256) check(await sha256(local.data) === source.sha256.toLowerCase(), 'HASH_MISMATCH', 'Local firmware SHA-256 does not match.');
    // Write only supplied segments: never erase the 0xff gaps used by simulation.
    for (const entry of local.segments) add({data:entry.data,url:null,sha256:await sha256(entry.data)},entry.offset,'flash_at');
  } else {
    let artifact;
    if (source.file) {
      check(source.file.size > 0 && source.file.size <= MAX_FILE && typeof source.file.arrayBuffer === 'function', 'INVALID_FILE', 'Choose a local merged flash image no larger than 256 MiB.');
      const data = new Uint8Array(await source.file.arrayBuffer()), actual = await sha256(data);
      check(!source.sha256 || actual === source.sha256.toLowerCase(), 'HASH_MISMATCH', 'Local firmware SHA-256 does not match.');
      artifact = { data, sha256: actual };
    } else artifact = await read(source, baseUrl);
    try { validateMergedImage(artifact.data,normalizedChip(product.chip),product.runtime.flashMb*MIB); }
    catch(error) {
      if(error.code!=='INCOMPLETE_IMAGE')throw error;
      try { await validateAppImage(artifact.data,normalizedChip(product.chip),product.runtime.flashMb*MIB); }
      catch(appError) {if(appError.code!=='APP_IMAGE_INVALID')throw appError;throw error;}
      throw problem('FLASH_LAYOUT','检测到独立应用分区镜像。请提供 flasher_args.json 及其引用的启动镜像、分区表和应用文件，按实际分区地址刷写。');
    }
    add(artifact, 0, 'flash');
  }
  return { product: clone(product), files: validateFlashLayout(files, product, {partitionTableOffset}), receipt: { productId: product.id, chip: product.chip, flashBytes: product.runtime.flashMb * MIB, format, manifestHash, partitionTableOffset, artifacts: receipt, hardwareValidated: false, persistence: 'user-browser-cache-for-pinned-remote-artifacts' } };
}

export function createFlasher({ onEvent, navigator: nav = globalThis.navigator, document: doc = globalThis.document, secureContext = globalThis.isSecureContext, fetchImpl = globalThis.fetch, baseUrl = globalThis.location?.href,
  loadWatchlistData = options => loadWatchlist({ fetchImpl, ...options }),
  loadTools = async () => { const [esptool, md5] = await Promise.all([import('./vendor/esptool-js-0.7.0/bundle.js'), import('./vendor/spark-md5-3.0.2/spark-md5.js')]); return { ...esptool, md5: data => md5.default.ArrayBuffer.hash(data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength)) }; } } = {}) {
  let state = { status: 'idle', progress: 0, message: '', writtenBytes: 0 }, active = false, aborter = null, wantsCancel = false, beganWrite = false;
  const emit = event => { try { onEvent?.(event); } catch (error) { console.error(error); } };
  const setState = (status, message, extra = {}) => { state = { ...state, status, message, ...extra }; emit({ type: 'state', state: clone(state) }); };
  const guard = () => { if (wantsCancel) throw cancelled(); };
  const capabilities = () => flasherCapabilities({ navigator: nav, document: doc, secureContext });
  return {
    capabilities,
    observe: () => clone(state),
    cancel() { if (active && state.status!=='closing') { wantsCancel = true; aborter?.abort(); setState('cancelling', beganWrite ? '正在当前协议块结束后停止；设备可能需要重新刷写。' : '正在取消。'); } return clone(state); },
    async flash({ scene, catalog, source = scene?.firmware, watchEntry, prepared, port: chosenPort } = {}) {
      check(!active, 'FLASH_BUSY', 'A flash operation is already active.');
      const capability = capabilities(); check(capability.supported, 'WEB_SERIAL_UNAVAILABLE', capability.reason);
      const session=prepared && preparedSessions.get(prepared);
      if(prepared)check(session?.active && session.nav===nav && session.ports.has(chosenPort),'FLASH_SESSION_INVALID','Start an authorized batch and select this device before writing.');
      else {
        gesture(nav);
        const product = selectedProduct(scene, catalog);
        if (watchEntry) artifactTarget(source?.url || source?.manifestUrl, product, catalog, watchEntry);
        const policy = firmwareFlashPolicy(product, source);
        check(policy.canFlash, policy.reason, policy.message);
        if (source.url || source.manifestUrl) artifactTarget(resolveFirmwareUrl(source.url || source.manifestUrl, baseUrl), product, catalog);
        check(!chosenPort,'FLASH_SESSION_INVALID','Ports must be selected within a prepared batch.');
      }
      active = true; wantsCancel = false; beganWrite = false; aborter = new AbortController();
      state = { status: 'choosing', message: '请选择要刷写的 USB 串口设备。', progress: 0, writtenBytes: 0 }; emit({ type: 'state', state: clone(state) });
      let transport, port, plan, ownsPort=false,verified=false;
      try {
        // This must be the first asynchronous call, directly inside the user's click.
        port = prepared ? chosenPort : await nav.serial.requestPort(); guard();
        check(!occupiedPorts.has(port),'FLASH_PORT_BUSY','This device is already being written by another operation.');
        occupiedPorts.add(port);ownsPort=true;
        if(prepared)plan=clone(session.plan);
        else {
          setState('preparing', '正在下载并校验固件；仅保留在本次操作的内存中。');
          const watchlist = await loadWatchlistData({ signal: aborter.signal }); guard();
          plan = await prepareFlashPlan({ scene, catalog, source, watchEntry, watchlist, fetchImpl, baseUrl, signal: aborter.signal }); guard();
        }
        const { ESPLoader, Transport, md5 } = await loadTools(); guard();
        transport = createFlashTransport(Transport,port);
        const loader = new ESPLoader({ transport, baudrate: 115200, romBaudrate: 115200, resetConstructors:{hardReset:createFlashHardReset}, debugLogging: false, terminal: { clean() {}, write: message => emit({ type: 'log', message: String(message) }), writeLine: message => emit({ type: 'log', message: String(message) }) } });
        setState('connecting', '正在连接 ROM 下载模式并识别芯片；此过程会复位设备。');
        await loader.main(); guard();
        const chip = normalizedChip(loader.chip?.CHIP_NAME), expected = normalizedChip(plan.product.chip);
        check(chip === expected, 'CHIP_MISMATCH', `所选设备为 ${loader.chip?.CHIP_NAME || '未知芯片'}，当前固件需要 ${plan.product.chip.replace(/^esp32/i,'ESP32-').toUpperCase()}。未写入 Flash，请点击“更换设备”选择正确的设备。`);
        setState('validating', '正在核对芯片、安全配置、Flash 容量与写入范围。', { chip: loader.chip.CHIP_NAME });
        // This release intentionally excludes secured/encrypted devices rather than
        // attempting efuse changes, forced writes or security-mode workarounds.
        const security = await loader.getSecurityInfo(); guard();
        check(!loader.secureDownloadMode && !security.parsedFlags?.SECURE_BOOT_EN && !security.parsedFlags?.SECURE_DOWNLOAD_ENABLE && security.flashCryptCnt === 0, 'SECURED_DEVICE', 'Secure boot, flash encryption or secure download mode is enabled; this platform will not write the device.');
        const detected = await loader.detectFlashSize(); guard();
        const flashBytes = loader.flashSizeBytes(detected || 'unknown');
        check(flashBytes === plan.receipt.flashBytes, 'FLASH_SIZE_MISMATCH', 'Detected flash capacity does not match the selected product.', { detectedBytes: flashBytes, expectedBytes: plan.receipt.flashBytes });
        validateFlashLayout(plan.files, plan.product, {partitionTableOffset:plan.receipt.partitionTableOffset});
        const totalBytes = plan.files.reduce((sum, file) => sum + file.data.length, 0);
        setState('writing', plan.receipt.format==='merged' ? '正在写入完整 merged 镜像并校验 MD5。' : '正在按分区偏移逐段写入并校验 MD5；仅擦除对应写入范围。', { flashBytes, receipt: plan.receipt });
        guard(); beganWrite = true;
        await loader.writeFlash({ fileArray: plan.files, flashMode: 'keep', flashFreq: 'keep', flashSize: 'keep', eraseAll: false, compress: true, calculateMD5Hash: md5,
          reportProgress(index, written, total) {
            guard();
            const completed = plan.files.slice(0, index).reduce((sum, file) => sum + file.data.length, 0), fraction = total ? Math.min(1, written / total) : 0;
            const writtenBytes = Math.round(completed + plan.files[index].data.length * fraction);
            setState('writing', '正在写入并校验；取消会在当前协议块结束后生效。', { progress: writtenBytes / totalBytes, writtenBytes });
          } });
        verified=true;guard(); setState('resetting', '写入与校验完成，正在复位设备。',{flashVerified:true});
        await resetFlashedDevice(loader,port);
        setState('closing', '固件已写入并通过 MD5 校验，正在关闭串口。', { progress: 1, writtenBytes: totalBytes });
      } catch (error) {
        const stopped = wantsCancel || error.name === 'NotFoundError';
        const failure = stopped ? cancelled() : error;
        setState(stopped ? 'cancelled' : 'error', stopped ? verified ? '固件已写入并通过校验，复位操作已取消。' : beganWrite ? '已停止；Flash 可能只有部分内容，请重新进入下载模式刷写。' : '已取消，未写入 Flash。' : error.message, { error: { code: failure.code || 'FLASH_FAILED', message: failure.message, details: failure.details }, mayBePartial: beganWrite && !verified,flashVerified:verified });
        throw failure;
      } finally {
        // Do not reset after an interrupted or failed write. Close only this chosen port.
        let closeError;
        if(transport) {
          const closingPort=port,closingTransport=transport;
          const closing=Promise.resolve().then(()=>closingTransport.disconnect()).then(()=>{if(ownsPort)occupiedPorts.delete(closingPort);});
          try {await waitForFlashClose(closing);} catch(error) {closeError=error;emit({type:'log',message:`串口关闭失败：${error.message}`});}
        } else if(ownsPort)occupiedPorts.delete(port);
        plan = null; transport = null; port = null; aborter = null; active = false;
        if(closeError) {
          const message=verified?'固件已写入并通过校验，但串口关闭失败；请断开设备并刷新页面后重试。':`${state.message} 串口关闭失败，请断开设备并刷新页面后重试。`;
          const failure=problem('SERIAL_CLOSE_FAILED',message,{flashVerified:verified,mayBePartial:beganWrite && !verified});
          setState('error',message,{error:{code:failure.code,message,details:failure.details},mayBePartial:beganWrite && !verified,flashVerified:verified});
          throw failure;
        }
        if(state.status==='closing')setState('done','固件已写入并通过 MD5 校验。');
      }
      return clone(state);
    },
  };
}
