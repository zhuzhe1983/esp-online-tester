import {createFlasher} from './flasher.js';
import {loadWatchlist, checkWatchEntry, buildFirmwareIssueUrl} from './watchlist.js';

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
  const record = button('● 重启并录制'), stop = button('■ 停止'), play = button('▷ 回放'), takeOver = button('接管', 'button tiny'), options = button('详情', 'button quiet tiny'), flash = button('刷到设备 ↗', 'button tiny');
  const recordingStatus = node('span', 'session-status', '未录制'); recordingStatus.setAttribute('aria-live', 'polite');
  row.append(record, stop, play, takeOver, recordingStatus, options, flash); root.append(row);
  const details = node('details', 'session-details'), summary = node('summary', '', '录制、回放与刷机说明'); details.append(summary);
  const recordingNote = node('p', 'helper', '录制从重启开始，保存按键、点击与滑动。共享录制需要接收者主动点击回放。串口、GPS 与刷机不会录入。');
  const recordingReceipt = node('p', 'helper'); recordingReceipt.hidden = true;
  const clearRecording = button('清除录制'), flashHeading = node('h3', '', '连接真实设备'), flashHint = node('p', 'helper', '实机刷写通路尚未完成硬件验收。选择串口后会写入当前固件，请先备份设备数据；写入前会检查芯片与容量。');
  const flashStatus = node('p', 'flash-status', '尚未连接硬件'); flashStatus.setAttribute('aria-live', 'polite');
  const progress = node('progress', 'flash-progress'); progress.max = 1; progress.value = 0; progress.hidden = true; progress.setAttribute('aria-label', '实体设备刷写进度');
  const cancelFlash = button('取消刷写'), flashIssue = link('检查失败：打开 GitHub issue 草稿 ↗', '#'), flashLog = node('pre', 'flash-log');
  cancelFlash.hidden = true; flashIssue.hidden = true; flashLog.hidden = true;
  details.append(flashHeading, flashStatus, progress, cancelFlash, flashHint, flashIssue, flashLog, recordingNote, recordingReceipt, clearRecording); root.append(details); mount.append(root);
  let recording = null, recordingState = {status: 'idle', recording: null, progress: {index: 0, total: 0}}, localSelection = false, recordingBusy = false, replayRequested = false, takeoverPending = null, active = true, flashState = {status: 'idle'}, flashTarget = null;
  const flashActive = () => ['choosing', 'preparing', 'connecting', 'validating', 'writing', 'resetting', 'cancelling'].includes(flashState.status);
  const recordingActive = () => ['preparing', 'recording', 'playing'].includes(recordingState.status);
  const api = () => getApi();
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
  play.addEventListener('click', async () => {
    if (!recording || recordingBusy) return; recordingBusy = true; replayRequested = true; render();
    try { await releaseInputs(); await api().recordingPlay(recording); await syncRecording(); }
    catch (error) { onError(error); }
    finally { recordingBusy = false; replayRequested = false; render(); }
  });
  takeOver.addEventListener('click', () => { void takeControl().catch(onError); });
  clearRecording.addEventListener('click', async () => {
    try { localSelection = false; await api().recordingClear(); await syncRecording(); }
    catch (error) { onError(error); }
  });
  options.addEventListener('click', () => { details.open = !details.open; });
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
  const unsubscribe = api()?.subscribe?.(event => { if (event.type === 'recording') acceptRecordingState(event.state); });
  render();
  return {
    refresh: render,
    syncRecording,
    getRecording: () => recording,
    getRecordingState: () => recordingState,
    setRecording(value) { localSelection = true; recording = value; recordingState = {status: 'idle', recording: value, progress: {index: 0, total: value?.events?.length || 0}}; onRecordingChange?.(value); onRecordingState?.(recordingState); render(); },
    async beforeHumanInput() { if (replayRequested || recordingState.status === 'playing') await takeControl(); },
    async settleForConfiguration() {
      if (replayRequested || ['preparing', 'playing'].includes(recordingState.status)) await takeControl();
      if (recordingState.status === 'recording') { await releaseInputs(); recording = await api().recordingStop(); onRecordingChange?.(recording); await syncRecording(); }
    },
    dispose() { active = false; unsubscribe?.(); if (flashActive()) flasher.cancel(); root.remove(); },
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

export async function mountWatchlist(mount, {getProductId, onError} = {}) {
  if (!mount) return () => {};
  const details = node('details', 'watchlist-details'), summary = node('summary', '', '知名固件 · 远程版本关注'); details.append(summary); mount.append(details);
  const body = node('div', 'watchlist-body'); body.append(node('p', 'helper', '读取关注列表…')); details.append(body);
  let active = true, checkSequence = 0, checkController;
  try {
    const list = await loadWatchlist(); if (!active) return () => {};
    body.replaceChildren(node('p', 'helper', '仅查询上游版本与资源状态，不下载或保存固件二进制。关注条目不代表已适配当前产品。'));
    const select = node('select'); select.setAttribute('aria-label', '选择关注的知名固件');
    for (const category of list.categories) {
      const group = node('optgroup'); group.label = category.name;
      for (const entry of list.entries.filter(item => item.category === category.id)) group.append(new Option(entry.name, entry.id));
      select.append(group);
    }
    const info = node('div', 'watch-entry'), check = button('检查远程版本', 'button wide'), result = node('div', 'watch-result'); result.setAttribute('aria-live', 'polite');
    body.append(select, info, check, result);
    function selectedEntry() { return list.entries.find(entry => entry.id === select.value); }
    function renderEntry() {
      checkSequence++; checkController?.abort(); const entry = selectedEntry(); info.replaceChildren(); result.replaceChildren(); check.disabled = false;
      if (!entry) return;
      info.append(node('p', 'helper', entry.description || entry.name), node('p', 'watch-compatibility', entry.compatibility?.note || '仅作资料关注，尚未验证产品适配。'));
      info.append(node('p', 'helper', `收录版本：${entry.version || '未固定'}${entry.checkedAt ? ` · 核对于 ${entry.checkedAt}` : ''}`));
      if (entry.releaseUrl) info.append(link('打开上游发布页 ↗', entry.releaseUrl));
    }
    select.addEventListener('change', renderEntry); renderEntry();
    check.addEventListener('click', async () => {
      const entry = selectedEntry(); if (!entry) return; const token = ++checkSequence; checkController?.abort(); checkController = new AbortController();
      check.disabled = true; result.replaceChildren(node('p', 'helper', '正在查询上游元信息…'));
      try {
        const report = await checkWatchEntry(entry, {signal: checkController.signal}); if (!active || token !== checkSequence) return;
        result.replaceChildren(node('p', 'watch-version', report.latestVersion ? `${report.updateAvailable ? '发现新版本' : '上游版本'}：${report.latestVersion}` : '暂时无法确认最新版本'));
        if (report.checkedAt) result.append(node('p', 'helper', `本次查询：${report.checkedAt}`));
        if (report.error) result.append(node('p', 'helper', report.error.message));
        for (const asset of report.assets || []) {
          const labels = {reachable: '可访问', missing: '404/410，资源已失效', 'http-error': `HTTP ${asset.status}，需要检查`, 'network-or-cors': '网络或 CORS 阻止核对，不能据此认定失效', cancelled: '已取消'};
          result.append(node('p', 'watch-asset', `${labels[asset.state] || asset.state} · ${new URL(asset.url).pathname.split('/').pop() || asset.url}`));
        }
        if (report.releaseUrl) result.append(link('查看当前发布页 ↗', report.releaseUrl));
        if (report.issueUrl) result.append(link('带上检查结果，打开 issue 草稿 ↗', report.issueUrl));
      } catch (error) {
        if (active && token === checkSequence) { result.replaceChildren(node('p', 'helper', error.message), link('打开 issue 草稿 ↗', buildFirmwareIssueUrl({entryId: entry.id, url: entry.metadataUrl, reason: error.message, productId: getProductId?.()}))); onError?.(error); }
      } finally { if (active && token === checkSequence) check.disabled = false; }
    });
  } catch (error) { body.replaceChildren(node('p', 'helper', error.message)); }
  return () => { active = false; checkSequence++; checkController?.abort(); details.remove(); };
}
