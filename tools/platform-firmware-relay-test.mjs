import test from 'node:test';
import assert from 'node:assert/strict';
import {Readable} from 'node:stream';
import {EventEmitter} from 'node:events';
import {createHash} from 'node:crypto';
import {firmwareAllowlist,allowedFirmwareRedirect,openFirmwareUpstream,startFirmwareRelay,loadFirmwareAllowlist} from './firmware-relay.mjs';
import {firmwareRelayTarget,firmwareDownloadUrl,configureFirmwareRelay} from '../web/platform/firmware-relay.js';
import {fetchBytes,createPlatformRuntime,sha256} from '../web/platform/runtime.js';
import {firmwareCache} from '../web/platform/firmware-cache.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {readFile} from 'node:fs/promises';

const origin='https://zhuzhe1983.github.io',url='https://github.com/BruceDevices/firmware/releases/download/1.16.1/Bruce-lilygo-t-deck.bin';
const data=Buffer.from([0xe9,1,2,3]),hash=createHash('sha256').update(data).digest('hex');
const catalog={items:[{source:{url,sha256:hash}}]},allowlist=firmwareAllowlist(catalog);
const response=(bytes=data,status=200,headers={})=>Object.assign(Readable.from([bytes]),{statusCode:status,headers});
const requestUrl=(base,source=url,digest=hash)=>base+'/firmware?'+new URLSearchParams({url:source,sha256:digest});
const get=(base,options={})=>fetch(requestUrl(base),{headers:{Origin:origin},...options});
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function waitFor(check){for(let i=0;i<200;i++){if(check())return;await delay(5);}throw Error('Timed out');}

test('only fixed upstream BIN URLs with hashes are routed; ordinary CORS downloads stay direct',()=>{
  const base='https://relay.example';
  assert.equal(firmwareDownloadUrl(url,hash,base),requestUrl(base));
  for(const source of [url+'?token=secret','https://github.com/owner/repo/blob/main/file.bin','https://github.com.evil.example/owner/repo/releases/download/v1/file.bin','https://user:pass@github.com/o/r/releases/download/v1/f.bin','http://github.com/o/r/releases/download/v1/f.bin'])assert.equal(firmwareRelayTarget(source),null);
  assert.equal(firmwareDownloadUrl(url,undefined,base),url);assert.equal(firmwareDownloadUrl(url,hash,''),url);
  assert.equal(firmwareDownloadUrl('https://m5burner-cdn.m5stack.com/firmware/test.bin',hash,base),'https://m5burner-cdn.m5stack.com/firmware/test.bin');
  assert.throws(()=>configureFirmwareRelay('http://public.example'));assert.throws(()=>configureFirmwareRelay('https://u:p@relay.example'));
  const mesh='https://release.meshtastic.org/2.7.26.54e0d8d/firmware-t-deck-2.7.26.54e0d8d.factory.bin';assert.equal(firmwareRelayTarget(mesh),mesh);
});

test('catalog allowlist includes current pinned artifacts and rejects unknown or changed versions before any upstream request',async()=>{
  const actual=await loadFirmwareAllowlist(),current=JSON.parse(await readFile(new URL('../web/platform/catalog/firmwares.json',import.meta.url),'utf8'));
  for(const id of ['lilygo-t-deck-bruce','lilygo-t-deck-meshtastic-stable']){
    const source=current.items.find(item=>item.id===id)?.source;assert.ok(source,`${id} retains an executable source`);
    assert.match(source.sha256,/^[a-f0-9]{64}$/i);assert.ok(actual.get(source.url)?.has(source.sha256.toLowerCase()),`${id} uses the catalog's current URL and hash after release sync`);
  }
  let calls=0;const relay=await startFirmwareRelay({port:0,allowlist,openUpstream:async()=>{calls++;return response();}});
  try{
    for(const value of [requestUrl(relay.url,url,'0'.repeat(64)),requestUrl(relay.url,url.replace('1.16.1','v999')),requestUrl(relay.url,'https://127.0.0.1/private'),requestUrl(relay.url)+'&url='+encodeURIComponent(url)])assert.equal((await fetch(value,{headers:{Origin:origin}})).status,403);
    assert.equal((await get(relay.url,{headers:{Origin:'https://other.example'}})).status,403);
    assert.equal((await get(relay.url,{method:'POST'})).status,405);assert.equal(calls,0);
  }finally{await relay.close();}
});

