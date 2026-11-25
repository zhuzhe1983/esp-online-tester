import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createDefaultScene,validateScene,encodeScene,decodeScene,firmwareSourceMatches} from '../web/platform/catalog.js';
import {C5_WIFI_DRIVER_COMPAT_PROFILE as PROFILE,C5_WIFI_DRIVER_COMPAT_BOARD as BOARD,C5_WIFI_DRIVER_COMPAT_PRODUCT_IDS as IDS,c5WifiDriverCompatAllowed,c5WifiDriverCompatIssue,withC5WifiDriverCompat} from '../web/platform/c5-wifi-driver-compat.js';
import {mountC5WifiDriverCompat,c5WifiDriverCompatStatus} from '../web/platform/c5-wifi-driver-compat-ui.js';
import {installAutomation} from '../web/platform/automation.js';
import {installEmbedBridge} from '../web/platform/embed.js';
import {validateRecording,encodeRecording,decodeRecording,createRecordingController} from '../web/platform/recording.js';

// Pure configuration, UI and RPC fixtures; no Worker, WASM or guest is created.
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async name=>[name,JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url))).items])));
const source={url:'https://example.org/lcd273.bin',sha256:'a'.repeat(64),bootMode:'rom'};
const strict=()=>({...createDefaultScene(catalog,IDS[0]),firmware:structuredClone(source)});
const selected=()=>withC5WifiDriverCompat(strict(),PROFILE);
const product=id=>catalog.products.find(p=>p.id===id);
const state=(scene=selected(),extra={})=>({status:'configured',scene,product:product(scene.productId),firmware:null,...extra});
const tick=()=>new Promise(resolve=>setImmediate(resolve));

test('compatibility is opt-in, exact and LCD273 ROM-only; local Files can defer BIN validation',()=>{
  for(const id of IDS) {
    const scene={...strict(),productId:id};
    assert.equal(Object.hasOwn(scene,'c5WifiDriverCompat'),false);
    assert.equal(validateScene(scene,catalog).valid,true);
    const next=withC5WifiDriverCompat(scene,PROFILE),result=validateScene(next,catalog);
    assert.equal(result.valid,true);assert.ok(result.warnings.some(w=>w.code==='C5_WIFI_DRIVER_COMPAT_OFFLINE'));
    assert.equal(c5WifiDriverCompatAllowed(product(id),PROFILE),true);
    assert.equal(c5WifiDriverCompatIssue({...next,firmware:null},product(id)),null);
    assert.equal(validateScene({...next,firmware:null},catalog).valid,true);
    assert.deepEqual(withC5WifiDriverCompat(next,''),scene);
  }
  for(const profile of [null,'strict',true,'WAVESHARE-LCD273-IDF553-OFFLINE-V1','unknown'])assert.equal(validateScene({...strict(),c5WifiDriverCompat:profile},catalog).valid,false);
  for(const p of catalog.products.filter(p=>p.runtime.board!==BOARD))assert.equal(validateScene({...createDefaultScene(catalog,p.id),c5WifiDriverCompat:PROFILE},catalog).valid,false,p.id);
  const direct=selected();direct.firmware.bootMode='app-direct';assert.equal(validateScene(direct,catalog).valid,false);
  assert.throws(()=>withC5WifiDriverCompat(strict(),'unknown'),{code:'C5_WIFI_DRIVER_COMPAT_PROFILE'});
  assert.equal(Object.hasOwn(createDefaultScene(catalog,'m5stack-stopwatch'),'c5WifiDriverCompat'),false,'Changing product creates a strict scene');
});

test('schema and source replacement preserve an explicit portable scene selection',async()=>{
  const schema=JSON.parse(await readFile(new URL('../web/platform/schemas/scene.schema.json',import.meta.url)));
  assert.equal(schema.properties.c5WifiDriverCompat.const,PROFILE);
  const gate=schema.allOf.find(rule=>rule.if.required.includes('c5WifiDriverCompat')).then.properties;
  assert.deepEqual(gate.productId.enum,IDS);assert.equal(gate.firmware.not.properties.bootMode.const,'app-direct');
  const original=selected(),replacement={...original,firmware:{...source,sha256:source.sha256.toUpperCase()}};
  assert.equal(firmwareSourceMatches(original.firmware,replacement.firmware),true);
  assert.deepEqual(decodeScene(encodeScene(replacement,catalog),catalog),replacement);
  assert.equal(replacement.c5WifiDriverCompat,PROFILE);
  const local={...original,firmware:null};assert.equal(decodeScene(encodeScene(local,catalog),catalog).c5WifiDriverCompat,PROFILE);
});

