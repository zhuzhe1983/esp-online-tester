// HTTP/1.1 framing for the browser's isolated virtual-device client. RFC 9112 §§5–7.
// Payloads are firmware bytes; this module never substitutes a provisioning response.
const encoder=new TextEncoder(), decoder=new TextDecoder();
const MAX_BODY=2*1024*1024, MAX_HEADERS=32768, MAX_REQUEST_BODY=32768;
const fail=(code,message)=>Object.assign(new Error(message),{code});
const join=chunks=>{const out=new Uint8Array(chunks.reduce((n,c)=>n+c.length,0));let at=0;for(const c of chunks){out.set(c,at);at+=c.length;}return out;};
const crlf=bytes=>{for(let i=0;i+1<bytes.length;i++)if(bytes[i]===13&&bytes[i+1]===10)return i;return -1;};
const headerEnd=bytes=>{for(let i=0;i+3<bytes.length;i++)if(bytes[i]===13&&bytes[i+1]===10&&bytes[i+2]===13&&bytes[i+3]===10)return i;return -1;};
export function bytesToBase64(bytes) {let text='';for(let i=0;i<bytes.length;i+=8192)text+=String.fromCharCode(...bytes.subarray(i,i+8192));return btoa(text);}
function fromBase64(value) {if(typeof value!=='string'||value.length>Math.ceil(MAX_REQUEST_BODY/3)*4||!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value))throw fail('INVALID_HTTP','Invalid bounded base64 request body.');return Uint8Array.from(atob(value),c=>c.charCodeAt(0));}
export function buildGuestHttpRequest({guestPort=80,path='/',method='GET',headers={},body,bodyBase64}={},address) {
  if(!Number.isInteger(guestPort)||guestPort<1||guestPort>65535||typeof path!=='string'||path.length>4096||!path.startsWith('/')||path.startsWith('//')||/[\x00-\x20\x7f]/.test(path)||typeof method!=='string'||! /^[!#$%&'*+.^_`|~0-9A-Za-z-]{1,20}$/.test(method))throw fail('INVALID_HTTP','Specify an HTTP port, method and origin-relative path.');
  if(!headers||typeof headers!=='object'||Array.isArray(headers)||body!==undefined&&bodyBase64!==undefined)throw fail('INVALID_HTTP','Invalid HTTP headers or body.');
  const content=bodyBase64!==undefined?fromBase64(bodyBase64):body===undefined?new Uint8Array():typeof body==='string'?encoder.encode(body):body instanceof Uint8Array?body.slice():body instanceof ArrayBuffer?new Uint8Array(body.slice(0)):null;
  if(!content||content.length>MAX_REQUEST_BODY)throw fail('HTTP_REQUEST_LIMIT','HTTP request body exceeds 32 KiB.');
  const fields=[`${method.toUpperCase()} ${new URL(path,'http://guest.invalid').pathname}${new URL(path,'http://guest.invalid').search} HTTP/1.1`,`Host: ${address}${guestPort===80?'':':'+guestPort}`,'Connection: close','Accept-Encoding: gzip, deflate'];
  for(const [key,value]of Object.entries(headers)) {
    if(!/^[!#$%&'*+.^_`|~0-9A-Za-z-]+$/.test(key)||typeof value!=='string'||/[\x00-\x1f\x7f]/.test(value))throw fail('INVALID_HTTP','Invalid HTTP header.');
    if(!['host','connection','content-length','transfer-encoding','accept-encoding','upgrade','proxy-authorization','proxy-connection'].includes(key.toLowerCase()))fields.push(`${key}: ${value}`);
  }
  if(body!==undefined||bodyBase64!==undefined||['POST','PUT','PATCH'].includes(method.toUpperCase()))fields.push(`Content-Length: ${content.length}`);
  const head=encoder.encode(fields.join('\r\n')+'\r\n\r\n');if(head.length>MAX_HEADERS)throw fail('HTTP_REQUEST_LIMIT','HTTP request headers exceed 32 KiB.');
  return {bytes:join([head,content]),guestPort,method:method.toUpperCase()};
}
function parseFields(lines) {
  const headers=Object.create(null);
  for(const line of lines) {
    const colon=line.indexOf(':');if(colon<=0||! /^[!#$%&'*+.^_`|~0-9A-Za-z-]+$/.test(line.slice(0,colon))||/[\x00-\x08\x0a-\x1f\x7f]/.test(line))throw fail('HTTP_PROTOCOL','Malformed HTTP header.');
    const name=line.slice(0,colon).toLowerCase(),value=line.slice(colon+1).trim();
    if(name==='set-cookie')headers[name]=[...(headers[name]||[]),value];
    else headers[name]=headers[name]===undefined?value:headers[name]+', '+value;
  }
  return headers;
}
export class GuestHttpResponse {
  constructor({method='GET',maxBody=MAX_BODY}={}){this.method=method;this.maxBody=maxBody;this.pending=new Uint8Array();this.chunks=[];this.tail=null;this.tailUsed=0;this.bytes=0;this.wireBytes=0;this.headerBytes=0;this.complete=false;this.mode='headers';this.remaining=0;this.trailers={};this.trailerLines=[];}
  append(bytes){if(this.complete)return;this.wireBytes+=bytes.length;if(this.wireBytes>this.maxBody+1024*1024+MAX_HEADERS)throw fail('HTTP_RESPONSE_LIMIT','HTTP wire response exceeds its bounded framing allowance.');this.pending=join([this.pending,bytes]);this.drain();}
  consume(n){const value=this.pending.subarray(0,n);this.pending=this.pending.subarray(n);return value;}
  body(n){if(this.bytes+n>this.maxBody)throw fail('HTTP_RESPONSE_LIMIT','HTTP response body exceeds 2 MiB.');const bytes=this.consume(n);let at=0;while(at<n){if(!this.tail||this.tailUsed===this.tail.length){this.tail=new Uint8Array(Math.min(32768,this.maxBody-this.bytes));this.tailUsed=0;this.chunks.push(this.tail);}const count=Math.min(n-at,this.tail.length-this.tailUsed);this.tail.set(bytes.subarray(at,at+count),this.tailUsed);this.tailUsed+=count;this.bytes+=count;at+=count;}}
  drain(){
    while(!this.complete) {
      if(this.mode==='headers') {
        const end=headerEnd(this.pending);if(end<0){if(this.pending.length+this.headerBytes>MAX_HEADERS)throw fail('HTTP_RESPONSE_LIMIT','HTTP headers exceed 32 KiB.');return;}
        this.headerBytes+=end+4;if(this.headerBytes>MAX_HEADERS)throw fail('HTTP_RESPONSE_LIMIT','HTTP headers exceed 32 KiB.');
        const lines=decoder.decode(this.consume(end+4)).slice(0,-4).split('\r\n'),status=/^HTTP\/1\.[01] ([1-5][0-9]{2})(?: (.*))?$/.exec(lines.shift());
        if(!status)throw fail('HTTP_PROTOCOL','Malformed HTTP response status.');this.status=Number(status[1]);this.statusText=status[2]||'';this.headers=parseFields(lines);
        if(this.status===101)throw fail('HTTP_UPGRADE_UNSUPPORTED','Browser-local guest HTTP does not support protocol upgrades.');
        if(this.status<200)continue;
        if(this.method==='HEAD'||[204,304].includes(this.status)){this.complete=true;return;}
        const transfer=this.headers['transfer-encoding'],length=this.headers['content-length'];
        if(transfer!==undefined){if(length!==undefined||transfer.toLowerCase()!=='chunked')throw fail('HTTP_PROTOCOL','Ambiguous or unsupported HTTP transfer framing.');this.mode='chunk-size';}
        else if(length!==undefined){const lengths=length.split(',').map(s=>s.trim());if(!lengths.every(n=>/^\d+$/.test(n)&&n===lengths[0])||!Number.isSafeInteger(Number(lengths[0])))throw fail('HTTP_PROTOCOL','Invalid HTTP Content-Length.');this.remaining=Number(lengths[0]);if(this.remaining>this.maxBody)throw fail('HTTP_RESPONSE_LIMIT','HTTP response body exceeds 2 MiB.');this.mode='length';if(!this.remaining)this.complete=true;}
        else this.mode='close';
      } else if(this.mode==='length') {if(!this.pending.length)return;const n=Math.min(this.pending.length,this.remaining);this.body(n);this.remaining-=n;if(!this.remaining)this.complete=true;}
      else if(this.mode==='close'){if(this.pending.length)this.body(this.pending.length);return;}
      else if(this.mode==='chunk-size') {
        const end=crlf(this.pending);if(end<0){if(this.pending.length>1024)throw fail('HTTP_PROTOCOL','Chunk size line is too long.');return;}
        if(end>1024)throw fail('HTTP_PROTOCOL','Chunk size line is too long.');const line=decoder.decode(this.consume(end+2)).slice(0,-2),hex=line.split(';',1)[0];
        if(!/^[0-9a-fA-F]+$/.test(hex)||hex.length>8)throw fail('HTTP_PROTOCOL','Invalid HTTP chunk size.');this.remaining=parseInt(hex,16);if(this.bytes+this.remaining>this.maxBody)throw fail('HTTP_RESPONSE_LIMIT','HTTP response body exceeds 2 MiB.');this.mode=this.remaining?'chunk-data':'trailers';
      } else if(this.mode==='chunk-data'){if(!this.pending.length)return;const n=Math.min(this.pending.length,this.remaining);this.body(n);this.remaining-=n;if(!this.remaining)this.mode='chunk-crlf';}
      else if(this.mode==='chunk-crlf'){if(this.pending.length<2)return;if(this.pending[0]!==13||this.pending[1]!==10)throw fail('HTTP_PROTOCOL','Missing HTTP chunk delimiter.');this.consume(2);this.mode='chunk-size';}
      else if(this.mode==='trailers') {
        const end=crlf(this.pending);if(end<0){if(this.pending.length+this.headerBytes>MAX_HEADERS)throw fail('HTTP_RESPONSE_LIMIT','HTTP trailers exceed 32 KiB.');return;}
        this.headerBytes+=end+2;if(this.headerBytes>MAX_HEADERS)throw fail('HTTP_RESPONSE_LIMIT','HTTP trailers exceed 32 KiB.');const line=decoder.decode(this.consume(end+2)).slice(0,-2);
        if(line)this.trailerLines.push(line);else{this.trailers=parseFields(this.trailerLines);this.complete=true;}
      }
    }
  }
  end(){if(this.mode==='close')this.complete=true;if(!this.complete)throw fail('HTTP_TRUNCATED','Firmware closed before the HTTP response was complete.');}
  result(){if(!this.complete)throw fail('HTTP_TRUNCATED','HTTP response is incomplete.');return {status:this.status,statusText:this.statusText,headers:this.headers,trailers:this.trailers,bodyBytes:join(this.chunks.map((chunk,i)=>i===this.chunks.length-1?chunk.subarray(0,this.tailUsed):chunk))};}
}
export async function decodeGuestHttpResponse(response,{maxBody=MAX_BODY}={}) {
  const {bodyBytes,headers}=response;let decodedBodyBytes=bodyBytes;
  const encodings=(headers['content-encoding']||'').split(',').map(s=>s.trim().toLowerCase()).filter(s=>s&&s!=='identity');
  if(encodings.length>3)throw fail('HTTP_DECODE_ERROR','Too many HTTP content encodings.');
  for(const format of encodings.reverse()) {
    if(!['gzip','deflate'].includes(format)||typeof DecompressionStream!=='function')throw fail('HTTP_DECODE_ERROR',`Unsupported browser HTTP encoding: ${format}.`);
    let reader;try {
      reader=new Blob([decodedBodyBytes]).stream().pipeThrough(new DecompressionStream(format)).getReader();const chunks=[];let size=0;
      for(;;){const {done,value}=await reader.read();if(done)break;size+=value.length;if(size>maxBody){await reader.cancel();throw fail('HTTP_DECODE_ERROR','Decompressed HTTP response exceeds 2 MiB.');}chunks.push(value);}
      decodedBodyBytes=join(chunks);
    }catch(error){throw fail('HTTP_DECODE_ERROR',error.code==='HTTP_DECODE_ERROR'?error.message:'Firmware returned invalid compressed HTTP data.');}finally{reader?.releaseLock();}
  }
  return {...response,body:decoder.decode(decodedBodyBytes),bodyBase64:bytesToBase64(bodyBytes),decodedBodyBytes,...(encodings.length?{decodedBodyBase64:bytesToBase64(decodedBodyBytes)}:{})};
}
