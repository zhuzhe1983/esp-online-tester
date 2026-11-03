// Cold observations of StampFly's four wired motor PWM outputs; never commands.
export const STAMPFLY_MOTORS = Object.freeze([
  Object.freeze({id:'front-left',gpio:5}), Object.freeze({id:'front-right',gpio:42}),
  Object.freeze({id:'rear-left',gpio:10}), Object.freeze({id:'rear-right',gpio:41}),
]);
export const hasMotorOutput = board => ['stampfly','stampfly-v11'].includes(board);
export function parseMotorSnapshot(value) {
  if(value?.type!=='stampfly-motors'||value.model!=='latched-pwm'||value.flightSimulated!==false||value.clockHz!==240000000||
    !Number.isSafeInteger(value.cycle)||value.cycle<0||!Number.isSafeInteger(value.version)||value.version<0||
    !Array.isArray(value.motors)||value.motors.length!==4)return null;
  const motors=[];
  for(const {id,gpio} of STAMPFLY_MOTORS) {
    const matches=value.motors.filter(m=>m?.id===id),motor=matches[0];
    if(matches.length!==1||motor.gpio!==gpio||!Number.isFinite(motor.duty)||motor.duty<0||motor.duty>1||
      !Number.isFinite(motor.frequencyHz)||motor.frequencyHz<0||motor.frequencyHz>240000000||motor.frequencyHz===0&&motor.duty!==0)return null;
    motors.push({id,gpio,duty:motor.duty,frequencyHz:motor.frequencyHz});
  }
  return {motors,motorOutput:{version:value.version,cycle:value.cycle,clockHz:value.clockHz,guestTime:value.cycle/value.clockHz,model:value.model,flightSimulated:false}};
}
