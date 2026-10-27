// Only a successfully restored shared scene may start on page entry. A malformed
// link must never fall through to running the workbench's default firmware.
export function sharedStartup(params, {restored, product, recording, invalidRecording = false}) {
  if (!restored || params.get('autostart') === '0') return 'manual';
  if (!product?.runtime?.board) return 'unavailable';
  if (!restored.firmware) return 'missing-firmware';
  if (invalidRecording && params.get('autoplay') !== '0') return 'invalid-recording';
  return recording && params.get('autoplay') !== '0' ? 'replay' : 'run';
}

export function setSharedPlayback(params, {autostart = true, autoplay = true} = {}) {
  if (typeof autostart !== 'boolean' || typeof autoplay !== 'boolean') throw Object.assign(new Error('autostart and autoplay must be booleans.'), {code: 'INVALID_INPUT'});
  params.set('autostart', autostart ? '1' : '0');
  params.set('autoplay', autostart && autoplay ? '1' : '0');
  return params;
}
