import test from 'node:test';
import assert from 'node:assert/strict';
import {createBurnerCaptchaClient,captchaPoint,captchaAttempt,CAPTCHA_ATTEMPT_MS,CAPTCHA_RESOURCE_MS} from '../web/platform/m5burner-captcha.js';
import {expectedCancellation} from '../web/platform/error-ui.js';
import {prepareBurnerSource} from '../web/platform/m5burner-download-ui.js';
import {createFirmwareCache} from '../web/platform/firmware-cache.js';
import {sha256,fetchBytes} from '../web/platform/runtime.js';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {gunzipSync} from 'node:zlib';
const url='https://burner.m5stack.com/api/v1/firmwares/1897200000000001943/versions/1897300000000004110/download';
const token='test-token-for-a-single-challenge',data=new Uint8Array([0xe9,0,1,2]);
const challenge={captchaToken:token,captchaImage:'data:image/png;base64,AAAA',width:520,height:300,targetCount:3,instruction:'Click in order: A, B, C',expiresInSeconds:120};
const rejected=()=>Response.json({code:400,data:{errorCode:'BURN_CAPTCHA_INVALID'},requestId:'private-request-id'},{status:400});

test('vendored OCR assets match their pins and retain the original OEM 1 components',async()=>{
  const base=new URL('../web/platform/vendor/tesseract/',import.meta.url),manifest=JSON.parse(await readFile(new URL('manifest.json',base)));
  for(const file of manifest.files){const bytes=await readFile(new URL(file.file,base));assert.equal(bytes.length,file.bytes);assert.equal(createHash('sha256').update(bytes).digest('hex'),file.sha256);}
  const data=gunzipSync(await readFile(new URL('eng.traineddata.gz',base)));assert.equal(data.readInt32LE(0),24);
  for(let id=0;id<17;id++)assert.equal(data.readBigInt64LE(4+8*id),-1n);
  for(const component of manifest.languageTransform.preservedComponents){
    const start=Number(data.readBigInt64LE(4+8*component.id)),end=component.id===23?data.length:Number(data.readBigInt64LE(4+8*(component.id+1)));
    assert.equal(end-start,component.bytes);assert.equal(createHash('sha256').update(data.subarray(start,end)).digest('hex'),component.sha256);
  }
  assert.equal(manifest.languageTransform.preservedComponents.length,7);
});

test('verification deadlines abort hanging work, discard late results and distinguish user cancellation',async()=>{
  assert.equal(CAPTCHA_ATTEMPT_MS,10000);assert.equal(CAPTCHA_RESOURCE_MS,20000);
  let taskSignal,late;
  const task=captchaAttempt(signal=>{taskSignal=signal;return new Promise((_,reject)=>{late=reject;});},{timeoutMs:10});
  await assert.rejects(task,{code:'CAPTCHA_TIMEOUT'});assert.equal(taskSignal.aborted,true);
  late(Error('late worker rejection'));await new Promise(resolve=>setTimeout(resolve,0));
  await assert.rejects(captchaAttempt(signal=>new Promise((_,reject)=>signal.addEventListener('abort',()=>reject(Object.assign(Error('aborted'),{code:'FETCH_ABORTED'})),{once:true})),{timeoutMs:10}),{code:'CAPTCHA_TIMEOUT'});
  await assert.rejects(captchaAttempt((signal,limit)=>{limit(5);return new Promise(()=>{});},{timeoutMs:1000}),{code:'CAPTCHA_TIMEOUT'});
  const controller=new AbortController();let calls=0;
  const cancelled=captchaAttempt(()=>{calls++;return new Promise(()=>{});},{signal:controller.signal});controller.abort();
  await assert.rejects(cancelled,{code:'FETCH_ABORTED'});assert.equal(calls,0);
  assert.equal(expectedCancellation({code:'FETCH_ABORTED'}),true);assert.equal(expectedCancellation({code:'CAPTCHA_TIMEOUT'}),false);
  assert.equal(await captchaAttempt(async()=>token),token);
});