test('window API sharing and origin-bound embedded configure/observe retain the scene marker',async()=>{
  let current=state(strict());const listeners=new Set();
  const runtime={subscribe(fn){listeners.add(fn);return()=>listeners.delete(fn);},observe:async()=>structuredClone(current),capabilities:async()=>({}),async configure(scene){current=state(structuredClone(scene));return structuredClone(current);}};
  const api=installAutomation(runtime,{target:{location:{href:'https://example.org/repo/platform/'}}});
  const replies=[];let receive;
  const target={parent:{postMessage(message,origin){replies.push({message,origin});}},addEventListener(type,fn){if(type==='message')receive=fn;},removeEventListener(){receive=null;}};
  const detach=installEmbedBridge(api,{target});
  const call=async(type,requestId,extra={})=>receive({source:target.parent,origin:'https://example.org',data:{channel:'esp-workbench',version:'1',session:'c5-test',type,requestId,...extra}});
  try {
    await call('hello','hello');await call('call','configure',{method:'configure',args:[selected()]});
    assert.equal(replies.at(-1).message.result.scene.c5WifiDriverCompat,PROFILE);
    await call('call','observe',{method:'observe',args:[]});assert.equal(replies.at(-1).message.result.scene.c5WifiDriverCompat,PROFILE);
    const link=new URL(await api.share({mode:'embed'}));assert.equal(link.pathname,'/repo/platform/embed.html');
    assert.equal(decodeScene(new URLSearchParams(link.hash.slice(1)).get('scene')).c5WifiDriverCompat,PROFILE);
    assert.ok(replies.every(reply=>reply.origin==='https://example.org'));
  } finally {detach();}
});

const receipt=compat=>({source:'remote',manifestUrl:null,manifestHash:null,artifacts:[{kind:'flash',url:source.url,sha256:source.sha256,bytes:1024}],appDirect:false,stubs:[],...(compat?{c5WifiDriverCompat:PROFILE}:{})});
const recording=compat=>({schemaVersion:'1',scene:compat?selected():strict(),firmware:receipt(compat),clock:{unit:'guest-seconds',origin:'reset',delivery:'worker-acknowledgement'},display:null,duration:0,events:[]});
test('recordings retain strict legacy compatibility and reject cross-mode or inconsistent receipts',()=>{
  for(const compat of [false,true]) {
    const recorded=recording(compat);assert.equal(validateRecording(recorded).valid,true);assert.deepEqual(decodeRecording(encodeRecording(recorded)),recorded);
    assert.equal(validateRecording(recorded,{scene:compat?strict():selected()}).valid,false);
  }
  for(const mutate of [r=>delete r.firmware.c5WifiDriverCompat,r=>r.firmware.c5WifiDriverCompat='unknown',r=>r.firmware.appDirect=true,r=>r.scene.productId='m5stack-stopwatch',r=>r.scene.firmware.bootMode='app-direct']) {
    const r=recording(true);mutate(r);assert.equal(validateRecording(r).valid,false);
  }
  const strictWithReceipt=recording(false);strictWithReceipt.firmware.c5WifiDriverCompat=PROFILE;assert.equal(validateRecording(strictWithReceipt).valid,false);
});

test('recorder keeps the receipt profile and checks it before replay reset',async()=>{
  let current=state(selected(),{status:'ready',product:{...product(IDS[0]),display:null},firmware:receipt(true),stats:{time:0}}),resets=0,loads=0;
  const runtime={subscribe:()=>()=>{},observe:async()=>structuredClone(current),async reset(){resets++;current.status='running';},async loadFirmware(){loads++;}};
  const controller=createRecordingController(runtime);
  await controller.api.recordingStart();const saved=await controller.api.recordingStop();
  assert.equal(saved.scene.c5WifiDriverCompat,PROFILE);assert.equal(saved.firmware.c5WifiDriverCompat,PROFILE);
  current={...current,firmware:receipt(false)};
  await assert.rejects(controller.api.recordingPlay(saved,{timeoutMs:100}),{code:'RECORDING_FIRMWARE_MISMATCH'});
  assert.equal(loads,1,'A selected scene does not make a strict loaded receipt compatible');assert.equal(resets,1,'Replay rejects the wrong receipt before another reset');
  await controller.dispose();
});