test('streaming response enables only the configured origin, forwards no credentials and preserves exact bytes',async()=>{
  const urls=[];const relay=await startFirmwareRelay({port:0,allowlist,openUpstream:async value=>{urls.push(value);return response(data,200,{'content-length':String(data.length),'set-cookie':'private=do-not-forward','location':'https://private.invalid'});}});
  try{
    const r=await get(relay.url);assert.equal(r.status,200);assert.equal(r.headers.get('access-control-allow-origin'),origin);assert.equal(r.headers.get('cache-control'),'no-store');assert.equal(r.headers.get('set-cookie'),null);assert.equal(r.headers.get('location'),null);assert.deepEqual(Buffer.from(await r.arrayBuffer()),data);assert.deepEqual(urls,[url]);
  }finally{await relay.close();}
});

test('first bytes arrive before the upstream completes, and a stalled connection times out',async()=>{
  let upstream;const relay=await startFirmwareRelay({port:0,allowlist,openUpstream:async()=>{upstream=Object.assign(new Readable({read(){}}),{statusCode:200,headers:{}});upstream.push(data.subarray(0,2));return upstream;}});
  try{
    const r=await get(relay.url),reader=r.body.getReader();assert.deepEqual(Buffer.from((await reader.read()).value),data.subarray(0,2));
    upstream.push(data.subarray(2));upstream.push(null);assert.deepEqual(Buffer.from((await reader.read()).value),data.subarray(2));assert.equal((await reader.read()).done,true);
  }finally{await relay.close();}
  const stalled=await startFirmwareRelay({port:0,allowlist,limits:{timeoutMs:20},openUpstream:(_url,{signal})=>new Promise((_resolve,reject)=>signal.addEventListener('abort',()=>reject(signal.reason),{once:true}))});
  try{const r=await get(stalled.url);assert.equal(r.status,504);assert.equal((await r.json()).error.code,'UPSTREAM_TIMEOUT');}finally{await stalled.close();}
});

test('GitHub signed redirects are followed but private destinations, arbitrary hosts and redirect loops are refused',async()=>{
  const target='https://release-assets.githubusercontent.com/github-production-release-asset/1/file?signature=public-temporary';
  assert.equal(allowedFirmwareRedirect(target,url),true);
  for(const value of ['https://127.0.0.1/file','https://evil.example/file','https://release-assets.githubusercontent.com.evil.example/github-production-release-asset/1','http://release-assets.githubusercontent.com/github-production-release-asset/1','https://u:p@release-assets.githubusercontent.com/github-production-release-asset/1'])assert.equal(allowedFirmwareRedirect(value,url),false);
  for(const destination of [target,'https://127.0.0.1/private']){
    let count=0;const relay=await startFirmwareRelay({port:0,allowlist,openUpstream:async()=>++count===1?response('',302,{location:destination}):response()});
    try{const r=await get(relay.url);if(destination===target){assert.deepEqual(Buffer.from(await r.arrayBuffer()),data);assert.equal(count,2);}else{assert.equal(r.status,502);assert.equal(count,1);}}finally{await relay.close();}
  }
  let calls=0;const relay=await startFirmwareRelay({port:0,allowlist,openUpstream:async()=>{calls++;return response('',302,{location:target});}});
  try{assert.equal((await get(relay.url)).status,502);assert.equal(calls,4);}finally{await relay.close();}
});

test('DNS cannot return private or mixed addresses; TLS connects to the checked address with certificate verification intact',async()=>{
  let connects=0;
  for(const addresses of [[{address:'127.0.0.1',family:4}],[{address:'1.1.1.1',family:4},{address:'169.254.169.254',family:4}],[]])await assert.rejects(()=>openFirmwareUpstream(url,{lookup:async()=>addresses,request:()=>{connects++;}}),{code:'DESTINATION_DENIED'});
  assert.equal(connects,0);
  await openFirmwareUpstream(url,{lookup:async()=>[{address:'1.1.1.1',family:4}],request:(_url,options,callback)=>{
    assert.equal(options.rejectUnauthorized,undefined);assert.equal(options.agent,false);assert.equal(options.headers.Cookie,undefined);assert.equal(options.headers.Authorization,undefined);
    options.lookup('github.com',{},(error,address,family)=>{assert.equal(error,null);assert.equal(address,'1.1.1.1');assert.equal(family,4);});
    options.lookup('github.com',{all:true},(error,addresses)=>{assert.equal(error,null);assert.deepEqual(addresses,[{address:'1.1.1.1',family:4}]);});
    const req=new EventEmitter();req.end=()=>callback(response());return req;
  }});
  const controller=new AbortController(),pending=openFirmwareUpstream(url,{signal:controller.signal,lookup:()=>new Promise(()=>{}),request:()=>{throw Error('Must not connect');}});controller.abort();await assert.rejects(pending,{name:'AbortError'});
});

