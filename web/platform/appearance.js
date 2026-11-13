// Presentation only: these finishes do not describe manufacturer material options.
export const SHELL_FINISHES = Object.freeze([
  {id: 'plastic', label: '哑光塑料'},
  {id: 'glossy-plastic', label: '亮面塑料'},
  {id: 'frosted-plastic', label: '磨砂半透明'},
  {id: 'clear-resin', label: '透明树脂'},
  {id: 'rubber', label: '橡胶'},
  {id: 'metal', label: '金属'},
  {id: 'brushed-metal', label: '拉丝金属'},
  {id: 'ceramic', label: '陶瓷'},
  {id: 'iridescent-titanium', label: '多彩钛金属'},
].map(Object.freeze));
export const isShellFinish = value => SHELL_FINISHES.some(finish => finish.id === value);
export const SHELL_DEFAULT = Object.freeze({color: '#080a0b', opacity: 1, finish: 'plastic'});
export const SHELL_PRESETS = Object.freeze({
  original: SHELL_DEFAULT,
  'black-clear': Object.freeze({color: '#171923', opacity: 0.32, finish: 'plastic'}),
  'purple-clear': Object.freeze({color: '#8456cf', opacity: 0.36, finish: 'plastic'}),
  gold: Object.freeze({color: '#d7ab52', opacity: 1, finish: 'metal'}),
  'titanium-spectrum': Object.freeze({color: '#dae2eb', opacity: 1, finish: 'iridescent-titanium'}),
  'titanium-midnight': Object.freeze({color: '#6682ae', opacity: 1, finish: 'iridescent-titanium'}),
  'titanium-rose': Object.freeze({color: '#e2b3b2', opacity: 1, finish: 'iridescent-titanium'}),
  'titanium-natural': Object.freeze({color: '#b8b3a8', opacity: 1, finish: 'brushed-metal'}),
  'anodized-ocean': Object.freeze({color: '#428aab', opacity: 1, finish: 'metal'}),
  'ceramic-pearl': Object.freeze({color: '#ede9df', opacity: 1, finish: 'ceramic'}),
});
export function shellPreset(shell) {
  return Object.entries(SHELL_PRESETS).find(([, value]) => value.color === shell.color.toLowerCase() && value.opacity === shell.opacity && value.finish === shell.finish)?.[0] || 'custom';
}

export const SHELL_PRESET_LABELS = Object.freeze({original:'原厂外观','black-clear':'黑透','purple-clear':'紫透',gold:'香槟金','titanium-spectrum':'幻彩钛','titanium-midnight':'深海钛','titanium-rose':'玫瑰钛','titanium-natural':'原钛拉丝','anodized-ocean':'海蓝阳极金属','ceramic-pearl':'珍珠陶瓷'});
