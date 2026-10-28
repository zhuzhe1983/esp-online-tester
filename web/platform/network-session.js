import {createGuestNetwork} from './guest-network.js';
// Session-local network credentials and bounded Ethernet gateway transport.
const fail=(code,message)=>Object.assign(new Error(message),{code});
const copy=value=>structuredClone(value);
const DEFAULT_ROUTER={ssid:'esp32sim',password:'esp32sim-pass'};
export function validateNetworkOptions(options={}) {
  if(!options||typeof options!=='object'||Array.isArray(options)||Object.keys(options).some(k=>!['enabled','autoProvision','ssid','password','endpoint','mode','relayUrl','softApSsid','softApPassword'].includes(k)))throw fail('INVALID_NETWORK','Unknown network configuration field.');
  for(const k of ['enabled','autoProvision'])if(options[k]!==undefined&&typeof options[k]!=='boolean')throw fail('INVALID_NETWORK',`${k} must be boolean.`);
  for(const [k,max] of [['ssid',32],['password',64],['softApSsid',32],['softApPassword',63]])if(options[k]!==undefined&&(typeof options[k]!=='string'||new TextEncoder().encode(options[k]).length>max||/[\x00-\x1f]/.test(options[k])||(['ssid','password'].includes(k)&&options[k].includes(','))))throw fail('INVALID_NETWORK',`Invalid ${k}.`);
  if(options.softApPassword&&new TextEncoder().encode(options.softApPassword).length<8)throw fail('INVALID_NETWORK','SoftAP WPA2 passphrase must be 8–63 bytes.');
  if(options.ssid==='')throw fail('INVALID_NETWORK','Virtual router SSID must not be empty.');
  if(options.endpoint) {
    let url;try{url=new URL(options.endpoint);}catch{throw fail('INVALID_NETWORK','Network helper endpoint must be a WebSocket URL.');}
    if(!['ws:','wss:'].includes(url.protocol)||url.username||url.password||url.hash||options.endpoint.length>2048)throw fail('INVALID_NETWORK','Invalid network helper URL.');
    if(url.protocol==='ws:'&&!['127.0.0.1','localhost','[::1]'].includes(url.hostname))throw fail('INVALID_NETWORK','Remote network helpers require WSS.');
  } else if(options.endpoint!==undefined&&options.endpoint!=='')throw fail('INVALID_NETWORK','Invalid network helper endpoint.');
  if(options.mode!==undefined&&!['public-relay','local-helper'].includes(options.mode))throw fail('INVALID_NETWORK','Choose public-relay or local-helper.');
  if(options.relayUrl){let u;try{u=new URL(options.relayUrl);}catch{throw fail('INVALID_NETWORK','Invalid public relay address.');}if(u.username||u.password||u.search||u.hash||!(u.protocol==='https:'||u.protocol==='http:'&&['127.0.0.1','localhost','[::1]'].includes(u.hostname)))throw fail('INVALID_NETWORK','Public relays require HTTPS; HTTP is only for localhost previews.');}
  else if(options.relayUrl!==undefined&&options.relayUrl!=='')throw fail('INVALID_NETWORK','Invalid public relay address.');
  return {...options,...(options.endpoint!==undefined&&options.mode===undefined?{mode:'local-helper'}:{})};
}
export function networkService({interface:iface=0,protocol='tcp',guestPort,label=''}={}) {
  if(![0,1].includes(iface)||!['http','https','tcp','udp'].includes(protocol)||!Number.isInteger(guestPort)||guestPort<1||guestPort>65535||typeof label!=='string'||label.length>120)throw fail('INVALID_NETWORK_SERVICE','Specify interface 0/1, protocol and a valid guest port.');
  return {id:`${iface}:${protocol}:${guestPort}`,interface:iface,protocol,guestPort,label:label||`${protocol.toUpperCase()} ${guestPort}`,source:'manual',verified:false};
}

