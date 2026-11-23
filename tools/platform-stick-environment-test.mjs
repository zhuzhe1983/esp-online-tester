import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {createDefaultScene,decodeScene,encodeScene,validateScene} from '../web/platform/catalog.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {installAutomation} from '../web/platform/automation.js';
import {installEmbedBridge} from '../web/platform/embed.js';
import {decodeRecording,encodeRecording,validateRecording} from '../web/platform/recording.js';
import {chipForBoard,chipIdForBoard} from '../web/platform/app-image.js';
import {POWER_DEFAULT,powerMessage} from '../web/platform/power-environment.js';
import {DEFAULT_THERMAL} from '../web/platform/thermal-input.js';
import {appendPowerEnvironmentControls} from '../web/platform/power-environment-controls.js';

const power = {...POWER_DEFAULT,batteryVoltageUv:0};
const heat = {...DEFAULT_THERMAL,centerX:40,centerY:30};
const plusId = 'm5stack-k016-p', t2Id = 'm5stack-k016-t2', cId = 'm5stack-k016-c', s3Id = 'm5stack-k150';
const baseUrl = 'https://fixture.invalid/platform/index.html';
const products = [[plusId,'stickc-plus'],[t2Id,'stickt2'],[cId,'stickc'],[s3Id,'sticks3']].map(([id,board])=>({
  id,chip:chipForBoard(board),runtime:{board,status:'partial',flashMb:4,psramMb:0,touch:false},
  display:{width:135,height:240},controls:[{type:'button',id:'power',transport:'board'}],
}));
const catalog = {products,accessories:[],firmwares:[]};

// Structural ROM image and worker fixtures exercise the transport and validators.
function mergedFixture() {
  const bytes = new Uint8Array(0x11000).fill(255), view = new DataView(bytes.buffer);
  for (const offset of [0x1000,0x10000]) {
    bytes.fill(0,offset,offset+64);
    bytes[offset] = 0xe9;
    bytes[offset+1] = 1;
    view.setUint32(offset+28,4,true);
  }
  bytes.fill(0,0x8000,0x8020);
  view.setUint16(0x8000,0x50aa,true);
  view.setUint32(0x8004,0x10000,true);
  view.setUint32(0x8008,0x1000,true);
  return bytes;
}

function runtimeFixture() {
  const messages = [];
  const bytes = mergedFixture();
  const source = {url:'https://fixture.invalid/flash.bin',sha256:createHash('sha256').update(bytes).digest('hex')};
  const workerFactory = () => {
    let timer, clock = 0;
    const worker = {
      terminate() { clearInterval(timer); },
      postMessage(message) {
        messages.push(message);
        const send = data => worker.onmessage({data});
        let reply;
        switch (message.op) {
          case 'init': reply = {ready:true}; break;
          case 'create': reply = {created:true,flashStorage:false,audioAvailable:true}; break;
          case 'load': reply = {loaded:message.at === undefined ? message.kind : `at${message.at}`,ok:true}; break;
          case 'imu': reply = {imuApplied:message.inputId,ok:true,time:clock}; break;
          case 'text': reply = {inputQueued:message.inputId,time:clock}; break;
          case 'stop': clearInterval(timer); reply = {stopped:clock}; break;
          case 'start':
          case 'resume':
            clearInterval(timer);
            timer = setInterval(()=>send({text:JSON.stringify({t:'stat',time:clock+=0.02})}),2);
            reply = message.op === 'start' ? {started:true} : {resumed:true};
            break;
          default: assert.fail(`Unexpected fixture worker command ${message.op}`);
        }
        queueMicrotask(()=>send(reply));
      },
    };
    return worker;
  };
  const runtime = createPlatformRuntime({
    catalog,workerFactory,baseUrl,assetBaseUrl:'https://fixture.invalid/',requestTimeout:1000,
    fetchImpl:async url=>new Response(String(url) === source.url ? bytes : new Uint8Array([1,2,3])),
  });
  const api = installAutomation(runtime,{target:{location:{href:baseUrl}}});
  return {
    api,messages,
    scene: (productId, extra={})=>({...createDefaultScene(catalog,productId),firmware:source,...extra}),
    wire: type=>messages.filter(message=>message.op==='text').map(message=>JSON.parse(message.data)).filter(message=>message.t===type),
  };
}

test('Stick images retain distinct classic ESP32 and S3 boot layouts',()=>{
  for (const board of ['stickc','stickc-plus','m5stack-k016-p','stickt2','core2-v13']) assert.equal(chipIdForBoard(board),0);
  assert.equal(chipIdForBoard('sticks3'),9);
  assert.equal(chipIdForBoard('seeed-xiao-epaper-75'),5);
});

