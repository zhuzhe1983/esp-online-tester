import assert from 'node:assert/strict';
import {test} from 'node:test';
import {createNetworkSession,validateNetworkOptions,networkService} from '../web/platform/network-session.js';
class Socket {
  static instances=[];
  constructor(url){this.url=url;this.readyState=0;this.bufferedAmount=0;this.sent=[];Socket.instances.push(this);queueMicrotask(()=>{this.readyState=1;this.deliver({type:'hello',version:1});});}
  deliver(value){this.onmessage?.({data:typeof value==='object'&&!(value instanceof ArrayBuffer)?JSON.stringify(value):value});}
  send(data){this.sent.push(data);if(typeof data==='string'){const m=JSON.parse(data);let result={};if(m.op==='map')result={mappingId:'map1',host:'127.0.0.1',hostPort:34567,url:'http://127.0.0.1:34567/'};if(m.op==='http')result={status:200,body:['/submit','/exit'].includes(m.path)?' {"success":true} ':m.path==='/done.html'?"<script>fetch('/exit',{method:'POST'})</script>":'<script>fetch("/scan");fetch("/submit"); const ssid=1,password=2; location="/done.html";</script>'};queueMicrotask(()=>this.deliver({id:m.id,result}));}}
  close(){this.readyState=3;this.onclose?.();}
}
const tick=()=>new Promise(resolve=>setTimeout(resolve,0));
test('choosing a self-hosted helper updates idle status without opening a network connection',async()=>{
  const n=createNetworkSession({emit(){},command:async()=>{},sendFrame(){},WebSocketImpl:class {constructor(){throw Error('Firmware is not attached');}}});
  assert.equal(n.snapshot().connection,'relay-unconfigured');
  await n.configure({mode:'local-helper'});assert.equal(n.snapshot().connection,'waiting-helper');
  await n.configure({endpoint:'ws://127.0.0.1:8787/network?token=private'});assert.equal(n.snapshot().connection,'disconnected');assert.equal(n.snapshot().helperConfigured,true);
  await n.configure({mode:'public-relay'});assert.equal(n.snapshot().connection,'relay-unconfigured');
  await n.configure({relayUrl:'https://relay.example.test'});assert.equal(n.snapshot().connection,'disconnected');assert.equal(n.snapshot().relayConfigured,true);
  await n.configure({relayUrl:''});assert.equal(n.snapshot().connection,'relay-unconfigured');n.detach();
});
test('network options reject unsafe protocols and secrets stay out of observation',async()=>{
  assert.throws(()=>validateNetworkOptions({endpoint:'ws://example.com/network'}),{code:'INVALID_NETWORK'});
  assert.throws(()=>validateNetworkOptions({ssid:'name,psk=unexpected'}),{code:'INVALID_NETWORK'});
  assert.throws(()=>networkService({guestPort:0}),{code:'INVALID_NETWORK_SERVICE'});
  const n=createNetworkSession({emit(){},command:async()=>{},sendFrame(){},WebSocketImpl:Socket});
  await n.configure({endpoint:'ws://127.0.0.1:8787/network?token=private-token',password:'private-password'});
  const json=JSON.stringify(n.snapshot());assert.ok(!json.includes('private-token'));assert.ok(!json.includes('private-password'));n.detach();
});
test('real transport direction, explicit SoftAP association, mappings and teardown',async()=>{
  const commands=[],frames=[],n=createNetworkSession({emit(){},command:async(...x)=>commands.push(x),sendFrame:(...x)=>frames.push(x),WebSocketImpl:Socket});
  await n.configure({endpoint:'ws://127.0.0.1:8787/network?token=test',autoProvision:false});
  await n.attach({available:true});const ws=Socket.instances.at(-1);
  assert.equal(n.snapshot().connection,'connected');assert.ok(commands.some(([op,v])=>op==='relay'&&v.enabled));
  const frame=new Uint8Array(14);n.receive({networkFrame:{interface:0,data:frame.buffer}});assert.equal(ws.sent.at(-1).length,15);
  ws.deliver(new Uint8Array([1,...frame]).buffer);assert.equal(frames[0][0],1);assert.equal(frames[0][1].byteLength,14);
  n.receive({softAp:{state:'connected',clientMac:'02:53:49:4d:00:03',ssid:'test-ap'}});await tick();
  assert.ok(ws.sent.some(x=>typeof x==='string'&&JSON.parse(x).op==='interface'));
  ws.deliver({type:'network-status',interface:1,leaseReady:true,guestAddress:'192.168.4.1'});
  assert.equal(n.snapshot().services[0].source,'softap-candidate');assert.equal(n.snapshot().provisioning,'idle');
  const mapping=await n.mapPort({interface:1,protocol:'http',guestPort:80});assert.equal(mapping.hostPort,34567);
  await n.unmap({mappingId:mapping.mappingId});assert.equal(n.snapshot().services[0].mapping,undefined);
  n.detach();assert.equal(ws.readyState,3);assert.equal(n.snapshot().services.length,0);
});
test('automatic provisioning uses original HTTP service and can be disabled independently',async()=>{
  const n=createNetworkSession({emit(){},command:async()=>{},sendFrame(){},WebSocketImpl:Socket});
  await n.configure({endpoint:'ws://127.0.0.1:8787/network?token=test'});await n.attach({available:true,manifestWifi:'ssid=open-test'});
  const ws=Socket.instances.at(-1);ws.deliver({type:'network-status',interface:1,leaseReady:true});await tick();await tick();
  const submit=ws.sent.filter(x=>typeof x==='string').map(x=>JSON.parse(x)).find(x=>x.path==='/submit');
  assert.deepEqual(JSON.parse(submit.body),{ssid:'open-test',password:''});assert.equal(n.snapshot().provisioning,'submitted');assert.ok(ws.sent.some(x=>typeof x==='string'&&JSON.parse(x).path==='/exit'));assert.equal(n.snapshot().services[0].verified,true);
  await n.configure({autoProvision:false});assert.equal(n.snapshot().connection,'connected');assert.equal(ws.readyState,1);
  ws.deliver({type:'network-status',interface:0,leaseReady:true,guestAddress:'10.0.2.15'});assert.equal(n.snapshot().provisioning,'connected');n.detach();
});
test('mapping deduplication preserves handles and rejects competing local ports',async()=>{
  const n=createNetworkSession({emit(){},command:async()=>{},sendFrame(){},WebSocketImpl:Socket});
  await n.configure({endpoint:'ws://localhost/network'});await n.attach({available:true});const ws=Socket.instances.at(-1);
  const options={interface:0,protocol:'tcp',guestPort:1883};
  const first=n.mapPort({...options,hostPort:12345});
  await assert.rejects(n.mapPort({...options,hostPort:12346}),{code:'NETWORK_ALREADY_MAPPED'});
  const a=await first,b=await n.mapPort(options);assert.deepEqual(a,b);
  assert.equal(ws.sent.filter(x=>typeof x==='string'&&JSON.parse(x).op==='map').length,1);
  n.addService({...options,label:'MQTT'});assert.equal(n.snapshot().services[0].mapping.mappingId,a.mappingId);n.detach();
});
test('disabling automatic provisioning cancels the pending credential submission',async()=>{
  class Delayed extends Socket {send(data){if(typeof data==='string'&&JSON.parse(data).path==='/'){this.sent.push(data);this.get=JSON.parse(data);return;}super.send(data);}}
  const n=createNetworkSession({emit(){},command:async()=>{},sendFrame(){},WebSocketImpl:Delayed});
  await n.configure({endpoint:'ws://localhost/network'});await n.attach({available:true});let ws=Socket.instances.at(-1);
  ws.deliver({type:'network-status',interface:1,leaseReady:true});await n.configure({autoProvision:false});
  ws.deliver({id:ws.get.id,result:{status:200,body:'/scan /submit ssid password'}});await tick();
  assert.equal(ws.sent.filter(x=>typeof x==='string'&&JSON.parse(x).path==='/submit').length,0);assert.equal(n.snapshot().provisioning,'idle');
  await n.configure({autoProvision:true});await n.connect();ws=Socket.instances.at(-1);
  ws.deliver({type:'network-status',interface:1,leaseReady:true});assert.ok(ws.get);n.detach();await tick();
});

