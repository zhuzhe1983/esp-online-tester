#!/usr/bin/env node
// Catalog-pinned streaming downloads. No binary filesystem writes or disk cache.
import http from 'node:http';
import {request as httpsRequest} from 'node:https';
import {lookup as dnsLookup} from 'node:dns/promises';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {Transform} from 'node:stream';
import {pipeline} from 'node:stream/promises';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {publicIpv4} from './public-ipv4.mjs';
import {firmwareRelayTarget,firmwareRelayBase} from '../web/platform/firmware-relay.js';

const defaults={maxBytes:64*1024*1024,timeoutMs:60_000,maxConcurrent:4,maxPerIp:2,requestsPerMinute:12};
const failure=(code,status=502)=>Object.assign(new Error(code),{code,status});
const json=(res,status,code)=>{res.writeHead(status,{'Content-Type':'application/json','X-Firmware-Relay-Error':code});res.end(JSON.stringify({error:{code}}));};
export function firmwareAllowlist(catalog) {
  const entries=new Map();
  for(const item of catalog.items||[]){const source=item.source,url=firmwareRelayTarget(source?.url),hash=source?.sha256;if(!url||!/^[a-f0-9]{64}$/i.test(hash||''))continue;const hashes=entries.get(url)||new Set();hashes.add(hash.toLowerCase());entries.set(url,hashes);}
  return entries;
}
export async function loadFirmwareAllowlist() {return firmwareAllowlist(JSON.parse(await readFile(new URL('../web/platform/catalog/firmwares.json',import.meta.url),'utf8')));}
export function allowedFirmwareRedirect(value,initial) {
  try {
    const url=new URL(value,initial);
    if(url.protocol!=='https:'||url.port||url.username||url.password||url.hash||url.href.length>8192)return false;
    if(new URL(initial).hostname==='github.com')return ['release-assets.githubusercontent.com','objects.githubusercontent.com'].includes(url.hostname)&&url.pathname.startsWith('/github-production-release-asset/');
    return url.origin===new URL(initial).origin&&!!firmwareRelayTarget(url.href);
  }catch{return false;}
}
export async function openFirmwareUpstream(value,{signal,lookup=dnsLookup,request=httpsRequest}={}) {
  signal?.throwIfAborted();
  const url=new URL(value);
  let abort;
  const addresses=await Promise.race([lookup(url.hostname,{all:true,family:4}),new Promise((_resolve,reject)=>{abort=()=>reject(signal.reason);signal?.addEventListener('abort',abort,{once:true});})]).finally(()=>signal?.removeEventListener('abort',abort));signal?.throwIfAborted();
  if(!addresses.length||addresses.some(item=>item.family!==4||!publicIpv4(item.address)))throw failure('DESTINATION_DENIED');
  const address=addresses[0].address;
  return new Promise((resolve,reject)=>{
    // Pin the checked address on the TLS connection; DNS cannot change between
    // validation and connect. Hostname/SNI/certificate validation stay enabled.
    const req=request(url,{method:'GET',signal,agent:false,family:4,headers:{Accept:'application/octet-stream','Accept-Encoding':'identity','User-Agent':'esp-online-tester-firmware-relay'},lookup:(_host,options,callback)=>callback(null,options.all?[{address,family:4}]:address,4)},resolve);
    req.on('error',reject);req.end();
  });
}
export function createFirmwareRelay({allowlist,limits={},openUpstream=openFirmwareUpstream}={}) {
  if(!(allowlist instanceof Map))throw failure('INVALID_ALLOWLIST',500);
  limits={...defaults,...limits};
  if(Object.keys(limits).some(key=>!(key in defaults))||Object.values(limits).some(value=>!Number.isSafeInteger(value)||value<1))throw failure('INVALID_LIMITS',500);
  const active=new Map(),rates=new Map();
  let closed=false;
  async function handle(req,res,{origin,ip}) {
    res.setHeader('Access-Control-Allow-Origin',origin);res.setHeader('Vary','Origin');res.setHeader('Cache-Control','no-store');res.setHeader('X-Content-Type-Options','nosniff');res.setHeader('Access-Control-Expose-Headers','X-Firmware-Relay-Error');
    if(closed){json(res,503,'RELAY_CLOSED');return;}
    if(req.method==='OPTIONS'){res.writeHead(204,{'Access-Control-Allow-Methods':'GET'});res.end();return;}
    if(req.method!=='GET'){json(res,405,'METHOD_DENIED');return;}
    if(req.headers['transfer-encoding']||Number(req.headers['content-length']||0)>0){json(res,400,'BODY_DENIED');return;}
    let url,hash;
    try {
      if(req.url.length>4096)throw Error();const query=new URL(req.url,'http://relay.invalid').searchParams;
      if([...query.keys()].length!==2||query.getAll('url').length!==1||query.getAll('sha256').length!==1)throw Error();
      url=query.get('url');hash=query.get('sha256');
      if(!firmwareRelayTarget(url)||!allowlist.get(url)?.has(hash))throw Error();
    }catch{json(res,403,'SOURCE_NOT_ALLOWED');return;}
    const now=Date.now();for(const [key,rate]of rates)if(now-rate.at>=60_000)rates.delete(key);
    const rate=rates.get(ip)||{at:now,count:0};
    if(active.size>=limits.maxConcurrent||[...active.values()].filter(value=>value.ip===ip).length>=limits.maxPerIp||rate.count>=limits.requestsPerMinute||!rates.has(ip)&&rates.size>=4096){json(res,429,'RELAY_BUSY');return;}
    rate.count++;rates.set(ip,rate);
    const controller=new AbortController(),abort=()=>controller.abort();active.set(controller,{ip});
    const timeout=setTimeout(abort,limits.timeoutMs);timeout.unref();res.once('close',abort);
    let upstream;
    try {
      let current=url;
      for(let redirects=0;;redirects++){
        upstream=await openUpstream(current,{signal:controller.signal});controller.signal.throwIfAborted();
        if(![301,302,303,307,308].includes(upstream.statusCode))break;
        const next=new URL(upstream.headers.location||'',current).href;upstream.destroy();
        if(redirects>=3||!allowedFirmwareRedirect(next,url))throw failure('REDIRECT_DENIED');current=next;
      }
      if(upstream.statusCode!==200)throw failure('UPSTREAM_HTTP',upstream.statusCode>=400&&upstream.statusCode<=599?upstream.statusCode:502);
      const rawLength=upstream.headers['content-length'],length=rawLength===undefined?null:Number(rawLength);
      if(length!==null&&(!Number.isSafeInteger(length)||length<=0||length>limits.maxBytes))throw failure('FILE_TOO_LARGE',413);
      if(upstream.headers['content-encoding']&&!/^identity$/i.test(upstream.headers['content-encoding']))throw failure('ENCODING_DENIED');
      const digest=createHash('sha256');let bytes=0;
      const verify=new Transform({transform(chunk,_encoding,done){bytes+=chunk.length;if(bytes>limits.maxBytes){done(failure('FILE_TOO_LARGE',413));return;}digest.update(chunk);done(null,chunk);},flush(done){done(bytes>0&&(length===null||bytes===length)&&digest.digest('hex')===hash?null:failure('HASH_MISMATCH'));}});
      // Do not forward upstream cookies, redirects, content-disposition or HTML.
      // A completed stream must match the catalog digest; the browser rechecks it.
      res.writeHead(200,{'Content-Type':'application/octet-stream',...(length!==null?{'Content-Length':length}:{})});
      await pipeline(upstream,verify,res,{signal:controller.signal});
    }catch(error){
      if(!res.destroyed){if(res.headersSent)res.destroy();else json(res,controller.signal.aborted?504:error.status||502,controller.signal.aborted?'UPSTREAM_TIMEOUT':error.code?.match(/^[A-Z_]+$/)?error.code:'UPSTREAM_FAILED');}
    }finally{upstream?.destroy();clearTimeout(timeout);res.off('close',abort);active.delete(controller);}
  }
  return {handle,close(){closed=true;for(const controller of active.keys())controller.abort();rates.clear();}};
}
export async function startFirmwareRelay({host='127.0.0.1',port=8789,publicUrl,origins=['https://zhuzhe1983.github.io'],allowlist,limits,openUpstream}={}) {
  let base=publicUrl?firmwareRelayBase(publicUrl):null;
  if(!base&&!['127.0.0.1','::1','localhost'].includes(host)||!Number.isInteger(port)||port<0||port>65535||!Array.isArray(origins)||!origins.length||origins.some(origin=>!origin||firmwareRelayBase(origin)!==origin))throw failure('INVALID_CONFIG',500);
  const relay=createFirmwareRelay({allowlist:allowlist||await loadFirmwareAllowlist(),limits,openUpstream}),sockets=new Set();
  const server=http.createServer((req,res)=>{
    req.resume();res.setHeader('Cache-Control','no-store');
    if(req.url==='/health'&&req.method==='GET'){res.setHeader('Content-Type','application/json');res.end(JSON.stringify({ok:true,service:'esp-firmware-relay'}));return;}
    if(req.headers.host!==new URL(base).host||!origins.includes(req.headers.origin)){json(res,403,'ORIGIN_DENIED');return;}
    if(req.url.split('?')[0]!=='/firmware'){json(res,404,'NOT_FOUND');return;}
    void relay.handle(req,res,{origin:req.headers.origin,ip:req.socket.remoteAddress}).catch(()=>res.destroy());
  });
  server.headersTimeout=10_000;server.requestTimeout=10_000;server.keepAliveTimeout=5000;server.maxConnections=64;
  server.on('connection',socket=>{sockets.add(socket);socket.on('close',()=>sockets.delete(socket));});server.on('clientError',(_error,socket)=>socket.destroy());
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(port,host,resolve);});
  base??=`http://${host==='::1'?'[::1]':host}:${server.address().port}`;
  return {url:base,close:()=>new Promise(resolve=>{relay.close();server.close(resolve);for(const socket of sockets)socket.destroy();})};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  const relay=await startFirmwareRelay({host:process.env.FIRMWARE_RELAY_HOST||'127.0.0.1',port:Number(process.env.FIRMWARE_RELAY_PORT||8789),publicUrl:process.env.FIRMWARE_RELAY_PUBLIC_URL,origins:(process.env.FIRMWARE_RELAY_ORIGINS||'https://zhuzhe1983.github.io').split(',').filter(Boolean)});
  console.log(`Firmware relay listening at ${relay.url}; catalog-pinned streams only; no binary files saved.`);
  for(const signal of ['SIGINT','SIGTERM'])process.on(signal,()=>void relay.close().then(()=>process.exit(0)));
}