test('scene fields round-trip and reject invalid types, extra keys and foreign products',()=>{
  const plus = {...createDefaultScene(catalog,plusId),environment:{power}};
  const t2 = {...createDefaultScene(catalog,t2Id),thermalEnvironment:heat};
  for (const scene of [plus,t2]) {
    assert.equal(validateScene(scene,catalog).valid,true);
    assert.deepEqual(decodeScene(encodeScene(scene)),scene);
    assert.equal(validateScene({...scene,productId:cId},catalog).valid,false);
  }
  for (const fields of [{vbusVoltageUv:7000001},{batteryVoltageUv:-1},{batteryVoltageUv:2.5},{charging:1},{extra:true}]) {
    assert.equal(validateScene({...plus,environment:{power:{...power,...fields}}},catalog).valid,false);
  }
  for (const fields of [{background:0},{hotspot:16384},{centerX:160},{centerY:-1},{radius:161},{celsius:27}]) {
    assert.equal(validateScene({...t2,thermalEnvironment:{...heat,...fields}},catalog).valid,false);
  }
});

test('Plus queues initial power, sends exact voltage/flags and resets to the scene',async()=>{
  const f = runtimeFixture();
  try {
    await f.api.configure(f.scene(plusId,{environment:{power}}));
    assert.equal((await f.api.capabilities()).environment.power.available,true);
    await f.api.loadFirmware();
    assert.deepEqual(f.wire('power-environment'),[powerMessage(power)]);
    assert.equal(f.messages.find(message=>message.op==='create').audioStreaming,true);
    const audio = (await f.api.capabilities()).audio;
    assert.equal(audio.input.available,false);
    assert.equal(audio.output.source,'guest-routed-ledc-g2');
    assert.equal(audio.output.sampleRate,16000);
    await assert.rejects(f.api.audioConfigure({input:{enabled:true,port:0,sampleRate:44100}}),{code:'AUDIO_UNSUPPORTED'});
    await assert.rejects(f.api.pushAudio({samples:[0],sampleRate:44100,port:0}),{code:'AUDIO_UNSUPPORTED'});
    assert.equal((await f.api.audioConfigure({output:{enabled:false}})).output.enabled,false);
    const update = {...power,batteryVoltageUv:3000000,vbusVoltageUv:5000000,vbusPresent:true,vbusValid:true,charging:true};
    await assert.rejects(f.api.input({type:'power-environment',...update}),{code:'NOT_RUNNING'});
    await f.api.start();
    await assert.rejects(f.api.input({type:'power-environment',...update,charging:1}),{code:'INVALID_INPUT'});
    assert.equal((await f.api.input({type:'power-environment',...update})).delivery,'queued');
    assert.deepEqual(f.wire('power-environment').at(-1),powerMessage(update));
    const state = await f.api.observe();
    assert.equal(state.powerEnvironment.virtual,true);
    assert.equal(state.powerEnvironment.batteryVoltageUv,3000000);
    assert.equal(state.scene.environment.power.batteryVoltageUv,0);
    const shared = new URL(await f.api.share()), params = new URLSearchParams(shared.hash.slice(1));
    assert.deepEqual(decodeScene(params.get('scene')).environment.power,power);
    await f.api.pause();
    await f.api.reset();
    assert.deepEqual(f.wire('power-environment').at(-1),powerMessage(power));
    await f.api.configure(f.scene(cId));
    assert.equal((await f.api.observe()).powerEnvironment,null);
    assert.equal((await f.api.capabilities()).audio.input.available,true);
    await f.api.loadFirmware();
    await f.api.start();
    await assert.rejects(f.api.input({type:'power-environment',...update}),{code:'POWER_UNSUPPORTED'});
  } finally { await f.api.dispose(); }
});

