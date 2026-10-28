#!/usr/bin/env node
// Public outbound-only relay. The local helper intentionally has a different trust boundary.
import http from 'node:http';
import {isIP} from 'node:net';
import {randomBytes,createHash,timingSafeEqual} from 'node:crypto';
import {existsSync} from 'node:fs';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {resolve} from 'node:path';
import {Backend,attachWebSocket} from './network-helper.mjs';

const ROOT=fileURLToPath(new URL('../',import.meta.url));
const DEFAULTS={maxSessions:16,maxSessionsPerIp:2,maxPending:64,sessionMs:10*60_000,tokenMs:30_000,idleMs:90_000,maxBytes:64*1024*1024,bytesPerSecond:256*1024,burstBytes:512*1024,packetsPerSecond:1000,newFlowsPerMinute:120,sessionsPerMinute:6};
const denied=(code,message)=>Object.assign(new Error(message),{code});
const ipNumber=bytes=>bytes.reduce((n,b)=>n*256+b,0);
// Conservative IANA special-purpose exclusions, plus multicast. Checked 2026-10-28:
// https://www.iana.org/assignments/iana-ipv4-special-registry/
const SPECIAL=['0.0.0.0/8','10.0.0.0/8','100.64.0.0/10','127.0.0.0/8','169.254.0.0/16','172.16.0.0/12','192.0.0.0/24','192.0.2.0/24','192.31.196.0/24','192.52.193.0/24','192.88.99.0/24','192.168.0.0/16','192.175.48.0/24','198.18.0.0/15','198.51.100.0/24','203.0.113.0/24','224.0.0.0/4','240.0.0.0/4'].map(cidr=>{const [ip,bits]=cidr.split('/'),size=2**(32-Number(bits));return [ipNumber(ip.split('.').map(Number)),size];});
export function publicIpv4(ip) {
  const bytes=typeof ip==='string'?(isIP(ip)===4?ip.split('.').map(Number):[]):Array.from(ip??[]);
  if(bytes.length!==4||bytes.some(b=>!Number.isInteger(b)||b<0||b>255))return false;
  const value=ipNumber(bytes);return !SPECIAL.some(([start,size])=>value>=start&&value<start+size);
}
function dnsQuery(bytes) {
  if(bytes.length<17||bytes.length>512||bytes.readUInt16BE(2)&~0x0100||bytes.readUInt16BE(4)!==1||bytes.readUInt16BE(6)||bytes.readUInt16BE(8)||bytes.readUInt16BE(10))return false;
  let at=12,length=0;
  while(at<bytes.length&&bytes[at]){const n=bytes[at++];if(n>63||at+n>=bytes.length||length+n+1>253)return false;at+=n;length+=n+1;}
  return length>0&&at+5===bytes.length&&[1,28].includes(bytes.readUInt16BE(at+1))&&bytes.readUInt16BE(at+3)===1;
}
export function inspectFrame(frame,{tcpPorts=[80,443,1883,8883],udpPorts=[123]}={}) {
  if(!Buffer.isBuffer(frame))frame=Buffer.from(frame);
  if(frame.length<14||frame.length>1518)throw denied('FRAME_DENIED','Invalid Ethernet length');
  const ether=frame.readUInt16BE(12);
  if(ether===0x806){if(frame.length<42||frame.readUInt16BE(14)!==1||frame.readUInt16BE(16)!==0x800||frame[18]!==6||frame[19]!==4||![1,2].includes(frame.readUInt16BE(20)))throw denied('FRAME_DENIED','Invalid ARP');return {protocol:'arp'};}
  if(ether!==0x800||frame.length<34)throw denied('FRAME_DENIED','Only ARP and IPv4 are supported');
  const ip=frame.subarray(14),total=ip.readUInt16BE(2);
  if(ip[0]!==0x45||total<20||total>ip.length||(ip.readUInt16BE(6)&0xbfff))throw denied('FRAME_DENIED','IPv4 options and fragments are not supported');
  const target=ip.subarray(16,20),destination=target.join('.'),sourceAddress=ip.subarray(12,16).join('.'),body=ip.subarray(20,total),protocol=ip[9];
  if(protocol===17){
    if(body.length<8||body.readUInt16BE(4)!==body.length)throw denied('FRAME_DENIED','Invalid UDP length');
    const sourcePort=body.readUInt16BE(0),port=body.readUInt16BE(2),payload=body.subarray(8);
    // DHCP stays inside the virtual router, never reaches a host socket.
    if(port===67&&sourcePort===68&&['255.255.255.255','10.0.2.2'].includes(destination)&&payload.length>=240&&payload.length<=576&&payload[0]===1)return {protocol:'dhcp'};
    // Nat redirects DNS to the explicitly configured PUBLIC resolver, never /etc/resolv.conf.
    if(port===53){if((destination==='10.0.2.3'||publicIpv4(target))&&dnsQuery(payload))return {protocol:'dns',port,destination,sourcePort,sourceAddress};throw denied('FRAME_DENIED','Only bounded DNS queries to public resolvers are supported');}
    if(!publicIpv4(target))throw denied('DESTINATION_DENIED','Private and reserved destinations are unavailable on the public relay');
    if(!udpPorts.includes(port))throw denied('PORT_DENIED','UDP port is not enabled on the public relay');
    if(port===123&&(payload.length!==48||(payload[0]&7)!==3))throw denied('FRAME_DENIED','Only bounded NTP client queries are supported');
    return {protocol:'udp',port,destination,sourcePort,sourceAddress};
  }
  if(!publicIpv4(target))throw denied('DESTINATION_DENIED','Private and reserved destinations are unavailable on the public relay');
  if(protocol!==6||body.length<20||body[12]>>>4<5||(body[12]>>>4)*4>body.length)throw denied('FRAME_DENIED','Only TCP and approved UDP are supported');
  const port=body.readUInt16BE(2);if(!tcpPorts.includes(port))throw denied('PORT_DENIED','TCP port is not enabled on the public relay');
  return {protocol:'tcp',port,destination,sourceAddress,sourcePort:body.readUInt16BE(0),syn:!!(body[13]&2)&&!(body[13]&16)};
}
const portList=value=>Array.isArray(value)&&value.length<=64&&value.every(n=>Number.isInteger(n)&&n>0&&n<=65535);
const equalSecret=(a,b)=>{const x=Buffer.from(a??''),y=Buffer.from(b??'');return x.length===y.length&&timingSafeEqual(x,y);};
const loopback=ip=>['127.0.0.1','::1','localhost','[::1]'].includes(ip);
function clientAddress(request,proxyToken) {
  let ip=request.socket.remoteAddress??'';
  if(proxyToken&&equalSecret(request.headers['x-relay-proxy-token'],proxyToken))ip=request.headers['x-relay-client-ip']??'';
  if(!isIP(ip))throw denied('INVALID_CLIENT','Invalid client address');
  return ip.startsWith('::ffff:')?ip.slice(7):ip;
}
export async function startNetworkRelay({host='127.0.0.1',port=8788,publicUrl,origins=['https://zhuzhe1983.github.io'],resolver='1.1.1.1',tcpPorts=[80,443,1883,8883],udpPorts=[123],limits={},proxyToken,backend=resolve(ROOT,'target/release/esp-network-backend'),backendFactory=(emit)=>new Backend(backend,0,emit,{resolver})}={}) {
  limits={...DEFAULTS,...limits};
  if(!Number.isInteger(port)||port<0||port>65535||!publicIpv4(resolver)||!portList(tcpPorts)||!portList(udpPorts)||Object.keys(limits).some(k=>!(k in DEFAULTS))||Object.values(limits).some(n=>!Number.isSafeInteger(n)||n<1))throw denied('INVALID_CONFIG','Invalid relay limits, ports or public DNS resolver');
  if(!Array.isArray(origins)||!origins.length||origins.some(o=>{try{const u=new URL(o);return u.origin!==o||!['https:','http:'].includes(u.protocol)||u.protocol==='http:'&&!loopback(u.hostname);}catch{return true;}}))throw denied('INVALID_CONFIG','Explicit HTTPS origins are required (HTTP allowed for loopback preview)');
  if(proxyToken!==undefined&&(typeof proxyToken!=='string'||proxyToken.length<32))throw denied('INVALID_CONFIG','Reverse proxy token must have at least 32 characters');
  if(!existsSync(backend))throw denied('BACKEND_MISSING','Build the backend: cargo build --release -p esp-network-backend');
  let base=publicUrl?new URL(publicUrl):undefined;
  if(base&&(base.pathname!=='/'||base.search||base.hash||base.username||base.password||base.protocol!=='https:'&&!(base.protocol==='http:'&&loopback(base.hostname)))||!base&&!loopback(host))throw denied('INVALID_CONFIG','Set an HTTPS public URL, or use a loopback preview');
  const tokens=new Map(),sessions=new Set(),issuance=new Map(),sockets=new Set();
  const caps={ethernet:true,tcp:true,udp:true,http:false,https:false,publicRelay:true,interfaces:[0],maxFrameBytes:1518,tcpPorts,udpPorts:[53,...udpPorts],inbound:false};
  const counts=ip=>[...sessions].filter(s=>s.ip===ip).length+[...tokens.values()].filter(s=>s.ip===ip).length;
  const prune=()=>{const now=Date.now();for(const [id,item]of tokens)if(item.expires<=now)tokens.delete(id);for(const [ip,item]of issuance)if(now-item.at>=60_000)issuance.delete(ip);};
  const timer=setInterval(prune,5000);timer.unref();
  const respond=(res,status,value,origin)=>{res.writeHead(status,{'Content-Type':'application/json','Cache-Control':'no-store','X-Content-Type-Options':'nosniff','Vary':'Origin',...(origin?{'Access-Control-Allow-Origin':origin}:{})});res.end(JSON.stringify(value));};
  const server=http.createServer((req,res)=>{
    req.resume();
    try{
      if(req.url==='/health'&&req.method==='GET'){respond(res,200,{ok:true,version:1,service:'esp-network-relay'});return;}
      const origin=req.headers.origin;
      if(req.headers.host!==base.host||!origins.includes(origin)){respond(res,403,{error:{code:'ORIGIN_DENIED',message:'Origin or Host is not allowed'}});return;}
      if(req.url!=='/session'){respond(res,404,{error:{code:'NOT_FOUND'}} ,origin);return;}
      if(req.method==='OPTIONS'){res.setHeader('Access-Control-Allow-Methods','POST');res.setHeader('Access-Control-Allow-Headers','Content-Type');respond(res,204,undefined,origin);return;}
      if(req.method!=='POST'){respond(res,405,{error:{code:'METHOD_DENIED'}},origin);return;}
      if(req.headers['transfer-encoding']||Number(req.headers['content-length']??0)>1024){respond(res,413,{error:{code:'BODY_LIMIT'}},origin);return;}
      const ip=clientAddress(req,proxyToken);prune();
      const rate=issuance.get(ip)??{at:Date.now(),count:0};
      if(rate.count>=limits.sessionsPerMinute||!issuance.has(ip)&&issuance.size>=4096||tokens.size>=limits.maxPending||sessions.size+tokens.size>=limits.maxSessions||counts(ip)>=limits.maxSessionsPerIp){respond(res,429,{error:{code:'RELAY_BUSY',message:'Relay quota reached; retry later'}},origin);return;}
      rate.count++;issuance.set(ip,rate);const token=randomBytes(32).toString('base64url'),issued=Date.now(),expires=issued+limits.tokenMs;tokens.set(token,{ip,origin,expires});
      const endpoint=new URL('/network',base);endpoint.protocol=base.protocol==='https:'?'wss:':'ws:';endpoint.searchParams.set('token',token);
      respond(res,201,{version:1,endpoint:endpoint.href,expiresAt:new Date(expires).toISOString(),sessionExpiresAt:new Date(issued+limits.sessionMs).toISOString(),capabilities:caps,limits:{...limits}},origin);
    }catch{respond(res,400,{error:{code:'INVALID_REQUEST'}});}
  });
  server.headersTimeout=10_000;server.requestTimeout=10_000;server.keepAliveTimeout=5000;server.maxConnections=limits.maxSessions+limits.maxPending+32;
  server.on('connection',socket=>{sockets.add(socket);socket.on('close',()=>sockets.delete(socket));});
  server.on('clientError',(_error,socket)=>socket.destroy());
  server.on('upgrade',(req,socket,head)=>{
    let ticket,token,ip;
    try{
      const url=new URL(req.url,base);token=url.searchParams.get('token');ticket=tokens.get(token);ip=clientAddress(req,proxyToken);
      if(req.method!=='GET'||url.pathname!=='/network'||req.headers.host!==base.host||!ticket||ticket.expires<=Date.now()||ticket.origin!==req.headers.origin||ticket.ip!==ip||!origins.includes(req.headers.origin)||sessions.size>=limits.maxSessions||req.headers['sec-websocket-version']!=='13'||!/^[A-Za-z0-9+/]{22}==$/.test(req.headers['sec-websocket-key']??'')||String(req.headers.upgrade).toLowerCase()!=='websocket'||!String(req.headers.connection).toLowerCase().split(/\s*,\s*/).includes('upgrade'))throw Error();
    }catch{socket.end('HTTP/1.1 403 Forbidden\r\nConnection: close\r\nContent-Length: 0\r\n\r\n');return;}
    tokens.delete(token);
    const accept=createHash('sha1').update(req.headers['sec-websocket-key']+'258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64');socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
    let connection,child,closed=false,totalBytes=0,credit=limits.burstBytes,last=Date.now(),activity=last,packetAt=last,packets=0,flowAt=last,flows=0;const flowKeys=new Set();
    const session={ip,close:()=>connection?.close(1001,'relay shutdown')};sessions.add(session);
    const close=()=>{if(closed)return;closed=true;clearInterval(watchdog);sessions.delete(session);child?.close();};
    const account=size=>{const now=Date.now();credit=Math.min(limits.burstBytes,credit+(now-last)*limits.bytesPerSecond/1000)-size;last=now;totalBytes+=size;if(credit<0||totalBytes>limits.maxBytes){connection?.close(1008,'relay byte quota exceeded');return false;}return !closed;};
    const send=(op,data)=>{if(closed)return;const body=op===1?JSON.stringify(data):data;if(account(Buffer.byteLength(body)))connection?.send(op,body);};
    const emit=(op,data)=>{if(op===2)activity=Date.now();send(op,data);if(op===1&&data.error)connection?.close(1011,'network backend stopped');};
    const watchdog=setInterval(()=>{if(Date.now()-activity>=limits.idleMs)connection?.close(1008,'relay idle timeout');else if(Date.now()>=ticket.expires-limits.tokenMs+limits.sessionMs)connection?.close(1008,'relay session expired');},Math.min(1000,limits.idleMs,limits.sessionMs));watchdog.unref();
    // Count raw bytes as well as messages: ping/pong cannot bypass traffic limits.
    socket.on('data',chunk=>account(chunk.length));
    connection=attachWebSocket(socket,head,(op,body)=>{
      if(closed)return;activity=Date.now();const now=Date.now();if(now-packetAt>=1000){packetAt=now;packets=0;}if(++packets>limits.packetsPerSecond){connection.close(1008,'relay packet quota exceeded');return;}
      if(op===2){
        if(body[0]!==0)throw denied('INTERFACE_DENIED','Public relay supports station interface 0 only');
        let inspected;
        try{inspected=inspectFrame(body.subarray(1),{tcpPorts,udpPorts});}catch(error){send(1,{type:'network-status',interface:0,...child.status,error:{code:error.code,message:error.message}});return;}
        if(inspected.syn||['udp','dns'].includes(inspected.protocol)){
          if(now-flowAt>=60_000){flowAt=now;flows=0;flowKeys.clear();}
          const key=[inspected.protocol,inspected.destination,inspected.sourceAddress,inspected.sourcePort,inspected.port].join(':');
          if(!flowKeys.has(key)){if(++flows>limits.newFlowsPerMinute){connection.close(1008,'relay new-flow quota exceeded');return;}flowKeys.add(key);}
        }
        child.write('frame '+body.subarray(1).toString('hex'));return;
      }
      const message=JSON.parse(body.toString('utf8'));
      if(!message||!['string','number'].includes(typeof message.id)||String(message.id).length>80)throw denied('INVALID_REQUEST','Expected a request id');
      if(message.op==='status')send(1,{id:message.id,result:{interfaces:[{interface:0,...child.status}],mappings:[],publicRelay:true}});
      else send(1,{id:message.id,error:{code:'PUBLIC_RELAY_OUTBOUND_ONLY',message:'Use the browser for firmware HTTP pages; use the local helper for host port mappings'}});
    },close);
    if(head.length&&!account(head.length))return;
    send(1,{type:'hello',version:1,interfaces:[0],capabilities:caps});
    try{child=backendFactory(emit);if(closed)child.close();}catch{connection.close(1011,'network backend unavailable');}
  });
  try{await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(port,host,resolve);});}catch(error){clearInterval(timer);throw error;}
  base??=new URL(`http://${host==='::1'?'[::1]':host}:${server.address().port}`);
  return {url:base.origin,address:server.address(),capabilities:caps,close:()=>new Promise(resolve=>{clearInterval(timer);tokens.clear();for(const session of sessions)session.close();server.close(resolve);for(const socket of sockets)socket.destroy();})};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  const env=process.env,options={host:env.RELAY_HOST??'127.0.0.1',port:Number(env.RELAY_PORT??8788),publicUrl:env.RELAY_PUBLIC_URL,origins:(env.RELAY_ORIGINS??'https://zhuzhe1983.github.io').split(',').filter(Boolean),resolver:env.RELAY_DNS??'1.1.1.1',proxyToken:env.RELAY_PROXY_TOKEN,...(env.RELAY_BACKEND?{backend:env.RELAY_BACKEND}:{}),limits:{}};
  for(const [key,value]of Object.entries(DEFAULTS)){const name='RELAY_'+key.replace(/[A-Z]/g,m=>'_'+m).toUpperCase();options.limits[key]=Number(env[name]??value);}
  for(const [key,name]of [['tcpPorts','RELAY_TCP_PORTS'],['udpPorts','RELAY_UDP_PORTS']])if(env[name]!==undefined)options[key]=env[name].split(',').filter(Boolean).map(Number);
  const relay=await startNetworkRelay(options);console.log(`ESP public network relay listening at ${relay.url}; station egress only; no session tokens are logged.`);for(const signal of ['SIGINT','SIGTERM'])process.on(signal,()=>void relay.close().then(()=>process.exit(0)));
}
