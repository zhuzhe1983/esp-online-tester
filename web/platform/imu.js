// Virtual sensor axes: +X model right, +Y model top, +Z out of the display.
// This registration is explicit, not a claim about the unmeasured PCB mounting.
// Official StopWatch HAL swaps sensor X/Y for its labels; firmware performs that swap.
export const imuModelForBoard = board => ({stopwatch:'bmi270-register-subset',corematrix:'bmi270-register-subset',atoms3:'mpu6886-register-subset','atom-matrix':'mpu6886-register-subset','atom-matrix-v11':'bmi270-register-subset',atoms3r:'bmi270-register-subset','atoms3r-chatbot':'bmi270-register-subset',papers3:'bmi270-register-subset',stampfly:'bmi270-register-subset','stampfly-v11':'bmi270-register-subset'}[board] || null);

// UI choice is deliberately ephemeral, separate from the agent IMU API and
// shared scene. At most one request runs and one latest sample waits for it.
export function createSceneImuLink({getContext, send, onError = () => {}}) {
  let enabled = false, activate = false, generation = 0, pending = null, draining = null, motion = false, motionSession;
  const context = () => {
    const c = getContext();
    const reason = !c.available ? 'unavailable' : c.flight ? 'flight' : c.preview ? 'preview' : c.recording ? 'recording' : c.mode !== '3d' ? '2d' : c.status !== 'running' ? 'not-running' : null;
    return {...c, reason};
  };
  function drain() {
    if (draining) return draining;
    draining = (async () => {
      while (pending) {
        const item = pending; pending = null;
        const c = context();
        if (item.session !== c.session || c.flight) { motion = false; continue; }
        if (!item.terminal && (item.generation !== generation || !enabled || c.reason || c.hidden)) continue;
        if (item.terminal && (!motion || motionSession !== c.session || !['running','paused'].includes(c.status))) continue;
        try {
          motion = !item.terminal && item.rotating; motionSession = c.session;
          const activation = activate; activate = false;
          await send(item.terminal ? {source:'scene', angularVelocityDps:[0,0,0]} : {source:'scene', ...(activation ? {enabled:true} : {}), quaternion:item.quaternion, ...(item.rotating ? {} : {angularVelocityDps:[0,0,0]})});
        } catch (error) { if (!['SESSION_REPLACED','FLIGHT_OWNS_IMU'].includes(error.code)) onError(error); }
      }
    })().finally(() => { draining = null; if (pending) void drain(); });
    return draining;
  }
  function stop() {
    generation++; pending = null;
    if (motion) { pending = {terminal:true,session:motionSession}; return drain(); }
    return draining || Promise.resolve();
  }
  return {
    get enabled() { return enabled; },
    state() { const c = context(); return {enabled, available:c.available, reason:c.reason}; },
    sync({reset = false} = {}) {
      if (reset || context().reason) { const active = enabled || motion || pending; enabled = false; if (active) void stop(); }
      return this.state();
    },
    toggle() { enabled = !enabled && !context().reason; activate = enabled; if (!enabled) void stop(); return enabled; },
    // An explicit agent sample takes ownership; do not zero its supplied gyro.
    invalidate() { enabled = activate = motion = false; generation++; pending = null; },
    offer({quaternion, rotating = false}) {
      const c = context();
      if (!enabled || c.reason || c.hidden) return Promise.resolve();
      pending = {quaternion:normalizeOrientation(quaternion),rotating,session:c.session,generation};
      return drain();
    },
    stop,
    idle() { return draining || Promise.resolve(); },
  };
}
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
