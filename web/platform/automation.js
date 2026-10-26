// Stable automation surface shared by coding agents and the human interface.
import { encodeScene } from './catalog.js';

export function installAutomation(runtime, { target = globalThis.window } = {}) {
  if (!target) throw new Error('Automation installation requires a window target.');
  const api = Object.freeze({
    ...runtime,
    async press(id, { durationMs = 100 } = {}) {
      if (!Number.isFinite(durationMs) || durationMs < 0 || durationMs > 10000) throw Object.assign(new Error('Press duration must be 0…10000 ms.'), { code: 'INVALID_INPUT' });
      await runtime.input({ type: 'button', id, pressed: true });
      try { await new Promise(resolve => setTimeout(resolve, durationMs)); }
      finally { await runtime.input({ type: 'button', id, pressed: false }); }
      return { accepted: true, id, durationMs };
    },
    async waitFor({ status, serialIncludes, port, fromNow = false, timeoutMs = 10000 } = {}) {
      if ((!status && typeof serialIncludes !== 'string') || !Number.isFinite(timeoutMs) || timeoutMs < 0 || timeoutMs > 120000) throw Object.assign(new Error('waitFor needs status or serialIncludes and a timeout of 0…120000 ms.'), { code: 'INVALID_WAIT' });
      const freshSerial = {};
      const matches = state => {
        const streams = fromNow ? freshSerial : state.serialByPort;
        return (!status || state.status === status) && (typeof serialIncludes !== 'string' || (port ? (streams[port] || '').includes(serialIncludes) : Object.values(streams).some(text => text.includes(serialIncludes))));
      };
      return new Promise((resolve, reject) => {
        let done = false, timer, lastState, unsubscribe = () => {};
        const finish = (err, value) => { if (done) return; done = true; clearTimeout(timer); unsubscribe(); err ? reject(err) : resolve(value); };
        const check = async () => {
          try {
            const state = await runtime.observe();
            lastState = state;
            if (matches(state)) finish(null, state);
            else if (['disposed', 'error'].includes(state.status)) finish(Object.assign(new Error(`Session entered ${state.status} before the expected condition.`), { code: 'WAIT_INTERRUPTED', details: state.lastError }));
          } catch (e) { finish(e); }
        };
        unsubscribe = runtime.subscribe(event => {
          if (event.type === 'serial') freshSerial[event.source] = ((freshSerial[event.source] || '') + event.data).slice(-65536);
          if (['serial', 'state', 'error'].includes(event.type)) void check();
        });
        timer = setTimeout(() => finish(Object.assign(new Error(`Timed out waiting for ${JSON.stringify({ status, serialIncludes, port, fromNow })}.`), {
          code: 'WAIT_TIMEOUT', details: { expected: { status, serialIncludes, port, fromNow }, status: lastState?.status, stats: lastState?.stats,
            serialTail: Object.fromEntries(Object.entries(lastState?.serialByPort || {}).map(([source, text]) => [source, text.slice(-1500)])) },
        })), timeoutMs);
        void check();
      });
    },
    async share({ mode, baseUrl = target.location?.href } = {}) {
      const state = await runtime.observe();
      if (!state.scene) throw Object.assign(new Error('Configure a scene before sharing.'), { code: 'NOT_CONFIGURED' });
      if (state.firmware?.source === 'local') throw Object.assign(new Error('Local firmware bytes cannot be included in a URL. Publish the firmware at a CORS-enabled URL or share its manifest first.'), { code: 'LOCAL_FIRMWARE_NOT_SHAREABLE' });
      if (mode !== undefined && !['studio', 'embed'].includes(mode)) throw Object.assign(new Error('Share mode must be studio or embed.'), { code: 'INVALID_INPUT' });
      const scene = structuredClone(state.scene);
      if (mode) scene.view.mode = mode;
      const url = new URL(mode === 'embed' ? 'embed.html' : mode === 'studio' ? 'index.html' : baseUrl, baseUrl);
      url.hash = `scene=${encodeScene(scene)}`;
      return url.href;
    },
  });
  Object.defineProperty(target, 'espTester', { value: api, configurable: true, enumerable: true, writable: false });
  return api;
}
