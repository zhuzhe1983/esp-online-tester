import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import {randomBytes} from 'node:crypto';
import {inspectFrame,publicIpv4,startNetworkRelay} from './network-relay.mjs';

const origin='https://zhuzhe1983.github.io';
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const wait=async check=>{for(let i=0;i<2000;i++){const value=check();if(value)return value;await delay(5);}throw new Error('condition timed out');};
function packet(destination,port,{udp=false,payload=Buffer.alloc(0),sourcePort=22000}={}) {
  const transport=Buffer.alloc(udp?8:20);transport.writeUInt16BE(sourcePort);transport.writeUInt16BE(port,2);
  if(udp)transport.writeUInt16BE(8+payload.length,4);else{transport[12]=0x50;transport[13]=2;transport.writeUInt16BE(5840,14);}
  const ip=Buffer.alloc(20);ip[0]=0x45;ip[8]=64;ip[9]=udp?17:6;ip.writeUInt16BE(ip.length+transport.length+payload.length,2);ip.set([10,0,2,15],12);ip.set(destination.split('.').map(Number),16);
  const eth=Buffer.from('0200000000020200000000150800','hex');return Buffer.concat([eth,ip,transport,payload]);
}
const dns=Buffer.from('123401000001000000000000076578616d706c6503636f6d0000010001','hex');
function masked(op,body){body=Buffer.from(body);const mask=Buffer.from([1,2,3,4]),prefix=body.length<126?Buffer.from([0x80|op,0x80|body.length]):Buffer.from([0x80|op,0xfe,body.length>>>8,body.length&255]);const encoded=Buffer.from(body);for(let i=0;i<encoded.length;i++)encoded[i]^=mask[i%4];return Buffer.concat([prefix,mask,encoded]);}
async function connect(endpoint,requestOrigin=origin,extra={}) {
  const url=new URL(endpoint),request=http.request({hostname:url.hostname,port:url.port,path:url.pathname+url.search,headers:{origin:requestOrigin,connection:'Upgrade',upgrade:'websocket','sec-websocket-version':'13','sec-websocket-key':randomBytes(16).toString('base64'),...extra}});
  return new Promise((resolve,reject)=>{request.on('error',reject);request.on('response',res=>{res.resume();resolve({status:res.statusCode});});request.on('upgrade',(response,socket,head)=>{
    const frames=[];let pending=Buffer.alloc(0),closed=false;
    const receive=chunk=>{pending=Buffer.concat([pending,chunk]);while(pending.length>=2){let n=pending[1]&127,at=2;if(n===126){if(pending.length<4)return;n=pending.readUInt16BE(2);at=4;}if(pending.length<at+n)return;frames.push({op:pending[0]&15,body:Buffer.from(pending.subarray(at,at+n))});pending=pending.subarray(at+n);}};
    socket.on('data',receive);socket.on('close',()=>{closed=true;});socket.on('error',()=>{});receive(head);
    resolve({status:response.statusCode,frames,socket,get closed(){return closed;},send:(op,body)=>socket.write(masked(op,body)),json:value=>socket.write(masked(1,JSON.stringify(value))),close:()=>socket.destroy()});
  });request.end();});
}
async function fixture(options={}) {
  const writes=[],children=[];const relay=await startNetworkRelay({port:0,backend:process.execPath,backendFactory:emit=>{const child={status:{ready:true},write:line=>writes.push(line),closed:false,close(){this.closed=true;},emit};children.push(child);queueMicrotask(()=>emit(1,{type:'network-status',interface:0,ready:true}));return child;},...options});
  const issue=(extra={})=>fetch(relay.url+'/session',{method:'POST',headers:{origin,...extra}});
  return {...relay,writes,children,issue};
}