test('captcha uses anonymous CORS POST, string IDs and original image coordinates',async()=>{
  const calls=[];const client=createBurnerCaptchaClient({fetchImpl:async(u,o)=>{calls.push({url:u,...o});return Response.json({code:200,data:calls.length===1?challenge:{captchaToken:token}});}});
  const c=await client.issue(url),point=captchaPoint({clientX:140,clientY:95},{left:10,top:20,width:260,height:150},c);
  assert.deepEqual(point,{x:260,y:150});assert.equal(await client.verify(c,[point,point,point]),token);
  assert.deepEqual(JSON.parse(calls[0].body),{firmwareId:'1897200000000001943',versionId:'1897300000000004110'});
  assert.deepEqual(JSON.parse(calls[1].body),{captchaToken:token,clicks:[point,point,point]});
  for(const call of calls){assert.equal(call.method,'POST');assert.equal(call.mode,'cors');assert.equal(call.credentials,'omit');assert.deepEqual(call.headers,{'Content-Type':'application/json'});}
});
test('untrusted images, malformed metadata and click counts fail closed',async()=>{
  for(const bad of [{captchaImage:'https://other.example/image'},{width:Infinity},{targetCount:100},{expiresInSeconds:0},{captchaToken:'bad'},{instruction:'x'.repeat(501)}]){
    const client=createBurnerCaptchaClient({fetchImpl:async()=>Response.json({code:200,data:{...challenge,...bad}})});await assert.rejects(client.issue(url),{code:'CAPTCHA_RESPONSE'});
  }
  let calls=0;const client=createBurnerCaptchaClient({fetchImpl:async()=>{calls++;return rejected();}});
  await assert.rejects(client.issue('https://evil.example/'),{code:'INVALID_SOURCE'});
  await assert.rejects(client.verify(challenge,[{x:1,y:1}]),{code:'CAPTCHA_INPUT'});
  await assert.rejects(client.verify(challenge,[{x:600,y:1},{x:1,y:1},{x:1,y:1}]),{code:'CAPTCHA_INPUT'});assert.equal(calls,0);
});
test('responses are bounded and provider secrets never enter errors',async()=>{
  const client=createBurnerCaptchaClient({fetchImpl:async()=>Response.json({code:400,msg:token,data:{errorCode:'BURN_CAPTCHA_INVALID'}})});
  await assert.rejects(client.issue(url),e=>e.code==='CAPTCHA_REJECTED'&&!JSON.stringify(e).includes(token)&&!e.message.includes(token));
  let cancelled=false;const large=createBurnerCaptchaClient({fetchImpl:async()=>new Response(new ReadableStream({pull(c){c.enqueue(new Uint8Array(1024*1024));},cancel(){cancelled=true;}}))});
  await assert.rejects(large.issue(url),{code:'CAPTCHA_RESPONSE'});assert.ok(cancelled);
});
test('human verification downloads once with token, then reuses canonical URL + hash cache',async()=>{
  const cache=createFirmwareCache({indexedDB:null}),calls=[];let prompts=0;
  const options={cache,verify:async u=>{assert.equal(u,url);prompts++;return token;},fetchImpl:async(u,o)=>{calls.push(u);assert.equal(o.credentials,'omit');return u===url?rejected():new Response(data);}};
  const source=await prepareBurnerSource({url},options);assert.deepEqual(source,{url,sha256:await sha256(data)});
  assert.deepEqual(calls,[url,url+'?captchaToken='+token]);assert.equal(prompts,1);
  assert.deepEqual((await cache.get(url,source.sha256)).data,data);
  assert.deepEqual(await prepareBurnerSource(source,options),source);assert.equal(calls.length,2);
});
test('wrong hash, second rejection and cancelled UI do not become firmware or leak tokens',async()=>{
  const cache=createFirmwareCache({indexedDB:null});let calls=0;
  const options={cache,verify:async()=>token,fetchImpl:async()=>++calls===1?rejected():new Response(data)};
  await assert.rejects(prepareBurnerSource({url,sha256:'a'.repeat(64)},options),{code:'HASH_MISMATCH'});assert.equal(await cache.get(url,'a'.repeat(64)),null);
  await assert.rejects(prepareBurnerSource({url},{...options,fetchImpl:async()=>rejected()}),e=>e.code==='FIRMWARE_AUTH_REQUIRED'&&!JSON.stringify(e).includes(token));
  calls=0;await assert.rejects(prepareBurnerSource({url},{...options,verify:async()=>{throw Object.assign(new Error('cancel'),{code:'FETCH_ABORTED'});}}),{code:'FETCH_ABORTED'});assert.equal(calls,1);
  await assert.rejects(fetchBytes(url,{fetchImpl:async()=>rejected()}),{code:'FIRMWARE_AUTH_REQUIRED'});
});
test('non-Burner sources pass through; aborted preparation does not request anything',async()=>{
  let calls=0;const opts={fetchImpl:async()=>{calls++;throw Error('unexpected');}};
  const source={url:'https://example.com/fw.bin'};assert.equal(await prepareBurnerSource(source,opts),source);
  const controller=new AbortController();controller.abort();await assert.rejects(prepareBurnerSource({url},{...opts,signal:controller.signal}),{code:'FETCH_ABORTED'});assert.equal(calls,0);
});

test('image points preserve requested order, accept only unique close matches, and never return partial click lists',async()=>{
  const {instructionTargets,matchInstructionTargets,captchaTextMasks}=await import('../web/platform/m5burner-image-points.js');
  for(const instruction of ['请按顺序点击：：UnitV, PaperS3, StickS3','Click in order:： UnitV, PaperS3, StickS3'])assert.deepEqual(instructionTargets(instruction),['UnitV','PaperS3','StickS3']);
  const candidates=[{text:'Cardputer',x:351,y:135,confidence:88},{text:'Dial',x:131,y:218,confidence:94},{text:'Tabb',x:176,y:45,confidence:85}];
  const r=matchInstructionTargets('Click in order: Dial, Cardputer, Tab5',candidates);assert.equal(r.complete,true);assert.deepEqual(r.points,[{x:131,y:218},{x:351,y:135},{x:176,y:45}]);assert.equal(r.matches[2].approximate,true);
  for(const list of [candidates.slice(1),[...candidates,{text:'Dial',x:400,y:200,confidence:95}],candidates.map(c=>({...c,confidence:0}))]){const v=matchInstructionTargets('Click in order: Dial, Cardputer, Tab5',list);assert.equal(v.complete,false);assert.deepEqual(v.points,[]);}
  assert.equal(matchInstructionTargets('Unexpected instruction',candidates).complete,false);
  assert.deepEqual(captchaTextMasks({width:520,height:300,data:new Uint8ClampedArray(520*300*4).fill(255)}),[]);
  assert.throws(()=>captchaTextMasks({width:1e6,height:300,data:[]}));
});

test('a removed version neither prompts for CAPTCHA nor caches JSON or chooses another version',async()=>{
  let fetches=0,prompts=0,writes=0;
  await assert.rejects(prepareBurnerSource({url},{fetchImpl:async u=>{fetches++;assert.equal(u,url);return Response.json({data:{errorCode:'VERSION_NOT_FOUND'}},{status:404});},verify:async()=>{prompts++;},cache:{get:async()=>null,putVerified:async()=>{writes++;}}}),{code:'FIRMWARE_VERSION_NOT_FOUND'});
  assert.deepEqual({fetches,prompts,writes},{fetches:1,prompts:0,writes:0});
});
