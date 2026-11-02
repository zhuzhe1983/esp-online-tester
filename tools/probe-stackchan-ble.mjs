#!/usr/bin/env node
// Local, caller-supplied official BIN and verified symbol metadata. No firmware is hosted.
// Exercise the production worker twice, including a persisted-Flash round trip.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {decodeFramebuffer} from '../web/platform/runtime.js';

const arg=name=>{const i=process.argv.indexOf(name);return i<0?null:process.argv[i+1];};
const required=name=>{const value=arg(name);if(!value)throw Error(`Supply ${name} <local-file>`);return resolve(value);};
const flow=arg('--flow')||'setup';
assert.ok(['setup','dance'].includes(flow),'--flow must be setup or dance');
const allowUnknown=process.argv.includes('--expect-unknown-hci');
const paths={firmware:required('--firmware'),symbols:required('--symbols'),rom:required('--rom'),wasm:required('--wasm')};
const inputs=Object.fromEntries(await Promise.all(Object.entries(paths).map(async([key,path])=>[key,await readFile(path)])));
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const firmwareSha='411578a2ebca2cfe3541fdc32aeddbc4703cf912a5daa7e1253f69306d6e1d87';
assert.equal(sha(inputs.firmware),firmwareSha,'This navigation probe is for the pinned official V1.5.1 image only');
const workerUrl=new URL('../web/wasm/worker.js',import.meta.url);
const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(workerUrl))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));
let sequence=0,serial='',frame=null,time=0,error=null;
const waiters=new Set(),deltas=[];
thread.on('error',e=>{error=e;});
thread.on('message',message=>{
  if(message.error&&!message.bleReply)error=Error(message.error);
  if(message.text){try{const event=JSON.parse(message.text);if(event.t==='serial'&&event.src==='uart0')serial+=(event.data||'').replace(/\x1b\[[0-9;]*m/g,'');if(event.t==='stat')time=event.time;}catch{/* Non-JSON diagnostic text is not guest serial. */}}
  if(message.bin&&new Uint8Array(message.bin)[0]===1){frame=decodeFramebuffer(message.bin);thread.postMessage({op:'frame-ack'});}
  if(message.flashDelta)deltas.push(message.flashDelta);
  for(const waiter of [...waiters])if(waiter.match(message)){waiters.delete(waiter);clearTimeout(waiter.timer);waiter.resolve(message);}
});
const request=(message,match)=>new Promise((resolve,reject)=>{const waiter={match,resolve,timer:setTimeout(()=>{waiters.delete(waiter);reject(Error(`Worker timeout: ${message.op}`));},120000)};waiters.add(waiter);thread.postMessage(message);});
const copied=bytes=>Uint8Array.from(bytes).buffer;
const wait=async(predicate,label)=>{const deadline=Date.now()+120000;while(Date.now()<deadline){if(error)throw error;if(predicate())return;await new Promise(r=>setTimeout(r,20));}throw Error(`Timed out ${label}; guest=${time}; serial=${serial.slice(-1600)}`);};
const flash=async()=>{const requestId=++sequence;return new Uint8Array((await request({op:'flash-snapshot',requestId},m=>m.flashReply===requestId)).data);};
const input=async(x,y,down)=>{const inputId=++sequence;const r=await request({op:'text',inputId,data:JSON.stringify({t:'touch',x,y,down:+down})},m=>m.inputQueued===inputId);return r.time;};
const tap=async(x,y)=>{const down=await input(x,y,true);await wait(()=>time>=down+0.3,'touch hold');return {x,y,down,up:await input(x,y,false)};};
const changedPages=(a,b)=>{const changed=[];assert.equal(a.length,b.length);for(let offset=0;offset<a.length;offset+=4096)if(!a.subarray(offset,offset+4096).every((value,i)=>value===b[offset+i]))changed.push(offset);return changed;};
const receipt={schemaVersion:1,flow,expectUnknownHci:allowUnknown,checkedAt:new Date().toISOString(),backend:'production-worker-wasm',firmware:{version:'1.5.1',sha256:firmwareSha},inputs:Object.fromEntries(Object.entries(inputs).map(([key,value])=>[key,{bytes:value.length,sha256:sha(value)}])),runs:[],limitations:['Virtual HCI controller; no physical radio, phone connection, pairing, or cloud-account acceptance.','Symbol metadata must independently match the fixed firmware; this probe does not recover addresses.']};
try {
  await request({op:'init',wasm:copied(inputs.wasm),frameAck:true},m=>m.ready);
  let restored=null;
  for(let run=0;run<2;run++){
    serial='';frame=null;time=0;deltas.length=0;
    await request({op:'create',board:'stackchan',flash_mb:16,psram_mb:8,jit:true},m=>m.created);
    for(const[kind,bytes]of [[0,inputs.rom],[5,restored||inputs.firmware]]){
      const result=await request({op:'load',kind,data:copied(bytes)},m=>m.loaded===kind);assert.equal(result.ok,true);
    }
    const baseline=await flash(),requestId=++sequence;
    const result=await request({op:'ble-hci',requestId,data:copied(inputs.symbols)},m=>m.bleReply===requestId);assert.equal(result.ok,true,result.error);
    const watch=++sequence;await request({op:'flash-watch',requestId:watch},m=>m.flashReply===watch);
    assert.equal((await request({op:'start'},m=>Object.hasOwn(m,'started'))).started,true);
    await wait(()=>serial.includes('start startup worker')&&time>=3,'Welcome page');
    const inputsTaken=[],milestones=[];
    const saveFrame=async label=>{
      const item={label,guestSeconds:time,sha256:sha(frame.pixels)};milestones.push(item);
      if(arg('--frame-prefix')){const prefix=resolve(arg('--frame-prefix'))+`-${run+1}-${label}`;await writeFile(prefix+'.rgba',frame.pixels);await writeFile(prefix+'.json',JSON.stringify({width:frame.width,height:frame.height}));}
      return item.sha256;
    };
    let before;
    if(flow==='setup'){
      inputsTaken.push(await tap(235,188));await wait(()=>serial.includes('start servo test'),'servo-test page');
      await wait(()=>time>=inputsTaken.at(-1).up+0.8,'servo page transition');
      inputsTaken.push(await tap(88,188));await wait(()=>serial.includes('start wifi setup'),'QR page');
      await wait(()=>time>=inputsTaken.at(-1).up+0.8&&frame,'QR draw');
      before=await saveFrame('qr');
      inputsTaken.push(await tap(232,211));
    }else{
      inputsTaken.push(await tap(88,188));await wait(()=>serial.includes('[LauncherView] init'),'launcher');
      await wait(()=>time>=inputsTaken.at(-1).up+0.8,'launcher transition');
      for(let i=0;i<5;i++){inputsTaken.push(await tap(294,120));await wait(()=>time>=inputsTaken.at(-1).up+0.7,'launcher scroll');}
      before=await saveFrame('dance-menu');
      inputsTaken.push(await tap(160,120));await wait(()=>serial.includes('[DANCE] on open'),'DANCE open');
    }
    await wait(()=>serial.includes('[HAL-BLE] init done'),'real guest BLE initialization return');
    await wait(()=>serial.includes('GAP procedure initiated: advertise'),'real guest advertising');
    await wait(()=>time>=inputsTaken.at(-1).up+2&&frame&&sha(frame.pixels)!==before,'application framebuffer');
    await saveFrame(flow==='dance'?'dance-avatar':'setup-next');
    if(flow==='dance'){
      // Official HomeGesture starts in bottom 20 pixels and needs >50 upward pixels.
      const homeVisible=()=>{let n=0;for(let y=190;y<235;y++)for(let x=90;x<230;x++){const p=(y*320+x)*4;if(frame.pixels[p]>140&&frame.pixels[p+2]>200&&frame.pixels[p+1]<170)n++;}return n>1000;};
      for(let attempt=0;attempt<3&&!homeVisible();attempt++){
        const down=await input(160,232,true);await wait(()=>time>=down+0.4,'home swipe down');
        const moves=[];for(const y of [212,192,172,152]){const at=await input(160,y,true);moves.push({y,at});await wait(()=>time>=at+0.15,'home swipe motion');}
        const up=await input(160,152,false);inputsTaken.push({type:'swipe',x:160,fromY:232,toY:152,down,moves,up});
        await wait(()=>time>=up+0.7,'Home reveal');
      }
      assert.ok(homeVisible(),'official Home button must be visible before it is clicked');await saveFrame('home-reveal');
      const startups=(serial.match(/start startup worker/g)||[]).length;
      inputsTaken.push(await tap(160,212));await wait(()=>serial.includes('[DANCE] on close'),'official Home close');
      await wait(()=>(serial.match(/start startup worker/g)||[]).length>startups,'unconfigured guest startup after Home reboot');
      const restarted=time;await wait(()=>time>=restarted+1,'Welcome redraw');inputsTaken.push(await tap(88,188));
      await wait(()=>serial.includes('warm boot was requested, app index: 5'),'official warm reboot to DANCE launcher');
      const returned=time;await wait(()=>time>=returned+1&&frame,'returned launcher draw');await saveFrame('returned-menu');
      const opens=(serial.match(/\[DANCE\] on open/g)||[]).length;
      const advertises=(serial.match(/GAP procedure initiated: advertise/g)||[]).length;
      inputsTaken.push(await tap(160,120));
      await wait(()=>(serial.match(/\[DANCE\] on open/g)||[]).length>opens,'DANCE reopen');
      await wait(()=>(serial.match(/GAP procedure initiated: advertise/g)||[]).length>advertises,'reopened advertising');
      const reentered=time;await wait(()=>time>=reentered+2&&frame,'reentered avatar');await saveFrame('reentered-avatar');
    }
    const unknown=serial.split(/\r?\n/).filter(line=>/hci_err=/.test(line));
    if(allowUnknown)assert.ok(unknown.some(line=>line.includes('Unknown HCI Command')),'baseline must reproduce Unknown HCI Command');
    else assert.equal(unknown.length,0,`unexpected HCI command errors: ${unknown.join('; ')}`);
    assert.doesNotMatch(serial,/Guru Meditation|assert failed|panic'ed|LoadProhibited/,'guest must remain healthy');
    const stopped=await request({op:'stop'},m=>Object.hasOwn(m,'stopped'));assert.equal(stopped.stopped,0);
    restored=await flash();
    assert.deepEqual(restored.subarray(0x20000),baseline.subarray(0x20000),'temporary controller code must never enter a persisted app/asset page');
    const changed=changedPages(baseline,restored);
    assert.ok(changed.every(offset=>offset>=0x9000&&offset<0x10000),'only NVS / otadata / PHY data may change');
    const receivedPages=[...new Set(deltas.flatMap(delta=>delta.pages.map(page=>page.offset)))].sort((a,b)=>a-b);
    assert.ok(receivedPages.every(offset=>offset>=0x9000&&offset<0x10000),'autosave must not contain a patched application page');
    if(run===0)assert.ok(changed.length>0,'real guest boot must make persistent state, not only a synthetic code-overlay test');
    const cleanSerial=serial.replace(/\x1b\[[0-9;]*m/g,'');
    receipt.runs.push({restoredPreviousFlash:!!run,guestSeconds:time,inputs:inputsTaken,milestones,unknownHciCommands:unknown,beforeFrameSha256:before,afterFrameSha256:sha(frame.pixels),changedPages:changed,autosavePages:receivedPages,serialEvidence:cleanSerial.split(/\r?\n/).filter(line=>/DANCE|warm boot|Unknown HCI Command|start startup worker|start servo test|start wifi setup|start app config server|\[HAL-BLE\] init(?:\s|$)|host.*sync|advertis/i.test(line)).map(line=>line.replace(/(?:[0-9a-f]{2}:){5}[0-9a-f]{2}/gi,'<generated-mac>'))});
    if(arg('--frame-prefix')){const prefix=resolve(arg('--frame-prefix'))+`-${run+1}`;await writeFile(prefix+'.rgba',frame.pixels);await writeFile(prefix+'.json',JSON.stringify({width:frame.width,height:frame.height}));}
  }
  receipt.status='pass';
  if(arg('--receipt'))await writeFile(resolve(arg('--receipt')),JSON.stringify(receipt,null,2)+'\n');
  console.log(JSON.stringify(receipt,null,2));
} finally {
  for(const waiter of waiters)clearTimeout(waiter.timer);
  await thread.terminate();
}
