import {isStickPlusBoard} from './board-compatibility.js';

export const POWER_PRODUCT_ID = 'm5stack-k016-p';
export const POWER_DEFAULT = Object.freeze({
  batteryVoltageUv: 4100000,
  batteryPresent: true,
  vbusVoltageUv: 0,
  vbusPresent: false,
  vbusValid: false,
  charging: false,
});
export const POWER_KEYS = Object.freeze(Object.keys(POWER_DEFAULT));
export const POWER_BOUNDS_UV = Object.freeze({
  batteryVoltageUv: Object.freeze([0, 5000000]),
  vbusVoltageUv: Object.freeze([0, 7000000]),
});
export const hasPowerEnvironment = product => product?.id === POWER_PRODUCT_ID && isStickPlusBoard(product.runtime?.board);
export const powerParameters = input => Object.fromEntries(POWER_KEYS.map(key => [key, input?.[key] ?? POWER_DEFAULT[key]]));
export function validPowerEnvironment(input) {
  if (!input || typeof input !== 'object' || Array.isArray(input) || Object.keys(input).length !== POWER_KEYS.length) return false;
  return POWER_KEYS.every(key => {
    if (!Object.hasOwn(input, key)) return false;
    const bounds = POWER_BOUNDS_UV[key];
    return bounds ? Number.isInteger(input[key]) && input[key] >= bounds[0] && input[key] <= bounds[1] : typeof input[key] === 'boolean';
  });
}
export const validPowerScene = value => !!value && typeof value === 'object' && !Array.isArray(value)
  && Object.keys(value).length === 1 && Object.hasOwn(value, 'power') && validPowerEnvironment(value.power);
export const powerMessage = input => ({t: 'power-environment', ...powerParameters(input)});
