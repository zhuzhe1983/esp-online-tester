// Browser-local Ethernet client for a firmware SoftAP. This is a deliberately bounded
// IPv4 HTTP client, not an external-network proxy or a replacement firmware server.
// DHCP wire layout follows the existing esp-soc/src/net/helper.rs client; protocol
// references: RFC 2131 (DHCP), RFC 826 (ARP), RFC 9293 (TCP), RFC 9112 (HTTP/1.1).
import {buildGuestHttpRequest,GuestHttpResponse,decodeGuestHttpResponse} from './guest-http.js';
const DEFAULT_MAC='02:53:49:4d:00:03', BROADCAST=new Uint8Array(6).fill(255), ANY=new Uint8Array(4), ALL=new Uint8Array(4).fill(255);
const MAX_ACTIVE=8, MAX_CONNECTIONS=32, MSS=1200, RECEIVE_WINDOW=65535, MAX_REORDER=32, TIME_WAIT=2000;
const SYN=2, ACK=16, FIN=1, RST=4, PSH=8;
const fail=(code,message)=>Object.assign(new Error(message),{code});
const same=(a,b)=>a.length===b.length&&a.every((v,i)=>v===b[i]);
const ipText=a=>a?Array.from(a).join('.'):null;
const macText=a=>Array.from(a,v=>v.toString(16).padStart(2,'0')).join(':');
const diff=(a,b)=>(a-b)|0, plus=(a,n)=>(a+n)>>>0;
const u16=(a,o)=>(a[o]<<8)|a[o+1], u32=(a,o)=>((a[o]*0x1000000)+(a[o+1]<<16)+(a[o+2]<<8)+a[o+3])>>>0;
const put16=(a,o,n)=>{a[o]=n>>>8;a[o+1]=n;},put32=(a,o,n)=>{a[o]=n>>>24;a[o+1]=n>>>16;a[o+2]=n>>>8;a[o+3]=n;};
function join(parts){const out=new Uint8Array(parts.reduce((n,p)=>n+p.length,0));let pos=0;for(const p of parts){out.set(p,pos);pos+=p.length;}return out;}
function checksum(a){let n=0;for(let i=0;i<a.length;i+=2)n+=(a[i]<<8)|(a[i+1]||0);while(n>>>16)n=(n&65535)+(n>>>16);return (~n)&65535;}
function pseudo(protocol,source,destination,body){return join([source,destination,new Uint8Array([0,protocol,body.length>>>8,body.length&255]),body]);}
function parseMac(value){if(typeof value!=='string'||!/^([\da-f]{2}:){5}[\da-f]{2}$/i.test(value))throw fail('INVALID_MAC','A six-byte client MAC address is required.');const mac=Uint8Array.from(value.split(':'),n=>parseInt(n,16));if(mac[0]&1||mac.every(n=>!n))throw fail('INVALID_MAC','The client MAC must be a unicast address.');return mac;}
function random32(){const out=new Uint32Array(1);if(globalThis.crypto?.getRandomValues)globalThis.crypto.getRandomValues(out);else out[0]=Math.floor(Math.random()*0x100000000);return out[0];}
function validAddress(a){return a[0]!==0&&a[0]<224&&!same(a,ALL)&&a[0]!==127;}

