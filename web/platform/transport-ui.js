import {createFlasher} from './flasher.js';
import {loadWatchlist, checkWatchEntry, buildFirmwareIssueUrl} from './watchlist.js';
import {renderHintList} from './hints.js';
import {firmwareRunPolicy} from './firmware-policy.js';
import {createSdCardUi} from './sd-card-ui.js';

const node = (tag, className, text) => { const result = document.createElement(tag); if (className) result.className = className; if (text !== undefined) result.textContent = text; return result; };
const button = (label, className = 'button quiet tiny') => { const result = node('button', className, label); result.type = 'button'; return result; };
const safeUrl = value => { if (typeof value !== 'string' || !value.trim()) return null; try { const url = new URL(value, location.href); return ['https:', 'http:'].includes(url.protocol) ? url.href : null; } catch { return null; } };
const link = (label, href, className = 'text-link') => { const result = node('a', className, label); const url = safeUrl(href); if (url) result.href = url; result.target = '_blank'; result.rel = 'noopener noreferrer'; return result; };
const describe = value => {
  if (value === null || value === undefined) return '待核对';
  if (Array.isArray(value)) return value.map(describe).join('；');
  if (typeof value === 'object') return Object.entries(value).map(([key, item]) => `${key}: ${describe(item)}`).join(' · ');
  if (typeof value === 'boolean') return value ? '是' : '否';
  return String(value);
};

