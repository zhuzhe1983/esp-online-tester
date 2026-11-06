// Wire formats independently implemented from the sources in flash-configuration.md.
// Credentials stay in private operation data, never receipts, logs or scene URLs.
const encoder=new TextEncoder(),decoder=new TextDecoder();
const check=(value,message)=>{if(!value)throw Object.assign(new Error(message),{code:'FLASH_CONFIG_INVALID'});};
export const configurationDefaults={uiflow1:'flow.m5stack.com',uiflow2:'uiflow2.m5stack.com'};
export function normalizeFlashConfiguration(value) {
  if(value==null)return null;
  check(value && ['uiflow1','uiflow2'].includes(value.type),'请选择 UIFlow1 或 UIFlow2 配置格式。');
  const text=(key,max,label)=>{
    check(typeof value[key]==='string' && !/[\x00-\x1f\x7f]/.test(value[key]) && encoder.encode(value[key]).length<=max,`${label}包含无效字符或超过允许长度。`);
    return value[key];
  };
  const name=text('name',120,'配置名称').trim(),server=text('server',253,'数据服务器').trim(),ssid=text('ssid',32,'Wi-Fi 名称'),password=text('password',value.type==='uiflow1'?48:64,'Wi-Fi 密码');
  check(name.length>0,'请填写配置名称。');check(ssid.length>0,'请填写 Wi-Fi 名称。');
  check(/^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/i.test(server),'数据服务器请填写主机名或 IPv4 地址，不含协议、端口、路径或账号。');
  check(!server.includes('..'),'数据服务器地址或端口无效。');
  check(password.length===0 || encoder.encode(password).length>=8,'Wi-Fi 密码至少 8 字节；开放网络可留空。');
  check(encoder.encode(password).length!==64 || /^[0-9a-f]{64}$/i.test(password),'64 字节的 Wi-Fi 密码必须是十六进制 PSK。');
  const bootOption=value.bootOption===undefined?1:value.bootOption;
  if(value.type==='uiflow2')check([1,2].includes(bootOption),'UIFlow2 启动方式须为菜单或仅连接网络。');
  return {name,type:value.type,server,ssid,password,...(value.type==='uiflow2'?{bootOption}:{})};
}
export function configurationSummary(value) {return value?{name:value.name,type:value.type}:null;}

function partitions(plan) {
  const offset=plan.receipt.partitionTableOffset,table=plan.files.find(file=>file.address<=offset && file.address+file.data.length>=offset+32);
  check(table,'固件缺少完整分区表，不能写入配置。');
  const bytes=table.data.subarray(offset-table.address,offset-table.address+4096),view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),items=[];
  for(let at=0;at+32<=bytes.length && view.getUint16(at,true)===0x50aa;at+=32)items.push({type:bytes[at+2],subtype:bytes[at+3],offset:view.getUint32(at+4,true),size:view.getUint32(at+8,true),name:decoder.decode(bytes.subarray(at+12,at+28)).replace(/\0.*$/s,''),flags:view.getUint32(at+28,true)});
  return items;
}
export function inspectFlashConfiguration(plan) {
  const table=partitions(plan),nvs=table.filter(p=>p.type===1 && p.subtype===2 && p.name==='nvs');
  const sorted=[...table].sort((a,b)=>a.offset-b.offset);
  if(sorted.some((p,i)=>p.offset%4096!==0 || p.size%4096!==0 || p.offset<plan.receipt.partitionTableOffset+4096 || p.offset+p.size>plan.receipt.flashBytes || i>0 && sorted[i-1].offset+sorted[i-1].size>p.offset))return {uiflow1:false,uiflow2:false};
  const merged=plan.receipt.format==='merged' && plan.files.length===1 && plan.files[0].address===0;
  const end=merged?plan.files[0].data.length:0;
  return {
    uiflow2:nvs.length===1 && nvs[0].offset===0x9000 && nvs[0].size===0x6000 && nvs[0].flags===0,
    uiflow1:!!(merged && end%4096===0 && end+8192<=plan.receipt.flashBytes && !table.some(p=>p.offset<end+8192 && p.offset+p.size>end)),
  };
}
export function validateFlashConfiguration(plan,profile) {
  const value=normalizeFlashConfiguration(profile);if(!value)return null;
  check(inspectFlashConfiguration(plan)[value.type],value.type==='uiflow2'?'此固件没有已支持的 UIFlow2 NVS 布局（0x9000 / 24 KiB），未修改固件。':'UIFlow1 需要完整且按 4 KiB 对齐的 BIN，末尾还须留有两个配置扇区，未修改固件。');
  return value;
}

