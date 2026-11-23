// Source stimulus for the emulated Lepton sensor and guest processing.
export const THERMAL_PRODUCT_ID = 'm5stack-k016-t2';
export const THERMAL_FIELDS = [
  ['background', '背景 Raw14', 1, 16383],
  ['hotspot', '热点 Raw14', 1, 16383],
  ['centerX', '热点 X', 0, 159],
  ['centerY', '热点 Y', 0, 119],
  ['radius', '半径', 0, 160],
];
export const DEFAULT_THERMAL = Object.freeze({background: 8000, hotspot: 9000, centerX: 80, centerY: 60, radius: 12});
export const hasThermalEnvironment = product => product?.id === THERMAL_PRODUCT_ID && product.runtime?.board === 'stickt2';
export const thermalParameters = input => Object.fromEntries(THERMAL_FIELDS.map(([key]) => [key, input?.[key] ?? DEFAULT_THERMAL[key]]));
export function validThermal(input) {
  if (!input || typeof input !== 'object' || Array.isArray(input) || Object.keys(input).length !== THERMAL_FIELDS.length) return false;
  return THERMAL_FIELDS.every(([key, , min, max]) => Object.hasOwn(input, key) && Number.isInteger(input[key]) && input[key] >= min && input[key] <= max);
}
