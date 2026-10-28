// Plain-text catalog facts become compact, accessible controls; no catalog HTML is executed.
const icons = {
  chip: ['M8 3v3m8-3v3M8 18v3m8-3v3M3 8h3m12 0h3M3 16h3m12 0h3', 'M6 6h12v12H6z', 'M9 9h6v6H9z'],
  display: ['M3 4h18v13H3z', 'M8 21h8m-4-4v4'],
  plug: ['M8 3v5m8-5v5', 'M6 8h12v3a6 6 0 0 1-6 6v4', 'M5 8h14'],
  cloud: ['M6 17a4 4 0 0 1-1-7.9A7 7 0 0 1 18.5 8a4.5 4.5 0 0 1 .5 9', 'M12 11v10m-4-4 4 4 4-4'],
  check: ['M12 3 3 7v5c0 5 9 9 9 9s9-4 9-9V7z', 'm8 12 3 3 5-6'],
  clock: ['M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18', 'M12 7v5l3 2'],
  hardware: ['M6 3h12v18H6z', 'M12 7v6m0 3v.1'],
  info: ['M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18', 'M12 10v7m0-10v.1'],
};
function groupFacts(facts, rules, fallback) {
  const groups = new Map();
  for (const fact of facts || []) {
    const rule = rules.find(item => item.test.test(fact)) || fallback;
    if (!groups.has(rule.id)) groups.set(rule.id, {...rule, paragraphs: []});
    groups.get(rule.id).paragraphs.push(fact);
  }
  return [...groups.values()].map(({test, ...item}) => item);
}
export function productHints(product) {
  const unavailableLabel = product.model ? '仅外观预览' : '资料与配置';
  const state = {partial: '部分硬件模型，按固件验证', unavailable: product.model ? '外观预览，板级模型待实现' : '产品资料，外观与板级模型待实现', diagnostic: '通用执行基准，无产品外设'}[product.runtime.status] || product.runtime.status;
  const hints = groupFacts(product.limitations, [
    {id: 'display', icon: 'display', label: '显示模型', title: '显示与帧缓冲', test: /帧缓冲|列偏移|可视屏幕/},
    {id: 'interfaces', icon: 'plug', label: '接口约束', title: '接口与硬件版本', test: /接线|引脚|G10\/G11|硬件.*v1/},
  ], {id: 'scope', icon: 'chip', label: product.runtime.status === 'unavailable' ? unavailableLabel : '模拟范围', title: state});
  if (!hints.some(item => item.id === 'scope')) hints.unshift({id: 'scope', icon: 'chip', label: '模拟范围', title: state, paragraphs: [state]});
  return hints;
}
export function firmwareHints(firmware) {
  if (!firmware) return [{id: 'choose', icon: 'info', label: '选择固件', title: '固件来源', paragraphs: ['选择已收录固件，或加载自己的远程镜像。']}];
  return groupFacts(firmware.limitations, [
    {id: 'source', icon: 'cloud', label: '远程加载', title: '下载与校验', test: /官方 CDN|不持久缓存|远程下载/},
    {id: 'verified', icon: 'check', label: '已验证操作', title: '已验证的模拟操作', test: /^已验证|^已通过/},
    {id: 'timing', icon: 'clock', label: '输入时序', title: '输入与回放时序', test: /主机输入|客户机时序|逐帧一致/},
    {id: 'hardware', icon: 'hardware', label: '未实机验证', title: '实体硬件验证范围', test: /未连接真实硬件|实机.*仍需验证/},
  ], {id: 'limits', icon: 'chip', label: !firmware.source ? '资料入口' : firmware.simulationVerified === true ? '模拟限制' : '可尝试运行', title: '固件兼容范围'});
}

export function placeHint(anchor, size, viewport, {padding = 12, gap = 8} = {}) {
  const leftEdge = (viewport.left || 0) + padding, topEdge = (viewport.top || 0) + padding;
  const rightEdge = (viewport.left || 0) + viewport.width - padding, bottomEdge = (viewport.top || 0) + viewport.height - padding;
  const width = Math.min(size.width, Math.max(1, viewport.width - padding * 2)), height = Math.min(size.height, Math.max(1, viewport.height - padding * 2));
  const left = Math.max(leftEdge, Math.min(anchor.left, rightEdge - width));
  const below = anchor.bottom + gap, above = anchor.top - gap - height;
  const top = Math.max(topEdge, Math.min(below + height <= bottomEdge || above < topEdge ? below : above, bottomEdge - height));
  return {left, top, width, maxHeight: Math.max(1, viewport.height - padding * 2)};
}

