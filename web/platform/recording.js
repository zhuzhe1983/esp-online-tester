// Portable semantic input recordings. Guest acknowledgements are evidence of delivery,
// not cycle-exact scheduling or a guarantee of identical firmware behavior.
import {encodeScene, decodeScene} from './catalog.js';

export const MAX_RECORDING_BYTES = 32768;
export const MAX_RECORDING_EVENTS = 500;
export const MAX_RECORDING_SECONDS = 120;
const clone = value => value == null ? value : structuredClone(value);
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const err = (code, message, details) => Object.assign(new Error(message), {code, details});
const canonical = value => Array.isArray(value) ? value.map(canonical) : object(value) ? Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])])) : value;
const json = value => JSON.stringify(canonical(value));
const config = scene => ({schemaVersion:scene.schemaVersion,productId:scene.productId,firmware:scene.firmware,accessories:scene.accessories,localAccessories:scene.localAccessories || []});
const sameScene = (a,b) => !!a && !!b && json(config(a)) === json(config(b));
const hashPattern = /^[a-f0-9]{64}$/i;
const pressInput=event=>event.type==='accessory-angle'?{type:event.type,accessoryId:event.accessoryId,id:event.id,value:event.value}:event.type==='encoder'?{type:'encoder',id:event.id,steps:event.steps}:event.type==='accessory-connection'?{type:event.type,accessoryId:event.accessoryId,connected:event.connected}:event.type==='accessory-button'?{type:event.type,accessoryId:event.accessoryId,id:event.id,pressed:event.pressed}:event.type==='matrix-key'?{type:'matrix-key',row:event.row,column:event.column,pressed:event.pressed}:{type:'button',id:event.id,pressed:event.pressed};
const pressId=event=>event.type==='accessory-button'?`accessory:${event.accessoryId}:${event.id}`:event.type==='matrix-key'?`matrix:${event.row}:${event.column}`:`button:${event.id}`;
const sourceUrl=value=>{try{const url=new URL(value);return typeof value==='string'&&value.length<=4096&&!url.username&&!url.password&&(url.protocol==='https:'||(url.protocol==='http:'&&['localhost','127.0.0.1','[::1]'].includes(url.hostname)));}catch{return false;}};
const fingerprint = firmware => json({manifestHash:firmware.manifestHash ?? null,appDirect:!!firmware.appDirect,executionMode:firmware.executionMode || 'wasm-jit',stubs:firmware.stubs || [],artifacts:firmware.artifacts.map(({kind,sha256,bytes,at}) => ({kind,sha256,bytes,...(at === undefined ? {} : {at})}))});
// Descriptive browser metadata is not part of the portable execution receipt.
const recordingFirmware = firmware => clone(Object.fromEntries(['source','manifestUrl','manifestHash','artifacts','appDirect','stubs','executionMode'].filter(key=>firmware[key]!==undefined).map(key=>[key,firmware[key]])));

