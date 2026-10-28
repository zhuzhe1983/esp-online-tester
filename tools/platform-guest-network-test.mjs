// Independent Ethernet peer fixtures exercise the browser client without host sockets.
// Packet helpers are adapted from tools/network-helper-test.mjs (same repository).
import test from 'node:test';
import assert from 'node:assert/strict';
import {gzipSync,deflateSync} from 'node:zlib';
import {createGuestNetwork} from '../web/platform/guest-network.js';
import {buildGuestHttpRequest,GuestHttpResponse,decodeGuestHttpResponse} from '../web/platform/guest-http.js';
const CLIENT_MAC=Buffer.from([2,0x53,0x49,0x4d,0,3]),SERVER_MAC=Buffer.from([2,3,4,5,6,7]),CLIENT_IP=Buffer.from([10,23,4,27]),SERVER_IP=Buffer.from([10,23,4,1]),ALL=Buffer.alloc(4,255),BROADCAST=Buffer.alloc(6,255);
const checksum=data=>{let sum=0;for(let i=0;i<data.length;i+=2)sum+=(data[i]<<8)|(data[i+1]??0);while(sum>>>16)sum=(sum&65535)+(sum>>>16);return (~sum)&65535;};
const pseudo=(protocol,src,dst,data)=>Buffer.concat([src,dst,Buffer.from([0,protocol,data.length>>>8,data.length&255]),data]);
function packet(protocol,body,{src=SERVER_IP,dst=CLIENT_IP,smac=SERVER_MAC,dmac=CLIENT_MAC}={}){const h=Buffer.alloc(20);h[0]=0x45;h.writeUInt16BE(h.length+body.length,2);h[8]=64;h[9]=protocol;src.copy(h,12);dst.copy(h,16);h.writeUInt16BE(checksum(h),10);return Buffer.concat([dmac,smac,Buffer.from([8,0]),h,body]);}
function udp(body,{src=SERVER_IP,dst=ALL}={}){const h=Buffer.alloc(8);h.writeUInt16BE(67);h.writeUInt16BE(68,2);h.writeUInt16BE(body.length+8,4);const segment=Buffer.concat([h,body]);segment.writeUInt16BE(checksum(pseudo(17,src,dst,segment))||65535,6);return packet(17,segment,{src,dst,dmac:BROADCAST});}
function tcp({port,seq,ack,flags=0x10,data=Buffer.alloc(0),remotePort=80,window=60000,mss}){data=Buffer.from(data);const h=Buffer.alloc(mss?24:20);h.writeUInt16BE(remotePort);h.writeUInt16BE(port,2);h.writeUInt32BE(seq>>>0,4);h.writeUInt32BE(ack>>>0,8);h[12]=(h.length/4)<<4;h[13]=flags;h.writeUInt16BE(window,14);if(mss){h.set([2,4],20);h.writeUInt16BE(mss,22);}const segment=Buffer.concat([h,data]);segment.writeUInt16BE(checksum(pseudo(6,SERVER_IP,CLIENT_IP,segment)),16);return packet(6,segment);}
function parse(frame){assert.ok(frame.length>=34);const ihl=(frame[14]&15)*4,total=frame.readUInt16BE(16),ip=frame.subarray(14,14+ihl),body=frame.subarray(14+ihl,14+total);assert.equal(checksum(ip),0,'valid outgoing IPv4 checksum');const protocol=ip[9];if([6,17].includes(protocol))assert.equal(checksum(pseudo(protocol,ip.subarray(12,16),ip.subarray(16,20),body)),0,'valid outgoing transport checksum');return {frame,ip,body,protocol};}
function tcpInfo(body){return {port:body.readUInt16BE(0),remotePort:body.readUInt16BE(2),seq:body.readUInt32BE(4),ack:body.readUInt32BE(8),flags:body[13],window:body.readUInt16BE(14),data:body.subarray((body[12]>>>4)*4)};}
function dhcpReply(request,kind,{lease=3600,server=SERVER_IP,ip=CLIENT_IP}={}){const reply=Buffer.alloc(300);reply.set([2,1,6]);request.copy(reply,4,4,8);ip.copy(reply,16);CLIENT_MAC.copy(reply,28);reply.set([99,130,83,99],236);reply.set([53,1,kind,54,4,...server,51,4,lease>>>24,(lease>>>16)&255,(lease>>>8)&255,lease&255,1,4,255,255,255,0,255],240);return udp(reply);}
const response=(body,headers={})=>{body=Buffer.from(body);return Buffer.concat([Buffer.from('HTTP/1.1 200 OK\r\n'+Object.entries({'Content-Length':body.length,...headers}).map(([k,v])=>`${k}: ${v}`).join('\r\n')+'\r\n\r\n'),body]);};
class Peer {
  constructor(options={}){this.options=options;this.time=0;this.queue=[];this.sent=[];this.status=[];this.sessions=new Map();this.requests=[];this.silent=false;this.client=createGuestNetwork({autoTick:false,now:()=>this.time,sendFrame:frame=>{const b=Buffer.from(frame);this.sent.push(b);this.queue.push({from:'client',frame:b});},emit:s=>this.status.push(s)});this.client.enable();}
  send(frame){this.queue.push({from:'server',frame});}
  flush(){let n=0;while(this.queue.length){assert.ok(++n<30000,'bounded packet exchange');const {from,frame}=this.queue.shift();if(from==='server')this.client.receive(frame);else if(!this.silent)this.receive(frame);}return n;}
  ready(){this.flush();assert.equal(this.client.snapshot().address,'10.23.4.27');assert.equal(this.client.snapshot().guestAddress,'10.23.4.1');return this;}
  receive(frame){
    if(frame.readUInt16BE(12)===0x0806)return;const {protocol,body}=parse(frame);
    if(protocol===17){assert.equal(body.readUInt16BE(0),68);assert.equal(body.readUInt16BE(2),67);const request=body.subarray(8),kind=request[242];this.send(dhcpReply(request,kind===1?2:5,{lease:this.options.lease??3600}));return;}
    if(protocol!==6)return;const p=tcpInfo(body);let c=this.sessions.get(p.port);
    if(p.flags&2){if(!c){c={port:p.port,remotePort:p.remotePort,tx:0xfffffff0,rx:(p.seq+1)>>>0,request:Buffer.alloc(0),responded:false,fin:false};this.sessions.set(p.port,c);}c.syns=(c.syns||0)+1;if(c.syns===1&&this.options.dropSynAck)return;this.send(tcp({...c,seq:c.tx,ack:c.rx,flags:0x12,mss:256,window:this.options.window??60000}));if(!c.synSent){c.tx=(c.tx+1)>>>0;c.synSent=true;}return;}
    if(!c||p.flags&4)return;
    if(p.data.length){c.dataPackets=(c.dataPackets||0)+1;if(this.options.dropFirstData&&c.dataPackets===1)return;if(p.seq===c.rx){c.rx=(c.rx+p.data.length)>>>0;c.request=Buffer.concat([c.request,p.data]);}this.send(tcp({...c,seq:c.tx,ack:c.rx}));const end=c.request.indexOf('\r\n\r\n'),length=Number(/\r\nContent-Length: (\d+)/i.exec(c.request.toString())?.[1]??0);if(end>=0&&c.request.length>=end+4+length&&!c.responded){c.responded=true;this.requests.push(c.request);if(this.options.response!==false)this.respond(c,this.options.response??response('<form>firmware</form>',{'Content-Type':'text/html'}));}}
    if(p.flags&1){if(p.seq===c.rx)c.rx=(c.rx+1)>>>0;this.send(tcp({...c,seq:c.tx,ack:c.rx}));if(!c.fin)this.finish(c);}
  }
  respond(c,bytes){const packets=[];for(let offset=0;offset<bytes.length;offset+=this.options.segmentSize??233){const data=bytes.subarray(offset,offset+(this.options.segmentSize??233));packets.push(tcp({...c,seq:c.tx,ack:c.rx,flags:0x18,data}));c.tx=(c.tx+data.length)>>>0;}if(this.options.reorder&&packets.length>1)[packets[0],packets[1]]=[packets[1],packets[0]];if(this.options.duplicate&&packets.length)packets.splice(1,0,packets[0]);for(const p of packets)this.send(p);if(this.options.closeResponse!==false)this.finish(c);}
  finish(c){this.send(tcp({...c,seq:c.tx,ack:c.rx,flags:0x11}));c.tx=(c.tx+1)>>>0;c.fin=true;}
  advance(ms){this.time+=ms;this.client.tick();this.flush();}
  close(){this.client.close();this.flush();}
}

