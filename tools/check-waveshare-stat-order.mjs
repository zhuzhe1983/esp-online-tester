#!/usr/bin/env node
// Delay genuine statistics only; guest UART, RMT, input and reload stay real.
import assert from 'node:assert/strict';
import {readFile,writeFile,unlink} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {randomUUID,createHash} from 'node:crypto';
import {execFileSync,spawnSync} from 'node:child_process';
const args=process.argv.slice(2),revision=args.length?args[1]:null;
if(args.length&&!(args.length===2&&args[0]==='--revision'&&/^[a-f0-9]{7,40}$/.test(revision)))throw Error('Usage: node tools/check-waveshare-stat-order.mjs [--revision COMMIT]');
const root=fileURLToPath(new URL('../',import.meta.url)),directory=new URL('./',import.meta.url),files=[],inputs={};
const needle="thread.on('message',data=>adapter.onmessage?.({data}));";
const delayed=`const statTimers=new Set(),terminate=adapter.terminate;adapter.terminate=()=>{for(const timer of statTimers)clearTimeout(timer);statTimers.clear();return terminate();};
  thread.on('message',data=>{let text;try{text=JSON.parse(data.text);}catch{}if(text?.t==='stat'){const timer=setTimeout(()=>{statTimers.delete(timer);adapter.onmessage?.({data});},250);statTimers.add(timer);}else adapter.onmessage?.({data});});`;
try {
  for(const name of ['platform-waveshare-c5-core-test.mjs','platform-waveshare-c6-zero-test.mjs','platform-waveshare-h2-test.mjs']){
    const source=revision?execFileSync('git',['show',`${revision}:tools/${name}`],{cwd:root,encoding:'utf8'}):await readFile(new URL(name,directory),'utf8');
    assert.equal(source.split(needle).length,2,'Expected one actual Worker transport in '+name);
    inputs[name]=createHash('sha256').update(source).digest('hex');
    const file=new URL(`.waveshare-stat-order-${randomUUID()}.mjs`,directory);files.push(file);
    await writeFile(file,source.replace(needle,delayed),{flag:'wx'});
  }
  const result=spawnSync(process.execPath,['--test',...files.map(file=>fileURLToPath(file))],{cwd:root,env:process.env,encoding:'utf8',timeout:180000,maxBuffer:4*1024*1024});
  process.stdout.write(result.stdout||'');process.stderr.write(result.stderr||'');
  console.log(JSON.stringify({source:revision||'working-tree',delayMs:250,onlyOriginalStatMessagesDelayed:true,sourceSha256:inputs,exitCode:result.status}));
  if(result.error)throw result.error;
  process.exitCode=result.status??1;
} finally {await Promise.all(files.map(file=>unlink(file).catch(error=>{if(error.code!=='ENOENT')throw error;})));}