export function validateRecording(recording, {scene} = {}) {
  const errors = [], add = (code,path,message) => errors.push({code,path,message});
  const keys = (value, allowed, path) => { for (const key of Object.keys(value)) if (!allowed.includes(key)) add('RECORDING_FIELD', `${path}/${key}`, `Unsupported field: ${key}`); };
  try {
    // Reuse the scene encoder's bounded plain-JSON/prototype-key checks.
    encodeScene({schemaVersion:'1', recording});
    if (new TextEncoder().encode(json(recording)).length > MAX_RECORDING_BYTES) add('RECORDING_SIZE','/','Recording exceeds 32 KiB.');
  } catch (error) { return {valid:false,errors:[{code:'RECORDING_JSON',path:'/',message:error.message}],warnings:[]}; }
  if (!object(recording)) return {valid:false,errors:[{code:'RECORDING_FORMAT',path:'/',message:'Recording must be an object.'}],warnings:[]};
  keys(recording,['schemaVersion','scene','firmware','clock','display','duration','events','expectedFrame'],'');
  if (recording.schemaVersion !== '1') add('RECORDING_VERSION','/schemaVersion','Unsupported recording version.');
  if (!object(recording.scene) || recording.scene.schemaVersion !== '1' || typeof recording.scene.productId !== 'string' || !object(recording.scene.firmware)) add('RECORDING_SCENE','/scene','Recording needs its configured scene and remote firmware source.');
  else {
    keys(recording.scene,['schemaVersion','productId','firmware','accessories','keyMapping','view','localAccessories'],'/scene');
    keys(recording.scene.firmware,['manifestUrl','url','sha256','bootMode'],'/scene/firmware');
    const source=recording.scene.firmware;
    if(source.bootMode!==undefined&&(!source.url||!['rom','app-direct'].includes(source.bootMode)))add('RECORDING_SOURCE','/scene/firmware/bootMode','Invalid raw-image boot mode.');
    if(Boolean(source.url)===Boolean(source.manifestUrl)||!sourceUrl(source.url||source.manifestUrl)||!hashPattern.test(source.sha256))add('RECORDING_SOURCE','/scene/firmware','A recording requires an absolute remote source with its SHA-256 pinned.');
    if(!Array.isArray(recording.scene.accessories)||!object(recording.scene.keyMapping)||!object(recording.scene.view))add('RECORDING_SCENE','/scene','Scene accessories, keyMapping and view are required.');
  }
  if (scene && !sameScene(recording.scene,scene)) add('RECORDING_SCENE_MISMATCH','/scene','Product, firmware and accessories must match the recording.');
  if (!object(recording.clock) || recording.clock.unit !== 'guest-seconds' || recording.clock.origin !== 'reset' || recording.clock.delivery !== 'worker-acknowledgement') add('RECORDING_CLOCK','/clock','Use reset-relative guest time and worker acknowledgements.');
  else keys(recording.clock,['unit','origin','delivery'],'/clock');
  if (!Number.isFinite(recording.duration) || recording.duration < 0 || recording.duration > MAX_RECORDING_SECONDS) add('RECORDING_DURATION','/duration','Duration must be 0–120 guest seconds.');
  if (recording.display !== null && (!object(recording.display) || ![recording.display.width,recording.display.height].every(n => Number.isInteger(n) && n > 0 && n <= 2048))) add('RECORDING_DISPLAY','/display','Display dimensions must be null or 1–2048 pixels.');
  else if (recording.display) keys(recording.display,['width','height'],'/display');
  const fw = recording.firmware;
  if (!object(fw) || fw.source !== 'remote' || !Array.isArray(fw.artifacts) || !fw.artifacts.length || fw.artifacts.length > 32) add('RECORDING_FIRMWARE','/firmware','A remote firmware receipt with 1–32 artifacts is required.');
  else {
    keys(fw,['source','manifestUrl','manifestHash','artifacts','appDirect','stubs','executionMode'],'/firmware');
    if(fw.executionMode!==undefined&&!['wasm-jit','wasm-interpreter'].includes(fw.executionMode))add('RECORDING_FIRMWARE','/firmware/executionMode','Invalid WASM execution mode.');
    if(fw.manifestUrl!==null&&!sourceUrl(fw.manifestUrl))add('RECORDING_SOURCE','/firmware/manifestUrl','Invalid manifest receipt URL.');
    if(typeof fw.appDirect!=='boolean'||!Array.isArray(fw.stubs)||fw.stubs.length>64||fw.stubs.some(s=>typeof s!=='string'||s.length>255))add('RECORDING_FIRMWARE','/firmware','Invalid execution-mode receipt.');
    if (fw.manifestHash !== null && !hashPattern.test(fw.manifestHash)) add('RECORDING_HASH','/firmware/manifestHash','Invalid manifest hash.');
    for (const [i,item] of fw.artifacts.entries()) {
      if (!object(item)) {add('RECORDING_FIRMWARE',`/firmware/artifacts/${i}`,'Invalid artifact.');continue;}
      keys(item,['kind','url','sha256','bytes','at'],`/firmware/artifacts/${i}`);
      if (!['rom','bootloader','ptable','app','elf','flash','script','picture'].includes(item.kind) || !hashPattern.test(item.sha256) || !Number.isInteger(item.bytes) || item.bytes < 1 || item.bytes > 32*1024*1024) add('RECORDING_FIRMWARE',`/firmware/artifacts/${i}`,'Invalid artifact kind, hash or size.');
      if(!sourceUrl(item.url))add('RECORDING_SOURCE',`/firmware/artifacts/${i}/url`,'Invalid artifact receipt URL.');
      if (item.at !== undefined && (!Number.isInteger(item.at) || item.at < 0 || item.at > 32*1024*1024)) add('RECORDING_FIRMWARE',`/firmware/artifacts/${i}/at`,'Invalid artifact offset.');
    }
  }
  const attached=new Set((recording.scene?.accessories||[]).filter(item=>item.wiring?.mode==='physical'&&(item.wiring.interfaceId==='grove'||item.wiring.viaAccessoryId)).map(item=>item.id));
  const connected=new Set(attached);
  const held = new Set(); let touch = false, previous = 0;
  if (!Array.isArray(recording.events) || recording.events.length > MAX_RECORDING_EVENTS) add('RECORDING_EVENTS','/events','At most 500 input events are allowed.');
  else for (const [i,event] of recording.events.entries()) {
    const path=`/events/${i}`;
    if (!object(event)) {add('RECORDING_EVENT',path,'Input event must be an object.');continue;}
    if (!Number.isFinite(event.at) || event.at < previous || event.at < 0 || event.at > recording.duration) add('RECORDING_TIME',`${path}/at`,'Input times must be ordered and within duration.');
    previous = event.at;
    if (event.type === 'button') {
      keys(event,['at','type','id','pressed'],path);
      if (typeof event.id !== 'string' || !/^[a-z0-9][a-z0-9-]{0,79}$/.test(event.id) || typeof event.pressed !== 'boolean') add('RECORDING_BUTTON',path,'Invalid semantic button input.');
      if (event.pressed) held.add(event.id); else held.delete(event.id);
    } else if(event.type==='encoder') {
      keys(event,['at','type','id','steps'],path);
      if(typeof event.id!=='string'||!/^[a-z0-9][a-z0-9-]{0,79}$/.test(event.id)||!Number.isInteger(event.steps)||event.steps===0||Math.abs(event.steps)>64)add('RECORDING_ENCODER',path,'Invalid encoder detents.');
    } else if(['accessory-button','accessory-angle','accessory-connection'].includes(event.type)) {
      if(!attached.has(event.accessoryId))add('RECORDING_ACCESSORY',path,'Accessory must have a physical route in the recorded scene.');
      const key=`accessory:${event.accessoryId}:button`;
      if(event.type==='accessory-button') {
        keys(event,['at','type','accessoryId','id','pressed'],path);
        if(recording.scene?.accessories?.find(item=>item.id===event.accessoryId)?.catalogId!=='m5stack-unit-button-u027'||event.id!=='button'||typeof event.pressed!=='boolean')add('RECORDING_ACCESSORY',path,'Invalid accessory button input.');
        if(event.pressed&&(!connected.has(event.accessoryId)||!connected.has(recording.scene.accessories.find(item=>item.id===event.accessoryId)?.wiring?.viaAccessoryId||event.accessoryId)))add('RECORDING_ACCESSORY',path,'A disconnected accessory cannot be pressed.');
        if(event.pressed)held.add(key);else held.delete(key);
      } else if(event.type==='accessory-angle') {
        keys(event,['at','type','accessoryId','id','value'],path);
        if(recording.scene?.accessories?.find(item=>item.id===event.accessoryId)?.catalogId!=='m5stack-unit-angle-u005'||event.id!=='knob'||!Number.isFinite(event.value)||event.value<0||event.value>1)add('RECORDING_ACCESSORY',path,'Invalid accessory potentiometer input.');
      } else {
        keys(event,['at','type','accessoryId','connected'],path);
        if(typeof event.connected!=='boolean')add('RECORDING_ACCESSORY',path,'Connection state must be boolean.');
        if(event.connected)connected.add(event.accessoryId);else connected.delete(event.accessoryId);held.delete(key);if(!event.connected)for(const child of (recording.scene?.accessories||[]).filter(item=>item.wiring?.viaAccessoryId===event.accessoryId))held.delete(`accessory:${child.id}:button`);
      }
    } else if(event.type==='matrix-key') {
      keys(event,['at','type','row','column','pressed'],path);
      if(!Number.isInteger(event.row)||event.row<0||event.row>3||!Number.isInteger(event.column)||event.column<0||event.column>13||typeof event.pressed!=='boolean')add('RECORDING_MATRIX_KEY',path,'Invalid matrix key coordinates.');
      if(event.pressed)held.add(pressId(event));else held.delete(pressId(event));
    } else if (event.type === 'touch') {
      keys(event,['at','type','x','y','down'],path);
      if (!recording.display || ![event.x,event.y].every(n => Number.isFinite(n) && n >= 0 && n <= 1) || typeof event.down !== 'boolean') add('RECORDING_TOUCH',path,'Touch needs a display and normalized coordinates in 0–1.');
      touch = event.down;
    } else add('RECORDING_EVENT',path,'Only semantic button, matrix key, encoder, touch and supported accessory inputs may be replayed.');
  }
  if (held.size || touch) add('RECORDING_HELD','/events','Recording must end with every button and touch released.');
  if (recording.expectedFrame !== undefined) {
    if (!object(recording.expectedFrame) || !hashPattern.test(recording.expectedFrame.sha256) || ![recording.expectedFrame.width,recording.expectedFrame.height].every(n=>Number.isInteger(n)&&n>0&&n<=2048)) add('RECORDING_FRAME','/expectedFrame','Invalid final framebuffer hash or dimensions.');
    else keys(recording.expectedFrame,['sha256','width','height'],'/expectedFrame');
  }
  return {valid:errors.length===0,errors,warnings:[]};
}
function requireValid(recording, options) { const result=validateRecording(recording,options); if(!result.valid) throw err(result.errors[0].code,result.errors.map(item=>item.message).join(' '),result); }
export function encodeRecording(recording) {
  requireValid(recording);
  const bytes=new TextEncoder().encode(json(recording)); let binary=''; for(const byte of bytes)binary+=String.fromCharCode(byte);
  return btoa(binary).replaceAll('+','-').replaceAll('/','_').replace(/=+$/,'');
}
export function decodeRecording(encoded) {
  if(typeof encoded!=='string'||encoded.length>Math.ceil(MAX_RECORDING_BYTES*4/3)||!/^[A-Za-z0-9_-]+$/.test(encoded)||encoded.length%4===1)throw err('RECORDING_ENCODING','Invalid or oversized recording encoding.');
  let recording;try{const raw=atob(encoded.replaceAll('-','+').replaceAll('_','/'));recording=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(Uint8Array.from(raw,c=>c.charCodeAt(0))));}catch{throw err('RECORDING_ENCODING','Recording must be UTF-8 base64url JSON.');}
  // decodeScene performs the same prototype-key check even for parsed JSON.
  decodeScene(encodeScene({schemaVersion:'1',recording})); requireValid(recording); return recording;
}

