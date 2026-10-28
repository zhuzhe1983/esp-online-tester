#!/usr/bin/env node
// End-to-end packet tunnel tests use real loopback TCP/UDP/TLS servers, not guest firmware.
import assert from 'node:assert/strict';
import {startNetworkHelper} from './network-helper.mjs';
import http from 'node:http';
import net from 'node:net';
import dgram from 'node:dgram';
import tls from 'node:tls';
import {Duplex} from 'node:stream';
import {once} from 'node:events';
import {mkdtemp,readFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {execFileSync} from 'node:child_process';
const backend=process.env.ESP_NETWORK_BACKEND??resolve('target/release/esp-network-backend');
const mac=Buffer.from([2,1,2,3,4,5]),gwmac=Buffer.from([2,0x53,0x49,0x4d,0,2]),ip=Buffer.from([10,0,2,15]),gateway=Buffer.from([10,0,2,2]),hostip=Buffer.from([127,0,0,1]);
function checksum(data){let sum=0;for(let i=0;i<data.length;i+=2)sum+=(data[i]<<8)|(data[i+1]??0);while(sum>>>16)sum=(sum&65535)+(sum>>>16);return(~sum)&65535;}
function packet(protocol,source,dest,body,sourceMac=mac,destMac=gwmac){const h=Buffer.alloc(20);h[0]=0x45;h.writeUInt16BE(20+body.length,2);h[8]=64;h[9]=protocol;source.copy(h,12);dest.copy(h,16);h.writeUInt16BE(checksum(h),10);return Buffer.concat([destMac,sourceMac,Buffer.from([8,0]),h,body]);}
function tcp(source,dest,sport,dport,seq,ack,flags,body=Buffer.alloc(0)){body=Buffer.from(body);const h=Buffer.alloc(20);h.writeUInt16BE(sport);h.writeUInt16BE(dport,2);h.writeUInt32BE(seq>>>0,4);h.writeUInt32BE(ack>>>0,8);h[12]=0x50;h[13]=flags;h.writeUInt16BE(60000,14);const seg=Buffer.concat([h,body]),pseudo=Buffer.concat([source,dest,Buffer.from([0,6,seg.length>>>8,seg.length&255]),seg]);seg.writeUInt16BE(checksum(pseudo),16);return packet(6,source,dest,seg);}
function udp(source,dest,sport,dport,body){body=Buffer.from(body);const h=Buffer.alloc(8);h.writeUInt16BE(sport);h.writeUInt16BE(dport,2);h.writeUInt16BE(8+body.length,4);return packet(17,source,dest,Buffer.concat([h,body]));}
class Guest {
 constructor(ws){this.ws=ws;this.requests=new Map();this.status=new Map();this.seq=0;this.connections=new Map();this.listeners=new Map();this.udp=[];ws.binaryType='arraybuffer';ws.addEventListener('message',event=>{if(typeof event.data==='string'){const m=JSON.parse(event.data);if(m.type==='network-status')this.status.set(m.interface,m);if(m.id){const r=this.requests.get(m.id);if(r){clearTimeout(r.timer);this.requests.delete(m.id);m.error?r.reject(Object.assign(new Error(m.error.message),m.error)):r.resolve(m.result);}}return;}const data=Buffer.from(event.data);this.frame(data[0],data.subarray(1));});}
 send(frame,iface=0){this.ws.send(Buffer.concat([Buffer.from([iface]),frame]));}
 rpc(op,options={}){const id=String(++this.seq);return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{this.requests.delete(id);reject(new Error(`RPC timeout ${op}`));},10000);this.requests.set(id,{resolve,reject,timer});this.ws.send(JSON.stringify({id,op,...options}));});}
 frame(iface,frame){if(iface!==0||frame.length<34)return;const protocol=frame[23],off=14+(frame[14]&15)*4;if(protocol===17){this.udp.push(frame);return;}if(protocol!==6)return;const t=frame.subarray(off),sport=t.readUInt16BE(0),dport=t.readUInt16BE(2),flags=t[13],seq=t.readUInt32BE(4),ack=t.readUInt32BE(8),payload=t.subarray((t[12]>>4)*4),remote=frame.subarray(26,30),key=`${dport}:${sport}`;let c=this.connections.get(key);
  if(flags&2){if(!c){const listener=this.listeners.get(dport);if(!listener)return; c=this.create(dport,sport,remote);c.rx=seq+1;this.send(tcp(ip,remote,dport,sport,c.tx,c.rx,0x12));c.tx++;listener(c.stream);return;}c.rx=seq+1;this.send(tcp(ip,remote,dport,sport,c.tx,c.rx,0x10));c.open?.();return;}
  if(!c)return;if(flags&4){c.stream.destroy(new Error('guest TCP reset'));return;}if(payload.length&&seq===c.rx){c.rx+=payload.length;c.stream.push(payload);this.send(tcp(ip,remote,dport,sport,c.tx,c.rx,0x10));}if(flags&1&&seq+payload.length===c.rx){c.rx++;this.send(tcp(ip,remote,dport,sport,c.tx,c.rx,0x10));c.stream.push(null);}
 }
 create(local,remotePort,remote){const c={tx:1000,rx:0};c.stream=new Duplex({read(){},write:(chunk,encoding,done)=>{for(let n=0;n<chunk.length;n+=1400){const data=chunk.subarray(n,n+1400);this.send(tcp(ip,remote,local,remotePort,c.tx,c.rx,0x18,data));c.tx+=data.length;}done();},final:done=>{this.send(tcp(ip,remote,local,remotePort,c.tx,c.rx,0x11));c.tx++;done();}});c.stream.on('error',()=>{});this.connections.set(`${local}:${remotePort}`,c);return c;}
 connect(port){const local=20000+(++this.seq);const c=this.create(local,port,hostip);return new Promise((resolve,reject)=>{const timer=setTimeout(()=>reject(new Error('TCP open timeout')),5000);c.open=()=>{clearTimeout(timer);resolve(c.stream);};this.send(tcp(ip,hostip,local,port,c.tx,0,2));c.tx++;});}
}
const listen=server=>new Promise((resolve,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',()=>resolve(server.address().port));});
const timeout=(ms=10)=>new Promise(r=>setTimeout(r,ms));
const wait=async condition=>{const end=Date.now()+10000;while(!condition()){assert.ok(Date.now()<end,'condition timeout');await timeout();}};
const closed=[];let helper,ws,temp;
try{
 helper=await startNetworkHelper({port:0,backend,token:'test-only-token-01234567890123456789'});
 assert.equal((await fetch(helper.origin+'/network-info')).status,200);assert.equal((await fetch(helper.origin+'/network-info',{headers:{origin:'https://evil.invalid'}})).status,403);
 assert.equal((await fetch(helper.origin+'/platform/')).status,200);assert.equal((await fetch(helper.origin+'/../Cargo.toml')).status,404);
 const denied=async(endpoint,origin)=>new Promise((resolve,reject)=>{const u=new URL(endpoint);const req=http.request({hostname:u.hostname,port:u.port,path:u.pathname+u.search,headers:{origin,connection:'Upgrade',upgrade:'websocket','sec-websocket-version':'13','sec-websocket-key':Buffer.alloc(16,1).toString('base64')}},res=>{res.resume();resolve(res.statusCode)});req.on('upgrade',(res,socket)=>{socket.destroy();resolve(101)});req.on('error',reject);req.end();});
 assert.equal(await denied(helper.endpoint,'https://foreign.invalid'),403);assert.equal(await denied(helper.endpoint.replace(/token=.*/, 'token=wrong'),'https://zhuzhe1983.github.io'),403);
 ws=new WebSocket(helper.endpoint);await once(ws,'open');const guest=new Guest(ws);
 // An ARP frame establishes the actual guest MAC before mappings may be opened.
 const arp=Buffer.alloc(28);arp.set([0,1,8,0,6,4,0,1]);mac.copy(arp,8);ip.copy(arp,14);gateway.copy(arp,24);guest.send(Buffer.concat([gwmac,mac,Buffer.from([8,6]),arp]));await wait(()=>guest.status.get(0)?.ready);
 const server=http.createServer((req,res)=>res.end('real-host-http-response'));closed.push(()=>new Promise(r=>server.close(r)));const port=await listen(server);
 const stream=await guest.connect(port);let body='';stream.on('data',chunk=>body+=chunk);stream.end('GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n');await once(stream,'end');assert.match(body,/real-host-http-response/);
 const echo=dgram.createSocket('udp4');closed.push(()=>new Promise(r=>echo.close(r)));echo.on('message',(message,peer)=>echo.send(Buffer.concat([Buffer.from('host:'),message]),peer.port,peer.address));await new Promise(r=>echo.bind(0,'127.0.0.1',r));guest.send(udp(ip,hostip,23000,echo.address().port,'udp-out'));await wait(()=>guest.udp.some(f=>f.subarray(42).toString()==='host:udp-out'));
 guest.listeners.set(80,stream=>{let text='';stream.on('data',chunk=>{text+=chunk;if(text.includes('\r\n\r\n'))stream.end('HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 14\r\nConnection: close\r\n\r\n<h1>guest</h1>');});});
 const response=await guest.rpc('http',{path:'/'});assert.equal(response.status,200);assert.equal(response.body,'<h1>guest</h1>');
 const mapping=await guest.rpc('map',{protocol:'http',guestPort:80});assert.notEqual(new URL(mapping.url).origin,helper.origin);assert.equal(await fetch(mapping.url).then(r=>r.text()),'<h1>guest</h1>');await guest.rpc('unmap',{mappingId:mapping.mappingId});await assert.rejects(fetch(mapping.url));
 const udpmapping=await guest.rpc('map',{protocol:'udp',guestPort:9000});const client=dgram.createSocket('udp4');closed.push(()=>new Promise(r=>client.close(r)));client.send(Buffer.from('mapped-udp'),udpmapping.hostPort,'127.0.0.1');await wait(()=>guest.udp.some(f=>f.subarray(42).toString()==='mapped-udp'));const request=guest.udp.find(f=>f.subarray(42).toString()==='mapped-udp');const received=once(client,'message');guest.send(udp(ip,gateway,9000,request.readUInt16BE(34),'guest-udp-response'));assert.equal((await received)[0].toString(),'guest-udp-response');await guest.rpc('unmap',{mappingId:udpmapping.mappingId});
 temp=await mkdtemp(join(tmpdir(),'esp-network-tls-'));execFileSync('openssl',['req','-x509','-newkey','rsa:2048','-nodes','-keyout',join(temp,'key.pem'),'-out',join(temp,'cert.pem'),'-days','1','-subj','/CN=localhost','-addext','subjectAltName=DNS:localhost'],{stdio:'ignore'});
 const cert=await readFile(join(temp,'cert.pem')),key=await readFile(join(temp,'key.pem'));const secure=tls.createServer({cert,key},socket=>socket.on('data',()=>socket.end('HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nreal-tls')));closed.push(()=>new Promise(r=>secure.close(r)));const securePort=await listen(secure);const tunnel=await guest.connect(securePort),ssl=tls.connect({socket:tunnel,servername:'localhost',ca:cert});let encrypted='';ssl.on('data',chunk=>encrypted+=chunk);await once(ssl,'secureConnect');assert.equal(ssl.authorized,true);ssl.end('GET / HTTP/1.1\r\nHost: localhost\r\n\r\n');await once(ssl,'end');assert.match(encrypted,/real-tls/);
 await assert.rejects(guest.rpc('map',{protocol:'tcp',guestPort:0}),{code:'INVALID_MAPPING'});
 ws.close();await once(ws,'close');ws=null;console.log('PASS: authenticated WS + real HTTP/TLS/UDP out, HTTP RPC/isolated proxy, UDP in/out, mapping cleanup, static/bootstrap bounds.');
}finally{ws?.close();await helper?.close();for(const close of closed.reverse())await close();if(temp)await rm(temp,{recursive:true,force:true});}