export function createNetworkSession({emit,command,sendFrame,WebSocketImpl=globalThis.WebSocket,fetchImpl=globalThis.fetch,guestFactory=createGuestNetwork,requestTimeout=120000}={}) {
  let config={enabled:true,autoProvision:true,...DEFAULT_ROUTER,endpoint:'',mode:'public-relay',relayUrl:'',softApSsid:'',softApPassword:''};
  let socket=null,epoch=0,sequence=0,attached=false,supported=false,softapStarted=false,autoAttempted=false;
  let guest=null,bootstrapController=null;
  let connection='relay-unconfigured',lastError=null,interfaces={},softAp=null,services=[],txFrames=0,rxFrames=0,droppedFrames=0,provisioning='idle';
  const pending=new Map(),mappingRequests=new Map();let finishConnecting=null,autoGeneration=0;
  const snapshot=()=>({enabled:config.enabled,mode:config.mode,relayConfigured:!!config.relayUrl,browserSoftAp:config.mode==='public-relay',autoProvision:config.autoProvision,router:{ssid:config.ssid,security:config.password?'wpa2-psk':'open'},helperConfigured:!!config.endpoint,connection,supported,interfaces:copy(interfaces),softAp:copy(softAp),services:copy(services),provisioning,txFrames,rxFrames,droppedFrames,lastError:copy(lastError)});
  const changed=()=>emit({type:'network',network:snapshot()});
  const report=error=>{lastError={code:error.code||'NETWORK_ERROR',message:error.message||String(error)};changed();};
  const rejectPending=()=>{for(const p of pending.values()){clearTimeout(p.timer);p.reject(fail('NETWORK_DISCONNECTED','Network helper disconnected.'));}pending.clear();};
  const request=(op,options={})=>new Promise((resolve,reject)=>{
    if(!socket||socket.readyState!==1){reject(fail('NETWORK_DISCONNECTED','Connect the network helper first.'));return;}
    if(pending.size>=16){reject(fail('NETWORK_BUSY','At most 16 network commands can be pending.'));return;}
    const id=String(++sequence),timer=setTimeout(()=>{pending.delete(id);reject(fail('NETWORK_TIMEOUT',`${op} timed out.`));},requestTimeout);
    pending.set(id,{resolve,reject,timer});socket.send(JSON.stringify({...options,id,op}));
  });
  const close=()=>{
    epoch++;autoGeneration++;bootstrapController?.abort();bootstrapController=null;mappingRequests.clear();finishConnecting?.(fail('SESSION_REPLACED','Network session replaced.'));finishConnecting=null;const old=socket;socket=null;old?.close();rejectPending();softapStarted=!!guest;interfaces=config.mode==='public-relay'&&interfaces[1]?{1:interfaces[1]}:{};services=services.filter(s=>s.source==='manual'||config.mode==='public-relay'&&s.interface===1).map(({mapping,...s})=>({...s,...(mapping?.browser?{mapping}:{}),verified:false}));
    connection=config.enabled?(config.mode==='public-relay'?(config.relayUrl?'disconnected':'relay-unconfigured'):(config.endpoint?'disconnected':'waiting-helper')):'offline';changed();
  };
  const mapPort=async(options)=>{
    const item=networkService(options),hostPort=options.hostPort??0;
    if(!Number.isInteger(hostPort)||hostPort<0||hostPort>65535)throw fail('INVALID_NETWORK_SERVICE','Invalid local port.');
    if(config.mode==='public-relay'){
      if(item.interface!==1||item.protocol!=='http')throw fail('LOCAL_HELPER_REQUIRED','This port needs the advanced local helper. Browser-only access currently supports SoftAP HTTP.');
      if(!interfaces[1]?.leaseReady)throw fail('SOFTAP_NOT_READY','Wait for the firmware SoftAP address.');
      const mapping={mappingId:`browser:${item.id}`,browser:true,interface:1,protocol:'http',guestPort:item.guestPort,guestAddress:interfaces[1].guestAddress};
      const existing=services.find(s=>s.id===item.id);services=services.filter(s=>s.id!==item.id);services.push({...item,...existing,mapping});changed();return mapping;
    }
    const existingMapping=services.find(s=>s.id===item.id)?.mapping;
    if(existingMapping){if(hostPort&&hostPort!==existingMapping.hostPort)throw fail('NETWORK_ALREADY_MAPPED','Unmap the existing service before changing its local port.');return copy(existingMapping);}
    if(mappingRequests.has(item.id)){const inFlight=mappingRequests.get(item.id);if(hostPort!==inFlight.hostPort)throw fail('NETWORK_ALREADY_MAPPED','A mapping for this service is already being created with another local port.');return inFlight.work;}
    const work=(async()=>{
    const generation=epoch;const mapping=await request('map',{interface:item.interface,protocol:item.protocol,guestPort:item.guestPort,hostPort});
    if(generation!==epoch)throw fail('SESSION_REPLACED','Network session replaced.');
    const existing=services.find(s=>s.id===item.id);services=services.filter(s=>s.id!==item.id);services.push({...item,...existing,mapping});changed();return mapping;
    })();mappingRequests.set(item.id,{work,hostPort});
    try{return await work;}finally{if(mappingRequests.get(item.id)?.work===work)mappingRequests.delete(item.id);}
  };
  const http=(options={})=>{
    if(config.mode==='public-relay'){if((options.interface??1)!==1||!guest)throw fail('SOFTAP_NOT_READY','Browser HTTP requires a connected firmware SoftAP.');return guest.requestHttp(options);}
    return request('http',options);
  };
  const provision=async({automatic=false}={})=>{
    if(!interfaces[1]?.leaseReady)throw fail('SOFTAP_NOT_READY','Wait for the virtual client to obtain an address from the firmware.');
    const generation=epoch,autoRun=autoGeneration;
    const check=()=>{if(generation!==epoch)throw fail('SESSION_REPLACED','Network session replaced.');if(automatic&&(autoRun!==autoGeneration||!config.autoProvision))throw fail('PROVISIONING_CANCELLED','Automatic provisioning was disabled.');};
    provisioning='checking';changed();
    try {
      const page=await http({interface:1,path:'/',method:'GET'});
      // The original page must advertise the exact known contract. Never submit
      // credentials to an arbitrary form merely because an HTTP server exists.
      check();
      const body=page.body||'';
      if(page.status!==200||!body.includes('/submit')||!body.includes('ssid')||!body.includes('password')||!body.includes('/scan'))throw fail('PROVISIONING_UNSUPPORTED','This firmware needs manual provisioning through its original page or a protocol-specific client.');
      services=services.map(s=>s.id==='1:http:80'?{...s,verified:true}:s);
      provisioning='submitting';changed();
      const response=await http({interface:1,path:'/submit',method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({ssid:config.ssid,password:config.password})});
      check();
      let result;try{result=JSON.parse(response.body);}catch{throw fail('PROVISIONING_FAILED','Firmware returned an unrecognized provisioning response.');}
      if(response.status!==200||result.success!==true)throw fail('PROVISIONING_FAILED',result.error||'Firmware rejected the network configuration.');
      // esp-wifi-connect saves credentials first. Its original success page
      // explicitly requests exit from config mode; saving alone is not online.
      if(body.includes('/done.html')) {
        const done=await http({interface:1,path:'/done.html',method:'GET'});
        check();
        if(done.status!==200||!done.body?.includes("fetch('/exit'")||!done.body?.includes('POST'))throw fail('PROVISIONING_UNSUPPORTED','Saved credentials; finish through the firmware success page.');
        const exited=await http({interface:1,path:'/exit',method:'POST'});
        check();
        let exitResult;try{exitResult=JSON.parse(exited.body);}catch{}
        if(exited.status!==200||exitResult?.success!==true)throw fail('PROVISIONING_FAILED','Firmware did not confirm exit from config mode.');
      }
      provisioning='submitted';changed();return {submitted:true,response:result,connected:!!interfaces[0]?.leaseReady};
    } catch(error){if(generation===epoch){if(error.code==='PROVISIONING_CANCELLED'){provisioning='idle';changed();}else{provisioning='error';report(error);}}throw error;}
  };
  const updateInterface=value=>{
    if(![0,1].includes(value.interface))return;
    interfaces[value.interface]=copy(value);
    if(value.interface===0&&value.leaseReady&&provisioning==='submitted')provisioning='connected';
    if(value.interface===1&&value.leaseReady) {
      if(!services.some(s=>s.id==='1:http:80'))services.push({...networkService({interface:1,protocol:'http',guestPort:80,label:'固件配网页'}),source:'softap-candidate'});
      tryAutoProvision();
    }
    changed();
  };
  const tryAutoProvision=()=>{if(config.autoProvision&&connection==='connected'&&interfaces[1]?.leaseReady&&!autoAttempted){autoAttempted=true;void provision({automatic:true}).catch(()=>{});}};
  const stopGuest=()=>{guest?.close();guest=null;softapStarted=false;delete interfaces[1];services=services.filter(s=>s.interface!==1);};
  const startSoftAp=()=>{
    if(config.mode==='public-relay'){
      if(!guest&&softAp?.state==='connected'){
        const client=guestFactory({sendFrame:data=>{if(guest===client){const bytes=new Uint8Array(data);sendFrame(1,bytes.slice().buffer);rxFrames++;}},emit:value=>{if(guest===client&&value.type==='network-status')updateInterface(value);}});
        guest=client;softapStarted=true;client.enable(softAp.clientMac);
      }return;
    }
    if(!softapStarted&&softAp?.state==='connected'&&socket?.readyState===1){softapStarted=true;const generation=epoch;void request('interface',{interface:1,enabled:true,mac:softAp.clientMac}).catch(error=>{if(generation===epoch){softapStarted=false;report(error);}});}
  };
  const connect=async()=>{
    close();
    autoAttempted=false;
    if(attached&&supported)await command('relay',{enabled:false});
    startSoftAp();
    if(!config.enabled||!attached||!supported||!(config.mode==='public-relay'?config.relayUrl:config.endpoint))return snapshot();
    if(!WebSocketImpl)throw fail('NETWORK_UNAVAILABLE','This environment has no WebSocket implementation.');
    const generation=epoch;connection='connecting';lastError=null;changed();
    let endpoint=config.endpoint;
    if(config.mode==='public-relay'){
      bootstrapController=new AbortController();const aborter=bootstrapController,timer=setTimeout(()=>aborter.abort(),10000);
      try{const base=config.relayUrl.replace(/\/$/,''),response=await fetchImpl(`${base}/session`,{method:'POST',signal:aborter.signal,credentials:'omit',cache:'no-store'});const body=await response.text();if(body.length>8192)throw fail('NETWORK_PROTOCOL','Relay bootstrap response too large.');const value=JSON.parse(body);if(!response.ok)throw fail(value.error?.code||'RELAY_UNAVAILABLE',value.error?.message||'Public relay is unavailable.');
        endpoint=value.endpoint;validateNetworkOptions({endpoint});const actual=new URL(endpoint),expected=new URL(base);if(actual.host!==expected.host||actual.protocol!==(expected.protocol==='https:'?'wss:':'ws:'))throw fail('NETWORK_PROTOCOL','Relay returned an endpoint outside its own origin.');
        if(generation!==epoch)throw fail('SESSION_REPLACED','Network session replaced.');
      }catch(error){if(generation===epoch){connection='disconnected';report(error);}throw error;}finally{clearTimeout(timer);if(bootstrapController===aborter)bootstrapController=null;}
    }
    const ws=new WebSocketImpl(endpoint);ws.binaryType='arraybuffer';socket=ws;
    await new Promise((resolve,reject)=>{
      let settled=false;const finish=error=>{if(settled)return;settled=true;clearTimeout(timer);if(finishConnecting===finish)finishConnecting=null;error?reject(error):resolve();};finishConnecting=finish;
      const timer=setTimeout(()=>{finish(fail('NETWORK_TIMEOUT','Network helper handshake timed out.'));ws.close();},10000);
      ws.onmessage=event=>{
        if(generation!==epoch||socket!==ws)return;
        if(event.data instanceof ArrayBuffer) {
          const bytes=new Uint8Array(event.data);
          if(bytes.length<15||bytes.length>1519||![0,1].includes(bytes[0])||config.mode==='public-relay'&&bytes[0]!==0){droppedFrames++;changed();return;}
          rxFrames++;sendFrame(bytes[0],bytes.slice(1).buffer);return;
        }
        let value;try{if(typeof event.data!=='string'||event.data.length>20*1024*1024)throw Error();value=JSON.parse(event.data);}catch{finish(fail('NETWORK_PROTOCOL','Malformed network helper response.'));ws.close();return;}
        if(value.type==='hello') {
          if(value.version!==1||config.mode==='public-relay'&&!value.capabilities?.publicRelay){finish(fail('NETWORK_PROTOCOL','Unsupported network helper version.'));ws.close();return;}
          connection='connected';changed();
          void command('relay',{enabled:true}).then(()=>{startSoftAp();tryAutoProvision();finish();}).catch(finish);
        } else if(value.type==='network-status'&&(config.mode==='local-helper'||value.interface===0))updateInterface(value);
        else if(value.id!==undefined) {const p=pending.get(String(value.id));if(p){pending.delete(String(value.id));clearTimeout(p.timer);value.error?p.reject(fail(value.error.code||'NETWORK_COMMAND_FAILED',value.error.message)):p.resolve(value.result);}}
      };
      ws.onerror=()=>{if(generation!==epoch||socket!==ws)return;const error=fail('NETWORK_CONNECTION_FAILED','Cannot connect to the network helper. Check its address and browser local-network permission.');report(error);finish(error);};
      ws.onclose=event=>{if(generation!==epoch||socket!==ws)return;socket=null;connection='disconnected';if(config.mode==='public-relay'&&event?.code===1008)lastError={code:'RELAY_LIMIT',message:`公共中继已断开：${event.reason||'会话期限或资源配额已达到上限'}。可点击连接重新尝试。`};rejectPending();interfaces=config.mode==='public-relay'&&interfaces[1]?{1:interfaces[1]}:{};services=services.map(({mapping,...s})=>({...s,...(mapping?.browser?{mapping}:{}),verified:false}));changed();void command('relay',{enabled:false}).catch(()=>{});finish(fail('NETWORK_DISCONNECTED','Network helper disconnected.'));};
    });return snapshot();
  };
  const configure=async(options={})=>{
    const next=validateNetworkOptions(options),previous=config;config={...config,...next};
    if(config.autoProvision!==previous.autoProvision){autoGeneration++;autoAttempted=false;}
    if(attached&&(config.ssid!==previous.ssid||config.password!==previous.password||config.softApSsid!==previous.softApSsid||config.softApPassword!==previous.softApPassword)) {
      config=previous;throw fail('NETWORK_RESTART_REQUIRED','Change router or SoftAP credentials before loading firmware. Existing device data is preserved.');
    }
    if(config.mode!==previous.mode){stopGuest();startSoftAp();}
    if(config.endpoint!==previous.endpoint||config.relayUrl!==previous.relayUrl||config.mode!==previous.mode||config.enabled!==previous.enabled) {
      if(!config.enabled){close();if(attached)await command('relay',{enabled:false});}
      else if(attached)await connect();
    }
    tryAutoProvision();changed();return snapshot();
  };
  const attach=async({available,manifestWifi}={})=>{
    supported=!!available;attached=true;autoAttempted=false;provisioning='idle';softAp=null;
    if(!supported){connection='unsupported';changed();return;}
    // A manifest's explicit router has priority over the default test network.
    if(manifestWifi){config.password='';for(const pair of manifestWifi.split(',')){const at=pair.indexOf('=');const k=pair.slice(0,at),v=pair.slice(at+1);if(k==='ssid')config.ssid=v;if(['psk','password','pass'].includes(k))config.password=v;}}
    await command('setup',{spec:manifestWifi||`ssid=${config.ssid}${config.password?`,psk=${config.password}`:''}`,softAp:{enabled:true,...(config.softApSsid?{ssid:config.softApSsid}:{}),...(config.softApPassword?{password:config.softApPassword}:{})}});
    if(config.enabled&&(config.mode==='public-relay'?config.relayUrl:config.endpoint)){const connecting=connect(),generation=epoch;await connecting.catch(error=>{if(generation===epoch)report(error);});}else changed();
  };
  const detach=()=>{stopGuest();attached=false;supported=false;softAp=null;autoAttempted=false;provisioning='idle';close();};
  const receive=message=>{
    if(message.networkFrame) {
      const {interface:iface,data}=message.networkFrame,bytes=new Uint8Array(data);
      if(iface===1&&config.mode==='public-relay'){guest?.receive(bytes);txFrames++;return;}
      if(socket?.readyState===1&&socket.bufferedAmount<1024*1024){const wire=new Uint8Array(bytes.length+1);wire[0]=iface;wire.set(bytes,1);socket.send(wire);txFrames++;}
      else {droppedFrames++;changed();}
    }
    if(message.softAp) {const old=softAp?.state;softAp=message.softAp;if(old==='connected'&&softAp.state!=='connected'){stopGuest();if(config.mode==='local-helper'&&socket?.readyState===1)void request('interface',{interface:1,enabled:false}).catch(report);}startSoftAp();changed();}
    if(message.networkError)report(fail('NETWORK_FRAME_REJECTED',message.networkError));
  };
  const addService=options=>{const item=networkService(options),previous=services.find(s=>s.id===item.id);services=services.filter(s=>s.id!==item.id);const saved={...item,...(previous?{mapping:previous.mapping,verified:previous.verified}:{})};services.push(saved);changed();return copy(saved);};
  const unmap=async({mappingId}={})=>{if(String(mappingId).startsWith('browser:')){services=services.map(s=>s.mapping?.mappingId===mappingId?{...s,mapping:undefined}:s);changed();return {removed:true};}const generation=epoch,result=await request('unmap',{mappingId});if(generation!==epoch)throw fail('SESSION_REPLACED','Network session replaced.');services=services.map(s=>s.mapping?.mappingId===mappingId?{...s,mapping:undefined}:s);changed();return result;};
  return {snapshot,configure,attach,detach,receive,connect,mapPort,unmap,http,provision,addService};
}
