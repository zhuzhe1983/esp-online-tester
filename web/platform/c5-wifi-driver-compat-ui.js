import {C5_WIFI_DRIVER_COMPAT_PROFILE,C5_WIFI_DRIVER_COMPAT_BOARD,C5_WIFI_DRIVER_COMPAT_NOTICE,C5_WIFI_DRIVER_COMPAT_RELOAD_NOTICE,withC5WifiDriverCompat} from './c5-wifi-driver-compat.js';

export function c5WifiDriverCompatStatus(state) {
  if (state?.product?.runtime?.board !== C5_WIFI_DRIVER_COMPAT_BOARD) return 'unavailable';
  if (state.scene?.c5WifiDriverCompat !== C5_WIFI_DRIVER_COMPAT_PROFILE) return 'strict';
  const observed = state.driverCompatibility;
  if(state.status==='error'&&observed?.enabled===true&&observed.profile===C5_WIFI_DRIVER_COMPAT_PROFILE)return 'stopped';
  return ['ready','running','paused'].includes(state.status) && observed?.enabled === true && observed.profile === C5_WIFI_DRIVER_COMPAT_PROFILE ? 'enabled' : 'selected';
}

// Selection is portable scene data; only an actual worker snapshot can say enabled.
export function mountC5WifiDriverCompat({firmwareContainer,stage,runtime,onChange,onError=console.error,document:doc=globalThis.document}={}) {
  if (!stage) return {setDisabled(){},dispose(){}};
  const node=(tag,className,text)=>{const e=doc.createElement(tag);if(className)e.className=className;if(text)e.textContent=text;return e;};
  const style=node('link');style.rel='stylesheet';style.href=new URL('./c5-wifi-driver-compat.css',import.meta.url).href;doc.head.append(style);
  const badge=node('aside','c5-wifi-compat-badge');badge.hidden=true;badge.setAttribute('role','status');badge.setAttribute('aria-live','polite');
  const title=node('strong'),notice=node('span','',C5_WIFI_DRIVER_COMPAT_NOTICE);badge.append(title,notice);stage.append(badge);
  let controls,select,apply;
  if (firmwareContainer) {
    controls=node('section','c5-wifi-compat-controls');controls.hidden=true;
    const label=node('label','field-label','运行模式');select=node('select');select.setAttribute('aria-label','C5 Wi-Fi 驱动运行模式');
    for (const [value,text] of [['','严格硬件模拟'],[C5_WIFI_DRIVER_COMPAT_PROFILE,'Wi-Fi 驱动兼容（离线）']]) {const option=node('option','',text);option.value=value;select.append(option);}
    label.append(select);apply=node('button','button quiet tiny','应用模式并停止运行');apply.type='button';
    controls.append(label,node('p','helper',C5_WIFI_DRIVER_COMPAT_RELOAD_NOTICE),node('p','helper',`兼容模式：${C5_WIFI_DRIVER_COMPAT_NOTICE}`),apply);
    const boundary=firmwareContainer.querySelector('#firmware-run-boundary');if(boundary)boundary.before(controls);else firmwareContainer.append(controls);
  }
  let alive=true,state=null,disabled=false,pending=false,signature='',revision=0;
  const render=()=>{
    if (!alive) return;
    const mode=c5WifiDriverCompatStatus(state),available=mode!=='unavailable',selected=mode==='selected'||mode==='enabled'||mode==='stopped';
    badge.hidden=!selected;
    title.textContent=`Wi-Fi 驱动兼容（离线） · ${mode==='stopped'?'本次兼容运行已停止':mode==='enabled'?'本次运行已启用':'已选择，尚未启用'}`;
    if (!controls) return;
    controls.hidden=!available;
    const nextSignature=JSON.stringify([state?.scene?.productId,state?.scene?.c5WifiDriverCompat]);
    if (nextSignature!==signature) {signature=nextSignature;select.value=selected?C5_WIFI_DRIVER_COMPAT_PROFILE:'';}
    select.disabled=disabled||pending||!available;
    apply.disabled=select.disabled||select.value===(selected?C5_WIFI_DRIVER_COMPAT_PROFILE:'');
  };
  select?.addEventListener('change',render);
  apply?.addEventListener('click',()=>{
    if (apply.disabled) return;
    const before=JSON.stringify(state.scene),next=withC5WifiDriverCompat(state.scene,select.value);pending=true;render();
    Promise.resolve().then(()=>{
      if(!alive)return;
      if(JSON.stringify(state?.scene)!==before)throw Object.assign(new Error('设备或场景已改变，请重新选择运行模式。'),{code:'SCENE_CHANGED'});
      return onChange(next);
    }).catch(error=>{if(alive)onError(error);}).finally(()=>{pending=false;render();});
  });
  const update=next=>{if(alive){state=next;render();}};
  const unsubscribe=runtime.subscribe(event=>{
    if (event.type==='state') {revision++;update(event.state);}
    else if (event.type==='driver-compatibility'&&state) {revision++;update({...state,driverCompatibility:event.driverCompatibility});}
  });
  const initialRevision=revision;
  void runtime.observe().then(next=>{if(revision===initialRevision)update(next);}).catch(onError);
  return {setDisabled(value){disabled=!!value;render();},dispose(){if(!alive)return;alive=false;unsubscribe();controls?.remove();badge.remove();style.remove();}};
}