test('browser SoftAP works without any external connection and survives disabling Internet',async()=>{
  const frames=[],commands=[];let client,closed=false;
  const n=createNetworkSession({emit(){},command:async(...x)=>commands.push(x),sendFrame:(...x)=>frames.push(x),WebSocketImpl:class {constructor(){throw Error('Must not open a socket');}},guestFactory:({emit,sendFrame})=>(client={enable(){emit({type:'network-status',interface:1,leaseReady:true,guestAddress:'10.23.4.1'});sendFrame(new Uint8Array(14).buffer);},receive(data){this.frame=data;},requestHttp:async options=>({status:200,body:options.path}),close(){closed=true;sendFrame(new Uint8Array(14).buffer);}})});
  await n.attach({available:true});n.receive({softAp:{state:'connected',clientMac:'02:53:49:4d:00:03'}});
  assert.equal(n.snapshot().connection,'relay-unconfigured');assert.equal(n.snapshot().interfaces[1].guestAddress,'10.23.4.1');
  assert.equal((await n.http({path:'/scan'})).body,'/scan');assert.equal(n.snapshot().services[0].verified,true);assert.equal(frames[0][0],1);
  n.receive({networkFrame:{interface:1,data:new Uint8Array([1,2]).buffer}});assert.deepEqual([...client.frame],[1,2]);
  const mapping=await n.mapPort({interface:1,protocol:'http',guestPort:80});assert.equal(mapping.browser,true);assert.equal(mapping.guestAddress,'10.23.4.1');
  await n.configure({enabled:false});assert.equal(closed,false);assert.equal(n.snapshot().interfaces[1].leaseReady,true);assert.equal((await n.http({path:'/'})).status,200);
  await assert.rejects(n.mapPort({interface:0,protocol:'tcp',guestPort:1883}),{code:'LOCAL_HELPER_REQUIRED'});
  n.detach();assert.equal(closed,true);assert.equal(frames.length,1,'closed guest must not send stale teardown frames');assert.equal(n.snapshot().services.length,0);
});