test('public IPv4 excludes local, CGNAT, metadata, documentation and special ranges',()=>{
  for(const ip of ['0.1.2.3','10.9.8.7','100.64.0.1','100.127.255.255','127.12.0.1','169.254.169.254','172.16.0.1','172.31.255.255','192.0.0.9','192.0.2.1','192.31.196.2','192.52.193.1','192.88.99.2','192.168.1.1','192.175.48.1','198.18.0.1','198.19.255.255','198.51.100.2','203.0.113.1','224.0.0.1','239.1.2.3','240.0.0.1','255.255.255.255','::1','1.2.3.999'])assert.equal(publicIpv4(ip),false,ip);
  for(const ip of ['1.1.1.1','8.8.8.8','100.63.255.255','100.128.0.1','172.15.255.255','172.32.0.1','198.17.255.255','198.20.0.1'])assert.equal(publicIpv4(ip),true,ip);
});
test('egress policy checks every IP destination; DNS cannot grant a private address exception',()=>{
  for(const port of [80,443,1883,8883])assert.equal(inspectFrame(packet('1.1.1.1',port)).port,port);
  for(const ip of ['127.0.0.1','10.0.2.2','192.168.4.1','169.254.169.254','100.100.100.200'])assert.throws(()=>inspectFrame(packet(ip,443)),{code:'DESTINATION_DENIED'});
  assert.equal(inspectFrame(packet('10.0.2.3',53,{udp:true,payload:dns})).protocol,'dns');
  assert.throws(()=>inspectFrame(packet('10.0.2.3',443)),{code:'DESTINATION_DENIED'});
  assert.throws(()=>inspectFrame(packet('1.1.1.1',22)),{code:'PORT_DENIED'});
  assert.throws(()=>inspectFrame(packet('1.1.1.1',53,{udp:true,payload:Buffer.alloc(512)}),{udpPorts:[53]}),{code:'FRAME_DENIED'});
  const ntp=Buffer.alloc(48);ntp[0]=0x23;assert.equal(inspectFrame(packet('1.1.1.1',123,{udp:true,payload:ntp})).protocol,'udp');ntp[0]=0x17;assert.throws(()=>inspectFrame(packet('1.1.1.1',123,{udp:true,payload:ntp})),{code:'FRAME_DENIED'});
  assert.throws(()=>inspectFrame(packet('1.1.1.1',67,{udp:true,payload:Buffer.alloc(240),sourcePort:68})),{code:'PORT_DENIED'});
  const fragmented=packet('1.1.1.1',443);fragmented[20]=0x20;assert.throws(()=>inspectFrame(fragmented),{code:'FRAME_DENIED'});
  for(let n=0;n<54;n++)assert.throws(()=>inspectFrame(Buffer.alloc(n)));
});
test('one-use Origin-bound sessions forward permitted frames and expose no inbound helpers',async()=>{
  const relay=await fixture();let ws;
  try{
    assert.equal((await fetch(relay.url+'/health')).status,200);
    assert.equal((await relay.issue({origin:'https://foreign.invalid'})).status,403);
    const response=await relay.issue();assert.equal(response.status,201);assert.equal(response.headers.get('access-control-allow-origin'),origin);const ticket=await response.json();assert.equal(ticket.capabilities.publicRelay,true);assert.equal(ticket.capabilities.inbound,false);
    assert.equal((await connect(ticket.endpoint,'https://foreign.invalid')).status,403);assert.equal((await connect(ticket.endpoint,undefined,{origin:''})).status,403);
    ws=await connect(ticket.endpoint);assert.equal(ws.status,101);assert.equal((await connect(ticket.endpoint)).status,403);
    await wait(()=>ws.frames.some(f=>f.op===1&&JSON.parse(f.body).type==='hello'));
    ws.send(2,Buffer.concat([Buffer.from([0]),packet('127.0.0.1',443)]));await wait(()=>ws.frames.some(f=>f.op===1&&JSON.parse(f.body).error?.code==='DESTINATION_DENIED'));assert.equal(relay.writes.length,0);
    ws.send(2,Buffer.concat([Buffer.from([0]),packet('1.1.1.1',443)]));await wait(()=>relay.writes.length===1);
    for(const op of ['map','http','interface'])ws.json({id:op,op});await wait(()=>ws.frames.filter(f=>f.op===1&&JSON.parse(f.body).error?.code==='PUBLIC_RELAY_OUTBOUND_ONLY').length===3);
    ws.send(2,Buffer.concat([Buffer.from([1]),packet('1.1.1.1',443)]));await wait(()=>ws.frames.some(f=>f.op===8));await wait(()=>relay.children[0].closed);
  }finally{ws?.close();await relay.close();}
});
test('pending ticket quotas, expiry, idle and byte limits close their resources',async()=>{
  const relay=await fixture({limits:{maxSessionsPerIp:1,tokenMs:30,idleMs:40,maxBytes:2048,burstBytes:2048}});let ws;
  try{
    const first=await (await relay.issue()).json();assert.equal((await relay.issue()).status,429);await delay(40);assert.equal((await connect(first.endpoint)).status,403);
    const ticket=await (await relay.issue()).json();ws=await connect(ticket.endpoint);assert.equal(ws.status,101);await wait(()=>ws.frames.some(f=>f.op===8));assert.equal(ws.frames.find(f=>f.op===8).body.readUInt16BE(0),1008);await wait(()=>relay.children[0].closed);ws.close();
    const next=await (await relay.issue()).json();ws=await connect(next.endpoint);ws.send(2,Buffer.alloc(2200));await wait(()=>ws.frames.some(f=>f.op===8));assert.match(ws.frames.find(f=>f.op===8).body.toString(),/quota/);
  }finally{ws?.close();await relay.close();}
});
test('proxy client IP headers require the shared reverse-proxy credential',async()=>{
  const relay=await fixture({proxyToken:'test-only-proxy-secret-with-32-characters',limits:{maxSessionsPerIp:1}});
  try{
    assert.equal((await relay.issue({'x-relay-client-ip':'1.1.1.1'})).status,201);
    assert.equal((await relay.issue({'x-relay-client-ip':'8.8.8.8'})).status,429);
    const headers={'x-relay-client-ip':'8.8.8.8','x-relay-proxy-token':'test-only-proxy-secret-with-32-characters'};
    const ticket=await (await relay.issue(headers)).json();assert.equal((await connect(ticket.endpoint)).status,403);const ws=await connect(ticket.endpoint,origin,headers);assert.equal(ws.status,101);ws.close();
  }finally{await relay.close();}
});
test('session duration, message rate and new-flow quotas terminate the backend',async()=>{
  for(const limits of [{sessionMs:30},{packetsPerSecond:1},{newFlowsPerMinute:1}]){
    const relay=await fixture({limits});let ws;
    try{
      const ticket=await (await relay.issue()).json();ws=await connect(ticket.endpoint);
      if(limits.packetsPerSecond){ws.json({id:1,op:'status'});ws.json({id:2,op:'status'});}
      if(limits.newFlowsPerMinute)for(const port of [22001,22002])ws.send(2,Buffer.concat([Buffer.from([0]),packet('1.1.1.1',443,{sourcePort:port})]));
      const frame=await wait(()=>ws.frames.find(f=>f.op===8));assert.equal(frame.body.readUInt16BE(0),1008);await wait(()=>relay.children[0].closed);
      if(limits.newFlowsPerMinute)assert.equal(relay.writes.length,1);
    }finally{ws?.close();await relay.close();}
  }
});
test('configuration cannot opt into private DNS or cleartext remote public URLs',async()=>{
  for(const options of [{resolver:'127.0.0.1'},{resolver:'169.254.169.254'},{publicUrl:'http://relay.example.invalid'},{host:'0.0.0.0'},{origins:['*']},{proxyToken:'short'}])await assert.rejects(startNetworkRelay({backend:process.execPath,...options}),{code:'INVALID_CONFIG'});
});
test('real native backend opens the caller-supplied public TCP target',{skip:!process.env.ESP_RELAY_TEST_TARGET},async()=>{
  const [address,portText]=process.env.ESP_RELAY_TEST_TARGET.split(':'),port=Number(portText);assert.ok(publicIpv4(address));
  const relay=await startNetworkRelay({port:0,tcpPorts:[port],...(process.env.ESP_NETWORK_BACKEND?{backend:process.env.ESP_NETWORK_BACKEND}:{})});let ws;
  try{
    const response=await fetch(relay.url+'/session',{method:'POST',headers:{origin}});assert.equal(response.status,201);ws=await connect((await response.json()).endpoint);
    ws.send(2,Buffer.concat([Buffer.from([0]),packet(address,port)]));
    const reply=await wait(()=>ws.frames.find(f=>f.op===2&&f.body.length>=55&&f.body[24]===6));
    assert.equal(reply.body[48]&0x17,0x12,'SYN+ACK proves that the native connector reached the public TCP target');
  }finally{ws?.close();await relay.close();}
});
test('real native backend forwards DNS through the caller-supplied public resolver',{skip:!process.env.ESP_RELAY_TEST_DNS},async()=>{
  const relay=await startNetworkRelay({port:0,resolver:process.env.ESP_RELAY_TEST_DNS,...(process.env.ESP_NETWORK_BACKEND?{backend:process.env.ESP_NETWORK_BACKEND}:{})});let ws;
  try{
    const response=await fetch(relay.url+'/session',{method:'POST',headers:{origin}});assert.equal(response.status,201);ws=await connect((await response.json()).endpoint);
    ws.send(2,Buffer.concat([Buffer.from([0]),packet('10.0.2.3',53,{udp:true,payload:dns})]));
    const reply=await wait(()=>ws.frames.find(f=>f.op===2&&f.body.length>=55&&f.body[24]===17&&f.body.readUInt16BE(43)===0x1234));
    assert.ok(reply.body.readUInt16BE(45)&0x8000,'DNS response arrived through the public UDP resolver');assert.equal(reply.body.readUInt16BE(45)&15,0);assert.ok(reply.body.readUInt16BE(49)>0);
  }finally{ws?.close();await relay.close();}
});