test('real DHCP discover/request/ACK learns non-default subnet and responds to firmware ARP',()=>{
  const p=new Peer().ready();try{assert.equal(p.client.snapshot().addressSource,'dhcp');assert.equal(p.client.snapshot().counters.dhcpDiscoveries,1);const arp=Buffer.alloc(28);arp.set([0,1,8,0,6,4,0,1]);SERVER_MAC.copy(arp,8);SERVER_IP.copy(arp,14);CLIENT_IP.copy(arp,24);p.client.receive(Buffer.concat([BROADCAST,SERVER_MAC,Buffer.from([8,6]),arp]));const reply=p.sent.at(-1);assert.equal(reply.readUInt16BE(20),2);assert.deepEqual(reply.subarray(22,28),CLIENT_MAC);assert.deepEqual(reply.subarray(28,32),CLIENT_IP);assert.deepEqual(reply.subarray(38,42),SERVER_IP);}finally{p.close();}
});
test('HTTP over real Ethernet/TCP wraps sequence numbers, reassembles reordered/duplicate segments, and keeps bytes',async()=>{
  const content=Buffer.from('<form action="/submit">'+('配网页'.repeat(400))+'</form>'),p=new Peer({response:response(content,{'Content-Type':'text/html'}),reorder:true,duplicate:true}).ready();try{const pending=p.client.requestHttp({guestPort:8080,path:'/index.html?mode=ap'});p.flush();const result=await pending;assert.equal(result.status,200);assert.deepEqual(Buffer.from(result.bodyBytes),content);assert.equal(result.body,content.toString());assert.match(p.requests[0].toString(),/^GET \/index.html\?mode=ap HTTP\/1\.1\r\nHost: 10\.23\.4\.1:8080/);assert.ok(p.client.snapshot().counters.tcpOutOfOrder>0);assert.ok(p.sent.some(f=>f[23]===6&&(f[47]&1)));}finally{p.close();}
});
test('gzip and deflate responses preserve original compressed bytes and decoded bytes',async()=>{
  const content=Buffer.from('<html>actual firmware response</html>');for(const [encoding,encode]of [['gzip',gzipSync],['deflate',deflateSync]]){const wire=encode(content),p=new Peer({response:response(wire,{'Content-Encoding':encoding})}).ready();try{const pending=p.client.requestHttp();p.flush();const result=await pending;assert.equal(result.body,content.toString());assert.deepEqual(Buffer.from(result.bodyBase64,'base64'),wire);assert.deepEqual(Buffer.from(result.decodedBodyBytes),content);}finally{p.close();}}
});
test('POST body is segmented to negotiated MSS; dropped SYN and request data are retransmitted',async()=>{
  const p=new Peer({dropSynAck:true,dropFirstData:true}).ready();try{const content=JSON.stringify({ssid:'network',password:'x'.repeat(1800)}),pending=p.client.requestHttp({path:'/submit',method:'POST',headers:{'Content-Type':'application/json'},body:content});p.flush();assert.equal(p.requests.length,0);p.advance(1000);assert.equal(p.requests.length,0);p.advance(1000);const result=await pending;assert.equal(result.status,200);assert.ok(p.client.snapshot().counters.tcpRetransmits>=2);assert.ok(p.sent.filter(f=>f[23]===6).every(f=>{const b=parse(f).body;return tcpInfo(b).data.length<=256;}));assert.ok(p.requests[0].toString().endsWith(content));}finally{p.close();}
});
test('chunked HTTP with trailers and close-delimited HTTP survive fragmented headers',async()=>{
  for(const wire of [Buffer.from('HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3;ext=1\r\nabc\r\n2\r\nde\r\n0\r\nX-Result: yes\r\n\r\n'),Buffer.from('HTTP/1.0 200 OK\r\n\r\nabcde')]){const p=new Peer({response:wire,segmentSize:7}).ready();try{const pending=p.client.requestHttp();p.flush();const result=await pending;assert.equal(result.body,'abcde');if(wire.includes('chunked'))assert.equal(result.trailers['x-result'],'yes');}finally{p.close();}}
});
test('invalid TCP checksum is discarded; retransmitted valid response completes',async()=>{
  const p=new Peer({response:false}).ready();try{const pending=p.client.requestHttp();p.flush();const c=[...p.sessions.values()][0],valid=tcp({...c,seq:c.tx,ack:c.rx,flags:0x18,data:response('valid')}),bad=Buffer.from(valid);bad[bad.length-1]^=1;p.client.receive(bad);assert.equal(p.client.snapshot().counters.invalidFrames,1);p.client.receive(valid);const result=await pending;assert.equal(result.body,'valid');}finally{p.close();}
});
test('HTTP timeouts, resets, AbortSignal, and disable reject outstanding requests',async()=>{
  for(const mode of ['timeout','reset','abort','close']){const p=new Peer({response:false}).ready(),controller=new AbortController();try{const pending=p.client.requestHttp({timeoutMs:100,signal:controller.signal}),expected=assert.rejects(pending,{code:{timeout:'HTTP_TIMEOUT',reset:'CONNECTION_RESET',abort:'ABORTED',close:'NETWORK_CLOSED'}[mode]});p.flush();if(mode==='timeout')p.advance(101);if(mode==='reset'){const c=[...p.sessions.values()][0];p.client.receive(tcp({...c,seq:c.tx,ack:c.rx,flags:0x14}));}if(mode==='abort')controller.abort();if(mode==='close')p.close();await expected;assert.equal(p.client.snapshot().activeRequests,0);}finally{p.close();}}
});
test('DHCP renews a short lease, and expiration invalidates a pending HTTP request',async()=>{
  const p=new Peer({lease:4,response:false}).ready();try{p.advance(2100);assert.equal(p.client.snapshot().dhcpState,'bound');assert.equal(p.client.snapshot().counters.dhcpRequests,2);const pending=p.client.requestHttp(),expected=assert.rejects(pending,{code:'NETWORK_CHANGED'});p.flush();p.silent=true;p.advance(4100);await expected;assert.equal(p.client.snapshot().ready,false);}finally{p.close();}
});
test('bounded request concurrency and input validation never send malformed packets',async()=>{
  const p=new Peer({response:false}).ready();try{await assert.rejects(p.client.requestHttp({path:'http://outside.invalid/'}),{code:'INVALID_HTTP'});await assert.rejects(p.client.requestHttp({body:'x'.repeat(32769)}),{code:'HTTP_REQUEST_LIMIT'});const pending=[];for(let i=0;i<8;i++){const req=p.client.requestHttp();pending.push(assert.rejects(req,{code:'NETWORK_CLOSED'}));}await assert.rejects(p.client.requestHttp(),{code:'HTTP_BUSY'});p.close();await Promise.all(pending);}finally{p.close();}
});
test('HTTP framing rejects ambiguous, malformed, truncated, and oversized responses',()=>{
  for(const wire of ['HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 3\r\n\r\n','HTTP/1.1 200 OK\r\nContent-Length: 3\r\nContent-Length: 4\r\n\r\n','HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nxyz\r\n','HTTP/1.1 200 OK\r\n bad: folded\r\n\r\n'])assert.throws(()=>new GuestHttpResponse().append(Buffer.from(wire)),{code:'HTTP_PROTOCOL'});
  const parser=new GuestHttpResponse();parser.append(Buffer.from('HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nshort'));assert.throws(()=>parser.end(),{code:'HTTP_TRUNCATED'});assert.throws(()=>new GuestHttpResponse().append(Buffer.from('HTTP/1.1 200 OK\r\nContent-Length: 2097153\r\n\r\n')),{code:'HTTP_RESPONSE_LIMIT'});
});
test('HTTP decompression is bounded and unsupported compression is reported',async()=>{
  const bomb=gzipSync(Buffer.alloc(2*1024*1024+1,65));await assert.rejects(decodeGuestHttpResponse({bodyBytes:bomb,headers:{'content-encoding':'gzip'}}),{code:'HTTP_DECODE_ERROR'});await assert.rejects(decodeGuestHttpResponse({bodyBytes:Buffer.from('x'),headers:{'content-encoding':'br'}}),{code:'HTTP_DECODE_ERROR'});
});
test('request headers cannot inject alternate framing, host, or proxy credentials',()=>{
  const r=buildGuestHttpRequest({path:'/a?q=中文',method:'POST',headers:{Host:'outside.invalid','Content-Length':'999','Transfer-Encoding':'chunked','Proxy-Authorization':'secret'},bodyBase64:'YWJj'},'10.23.4.1'),text=Buffer.from(r.bytes).toString();assert.match(text,/Host: 10\.23\.4\.1/);assert.match(text,/Content-Length: 3/);assert.ok(text.endsWith('\r\n\r\nabc'));assert.doesNotMatch(text,/secret|outside.invalid|chunked/);assert.throws(()=>buildGuestHttpRequest({headers:{test:'x\r\ninject: yes'}},'10.23.4.1'),{code:'INVALID_HTTP'});
});

