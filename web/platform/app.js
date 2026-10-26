import {loadCatalog, createDefaultScene, validateScene, encodeScene, decodeScene, createLocalAccessory, importLocalAccessory, exportLocalAccessory, saveLocalAccessory, listLocalAccessories, removeLocalAccessory} from './catalog.js';
import {createPlatformRuntime} from './runtime.js';
import {installAutomation} from './automation.js';
import {createSceneView, createHumanButtonController} from './scene-view.js';
import {installEmbedBridge} from './embed.js';
import {createTransportUi, createKnowledgePanel, mountWatchlist} from './transport-ui.js';
import {encodeRecording, decodeRecording, validateRecording} from './recording.js';
import {buildFirmwareIssueUrl} from './watchlist.js';

const $ = id => document.getElementById(id);
const clone = value => structuredClone(value);
const embedded = document.body.dataset.mode === 'embed';
let catalog, scene, runtime, sceneView, humanButtons, api, detachBridge, localDefinitions = [], status = 'idle', firmwareTab = 'library';
let busy = false, frameCount = 0, renderedProduct = '', renderedScene = '', recordingControl = null, toastTimer, localFile = null, editingAccessoryId = null, uiConfiguring = false;
let transportUi, knowledgePanel, removeWatchlist, recordingDocument = null, recordingState = {status: 'idle'};
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
    if (['configured', 'loading'].includes(status) && status !== previousStatus) {
      frameCount = 0; sceneView?.clearDisplay(); text('frame-count', '0 帧');
    }
    renderTransport(); renderEmpty();
    if (status === 'ready') text('load-status', '固件已加载并校验，可以启动。');
    else if (status === 'running') text('load-status', '固件正在本地 WebAssembly 中执行。');
    else if (status === 'loading') text('load-status', '正在读取固件并初始化 WebAssembly…');
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
  const canLoad = Boolean(scene?.firmware || localFile);
  const sessionBusy = ['preparing', 'recording', 'playing'].includes(recordingState.status);
  if ($('start-runtime')) {
    $('start-runtime').disabled = busy || sessionBusy || !executable || status === 'running' || status === 'loading' || !(['ready', 'paused'].includes(status) || canLoad);
    $('start-runtime').textContent = status === 'paused' ? '▶ 继续' : status === 'loading' ? '加载中…' : '▶ 启动';
  }
  if ($('pause-runtime')) $('pause-runtime').disabled = busy || sessionBusy || status !== 'running';
  if ($('reset-runtime')) $('reset-runtime').disabled = busy || sessionBusy || !['running', 'paused', 'ready'].includes(status);
  if ($('load-firmware')) $('load-firmware').disabled = busy || !executable;
  if ($('capture-frame')) $('capture-frame').disabled = frameCount === 0;
  const badge = $('runtime-badge');
  if (badge) { badge.className = `runtime-badge ${status}`; badge.replaceChildren(element('span'), document.createTextNode(statuses[status] || status)); }
  document.querySelectorAll('.device-button').forEach(button => { button.disabled = status !== 'running'; });
  transportUi?.refresh();
}
function renderEmpty() {
  if (!$('screen-empty') || !product()) return;
  $('screen-empty').hidden = frameCount > 0;
  if (product().runtime.status === 'unavailable') {
    text('empty-title', 'PaperMono · 外观与配置预览');
    text('empty-description', 'SSD1677 / FT6336G 板级模型待实现，当前不能运行固件。');
  } else if (!product().display) {
    text('empty-title', status === 'running' ? '真实 CPU 正在执行 Hello World' : 'ESP32-S3 · 串口执行基准');
    text('empty-description', '此诊断板没有屏幕，请在下方串口查看固件输出。');
  } else if (status === 'loading') {
    text('empty-title', '正在加载真实固件'); text('empty-description', '初始化 WebAssembly 与 Flash 镜像，请稍候。');
  } else if (status === 'running') {
    text('empty-title', '固件运行中，等待屏幕帧'); text('empty-description', '屏幕仅显示模拟器的实际输出。更多状态请查看串口。');
  } else {
    text('empty-title', '按下启动，让固件点亮屏幕'); text('empty-description', '平台诊断固件：A 启停 · B 清零 · 支持触摸与 GPS UART1 输入。');
  }
}
function renderProduct() {
  const selected = product(); if (!selected) return;
  text('stage-title', selected.name); text('product-name', selected.name.replace(/^M5Stack /, ''));
  text('model-origin', selected.id === 'm5stack-stopwatch' ? '官方 CAD 外壳 · 实际帧缓冲' : '示意外壳 · 实际帧缓冲');
  text('product-model', `${selected.manufacturer} / ${selected.sku}`);
  text('product-symbol', selected.display?.shape === 'round' ? '◉' : selected.display ? '▤' : '▧');
  if ($('product-select')) $('product-select').value = selected.id;
  const specs = $('product-specs');
  if (specs) {
    specs.replaceChildren();
    for (const [label, value] of [['芯片', selected.module || selected.chip.toUpperCase()], ['存储', `${selected.runtime.flashMb} MB / ${selected.runtime.psramMb} MB PSRAM`], ['屏幕', selected.display ? `${selected.display.width} × ${selected.display.height} ${selected.display.type}` : '串口输出 / 无屏幕'], ['接口', selected.interfaces.filter(item => item.external).map(item => item.name || item.type).join(' · ') || '详见产品元信息']]) {
      specs.append(element('dt', '', label), element('dd', '', value));
    }
  }
  const compatibility = $('compatibility');
  if (compatibility) {
    const labels = {partial: '部分硬件模型 · 按固件验证', unavailable: '外观预览 · 板级模型待实现', diagnostic: '通用执行基准 · 无产品外设'};
    compatibility.classList.toggle('supported', selected.runtime.status === 'diagnostic');
    compatibility.replaceChildren(element('strong', '', labels[selected.runtime.status] || selected.runtime.status), document.createTextNode(selected.limitations[0] || ''));
    const details = element('details'); details.style.marginTop = '5px'; details.append(element('summary', '', '硬件与模拟范围'));
    for (const limitation of selected.limitations.slice(1)) { const note = element('p', '', limitation); note.style.marginTop = '5px'; details.append(note); }
    compatibility.append(details);
  }
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
}
function renderFirmwareLibrary() {
  const select = $('firmware-select'); if (!select) return;
  select.replaceChildren(new Option('选择适配固件', ''));
  for (const firmware of catalog.firmwares.filter(item => item.productIds.includes(scene.productId))) {
    select.append(new Option(`${firmware.name}${firmware.status === 'reference' ? ' · 资料入口' : ''}`, firmware.id));
  }
  const current = catalog.firmwares.find(item => item.source?.manifestUrl === scene.firmware?.manifestUrl);
  select.value = current?.id || '';
  renderFirmwareNote(current);
}
function renderFirmwareNote(firmware) {
  const node = $('firmware-note'); if (!node) return;
  node.replaceChildren();
  if (firmware) {
    node.append(document.createTextNode(firmware.limitations?.join(' ') || ''));
    if (firmware.status === 'reference' && firmware.sources?.[0]) {
      const link = element('a', 'text-link', '打开官方固件入口 ↗'); link.href = firmware.sources[0].url; link.target = '_blank'; link.rel = 'noopener noreferrer'; node.append(document.createElement('br'), link);
    }
  } else node.textContent = '选择已固定的诊断固件，或加载自己的远程镜像。';
}
function reflectFirmwareSource({preserveLocal = false} = {}) {
  if (!scene.firmware) {
    if ($('firmware-select')) $('firmware-select').value = '';
    if ($('firmware-url')) $('firmware-url').value = '';
    if ($('firmware-sha')) $('firmware-sha').value = '';
    renderFirmwareNote(null);
    if (preserveLocal && localFile && firmwareTab === 'local') return;
    localFile = null;
    if ($('firmware-file')) $('firmware-file').value = '';
    text('local-file-name', 'merged .bin · 文件留在本机'); setFirmwareTab('library'); return;
  }
  const known = catalog.firmwares.find(item => item.source?.manifestUrl === scene.firmware.manifestUrl);
  if (known && $('firmware-select')) { $('firmware-select').value = known.id; renderFirmwareNote(known); setFirmwareTab('library'); }
  else if ($('firmware-url')) {
    $('firmware-url').value = scene.firmware.manifestUrl || scene.firmware.url || '';
    $('firmware-kind').value = scene.firmware.manifestUrl ? 'manifest' : 'merged'; $('firmware-sha').value = scene.firmware.sha256 || ''; setFirmwareTab('remote');
  }
}
function sourceFromInputs() {
  if (embedded) return scene.firmware;
  if (firmwareTab === 'local') {
    if (!localFile) throw new Error('请选择完整 Flash 镜像文件。');
    return {file: localFile, format: 'merged'};
  }
  if (firmwareTab === 'remote') {
    const url = $('firmware-url').value.trim(); if (!url) throw new Error('请输入固件地址。');
    const result = {[ $('firmware-kind').value === 'manifest' ? 'manifestUrl' : 'url' ]: url};
    const hash = $('firmware-sha').value.trim(); if (hash) result.sha256 = hash; return result;
  }
  const selected = catalog.firmwares.find(item => item.id === $('firmware-select')?.value);
  if (!selected?.source) throw new Error('请选择可加载固件；“资料入口”尚不包含已验证的镜像。');
  const source = clone(selected.source);
  if (scene.firmware?.manifestUrl === source.manifestUrl && scene.firmware.sha256) source.sha256 = scene.firmware.sha256;
  return source;
}
function renderControls() {
  const controls = product()?.controls.filter(control => control.type === 'button') || [];
  const bindings = $('key-bindings'), buttons = $('device-buttons'); bindings?.replaceChildren(); buttons?.replaceChildren();
  if (!controls.length && bindings) bindings.append(element('span', 'helper', '此设备没有可映射的按键。'));
  for (const control of controls) {
    const code = Object.keys(scene.keyMapping).find(key => scene.keyMapping[key] === control.id);
    if (bindings) {
      const row = element('div', 'key-binding'); row.append(element('span', '', `${control.name} · GPIO ${control.pin}`));
      const binding = element('button', '', code ? code.replace(/^Key/, '') : '未映射'); binding.type = 'button'; binding.setAttribute('aria-label', `重新映射 ${control.name} 的键盘按键`);
      binding.addEventListener('click', () => {
        recordingControl = control.id; document.querySelectorAll('.key-binding button').forEach(node => node.classList.remove('recording'));
        binding.classList.add('recording'); binding.textContent = '请按键';
      }); row.append(binding); bindings.append(row);
    }
    if (buttons) {
      const button = element('button', 'device-button'); button.dataset.control = control.id; button.setAttribute('aria-label', `按住 ${control.name}`);
      button.append(document.createTextNode(control.name.replace('Button ', '按键 '))); if (code) button.append(element('kbd', '', code.replace(/^Key/, '')));
      button.addEventListener('pointerdown', event => { if (event.button !== 0) return; event.preventDefault(); button.setPointerCapture(event.pointerId); sendInput({type: 'button', id: control.id, pressed: true}, `pointer:${event.pointerId}`); });
      for (const type of ['pointerup', 'pointercancel', 'lostpointercapture']) button.addEventListener(type, event => sendInput({type: 'button', id: control.id, pressed: false}, `pointer:${event.pointerId}`));
      button.addEventListener('click', event => { if (event.detail === 0) { void sendInput({type: 'button', id: control.id, pressed: true}, 'activation'); void sendInput({type: 'button', id: control.id, pressed: false}, 'activation'); } }); buttons.append(button);
    }
  }
}
async function sendInput(event, source = 'scene') {
  await transportUi?.beforeHumanInput();
  if (status !== 'running') return;
  if (event.type === 'button') return humanButtons.input(event, source);
  try { await runtime.input(event); } catch (error) { report(error); }
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
  if ($('edit-scene')) $('edit-scene').href = shareUrl('studio', {strictRecording: false});
}
function reflectMode(mode, unavailable = false) {
  for (const candidate of ['2d', '3d']) { const button = $(`view-${candidate}`); if (button) { button.classList.toggle('active', mode === candidate); button.setAttribute('aria-pressed', String(mode === candidate)); } }
  if ($('view-3d')) $('view-3d').disabled = unavailable;
  text('stage-gesture', mode === '3d' ? '拖动外壳旋转 · 滚轮缩放 · 屏幕支持点击与滑动' : '点击或左右滑动屏幕 · 按键支持键盘映射');
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
  if ($('firmware-issue')) $('firmware-issue').hidden = true;
  const source = sourceFromInputs();
  const next = clone(scene); next.firmware = source.file ? null : source;
  await configure(next); await runtime.loadFirmware(source); return source;
}
function defaultFirmware(next) {
  const firmware = catalog.firmwares.find(item => item.productIds.includes(next.productId) && item.status === 'available' && item.source);
  if (firmware) next.firmware = clone(firmware.source); return next;
}