test('404 remains an upstream error; oversized, encoded and truncated streams cannot be accepted as firmware',async()=>{
  for(const [body,status,headers,expected]of [[data,404,{},404],[data,200,{'content-length':'100'},413],[data,200,{'content-encoding':'gzip'},502],[data,200,{'content-length':'invalid'},413]]){
    const relay=await startFirmwareRelay({port:0,allowlist,limits:{maxBytes:8},openUpstream:async()=>response(body,status,headers)});
    try{const r=await get(relay.url);assert.equal(r.status,expected);if(status===404)assert.equal(r.headers.get('x-firmware-relay-error'),'UPSTREAM_HTTP');await r.arrayBuffer();}finally{await relay.close();}
  }
  for(const bytes of [Buffer.alloc(9),Buffer.from('wrong'),data.subarray(0,2)]){
    const relay=await startFirmwareRelay({port:0,allowlist,limits:{maxBytes:8},openUpstream:async()=>response(bytes)});
    try{await assert.rejects(async()=>{const r=await get(relay.url);await r.arrayBuffer();});}finally{await relay.close();}
  }
});

test('concurrency, rate quotas and client cancellation release upstream resources',async()=>{
  let started=false,aborted=false;
  const relay=await startFirmwareRelay({port:0,allowlist,limits:{maxPerIp:1,requestsPerMinute:2},openUpstream:(_url,{signal})=>new Promise((_resolve,reject)=>{started=true;signal.addEventListener('abort',()=>{aborted=true;reject(signal.reason);},{once:true});})});
  const controller=new AbortController();const pending=get(relay.url,{signal:controller.signal});pending.catch(()=>{});
  try{await waitFor(()=>started);assert.equal((await get(relay.url)).status,429);controller.abort();await assert.rejects(pending);await waitFor(()=>aborted);}finally{await relay.close();}
  const limited=await startFirmwareRelay({port:0,allowlist,limits:{requestsPerMinute:1},openUpstream:async()=>response()});
  try{await (await get(limited.url)).arrayBuffer();assert.equal((await get(limited.url)).status,429);}finally{await limited.close();}
});

test('client progress and errors retain the publisher URL while transport uses the relay',async()=>{
  const events=[],relayUrl='https://relay.example';
  const bytes=await fetchBytes(url,{sha256:hash,relayUrl,onProgress:e=>events.push(e),fetchImpl:async(value,options)=>{assert.equal(value,requestUrl(relayUrl));assert.equal(options.credentials,'omit');assert.equal(options.mode,'cors');return new Response(data);}});
  assert.deepEqual(Buffer.from(bytes),data);assert.ok(events.every(event=>event.url===url));
  await assert.rejects(()=>fetchBytes(url,{sha256:hash,relayUrl,fetchImpl:async()=>new Response('',{status:429})}),e=>e.code==='FIRMWARE_RELAY_FAILED'&&e.details.url===url);
  await assert.rejects(()=>fetchBytes(url,{sha256:hash,relayUrl,fetchImpl:async()=>new Response('',{status:404,headers:{'X-Firmware-Relay-Error':'UPSTREAM_HTTP'}})}),e=>e.code==='HTTP_ERROR'&&e.details.status===404);
});

test('runtime still checks SHA-256 after relay transport before creating any worker or caching bad bytes',async()=>{
  const definitions={};for(const name of ['products','accessories','firmwares'])definitions[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url),'utf8')).items;
  const scene=createDefaultScene(definitions,'lilygo-t-deck');scene.firmware={url,sha256:hash,bootMode:'rom'};let calls=0,workers=0;
  configureFirmwareRelay('https://relay.example');
  const runtime=createPlatformRuntime({catalog:definitions,baseUrl:origin+'/platform/',fetchImpl:async value=>{calls++;assert.equal(value,requestUrl('https://relay.example'));return new Response('wrong');},workerFactory:()=>{workers++;throw Error('Must not create worker');}});
  try{await runtime.configure(scene);await assert.rejects(()=>runtime.loadFirmware(),{code:'HASH_MISMATCH'});assert.equal(calls,1);assert.equal(workers,0);assert.equal(await firmwareCache.get(url,hash),null);}finally{await runtime.dispose();configureFirmwareRelay('');}
});
