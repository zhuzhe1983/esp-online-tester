import {createBrowserAudio} from './audio.js';

const microphoneIcon = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.7" aria-hidden="true"><rect x="9" y="3" width="6" height="12" rx="3"/><path d="M5 11v1a7 7 0 0 0 14 0v-1M12 19v3m-4 0h8"/><path class="audio-icon-slash" d="m3 3 18 18"/></svg>';
const speakerIcon = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.7" aria-hidden="true"><path d="M11 4 6 8H3v8h3l5 4V4Zm4 4a6 6 0 0 1 0 8m3-11a10 10 0 0 1 0 14"/><path class="audio-icon-slash" d="m3 3 18 18"/></svg>';

export function mountAudioControls({container, runtime, createAudio = createBrowserAudio} = {}) {
  if (!container || !runtime) throw new Error('Audio controls require a container and runtime.');
  const document = container.ownerDocument;
  container.classList.add('browser-audio-controls');
  container.setAttribute('role','group'); container.setAttribute('aria-label','麦克风与扬声器');
  const button = (kind, icon, label) => {
    const node = document.createElement('button'); node.type = 'button'; node.className = 'icon-button audio-toggle';
    node.dataset.audio = kind; node.dataset.state = 'off'; node.innerHTML = icon;
    node.setAttribute('aria-label',label); node.setAttribute('aria-pressed','false'); node.title = label; node.disabled = true;
    container.append(node); return node;
  };
  const microphone = button('microphone',microphoneIcon,'开启麦克风'), speaker = button('speaker',speakerIcon,'开启扬声器');
  const message = document.createElement('span'); message.className = 'browser-audio-status'; message.setAttribute('role','status'); message.setAttribute('aria-live','polite'); message.hidden = true; container.append(message);
  let destroyed = false, status = 'idle', caps = null, current = null, readGeneration = 0;
  const supported = kind => caps?.audio?.[kind === 'microphone' ? 'input' : 'output']?.available !== false;
  function render(state) {
    current = state;
    for (const [kind,node,name] of [['microphone',microphone,'麦克风'],['speaker',speaker,'扬声器']]) {
      const on = state[kind] === 'on', starting = state[kind] === 'starting', error = state[`${kind}Error`];
      const usable = kind === 'microphone' ? status === 'running' : ['ready','running','paused'].includes(status);
      const label = `${on || starting ? '关闭' : '开启'}${name}`;
      node.dataset.state = state[kind]; node.setAttribute('aria-pressed',String(on)); node.setAttribute('aria-busy',String(starting));
      node.setAttribute('aria-label',label); node.disabled = !supported(kind) || !usable && !on && !starting;
      node.title = error ? `${name}：${error.message}` : !supported(kind) ? `此设备尚不支持${name}` : !usable ? `运行固件后开启${name}` : starting ? `正在开启${name}，再次点击可取消` : label;
    }
    const error = state.microphoneError || state.speakerError;
    const explanations = {
      NotAllowedError:'浏览器未允许麦克风访问，请检查权限。', NotFoundError:'没有检测到可用的麦克风。',
      NotReadableError:'麦克风无法打开，可能正被其他应用占用。',
      AUDIO_INPUT_FULL:'模拟器来不及接收音频，麦克风已停止。可使用 PCM 文件进行离线测试。',
      AUDIO_INPUT_OVERFLOW:'模拟器来不及接收音频，麦克风已停止。可使用 PCM 文件进行离线测试。',
      AUDIO_OUTPUT_OVERFLOW:'播放队列已满，扬声器已停止。可导出 PCM 进行离线试听。',
      MICROPHONE_ENDED:'麦克风访问已结束。', MICROPHONE_UNAVAILABLE:'请使用 HTTPS 或 localhost，并允许浏览器访问麦克风。',
      AUDIO_UNAVAILABLE:'当前浏览器不支持 AudioWorklet 音频处理。', AUDIO_PROCESSOR_ERROR:'浏览器音频处理已停止，请重新开启。'
    };
    let text = error ? explanations[error.code] || error.message : '';
    if (!text && state.speaker === 'on' && state.underflows) text = `音频供给中断 ${state.underflows} 次；保持原始语速。`;
    message.textContent = text; message.hidden = !text; message.dataset.error = String(!!error);
  }
  let audio;
  try { audio = createAudio({runtime,onState:state => { if (!destroyed) render(state); }}); }
  catch (error) {
    message.textContent = error.message; message.hidden = false; message.dataset.error = 'true';
    return {observe:() => ({microphone:'unavailable',speaker:'unavailable'}),async dispose(){container.replaceChildren();}};
  }
  render(audio.observe());
  const micClick = () => { void audio.setMicrophone(!['on','starting'].includes(audio.observe().microphone)).catch(() => {}); };
  const speakerClick = () => { void audio.setSpeaker(!['on','starting'].includes(audio.observe().speaker)).catch(() => {}); };
  microphone.addEventListener('click',micClick); speaker.addEventListener('click',speakerClick);
  async function refresh() {
    const generation = ++readGeneration;
    try {
      const [state, capabilities] = await Promise.all([runtime.observe(),runtime.capabilities()]);
      if (destroyed || generation !== readGeneration) return;
      status = state.status; caps = capabilities; render(current);
    } catch { /* Runtime teardown will disable controls through its state event. */ }
  }
  const unsubscribe = runtime.subscribe(event => {
    if (event.type === 'state') { readGeneration++; status = event.state?.status || 'idle'; render(audio.observe()); if (status === 'configured') void refresh(); }
  });
  const leave = () => { void audio.reset(); };
  document.defaultView?.addEventListener('pagehide',leave);
  void refresh();
  return {...audio,async dispose() {
    if (destroyed) return; destroyed = true; unsubscribe(); readGeneration++;
    document.defaultView?.removeEventListener('pagehide',leave);
    microphone.removeEventListener('click',micClick); speaker.removeEventListener('click',speakerClick);
    await audio.dispose(); container.replaceChildren();
  }};
}