export function createTransportUi({mount, catalog, getScene, getSource, getApi, prepareRecording, releaseInputs, recordingMatchesScene, onRecordingState, onRecordingChange, onError, onLog}) {
  const root = node('section', 'session-tools'); root.setAttribute('aria-label', '操作录制与实体刷机');
  const row = node('div', 'session-tool-row');
  const storageButton = button('设备存储'); storageButton.dataset.action = 'device-storage';
  const record = button('● 重启并录制'), stop = button('■ 停止'), play = button('▷ 回放'), takeOver = button('接管', 'button tiny'), options = button('详情', 'button quiet tiny'), flash = button('刷到设备 ↗', 'button tiny');
  const recordingStatus = node('span', 'session-status', '未录制'); recordingStatus.setAttribute('aria-live', 'polite');
  row.append(record, stop, play, takeOver, recordingStatus, options, storageButton, flash); root.append(row);
  const details = node('details', 'session-details'), summary = node('summary', '', '录制、存储与刷机说明'); details.append(summary);
  const recordingNote = node('p', 'helper', '录制从重启开始，保存按键、点击与滑动。共享录制需要接收者主动点击回放。串口、GPS 与刷机不会录入。');
  const recordingReceipt = node('p', 'helper'); recordingReceipt.hidden = true;
  const clearRecording = button('清除录制'), flashHeading = node('h3', '', '连接真实设备'), flashHint = node('p', 'helper', '实机刷写通路尚未完成硬件验收。选择串口后会写入当前固件，请先备份设备数据；写入前会检查芯片与容量。');
  const flashStatus = node('p', 'flash-status', '尚未连接硬件'); flashStatus.setAttribute('aria-live', 'polite');
  const progress = node('progress', 'flash-progress'); progress.max = 1; progress.value = 0; progress.hidden = true; progress.setAttribute('aria-label', '实体设备刷写进度');
  const cancelFlash = button('取消刷写'), flashIssue = link('检查失败：打开 GitHub issue 草稿 ↗', '#'), flashLog = node('pre', 'flash-log');
  cancelFlash.hidden = true; flashIssue.hidden = true; flashLog.hidden = true;
  details.append(flashHeading, flashStatus, progress, cancelFlash, flashHint, flashIssue, flashLog, recordingNote, recordingReceipt, clearRecording); root.append(details); mount.append(root);
  const storageSection = node('section', 'device-storage-details'), storageHeading = node('h3', '', '模拟设备存储');
  const storageIdentity = node('p', 'helper'), storageStatus = node('p', 'helper'); storageStatus.setAttribute('role', 'status');
  const storageNote = node('p', 'helper', '运行中写入的数据按设备型号和固件内容分别保存在当前浏览器。清空只影响这里显示的设备与固件，固件下载缓存单独管理。');
  const clearStorage = button('清空当前设备数据'), confirmStorage = node('div', 'storage-confirm'); confirmStorage.hidden = true;
  clearStorage.dataset.action = 'clear-device-storage';
  const confirmNote = node('p', 'helper', '将删除此固件写入的配置和文件，并停止模拟器。再次启动会从原始固件状态开始。'), confirmClearStorage = button('确认清空', 'button tiny'), cancelClearStorage = button('取消');
  confirmClearStorage.dataset.action = 'confirm-clear-device-storage';
  confirmStorage.append(confirmNote, confirmClearStorage, cancelClearStorage);
  storageSection.append(storageHeading, storageIdentity, storageStatus, storageNote, clearStorage, confirmStorage); details.prepend(storageSection);
  let deviceStorage = null, storageOperation = false, storageScope = null;
  let recording = null, recordingState = {status: 'idle', recording: null, progress: {index: 0, total: 0}}, localSelection = false, recordingBusy = false, replayRequested = false, takeoverPending = null, active = true, flashState = {status: 'idle'}, flashTarget = null;
  const flashActive = () => ['choosing', 'preparing', 'connecting', 'validating', 'writing', 'resetting', 'cancelling'].includes(flashState.status);
  const recordingActive = () => ['preparing', 'recording', 'playing'].includes(recordingState.status);
  const api = () => getApi();
  const sdUi = createSdCardUi({mount: storageSection, getApi, blocked: () => storageOperation || recordingBusy || recordingActive() || flashActive(), releaseInputs, onError});
  function render() {
    if (!active) return;
    const supported = typeof api()?.recordingStart === 'function';
    const executable = Boolean(catalog.products.find(product => product.id === getScene().productId)?.runtime?.board);
    record.disabled = !supported || recordingBusy || recordingActive() || !executable;
    stop.disabled = recordingState.status !== 'recording';
    const matchesScene = !recording || !recordingMatchesScene || recordingMatchesScene(recording);
    play.disabled = !supported || !recording || recordingBusy || recordingActive() || !matchesScene;
    takeOver.hidden = !replayRequested && recordingState.status !== 'playing';
    takeOver.disabled = Boolean(takeoverPending);
    clearRecording.disabled = recordingBusy || recordingActive() || !recording;
    const count = recordingState.progress?.total ?? recording?.events?.length ?? 0;
    const labels = {idle: recording ? `${recording.events?.length || 0} 个操作` : '未录制', preparing: '准备中…', recording: `录制中 · ${count} 个操作`, playing: `回放 ${recordingState.progress?.index || 0}/${count}`, completed: `${recordingState.receipt?.kind === 'recording' ? '已录制' : '回放完成'} · ${count} 个操作`, cancelled: '已接管，可继续操作', error: '录制或回放失败'};
    recordingStatus.textContent = !matchesScene && !recordingActive() ? '配置已变更 · 旧录制已禁用' : labels[recordingState.status] || recordingState.status;
    root.classList.toggle('is-recording', recordingState.status === 'recording');
    recordingReceipt.hidden = recordingState.receipt?.kind !== 'replay';
    if (!recordingReceipt.hidden) {
      const comparison = {equal: '最终屏幕一致', different: '最终屏幕不同', unavailable: '暂无屏幕对比'}[recordingState.receipt.frameComparison] || '尚未完成屏幕对比';
      recordingReceipt.textContent = `${comparison} · ${recordingState.receipt.timingDifferences?.length || 0} 项时序偏差。回放依据虚拟时钟观察值，不保证逐周期一致。`;
    }
    const scope = deviceStorage?.scope;
    const supportedStorage = typeof api()?.clearDeviceStorage === 'function';
    storageButton.disabled = !supportedStorage;
    if (storageScope !== scope?.key) { storageScope = scope?.key; confirmStorage.hidden = true; }
    const storageProduct = catalog.products.find(product => product.id === scope?.productId);
    storageIdentity.textContent = scope ? `${storageProduct?.name || scope.productId} · 固件 ${scope.firmwareSha256.slice(0, 12)}` : '加载固件后显示对应的设备存储。';
    const storageLabels = {unloaded:'尚未加载固件',loading:'正在恢复设备数据…',clean:deviceStorage?.restored?'已恢复此前的设备数据':'使用原始固件数据',saving:'正在保存设备数据…',saved:'设备数据已保存到此浏览器', 'memory-only':'浏览器持久化不可用，当前数据仅在本次页面中保留',conflict:'另一页面已修改此设备数据，模拟已停止，请重新加载固件',error:'设备数据保存失败，请勿关闭页面'};
    storageStatus.textContent = storageLabels[deviceStorage?.status] || (supportedStorage ? '正在读取存储状态…' : '此运行时尚未提供设备存储管理');
    storageStatus.classList.toggle('storage-warning', ['memory-only','conflict','error'].includes(deviceStorage?.status));
    clearStorage.disabled = !supportedStorage || !scope || storageOperation || recordingBusy || recordingActive() || flashActive() || ['loading','saving','conflict'].includes(deviceStorage?.status);
    confirmClearStorage.disabled = clearStorage.disabled;
    cancelClearStorage.disabled = storageOperation;
    if (storageOperation) storageStatus.textContent = '正在清空设备数据…';
    sdUi.refresh();
    const capability = flasher.capabilities();
    flash.disabled = !capability.supported || flashActive();
    flash.title = capability.supported ? '选择串口并将当前固件写入真实设备' : capability.reason;
    if (!capability.supported && flashState.status === 'idle') flashStatus.textContent = capability.reason;
  }
  function acceptRecordingState(state) {
    if (localSelection && !['preparing', 'recording', 'playing'].includes(state.status)) return;
    localSelection = false;
    recordingState = state;
    if (state.recording !== undefined) { recording = state.recording; onRecordingChange?.(recording); }
    onRecordingState?.(state); render();
  }
  async function syncRecording() {
    if (typeof api()?.recordingObserve === 'function') acceptRecordingState(await api().recordingObserve());
    return recording;
  }
  async function takeControl() {
    if (takeoverPending) return takeoverPending;
    if (!replayRequested && !['preparing', 'playing'].includes(recordingState.status)) return;
    takeoverPending = Promise.resolve(api().recordingTakeOver()).then(syncRecording).finally(() => { takeoverPending = null; replayRequested = false; render(); });
    render(); return takeoverPending;
  }
  record.addEventListener('click', async () => {
    if (recordingBusy) return; recordingBusy = true; render();
    try { await prepareRecording(); await api().recordingStart(); await syncRecording(); }
    catch (error) { onError(error); }
    finally { recordingBusy = false; render(); }
  });
  stop.addEventListener('click', async () => {
    try { await releaseInputs(); recording = await api().recordingStop(); onRecordingChange?.(recording); await syncRecording(); }
    catch (error) { onError(error); }
  });
  async function playRecording() {
    if (!recording || recordingBusy) return; recordingBusy = true; replayRequested = true; render();
    try { await releaseInputs(); await api().recordingPlay(recording); await syncRecording(); }
    finally { recordingBusy = false; replayRequested = false; render(); }
  }
  play.addEventListener('click', () => { void playRecording().catch(onError); });
  takeOver.addEventListener('click', () => { void takeControl().catch(onError); });
  clearRecording.addEventListener('click', async () => {
    try { localSelection = false; await api().recordingClear(); await syncRecording(); }
    catch (error) { onError(error); }
  });
  options.addEventListener('click', () => { details.open = !details.open; });
  storageButton.addEventListener('click', async () => {
    details.open = true;
    try { deviceStorage = (await api().observe()).storage || null; render(); storageSection.scrollIntoView({block:'nearest'}); }
    catch (error) { onError(error); }
  });
  clearStorage.addEventListener('click', () => { confirmStorage.hidden = false; });
  cancelClearStorage.addEventListener('click', () => { confirmStorage.hidden = true; });
  confirmClearStorage.addEventListener('click', async () => {
    if (storageOperation) return;
    storageOperation = true; render();
    try { await releaseInputs(); const cleared = await api().clearDeviceStorage(); deviceStorage = cleared.storage; confirmStorage.hidden = true; onLog?.('[设备存储] 已还原当前设备与固件的初始数据。\n'); }
    catch (error) { onError(error); }
    finally { storageOperation = false; render(); }
  });
  const flasher = createFlasher({onEvent(event) {
    if (!active) return;
    if (event.type === 'log') { flashLog.hidden = false; flashLog.textContent = (flashLog.textContent + event.message + '\n').slice(-12000); return; }
    if (event.type !== 'state') return;
    flashState = event.state; details.open = true;
    const suffix = flashState.chip ? ` · ${describe(flashState.chip)}` : '';
    flashStatus.textContent = `${flashTarget?.name || '设备'}：${flashState.message || flashState.status}${suffix}`;
    progress.value = Math.max(0, Math.min(1, flashState.progress || 0)); progress.hidden = !flashActive() && flashState.status !== 'done';
    cancelFlash.hidden = !flashActive(); cancelFlash.disabled = flashState.status === 'cancelling';
    if (flashState.status === 'error') {
      const source = flashTarget?.source, url = source?.manifestUrl || source?.url;
      if (url) { flashIssue.href = safeUrl(flashState.error?.details?.issueUrl) || buildFirmwareIssueUrl({entryId: 'custom-source', url, reason: flashState.error?.message || flashState.message, status: flashState.error?.details?.status, productId: flashTarget?.id}); flashIssue.hidden = false; }
    }
    onLog?.(`[刷机] ${flashState.message || flashState.status}\n`); render();
  }});
  // Keep requestPort inside the direct click call stack: no awaited preparation or automation API.
  flash.addEventListener('click', () => {
    try {
      const scene = getScene(), source = getSource(), selected = catalog.products.find(product => product.id === scene.productId);
      if (!source) throw new Error('此场景没有固件地址，请先在工作台选择固件。');
      flashTarget = {id: scene.productId, name: selected?.name, source}; flashIssue.hidden = true; flashLog.textContent = ''; flashLog.hidden = true;
      const operation = flasher.flash({scene, catalog, source});
      Promise.resolve(operation).catch(error => { if (error.code !== 'FLASH_CANCELLED') onError(error); });
    } catch (error) { onError(error); }
  });
  cancelFlash.addEventListener('click', () => { flasher.cancel(); });
  const unsubscribe = api()?.subscribe?.(event => { if (event.type === 'recording') acceptRecordingState(event.state); if (event.type === 'storage') { deviceStorage = event.storage; render(); } });
  Promise.resolve(api()?.observe?.()).then(state => { if (active) { deviceStorage = state?.storage || null; render(); } }).catch(() => {});
  render();
  return {
    refresh: render,
    syncRecording,
    playRecording,
    getRecording: () => recording,
    getRecordingState: () => recordingState,
    setRecording(value) { localSelection = true; recording = value; recordingState = {status: 'idle', recording: value, progress: {index: 0, total: value?.events?.length || 0}}; onRecordingChange?.(value); onRecordingState?.(recordingState); render(); },
    async beforeHumanInput() { if (replayRequested || recordingState.status === 'playing') await takeControl(); },
    async settleForConfiguration() {
      if (replayRequested || ['preparing', 'playing'].includes(recordingState.status)) await takeControl();
      if (recordingState.status === 'recording') { await releaseInputs(); recording = await api().recordingStop(); onRecordingChange?.(recording); await syncRecording(); }
    },
    dispose() { active = false; unsubscribe?.(); sdUi.dispose(); if (flashActive()) flasher.cancel(); root.remove(); },
  };
}

