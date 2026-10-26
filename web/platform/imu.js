// Virtual sensor axes: +X model right, +Y model top, +Z out of the display.
// This registration is explicit, not a claim about the unmeasured PCB mounting.
// Official StopWatch HAL swaps sensor X/Y for its labels; firmware performs that swap.
const invalid = message => Object.assign(new Error(message), {code: 'INVALID_IMU'});
export function normalizeOrientation(value) {
  if (!Array.isArray(value) || value.length !== 4 || !value.every(Number.isFinite)) throw invalid('Orientation must be a finite [x, y, z, w] quaternion.');
  const length = Math.hypot(...value);
  if (!Number.isFinite(length) || length < 1e-9) throw invalid('Orientation quaternion must have a finite nonzero magnitude.');
  return value.map(v => v / length);
}
const conjugate = q => [-q[0], -q[1], -q[2], q[3]];
const multiply = ([x,y,z,w], [a,b,c,d]) => [w*a+x*d+y*c-z*b,w*b-x*c+y*d+z*a,w*c+x*b-y*a+z*d,w*d-x*a-y*b-z*c];
export function orientationSample(quaternion, {previous, elapsedSeconds, angularVelocityDps} = {}) {
  const q = normalizeOrientation(quaternion);
  // A stationary accelerometer measures support force, opposite gravity (world -Y).
  const accelG = multiply(multiply(conjugate(q), [0,1,0,0]), q).slice(0,3);
  let gyroDps = [0,0,0];
  if (angularVelocityDps !== undefined) {
    if (!Array.isArray(angularVelocityDps) || angularVelocityDps.length !== 3 || !angularVelocityDps.every(Number.isFinite)) throw invalid('Angular velocity must be three finite degrees/second values.');
    gyroDps = angularVelocityDps.map(v => Math.max(-2000,Math.min(2000,v)));
  } else if (previous && elapsedSeconds > 0 && elapsedSeconds <= .25) {
    let delta = multiply(conjugate(normalizeOrientation(previous)), q);
    if (delta[3] < 0) delta = delta.map(v => -v);
    const length = Math.hypot(...delta.slice(0,3));
    if (length > 1e-9) {
      const factor = 2 * Math.atan2(length, delta[3]) * 180 / Math.PI / elapsedSeconds / length;
      gyroDps = delta.slice(0,3).map(v => Math.max(-2000,Math.min(2000,v * factor)));
    }
  }
  return {quaternion:q, accelG, gyroDps};
}
