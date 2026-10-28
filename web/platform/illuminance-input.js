export const DEFAULT_ILLUMINANCE=1000;
export const validIlluminance=lux=>Number.isFinite(lux)&&lux>=0&&lux<=100000;
export const illuminanceParameter=parameters=>parameters?.lux===undefined?DEFAULT_ILLUMINANCE:parameters.lux;
