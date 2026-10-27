// Browser-only metadata. Firmware bytes stay in the separate firmware cache.
import {parseBurnerDownload} from './m5burner.js';
export const FIRMWARE_HISTORY_KEY = 'esp-online-tester:firmware-history:v1';
const LIMIT = 40;
const clean = value => typeof value === 'string' ? value.replace(/[\u0000-\u001f\u007f]/g, '').trim().slice(0, 120) : '';
function normalized(entry) {
  if (!entry || !['manifest', 'merged'].includes(entry.kind) || typeof entry.url !== 'string' || entry.url.length > 8192) return null;
  let url; try { url = new URL(entry.url); } catch { return null; }
  if (url.protocol !== 'https:' || url.username || url.password || !Number.isFinite(entry.lastUsedAt) || entry.lastUsedAt <= 0) return null;
  const sha256 = typeof entry.sha256 === 'string' ? entry.sha256.trim().toLowerCase() : '';
  if (sha256 && !/^[a-f0-9]{64}$/.test(sha256)) return null;
  url.hash = '';
  const burner=entry.provider==='m5burner'&&parseBurnerDownload(url.href);
  return {url:url.href, kind:entry.kind, sha256, productId:clean(entry.productId), name:clean(entry.name), version:clean(entry.version), lastUsedAt:entry.lastUsedAt,
    ...(burner?{provider:'m5burner',deviceId:typeof entry.deviceId==='string'&&/^\d{1,30}$/.test(entry.deviceId)?entry.deviceId:''}:{})};
}
const key = entry => JSON.stringify([entry.productId, entry.kind, entry.url, entry.provider==='m5burner'?'':entry.sha256]);
export function createFirmwareHistory(getStorage = () => globalThis.localStorage) {
  let memory = [], persistent = true;
  function read() {
    if (!persistent) return memory;
    try {
      const value = JSON.parse(getStorage().getItem(FIRMWARE_HISTORY_KEY) || '[]');
      memory = Array.isArray(value) ? value.map(normalized).filter(Boolean).sort((a,b)=>b.lastUsedAt-a.lastUsedAt).slice(0,LIMIT) : [];
    } catch { persistent = false; }
    return memory;
  }
  function write(value) {
    memory = value;
    try { getStorage().setItem(FIRMWARE_HISTORY_KEY, JSON.stringify(value)); persistent = true; }
    catch { persistent = false; }
    return persistent;
  }
  return {
    list(productId) { return read().filter(entry => productId === undefined || entry.productId === productId); },
    remember(entry) {
      const item = normalized(entry); if (!item) return false;
      return write([item, ...read().filter(old => key(old) !== key(item))].sort((a,b)=>b.lastUsedAt-a.lastUsedAt).slice(0,LIMIT));
    },
    clear(productId) { return write(productId === undefined ? [] : read().filter(entry=>entry.productId!==productId)); },
    get persistent() { return persistent; },
  };
}
export function firmwareHistoryName(url) {
  try { const path = new URL(url).pathname.split('/').filter(Boolean).at(-1); return clean(decodeURIComponent(path || '远程固件')); }
  catch { return '远程固件'; }
}
export function firmwareHistoryAge(time, now = Date.now()) {
  const minutes = Math.floor(Math.max(0, now-time)/60000);
  if (minutes < 1) return '刚刚';
  if (minutes < 60) return `${minutes} 分钟前`;
  if (minutes < 1440) return `${Math.floor(minutes/60)} 小时前`;
  if (minutes < 43200) return `${Math.floor(minutes/1440)} 天前`;
  return new Date(time).toLocaleDateString('zh-CN');
}
export function mountFirmwareHistory({input, kindInput, shaInput, productId, onSelect, mount}) {
  if (!input) return null;
  const store = createFirmwareHistory();
  const region = document.createElement('div'); region.className = 'firmware-history';
  const heading = document.createElement('div'); heading.className = 'firmware-history-heading';
  const label = document.createElement('label'); label.htmlFor = 'firmware-history-select'; label.textContent = '最近使用 · 当前设备';
  const clear = document.createElement('button'); clear.type = 'button'; clear.className = 'text-link'; clear.textContent = '清空记录';
  const select = document.createElement('select'); select.id = label.htmlFor;
  const detail = document.createElement('p'); detail.className = 'helper firmware-history-detail';
  heading.append(label, clear); region.append(heading, select, detail); if(mount)mount.append(region);else input.after(region);
  let entries = [];
  function render() {
    entries = store.list(productId()); region.hidden = !entries.length;
    select.replaceChildren(new Option('选择历史地址…', ''));
    entries.forEach((entry,index) => {
      const option = new Option(`${entry.name || firmwareHistoryName(entry.url)} · ${entry.version || '版本未知'} · ${firmwareHistoryAge(entry.lastUsedAt)}`, String(index));
      option.title = `${entry.url}\n${new Date(entry.lastUsedAt).toLocaleString('zh-CN')}`; select.append(option);
    });
    detail.textContent = store.persistent ? '地址记录仅保存在此浏览器；清空记录不影响固件缓存。' : '浏览器不允许保存记录，本次历史仅在当前页面有效。';
  }
  select.addEventListener('change', () => {
    if (select.value === '') return;
    const entry = entries[Number(select.value)]; if (!entry) return;
    input.value = entry.url; kindInput.value = entry.kind; shaInput.value = entry.sha256;
    detail.textContent = `${entry.name || firmwareHistoryName(entry.url)} · ${entry.version || '版本未知'} · ${entry.kind === 'manifest' ? '固件清单' : '完整镜像'} · ${new Date(entry.lastUsedAt).toLocaleString('zh-CN')}`;
    input.title = entry.url; onSelect?.(entry);
  });
  clear.addEventListener('click', () => { store.clear(productId()); render(); });
  select.addEventListener('focus', () => { const previous=select.value; render(); if ([...select.options].some(option=>option.value===previous)) select.value=previous; });
  window.addEventListener('storage', event => { if (!event.key || event.key === FIRMWARE_HISTORY_KEY) render(); });
  render();
  return {render, remember(entry) { const saved = store.remember(entry); render(); return saved; }};
}