test('partial ACK retransmits only the unacknowledged request suffix',async()=>{
  const p=new Peer({response:false}).ready();try{
    p.silent=true;const content='x'.repeat(500),pending=p.client.requestHttp({method:'POST',body:content});const expected=assert.rejects(pending,{code:'NETWORK_CLOSED'});p.queue=[];const syn=tcpInfo(parse(p.sent.at(-1)).body),start=(syn.seq+1)>>>0;
    p.client.receive(tcp({port:syn.port,seq:9000,ack:start,flags:0x12,mss:256}));let data=tcpInfo(parse(p.sent.at(-1)).body);assert.equal(data.data.length,256);const first=Buffer.from(data.data);
    p.client.receive(tcp({port:syn.port,seq:9001,ack:(start+100)>>>0}));p.advance(1000);data=tcpInfo(parse(p.sent.at(-1)).body);assert.equal(data.seq,(start+100)>>>0);assert.deepEqual(data.data,first.subarray(100));
    p.client.receive(tcp({port:syn.port,seq:9001,ack:(start+256)>>>0}));data=tcpInfo(parse(p.sent.at(-1)).body);assert.equal(data.seq,(start+256)>>>0);assert.equal(data.data.length,256);p.close();await expected;
  }finally{p.close();}
});
test('zero receive window pauses request data; a probe and window update resume it',async()=>{
  const p=new Peer({response:false}).ready();try{
    p.silent=true;const pending=p.client.requestHttp(),expected=assert.rejects(pending,{code:'NETWORK_CLOSED'});p.queue=[];const syn=tcpInfo(parse(p.sent.at(-1)).body),start=(syn.seq+1)>>>0;
    p.client.receive(tcp({port:syn.port,seq:500,ack:start,flags:0x12,window:0}));assert.equal(tcpInfo(parse(p.sent.at(-1)).body).data.length,0);p.advance(1000);const probe=tcpInfo(parse(p.sent.at(-1)).body);assert.equal(probe.seq,(start-1)>>>0);assert.equal(probe.data.length,0);
    p.client.receive(tcp({port:syn.port,seq:501,ack:start,window:1000}));assert.match(tcpInfo(parse(p.sent.at(-1)).body).data.toString(),/^GET /);p.close();await expected;
  }finally{p.close();}
});
test('bad DHCP transaction and checksum cannot configure a lease; fragmented IPv4 is rejected',()=>{
  const p=new Peer();try{
    p.silent=true;p.queue=[];const discover=parse(p.sent[0]).body.subarray(8),wrong=Buffer.from(discover);wrong[4]^=1;p.client.receive(dhcpReply(wrong,2));assert.equal(p.client.snapshot().ready,false);assert.equal(p.client.snapshot().dhcpState,'discovering');
    const invalid=dhcpReply(discover,2);invalid[invalid.length-1]^=1;p.client.receive(invalid);assert.equal(p.client.snapshot().counters.invalidFrames,1);
    const fragmented=dhcpReply(discover,2);fragmented.writeUInt16BE(0x2000,20);fragmented.writeUInt16BE(0,24);fragmented.writeUInt16BE(checksum(fragmented.subarray(14,34)),24);p.client.receive(fragmented);assert.equal(p.client.snapshot().counters.unsupportedFragments,1);assert.equal(p.client.snapshot().ready,false);
  }finally{p.close();}
});
test('HEAD, informational responses, empty bodies, and arbitrary header names are framed safely',()=>{
  for(const [method,wire,body] of [['HEAD','HTTP/1.1 200 OK\r\nContent-Length: 999\r\n\r\n',''],['GET','HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 204 No Content\r\n\r\n',''],['GET','HTTP/1.1 200 OK\r\nContent-Length: 0\r\n__proto__: firmware-value\r\n\r\n','']]){const p=new GuestHttpResponse({method});p.append(Buffer.from(wire));assert.equal(p.complete,true);assert.equal(Buffer.from(p.result().bodyBytes).toString(),body);if(wire.includes('__proto__'))assert.equal(p.result().headers.__proto__,'firmware-value');}
});
