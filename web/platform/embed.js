const CHANNEL = 'esp-workbench';
const VERSION = '1';
const METHODS = new Set(['observe', 'capabilities', 'configure', 'updateSceneMetadata', 'loadFirmware', 'clearFirmwareCache', 'flushDeviceStorage', 'clearDeviceStorage', 'attachSdCard', 'clearSdCard', 'exportSdCard', 'flushSdCard', 'start', 'pause', 'reset', 'input', 'setGpsFix', 'injectGps', 'setImu', 'capture', 'waitFor', 'press', 'share', 'recordingStart', 'recordingStop', 'recordingPlay', 'recordingTakeOver', 'recordingObserve', 'recordingClear']);

// Only the direct embedding parent may establish one origin/session-bound connection.
// No broad window message listener is exposed as an unchecked runtime command endpoint.
export function installEmbedBridge(api, {target = window} = {}) {
  let origin = null, session = null, active = true, pending = 0;
  const seen = new Set();
  const idValid = value => typeof value === 'string' && /^[A-Za-z0-9_-]{1,80}$/.test(value);
  function reply(requestId, type, payload) {
    if (active && origin && target.parent !== target) target.parent.postMessage({channel: CHANNEL, version: VERSION, type, session, requestId, ...payload}, origin);
  }
  async function receive(event) {
    if (!active || target.parent === target || event.source !== target.parent) return;
    const message = event.data;
    if (!message || typeof message !== 'object' || message.channel !== CHANNEL || message.version !== VERSION || !idValid(message.requestId) || !idValid(message.session)) return;
    let parentUrl;
    try { parentUrl = new URL(event.origin); } catch { return; }
    if (!['https:', 'http:'].includes(parentUrl.protocol) || parentUrl.origin !== event.origin) return;
    if (message.type === 'hello') {
      if ((origin && event.origin !== origin) || (session && message.session !== session)) return;
      origin = event.origin; session = message.session;
      try { reply(message.requestId, 'ready', {result: {version: api.version, capabilities: await api.capabilities()}}); }
      catch (error) { reply(message.requestId, 'ready', {error: {code: error.code || 'BRIDGE_ERROR', message: error.message}}); }
      return;
    }
    if (!origin || event.origin !== origin || message.session !== session || message.type !== 'call') return;
    if (!METHODS.has(message.method) || typeof api[message.method] !== 'function') {
      reply(message.requestId, 'result', {error: {code: 'UNKNOWN_METHOD', message: 'This method is not exposed by the embed API.'}}); return;
    }
    if (!Array.isArray(message.args) || message.args.length > 4) {
      reply(message.requestId, 'result', {error: {code: 'INVALID_ARGUMENTS', message: 'args must be an array with at most four values.'}}); return;
    }
    if (seen.has(message.requestId)) {
      reply(message.requestId, 'result', {error: {code: 'DUPLICATE_REQUEST', message: 'Use a new requestId for each command.'}}); return;
    }
    if (pending >= 8) { reply(message.requestId, 'result', {error: {code: 'BUSY', message: 'At most eight embed requests may be pending.'}}); return; }
    seen.add(message.requestId); if (seen.size > 1024) seen.delete(seen.values().next().value);
    pending++;
    try { reply(message.requestId, 'result', {result: await api[message.method](...message.args)}); }
    catch (error) { reply(message.requestId, 'result', {error: {code: error.code || 'COMMAND_FAILED', message: error.message || String(error)}}); }
    finally { pending--; }
  }
  target.addEventListener('message', receive);
  return () => { active = false; target.removeEventListener('message', receive); seen.clear(); };
}
