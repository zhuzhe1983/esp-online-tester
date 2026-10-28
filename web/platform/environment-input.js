// U001-D shared ambient input; BMP280 limits the common temperature range to 85 C.
export const ENVIRONMENT_FIELDS=[['temperatureC','温度 (°C)',-40,85],['humidityPercent','相对湿度 (%)',0,100],['pressureHpa','气压 (hPa)',300,1100]];
export const DEFAULT_ENVIRONMENT=Object.freeze({temperatureC:25,humidityPercent:50,pressureHpa:1013.25});
export const validEnvironment=value=>!!value&&ENVIRONMENT_FIELDS.every(([key,,min,max])=>Number.isFinite(value[key])&&value[key]>=min&&value[key]<=max);
export const environmentParameters=parameters=>Object.fromEntries(ENVIRONMENT_FIELDS.map(([key])=>[key,parameters?.[key]===undefined?DEFAULT_ENVIRONMENT[key]:parameters[key]]));