for (const sample of [
  {productId:plusId,type:'power-environment',sceneField:'environment',initial:{power},values:{...power,batteryVoltageUv:5000000}},
  {productId:t2Id,type:'thermal-environment',sceneField:'thermalEnvironment',initial:heat,values:{...heat,centerX:0,radius:0}},
]) {
  test(`${sample.type} share and replay preserve exact guest-acknowledged stimuli`,async()=>{
    const f = runtimeFixture();
    const scene = f.scene(sample.productId,{[sample.sceneField]:sample.initial});
    try {
      await f.api.configure(scene);
      await f.api.loadFirmware();
      await f.api.recordingStart();
      await f.api.input({type:sample.type,...sample.values});
      const recording = await f.api.recordingStop();
      assert.deepEqual(recording.scene[sample.sceneField],sample.initial);
      assert.equal(recording.events.length,1);
      const {at,...recorded} = recording.events[0];
      assert.ok(Number.isFinite(at));
      assert.deepEqual(recorded,{type:sample.type,...sample.values});
      assert.deepEqual(decodeRecording(encodeRecording(recording)),recording);
      const invalid = structuredClone(recording);
      invalid.events[0].unmodeled = true;
      assert.equal(validateRecording(invalid,{scene}).valid,false);
      const changed = {...scene,[sample.sceneField]:sample.type==='power-environment' ? {power:sample.values} : sample.values};
      assert.equal(validateRecording(recording,{scene:changed}).valid,false);
      const share = async includeRecording=>new URLSearchParams(new URL(await f.api.share({mode:'embed',includeRecording})).hash.slice(1));
      const withReplay = await share(true);
      assert.deepEqual(decodeScene(withReplay.get('scene'))[sample.sceneField],sample.initial);
      assert.deepEqual(decodeRecording(withReplay.get('recording')),recording);
      const withoutReplay = await share(false);
      assert.equal(withoutReplay.has('recording'),false);
      assert.deepEqual(decodeScene(withoutReplay.get('scene'))[sample.sceneField],sample.type==='power-environment' ? sample.initial : sample.values);
      const result = await f.api.recordingPlay(recording,{timeoutMs:2000});
      assert.equal(result.result,'completed');
      assert.equal(result.bitExact,false);
      const initial = sample.type === 'power-environment' ? sample.initial.power : sample.initial;
      assert.deepEqual(f.wire(sample.type).slice(-2),[{t:sample.type,...initial},{t:sample.type,...sample.values}]);
    } finally { await f.api.dispose(); }
  });
}

test('T2 mini RPC shares the bounded Raw14 input and acknowledged observation',async()=>{
  const f = runtimeFixture(), listeners = new Map(), replies = [];
  const parent = {postMessage:(message,origin)=>replies.push({message,origin})};
  const target = {parent,addEventListener:(type,fn)=>listeners.set(type,fn),removeEventListener:type=>listeners.delete(type)};
  const stop = installEmbedBridge(f.api,{target});
  const call = async (type, method, args=[]) => {
    await listeners.get('message')({
      source:parent,origin:'https://iframe.invalid',
      data:{channel:'esp-workbench',version:'1',session:'t2-test',requestId:String(replies.length),type,method,args},
    });
    return replies.at(-1).message;
  };
  try {
    await f.api.configure(f.scene(t2Id));
    await f.api.loadFirmware();
    await f.api.start();
    assert.equal((await call('hello')).result.capabilities.thermal.sceneUnits,'raw14-counts');
    assert.equal((await call('call','input',[{type:'thermal-environment',...heat,hotspot:65535}])).error.code,'INVALID_INPUT');
    assert.deepEqual(f.wire('thermal-environment'),[{t:'thermal-environment',...DEFAULT_THERMAL}]);
    assert.equal((await call('call','input',[{type:'thermal-environment',...heat}])).result.delivery,'queued');
    assert.deepEqual((await call('call','observe')).result.thermalEnvironment,heat);
    assert.ok(replies.every(reply=>reply.origin==='https://iframe.invalid'));
  } finally { stop(); await f.api.dispose(); }
});

test('power form preserves zero and converts virtual volts to integer microvolts',()=>{
  class Node {
    constructor() { this.children=[]; this.handlers={}; }
    append(...children) { this.children.push(...children); }
    setAttribute() {}
    addEventListener(type, fn) { this.handlers[type]=fn; }
  }
  const document = {createElement:()=>new Node()}, container = new Node();
  const saved = [], sent = [], product = products[0];
  const mount = (status,state)=>appendPowerEnvironmentControls(container,{
    product,scene:{environment:{power}},state,status,document,
    onInitial:values=>saved.push(values),onInput:values=>sent.push(values),
  });
  const initial = mount('configured');
  assert.equal(initial.inputs.batteryVoltageUv.value,'0');
  initial.apply.handlers.click();
  assert.deepEqual(saved,[power]);
  const live = mount('running',power);
  live.inputs.batteryVoltageUv.value = '4.5045';
  live.inputs.vbusPresent.checked = true;
  live.apply.handlers.click();
  assert.deepEqual(sent,[{type:'power-environment',...power,batteryVoltageUv:4504500,vbusPresent:true}]);
  live.inputs.batteryVoltageUv.value = '-0.0000001';
  assert.ok(Number.isNaN(live.read().batteryVoltageUv));
  const loading = mount('loading');
  assert.equal(loading.apply.disabled,true);
  loading.apply.handlers.click();
  assert.equal(saved.length,1);
});
