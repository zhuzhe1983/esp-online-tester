import {browserFlashProfiles} from './flash-profiles.js';
import {SIMULATOR_UIFLOW2_PROFILE} from './simulation-network.js';
import {t} from './i18n.js';

// An explicit virtual Flash operation. Merely selecting a profile never writes it.
export function mountVirtualConfiguration({container,runtime,run=fn=>fn(),onError=console.error,profiles=browserFlashProfiles()}={}) {
  if(!container)return {dispose(){}};
  const style=document.createElement('link');style.rel='stylesheet';style.href=new URL('./virtual-configuration-ui.css',import.meta.url).href;document.head.append(style);
  container.className='virtual-configuration';
  container.innerHTML='<details><summary>虚拟写入配置</summary><div class="virtual-config-body"><p class="helper">将 UIFlow2 配置写入当前模拟器；不会烧录连接到电脑的设备。</p><label>使用配置<select aria-label="虚拟写入使用配置"></select></label><p data-profile class="helper"></p><p class="helper">会替换当前设备的 NVS 设置，保留固件程序和 TF 文件。仅连接网络模式不加载 UIFlow 云菜单。</p><button type="button" class="button" data-write disabled>写入并重启</button><p data-result class="helper" role="status"></p></div></details>';
  const $=s=>container.querySelector(s),select=$('select'),write=$('[data-write]'),details=$('details');
  let alive=true,state=null,inspection=null,pending=false,epoch=0,values=[];
  const selected=()=>values.find(p=>p.id===select.value);
  const usable=()=>state&&['ready','running','paused'].includes(state.status)&&!!state.firmware;
  const render=()=>{
    if(!alive)return;write.disabled=pending||!usable()||!inspection?.available||!selected();select.disabled=pending;
    const p=selected();$('[data-profile]').textContent=p?`${p.builtin?t(p.name):p.name} · Wi-Fi：${p.ssid} · ${p.bootOption===2?'仅连接网络':'UIFlow 菜单'}`:'';
  };
  const choices=()=>{const previous=select.value;values=profiles.list().filter(p=>p.type==='uiflow2');select.replaceChildren(...values.map(p=>{const o=document.createElement('option');o.value=p.id;o.setAttribute('data-i18n-skip','');o.textContent=p.builtin?t(p.name):p.name;return o;}));select.value=values.some(p=>p.id===previous)?previous:SIMULATOR_UIFLOW2_PROFILE.id;render();};
  const inspect=async({keepMessage=false}={})=>{
    const token=++epoch;inspection=null;render();
    if(!usable()){$('[data-result]').textContent='加载 UIFlow2 固件后，可选择配置并写入。';return;}
    try{const result=await runtime.inspectFirmwareConfiguration();if(!alive||token!==epoch)return;inspection=result;if(!keepMessage)$('[data-result]').textContent=result.available?'点击写入后应用所选配置。':result.reason?.message||'当前固件不支持此配置格式。';render();}
    catch(e){if(alive&&token===epoch){$('[data-result]').textContent=e.message;render();}}
  };
  const update=next=>{state=next;if(!usable()){epoch++;inspection=null;render();if(details.open)void inspect();}else if(details.open&&!pending)void inspect();else render();};
  details.addEventListener('toggle',()=>{if(details.open){choices();void inspect();}});
  select.addEventListener('change',()=>{$('[data-result]').textContent='点击写入后应用所选配置。';render();});
  write.addEventListener('click',()=>{
    if(write.disabled)return;const profile=structuredClone(selected()),sessionId=inspection.sessionId;pending=true;render();$('[data-result]').textContent='正在写入虚拟配置…';
    Promise.resolve().then(()=>run(async()=>{
      const written=await runtime.writeFirmwareConfiguration(profile,{sessionId}),current=await runtime.observe();
      if(current.status!=='ready'||current.product?.id!==written.product?.id||JSON.stringify(current.firmware?.artifacts)!==JSON.stringify(written.firmware?.artifacts))throw new Error('设备或固件已改变，未自动启动。');
      await runtime.start();
      if(alive)$('[data-result]').textContent='配置已写入，模拟器已重新启动。';
    })).catch(e=>{if(alive){$('[data-result]').textContent=e.message;onError(e);}}).finally(()=>{pending=false;if(alive)void inspect({keepMessage:true});});
  });
  const unsubscribe=runtime.subscribe(event=>{if(event.type==='state')update(event.state);});
  choices();void runtime.observe().then(s=>{if(alive)update(s);});
  return {dispose(){alive=false;epoch++;unsubscribe();container.replaceChildren();style.remove();}};
}