// sendFrame receives an owned ArrayBuffer for virtual interface 1. enable() starts
// DHCP after the emulator has associated its virtual station with the SoftAP. HTTP
// waits for no implicit configuration: callers observe snapshot().leaseReady first.
// Scope: one DHCP-learned IPv4 firmware peer, HTTP/1.x, 8 concurrent requests;
// 32 KiB request bodies and 2 MiB encoded/decoded responses. No IP-fragment
// reassembly, TLS, WebSockets, generic port forwarding, or external networking.
export function createGuestNetwork({sendFrame,emit=()=>{},now=()=>globalThis.performance?.now()??Date.now(),autoTick=true}={}) {
  if(typeof sendFrame!=='function')throw new TypeError('sendFrame(ArrayBuffer) is required.');
  let enabled=false,mac=parseMac(DEFAULT_MAC),address=null,guestAddress=null,guestMac=null,leaseUntil=0,renewAt=0,renewing=false,dhcpState='disabled',xid=0,offered=null,server=null,dhcpLast=0,dhcpRetries=0,ipId=0,port=49152,timer=null,lastError=null,lastEmit=0;
  const connections=new Map(),counters={framesSent:0,framesReceived:0,invalidFrames:0,unsupportedFragments:0,dhcpDiscoveries:0,dhcpRequests:0,tcpRetransmits:0,tcpOutOfOrder:0,httpRequests:0,httpCompleted:0,httpFailed:0};
  const snapshot=()=>({type:'network-status',interface:1,transport:'browser',enabled,ready:!!address,leaseReady:!!address,address:ipText(address),guestAddress:ipText(guestAddress),gateway:ipText(guestAddress),addressSource:address?'dhcp':null,clientMac:macText(mac),dhcpState,activeRequests:[...connections.values()].filter(c=>!c.settled).length,counters:{...counters},lastError});
  function status(){lastEmit=now();emit(snapshot());}
  function output(frame){if(!enabled)return;counters.framesSent++;try{sendFrame(frame.buffer);}catch(error){lastError=String(error?.message||error);}}
  function ethernet(type,body,destination=guestMac){if(!destination)return;const frame=new Uint8Array(14+body.length);frame.set(destination);frame.set(mac,6);put16(frame,12,type);frame.set(body,14);output(frame);}
  function ipv4(protocol,body,source=address,destination=guestAddress,destinationMac=guestMac){if(!source||!destination)return;const packet=new Uint8Array(20+body.length);packet[0]=0x45;put16(packet,2,packet.length);put16(packet,4,++ipId);put16(packet,6,0x4000);packet[8]=64;packet[9]=protocol;packet.set(source,12);packet.set(destination,16);put16(packet,10,checksum(packet.subarray(0,20)));packet.set(body,20);ethernet(0x0800,packet,destinationMac);}
  function udp(body){const data=new Uint8Array(8+body.length);put16(data,0,68);put16(data,2,67);put16(data,4,data.length);data.set(body,8);put16(data,6,checksum(pseudo(17,ANY,ALL,data))||65535);ipv4(17,data,ANY,ALL,BROADCAST);}
  function sendDhcp(request=false){
    dhcpLast=now();dhcpRetries++;const payload=new Uint8Array(300);payload[0]=1;payload[1]=1;payload[2]=6;put32(payload,4,xid);put16(payload,10,0x8000);payload.set(mac,28);payload.set([99,130,83,99],236);
    const options=[53,1,request?3:1,61,7,1,...mac];if(request&&offered&&server)options.push(50,4,...offered,54,4,...server);options.push(55,3,1,3,6,255);payload.set(options,240);
    counters[request?'dhcpRequests':'dhcpDiscoveries']++;udp(payload);
  }
  function beginDhcp(){xid=random32();offered=null;server=null;renewing=false;dhcpRetries=0;dhcpState='discovering';sendDhcp();}
  function receiveDhcp(body,source,sourceMac){
    if(body.length<240||body[0]!==2||body[1]!==1||body[2]!==6||u32(body,4)!==xid||!same(body.subarray(28,34),mac)||!same(body.subarray(236,240),new Uint8Array([99,130,83,99])))return;
    const opts=new Map();for(let p=240;p<body.length;){const kind=body[p++];if(kind===255)break;if(!kind)continue;if(p>=body.length)return;const size=body[p++];if(p+size>body.length)return;if(!opts.has(kind))opts.set(kind,body.slice(p,p+size));p+=size;}
    const kind=opts.get(53)?.[0],next=body.slice(16,20),identity=opts.get(54)?.length===4?opts.get(54):source;
    if(!validAddress(identity))return;
    if(kind===2&&dhcpState==='discovering'&&validAddress(next)){offered=next;server=identity.slice();dhcpState='requesting';dhcpRetries=0;sendDhcp(true);status();}
    else if(kind===5&&dhcpState==='requesting'&&offered&&server&&same(next,offered)&&same(identity,server)){
      if(address&&(!same(address,next)||!same(guestAddress,identity)))for(const c of [...connections.values()])abort(c,fail('NETWORK_CHANGED','The firmware DHCP lease changed.'));
      address=next;guestAddress=identity.slice();guestMac=sourceMac.slice();const duration=opts.get(51)?.length===4?u32(opts.get(51),0):3600;const seconds=Math.max(2,Math.min(duration||3600,7*86400));leaseUntil=now()+seconds*1000;renewAt=now()+seconds*500;dhcpState='bound';renewing=false;lastError=null;status();
    }else if(kind===6&&server&&same(identity,server)){loseLease('Firmware rejected the DHCP lease.');beginDhcp();status();}
  }
  function loseLease(message){address=null;guestAddress=null;guestMac=null;leaseUntil=0;renewAt=0;for(const c of [...connections.values()])abort(c,fail('NETWORK_CHANGED',message));lastError=message;}
  function receiveArp(body,sourceMac){
    if(body.length<28||u16(body,0)!==1||u16(body,2)!==0x0800||body[4]!==6||body[5]!==4||!same(body.subarray(8,14),sourceMac)||!address)return;
    const sender=body.subarray(14,18),target=body.subarray(24,28),op=u16(body,6);if(!same(sender,guestAddress)||!same(target,address)||![1,2].includes(op))return;guestMac=sourceMac.slice();
    if(op===1){const reply=new Uint8Array(28);reply.set([0,1,8,0,6,4,0,2]);reply.set(mac,8);reply.set(address,14);reply.set(sourceMac,18);reply.set(sender,24);ethernet(0x0806,reply,sourceMac);}
  }
  function tcp(c,flags,data=new Uint8Array(),seq=c.sndNxt){
    const head=flags&SYN?24:20,packet=new Uint8Array(head+data.length);put16(packet,0,c.localPort);put16(packet,2,c.remotePort);put32(packet,4,seq);put32(packet,8,c.rcvNxt);packet[12]=(head/4)<<4;packet[13]=flags;put16(packet,14,RECEIVE_WINDOW);if(flags&SYN)packet.set([2,4,MSS>>>8,MSS&255],20);packet.set(data,head);put16(packet,16,checksum(pseudo(6,address,guestAddress,packet)));ipv4(6,packet);
  }
  function transmit(c,flags,data=new Uint8Array()){
    const seq=c.sndNxt,len=data.length+((flags&SYN)?1:0)+((flags&FIN)?1:0);c.sndNxt=plus(c.sndNxt,len);c.flight={seq,flags,data:data.slice(),end:c.sndNxt,sent:now(),rto:1000,retries:0};tcp(c,flags,data,seq);
  }
  function pump(c){
    if(c.state!=='established'||c.flight)return;
    if(!c.readComplete&&c.offset<c.request.length){const count=Math.min(c.mss,c.window,c.request.length-c.offset);if(!count){if(!c.persistAt)c.persistAt=now()+1000;return;}c.persistAt=0;const chunk=c.request.subarray(c.offset,c.offset+count);c.offset+=count;transmit(c,ACK|PSH,chunk);}
    else if(c.readComplete&&!c.finSent){c.finSent=true;c.state='closing';transmit(c,ACK|FIN);}
  }
  function settle(c,error,result){if(c.settled)return;c.settled=true;c.signal?.removeEventListener('abort',c.onAbort);if(error){lastError=error.message;counters.httpFailed++;c.reject(error);}else{counters.httpCompleted++;c.resolve(result);}status();}
  function abort(c,error,{reset=true}={}){if(reset&&address&&c.state!=='syn-sent')tcp(c,RST|ACK);connections.delete(c.localPort);settle(c,error);}
  function complete(c){
    if(c.readComplete)return;c.readComplete=true;c.closeAt=now()+15000;
    const response=c.parser.result();decodeGuestHttpResponse(response).then(result=>settle(c,null,result),error=>settle(c,error));pump(c);
  }
  function acceptData(c,sequence,data,fin){
    let ahead=diff(sequence,c.rcvNxt);
    if(ahead<0){const duplicate=-ahead;if(duplicate>=data.length){sequence=plus(sequence,data.length);data=new Uint8Array();if(diff(sequence,c.rcvNxt)!==0)fin=false;}else{data=data.subarray(duplicate);sequence=c.rcvNxt;}ahead=diff(sequence,c.rcvNxt);}
    if(ahead>0){if(ahead+data.length+(fin?1:0)<=RECEIVE_WINDOW&&c.reorder.length<MAX_REORDER&&c.reorderBytes+data.length<=RECEIVE_WINDOW&&!c.reorder.some(p=>p.seq===sequence&&p.data.length===data.length&&p.fin===fin)){c.reorder.push({seq:sequence,data:data.slice(),fin});c.reorderBytes+=data.length;counters.tcpOutOfOrder++;}tcp(c,ACK);return;}
    if(ahead<0){tcp(c,ACK);return;}
    if(data.length){c.rcvNxt=plus(c.rcvNxt,data.length);if(!c.readComplete)c.parser.append(data);}
    if(fin&&!c.finReceived){c.rcvNxt=plus(c.rcvNxt,1);c.finReceived=true;if(!c.readComplete){c.parser.end();complete(c);}c.closeAt=now()+TIME_WAIT;}
    tcp(c,ACK);if(c.parser.complete&&!c.readComplete)complete(c);
    for(;;){const i=c.reorder.findIndex(p=>diff(p.seq,c.rcvNxt)<=0);if(i<0)break;const p=c.reorder.splice(i,1)[0];c.reorderBytes-=p.data.length;acceptData(c,p.seq,p.data,p.fin);}
    if(c.finReceived&&c.finSent&&!c.flight)c.closeAt=now()+TIME_WAIT;
  }
  function receiveTcp(body){
    if(body.length<20)return;const header=(body[12]>>>4)*4;if(header<20||header>body.length||body.length-header>RECEIVE_WINDOW)return;
    const c=connections.get(u16(body,2));if(!c||u16(body,0)!==c.remotePort)return;const seq=u32(body,4),ack=u32(body,8),flags=body[13],data=body.subarray(header);
    try{
      if(c.state==='syn-sent'){
        if(flags&RST){if(flags&ACK&&ack===c.sndNxt)abort(c,fail('CONNECTION_REFUSED','Firmware refused the HTTP connection.'),{reset:false});return;}
        if((flags&(SYN|ACK))!==(SYN|ACK)||ack!==c.sndNxt||flags&FIN)return;
        for(let at=20;at<header;){const kind=body[at++];if(!kind)break;if(kind===1)continue;if(at>=header)return;const len=body[at++];if(len<2||at+len-2>header)return;if(kind===2&&len===4)c.mss=Math.max(1,Math.min(MSS,u16(body,at)));at+=len-2;}
        c.sndUna=ack;c.rcvNxt=plus(seq,1);c.window=u16(body,14);c.sndWl1=seq;c.sndWl2=ack;c.flight=null;c.state='established';tcp(c,ACK);if(data.length)acceptData(c,c.rcvNxt,data,false);pump(c);return;
      }
      if(flags&RST){if(seq===c.rcvNxt){if(c.readComplete){c.flight=null;c.closeAt=now();if(c.settled)connections.delete(c.localPort);return;}abort(c,fail('CONNECTION_RESET','Firmware reset the HTTP connection.'),{reset:false});}else tcp(c,ACK);return;}
      if(flags&SYN){tcp(c,ACK);return;}
      // RFC 9293: reject data outside the receive window before accepting its ACK.
      const span=data.length+((flags&FIN)?1:0),relative=diff(seq,c.rcvNxt);if(relative>=RECEIVE_WINDOW||relative+span<0){tcp(c,ACK);return;}
      if(!(flags&ACK))return;
      if(diff(ack,c.sndNxt)>0){tcp(c,ACK);return;}
      if(diff(ack,c.sndUna)>=0){
        if(diff(seq,c.sndWl1)>0||seq===c.sndWl1&&diff(ack,c.sndWl2)>=0){c.window=u16(body,14);c.sndWl1=seq;c.sndWl2=ack;}
        if(diff(ack,c.sndUna)>0){c.sndUna=ack;c.duplicateAcks=0;if(c.flight){const acknowledged=diff(ack,c.flight.seq);if(diff(ack,c.flight.end)>=0)c.flight=null;else if(acknowledged>0&&c.flight.data.length){c.flight.data=c.flight.data.subarray(acknowledged);c.flight.seq=ack;c.flight.sent=now();c.flight.retries=0;}}}
        else if(!data.length&&c.flight?.data.length&&++c.duplicateAcks===3){counters.tcpRetransmits++;c.flight.sent=now();tcp(c,c.flight.flags,c.flight.data,c.flight.seq);}
      }
      if(data.length||flags&FIN)acceptData(c,seq,data,!!(flags&FIN));pump(c);
    }catch(error){abort(c,error);}
  }
  function receive(input){
    if(!enabled)return;const frame=input instanceof ArrayBuffer?new Uint8Array(input):input instanceof Uint8Array?input:null;if(!frame||frame.length<14||frame.length>1518){counters.invalidFrames++;return;}
    counters.framesReceived++;const destination=frame.subarray(0,6),sourceMac=frame.subarray(6,12);if(same(sourceMac,mac)||sourceMac[0]&1||(!same(destination,mac)&&!same(destination,BROADCAST)))return;
    const type=u16(frame,12),packet=frame.subarray(14);if(type===0x0806){receiveArp(packet,sourceMac);return;}if(type!==0x0800)return;
    const header=(packet[0]&15)*4,total=u16(packet,2);if(packet.length<20||packet[0]>>>4!==4||header<20||header>packet.length||total<header||total>packet.length||checksum(packet.subarray(0,header))!==0){counters.invalidFrames++;return;}
    if(u16(packet,6)&0x3fff){counters.unsupportedFragments++;return;}
    const source=packet.subarray(12,16),destinationIp=packet.subarray(16,20),protocol=packet[9],body=packet.subarray(header,total);
    if(protocol===17&&body.length>=8&&u16(body,0)===67&&u16(body,2)===68){const len=u16(body,4);if(len<8||len>body.length||u16(body,6)!==0&&checksum(pseudo(17,source,destinationIp,body.subarray(0,len)))!==0){counters.invalidFrames++;return;}if(same(destinationIp,ALL)||address&&same(destinationIp,address)||offered&&same(destinationIp,offered)||same(destinationIp,ANY))receiveDhcp(body.subarray(8,len),source,sourceMac);}
    else if(protocol===6&&address&&same(source,guestAddress)&&same(destinationIp,address)){if(checksum(pseudo(6,source,destinationIp,body))!==0){counters.invalidFrames++;return;}receiveTcp(body);}
  }
  function tick(){
    if(!enabled)return;const time=now();
    if(address&&time>=leaseUntil){loseLease('The firmware DHCP lease expired.');beginDhcp();status();}
    else if(address&&!renewing&&time>=renewAt){renewing=true;offered=address.slice();server=guestAddress.slice();xid=random32();dhcpState='requesting';dhcpRetries=0;sendDhcp(true);}
    else if(dhcpState!=='bound'&&time-dhcpLast>=Math.min(8000,1000*2**Math.min(dhcpRetries,3)))sendDhcp(dhcpState==='requesting');
    for(const c of [...connections.values()]){
      if(!c.settled&&time>=c.deadline){abort(c,fail('HTTP_TIMEOUT','Firmware HTTP response timed out.'));continue;}
      if(c.readComplete&&c.settled&&time>=c.closeAt){connections.delete(c.localPort);continue;}
      if(c.flight&&time-c.flight.sent>=c.flight.rto){if(++c.flight.retries>8){if(c.readComplete){c.flight=null;c.closeAt=time;if(c.settled)connections.delete(c.localPort);}else abort(c,fail('TCP_TIMEOUT','Firmware did not acknowledge the TCP connection.'));continue;}counters.tcpRetransmits++;c.flight.sent=time;c.flight.rto=Math.min(8000,c.flight.rto*2);tcp(c,c.flight.flags,c.flight.data,c.flight.seq);}
      // A zero-window probe is an empty segment immediately before SND.NXT. Its
      // out-of-window sequence elicits an ACK containing the current receive window.
      else if(!c.flight&&c.state==='established'&&!c.window&&c.persistAt&&time>=c.persistAt){c.persistAt=time+1000;tcp(c,ACK,new Uint8Array(),plus(c.sndNxt,-1));}
    }
    if(time-lastEmit>=1000&&connections.size)status();
  }
  function close(){if(!enabled)return;for(const c of [...connections.values()])abort(c,fail('NETWORK_CLOSED','Browser-local firmware network was closed.'));enabled=false;address=null;guestAddress=null;guestMac=null;offered=null;server=null;dhcpState='disabled';if(timer!==null)clearInterval(timer);timer=null;status();}
  function enable(value=DEFAULT_MAC){const next=parseMac(value);if(enabled&&same(mac,next))return snapshot();if(enabled)close();mac=next;enabled=true;lastError=null;beginDhcp();if(autoTick)timer=setInterval(tick,100);status();return snapshot();}
  function requestHttp(options={}){
    if(!enabled||!address)return Promise.reject(fail('NETWORK_NOT_READY','Wait for the firmware SoftAP DHCP lease before opening its HTTP page.'));
    let request;try{request=buildGuestHttpRequest(options,ipText(guestAddress));}catch(error){return Promise.reject(error);}
    if([...connections.values()].filter(c=>!c.settled).length>=MAX_ACTIVE||connections.size>=MAX_CONNECTIONS)return Promise.reject(fail('HTTP_BUSY','The browser-local firmware HTTP request limit has been reached.'));
    const timeout=options.timeoutMs??90000;if(!Number.isFinite(timeout)||timeout<100||timeout>300000)return Promise.reject(fail('INVALID_HTTP','HTTP timeout must be between 100 ms and 300 seconds.'));
    if(options.signal?.aborted)return Promise.reject(fail('ABORTED','HTTP request was cancelled.'));
    while(connections.has(port))port=port===65535?49152:port+1;const localPort=port;port=port===65535?49152:port+1;
    return new Promise((resolve,reject)=>{
      const isn=random32(),c={localPort,remotePort:request.guestPort,state:'syn-sent',sndUna:isn,sndNxt:isn,rcvNxt:0,window:65535,mss:MSS,request:request.bytes,offset:0,parser:new GuestHttpResponse({method:request.method}),flight:null,reorder:[],reorderBytes:0,deadline:now()+timeout,resolve,reject,settled:false,readComplete:false,finSent:false,finReceived:false,closeAt:Infinity,duplicateAcks:0,persistAt:0,signal:options.signal};
      c.onAbort=()=>abort(c,fail('ABORTED','HTTP request was cancelled.'));options.signal?.addEventListener('abort',c.onAbort,{once:true});connections.set(localPort,c);counters.httpRequests++;transmit(c,SYN);status();
    });
  }
  return {enable,receive,requestHttp,snapshot,tick,close};
}
