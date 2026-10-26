import {isPressControl,controlInput,shouldCaptureMappedKey} from './control-input.js';
import {loadCatalog, createDefaultScene, validateScene, encodeScene, decodeScene, createLocalAccessory, importLocalAccessory, exportLocalAccessory, saveLocalAccessory, listLocalAccessories, removeLocalAccessory, firmwareSourceMatches} from './catalog.js';
import {createPlatformRuntime, validateMergedImage} from './runtime.js';
import {inspectLocalBuild} from './local-firmware.js';
import {installAutomation} from './automation.js';
import {createSceneView, createHumanButtonController} from './scene-view.js';
import {installEmbedBridge} from './embed.js';
import {createTransportUi, createKnowledgePanel, mountFirmwareUpdates} from './transport-ui.js';
import {encodeRecording, decodeRecording, validateRecording} from './recording.js';
import {buildFirmwareIssueUrl} from './watchlist.js';
import {SHELL_DEFAULT, SHELL_PRESETS, SHELL_FINISHES, shellPreset} from './appearance.js';
import {renderHintList, productHints, firmwareHints} from './hints.js';
import {lookupIpLocation, IP_LOCATION_PROVIDER} from './ip-geolocation.js';

const $ = id => document.getElementById(id);
const clone = value => structuredClone(value);
const embedded = document.body.dataset.mode === 'embed';
let catalog, scene, runtime, sceneView, humanButtons, api, detachBridge, localDefinitions = [], status = 'idle', firmwareTab = 'library';
let busy = false, frameCount = 0, renderedProduct = '', renderedScene = '', recordingControl = null, toastTimer, localFile = null, editingAccessoryId = null, uiConfiguring = false;
let transportUi, knowledgePanel, firmwareUpdates, recordingDocument = null, recordingState = {status: 'idle'};
let downloadView = null, loadProgress = null, runAfterLoad = false;
let orientationSettlement = null;
let localFiles = [], localSource = null, localSelectionToken = 0;
const gpsLookups = new Map(), gpsMessages = new Map();
const heldKeys = new Map();
const consoleBuffers = {usb: '', uart0: '', uart1: ''};
const statuses = {idle: '未启动', configured: '配置就绪', loading: '加载中', ready: '固件就绪', running: '运行中', paused: '已暂停', error: '运行失败', disposed: '已结束'};
const errorMessages = {
  MODEL_UNAVAILABLE: '此产品的板级模型尚未实现，可配置和预览外观。',
  UNSUPPORTED_PRODUCT: '此产品的板级模型尚未实现，可配置和预览外观。',
  NO_FIRMWARE: '请先选择固件。', FIRMWARE_MISSING: '请先选择固件。',
  NO_FRAME: '固件尚未输出真实屏幕帧。',
  FETCH_FAILED: '固件下载失败，请检查地址与远程服务器的 CORS 设置。',
  INCOMPLETE_IMAGE: '需要从地址 0 开始的完整 Flash 镜像。单独 app.bin 请改用包含启动镜像的清单。',
  HASH_MISMATCH: '下载内容与 SHA-256 校验值不一致。',
};