const factKeyLabels = {architecture: '架构', cores: '核心', wordBits: '位宽', clockMHz: '主频 MHz', standard: '标准', bandGHz: '频段 GHz', type: '类型', shape: '形状', sizeInches: '英寸', widthPixels: '宽 px', heightPixels: '高 px', controller: '驱动', bus: '总线', capacityMah: '容量 mAh', voltage: '电压 V', width: '宽', height: '高', depth: '厚', widthMm: '宽 mm', heightMm: '高 mm', depthMm: '厚 mm', lengthMm: '长 mm', pin: '引脚', gpio: 'GPIO', name: '名称', address: '地址', port: '接口', color: '颜色', voltageV: '电压 V', pins: '引脚', resolution: '分辨率', default: '默认', signal: '信号', role: '作用', status: '状态', source: '来源', baudRate: '波特率', note: '说明'};
function factValue(value) {
  if (Array.isArray(value)) return value.map(factValue).join('；');
  if (value && typeof value === 'object') return Object.entries(value).map(([key, item]) => `${factKeyLabels[key] || key} ${factValue(item)}`).join(' · ');
  return describe(value);
}

export function createKnowledgePanel(mount) {
  if (!mount) return {show() {}, dispose() {}};
  const details = node('details', 'knowledge-details'), summary = node('summary', '', '技术参数与资料来源'), body = node('div', 'knowledge-body'); details.append(summary, body); mount.append(details);
  const cache = new Map(); let sequence = 0, active = true;
  return {
    async show(product) {
      const token = ++sequence; body.replaceChildren(); details.hidden = !product?.knowledgeUrl;
      if (!product?.knowledgeUrl) return;
      const url = new URL(product.knowledgeUrl, import.meta.url); body.append(node('p', 'helper', '正在读取产品知识库…'));
      try {
        if (!cache.has(url.href)) cache.set(url.href, fetch(url).then(async response => { if (!response.ok) throw new Error(`产品资料 HTTP ${response.status}`); return response.json(); }));
        const knowledge = await cache.get(url.href); if (!active || token !== sequence) return;
        body.replaceChildren(node('p', 'knowledge-summary', knowledge.summary || product.name));
        body.append(link('打开产品知识 JSON ↗', url.href));
        if (product.id === 'm5stack-stopwatch') body.append(link('模型与官方照片对照 ↗', new URL('./model-review.html?product=m5stack-stopwatch', import.meta.url).href));
        body.append(node('p', 'helper', product.id === 'm5stack-stopwatch' ? '外壳采用官方 CAD。红色电源键仅作外观展示，未模拟电源控制。' : '当前外观为示意模型；硬件能力以已实现的板级模型为准。'));
        if (knowledge.updatedAt) body.append(node('p', 'helper', `知识库更新：${knowledge.updatedAt}`));
        if (knowledge.hardwareRevision === 'unspecified') body.append(node('p', 'helper', '硬件修订版尚未指定，接线请同时核对实物版本。'));
        const groupLabels = {compute: '主控与存储', display: '屏幕与触摸', power: '供电', interfaces: '接口', input: '输入', sensors: '传感器', mechanical: '尺寸与外观', dimensions: '尺寸', connectivity: '连接', audio: '音频', storage: '存储', pins: '引脚', peripherals: '外设'};
        const groups = new Map();
        for (const fact of knowledge.facts || []) { const group = fact.group || 'other'; if (!groups.has(group)) groups.set(group, []); groups.get(group).push(fact); }
        for (const [group, facts] of groups) {
          const section = node('section', 'knowledge-group'); section.append(node('h3', '', groupLabels[group] || group));
          const list = node('dl', 'knowledge-facts');
          for (const fact of facts) {
            list.append(node('dt', '', fact.label));
            const value = node('dd', '', `${factValue(fact.value)}${fact.unit ? ` ${fact.unit}` : ''}`);
            const basis = {'official': '官方资料', 'cad-measurement': 'CAD 测量', 'image-measurement': '图片估算', 'unknown': '待核对'}[fact.basis];
            if (basis) value.append(node('span', 'fact-basis', basis));
            if (fact.notes) value.append(node('small', 'fact-notes', factValue(fact.notes)));
            if (fact.method) value.append(node('small', 'fact-notes', `方法：${factValue(fact.method)}`));
            list.append(value);
          }
          section.append(list); body.append(section);
        }
        if (knowledge.revisions?.length) {
          const revisions = node('section', 'knowledge-group'); revisions.append(node('h3', '', '硬件版本差异'));
          for (const revision of knowledge.revisions) { const row = node('p', 'helper'); row.append(node('strong', '', `${revision.id}${revision.releaseDate ? ` · ${revision.releaseDate}` : ''}`), document.createElement('br'), document.createTextNode(revision.changes || '')); revisions.append(row); }
          body.append(revisions);
        }
        const sources = node('section', 'knowledge-group'); sources.append(node('h3', '', '资料来源与抓取记录'));
        for (const source of knowledge.sources || []) {
          const item = node('div', 'knowledge-source'); item.append(link(source.title || source.id || '资料链接', source.url));
          item.append(node('small', '', source.status === 'fetched' && source.fetchedAt ? `实际抓取：${source.fetchedAt}` : '仅提供链接，尚无抓取记录'));
          if (source.sha256) { const hash = node('code', '', `SHA-256 ${source.sha256}`); hash.title = source.sha256; item.append(hash); }
          sources.append(item);
        }
        body.append(sources);
        if (knowledge.unknowns?.length) { const unknowns = node('section', 'knowledge-group'); unknowns.append(node('h3', '', '仍待核对')); for (const unknown of knowledge.unknowns) { const row = node('p', 'helper'); row.append(node('strong', '', unknown.label || unknown.id || '待核对项目')); if (unknown.notes) row.append(document.createElement('br'), document.createTextNode(factValue(unknown.notes))); unknowns.append(row); } body.append(unknowns); }
      } catch (error) { if (active && token === sequence) body.replaceChildren(node('p', 'helper', error.message), link('查看知识库文件 ↗', url.href)); }
    },
    dispose() { active = false; sequence++; details.remove(); },
  };
}

