import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {encodeUIFlow1,encodeUIFlow2,normalizeFlashConfiguration,inspectFlashConfiguration,applyFlashConfiguration} from '../web/platform/flash-configuration.js';
import {createFlashProfiles} from '../web/platform/flash-profiles.js';
import {SIMULATOR_UIFLOW2_PROFILE} from '../web/platform/simulation-network.js';
import {prepareFlashPlan,prepareFlashSession,prepareFlashConfiguration,chooseFlashPort,activateFlashSession,createFlasher,releaseFlashSession,validateFlashLayout} from '../web/platform/flasher.js';
import {createFlashBatch} from '../web/platform/flash-batch.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const profile={name:'测试配置',type:'uiflow2',server:'uiflow2.m5stack.com',ssid:'Provisioning-Test',password:'test-only-123'};
const catalog={};for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url))).items;
function firmware(chip=9) {
  const bytes=new Uint8Array(0x11000).fill(255),v=new DataView(bytes.buffer);
  for(const at of [chip===0?0x1000:0,0x10000]){bytes.fill(0,at,at+64);bytes[at]=0xe9;bytes[at+1]=1;v.setUint16(at+12,chip,true);v.setUint32(at+28,4,true);}
  for(const [i,type,subtype,at,size,name] of [[0,1,2,0x9000,0x6000,'nvs'],[1,1,1,0xf000,0x1000,'phy_init'],[2,0,0,0x10000,0x1000,'factory']]) {
    const off=0x8000+i*32;bytes.fill(0,off,off+32);v.setUint16(off,0x50aa,true);bytes[off+2]=type;bytes[off+3]=subtype;v.setUint32(off+4,at,true);v.setUint32(off+8,size,true);bytes.set(new TextEncoder().encode(name),off+12);
  }
  bytes.fill(0x5a,0x9000,0xf000);return bytes;
}
function input(classic=false) {const scene=createDefaultScene(catalog,classic?'m5stack-c008':'m5stack-stopwatch');return {scene,catalog,source:{file:new File([firmware(classic?0:9)],'protocol-fixture.bin')}};}
function readNvs(bytes) {
  const out={},v=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),decoder=new TextDecoder();
  for(let slot=0;slot<126;) {
    if(((bytes[32+Math.floor(slot/4)]>>((slot%4)*2))&3)!==2){slot++;continue;}
    const off=64+slot*32,span=bytes[off+2],key=decoder.decode(bytes.subarray(off+8,off+24)).replace(/\0.*$/s,'');
    if(bytes[off]===1)out[key]=bytes[off+1]===0x21?decoder.decode(bytes.subarray(off+32,off+32+v.getUint16(off+24,true)-1)):bytes[off+24];slot+=span;
  }
  return out;
}
test('UIFlow2 bytes equal the independent official Python NVS generator fixture',async()=>{
  assert.deepEqual(Buffer.from(encodeUIFlow2(profile)),await readFile(new URL('./fixtures/flash-configuration/uiflow2-nvs.bin',import.meta.url)));
  const unicode={...profile,server:'flow.example',ssid:'产线,WiFi',password:'测试密码123456'};
  const values=readNvs(encodeUIFlow2(unicode));assert.equal(values.server,unicode.server);assert.equal(values.ssid0,unicode.ssid);assert.equal(values.pswd0,unicode.password);assert.equal(values.boot_option,1);
});
test('simulator profile uses real UIFlow2 Wi-Fi keys and explicit network-only boot',()=>{
  const values=readNvs(encodeUIFlow2(SIMULATOR_UIFLOW2_PROFILE));
  assert.equal(values.ssid0,'esp32_online_tester');assert.equal(values.pswd0,'rilakkuma_cool');assert.equal(values.boot_option,2);
  assert.equal(values.net_mode,'WIFI');assert.equal(values.protocol,'DHCP');
  assert.equal(readNvs(encodeUIFlow2(profile)).boot_option,1,'Existing menu profiles retain their original default');
  for(const bootOption of [0,3,'2',null,NaN])assert.throws(()=>encodeUIFlow2({...profile,bootOption}),{code:'FLASH_CONFIG_INVALID'});
});
test('UIFlow1 JSON length/checksum, UTF-8 Wi-Fi blocks and per-device identity',()=>{
  const p={...profile,type:'uiflow1',server:'flow-cn.m5stack.com',ssid:'产线,测试'};
  const [config,wifi]=encodeUIFlow1(p,{mac:'02:00:00:00:00:01',apiKey:'1234ABCD'}),length=(config[0]<<8)|config[1];
  assert.equal(config[length+2],config.subarray(0,length+2).reduce((a,b)=>a+b,0)&255);
  const json=JSON.parse(new TextDecoder().decode(config.subarray(2,length+2)));
  assert.deepEqual(json.wifi,[p.ssid,p.password]);assert.equal(json.node_id,'020000000001');assert.equal(json.apikey,'1234ABCD');assert.equal(json.server,p.server);
  assert.equal(new TextDecoder().decode(wifi.subarray(1,1+wifi[0])),p.ssid);assert.equal(wifi[1+wifi[0]],wifi.subarray(1,1+wifi[0]).reduce((a,b)=>a+b,0)&255);
  assert.equal(new TextDecoder().decode(wifi.subarray(51,51+wifi[50])),p.password);assert.ok(wifi.subarray(100).every(v=>v===255));
  assert.notDeepEqual(config,encodeUIFlow1(p,{mac:'02:00:00:00:00:02',apiKey:'5678ABCD'})[0]);
});
test('optional configuration preserves original source; exact partition overlay is bounded',async()=>{
  const original=await prepareFlashPlan(input()),copy=structuredClone(original);assert.equal(applyFlashConfiguration(copy,null),copy);assert.deepEqual(copy,original);
  assert.deepEqual(inspectFlashConfiguration(original),{uiflow1:true,uiflow2:true});
  applyFlashConfiguration(copy,profile);validateFlashLayout(copy.files,copy.product,copy.receipt);
  assert.deepEqual(copy.files.map(f=>f.address),[0,0x9000,0xf000]);
  assert.deepEqual(copy.files[0].data,original.files[0].data.subarray(0,0x9000));assert.deepEqual(copy.files[2].data,original.files[0].data.subarray(0xf000));
  assert.ok(original.files[0].data.subarray(0x9000,0xf000).every(v=>v===0x5a));assert.deepEqual(copy.receipt.artifacts,original.receipt.artifacts);
  assert.equal(JSON.stringify(copy.receipt).includes(profile.password),false);
  const legacy=structuredClone(original);applyFlashConfiguration(legacy,{...profile,type:'uiflow1'},{mac:'02:00:00:00:00:01',apiKey:'00000001'});
  assert.deepEqual(legacy.files.map(f=>f.address),[0,0x11000,0x12000]);validateFlashLayout(legacy.files,legacy.product,legacy.receipt);
  for(const [offset,value] of [[0x8004,0xa000],[0x8008,0x5000],[0x801c,1],[0x8024,0xa000]]) {
    const bad=structuredClone(original);new DataView(bad.files[0].data.buffer).setUint32(offset,value,true);
    const before=structuredClone(bad);assert.throws(()=>applyFlashConfiguration(bad,profile),{code:'FLASH_CONFIG_INVALID'});assert.deepEqual(bad,before);
  }
  const full=structuredClone(original);full.receipt.flashBytes=0x12000;assert.throws(()=>applyFlashConfiguration(full,{...profile,type:'uiflow1'}),{code:'FLASH_CONFIG_INVALID'});
  for(const subtype of [0,1,2,4,0x81,0x82]) {
    const occupied=structuredClone(original),b=occupied.files[0].data,v=new DataView(b.buffer),off=0x8060;
    b.fill(0,off,off+32);v.setUint16(off,0x50aa,true);b[off+2]=1;b[off+3]=subtype;v.setUint32(off+4,0x11000,true);v.setUint32(off+8,0x2000,true);
    assert.throws(()=>applyFlashConfiguration(occupied,{...profile,type:'uiflow1'}),{code:'FLASH_CONFIG_INVALID'},'Tail configuration must never overwrite a declared data partition');
  }
});
test('input validation never truncates credentials, supports custom host and rejects injection',()=>{
  for(const change of [{type:'unknown'},{ssid:''},{ssid:'中'.repeat(11)},{password:'short'},{password:'中'.repeat(21)+'x'},{server:'https://flow.example/path'},{server:'x:99999'},{password:'a\0'.repeat(8)},{type:'uiflow1',password:'x'.repeat(49)}])assert.throws(()=>normalizeFlashConfiguration({...profile,...change}),{code:'FLASH_CONFIG_INVALID'});
  assert.equal(normalizeFlashConfiguration({...profile,password:'',server:'192.0.2.1'}).server,'192.0.2.1');
});
test('profiles stay in page memory unless explicitly saved, and persistence/deletion are atomic',()=>{
  const data=new Map(),storage={getItem:key=>data.get(key),setItem:(key,value)=>data.set(key,value)};
  const store=createFlashProfiles({storage}),p=store.put(profile);assert.equal(createFlashProfiles({storage}).list().length,0);assert.equal(JSON.stringify([...data.values()]).includes(profile.password),false);
  store.put({...profile,server:'custom.example'},{id:p.id,persistent:true});assert.equal(createFlashProfiles({storage}).list()[0].server,'custom.example');
  store.put(profile,{id:p.id});assert.equal(createFlashProfiles({storage}).list().length,0);store.remove(p.id);assert.equal(store.list().length,0);
  const unavailable=createFlashProfiles();assert.throws(()=>unavailable.put(profile,{persistent:true}));assert.equal(unavailable.list().length,0);
});
test('built-in virtual-network profile cannot overwrite private profiles or persist credentials implicitly',()=>{
  const rows=new Map(),storage={getItem:k=>rows.get(k),setItem:(k,v)=>rows.set(k,v)},builtins=[SIMULATOR_UIFLOW2_PROFILE];
  const store=createFlashProfiles({storage,builtins}),fixed=store.list()[0];assert.equal(fixed.builtin,true);assert.equal(rows.size,0);
  fixed.password='mutated-local-copy';assert.equal(store.list()[0].password,SIMULATOR_UIFLOW2_PROFILE.password);
  assert.throws(()=>store.put(profile,{id:fixed.id}));assert.throws(()=>store.remove(fixed.id));
  const copy=store.put({...fixed,name:'Private edited profile'},{persistent:true});assert.notEqual(copy.id,fixed.id);
  const reopened=createFlashProfiles({storage,builtins}).list();assert.equal(reopened.length,2);assert.equal(reopened[0].password,SIMULATOR_UIFLOW2_PROFILE.password);assert.equal(reopened[1].password,'mutated-local-copy');
  assert.equal(JSON.stringify([...rows.values()]).includes(SIMULATOR_UIFLOW2_PROFILE.password),false);
});

