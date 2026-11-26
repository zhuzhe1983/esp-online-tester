// Sensor-native raw ADC stimulus; neither profile receives host temperatures.
export const THERMAL_PRODUCT_ID = 'm5stack-k016-t2';
export const MLX_THERMAL_PRODUCT_ID = 'm5stack-u149';
export const THERMAL_PRODUCT_IDS = [THERMAL_PRODUCT_ID,MLX_THERMAL_PRODUCT_ID,'m5stack-k016-t','m5stack-k126'];
export const isThermalProductId=id=>THERMAL_PRODUCT_IDS.includes(id);
export const THERMAL_FIELDS = [
  ['background', '背景原始计数', 1, 16383],
  ['hotspot', '热点原始计数', 1, 16383],
  ['centerX', '热点 X', 0, 159],
  ['centerY', '热点 Y', 0, 119],
  ['radius', '半径', 0, 160],
];
const MLX_FIELDS=[['background','背景 ADC',0,32767],['hotspot','热点 ADC',0,32767],['centerX','热点 X',0,31],['centerY','热点 Y',0,23],['radius','半径',0,32]];
export const DEFAULT_THERMAL = Object.freeze({background:8000,hotspot:9000,centerX:80,centerY:60,radius:12});
const MLX_DEFAULT=Object.freeze({background:1000,hotspot:2000,centerX:16,centerY:12,radius:4});
const productId=product=>typeof product==='string'?product:product?.id;
export const thermalFields=product=>productId(product)===MLX_THERMAL_PRODUCT_ID?MLX_FIELDS:THERMAL_FIELDS;
export const thermalDefaults=product=>productId(product)===MLX_THERMAL_PRODUCT_ID?MLX_DEFAULT:DEFAULT_THERMAL;
export const hasThermalEnvironment = product => isThermalProductId(product?.id) && ({'m5stack-k016-t2':'stickt2','m5stack-k016-t':'stickt-original','m5stack-k126':'tlite','m5stack-u149':'unit-thermal2'})[product.id]===product.runtime?.board;
export const thermalParameters = (input,product) => Object.fromEntries(thermalFields(product).map(([key])=>[key,input?.[key]??thermalDefaults(product)[key]]));
export function validThermal(input,product) {
  const fields=thermalFields(product);
  if(!input||typeof input!=='object'||Array.isArray(input)||Object.keys(input).length!==fields.length)return false;
  return fields.every(([key,,min,max])=>Object.hasOwn(input,key)&&Number.isInteger(input[key])&&input[key]>=min&&input[key]<=max);
}