// NVS v2, zlib.crc32(data, 0xffffffff), strings + u8 only. All entries fit one
// page; the six-page layout matches M5Stack's partition_nvs.csv generator.
function crc32(data) {let crc=0;for(const byte of data){crc^=byte;for(let bit=0;bit<8;bit++)crc=(crc>>>1)^((crc&1)?0xedb88320:0);}return (crc^0xffffffff)>>>0;}
export function encodeUIFlow2(profile) {
  const p=normalizeFlashConfiguration(profile);check(p?.type==='uiflow2','需要 UIFlow2 配置。');
  const bytes=new Uint8Array(0x6000).fill(255),view=new DataView(bytes.buffer);
  for(let page=0;page<5;page++) {
    const at=page*4096;view.setUint32(at,0xfffffffc,true);view.setUint32(at+4,page,true);bytes[at+8]=0xfe;view.setUint32(at+28,crc32(bytes.subarray(at+4,at+28)),true);
  }
  let slot=0;
  const entry=(ns,key,value)=>{
    const string=typeof value==='string',data=string?encoder.encode(value+'\0'):null,span=string?1+Math.ceil(data.length/32):1;
    check(slot+span<126,'配置超过 NVS 单页容量。');
    const at=64+slot*32;bytes.set([ns,string?0x21:1,span,255],at);bytes.fill(0,at+8,at+24);bytes.set(encoder.encode(key),at+8);
    if(string){view.setUint16(at+24,data.length,true);view.setUint32(at+28,crc32(data),true);bytes.set(data,at+32);}else bytes[at+24]=value;
    view.setUint32(at+4,crc32(new Uint8Array([...bytes.subarray(at,at+4),...bytes.subarray(at+8,at+32)])),true);
    for(let i=0;i<span;i++,slot++)bytes[32+Math.floor(slot/4)]&=~(1<<((slot%4)*2));
  };
  entry(0,'uiflow',1);
  for(const [key,value] of Object.entries({server:p.server,ssid0:p.ssid,pswd0:p.password,ssid1:'',pswd1:'',ssid2:'',pswd2:'',sntp0:'ntp.aliyun.com',sntp1:'jp.pool.ntp.org',sntp2:'pool.ntp.org',tz:'GMT0',boot_option:p.bootOption,net_mode:'WIFI',protocol:'DHCP',ip_addr:'',netmask:'255.255.255.0',gateway:'',dns:'8.8.8.8'}))entry(1,key,value);
  return bytes;
}
export function encodeUIFlow1(profile,{mac,apiKey}) {
  const p=normalizeFlashConfiguration(profile);check(p?.type==='uiflow1','需要 UIFlow1 配置。');
  check(typeof mac==='string' && /^(?:[0-9a-f]{2}:){5}[0-9a-f]{2}$/i.test(mac) && /^[0-9A-F]{8}$/.test(apiKey),'无法生成 UIFlow1 的独立设备标识。');
  const json=encoder.encode(JSON.stringify({APN:'',COMX:false,apikey:apiKey,node_id:mac.replaceAll(':','').toLowerCase(),server:p.server,start:0,'startup-beep':false,'startup-hold':false,wifi:[p.ssid,p.password]}));
  check(json.length<=4093,'UIFlow1 配置过长。');
  const config=new Uint8Array(4096).fill(255),wifi=new Uint8Array(4096).fill(255);config[0]=json.length>>8;config[1]=json.length&255;config.set(json,2);config[json.length+2]=config.subarray(0,json.length+2).reduce((sum,byte)=>sum+byte,0)&255;
  for(const [at,text] of [[0,p.ssid],[50,p.password]]) {const data=encoder.encode(text);check(data.length<=48,'UIFlow1 Wi-Fi 字段不能超过 48 字节。');wifi[at]=data.length;wifi.set(data,at+1);wifi[at+data.length+1]=data.reduce((sum,byte)=>sum+byte,0)&255;}
  return [config,wifi];
}
export function applyFlashConfiguration(plan,profile,identity) {
  const p=validateFlashConfiguration(plan,profile);if(!p)return plan;
  // Caller owns this plan copy. Never modify the pinned source or cached BIN.
  if(p.type==='uiflow1') {
    const at=plan.files[0].data.length;
    encodeUIFlow1(p,identity).forEach((data,index)=>plan.files.push({address:at+index*4096,data}));
  } else {
    const data=encodeUIFlow2(p),start=0x9000,end=0xf000,files=[];
    for(const file of plan.files) {
      const stop=file.address+file.data.length;
      if(stop<=start || file.address>=end){files.push(file);continue;}
      if(file.address<start)files.push({address:file.address,data:file.data.slice(0,start-file.address)});
      if(stop>end)files.push({address:end,data:file.data.slice(end-file.address)});
    }
    files.push({address:start,data});plan.files=files.sort((a,b)=>a.address-b.address);
  }
  plan.receipt.configuration=configurationSummary(p);
  return plan;
}