test('classic ESP32 + UIFlow1 checks eFuses before writing; opaque profiles, unique identity, retry stability and no secrets in state',async()=>{
  const selected=[],events=[],writes=[],reads=[],nav={userActivation:{isActive:true},serial:{requestPort:async()=>selected.shift()}};
  class Transport {constructor(port){this.port=port;}async disconnect(){}}
  class Loader {
    constructor({transport}){this.port=transport.port;this.chip={CHIP_NAME:'ESP32',readMac:async()=>this.port.mac};}
    async main(){}async readReg(at){reads.push(at);return this.port.secured?16:0;}async detectFlashSize(){return '4MB';}flashSizeBytes(){return 4*1024*1024;}
    async writeFlash(options){assert.ok(reads.includes(0x3ff5a000)&&reads.includes(0x3ff5a018));writes.push(structuredClone(options.fileArray));options.reportProgress(0,1,1);}async after(){}
  }
  const deps={navigator:nav,secureContext:true,loadWatchlistData:async()=>({entries:[]}),loadTools:async()=>({Transport,ESPLoader:Loader,md5:()=>''}),onEvent:event=>events.push(event)};
  const session=await prepareFlashSession(input(true),deps),p={...profile,type:'uiflow1'},token=prepareFlashConfiguration(session,p);p.password='changed-secret';activateFlashSession(session,true,deps);
  const ports=[{mac:'02:00:00:00:00:01'},{mac:'02:00:00:00:00:02'}];
  for(const port of ports){selected.push(port);await chooseFlashPort(session,deps);await createFlasher(deps).flash({prepared:session,port,configuration:token});}
  await createFlasher(deps).flash({prepared:session,port:ports[0],configuration:token});
  assert.deepEqual(writes[0],writes[2]);assert.notDeepEqual(writes[0][1].data,writes[1][1].data);
  ports[0].mac='02:00:00:00:00:04';await createFlasher(deps).flash({prepared:session,port:ports[0],configuration:token});
  assert.notDeepEqual(writes[0][1].data,writes[3][1].data,'A different device reusing a port must not inherit the old node identity');
  const reconnected={mac:'02:00:00:00:00:01'};selected.push(reconnected);await chooseFlashPort(session,deps);await createFlasher(deps).flash({prepared:session,port:reconnected,configuration:token});assert.deepEqual(writes[0],writes[4],'Same device on a new port handle retains its batch identity');
  assert.ok(new TextDecoder().decode(writes[0][1].data).includes(profile.password));
  await assert.rejects(()=>createFlasher(deps).flash({prepared:session,port:ports[0],configuration:{...token}}),{code:'FLASH_CONFIG_INVALID'});
  const secured={mac:'02:00:00:00:00:03',secured:true};selected.push(secured);await chooseFlashPort(session,deps);
  await assert.rejects(()=>createFlasher(deps).flash({prepared:session,port:secured,configuration:token}),{code:'SECURED_DEVICE'});assert.equal(writes.length,5);
  for(const secret of [profile.password,profile.ssid,...ports.map(p=>p.mac)])assert.equal(JSON.stringify(events).includes(secret),false);
  releaseFlashSession(session);
});

test('continuous batches freeze each chosen profile; editing after completion affects only next device',async()=>{
  const nav={userActivation:{isActive:true},serial:new EventTarget()},selected=[],calls=[];nav.serial.requestPort=async()=>selected.shift();
  const batch=createFlashBatch({navigator:nav,secureContext:true,loadWatchlistData:async()=>({entries:[]}),createDeviceFlasher:()=>({cancel(){},async flash(args){calls.push(args);return {message:'done'};}})});
  await batch.prepare(input());batch.setConfiguration(profile);batch.start({mode:'sequential'});
  selected.push({});await batch.addDevice();await new Promise(r=>setTimeout(r,0));
  batch.setConfiguration({...profile,name:'下一批',server:'custom.example'});selected.push({});await batch.addDevice();await new Promise(r=>setTimeout(r,0));
  assert.equal(calls[0].configuration.name,profile.name);assert.equal(calls[1].configuration.name,'下一批');assert.equal(JSON.stringify(batch.observe()).includes(profile.password),false);
  assert.notEqual(calls[0].configuration,calls[1].configuration);await batch.dispose();
});
