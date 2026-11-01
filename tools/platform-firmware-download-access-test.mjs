import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,readFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {fetchBytes} from '../web/platform/runtime.js';
import {resolveBurnerDownload,resolveCatalogDownloads,stableBurnerCdnUrl} from './resolve-firmware-downloads.mjs';
import {runCatalogSync} from './sync-firmware-catalog.mjs';

const URL='https://burner.m5stack.com/api/v1/firmwares/1897200000000002059/versions/2098281031283200002/download';
const CDN='https://m5burner-cdn.m5stack.com/firmware/abcdef123456.bin',NOW='2026-11-01T02:00:00Z',HASH='a'.repeat(64);
const captcha=(status=400)=>Response.json({code:400,msg:'验证码令牌不正确',data:{errorCode:'BURN_CAPTCHA_INVALID'},requestId:'do-not-retain'},{status});
const fixture=()=>({firmwares:{items:[{id:'c6l',source:{url:URL,sha256:HASH},simulationVerified:false}]},watchlist:{entries:[{assets:[{url:URL,sha256:HASH}]}]},report:{summary:{}}});

test('HTTP 400 and HTTP 200 CAPTCHA JSON stop downloads before image parsing or caching',async()=>{
  for(const status of [400,200])await assert.rejects(fetchBytes(URL,{fetchImpl:async()=>captcha(status)}),error=>{
    assert.equal(error.code,'FIRMWARE_AUTH_REQUIRED');assert.match(error.message,/本地文件/);
    assert.equal(error.details.officialUrl,'https://burner.m5stack.com/firmware/1897200000000002059');
    assert.equal(error.details.providerCode,'BURN_CAPTCHA_INVALID');assert.ok(!JSON.stringify(error).includes('do-not-retain'));return true;
  });
});
test('unexpected provider JSON is never treated as BIN; ordinary binary and non-provider failures keep their semantics',async()=>{
  for(const payload of [{code:200,data:{}},{code:500}])await assert.rejects(fetchBytes(URL,{fetchImpl:async()=>Response.json(payload)}),{code:'FIRMWARE_DOWNLOAD_RESPONSE'});
  const bytes=new Uint8Array([0xe9,1,13]);assert.deepEqual(await fetchBytes(URL,{fetchImpl:async()=>new Response(bytes)}),bytes);
  await assert.rejects(fetchBytes('https://firmware.example/missing.bin',{fetchImpl:async()=>captcha()}),{code:'HTTP_ERROR'});
});
test('provider JSON parsing is bounded and cancelled on overflow or abort',async()=>{
  let cancelled=false;
  const body=new ReadableStream({pull(controller){controller.enqueue(new Uint8Array(9000));},cancel(){cancelled=true;}});
  await assert.rejects(fetchBytes(URL,{fetchImpl:async()=>new Response(body,{headers:{'content-type':'application/json'}})}),{code:'FIRMWARE_DOWNLOAD_RESPONSE'});assert.ok(cancelled);
  const controller=new AbortController();let started;
  const reading=new Promise(resolve=>{started=resolve;});cancelled=false;
  const pending=fetchBytes(URL,{signal:controller.signal,fetchImpl:async()=>new Response(new ReadableStream({pull(){started();},cancel(){cancelled=true;}}),{headers:{'content-type':'application/json'}})});
  await reading;controller.abort();await assert.rejects(pending,{code:'FETCH_ABORTED'});assert.ok(cancelled);
});
test('predeploy resolves only public redirects and HEAD-checks CDN without reading a binary body or sending tokens',async()=>{
  const calls=[];let cancelled=false;
  const r=await resolveBurnerDownload(URL,{now:NOW,fetchImpl:async(url,options)=>{
    calls.push({url,method:options.method||'GET'});assert.equal(options.credentials,'omit');assert.equal(options.redirect,'manual');assert.equal(options.headers,undefined);
    if(url===URL)return new Response(new ReadableStream({cancel(){cancelled=true;}}),{status:302,headers:{location:CDN}});
    assert.equal(options.method,'HEAD');return new Response(null,{headers:{'content-type':'application/octet-stream','content-length':'8384512'}});
  }});
  assert.equal(r.status,'resolved');assert.equal(r.directUrl,CDN);assert.ok(cancelled);assert.deepEqual(calls,[{url:URL,method:'GET'},{url:CDN,method:'HEAD'}]);assert.equal(r.sha256,undefined);
});
test('CAPTCHA requires a user; signed or untrusted redirects are neither followed nor stored',async()=>{
  let calls=0;const r=await resolveBurnerDownload(URL,{now:NOW,fetchImpl:async()=>{calls++;return captcha();}});
  assert.equal(calls,1);assert.equal(r.status,'verification-required');assert.equal(r.directUrl,undefined);assert.ok(!JSON.stringify(r).includes('do-not-retain'));
  for(const location of [CDN+'?token=secret',CDN+'#secret','http://m5burner-cdn.m5stack.com/firmware/a.bin','https://evil.example/fw.bin','https://secret@m5burner-cdn.m5stack.com/firmware/a.bin']){
    calls=0;assert.equal(stableBurnerCdnUrl(location),null);
    const result=await resolveBurnerDownload(URL,{fetchImpl:async()=>{calls++;return new Response(null,{status:302,headers:{location}});}});
    assert.equal(calls,1);assert.equal(result.status,'unstable-or-untrusted-redirect');assert.ok(!JSON.stringify(result).includes('secret'));
  }
});
test('failed CDN checks and nonredirecting responses do not become public file addresses',async()=>{
  for(const response of [()=>new Response(null,{status:404}),()=>new Response(null,{status:302,headers:{location:CDN+'?secret=x'}}),()=>new Response(null,{headers:{'content-type':'text/html'}})]){
    const r=await resolveBurnerDownload(URL,{fetchImpl:async url=>url===URL?new Response(null,{status:302,headers:{location:CDN}}):response()});assert.equal(r.status,'cdn-unavailable');assert.equal(r.directUrl,undefined);
  }
  let cancelled=false;const r=await resolveBurnerDownload(URL,{fetchImpl:async()=>new Response(new ReadableStream({cancel(){cancelled=true;}}))});assert.equal(r.status,'no-public-redirect');assert.ok(cancelled);
});
test('catalog preserves hashes, aliases and simulator status; repeated checks do not churn or overwrite a pinned CDN',async()=>{
  const f=fixture();let calls=0;
  const fetchImpl=async url=>{calls++;return url===URL?new Response(null,{status:302,headers:{location:CDN}}):new Response(null);};
  await resolveCatalogDownloads(f,{fetchImpl,now:NOW});assert.equal(f.firmwares.items[0].source.url,CDN);assert.equal(f.firmwares.items[0].source.sha256,HASH);assert.equal(f.firmwares.items[0].simulationVerified,false);
  assert.equal(f.watchlist.entries[0].assets[0].url,CDN);assert.equal(f.watchlist.entries[0].assets[0].upstreamUrl,URL);assert.equal(calls,2);
  const saved=structuredClone(f);await resolveCatalogDownloads(f,{now:'2026-11-01T03:00:00Z',fetchImpl:()=>{throw Error('Must keep pinned CDN');}});assert.deepEqual(f,saved);
  const blocked=fixture();await resolveCatalogDownloads(blocked,{now:NOW,fetchImpl:async()=>captcha()});assert.equal(blocked.firmwares.items[0].source.url,URL);assert.equal(blocked.report.summary.downloadsNeedVerification,1);
  const before=structuredClone(blocked);await resolveCatalogDownloads(blocked,{now:'2026-11-01T03:00:00Z',fetchImpl:()=>{throw Error('Same-day check must be reused');}});assert.deepEqual(blocked,before);
  await resolveCatalogDownloads(blocked,{now:'2026-11-02T03:00:00Z',fetchImpl});assert.equal(blocked.firmwares.items[0].source.url,CDN);
});
test('sync invokes address checks before writing, dry-run stays read-only, snapshot-only stays offline',async()=>{
  const directory=await mkdtemp(join(tmpdir(),'firmware-download-check-'));
  try{
    const catalog=join(directory,'web/platform/catalog');await mkdir(catalog,{recursive:true});
    for(const[name,value]of Object.entries({'products.json':{items:[]},'firmwares.json':fixture().firmwares,'firmware-watchlist.json':{schemaVersion:'1',categories:[],entries:[]},'firmware-release-snapshot.json':{entries:[]}}))await writeFile(join(catalog,name),JSON.stringify(value,null,2)+'\n');
    const base=pathToFileURL(directory+'/'),original=await readFile(join(catalog,'firmwares.json'),'utf8');let calls=0;
    const fetchImpl=async()=>{calls++;return captcha();};
    await runCatalogSync({base,now:NOW,snapshotOnly:true,dryRun:true,fetchImpl});assert.equal(calls,0);
    const preview=await runCatalogSync({base,now:NOW,dryRun:true,fetchImpl});assert.equal(calls,1);assert.equal(preview.report.summary.downloadsNeedVerification,1);assert.equal(await readFile(join(catalog,'firmwares.json'),'utf8'),original);
    await runCatalogSync({base,now:NOW,fetchImpl});assert.equal(calls,2);
    const again=await runCatalogSync({base,now:'2026-11-01T03:00:00Z',fetchImpl});assert.equal(calls,2);assert.deepEqual(again.changed,[]);
  }finally{await rm(directory,{recursive:true,force:true});}
});
