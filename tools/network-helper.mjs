#!/usr/bin/env node
// Local TCP/UDP gateway for the browser simulator. No npm packages or privileged network device.
import http from 'node:http';
import https from 'node:https';
import net from 'node:net';
import {spawn} from 'node:child_process';
import {randomBytes, createHash, timingSafeEqual} from 'node:crypto';
import {readFile, realpath, stat} from 'node:fs/promises';
import {existsSync} from 'node:fs';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {resolve, extname, sep} from 'node:path';
import {createInterface} from 'node:readline';

const ROOT=fileURLToPath(new URL('../',import.meta.url)), MAX_MESSAGE=65536, MAX_RESPONSE=2*1024*1024, MAX_BUFFER=1024*1024;
const types={'.html':'text/html; charset=utf-8','.js':'application/javascript','.mjs':'application/javascript','.css':'text/css','.json':'application/json','.wasm':'application/wasm','.svg':'image/svg+xml','.png':'image/png','.webp':'image/webp','.md':'text/markdown; charset=utf-8'};
const fail=(code,message)=>Object.assign(new Error(message),{code});
const validPort=(n,zero=false)=>Number.isInteger(n)&&n>=(zero?0:1)&&n<=65535;
const headerAllowed=name=>!['host','connection','content-length','transfer-encoding','upgrade','proxy-authorization','proxy-connection'].includes(name.toLowerCase());
function websocketFrame(opcode,bytes) {
  bytes=Buffer.from(bytes);const prefix=bytes.length<126?Buffer.from([0x80|opcode,bytes.length]):bytes.length<65536?Buffer.from([0x80|opcode,126,bytes.length>>>8,bytes.length&255]):Buffer.from([0x80|opcode,127,0,0,0,0,(bytes.length>>>24)&255,(bytes.length>>>16)&255,(bytes.length>>>8)&255,bytes.length&255]);return Buffer.concat([prefix,bytes]);
}
function attachWebSocket(socket,head,onMessage,onClose) {
  let input=Buffer.alloc(0),closed=false;const send=(op,data)=>{if(closed)return;if(socket.writableLength>MAX_BUFFER){close(1013,'network client too slow');return;}socket.write(websocketFrame(op,data));};
  const close=(code=1000,reason='closed')=>{if(closed)return;closed=true;const body=Buffer.alloc(2);body.writeUInt16BE(code);socket.end(websocketFrame(8,Buffer.concat([body,Buffer.from(reason).subarray(0,120)])));onClose();};
  const receive=chunk=>{if(closed)return;input=Buffer.concat([input,chunk]);if(input.length>MAX_MESSAGE+14){close(1009,'message limit');return;}
    while(input.length>=2){const op=input[0]&15,fin=input[0]&128,masked=input[1]&128;let length=input[1]&127,offset=2;
      if(!fin||(input[0]&0x70)||!masked||![1,2,8,9,10].includes(op)){close(1002,'unsupported frame');return;}
      if(length===126){if(input.length<4)return;length=input.readUInt16BE(2);offset=4;}
      else if(length===127){if(input.length<10)return;const n=input.readBigUInt64BE(2);if(n>BigInt(MAX_MESSAGE)){close(1009,'message limit');return;}length=Number(n);offset=10;}
      if(length>MAX_MESSAGE||op>=8&&length>125){close(1009,'message limit');return;}if(input.length<offset+4+length)return;
      const mask=input.subarray(offset,offset+4),body=Buffer.from(input.subarray(offset+4,offset+4+length));for(let i=0;i<body.length;i++)body[i]^=mask[i%4];input=input.subarray(offset+4+length);
      if(op===8){close();return;}if(op===9){send(10,body);continue;}if(op===10)continue;
      try{onMessage(op,body);}catch(error){close(1007,error.message);return;}
    }
  };
  socket.on('data',receive);socket.on('error',()=>close(1011,'socket error'));socket.on('close',()=>{if(!closed){closed=true;onClose();}});if(head.length)queueMicrotask(()=>receive(head));return {send,close};
}
class Backend {
  constructor(executable,iface,emit){this.iface=iface;this.emit=emit;this.pending=new Map();this.sequence=0;this.closed=false;this.status={ready:false,leaseReady:false};
    this.process=spawn(executable,iface===1?['--client']:[],{stdio:['pipe','pipe','pipe']});
    createInterface({input:this.process.stdout}).on('line',line=>{if(line.startsWith('frame ')){const frame=Buffer.from(line.slice(6),'hex');if(frame.length>=14&&frame.length<=1518)this.emit(2,Buffer.concat([Buffer.from([iface]),frame]));}
      else if(line.startsWith('status ')){try{this.status=JSON.parse(line.slice(7));this.status.leaseReady=this.status.ready;this.emit(1,{type:'network-status',interface:iface,...this.status});}catch{this.close();}}
      else if(line.startsWith('reply ')){const [,id,result,value]=line.split(' '),request=this.pending.get(id);if(!request)return;this.pending.delete(id);clearTimeout(request.timer);result==='ok'?request.resolve(Number(value)):request.reject(fail('MAPPING_FAILED',value||'Mapping rejected'));}});
    this.process.stderr.on('data',()=>{});this.process.on('error',error=>this.close(error));this.process.on('exit',()=>this.close(fail('BACKEND_EXIT','Network backend exited')));
  }
  write(line){if(this.closed)throw fail('BACKEND_EXIT','Network backend closed');if(this.process.stdin.writableLength>MAX_BUFFER)throw fail('NETWORK_BACKPRESSURE','Network backend queue full');this.process.stdin.write(line+'\n');}
  request(command){if(this.pending.size>=16)return Promise.reject(fail('TOO_MANY_REQUESTS','Too many pending network operations'));const id=String(++this.sequence);return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{this.pending.delete(id);reject(fail('BACKEND_TIMEOUT','Network backend timeout'));},10000);this.pending.set(id,{resolve,reject,timer});try{this.write(command(id));}catch(error){clearTimeout(timer);this.pending.delete(id);reject(error);}});}
  map(protocol,hostPort,guestPort){let backendId;return this.request(id=>{backendId=Number(id);return `map ${id} ${protocol} ${hostPort} ${guestPort}`;}).then(port=>({backendId,port}));}
  unmap(backendId){this.write(`unmap ${backendId}`);}
  close(error=fail('SESSION_CLOSED','Network session closed')){if(this.closed)return;this.closed=true;for(const pending of this.pending.values()){clearTimeout(pending.timer);pending.reject(error);}this.pending.clear();this.process.stdin.destroy();this.process.kill();this.emit(1,{type:'network-status',interface:this.iface,enabled:false,ready:false,leaseReady:false,error:{code:error.code??'BACKEND_EXIT',message:error.message}});}
}
function guestRequest(port,guest,options={}) {
  const method=options.method??'GET',path=options.path??'/';if(!['GET','HEAD','POST','PUT','PATCH','DELETE','OPTIONS'].includes(method)||typeof path!=='string'||!path.startsWith('/')||path.startsWith('//')||/[\r\n]/.test(path))throw fail('INVALID_HTTP','Invalid guest HTTP method or path');
  const headers={host:guest,connection:'close'};for(const [key,value]of Object.entries(options.headers??{})){if(headerAllowed(key)&&typeof value==='string')headers[key]=value;}
  const body=options.bodyBase64?Buffer.from(options.bodyBase64,'base64'):Buffer.from(options.body??'');if(body.length>32768)throw fail('HTTP_BODY_LIMIT','Request body exceeds 32 KiB');if(body.length)headers['content-length']=body.length;
  return new Promise((resolve,reject)=>{const request=http.request({host:'127.0.0.1',port,path,method,headers},response=>{const chunks=[];let length=0;response.on('data',chunk=>{length+=chunk.length;if(length>MAX_RESPONSE){response.destroy(fail('HTTP_RESPONSE_LIMIT','Guest response exceeds 2 MiB'));return;}chunks.push(chunk);});response.on('error',reject);response.on('end',()=>{const data=Buffer.concat(chunks);resolve({status:response.statusCode,headers:response.headers,body:data.toString('utf8'),bodyBase64:data.toString('base64')});});});request.setTimeout(90000,()=>request.destroy(fail('HTTP_TIMEOUT','Guest HTTP request timed out')));request.on('error',reject);request.end(body);});
}
async function guestProxy(mapping,backend,secureOptions) {
  const sockets=new Set();const server=(secureOptions?https:http).createServer(secureOptions??{},(request,response)=>{
    const proxy=http.request({host:'127.0.0.1',port:mapping.port,method:request.method,path:request.url,headers:{...request.headers,host:backend.status.guestAddress}},upstream=>{
      const headers={...upstream.headers};const location=headers.location;
      if(location){try{const target=new URL(location,`http://${backend.status.guestAddress}/`);if(target.hostname===backend.status.guestAddress)headers.location=`${target.pathname}${target.search}${target.hash}`;}catch{}}
      response.writeHead(upstream.statusCode??502,headers);upstream.pipe(response);upstream.on('error',()=>response.destroy());
    });proxy.setTimeout(90000,()=>proxy.destroy());proxy.on('error',()=>{if(!response.headersSent)response.writeHead(502);response.end('Guest service unavailable');});request.pipe(proxy);request.on('aborted',()=>proxy.destroy());
  });
  server.on('connection',socket=>{sockets.add(socket);socket.on('close',()=>sockets.delete(socket));});
  server.on('upgrade',(request,socket,head)=>{const target=net.connect(mapping.port,'127.0.0.1');target.once('connect',()=>{const headers=Object.entries({...request.headers,host:backend.status.guestAddress}).map(([k,v])=>`${k}: ${v}`).join('\r\n');target.write(`${request.method} ${request.url} HTTP/1.1\r\n${headers}\r\n\r\n`);if(head.length)target.write(head);socket.pipe(target);target.pipe(socket);});target.on('error',()=>socket.destroy());socket.on('error',()=>target.destroy());socket.on('close',()=>target.destroy());});
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',resolve);});
  return {url:`${secureOptions?'https':'http'}://127.0.0.1:${server.address().port}/`,close(){for(const socket of sockets)socket.destroy();server.close();}};
}
export async function startNetworkHelper({port=8787,host='127.0.0.1',token=randomBytes(24).toString('hex'),backend=resolve(ROOT,'target/release/esp-network-backend'),web=resolve(ROOT,'web'),origins=['https://zhuzhe1983.github.io'],cert,key}={}) {
  if(!['127.0.0.1','::1','localhost'].includes(host))throw fail('BIND_DENIED','Helper must bind loopback; use an authenticated TLS reverse proxy for remote hosting');
  if(!validPort(port,true)||typeof token!=='string'||token.length<24)throw fail('INVALID_CONFIG','Invalid port or token');
  if(!existsSync(backend))throw fail('BACKEND_MISSING','Build the backend first: cargo build --release -p esp-network-backend');
  const secureOptions=cert&&key?{cert:await readFile(cert),key:await readFile(key)}:undefined;if(!!cert!==!!key)throw fail('INVALID_TLS','Supply both --cert and --key');
  const sessions=new Set();let origin,endpoint;const root=await realpath(web);
  const server=(secureOptions?https:http).createServer(secureOptions??{},async(request,response)=>{
    response.setHeader('X-Content-Type-Options','nosniff');response.setHeader('Cache-Control','no-store');
    if(request.headers.host!==new URL(origin).host){response.writeHead(403);response.end('Unexpected Host');return;}
    if(request.method==='OPTIONS'){if(origins.includes(request.headers.origin)||request.headers.origin===origin){response.setHeader('Access-Control-Allow-Origin',request.headers.origin);response.setHeader('Access-Control-Allow-Private-Network','true');response.writeHead(204);}else response.writeHead(403);response.end();return;}
    const pathname=new URL(request.url,origin).pathname;
    if(pathname==='/network-info'){if(request.headers.origin&&request.headers.origin!==origin){response.writeHead(403);response.end('Local bootstrap only');return;}response.setHeader('Content-Type','application/json');response.end(JSON.stringify({version:1,endpoint}));return;}
    if(!['GET','HEAD'].includes(request.method)){response.writeHead(405);response.end();return;}
    try{let path=decodeURIComponent(pathname);if(path==='/'||path==='/platform/')path='/platform/index.html';const file=await realpath(resolve(root,'.'+path));if(!file.startsWith(root+sep)||!(await stat(file)).isFile())throw new Error('not found');const body=await readFile(file);response.setHeader('Content-Type',types[extname(file)]??'application/octet-stream');response.setHeader('Content-Length',body.length);response.end(request.method==='HEAD'?undefined:body);}catch{response.writeHead(404);response.end('Not found');}
  });
  server.on('upgrade',(request,socket,head)=>{
    const url=new URL(request.url,origin),given=Buffer.from(url.searchParams.get('token')??''),expected=Buffer.from(token);
    if(request.method!=='GET'||!String(request.headers.connection??'').toLowerCase().split(/\s*,\s*/).includes('upgrade')||String(request.headers.upgrade??'').toLowerCase()!=='websocket'||url.pathname!=='/network'||request.headers.host!==new URL(origin).host||given.length!==expected.length||!timingSafeEqual(given,expected)||(request.headers.origin&&!origins.includes(request.headers.origin)&&request.headers.origin!==origin)||request.headers['sec-websocket-version']!=='13'||typeof request.headers['sec-websocket-key']!=='string'||Buffer.from(request.headers['sec-websocket-key'],'base64').length!==16||sessions.size>=8){socket.end('HTTP/1.1 403 Forbidden\r\nConnection: close\r\nContent-Length: 0\r\n\r\n');return;}
    const accept=createHash('sha1').update(request.headers['sec-websocket-key']+'258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64');socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
    let connection,sequence=0,closed=false;const backends=new Map(),mappings=new Map(),httpMappings=new Map();let pending=0;
    const emit=(op,data)=>{if(!closed)connection?.send(op,op===1?JSON.stringify({...data,...(data.type==='network-status'?{mappings:[...mappings.values()].map(m=>m.public)}:{})}):data);};
    const backendFor=iface=>{if(![0,1].includes(iface))throw fail('INVALID_INTERFACE','Interface must be 0 or 1');if(!backends.has(iface))backends.set(iface,new Backend(backend,iface,emit));return backends.get(iface);};
    const dropMapping=id=>{const mapping=mappings.get(id);if(!mapping)return;mapping.proxy?.close();mapping.backend.unmap(mapping.backendId);mappings.delete(id);};
    const close=()=>{if(closed)return;closed=true;for(const map of mappings.values())map.proxy?.close();for(const child of backends.values())child.close();sessions.delete(session);};
    const session={close:()=>connection.close()};sessions.add(session);
    async function call(message){const iface=message.interface??0;
      if(message.op==='interface'){const child=backendFor(iface);if(message.enabled===false){for(const [id,map]of mappings)if(map.public.interface===iface)dropMapping(id);child.close();backends.delete(iface);for(const cacheKey of httpMappings.keys())if(cacheKey.startsWith(`${iface}:`))httpMappings.delete(cacheKey);emit(1,{type:'network-status',interface:iface,enabled:false,ready:false,leaseReady:false});return {enabled:false};}if(iface===1&&message.mac&&message.mac!=='02:53:49:4d:00:03')throw fail('INVALID_MAC','Unsupported client MAC');child.write('enable');return {enabled:true};}
      if(message.op==='status')return {interfaces:[...backends].map(([interfaceId,child])=>({interface:interfaceId,...child.status})),mappings:[...mappings.values()].map(m=>m.public)};
      if(message.op==='unmap'){dropMapping(message.mappingId);return {removed:true};}
      const child=backendFor(iface);if(!child.status.ready)throw fail('NETWORK_NOT_READY','Wait for the virtual interface address and guest MAC');
      if(message.op==='http'){const guestPort=message.guestPort??80;if(!validPort(guestPort))throw fail('INVALID_PORT','Invalid HTTP port');const cacheKey=`${iface}:${guestPort}`;let mapping=httpMappings.get(cacheKey);if(!mapping){mapping=child.map('tcp',0,guestPort);httpMappings.set(cacheKey,mapping);}try{mapping=await mapping;}catch(error){httpMappings.delete(cacheKey);throw error;}return guestRequest(mapping.port,child.status.guestAddress,message);}
      if(message.op==='map'){
        const {protocol='tcp',guestPort,hostPort=0}=message;if(!['tcp','udp','http','https'].includes(protocol)||!validPort(guestPort)||!validPort(hostPort,true))throw fail('INVALID_MAPPING','Invalid mapping protocol or port');if(mappings.size>=8)throw fail('MAPPING_LIMIT','At most eight mappings per session');
        const mapped=await child.map(['http','https'].includes(protocol)?'tcp':protocol,hostPort,guestPort);let proxy;
        try{if(protocol==='http')proxy=await guestProxy(mapped,child,secureOptions);}catch(error){child.unmap(mapped.backendId);throw error;}
        if(closed){proxy?.close();throw fail('SESSION_CLOSED','Network session closed');}
        const mappingId=String(++sequence),publicMapping={mappingId,interface:iface,protocol,guestPort,guestAddress:child.status.guestAddress,host:'127.0.0.1',hostPort:mapped.port,...(proxy?{url:proxy.url}:protocol==='https'?{url:`https://127.0.0.1:${mapped.port}/`}:{})};
        mappings.set(mappingId,{backend:child,...mapped,proxy,public:publicMapping});return publicMapping;
      }
      throw fail('UNKNOWN_METHOD','Unknown network operation');
    }
    connection=attachWebSocket(socket,head,(op,body)=>{
      if(op===2){if(body.length<15||body.length>1519||![0,1].includes(body[0]))throw fail('INVALID_FRAME','Invalid Ethernet frame');backendFor(body[0]).write('frame '+body.subarray(1).toString('hex'));return;}
      const message=JSON.parse(body.toString('utf8'));if(!message||typeof message!=='object'||Array.isArray(message)||!['string','number'].includes(typeof message.id)||String(message.id).length>80)throw fail('INVALID_REQUEST','Expected a request id');
      if(pending>=16){emit(1,{id:message.id,error:{code:'TOO_MANY_REQUESTS',message:'Too many pending operations'}});return;}pending++;
      Promise.resolve().then(()=>call(message)).then(result=>emit(1,{id:message.id,result}),error=>emit(1,{id:message.id,error:{code:error.code??'NETWORK_ERROR',message:error.message}})).finally(()=>pending--);
    },close);
    emit(1,{type:'hello',version:1,interfaces:[0,1],capabilities:{ethernet:true,tcp:true,udp:true,http:true,https:true,maxFrameBytes:1518}});backendFor(0);
  });
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(port,host,resolve);});
  origin=`${secureOptions?'https':'http'}://${host==='::1'?'[::1]':host}:${server.address().port}`;endpoint=`${secureOptions?'wss':'ws'}://${new URL(origin).host}/network?token=${encodeURIComponent(token)}`;
  return {origin,endpoint,url:`${origin}/platform/#network=${encodeURIComponent(endpoint)}`,close:()=>new Promise(resolve=>{for(const session of sessions)session.close();server.close(resolve);server.closeAllConnections();})};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  const options={};const args=process.argv.slice(2);for(let i=0;i<args.length;i++){const arg=args[i];if(arg==='--help'){console.log('node tools/network-helper.mjs [--port 8787] [--backend path] [--web path] [--allow-origin https://site] [--cert cert.pem --key key.pem]');process.exit(0);}const value=args[++i];if(value===undefined)throw new Error(`Missing value for ${arg}`);if(arg==='--allow-origin')(options.origins??=['https://zhuzhe1983.github.io']).push(value);else if(['--port','--backend','--web','--cert','--key','--token'].includes(arg))options[arg.slice(2)]=arg==='--port'?Number(value):value;else throw new Error(`Unknown option ${arg}`);}
  const helper=await startNetworkHelper(options);console.log(`ESP network helper\nOpen ${helper.url}\nPages connection endpoint: ${helper.endpoint}`);for(const signal of ['SIGINT','SIGTERM'])process.on(signal,()=>void helper.close().then(()=>process.exit(0)));
}
