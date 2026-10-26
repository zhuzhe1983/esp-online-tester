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
].map(Object.freeze));
export const isShellFinish = value => SHELL_FINISHES.some(finish => finish.id === value);
export const SHELL_DEFAULT = Object.freeze({color: '#080a0b', opacity: 1, finish: 'plastic'});
export const SHELL_PRESETS = Object.freeze({
  original: SHELL_DEFAULT,
  'black-clear': Object.freeze({color: '#171923', opacity: 0.32, finish: 'plastic'}),
  'purple-clear': Object.freeze({color: '#8456cf', opacity: 0.36, finish: 'plastic'}),
  gold: Object.freeze({color: '#d7ab52', opacity: 1, finish: 'metal'}),
});
export function shellPreset(shell) {
  return Object.entries(SHELL_PRESETS).find(([, value]) => value.color === shell.color.toLowerCase() && value.opacity === shell.opacity && value.finish === shell.finish)?.[0] || 'custom';
}