// Version metadata belongs to the selected, product-filtered firmware entry.
export async function mountFirmwareUpdates(mount, {getProductId} = {}) {
  if (!mount) return {show() {}, dispose() {}};
  const body = node('div', 'firmware-updates'), identity = node('p', 'firmware-update-identity'), hints = node('div', 'hint-list'), actions = node('div', 'firmware-update-actions');
  hints.setAttribute('role', 'group'); hints.setAttribute('aria-label', '固件来源与版本说明');
  const check = button('检查版本'), result = node('div', 'watch-result'); result.setAttribute('aria-live', 'polite');
  const downloadDetails=node('details','firmware-download-details'),downloadSummary=node('summary','','固件下载地址'),downloadLink=link('','#','firmware-download-url'),copyDownload=button('复制地址');
  downloadDetails.append(downloadSummary,downloadLink,copyDownload);
  copyDownload.addEventListener('click',async()=>{try{await navigator.clipboard.writeText(downloadLink.href);copyDownload.textContent='已复制';}catch{copyDownload.textContent='请选中地址复制';}});
  body.append(identity, hints, actions, downloadDetails, result); mount.append(body); body.hidden = true;
  let active = true, checkSequence = 0, checkController, selected = null, selectedProductId = null, entry = null, list = null, listError = null;
  try { list = await loadWatchlist(); } catch (error) { listError = error; }
  function show(firmware) {
    if (!active) return;
    const productId=getProductId?.();
    if (selected?.id === firmware?.id && selectedProductId===productId) return;
    selected = firmware || null; selectedProductId=productId; entry = null; checkSequence++; checkController?.abort(); result.replaceChildren(); actions.replaceChildren(); renderHintList(hints, []);
    downloadDetails.open=false;downloadDetails.hidden=true;copyDownload.textContent='复制地址';downloadLink.removeAttribute('href');downloadLink.textContent='';
    body.hidden = !firmware || !firmware.productIds.includes(getProductId?.()); if (body.hidden) return;
    const downloadUrl=safeUrl(firmware.source?.url||firmware.source?.manifestUrl);
    if(downloadUrl){downloadDetails.hidden=false;downloadSummary.textContent=firmware.source?.manifestUrl?'固件清单地址':'固件下载地址';downloadLink.href=downloadUrl;downloadLink.textContent=downloadUrl;}
    entry = list?.entries.find(item => item.id === firmware.watchlistId) || null;
    const role = firmware.role === 'factory' ? '官方出厂' : firmware.role === 'third-party' ? '第三方项目' : '已收录项目';
    identity.textContent = `${firmwareRunPolicy(null, firmware).label} · ${role}${firmware.revision || entry?.version ? ` · ${firmware.revision || entry.version}` : ''}`;
    const facts = [firmware.publisher ? `发布者：${firmware.publisher}` : null, entry?.description, entry?.compatibility?.note, entry?.browserNote, entry?.checkedAt ? `元信息核对于 ${entry.checkedAt}` : null].filter(Boolean);
    const hintItems = facts.length ? [{id: 'source-detail', icon: 'cloud', label: '来源说明', title: '发布来源与访问条件', paragraphs: facts}] : [];
    if (entry) hintItems.push({id: 'update-check', icon: 'clock', label: '版本核对', title: '发布版本与同步时间', paragraphs: ['发布前按产品、渠道和镜像格式同步版本；缺少匹配文件或可信 hash 时保留原镜像。检查版本只核对当前设备的固件，读取本站快照，不直接调用 GitHub API，也不下载固件正文。运行所需固件可缓存在当前浏览器，服务器不保存。', '资源链接核对可能受上游 CORS 设置限制。版本快照过期或同步失败不代表固件链接已经失效。']});
    renderHintList(hints, hintItems);
    const releaseUrl = entry?.releaseUrl || firmware.sources?.find(source => /releases/.test(source.url))?.url || firmware.sources?.[0]?.url;
    if (releaseUrl) actions.append(link('上游发布页 ↗', releaseUrl));
    if (entry?.browserInstallUrl && entry.browserInstallUrl !== releaseUrl) actions.append(link('下载入口 ↗', entry.browserInstallUrl));
    check.hidden = !entry; check.disabled = false; actions.append(check);
    if (listError) result.append(node('p', 'helper', '版本元信息暂时无法读取，仍可打开上游发布页。'));
  }
  check.addEventListener('click', async () => {
    if (!entry) return; const requested = entry, token = ++checkSequence; checkController?.abort(); checkController = new AbortController();
    check.disabled = true; result.replaceChildren(node('p', 'helper', '正在读取发布信息…'));
    try {
      const report = await checkWatchEntry(requested, {signal: checkController.signal,firmwareId:selected?.id,productId:selectedProductId,currentVersion:selected?.revision||selected?.version,source:selected?.source}); if (!active || token !== checkSequence) return;
      result.replaceChildren(node('p', 'watch-version', report.latestVersion ? `${report.updateAvailable ? '发现新版本' : report.metadata?.source === 'site-snapshot' ? '已同步版本' : '上游版本'}：${report.latestVersion}` : '暂时无法确认最新版本'));
      if (report.metadata?.source === 'site-snapshot') {
        result.append(node('p', 'helper', report.metadata.checkedAt ? `发布信息同步：${report.metadata.checkedAt}` : '本站暂无成功同步记录。'));
        if (report.metadata.stale) result.append(node('p', 'helper', '当前为较早快照，最新发布信息请查看上游发布页。'));
        if (report.metadata.refreshError) result.append(node('p', 'helper', `上游同步暂未更新：${report.metadata.refreshError.message || String(report.metadata.refreshError)}。这不代表固件链接失效。`));
      }
      if(report.metadata?.probeReason==='selected-artifact-ambiguous')result.append(node('p','helper','当前固件包含多份候选资源，暂未探测链接，避免把其他设备的资源状态当作本机结果。'));
      if (report.checkedAt) result.append(node('p', 'helper', `本次查询：${report.checkedAt}`));
      if (report.error) result.append(node('p', 'helper', report.error.message));
      for (const asset of report.assets || []) {
        const labels = {reachable: '可访问', missing: '404/410，资源已失效', 'http-error': `HTTP ${asset.status}，需要检查`, 'network-or-cors': '网络或 CORS 阻止核对，不能据此认定失效', cancelled: '已取消'};
        result.append(node('p', 'watch-asset', `${labels[asset.state] || asset.state} · ${asset.name || new URL(asset.url).pathname.split('/').pop() || asset.url}`));
      }
      if (report.releaseUrl) result.append(link('查看当前发布页 ↗', report.releaseUrl));
      if (report.issueUrl && report.assets?.some(asset => [404, 410].includes(asset.status))) result.append(link('固件资源失效：打开 issue 草稿 ↗', report.issueUrl));
    } catch (error) {
      if (active && token === checkSequence) result.replaceChildren(node('p', 'helper', `发布信息暂时无法读取：${error.message}`), node('p', 'helper', '可稍后重试，或打开上游发布页。查询失败不代表固件失效。'));
    } finally { if (active && token === checkSequence) check.disabled = false; }
  });
  return {show, dispose() { active = false; checkSequence++; checkController?.abort(); renderHintList(hints, []); body.remove(); }};
}