let sequence = 0, active = null, timer = null, installed = false;
const lists = new WeakMap();
function closeHint() {
  clearTimeout(timer); timer = null;
  if (active) { active.popup.hidden = true; active.button.setAttribute('aria-expanded', 'false'); active.pinned = false; active.hovered = false; }
  active = null;
}
function positionHint() {
  if (!active) return;
  const {button, popup} = active, rect = button.getBoundingClientRect(), vv = window.visualViewport;
  const viewport = {left: vv?.offsetLeft || 0, top: vv?.offsetTop || 0, width: vv?.width || innerWidth, height: vv?.height || innerHeight};
  if (!button.isConnected || rect.bottom < viewport.top || rect.top > viewport.top + viewport.height) { closeHint(); return; }
  popup.style.width = `${Math.min(326, Math.max(1, viewport.width - 24))}px`;
  popup.style.maxHeight = `${Math.max(1, viewport.height - 24)}px`;
  const placement = placeHint(rect, popup.getBoundingClientRect(), viewport);
  popup.style.left = `${placement.left}px`; popup.style.top = `${placement.top}px`;
}
function showHint(item) {
  clearTimeout(timer); timer = null;
  if (active !== item) { closeHint(); active = item; item.pinned = false; }
  item.popup.hidden = false; item.button.setAttribute('aria-expanded', 'true'); positionHint();
}
function deferClose(item) {
  clearTimeout(timer); timer = setTimeout(() => {
    if (active === item && !item.pinned && !item.hovered && document.activeElement !== item.button) closeHint();
  }, 180);
}
function install() {
  if (installed) return; installed = true;
  document.addEventListener('pointerdown', event => { if (active && !active.button.contains(event.target) && !active.popup.contains(event.target)) closeHint(); }, true);
  document.addEventListener('keydown', event => {
    if (!active) return;
    if (event.key === 'Escape') { closeHint(); event.preventDefault(); event.stopPropagation(); }
    else if (['PageDown', 'PageUp'].includes(event.key) && active.popup.scrollHeight > active.popup.clientHeight) { active.popup.scrollBy(0, active.popup.clientHeight * (event.key === 'PageDown' ? .8 : -.8)); event.preventDefault(); event.stopPropagation(); }
  }, true);
  window.addEventListener('resize', positionHint);
  document.addEventListener('scroll', event => { if (!active?.popup.contains(event.target)) positionHint(); }, true);
  window.visualViewport?.addEventListener('resize', positionHint); window.visualViewport?.addEventListener('scroll', positionHint);
  window.addEventListener('blur', closeHint); window.addEventListener('pagehide', closeHint);
}
function icon(name) {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  for (const [key, value] of Object.entries({viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', 'stroke-width': '1.6', 'stroke-linecap': 'round', 'stroke-linejoin': 'round', 'aria-hidden': 'true', focusable: 'false'})) svg.setAttribute(key, value);
  for (const d of icons[name] || icons.info) { const path = document.createElementNS(svg.namespaceURI, 'path'); path.setAttribute('d', d); svg.append(path); }
  return svg;
}
export function renderHintList(container, hints) {
  install();
  for (const item of lists.get(container) || []) { if (active === item) closeHint(); item.popup.remove(); }
  container.replaceChildren(); const items = [];
  for (const hint of hints) {
    const button = document.createElement('button'); button.type = 'button'; button.className = 'hint-trigger'; button.dataset.hint = hint.id;
    const label = document.createElement('span'); label.textContent = hint.label; button.append(icon(hint.icon), label);
    const popup = document.createElement('div'); popup.className = 'hint-popover'; popup.id = `catalog-hint-${++sequence}`; popup.hidden = true; popup.setAttribute('role', 'tooltip');
    const title = document.createElement('strong'); title.textContent = hint.title; popup.append(title);
    for (const paragraph of hint.paragraphs) { const p = document.createElement('p'); p.textContent = paragraph; popup.append(p); }
    button.setAttribute('aria-describedby', popup.id); button.setAttribute('aria-controls', popup.id); button.setAttribute('aria-expanded', 'false');
    const item = {button, popup, pinned: false, hovered: false}; items.push(item);
    button.addEventListener('pointerenter', event => { if (event.pointerType === 'touch') return; item.hovered = true; showHint(item); });
    button.addEventListener('pointerleave', () => { item.hovered = false; deferClose(item); });
    button.addEventListener('focus', () => showHint(item)); button.addEventListener('blur', () => deferClose(item));
    button.addEventListener('click', () => { if (active === item && item.pinned) closeHint(); else { showHint(item); item.pinned = true; } });
    popup.addEventListener('pointerenter', () => { item.hovered = true; clearTimeout(timer); });
    popup.addEventListener('pointerleave', () => { item.hovered = false; deferClose(item); });
    container.append(button); document.body.append(popup);
  }
  lists.set(container, items);
}