export function createRecordingController(runtime) {
  let status='idle', recording=null, receipt=null, error=null, progress={index:0,total:0,elapsedGuestSeconds:0};
  let token=null, ownLifecycle=false, sending=false, closing=false, stopPending=null, lastGuest=0, heldTouch=null;
  const held=new Map(), listeners=new Set();
  const observe=()=>clone({status,recording,progress,receipt,error});
  const emit=()=>{const event={type:'recording',state:observe()};for(const listener of listeners){try{listener(event);}catch{}}};
  const newToken=()=>{let finish;const done=new Promise(resolve=>{finish=resolve;});return {cancelled:false,done,finish};};
  const change=next=>{status=next;emit();};
  const failure=e=>{error={code:e.code||'RECORDING_ERROR',message:e.message};change('error');};
  const check=t=>{if(t.cancelled)throw err('RECORDING_CANCELLED','Recording operation cancelled.');};
  const requirePristineStorage=state=>{if(state.sdCard?.pristine===false)throw err('RECORDING_STORAGE_NOT_PRISTINE','当前虚拟 SD 卡含有导入文件或持久化改动，分享录制不会携带这些私人内容。请先导出备份，并手动清空 SD 卡后再录制或回放。');if(state.storage?.pristine===false)throw err('RECORDING_STORAGE_NOT_PRISTINE','当前设备已有持久化数据，分享录制无法携带这些内容。请先在设备存储中手动清空，再开始录制或回放。');};
  const tracked=event=>{if(['button','matrix-key','accessory-button'].includes(event.type)){if(event.pressed)held.set(pressId(event),pressInput(event));else held.delete(pressId(event));}else if(event.type==='accessory-connection')held.delete(`accessory:${event.accessoryId}:button`);else if(event.type==='touch')heldTouch=event.down?clone(event):null;};
  const release=async()=>{
    const current=await runtime.observe();
    if(['running','paused'].includes(current.status)) {
      for(const action of [...held.values()]){try{sending=true;await runtime.input({...action,pressed:false});}catch{}finally{sending=false;}}
      if(heldTouch){try{sending=true;await runtime.input({...heldTouch,down:false});}catch{}finally{sending=false;}}
    }
    held.clear();heldTouch=null;
  };
  const frameHash=async()=>{
    try{const frame=await runtime.capture({format:'rgba'});const bytes=new Uint8Array(frame.pixels.buffer,frame.pixels.byteOffset,frame.pixels.byteLength);const hash=await crypto.subtle.digest('SHA-256',bytes);return {sha256:Array.from(new Uint8Array(hash),n=>n.toString(16).padStart(2,'0')).join(''),width:frame.width,height:frame.height};}catch{return null;}
  };
  const stop=async(reason='stopped')=>{
    if(status!=='recording'||closing)throw err('RECORDING_NOT_ACTIVE','No active recording to stop.');
    closing=true;
    let finishStop;stopPending=new Promise(resolve=>{finishStop=resolve;});
    try {
      await release();
      const current=await runtime.observe();
      recording.duration=Math.max(lastGuest,Number(current.stats?.time)||0);
      status='completed'; // End the input capture window before asynchronous frame hashing.
      const expected=await frameHash();if(expected)recording.expectedFrame=expected;
      requireValid(recording);
      progress={index:recording.events.length,total:recording.events.length,elapsedGuestSeconds:recording.duration};
      receipt={kind:'recording',reason,eventCount:recording.events.length,bitExact:false};change('completed');return clone(recording);
    } catch(e){failure(e);throw e;} finally{closing=false;stopPending=null;finishStop();}
  };
  const cancel=async(reason='takeover')=>{
    if(stopPending)await stopPending;
    const active=token;if(active)active.cancelled=true;
    if(!['preparing','recording','playing'].includes(status))return observe();
    const wasRecording=status==='recording';
    if(wasRecording&&!closing){try{await stop(reason);return observe();}catch{recording=null;}}
    change('cancelled');
    await runtime.cancelPendingLoad?.();await release();await active?.done;
    receipt={...(receipt||{}),kind:'replay',result:'cancelled',reason,bitExact:false};emit();return observe();
  };
  const onEvent=event=>{
    if(event.type==='input'&&['button','matrix-key','touch','encoder','accessory-button','accessory-angle','accessory-connection'].includes(event.event.type)) {
      if(status==='recording') {
        tracked(event.event);
        if(event.receipt?.delivery!=='queued'||!Number.isFinite(event.receipt.guestTime)){void cancel('missing-acknowledgement');return;}
        const action=event.event.type!=='touch'?{at:event.receipt.guestTime,...pressInput(event.event)}:{at:event.receipt.guestTime,type:'touch',x:event.event.x,y:event.event.y,down:event.event.down};
        if(action.type==='touch'){
          const display=recording.display;
          if(!display){void cancel('no-display');return;}
          action.x/=Math.max(1,display.width-1);action.y/=Math.max(1,display.height-1);
        }
        recording.events.push(action);lastGuest=action.at;recording.duration=lastGuest;
        progress={index:recording.events.length,total:recording.events.length,elapsedGuestSeconds:lastGuest};emit();
        if(!closing&&(recording.events.length+held.size+(heldTouch?1:0)>=MAX_RECORDING_EVENTS||new TextEncoder().encode(json(recording)).length>MAX_RECORDING_BYTES-2048))void stop('limit').catch(()=>{});
      } else if(status==='playing'&&!sending)void cancel('human-input');
    }
    if(event.type==='stats'&&status==='recording'&&!closing&&Number(event.stats.time)>=MAX_RECORDING_SECONDS-1)void stop('duration-limit').catch(()=>{});
    if(event.type==='state'&&!ownLifecycle&&['recording','playing','preparing'].includes(status)&&['configured','loading','ready','paused','error','disposed'].includes(event.state.status)) {
      if(status==='recording'&&event.state.status==='paused'){void stop('paused').catch(()=>{});return;}
      if(status==='recording'){recording=null;held.clear();heldTouch=null;if(token)token.cancelled=true;receipt={kind:'recording',result:'cancelled',reason:`runtime-${event.state.status}`,bitExact:false};change('cancelled');return;}
      void cancel(`runtime-${event.state.status}`);
    }
  };
  const unsubscribe=runtime.subscribe(onEvent);
  const waitUntil=async(target,t,timeoutMs)=>{
    const start=Date.now();
    while(true){check(t);const state=await runtime.observe();if(!sameScene(state.scene,recording.scene))throw err('RECORDING_SCENE_MISMATCH','Scene changed during playback.');if(state.status!=='running')throw err('RECORDING_INTERRUPTED',`Runtime is ${state.status}.`);const time=Number(state.stats?.time)||0;progress.elapsedGuestSeconds=time;if(time>=target)return time;if(Date.now()-start>timeoutMs)throw err('RECORDING_TIMEOUT','Guest clock did not reach the next input before the wall-time limit.');await new Promise(resolve=>setTimeout(resolve,20));}
  };
  return {
    subscribe(listener){listeners.add(listener);return()=>listeners.delete(listener);},
    async beforeInput(){if(['playing','preparing'].includes(status))await cancel('human-input');},
    async beforeLifecycle(reason){await cancel(reason);},
    api:{
      async recordingObserve(){return observe();},
      async recordingStart(){
        await cancel('replaced');
        let state=await runtime.observe();
        if(state.storage?.scope && runtime.flushDeviceStorage){await runtime.flushDeviceStorage();state=await runtime.observe();}
        requirePristineStorage(state);
        if(!state.firmware||state.firmware.source!=='remote'||!state.scene?.firmware)throw err('RECORDING_NOT_READY','Load a remote firmware source before recording. Local firmware cannot be shared in a recording URL.');
        token=newToken();const active=token;recording=null;receipt=null;error=null;held.clear();heldTouch=null;lastGuest=0;
        change('preparing');ownLifecycle=true;
        try{
          await runtime.reset();check(active);
          const current=await runtime.observe(),display=current.product.display;
          if(!current.firmware||!sameScene(current.scene,state.scene))throw err('RECORDING_SCENE_MISMATCH','Scene changed while preparing the recording.');
          recording={schemaVersion:'1',scene:clone(current.scene),firmware:recordingFirmware(current.firmware),clock:{unit:'guest-seconds',origin:'reset',delivery:'worker-acknowledgement'},display:display?{width:display.framebufferWidth||display.width,height:display.framebufferHeight||display.height}:null,duration:0,events:[]};
          requireValid(recording);
          progress={index:0,total:0,elapsedGuestSeconds:0};change('recording');return observe();
        }catch(e){if(active.cancelled){change('cancelled');return observe();}failure(e);throw e;}finally{ownLifecycle=false;active.finish();}
      },
      recordingStop(){return stop();},
      recordingTakeOver(){return cancel();},
      async recordingClear(){await cancel('cleared');recording=null;receipt=null;error=null;progress={index:0,total:0,elapsedGuestSeconds:0};change('idle');return observe();},
      async recordingPlay(input,{timeoutMs=180000}={}){
        if(stopPending)await stopPending;
        let current=await runtime.observe();
        if(current.storage?.scope && runtime.flushDeviceStorage){await runtime.flushDeviceStorage();current=await runtime.observe();}
        requireValid(input,{scene:current.scene});
        if(!Number.isFinite(timeoutMs)||timeoutMs<100||timeoutMs>300000)throw err('RECORDING_TIMEOUT','Replay timeout must be 100–300000 ms.');
        if(['preparing','recording','playing'].includes(status))throw err('RECORDING_BUSY','Stop the current recording or playback first.');
        const controls=new Set(current.product?.controls.filter(item=>item.type==='button').map(item=>item.id));
        const accessories=new Set((current.accessoryInputs||[]).map(item=>item.accessoryId));
        for(const event of input.events) {
          if(event.type==='encoder'&&!(current.product?.runtime?.encoder&&current.product.controls.some(c=>c.id===event.id&&c.type==='encoder')))throw err('RECORDING_CONTROL','Recording names an encoder absent from this product.');
          if(['accessory-button','accessory-angle','accessory-connection'].includes(event.type)&&!accessories.has(event.accessoryId))throw err('RECORDING_ACCESSORY','Recording names an accessory without a supported physical route.');
          if(event.type==='button'&&!controls.has(event.id))throw err('RECORDING_CONTROL','Recording names a button absent from this product.');
          if(event.type==='matrix-key'&&!current.product?.controls.some(control=>control.type==='matrix-key'&&control.row===event.row&&control.column===event.column))throw err('RECORDING_CONTROL','Recording names a matrix key absent from this product.');
        }
        const display=current.product?.display;
        if(input.display&&(!display||input.display.width!==(display.framebufferWidth||display.width)||input.display.height!==(display.framebufferHeight||display.height)))throw err('RECORDING_DISPLAY','Recording display dimensions do not match this product.');
        recording=clone(input);token=newToken();const active=token;receipt={kind:'replay',result:'running',bitExact:false,clock:'stats-observed guest time',events:[],timingDifferences:[]};error=null;held.clear();heldTouch=null;
        progress={index:0,total:recording.events.length,elapsedGuestSeconds:0};change('preparing');ownLifecycle=true;
        try{
          if(!current.firmware||fingerprint(current.firmware)!==fingerprint(recording.firmware))await runtime.loadFirmware(recording.scene.firmware);
          check(active);const loaded=await runtime.observe();requirePristineStorage(loaded);
          if(fingerprint(loaded.firmware)!==fingerprint(recording.firmware))throw err('RECORDING_FIRMWARE_MISMATCH','Downloaded artifact hashes differ from the recording.');
          await runtime.reset();check(active);ownLifecycle=false;change('playing');
          let previousActual=0,previousOriginal=0;
          for(const [index,event]of recording.events.entries()){
            // Preserve guest-time spacing when an earlier host delivery was late;
            // otherwise a delayed down/up pair could collapse into one guest tick.
            const target=Math.max(event.at,previousActual+event.at-previousOriginal);
            await waitUntil(target,active,timeoutMs);check(active);
            const action=event.type!=='touch'?pressInput(event):{type:'touch',x:Math.round(event.x*(recording.display.width-1)),y:Math.round(event.y*(recording.display.height-1)),down:event.down};
            sending=true;let result;try{result=await runtime.input(action);tracked(action);}finally{sending=false;}
            check(active);const actual=Number(result.guestTime);if(!Number.isFinite(actual))throw err('RECORDING_ACK','Input acknowledgement has no guest timestamp.');
            const delivery={index,originalGuestSeconds:event.at,scheduledGuestSeconds:target,actualGuestSeconds:actual,lateBySeconds:actual-event.at};receipt.events.push(delivery);
            previousActual=actual;previousOriginal=event.at;
            if(Math.abs(delivery.lateBySeconds)>0.05)receipt.timingDifferences.push(delivery);
            progress.index=index+1;progress.elapsedGuestSeconds=actual;emit();
          }
          await waitUntil(Math.max(recording.duration,previousActual+recording.duration-previousOriginal),active,timeoutMs);await release();check(active);
          const actualFrame=await frameHash();receipt.finalFrame=actualFrame;
          receipt.frameComparison=recording.expectedFrame&&actualFrame?(json(recording.expectedFrame)===json(actualFrame)?'equal':'different'):'unavailable';
          receipt.result='completed';change('completed');return clone(receipt);
        }catch(e){await release();if(active.cancelled){receipt.result='cancelled';change('cancelled');return clone(receipt);}receipt.result='error';failure(e);throw e;}finally{ownLifecycle=false;sending=false;active.finish();}
      },
    },
    async dispose(){await cancel('disposed');unsubscribe();listeners.clear();},
  };
}
