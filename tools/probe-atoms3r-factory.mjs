#!/usr/bin/env node
// Diagnose an unchanged caller-selected AtomS3R image; the firmware bytes stay in RAM.
import assert from 'node:assert/strict';
import {readFileSync,mkdirSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {performance} from 'node:perf_hooks';
import {createJitHost} from '../web/wasm/jit.mjs';
const args=process.argv.slice(2),arg=name=>args[args.indexOf(name)+1];
for(const flag of ['--url','--sha256'])assert(args.includes(flag)&&arg(flag),`Required ${flag}; select the catalog firmware explicitly.`);
const url=arg('--url'),expected=arg('--sha256');assert(/^https:\/\//.test(url));assert(/^[a-f0-9]{64}$/.test(expected));
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const romPath=resolve(arg('--rom')&&args.includes('--rom')?arg('--rom'):'web/wasm/fw/esp32s3_rev0_rom.elf');
const nativePath=resolve(arg('--native')&&args.includes('--native')?arg('--native'):'target/release/esp32sim');
const rom=readFileSync(romPath),wasm=readFileSync('web/wasm/esp32sim.wasm');
const evidenceDir='output/atoms3r-factory-probe';mkdirSync(evidenceDir,{recursive:true});
const response=await fetch(url,{signal:AbortSignal.timeout(45000)});assert(response.ok,`Firmware HTTP ${response.status}`);
const firmware=new Uint8Array(await response.arrayBuffer());assert.equal(hash(firmware),expected,'Firmware SHA-256 differs; do not reuse this diagnosis for a changed image.');
const receipt={checkedAt:new Date().toISOString().replace(/\.\d{3}Z$/,'Z'),source:{url,resolvedUrl:response.url,mediaType:response.headers.get('content-type'),bytes:firmware.length,sha256:hash(firmware)},inputs:{romSha256:hash(rom),wasmSha256:hash(wasm),nativeSha256:hash(readFileSync(nativePath))},board:'atoms3r',flashMiB:8,psramMiB:8,firmwarePatched:false,runs:[]};
const summarize=serial=>({cpu1Panic:/Core\s+1 panic'ed \(LoadProhibited\)/.test(serial),pc:serial.match(/PC\s*:\s*(0x[0-9a-f]+)/i)?.[1]??null,exceptionAddress:serial.match(/EXCVADDR:\s*(0x[0-9a-f]+)/i)?.[1]??null,serialEvidence:[...new Set(serial.replace(/\x1b\[[0-9;]*m/g,'').replace(/\x1b/g,'').split(/\r?\n/).filter(line=>/ESP-IDF|Starting app cpu|panic'ed|PC\s*:|A2\s*:|EXCCAUSE|EXCVADDR/.test(line)))].slice(0,12)});
for(const noJit of [false,true]) {
  const command=['--chip','s3','--board','atoms3r','--flash-mb','8','--psram-mb','8','--boot','rom','--rom',romPath,'--flash-image','/dev/stdin','--console','both','--max-seconds','1','--no-reboot',...(noJit?['--no-jit']:[])];
  const run=spawnSync(nativePath,command,{input:firmware,encoding:'utf8',timeout:90000,maxBuffer:32*1024*1024});if(run.error)throw run.error;
  const log=(run.stdout||'')+(run.stderr||'');assert.equal(run.status,0,log.slice(-1000));
  writeFileSync(`${evidenceDir}/${noJit?'native-interpreter':'native-jit'}.log`,log);
  receipt.runs.push({backend:noJit?'native-interpreter':'native-jit',...summarize(log),displayReport:log.match(/\[atoms3r\][^\r\n]*/)?.[0]??null,stop:log.match(/\[emu\] stop:[^\r\n]*/)?.[0]?.replace(/ in [0-9.]+s wall = [0-9.]+ Minsn\/s;/,';')??null,rawLogBytes:Buffer.byteLength(log),rawLogSha256:hash(log)});
}
const dec=new TextDecoder(),enc=new TextEncoder();let w,serial='',frames=0;const messages=[],framePackets=[];
const jit=createJitHost(()=>w);
const {instance}=await WebAssembly.instantiate(wasm,{env:{...jit.imports,host_profile_now:()=>performance.now(),host_log:()=>{}}});w=instance.exports;
const withBytes=(bytes,fn)=>{const p=w.esp32sim_alloc(bytes.length);new Uint8Array(w.memory.buffer).set(bytes,p);try{return fn(p,bytes.length);}finally{w.esp32sim_free(p,bytes.length);}};
const emu=withBytes(enc.encode('atoms3r'),(p,n)=>w.esp32sim_new(p,n,8,8));
const drain=()=>{const count=w.esp32sim_out_take(emu);for(let i=0;i<count;i++){const p=w.esp32sim_out_ptr(emu,i),n=w.esp32sim_out_len(emu,i),bytes=new Uint8Array(w.memory.buffer,p,n);if(w.esp32sim_out_kind(emu,i)!==1){frames++;framePackets.push({kind:w.esp32sim_out_kind(emu,i),bytes:n,header:[...bytes.subarray(0,5)],nonzeroPayloadBytes:bytes.subarray(5).reduce((sum,b)=>sum+(b!==0),0)});continue;}const m=JSON.parse(dec.decode(bytes));if(m.t==='serial')serial+=m.data;else messages.push(m);}};
try {
  for(const [kind,data] of [[0,rom],[5,firmware]])assert.equal(withBytes(data,(p,n)=>w.esp32sim_load(emu,kind,p,n)),0);
  w.esp32sim_set_jit(emu,1);assert.equal(w.esp32sim_boot(emu,0),0);
  const hz=w.esp32sim_cpu_hz(emu),deadline=performance.now()+90000;
  while(w.esp32sim_cycles(emu)<hz&&performance.now()<deadline){assert.equal(w.esp32sim_run(emu,2000000,Date.now()),0);drain();if(summarize(serial).exceptionAddress)break;}
  w.esp32sim_reports(emu);drain();receipt.runs.push({backend:'wasm-jit',...summarize(serial),frames,framePackets,guestSeconds:w.esp32sim_cycles(emu)/hz,displayReports:messages.filter(m=>JSON.stringify(m).includes('[atoms3r]'))});writeFileSync(`${evidenceDir}/wasm-serial.log`,serial);
} finally {w.esp32sim_delete(emu);}
receipt.result=receipt.runs.every(r=>r.cpu1Panic&&r.pc==='0x40048839'&&r.exceptionAddress==='0x60ffe01c')?'same-cpu1-startup-failure':'different-result-inspect-before-reusing-diagnosis';
console.log(JSON.stringify(receipt,null,2));