test('public relay uses ephemeral same-origin bootstrap, only STA packets, and keeps tokens private',async()=>{
  const requests=[],frames=[],commands=[];
  class PublicSocket extends Socket {constructor(url){super(url);}deliver(value){if(value.type==='hello')value={...value,capabilities:{publicRelay:true}};super.deliver(value);}}
  const n=createNetworkSession({emit(){},command:async(...x)=>commands.push(x),sendFrame:(...x)=>frames.push(x),WebSocketImpl:PublicSocket,fetchImpl:async(url,options)=>{requests.push({url,options});return {ok:true,text:async()=>JSON.stringify({version:1,endpoint:'wss://relay.example.test/network?token=secret'})};}});
  await n.configure({relayUrl:'https://relay.example.test',autoProvision:false});await n.attach({available:true});
  assert.equal(requests[0].url,'https://relay.example.test/session');assert.equal(requests[0].options.method,'POST');assert.equal(requests[0].options.credentials,'omit');
  const ws=Socket.instances.at(-1);assert.equal(ws.url,'wss://relay.example.test/network?token=secret');assert.equal(n.snapshot().connection,'connected');assert.ok(!JSON.stringify(n.snapshot()).includes('secret'));
  ws.deliver(new Uint8Array(15).buffer);assert.equal(frames.length,1);ws.deliver(new Uint8Array([1,...new Uint8Array(14)]).buffer);assert.equal(frames.length,1);
  ws.deliver({type:'network-status',interface:1,leaseReady:true});assert.equal(n.snapshot().interfaces[1],undefined);
  assert.ok(commands.some(([op,v])=>op==='relay'&&v.enabled));n.detach();
});

test('public bootstrap rejects a different origin or a helper impersonating the public relay',async()=>{
  let endpoint='wss://elsewhere.example.test/network';
  const n=createNetworkSession({emit(){},command:async()=>{},sendFrame(){},WebSocketImpl:Socket,fetchImpl:async()=>({ok:true,text:async()=>JSON.stringify({endpoint})})});
  await n.configure({relayUrl:'https://relay.example.test'});await n.attach({available:true});assert.equal(n.snapshot().lastError.code,'NETWORK_PROTOCOL');
  endpoint='wss://relay.example.test/network';await assert.rejects(n.connect(),{code:'NETWORK_PROTOCOL'});n.detach();
});