function text(id, value) { const node = $(id); if (node) node.textContent = value; }
function bind(id, event, handler) { $(id)?.addEventListener(event, handler); }
function product() { return catalog?.products.find(item => item.id === scene?.productId); }
function definition(id) { return scene?.localAccessories?.find(item => item.id === id) || localDefinitions.find(item => item.id === id) || catalog.accessories.find(item => item.id === id); }
function element(tag, className, content) { const node = document.createElement(tag); if (className) node.className = className; if (content !== undefined) node.textContent = content; return node; }
function toast(message, isError = false) {
  const node = $('toast'); clearTimeout(toastTimer); node.textContent = message; node.classList.toggle('error', isError); node.hidden = false;
  toastTimer = setTimeout(() => { node.hidden = true; }, isError ? 7000 : 3500);
}
function appendConsole(message) {
  const output = $('serial-output'); if (!output) return;
  const atBottom = output.scrollTop + output.clientHeight >= output.scrollHeight - 30;
  output.textContent = (output.textContent + message).slice(-65536);
  if (atBottom) output.scrollTop = output.scrollHeight;
}
function log(message) { for (const port of Object.keys(consoleBuffers)) consoleBuffers[port] = (consoleBuffers[port] + message).slice(-65536); appendConsole(message); }
function report(error) {
  const message = errorMessages[error.code] || error.message || String(error);
  toast(message, true); text('load-status', message);
  showFirmwareIssue(error);
  log(`\n[${error.code || 'ERROR'}] ${error.message || message}\n`);
}
function showFirmwareIssue(error) {
  const issueLink = $('firmware-issue');
  if (!issueLink || !['HTTP_ERROR', 'FETCH_FAILED', 'FETCH_ABORTED'].includes(error.code)) return;
  const url = error.details?.url || $('firmware-url')?.value.trim() || scene?.firmware?.manifestUrl || scene?.firmware?.url;
  if (!url) return;
  issueLink.href = buildFirmwareIssueUrl({entryId: 'custom-source', url, reason: error.message, status: error.details?.status, productId: scene?.productId});
  issueLink.textContent = [404, 410].includes(error.details?.status) ? '远程资源失效：打开 issue 草稿 ↗' : '网络或 CORS 无法确认：打开 issue 草稿 ↗';
  issueLink.hidden = false;
}
async function action(work) {
  if (busy) return;
  busy = true; renderTransport();
  try { return await work(); } catch (error) { report(error); }
  finally { busy = false; renderTransport(); }
}
function download(name, value, type = 'application/json') {
  const blob = value instanceof Blob ? value : new Blob([value], {type});
  const url = URL.createObjectURL(blob), link = element('a'); link.href = url; link.download = name; link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
async function copy(value, confirmation) {
  try { await navigator.clipboard.writeText(value); toast(confirmation); }
  catch {
    const area = element('textarea'); area.value = value; area.style.position = 'fixed'; area.style.opacity = '0'; document.body.append(area); area.select();
    const copied = document.execCommand('copy'); area.remove(); if (copied) toast(confirmation); else throw new Error('浏览器不允许复制，请导出场景 JSON。');
  }
}

function formatDownloadBytes(value) {
  if (value < 1024) return `${Math.round(value)} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}
function renderDownloadProgress() {
  const active = status === 'loading' || (runAfterLoad && status === 'ready');
  if (!downloadView && active) {
    const region = element('div', 'download-progress'); region.id = 'firmware-progress';
    const label = element('span', 'download-progress-label'), value = element('span', 'download-progress-value'), bar = element('progress');
    label.setAttribute('aria-live', 'polite'); label.setAttribute('aria-atomic', 'true'); bar.max = 1; bar.setAttribute('aria-label', '固件下载进度');
    region.append(label, value, bar); downloadView = {region, label, value, bar};
    if ($('load-status')) $('load-status').before(region);
    else document.querySelector('.workspace > .transport')?.before(region);
  }
  if (!downloadView) return;
  const {region, label, value, bar} = downloadView; region.hidden = !active; if (!active) return;
  let message = '正在准备下载', detail = '', fraction = null;
  if (status === 'ready' && runAfterLoad) message = '正在启动设备';
  else if (loadProgress?.phase === 'cache') {
    message = loadProgress.storage === 'memory' ? '使用本页缓存' : '使用浏览器缓存';
    detail = Number.isFinite(loadProgress.loadedBytes) ? formatDownloadBytes(loadProgress.loadedBytes) : '';
  } else if (loadProgress?.phase === 'download') {
    const names = {manifest: '固件清单', flash: '固件镜像', rom: '启动 ROM', wasm: '模拟引擎', bootloader: '启动镜像', ptable: '分区表', app: '应用镜像', flash_at: '固件分段'};
    message = `正在下载${names[loadProgress.kind] || '设备资源'}`;
    const loaded = loadProgress.loadedBytes, total = loadProgress.totalBytes;
    if (Number.isFinite(loaded) && Number.isFinite(total) && total > 0) {
      fraction = Math.max(0, Math.min(1, loaded / total));
      detail = `${Math.floor(fraction * 100)}% · ${formatDownloadBytes(loaded)} / ${formatDownloadBytes(total)}`;
    } else if (Number.isFinite(loaded)) detail = `${formatDownloadBytes(loaded)} · 总大小未知`;
  } else if (loadProgress?.phase === 'prepare') {
    message = {verify: '正在校验文件', local: '正在读取本地固件', worker: '正在准备设备'}[loadProgress.stage] || '正在准备设备';
  }
  if (label.textContent !== message) label.textContent = message;
  value.textContent = detail;
  if (fraction === null) bar.removeAttribute('value'); else bar.value = fraction;
}
async function downloadAndRun() {
  runAfterLoad = true;
  try { await loadSelected(); await runtime.start(); }
  finally { runAfterLoad = false; renderDownloadProgress(); }
}
function clearLocalFiles() { localSelectionToken++; localFiles=[];localSource=null;localFile=null;for(const id of ['firmware-file','firmware-folder'])if($(id))$(id).value='';text('local-file-name','merged .bin、应用 .bin 或 flasher_args.json');text('local-build-status','完整镜像可直接加载；应用镜像需要 flasher_args.json 和其中列出的文件。');renderTransport(); }
async function selectLocalFiles(incoming, {replace=false}={}) {
  const token=++localSelectionToken, accepted=[...incoming].filter(file=>/\.bin$/i.test(file.name)||file.name==='flasher_args.json');
  const seen=new Set();for(const file of accepted){const path=file.webkitRelativePath||file.name;if(seen.has(path)){toast('所选文件存在重名，请选择整个构建目录以保留路径。',true);return;}seen.add(path);}
  const existing=new Map((replace?[]:localFiles).map(file=>[file.webkitRelativePath||file.name,file]));for(const file of accepted)existing.set(file.webkitRelativePath||file.name,file);
  localFiles=[...existing.values()];localFile=localFiles[0]||null;localSource=null;
  text('local-file-name',localFiles.length?`已选择 ${localFiles.length} 个文件`:'未选择固件文件');text('local-build-status','正在检查本地文件…');renderTransport();
  try {
    if(!localFiles.length)throw Error('请选择 .bin 或 flasher_args.json 文件。');
    if(localFiles.some(file=>file.name==='flasher_args.json')) {
      const layout=await inspectLocalBuild(localFiles);if(token!==localSelectionToken)return;
      if(layout.missingFiles.length){text('local-build-status',`还需要：${layout.missingFiles.join('、')}。可继续选择文件，或直接选择整个构建目录。`);return;}
      localSource={format:'build',files:[...localFiles]};text('local-build-status',`${layout.chip || '芯片待校验'} · ${layout.segments.length} 个镜像：${layout.segments.map(s=>`0x${s.offset.toString(16)} ${s.file.name}`).join('；')}。启动前还会校验芯片、分区及完整性。`);
    } else if(localFiles.length===1) {
      const file=localFiles[0];if(file.size>32*1024*1024)throw Error('固件文件不能超过32 MiB。');const data=new Uint8Array(await file.arrayBuffer());if(token!==localSelectionToken)return;
      try {validateMergedImage(data,product().runtime.board || product().chip,(product().runtime.flashMb || 32)*1024*1024);localSource={format:'merged',file};text('local-build-status',`已识别完整合并镜像：${file.name}，可直接加载。`);}
      catch(error){if(error.code!=='INCOMPLETE_IMAGE')throw error;text('local-build-status',`这不是完整合并镜像。请继续选择 flasher_args.json，再补齐其中列出的启动镜像和分区表；也可直接选择构建目录。`);}
    } else text('local-build-status','已选择多个镜像，请补选对应的 flasher_args.json，以读取实际烧录偏移。');
  } catch(error){if(token===localSelectionToken){text('local-build-status',error.message);localSource=null;}}
  finally {if(token===localSelectionToken)renderTransport();}
}

function onRuntimeEvent(event) {
  if (event.type === 'state') {
    const previousStatus = status; status = event.state.status;
    if (['configured', 'loading', 'error', 'disposed'].includes(status)) humanButtons?.invalidate();
    else if (status === 'paused') humanButtons?.suspend();
    else if (status === 'running' && previousStatus === 'paused') void humanButtons?.releaseAll();
    if (event.state.scene) {
      const previousFirmware = JSON.stringify(scene?.firmware);
      scene = clone(event.state.scene);
      const signature = JSON.stringify(scene);
      if (signature !== renderedScene) { renderedScene = signature; renderScene(); }
      if (previousFirmware !== JSON.stringify(scene.firmware)) reflectFirmwareSource({preserveLocal: uiConfiguring});
    }
    if (status === 'loading' && previousStatus !== 'loading') loadProgress = null;
    if (['configured', 'loading'].includes(status) && status !== previousStatus) {
      frameCount = 0; sceneView?.clearDisplay(); text('frame-count', '0 帧');
    }
    if (['configured', 'loading', 'error', 'disposed'].includes(status)) text('runtime-speed', '');
    renderTransport(); renderEmpty(); renderDownloadProgress();
    if (status === 'ready') text('load-status', '固件已加载并校验，可以启动。');
    else if (status === 'running') text('load-status', '固件正在本地 WebAssembly 中执行。');
    else if (status === 'loading') text('load-status', '正在读取固件并初始化 WebAssembly…');
  } else if (event.type === 'imu') {
    if (event.imu?.source === 'agent') sceneView?.setOrientation?.(event.imu.quaternion, {notify: false});
  } else if (event.type === 'load-progress') {
    if (status === 'loading' && (!loadProgress || event.loadId >= loadProgress.loadId)) { loadProgress = event; renderDownloadProgress(); }
  } else if (event.type === 'frame') {
    frameCount = event.frameNumber || frameCount + 1; sceneView?.updateFrame(event);
    text('frame-count', `${frameCount} 帧 · ${event.width} × ${event.height}`); renderEmpty();
    if ($('capture-frame')) $('capture-frame').disabled = false;
  } else if (event.type === 'serial') {
    if (Object.hasOwn(consoleBuffers, event.source)) {
      consoleBuffers[event.source] = (consoleBuffers[event.source] + event.data).slice(-65536);
      if (event.source === ($('serial-port')?.value || 'uart0')) appendConsole(event.data);
    } else log(event.data);
  }
  else if (event.type === 'error') { text('load-status', errorMessages[event.error.code] || event.error.message); showFirmwareIssue(event.error); }
  else if (event.type === 'stats') {
    text('runtime-speed', event.stats.mips ? `${event.stats.mips.toFixed(1)} MIPS${event.stats.speed == null ? '' : ` · ${Math.round(event.stats.speed * 100)}% 实时`}` : '');
  }
}

function renderTransport() {
  const executable = Boolean(product()?.runtime.board);
  const canLoad = Boolean(scene?.firmware || localSource);
  const sessionBusy = ['preparing', 'recording', 'playing'].includes(recordingState.status);
  if ($('start-runtime')) {
    $('start-runtime').disabled = busy || sessionBusy || !executable || status === 'running' || status === 'loading' || !(['ready', 'paused'].includes(status) || canLoad);
    $('start-runtime').textContent = status === 'paused' ? '▶ 继续' : status === 'loading' ? '加载中…' : '▶ 启动';
  }
  if ($('pause-runtime')) $('pause-runtime').disabled = busy || sessionBusy || status !== 'running';
  if ($('reset-runtime')) $('reset-runtime').disabled = busy || sessionBusy || !['running', 'paused', 'ready'].includes(status);
  if ($('load-firmware')) $('load-firmware').disabled = busy || sessionBusy || status === 'loading' || !executable || (firmwareTab === 'library' && !catalog.firmwares.find(item => item.id === $('firmware-select')?.value)?.source);
  if ($('load-firmware')) {
    $('load-firmware').hidden = false;
    $('load-firmware').textContent = firmwareTab === 'local' ? '加载并运行' : '下载并运行';
    if (firmwareTab === 'local' && !localSource) $('load-firmware').disabled = true;
  }
  const boundary = $('firmware-run-boundary');
  if (boundary) {
    const selected = firmwareTab === 'library' && catalog?.firmwares.find(item => item.id === $('firmware-select')?.value);
    const unavailable = !executable || (selected && !selected.source);
    boundary.hidden = !unavailable;
    boundary.replaceChildren();
    if (!executable) {
      boundary.append(element('strong', '', `${product()?.name.replace(/^M5Stack /, '') || '此设备'} 模拟器尚未就绪`), document.createTextNode(`目前仅提供${product()?.model ? '外观与配置预览' : '资料与配置'}，暂不能下载并运行固件。找到固件版本不代表模拟器已支持运行。`));
    } else if (unavailable) {
      boundary.append(element('strong', '', '此固件尚未完成模拟适配'), document.createTextNode('目前仅提供固件资料，暂不能下载并运行。'));
    }
    for (const id of ['load-firmware', 'start-runtime']) {
      if (unavailable) $(id)?.setAttribute('aria-describedby', boundary.id);
      else $(id)?.removeAttribute('aria-describedby');
    }
  }
  if ($('capture-frame')) $('capture-frame').disabled = frameCount === 0;
  if ($('clear-firmware-cache')) $('clear-firmware-cache').disabled = busy || status === 'loading';
  document.querySelectorAll('[data-gps-action]').forEach(button => { button.disabled = busy; });
  const badge = $('runtime-badge');
  if (badge) { badge.className = `runtime-badge ${status}`; badge.replaceChildren(element('span'), document.createTextNode(statuses[status] || status)); }
  document.querySelectorAll('.device-button').forEach(button => { button.disabled = status !== 'running'; });
  transportUi?.refresh();
}
function renderEmpty() {
  if (!$('screen-empty') || !product()) return;
  $('screen-empty').hidden = frameCount > 0;
  if (product().runtime.status === 'unavailable') {
    text('empty-title', `${product().name.replace(/^M5Stack /,'')} · ${product().model?'外观与配置预览':'资料与配置'}`);
    text('empty-description', product().model?'此设备的板级模型待实现，当前不能运行固件。':'暂无已核验的 3D 模型与板级模拟器，当前提供产品资料。');
  } else if (!product().display) {
    text('empty-title', status === 'running' ? '固件运行中' : '设备已准备');
    text('empty-description', '此设备没有屏幕，请在下方串口查看固件输出。');
  } else if (status === 'loading') {
    text('empty-title', '正在准备设备'); text('empty-description', runAfterLoad ? '下载校验完成后，设备会自动启动。' : '正在准备所选固件，请稍候。');
  } else if (status === 'running') {
    text('empty-title', '固件运行中，等待屏幕帧'); text('empty-description', '屏幕仅显示模拟器的实际输出。更多状态请查看串口。');
  } else {
    const selected = catalog.firmwares.find(item => firmwareSourceMatches(item.source, scene.firmware));
    text('empty-title', '按下启动，让固件点亮屏幕');
    text('empty-description', selected?.id === 'm5stack-stopwatch-official' ? '官方固件：同时长按 A+B 进入菜单，A/B 切换，点击图标进入。' : '加载所选固件；操作行为以该固件的说明为准。');
  }
}
function renderProduct() {
  const selected = product(); if (!selected) return;
  text('stage-title', selected.name); text('product-name', selected.name.replace(/^M5Stack /, ''));
  text('model-origin', selected.model?.type === 'source-derived' ? '照片与尺寸建模 · 内部结构有未知项' : selected.model || selected.id === 'm5stack-stopwatch' ? '官方 CAD · 外观来源可查' : '模型待建 · 来源资料可查');
  text('product-model', `${selected.manufacturer} / ${selected.sku}`);
  text('product-symbol', selected.display?.shape === 'round' ? '◉' : selected.display ? '▤' : '▧');
  if ($('product-select')) $('product-select').value = selected.id;
  const specs = $('product-specs');
  if (specs) {
    specs.replaceChildren();
    for (const [label, value] of [['芯片', selected.module || selected.chip.toUpperCase()], ['存储', `${selected.runtime.flashMb == null ? 'Flash 待核对' : `${selected.runtime.flashMb} MB Flash`} / ${selected.runtime.psramMb == null ? 'PSRAM 待核对' : `${selected.runtime.psramMb} MB PSRAM`}`], ['屏幕', selected.display ? `${selected.display.width} × ${selected.display.height} ${selected.display.type || selected.display.technology || selected.display.controller || ''}`.trim() : '串口输出 / 无屏幕'], ['接口', selected.interfaces.filter(item => item.external).map(item => item.name || item.type).join(' · ') || '详见产品元信息']]) {
      specs.append(element('dt', '', label), element('dd', '', value));
    }
  }
  const compatibility = $('compatibility');
  if (compatibility) renderHintList(compatibility, productHints(selected));
  const source = selected.sources?.[0];
  if ($('product-source')) { $('product-source').hidden = !source; if (source) $('product-source').href = source.url; }
  if (renderedProduct !== selected.id) {
    renderedProduct = selected.id; sceneView?.setProduct(selected); frameCount = 0; text('frame-count', '0 帧'); renderFirmwareLibrary();
    void knowledgePanel?.show(selected);
  }
  renderControls(); renderEmpty();
}
function setFirmwareTab(tab) {
  firmwareTab = tab;
  document.querySelectorAll('[data-firmware-tab]').forEach(button => { const active = button.dataset.firmwareTab === tab; button.classList.toggle('active', active); button.setAttribute('aria-pressed', String(active)); });
  for (const name of ['library', 'remote', 'local']) if ($(`firmware-${name}`)) $(`firmware-${name}`).hidden = name !== tab;
  renderTransport();
}
function renderFirmwareLibrary() {
  const select = $('firmware-select'); if (!select) return;
  select.replaceChildren(new Option('选择适配固件', ''));
  for (const firmware of catalog.firmwares.filter(item => item.productIds.includes(scene.productId))) {
    select.append(new Option(`${firmware.name} · ${firmware.status === 'available' && firmware.source ? '可运行' : '待验证'}`, firmware.id));
  }
  const current = catalog.firmwares.find(item => firmwareSourceMatches(item.source, scene.firmware));
  select.value = current?.id || '';
  renderFirmwareNote(current);
}
function renderFirmwareNote(firmware) {
  const node = $('firmware-note'); if (!node) return;
  renderHintList(node, firmwareHints(firmware));
  firmwareUpdates?.show(firmware);
}
function reflectFirmwareSource({preserveLocal = false} = {}) {
  if (!scene.firmware) {
    if ($('firmware-select')) $('firmware-select').value = '';
    if ($('firmware-url')) $('firmware-url').value = '';
    if ($('firmware-sha')) $('firmware-sha').value = '';
    renderFirmwareNote(null);
    if (preserveLocal && localFile && firmwareTab === 'local') return;
    clearLocalFiles();
    if ($('firmware-file')) $('firmware-file').value = '';
    text('local-file-name', 'merged .bin · 文件留在本机'); setFirmwareTab('library'); return;
  }
  const known = catalog.firmwares.find(item => firmwareSourceMatches(item.source, scene.firmware));
  if (known && $('firmware-select')) { $('firmware-select').value = known.id; renderFirmwareNote(known); setFirmwareTab('library'); }
  else if ($('firmware-url')) {
    $('firmware-url').value = scene.firmware.manifestUrl || scene.firmware.url || '';
    $('firmware-kind').value = scene.firmware.manifestUrl ? 'manifest' : 'merged'; $('firmware-sha').value = scene.firmware.sha256 || ''; setFirmwareTab('remote');
  }
}
function sourceFromInputs() {
  if (embedded) return scene.firmware;
  if (firmwareTab === 'local') {
    if (!localSource) throw new Error('请补齐本地镜像与 flasher_args.json 所列文件。');
    return localSource;
  }
  if (firmwareTab === 'remote') {
    const url = $('firmware-url').value.trim(); if (!url) throw new Error('请输入固件地址。');
    const result = {[ $('firmware-kind').value === 'manifest' ? 'manifestUrl' : 'url' ]: url};
    const hash = $('firmware-sha').value.trim(); if (hash) result.sha256 = hash; return result;
  }
  const selected = catalog.firmwares.find(item => item.id === $('firmware-select')?.value);
  if (!selected?.source) throw new Error('请选择可加载固件；“资料入口”尚不包含已验证的镜像。');
  const source = clone(selected.source);
  if (firmwareSourceMatches(scene.firmware, source) && scene.firmware.sha256) source.sha256 = scene.firmware.sha256;
  return source;
}
function renderControls() {
  const controls = product()?.controls.filter(isPressControl) || [];
  const bindings = $('key-bindings'), buttons = $('device-buttons'); bindings?.replaceChildren(); buttons?.replaceChildren();
  const keyboardOpen = $('device-keyboard')?.open || false; $('device-keyboard')?.remove();
  let matrixBindings=null,matrixButtons=null;
  if(controls.some(control=>control.type==='matrix-key')) {
    if(bindings){const details=element('details','matrix-key-bindings'),summary=element('summary','','键盘映射');matrixBindings=element('div','matrix-bindings-grid');details.append(summary,matrixBindings);bindings.append(details);}
    if(buttons){const details=element('details','device-keyboard');details.id='device-keyboard';details.open=keyboardOpen;details.append(element('summary','','屏幕键盘'));matrixButtons=element('div','device-matrix');details.append(matrixButtons);buttons.closest('.transport').after(details);}
  }
  if (!controls.length && bindings) bindings.append(element('span', 'helper', '此设备没有可映射的按键。'));
  for (const control of controls) {
    const code = Object.keys(scene.keyMapping).find(key => scene.keyMapping[key] === control.id);
    if (bindings) {
      const row = element('div', 'key-binding'); row.append(element('span', '', control.type==='matrix-key'?control.name:`${control.name} · GPIO ${control.pin}`));
      const binding = element('button', '', code ? code.replace(/^Key/, '') : '未映射'); binding.type = 'button'; binding.setAttribute('aria-label', `重新映射 ${control.name} 的键盘按键`);
      binding.addEventListener('click', () => {
        recordingControl = control.id; document.querySelectorAll('.key-binding button').forEach(node => node.classList.remove('recording'));
        binding.classList.add('recording'); binding.textContent = '请按键';
      }); row.append(binding); (control.type==='matrix-key'?matrixBindings:bindings).append(row);
    }
    if (buttons) {
      const button = element('button', 'device-button'); button.dataset.control = control.id; button.setAttribute('aria-label', `按住 ${control.name}`);
      const label=control.type==='matrix-key'?control.name.split(' / ')[0]:control.name.replace('Button ', '按键 ');
      button.append(document.createTextNode(({Backspace:'⌫',Enter:'↵',Space:'␣',Shift:'⇧'})[label]||label));button.title=control.name+(code?` · ${code}`:''); if (code) button.append(element('kbd', '', code.replace(/^Key/, '')));
      button.addEventListener('pointerdown', event => { if (event.button !== 0) return; event.preventDefault(); button.setPointerCapture(event.pointerId); sendInput({type: 'button', id: control.id, pressed: true}, `pointer:${event.pointerId}`); });
      for (const type of ['pointerup', 'pointercancel', 'lostpointercapture']) button.addEventListener(type, event => sendInput({type: 'button', id: control.id, pressed: false}, `pointer:${event.pointerId}`));
      button.addEventListener('click', event => { if (event.detail === 0) { void sendInput({type: 'button', id: control.id, pressed: true}, 'activation'); void sendInput({type: 'button', id: control.id, pressed: false}, 'activation'); } }); if(control.type==='matrix-key'){button.classList.add('matrix-key');button.style.gridRow=control.row+1;button.style.gridColumn=control.column+1;matrixButtons.append(button);}else buttons.append(button);
    }
  }
}
async function sendInput(event, source = 'scene') {
  await transportUi?.beforeHumanInput();
  if (status !== 'running') return;
  if (event.type === 'button') return humanButtons.input(event, source);
  try { await runtime.input(event); } catch (error) { report(error); }
}
async function sendOrientation({quaternion, rotating}) {
  if (!runtime || product()?.runtime.board !== 'stopwatch') return;
  const productId = scene.productId;
  try {
    // Pose changes are not part of button/touch recordings. Initial model setup is harmless.
    if (rotating && ['preparing', 'playing', 'recording'].includes(recordingState.status)) orientationSettlement ||= transportUi.settleForConfiguration().finally(() => { orientationSettlement = null; });
    if (orientationSettlement) await orientationSettlement;
    if (scene.productId === productId) await runtime.setImu({quaternion, source: 'scene', angularVelocityDps: rotating ? undefined : [0, 0, 0]});
  } catch (error) { if (error.code !== 'SESSION_REPLACED') report(error); }
}
function renderValidation() {
  const node = $('validation-notes'); if (!node) return;
  const validation = validateScene(scene, catalog); node.replaceChildren();
  const labels = {LOGICAL_UART: '逻辑 UART 输入不验证 GPIO 接线、波特率时序或外部供电。', PHYSICAL_WIRING_UNSUPPORTED: '物理接线已记录；当前适配器不会执行这条电气连接。', ACCESSORY_MODEL_UNAVAILABLE: 'DIY 配件仅保存设计，尚无可执行的外设模型。'};
  for (const warning of validation.warnings.filter(item => labels[item.code])) node.append(element('p', '', labels[warning.code]));
  for (const error of validation.errors) node.append(element('p', '', `${error.path}: ${error.message}`));
}
function renderScene() {
  if (!scene || !catalog) return;
  renderProduct(); renderAccessories(); renderValidation(); renderTransport();
  sceneView?.setMode(scene.view.deviceView);
  sceneView?.setAppearance(scene.view.appearance);
  renderAppearance();
  if ($('edit-scene')) $('edit-scene').href = shareUrl('studio', {strictRecording: false});
}
function renderAppearance() {
  if ($('shell-settings')) $('shell-settings').hidden = scene.view.deviceView !== '3d' || sceneView?.hasShell === false || (!product()?.display && !product()?.model);
  const shell = scene.view.appearance?.shell || sceneView?.getAppearance()?.shell || SHELL_DEFAULT;
  if ($('shell-preset')) $('shell-preset').value = scene.view.appearance ? shellPreset(shell) : 'original';
  if ($('shell-color')) $('shell-color').value = shell.color;
  if ($('shell-opacity')) $('shell-opacity').value = Math.round(shell.opacity * 100);
  if ($('shell-finish')) $('shell-finish').value = shell.finish;
  text('shell-opacity-value', `${Math.round(shell.opacity * 100)}%`);
}
function appearanceFromInputs() {
  return {shell: {color: $('shell-color').value, opacity: Number($('shell-opacity').value) / 100, finish: $('shell-finish').value}};
}
function updateAppearance(appearance) {
  const view = clone(scene.view);
  if (appearance) view.appearance = appearance; else delete view.appearance;
  return runtime.updateSceneMetadata({view});
}
function mountAppearance() {
  const controls = document.querySelector('.view-controls'); if (!controls) return;
  const details = element('details', 'shell-settings'); details.id = 'shell-settings';
  details.innerHTML = `<summary>外壳</summary><div class="shell-settings-body">
    <label for="shell-preset">展示预设</label><select id="shell-preset"><option value="original">原色</option><option value="black-clear">黑透</option><option value="purple-clear">紫透</option><option value="gold">金色</option><option value="custom" disabled>自定义</option></select>
    <label for="shell-color">外壳颜色</label><input id="shell-color" type="color" value="#080a0b">
    <label for="shell-opacity">不透明度 <output id="shell-opacity-value" for="shell-opacity">100%</output></label><input id="shell-opacity" type="range" min="0" max="100" step="1" value="100">
    <label for="shell-finish">表面效果</label><select id="shell-finish"></select>
    <p class="helper">自定义展示配色。未知内部结构以 UNKNOWN 占位。</p><button id="reset-shell" class="button quiet tiny" type="button">恢复原色</button>
  </div>`;
  controls.prepend(details);
  $('shell-finish').replaceChildren(...SHELL_FINISHES.map(({id, label}) => new Option(label, id)));
  bind('shell-preset', 'change', () => action(() => {
    const key = $('shell-preset').value;
    return updateAppearance(key === 'original' ? null : {shell: clone(SHELL_PRESETS[key])});
  }));
  for (const id of ['shell-color', 'shell-opacity', 'shell-finish']) {
    bind(id, 'input', () => {
      const appearance = appearanceFromInputs(); sceneView?.setAppearance(appearance);
      text('shell-opacity-value', `${Math.round(appearance.shell.opacity * 100)}%`);
      $('shell-preset').value = shellPreset(appearance.shell);
    });
    bind(id, 'change', () => action(() => updateAppearance(appearanceFromInputs())));
  }
  bind('reset-shell', 'click', () => action(() => updateAppearance(null)));
}
function reflectMode(mode, unavailable = false) {
  for (const candidate of ['2d', '3d']) { const button = $(`view-${candidate}`); if (button) { button.classList.toggle('active', mode === candidate); button.setAttribute('aria-pressed', String(mode === candidate)); } }
  if ($('view-3d')) $('view-3d').disabled = unavailable;
  const touch = product()?.runtime?.board && (product()?.runtime?.touch || product()?.display?.touch);
  text('stage-gesture', mode === '3d' ? `拖动模型旋转 · 滚轮缩放${touch ? ' · 点击或滑动触摸屏' : ''}` : touch ? '点击或滑动触摸屏 · 按键支持键盘映射' : '实际固件画面 · 按键支持键盘映射');
}
async function configure(next, {reflectSource = false} = {}) {
  const validation = validateScene(next, catalog);
  if (!validation.valid) throw Object.assign(new Error(validation.errors.map(item => `${item.path}: ${item.message}`).join('\n')), {code: 'INVALID_SCENE'});
  if ($('firmware-issue')) $('firmware-issue').hidden = true;
  await transportUi?.settleForConfiguration();
  await releaseHumanInputs();
  uiConfiguring = true;
  try { await runtime.configure(next); } finally { uiConfiguring = false; }
  if (reflectSource) reflectFirmwareSource();
  text('load-status', '配置已更新，启动时会重新加载固件。');
}
async function loadSelected() {
  if ($('toast')?.classList.contains('error')) { clearTimeout(toastTimer); $('toast').hidden = true; }
  if ($('firmware-issue')) $('firmware-issue').hidden = true;
  const source = sourceFromInputs();
  const next = clone(scene); next.firmware = source.file || source.files ? null : source;
  await configure(next); await runtime.loadFirmware(source); return source;
}
function defaultFirmware(next) {
  const defaultId = catalog.products.find(item => item.id === next.productId)?.defaultFirmwareId;
  const firmware = defaultId ? catalog.firmwares.find(item => item.id === defaultId && item.productIds.includes(next.productId))
    : catalog.firmwares.find(item => item.productIds.includes(next.productId) && item.status === 'available' && item.source);
  if (firmware) next.firmware = clone(firmware.source); return next;
}

function allDefinitions() {
  const items = new Map([...catalog.accessories, ...localDefinitions, ...(scene.localAccessories || [])].map(item => [item.id, item])); return [...items.values()];
}
function renderAccessories() {
  const selector = $('accessory-select');
  if (selector) { const previous = selector.value; selector.replaceChildren(new Option('选择配件…', '')); for (const item of allDefinitions()) selector.append(new Option(item.name, item.id)); selector.value = previous; }
  text('accessory-count', String(scene.accessories.length));
  sceneView?.setAccessories(scene.accessories.map(item => { const value = definition(item.catalogId); return value && {...value, instanceId: item.id, wiring: item.wiring}; }).filter(Boolean));
  if (embedded && !$('accessory-list')) {
    const details = element('details', 'embed-accessories'); details.id = 'embed-accessories';
    const summary = element('summary', '', '配件与 GPS 数据'), content = element('div'); content.id = 'accessory-list'; details.append(summary, content); $('session-tools-mount').after(details);
  }
  if ($('embed-accessories')) $('embed-accessories').hidden = !scene.accessories.length;
  for (const [id, pending] of gpsLookups) if (!scene.accessories.some(item => item.id === id) || pending.productId !== scene.productId) { pending.aborter.abort(); gpsLookups.delete(id); }
  const list = $('accessory-list'); if (!list) return; list.replaceChildren();
  if (!scene.accessories.length) list.append(element('p', 'empty-list', '连接 GPS、传感器，或设计自己的配件。'));
  for (const instance of scene.accessories) {
    const item = definition(instance.catalogId); if (!item) continue;
    const card = element('article', 'accessory-card');
    const heading = element('div', 'accessory-card-header'), identity = element('div');
    identity.append(element('h3', '', item.name), element('span', 'small-label', item.model.id === 'uart-nmea' ? `${item.controller || 'NMEA'} · ${instance.parameters.baudRate || item.parameters.baudRate} baud · 逻辑 UART1` : '仅元信息 · 行为模型未实现'));
    const remove = element('button', 'icon-button', '×'); remove.setAttribute('aria-label', `移除 ${item.name}`);
    remove.addEventListener('click', () => action(async () => { const next = clone(scene); next.accessories = next.accessories.filter(other => other.id !== instance.id); pruneDefinitions(next); await configure(next); }));
    heading.append(identity, remove); card.append(heading);
    card.append(element('p', 'helper', item.model.id === 'uart-nmea' ? '向 UART1 注入 NMEA 数据；未模拟物理接线、供电或无线信号。' : '此配件可导出和分享；不会自动执行电路或协议行为。'));
    const details = element('details'); details.append(element('summary', '', '接线与参数'));
    const wiring = element('textarea'); wiring.rows = 3; wiring.value = JSON.stringify(instance.wiring, null, 2); wiring.setAttribute('aria-label', `${item.name} 接线 JSON`);
    const parameters = element('textarea'); parameters.rows = 3; parameters.value = JSON.stringify(instance.parameters, null, 2); parameters.setAttribute('aria-label', `${item.name} 参数 JSON`);
    details.append(element('label', '', '接线定义（logical 或 physical）'), wiring, element('label', '', '配件参数'), parameters);
    const save = element('button', 'button', '应用配置');
    save.addEventListener('click', () => action(async () => { const next = clone(scene), current = next.accessories.find(other => other.id === instance.id); current.wiring = JSON.parse(wiring.value); current.parameters = JSON.parse(parameters.value); await configure(next); toast('接线与参数已保存，需重新启动固件。'); }));
    details.append(save); card.append(details);
    if (item.model.id === 'uart-nmea') card.append(createGpsControls(instance, item));
    list.append(card);
  }
}
const GPS_FIELDS = [
  {key: 'latitude', label: '纬度 °', min: -90, max: 90, fallback: 0},
  {key: 'longitude', label: '经度 °', min: -180, max: 180, fallback: 0},
  {key: 'altitude', label: '海拔 m', min: -10000, max: 100000, fallback: 0},
  {key: 'speedKnots', label: '速度 kn', min: 0, max: 10000, fallback: 0},
  {key: 'course', label: '航向 °', min: 0, max: 359.999999, fallback: 0},
  {key: 'satellites', label: '模拟卫星数', min: 1, max: 99, fallback: 8, step: '1'},
];
function gpsFeedback(id, message, error = false) {
  const current = scene.accessories.find(item => item.id === id); if (!current) return;
  gpsMessages.set(id, {message, error, signature: JSON.stringify(current.parameters)});
  for (const node of document.querySelectorAll('[data-gps-feedback]')) if (node.dataset.gpsFeedback === id) { node.textContent = message; node.hidden = !message; node.classList.toggle('error', error); }
}
async function applyGpsFix(id, values) {
  await transportUi?.settleForConfiguration();
  await api.setGpsFix({accessoryId: id, ...values});
  if (status === 'running') {
    const current = scene.accessories.find(item => item.id === id);
    if (product()?.runtime.board === 'stopwatch' && current?.wiring.mode === 'logical' && current?.wiring.port === 'uart1') {
      const result = await api.injectGps({accessoryId: id});
      log(`\n[GPS → ${result.port}] ${result.data}`); gpsFeedback(id, '数值已保存，NMEA 已送入 UART1；响应以固件实际输出为准。'); return;
    }
    gpsFeedback(id, '数值已保存；当前产品或接线尚不支持 NMEA 注入。');
  } else gpsFeedback(id, '数值已保存到场景。启动设备后，再点“应用并注入”发送一帧。');
}
function createGpsControls(instance, item) {
  const root = element('section', 'gps-controls'), title = element('h4', '', 'GPS 数据 · 模拟输入'); root.append(title);
  const fields = element('div', 'gps-fields'), inputs = new Map(), values = {...item.parameters, ...instance.parameters};
  for (const field of GPS_FIELDS) {
    const input = element('input'); input.type = 'number'; input.required = true; input.step = field.step || 'any'; input.min = String(field.min); input.max = String(field.max); input.value = values[field.key] ?? field.fallback;
    input.setAttribute('aria-label', `${item.name} ${field.label}`); input.dataset.gpsField = field.key; inputs.set(field.key, input);
    const label = element('label', '', field.label); label.append(input); fields.append(label);
  }
  function readFields() {
    const result = {};
    for (const [key, input] of inputs) { if (!input.reportValidity() || input.value === '' || !Number.isFinite(Number(input.value))) throw new Error('请填写范围内的 GPS 数值。'); result[key] = Number(input.value); }
    return result;
  }
  const note = element('p', 'helper', '经纬度为 WGS84。海拔、速度和卫星数是可编辑的模拟值。');
  const apply = element('button', 'button wide', '应用并注入一帧'); apply.type = 'button'; apply.dataset.gpsAction = 'apply';
  apply.addEventListener('click', () => action(async () => { try { await applyGpsFix(instance.id, {...readFields(), positionSource: {kind: 'manual', fetchedAt: new Date().toISOString()}}); } catch (error) { gpsFeedback(instance.id, error.message, true); throw error; } }));
  const locate = element('button', 'button quiet wide gps-ip-button', '获取当前 IP 的 GPS（估算）'); locate.type = 'button'; locate.dataset.gpsAction = 'ip';
  locate.addEventListener('click', () => action(async () => {
    const initial = scene.accessories.find(value => value.id === instance.id); if (!initial) return;
    const before = JSON.stringify(initial.parameters), productId = scene.productId, aborter = new AbortController();
    gpsLookups.set(instance.id, {aborter, productId}); locate.disabled = true; locate.textContent = '正在获取城市级估算…'; gpsFeedback(instance.id, '正在查询 IP 位置；手填值仍保留。');
    try {
      const previousValues = readFields(), fix = await lookupIpLocation({signal: aborter.signal});
      const current = scene.accessories.find(value => value.id === instance.id);
      if (scene.productId !== productId || !current || current.catalogId !== initial.catalogId || JSON.stringify(current.parameters) !== before || JSON.stringify(readFields()) !== JSON.stringify(previousValues)) throw new Error('场景或 GPS 参数已变更，请重新获取位置。');
      await applyGpsFix(instance.id, {...previousValues, ...fix});
    } catch (error) { if (error.code !== 'IP_LOCATION_CANCELLED') { gpsFeedback(instance.id, error.message, true); throw error; } }
    finally { if (gpsLookups.get(instance.id)?.aborter === aborter) gpsLookups.delete(instance.id); if (locate.isConnected) { locate.disabled = false; locate.textContent = '获取当前 IP 的 GPS（估算）'; } }
  }));
  const ipNote = element('details', 'gps-ip-note'); ipNote.append(element('summary', '', '城市级估算，非卫星 GPS'));
  ipNote.append(element('p', 'helper', '仅点击时请求 ipwho.is；服务商会看到公网 IP。请求不含固件、场景或手填坐标。IP 位置可能是网络出口，不能代表设备的精确位置；失败时可手填。'));
  const docs = element('a', 'text-link', '定位服务说明 ↗'); docs.href = IP_LOCATION_PROVIDER.documentationUrl; docs.target = '_blank'; docs.rel = 'noopener noreferrer';
  ipNote.append(element('p', 'helper', '免费限额为每日 1000 次，浏览器请求按域共享；无可用性保证。IP 服务不提供海拔、速度、航向或卫星测量。'), docs);
  const source = values.positionSource, provenance = element('div', 'gps-provenance');
  provenance.append(element('p', '', source?.kind === 'ip-estimate' ? `来源：城市级 IP 估算 · ${source.provider || IP_LOCATION_PROVIDER.id}` : '来源：手动输入 / 默认模拟值'));
  if (source?.kind === 'ip-estimate') { const locality = [source.city, source.region, source.country].filter(Boolean); if (locality.length) provenance.append(element('p', '', [...new Set(locality)].join(' · '))); }
  if (source?.fetchedAt) { const time = element('time', '', `获取时间：${new Date(source.fetchedAt).toLocaleString()}`); time.dateTime = source.fetchedAt; time.title = source.fetchedAt; provenance.append(time); }
  const feedback = element('p', 'gps-feedback'); feedback.dataset.gpsFeedback = instance.id; feedback.setAttribute('role', 'status'); feedback.hidden = true;
  const saved = gpsMessages.get(instance.id); if (saved?.signature === JSON.stringify(instance.parameters)) { feedback.textContent = saved.message; feedback.hidden = false; feedback.classList.toggle('error', saved.error); }
  root.append(fields, note, apply, locate, ipNote, provenance, feedback); return root;
}
function pruneDefinitions(next) { if (next.localAccessories) next.localAccessories = next.localAccessories.filter(item => next.accessories.some(instance => instance.catalogId === item.id)); }
async function addAccessory(id) {
  const item = allDefinitions().find(entry => entry.id === id); if (!item) throw new Error('请先选择配件。');
  const next = clone(scene);
  next.accessories.push({id: `accessory-${Date.now().toString(36)}`, catalogId: id, parameters: clone(item.parameters || {}), wiring: {mode: 'logical', port: 'uart1'}});
  if (id.startsWith('local:') && !next.localAccessories?.some(entry => entry.id === id)) { next.localAccessories ||= []; next.localAccessories.push(clone(item)); }
  await configure(next); toast('配件已加入场景，启动时应用配置。');
}
function renderLocalAccessories() {
  const list = $('local-accessories'); if (!list) return; list.replaceChildren();
  for (const item of localDefinitions) {
    const row = element('div', 'local-accessory'); row.append(element('span', '', item.name));
    const add = element('button', '', '加入'); add.setAttribute('aria-label', `加入 ${item.name}`); add.addEventListener('click', () => action(() => addAccessory(item.id)));
    const edit = element('button', '', '编辑'); edit.setAttribute('aria-label', `编辑 ${item.name}`); edit.addEventListener('click', () => {
      editingAccessoryId = item.id; $('diy-name').value = item.name; $('diy-description').value = item.description || '';
      $('diy-protocol').value = item.connector.protocol || 'uart'; $('diy-pins').value = JSON.stringify(item.connector.pins || [], null, 2);
      text('diy-error', ''); $('diy-dialog').showModal();
    });
    const exp = element('button', '', '导出'); exp.setAttribute('aria-label', `导出 ${item.name}`); exp.addEventListener('click', () => download(`${item.id.replace(':', '-')}.json`, exportLocalAccessory(item)));
    const remove = element('button', '', '×'); remove.setAttribute('aria-label', `从本地库删除 ${item.name}`); remove.addEventListener('click', () => action(async () => { removeLocalAccessory(item.id); localDefinitions = listLocalAccessories(); renderLocalAccessories(); renderAccessories(); toast('已从本地库删除；当前场景中的定义仍保留。'); }));
    row.append(add, edit, exp, remove); list.append(row);
  }
}
function makeDiy() {
  const pins = JSON.parse($('diy-pins').value);
  const existing = localDefinitions.find(item => item.id === editingAccessoryId);
  const result = createLocalAccessory({id: editingAccessoryId || `local:accessory-${Date.now().toString(36)}`, name: $('diy-name').value.trim(), description: $('diy-description').value.trim(), modelId: existing?.model.id || 'metadata-only', parameters: existing?.parameters || {}, connector: {type: existing?.connector.type || 'custom', voltage: existing?.connector.voltage ?? 3.3, protocol: $('diy-protocol').value, pins}});
  if (existing) { result.sources = clone(existing.sources); result.limitations = clone(existing.limitations); }
  return result;
}

function portableScene() {
  const next = clone(scene); next.view.mode = embedded ? 'embed' : 'studio';
  if (!embedded) {
    if (firmwareTab === 'local') next.firmware = null;
    else if (firmwareTab === 'remote' && !$('firmware-url').value.trim()) next.firmware = null;
    else if (firmwareTab === 'library' && !catalog.firmwares.find(item => item.id === $('firmware-select').value)?.source) next.firmware = null;
    else next.firmware = sourceFromInputs();
  }
  return next;
}
function recordingForScene(candidate, {strict = true} = {}) {
  if (!recordingDocument) return null;
  if (['preparing', 'recording', 'playing'].includes(recordingState.status)) {
    if (strict) throw new Error('请先停止录制或接管回放，再分享完整操作。');
    return null;
  }
  const validation = validateRecording(recordingDocument, {scene: candidate});
  if (!validation.valid) {
    if (strict) throw new Error('当前设备、固件或配件与录制不一致。请恢复原场景，或清除旧录制后分享。');
    return null;
  }
  return recordingDocument;
}
function shareUrl(mode = 'studio', {strictRecording = true} = {}) {
  const shared = portableScene(); shared.view.mode = mode;
  const url = new URL(mode === 'embed' ? './embed.html' : './index.html', import.meta.url);
  const params = new URLSearchParams({scene: encodeScene(shared)}), recorded = recordingForScene(shared, {strict: strictRecording});
  if (recorded) params.set('recording', encodeRecording(recorded));
  url.hash = params.toString(); return url.href;
}
function validatePortable(next) {
  const validation = validateScene(next, catalog); if (!validation.valid) throw new Error(validation.errors.map(item => item.message).join(' '));
}
async function releaseKeys() {
  const held = [...heldKeys.entries()]; heldKeys.clear();
  for (const [code, id] of held) await sendInput({type: 'button', id, pressed: false}, `key:${code}`);
}
async function releaseHumanInputs() {
  heldKeys.clear(); sceneView?.releaseInputs(); await humanButtons?.releaseAll();
}

function setupEvents() {
  bind('product-select', 'change', () => action(async () => { clearLocalFiles(); const next = defaultFirmware(createDefaultScene(catalog, $('product-select').value)); await configure(next, {reflectSource: true}); if (!next.firmware) { $('firmware-select').value = ''; setFirmwareTab('library'); } }));
  document.querySelectorAll('[data-firmware-tab]').forEach(button => button.addEventListener('click', () => setFirmwareTab(button.dataset.firmwareTab)));
  bind('firmware-select', 'change', () => action(async () => { const selected = catalog.firmwares.find(item => item.id === $('firmware-select').value); const next = clone(scene); next.firmware = selected?.source ? clone(selected.source) : null; await configure(next); $('firmware-select').value = selected?.id || ''; renderFirmwareNote(selected); }));
  bind('firmware-file', 'change', () => { void selectLocalFiles($('firmware-file').files); });
  bind('firmware-folder', 'change', () => { void selectLocalFiles($('firmware-folder').files,{replace:true}); });
  bind('clear-local-files', 'click', clearLocalFiles);
  bind('load-firmware', 'click', () => action(downloadAndRun));
  bind('clear-firmware-cache', 'click', () => action(async () => { await runtime.clearFirmwareCache(); toast('已清除当前浏览器的固件缓存。'); }));
  bind('start-runtime', 'click', () => action(async () => { if (['ready', 'paused'].includes(status)) await runtime.start(); else await downloadAndRun(); }));
  bind('pause-runtime', 'click', () => action(async () => { await releaseHumanInputs(); await runtime.pause(); }));
  bind('reset-runtime', 'click', () => action(async () => { await releaseHumanInputs(); await runtime.reset(); }));
  for (const mode of ['2d', '3d']) bind(`view-${mode}`, 'click', () => action(async () => {
    const actual = sceneView.setMode(mode); const view = {...scene.view, deviceView: actual}; await runtime.updateSceneMetadata({view});
  }));
  bind('reset-view', 'click', () => action(async () => { await transportUi?.settleForConfiguration(); sceneView.resetView(); }));
  bind('fullscreen', 'click', () => action(async () => {
    if (document.fullscreenElement) await document.exitFullscreen(); else if ($('device-stage').closest('.workspace').requestFullscreen) await $('device-stage').closest('.workspace').requestFullscreen(); else window.open(shareUrl('embed'), '_blank', 'noopener');
  }));
  bind('capture-frame', 'click', () => action(async () => { const frame = await runtime.capture({format: 'png'}); const link = element('a'); link.href = frame.dataUrl; link.download = `${scene.productId}-frame-${frame.frameNumber}.png`; link.click(); }));
  bind('clear-console', 'click', () => { for (const port of Object.keys(consoleBuffers)) consoleBuffers[port] = ''; text('serial-output', ''); });
  bind('serial-port', 'change', () => { text('serial-output', consoleBuffers[$('serial-port').value] || ''); $('serial-output').scrollTop = $('serial-output').scrollHeight; });
  bind('serial-form', 'submit', event => { event.preventDefault(); action(async () => { const input = $('serial-input'); if (!input.value) return; await runtime.input({type: 'serial', port: $('serial-port').value, data: input.value + '\n'}); input.value = ''; }); });
  bind('add-accessory', 'click', () => action(() => addAccessory($('accessory-select').value)));
  bind('open-diy', 'click', () => { editingAccessoryId = null; $('diy-form').reset(); text('diy-error', ''); $('diy-dialog').showModal(); });
  bind('close-diy', 'click', () => $('diy-dialog').close());
  bind('diy-form', 'submit', async event => {
    event.preventDefault();
    try {
      const item = makeDiy(); saveLocalAccessory(item); localDefinitions = listLocalAccessories();
      if (scene.localAccessories?.some(entry => entry.id === item.id)) {
        const next = clone(scene); next.localAccessories = next.localAccessories.map(entry => entry.id === item.id ? clone(item) : entry); await configure(next);
      }
      renderLocalAccessories(); renderAccessories(); $('diy-dialog').close(); toast('DIY 配件已保存在本机。');
    } catch (error) { text('diy-error', error.message); }
  });
  bind('export-diy-draft', 'click', () => { try { const item = makeDiy(); download(`${item.id.replace(':', '-')}.json`, exportLocalAccessory(item)); } catch (error) { text('diy-error', error.message); } });
  bind('import-diy', 'change', () => action(async () => { const input = $('import-diy'), file = input.files[0]; if (!file) return; if (file.size > 16384) throw new Error('配件定义不能超过 16 KiB。'); const item = importLocalAccessory(await file.text()); saveLocalAccessory(item); localDefinitions = listLocalAccessories(); renderLocalAccessories(); renderAccessories(); input.value = ''; toast('配件定义已导入本地库。'); }));
  bind('export-scene', 'click', () => action(async () => {
    await transportUi?.syncRecording(); const shared = portableScene(); validatePortable(shared); const recorded = recordingForScene(shared);
    const document = recorded ? {schemaVersion: '1', scene: shared, recording: recorded} : shared;
    download(`${scene.productId}-${recorded ? 'session' : 'scene'}.json`, JSON.stringify(document, null, 2) + '\n');
    if (localFile) toast('配置已导出；本地固件文件需单独提供。');
  }));
  bind('import-scene', 'change', () => action(async () => {
    const input = $('import-scene'), file = input.files[0]; if (!file) return;
    if (file.size > 131072) throw new Error('场景与录制包不能超过 128 KiB。');
    const document = JSON.parse(await file.text()), bundle = document.scene !== undefined;
    if (bundle && (document.schemaVersion !== '1' || Object.keys(document).some(key => !['schemaVersion', 'scene', 'recording'].includes(key)))) throw new Error('录制包格式无效。');
    const imported = bundle ? document.scene : document, recorded = bundle ? document.recording || null : null; validatePortable(imported);
    if (recorded) { const validation = validateRecording(recorded, {scene: imported}); if (!validation.valid) throw new Error(validation.errors.map(item => item.message).join(' ')); }
    clearLocalFiles(); await configure(imported, {reflectSource: true}); transportUi?.setRecording(recorded); input.value = ''; toast(recorded ? '场景与录制已导入，点击回放即可重现操作。' : '场景已导入。');
  }));
  bind('share-scene', 'click', () => action(async () => { await transportUi?.syncRecording(); validatePortable(portableScene()); const url = shareUrl('studio'); await copy(url, localFile ? '分享链接已复制；本地固件需接收者自行选择。' : recordingDocument ? '场景与录制链接已复制；接收者需主动点击回放。' : '场景链接已复制。'); history.replaceState(null, '', url); }));
  bind('copy-embed', 'click', () => action(async () => { await transportUi?.syncRecording(); validatePortable(portableScene()); const url = shareUrl('embed'); await copy(`<iframe src="${url}" title="${product().name.replaceAll('"', '&quot;')} · ESP Player" width="800" height="640" allow="serial; fullscreen" loading="lazy"></iframe>`, 'iframe 代码已复制。'); }));
  bind('open-player', 'click', () => { try { validatePortable(portableScene()); window.open(shareUrl('embed'), '_blank', 'noopener'); } catch (error) { report(error); } });
  bind('keyboard-enabled', 'change', () => { if (!$('keyboard-enabled').checked) releaseKeys(); });
  document.addEventListener('keydown', event => {
    if (recordingControl) {
      event.preventDefault();
      if (!event.code || (/^(Control|Alt|Meta)/.test(event.code)&&product()?.controls.find(item=>item.id===recordingControl)?.type!=='matrix-key')) { toast('请选择普通按键，例如 A、空格或方向键。'); return; }
      const control = recordingControl; recordingControl = null;
      if (event.code === 'Escape') { renderControls(); return; }
      action(async () => { const keyMapping = Object.fromEntries(Object.entries(scene.keyMapping).filter(([, value]) => value !== control)); keyMapping[event.code] = control; try { await runtime.updateSceneMetadata({keyMapping}); } finally { renderControls(); } }); return;
    }
    if (event.repeat || !shouldCaptureMappedKey(event,product(),scene?.keyMapping) || event.target.closest('input,select,textarea,[contenteditable="true"]') || $('diy-dialog')?.open || ($('keyboard-enabled') && !$('keyboard-enabled').checked)) return;
    const id = scene?.keyMapping[event.code]; if (!id || status !== 'running') return;
    event.preventDefault(); heldKeys.set(event.code, id); sendInput({type: 'button', id, pressed: true}, `key:${event.code}`);
  });
  document.addEventListener('keyup', event => { const id = heldKeys.get(event.code); if (!id) return; event.preventDefault(); heldKeys.delete(event.code); sendInput({type: 'button', id, pressed: false}, `key:${event.code}`); });
  window.addEventListener('blur', releaseHumanInputs);
  document.addEventListener('visibilitychange', () => { if (document.hidden) releaseHumanInputs(); });
  window.addEventListener('pagehide', () => { for (const pending of gpsLookups.values()) pending.aborter.abort(); humanButtons?.invalidate(); transportUi?.dispose(); knowledgePanel?.dispose(); firmwareUpdates?.dispose(); detachBridge?.(); sceneView?.dispose(); runtime?.dispose(); }, {once: true});
}

async function init() {
  catalog = await loadCatalog();
  try { localDefinitions = listLocalAccessories(); } catch (error) { report(error); }
  let restored;
  const params = new URLSearchParams(location.hash.slice(1));
  if (params.has('scene')) {
    try { restored = decodeScene(params.get('scene')); validatePortable(restored); }
    catch (error) { restored = null; report(error); log('[SCENE] 无法加载共享配置，已回到默认设备。\n'); }
  }
  scene = restored || defaultFirmware(createDefaultScene(catalog));
  if (params.has('recording')) {
    try {
      const recorded = decodeRecording(params.get('recording')), validation = validateRecording(recorded, {scene});
      if (!validation.valid) throw new Error(validation.errors.map(item => item.message).join(' ')); recordingDocument = recorded;
    } catch (error) { report(error); log('[RECORDING] 共享录制无效，设备仍可手动操作。\n'); }
  }
  sceneView = createSceneView($('scene-mount'), {onInput: sendInput, onModeChange: reflectMode, onUnsupported: message => toast(message), onOrientation: sendOrientation, onModelReady: renderAppearance});
  runtime = createPlatformRuntime({catalog, onEvent: onRuntimeEvent});
  humanButtons = createHumanButtonController({send: event => runtime.input(controlInput(event,product())), isRunning: () => status === 'running', onError: report,
    onVisual: (id, pressed) => document.querySelectorAll('.device-button').forEach(button => { if (button.dataset.control === id) button.classList.toggle('pressed', pressed); }),
  });
  api = installAutomation(runtime, {target: window});
  detachBridge = installEmbedBridge(api);
  knowledgePanel = createKnowledgePanel($('product-knowledge'));
  transportUi = createTransportUi({mount: $('session-tools-mount'), catalog, getScene: () => clone(scene), getSource: sourceFromInputs, getApi: () => api,
    prepareRecording: async () => { await releaseHumanInputs(); if (!['ready', 'running', 'paused'].includes(status)) await loadSelected(); },
    releaseInputs: releaseHumanInputs,
    recordingMatchesScene: recorded => validateRecording(recorded, {scene}).valid,
    onRecordingState: state => { recordingState = state; renderTransport(); },
    onRecordingChange: value => { recordingDocument = value; if ($('edit-scene')) $('edit-scene').href = shareUrl('studio', {strictRecording: false}); },
    onError: report, onLog: log,
  });
  if ($('product-select')) { $('product-select').replaceChildren(...catalog.products.map(item => new Option(item.name, item.id))); $('product-select').disabled = false; }
  text('product-count', `${catalog.products.length} 款`);
  mountAppearance(); setupEvents(); await runtime.configure(scene); renderLocalAccessories(); reflectFirmwareSource(); transportUi.setRecording(recordingDocument);
  firmwareUpdates = await mountFirmwareUpdates($('firmware-updates'), {getProductId: () => scene.productId, onError: report});
  firmwareUpdates.show(catalog.firmwares.find(item => item.id === $('firmware-select')?.value));
  if (embedded) {
    if (product().runtime.status !== 'unavailable') text('empty-description', scene.firmware ? '点击启动，执行此场景中的固件。' : '此场景未指定固件，可点击“编辑”进行配置。');
    // Playback requires an explicit user action; embedding a URL never starts untrusted firmware silently.
  }
}
init().catch(error => { report(error); text('empty-title', '工作台初始化失败'); text('empty-description', error.message); });