function allDefinitions() {
  const items = new Map([...catalog.accessories, ...localDefinitions, ...(scene.localAccessories || [])].map(item => [item.id, item])); return [...items.values()];
}
function renderAccessories() {
  const selector = $('accessory-select');
  if (selector) { const previous = selector.value; selector.replaceChildren(new Option('选择配件…', '')); for (const item of allDefinitions()) selector.append(new Option(item.name, item.id)); selector.value = previous; }
  text('accessory-count', String(scene.accessories.length));
  sceneView?.setAccessories(scene.accessories.map(item => definition(item.catalogId)).filter(Boolean));
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
    if (item.model.id === 'uart-nmea') {
      const fields = element('div', 'gps-fields');
      const latitude = element('input'), longitude = element('input'); latitude.type = longitude.type = 'number'; latitude.step = longitude.step = 'any';
      latitude.min = '-90'; latitude.max = '90'; longitude.min = '-180'; longitude.max = '180';
      latitude.value = instance.parameters.latitude ?? 0; longitude.value = instance.parameters.longitude ?? 0;
      latitude.setAttribute('aria-label', `${item.name} 纬度`); longitude.setAttribute('aria-label', `${item.name} 经度`);
      const latLabel = element('label', '', '纬度'), lonLabel = element('label', '', '经度'); latLabel.append(latitude); lonLabel.append(longitude); fields.append(latLabel, lonLabel);
      const inject = element('button', 'button wide', '注入一帧 GPS 定位');
      inject.addEventListener('click', () => action(async () => {
        if (status !== 'running') throw new Error('请先启动固件，再向 UART1 注入 GPS 数据。');
        if (latitude.value === '' || longitude.value === '') throw new Error('请输入经纬度。');
        const result = await runtime.injectGps({accessoryId: instance.id, latitude: Number(latitude.value), longitude: Number(longitude.value)});
        log(`\n[GPS → ${result.port}] ${result.data}`); toast('NMEA 已送入 UART1；请查看固件的实际响应。');
      })); card.append(fields, inject);
    }
    list.append(card);
  }
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
  bind('product-select', 'change', () => action(async () => { localFile = null; const next = defaultFirmware(createDefaultScene(catalog, $('product-select').value)); await configure(next, {reflectSource: true}); if (!next.firmware) { $('firmware-select').value = ''; setFirmwareTab('library'); } }));
  document.querySelectorAll('[data-firmware-tab]').forEach(button => button.addEventListener('click', () => setFirmwareTab(button.dataset.firmwareTab)));
  bind('firmware-select', 'change', () => action(async () => { const selected = catalog.firmwares.find(item => item.id === $('firmware-select').value); const next = clone(scene); next.firmware = selected?.source ? clone(selected.source) : null; await configure(next); renderFirmwareNote(selected); }));
  bind('firmware-file', 'change', () => { localFile = $('firmware-file').files[0] || null; text('local-file-name', localFile ? `${localFile.name} · ${(localFile.size / 1024 / 1024).toFixed(2)} MB` : 'merged .bin · 文件留在本机'); renderTransport(); });
  bind('load-firmware', 'click', () => action(loadSelected));
  bind('start-runtime', 'click', () => action(async () => { if (!['ready', 'paused'].includes(status)) await loadSelected(); await runtime.start(); }));
  bind('pause-runtime', 'click', () => action(async () => { await releaseHumanInputs(); await runtime.pause(); }));
  bind('reset-runtime', 'click', () => action(async () => { await releaseHumanInputs(); await runtime.reset(); }));
  bind('run-demo', 'click', () => action(async () => {
    localFile = null; await configure(defaultFirmware(createDefaultScene(catalog, 'generic-esp32s3')), {reflectSource: true});
    await runtime.loadFirmware(); await runtime.start();
  }));
  for (const mode of ['2d', '3d']) bind(`view-${mode}`, 'click', () => action(async () => {
    const actual = sceneView.setMode(mode); const view = {...scene.view, deviceView: actual}; await runtime.updateSceneMetadata({view});
  }));
  bind('reset-view', 'click', () => sceneView.resetView());
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
    localFile = null; await configure(imported, {reflectSource: true}); transportUi?.setRecording(recorded); input.value = ''; toast(recorded ? '场景与录制已导入，点击回放即可重现操作。' : '场景已导入。');
  }));
  bind('share-scene', 'click', () => action(async () => { await transportUi?.syncRecording(); validatePortable(portableScene()); const url = shareUrl('studio'); await copy(url, localFile ? '分享链接已复制；本地固件需接收者自行选择。' : recordingDocument ? '场景与录制链接已复制；接收者需主动点击回放。' : '场景链接已复制。'); history.replaceState(null, '', url); }));
  bind('copy-embed', 'click', () => action(async () => { await transportUi?.syncRecording(); validatePortable(portableScene()); const url = shareUrl('embed'); await copy(`<iframe src="${url}" title="${product().name.replaceAll('"', '&quot;')} · ESP Player" width="800" height="640" allow="serial; fullscreen" loading="lazy"></iframe>`, 'iframe 代码已复制。'); }));
  bind('open-player', 'click', () => { try { validatePortable(portableScene()); window.open(shareUrl('embed'), '_blank', 'noopener'); } catch (error) { report(error); } });
  bind('keyboard-enabled', 'change', () => { if (!$('keyboard-enabled').checked) releaseKeys(); });
  document.addEventListener('keydown', event => {
    if (recordingControl) {
      event.preventDefault();
      if (!event.code || /^(Control|Alt|Meta)/.test(event.code)) { toast('请选择普通按键，例如 A、空格或方向键。'); return; }
      const control = recordingControl; recordingControl = null;
      if (event.code === 'Escape') { renderControls(); return; }
      action(async () => { const keyMapping = Object.fromEntries(Object.entries(scene.keyMapping).filter(([, value]) => value !== control)); keyMapping[event.code] = control; try { await runtime.updateSceneMetadata({keyMapping}); } finally { renderControls(); } }); return;
    }
    if (event.repeat || event.altKey || event.ctrlKey || event.metaKey || event.target.closest('input,select,textarea,[contenteditable="true"]') || $('diy-dialog')?.open || ($('keyboard-enabled') && !$('keyboard-enabled').checked)) return;
    const id = scene?.keyMapping[event.code]; if (!id || status !== 'running') return;
    event.preventDefault(); heldKeys.set(event.code, id); sendInput({type: 'button', id, pressed: true}, `key:${event.code}`);
  });
  document.addEventListener('keyup', event => { const id = heldKeys.get(event.code); if (!id) return; event.preventDefault(); heldKeys.delete(event.code); sendInput({type: 'button', id, pressed: false}, `key:${event.code}`); });
  window.addEventListener('blur', releaseHumanInputs);
  document.addEventListener('visibilitychange', () => { if (document.hidden) releaseHumanInputs(); });
  window.addEventListener('pagehide', () => { humanButtons?.invalidate(); transportUi?.dispose(); knowledgePanel?.dispose(); removeWatchlist?.(); detachBridge?.(); sceneView?.dispose(); runtime?.dispose(); }, {once: true});
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
  sceneView = createSceneView($('scene-mount'), {onInput: sendInput, onModeChange: reflectMode});
  runtime = createPlatformRuntime({catalog, onEvent: onRuntimeEvent});
  humanButtons = createHumanButtonController({send: event => runtime.input(event), isRunning: () => status === 'running', onError: report,
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
  setupEvents(); await runtime.configure(scene); renderLocalAccessories(); reflectFirmwareSource(); transportUi.setRecording(recordingDocument);
  removeWatchlist = await mountWatchlist($('firmware-watchlist'), {getProductId: () => scene.productId, onError: report});
  if (embedded) {
    text('empty-description', scene.firmware ? '点击启动，执行此场景中的固件。' : '此场景未指定固件，可点击“编辑”进行配置。');
    // Playback requires an explicit user action; embedding a URL never starts untrusted firmware silently.
  }
}
init().catch(error => { report(error); text('empty-title', '工作台初始化失败'); text('empty-description', error.message); });