class Element {
  constructor(tag){this.tagName=tag;this.children=[];this.listeners=new Map();this.hidden=false;this.disabled=false;this.value='';this.className='';this.text='';}
  set textContent(value){this.text=value;this.children=[];}get textContent(){return this.text+this.children.map(c=>c.textContent).join('');}
  setAttribute(key,value){this[key]=value;}
  append(...nodes){for(const child of nodes){child.parent=this;this.children.push(child);}}
  before(child){const at=this.parent.children.indexOf(this);child.parent=this.parent;this.parent.children.splice(at,0,child);}
  remove(){if(this.parent)this.parent.children=this.parent.children.filter(c=>c!==this);this.parent=null;}
  querySelector(selector){return this.children.flatMap(child=>[child,child.querySelector(selector)]).find(child=>child&&(selector.startsWith('.')?child.className.split(' ').includes(selector.slice(1)):selector.startsWith('#')?child.id===selector.slice(1):child.tagName===selector))||null;}
  addEventListener(type,fn){this.listeners.set(type,fn);}fire(type){this.listeners.get(type)?.({target:this});}
}
function uiFixture({embedded=false,initial=state(strict()),delayedInitial=false}={}) {
  const doc={head:new Element('head'),createElement:tag=>new Element(tag)},stage=new Element('div'),firmware=embedded?null:new Element('section');
  if(firmware){const boundary=new Element('div');boundary.id='firmware-run-boundary';firmware.append(boundary);}
  let current=initial,listener,resolveInitial;const changes=[],errors=[];
  const runtime={subscribe(fn){listener=fn;return()=>{listener=null;};},observe:()=>delayedInitial?new Promise(resolve=>{resolveInitial=resolve;}):Promise.resolve(current)};
  const emit=next=>{current=next;listener?.({type:'state',state:next});};
  const ui=mountC5WifiDriverCompat({document:doc,firmwareContainer:firmware,stage,runtime,onChange:async next=>{changes.push(next);emit(state(next));},onError:e=>errors.push(e)});
  return {doc,stage,firmware,ui,changes,errors,emit,event:driverCompatibility=>listener?.({type:'driver-compatibility',driverCompatibility}),resolve:next=>resolveInitial(next)};
}

test('workbench applies a mode only after the explicit stop/reload action and marks observations separately',async()=>{
  const f=uiFixture();await tick();
  try {
    const select=f.firmware.querySelector('select'),apply=f.firmware.querySelector('button'),controls=f.firmware.querySelector('.c5-wifi-compat-controls'),badge=f.stage.querySelector('aside');
    assert.equal(select.value,'');assert.equal(apply.disabled,true);assert.equal(badge.hidden,true);
    assert.match(controls.textContent,/停止当前运行/);assert.match(controls.textContent,/重新加载固件/);assert.match(controls.textContent,/无热点\/网络连接/);
    select.value=PROFILE;select.fire('change');assert.equal(apply.disabled,false);assert.equal(f.changes.length,0,'Draft selection does not configure or enable the driver');
    apply.fire('click');assert.equal(apply.disabled,true);await tick();
    assert.equal(f.changes.length,1);assert.equal(f.changes[0].c5WifiDriverCompat,PROFILE);assert.match(badge.textContent,/已选择，尚未启用/);
    f.emit(state(selected(),{status:'running',driverCompatibility:{enabled:true,profile:PROFILE}}));assert.match(badge.textContent,/本次运行已启用/);
    f.event({enabled:false,profile:PROFILE});assert.match(badge.textContent,/尚未启用/);
    f.event({enabled:true,profile:'unknown'});assert.match(badge.textContent,/尚未启用/);
    f.emit(state(selected(),{status:'error',driverCompatibility:{enabled:true,profile:PROFILE}}));assert.match(badge.textContent,/本次兼容运行已停止/);assert.equal(badge.hidden,false,'A stopped assisted guest stays visibly labeled');
    f.ui.setDisabled(true);assert.equal(select.disabled,true);f.ui.setDisabled(false);
    f.emit(state({...strict(),productId:'m5stack-stopwatch'}));assert.equal(controls.hidden,true);assert.equal(badge.hidden,true);
    f.emit(state(strict()));assert.equal(select.value,'');assert.equal(controls.hidden,false,'Returning to LCD273 keeps the fresh strict default');
    assert.deepEqual(f.errors,[]);
  } finally {f.ui.dispose();}
  assert.equal(f.stage.children.length,0);assert.equal(f.doc.head.children.length,0);
});

test('embedded mode always labels selection; stale startup observation cannot override a newer scene',async()=>{
  const f=uiFixture({embedded:true,delayedInitial:true});
  try {
    f.emit(state(selected()));f.resolve(state(strict()));await tick();
    const badge=f.stage.querySelector('aside');assert.equal(badge.hidden,false);assert.match(badge.textContent,/已选择，尚未启用/);assert.match(badge.textContent,/仅本次运行保留/);
    assert.equal(c5WifiDriverCompatStatus(state(selected(),{driverCompatibility:{enabled:true,profile:PROFILE}})),'selected','Configured state has no enabled worker');
    f.emit(state(strict(),{status:'running',driverCompatibility:{enabled:true,profile:PROFILE}}));assert.equal(badge.hidden,true,'A stale snapshot cannot opt a strict scene in');
  } finally {f.ui.dispose();}
});

test('a queued apply cannot restore an old device or configure after UI disposal',async()=>{
  for(const dispose of [false,true]) {
    const f=uiFixture();await tick();
    const select=f.firmware.querySelector('select');select.value=PROFILE;select.fire('change');f.firmware.querySelector('button').fire('click');
    if(dispose)f.ui.dispose();else f.emit(state({...strict(),productId:'m5stack-stopwatch'}));
    await tick();assert.equal(f.changes.length,0);
    if(!dispose){assert.equal(f.errors[0].code,'SCENE_CHANGED');f.ui.dispose();}
  }
});
