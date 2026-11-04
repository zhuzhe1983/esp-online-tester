import {hasMotorOutput,STAMPFLY_MOTORS} from './motor-state.js';
export const FLIGHT_MODEL='stampfly-rigid-body-v1';
export const validFlightConfig=value=>!!value&&typeof value==='object'&&!Array.isArray(value)&&Object.keys(value).every(k=>['enabled','model'].includes(k))&&typeof value.enabled==='boolean'&&value.model===FLIGHT_MODEL;
export const hasFlight=hasMotorOutput;
const vector=(v,n)=>Array.isArray(v)&&v.length===n&&v.every(Number.isFinite);
export function parseFlightSnapshot(s){
  if(!s||s.enabled!==true||s.model!==FLIGHT_MODEL||s.calibrated!==false||s.source!=='guest-pwm-rigid-body'||s.worldFrame!=='NED'||s.bodyFrame!=='FRD'||
    !Number.isSafeInteger(s.epoch)||s.epoch<0||!Number.isSafeInteger(s.cycle)||s.cycle<0||s.guestTime!==s.cycle/240000000||typeof s.contact!=='boolean'||
    !vector(s.positionM,3)||!vector(s.velocityMps,3)||!vector(s.quaternionXyzw,4)||Math.abs(Math.hypot(...s.quaternionXyzw)-1)>1e-6||!vector(s.angularVelocityDpsFRD,3)||s.heightM!==-s.positionM[2]||
    !Array.isArray(s.motors)||s.motors.length!==4||!vector(s.sensors?.rawAccelG,3)||!vector(s.sensors?.rawGyroDps,3)||
    [s.sensors.bottomDistanceM,s.sensors.frontDistanceM].some(x=>x!==null&&(!Number.isFinite(x)||x<.05||x>3)))return null;
  for(let i=0;i<4;i++){const m=s.motors[i],definition=STAMPFLY_MOTORS[i];if(!m||m.id!==definition.id||m.gpio!==definition.gpio||!Number.isFinite(m.duty)||m.duty<0||m.duty>1||!Number.isFinite(m.frequencyHz)||m.frequencyHz<0||m.frequencyHz>240000000||m.frequencyHz===0&&m.duty!==0||![m.omegaRadS,m.thrustN].every(v=>Number.isFinite(v)&&v>=0))return null;}
  return structuredClone(s);
}
