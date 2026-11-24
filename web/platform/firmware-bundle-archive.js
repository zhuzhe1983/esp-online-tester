// Some manufacturers publish a complete recovery BIN inside a source/demo ZIP.
// The catalog pins both identities and an exact relative member. Read only that
// member in memory; never unpack the source tree, execute it, or guess a layout.
export const MAX_FIRMWARE_BUNDLE_BYTES=128*1024*1024;
export const MAX_FIRMWARE_BUNDLE_ENTRIES=16384;
export const MAX_FIRMWARE_BUNDLE_DECLARED_BYTES=512*1024*1024;
const MAX_DIRECTORY_BYTES=16*1024*1024;
const hashPattern=/^[a-f0-9]{64}$/i;
const fail=message=>{throw Object.assign(new Error(message),{code:'INVALID_FIRMWARE_ARCHIVE'});};
const active=signal=>{if(signal?.aborted)throw Object.assign(new Error('固件解压已取消。'),{code:'FETCH_ABORTED'});};
function safePath(name,directory=false) {
  if(typeof name!=='string'||!name.length||name.length>512||/[\\\x00-\x1f\x7f-\x9f:]/.test(name)||name.startsWith('/'))return false;
  const parts=(directory&&name.endsWith('/')?name.slice(0,-1):name).split('/');
  return parts.every(p=>p.length&&p!=='.'&&p!=='..')&&!parts.some(p=>p.trim()!==p);
}
export function validFirmwareBundle(source) {
  const a=source?.archive;
  return !!(a&&typeof a==='object'&&!Array.isArray(a)&&Object.keys(a).length===3&&a.format==='zip-bundle'
    &&hashPattern.test(a.sha256||'')&&hashPattern.test(source.sha256||'')
    &&safePath(a.entry)&&/\.bin$/i.test(a.entry)&&typeof source.url==='string'&&source.url&&!source.manifestUrl);
}
const digest=async bytes=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),b=>b.toString(16).padStart(2,'0')).join('');
export async function extractFirmwareBundle(bytes,source,{maxBytes=64*1024*1024,signal,decodeSingle}={}) {
  active(signal);
  if(!validFirmwareBundle(source)||!(bytes instanceof Uint8Array)||bytes.length<22||bytes.length>MAX_FIRMWARE_BUNDLE_BYTES||typeof decodeSingle!=='function')fail('官方 ZIP 恢复包需要固定容器、BIN 校验值和精确相对路径。');
  if(await digest(bytes)!==source.archive.sha256.toLowerCase())throw Object.assign(new Error('固件 ZIP SHA-256 不匹配。'),{code:'HASH_MISMATCH'});
  active(signal);
  const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),u16=p=>view.getUint16(p,true),u32=p=>view.getUint32(p,true);
  let end=-1;
  for(let p=bytes.length-22;p>=Math.max(0,bytes.length-65557);p--)if(u32(p)===0x06054b50&&p+22+u16(p+20)===bytes.length){end=p;break;}
  if(end<0||u16(end+4)||u16(end+6)||u16(end+8)!==u16(end+10)||u16(end+10)<1||u16(end+10)>MAX_FIRMWARE_BUNDLE_ENTRIES)fail('官方 ZIP 必须是有界的普通单卷压缩包。');
  const count=u16(end+10),cd=u32(end+16),cdSize=u32(end+12),entries=[],names=new Set();
  if(cd+cdSize!==end||cdSize>MAX_DIRECTORY_BYTES)fail('ZIP 目录范围或大小无效。');
  const decode=b=>{try{return new TextDecoder('utf-8',{fatal:true}).decode(b);}catch{fail('ZIP 文件名必须是有效 UTF-8。');}};
  let cursor=cd,total=0;
  for(let i=0;i<count;i++) {
    active(signal);
    if(cursor+46>end||u32(cursor)!==0x02014b50)fail('ZIP 目录不完整。');
    const flags=u16(cursor+8),method=u16(cursor+10),packed=u32(cursor+20),size=u32(cursor+24),nameSize=u16(cursor+28),extraSize=u16(cursor+30),commentSize=u16(cursor+32),length=46+nameSize+extraSize+commentSize,local=u32(cursor+42),type=(u32(cursor+38)>>>16)&0xf000;
    if(cursor+length>end||u16(cursor+34)||u16(cursor+6)>20||flags&~0x080e||![0,8].includes(method)||![0,0x4000,0x8000].includes(type)||packed>MAX_FIRMWARE_BUNDLE_BYTES||size>MAX_FIRMWARE_BUNDLE_BYTES)fail('ZIP 含加密、链接、ZIP64、不支持的压缩或过大的条目。');
    const name=decode(bytes.subarray(cursor+46,cursor+46+nameSize)),directory=name.endsWith('/');
    if(!safePath(name,directory)||names.has(name)||type===0x4000&&!directory||directory&&size!==0)fail('ZIP 路径重复、目录类型错误或包含不允许的路径。');
    names.add(name);total+=size;
    if(total>MAX_FIRMWARE_BUNDLE_DECLARED_BYTES||local+30>cd||u32(local)!==0x04034b50||u16(local+4)>20||u16(local+6)!==flags||u16(local+8)!==method||u16(local+26)!==nameSize)fail('ZIP 本地头、长度或声明总大小无效。');
    const localExtra=u16(local+28),start=local+30+nameSize+localExtra,finish=start+packed;
    if(start>cd||finish>cd||decode(bytes.subarray(local+30,local+30+nameSize))!==name)fail('ZIP 本地条目范围或名称不一致。');
    let tail=finish;
    if(flags&8){const d=finish+((finish+4<=cd&&u32(finish)===0x08074b50)?4:0);tail=d+12;if(tail>cd||u32(d)!==u32(cursor+16)||u32(d+4)!==packed||u32(d+8)!==size)fail('ZIP 数据描述符无效。');}
    else if(u32(local+14)!==u32(cursor+16)||u32(local+18)!==packed||u32(local+22)!==size)fail('ZIP 本地头和中央目录校验信息不一致。');
    entries.push({name,directory,local,localExtra,start,tail,central:cursor,length,nameSize,extraSize,commentSize,size});cursor+=length;
  }
  if(cursor!==end)fail('ZIP 目录长度不一致。');
  let next=0;
  for(const e of [...entries].sort((a,b)=>a.local-b.local)){if(e.local!==next)fail('ZIP 本地条目重叠或存在未声明数据。');next=e.tail;}
  if(next!==cd)fail('ZIP 数据与目录间存在未声明内容。');
  const selected=entries.find(e=>e.name===source.archive.entry);
  if(!selected||selected.directory||selected.size<1||selected.size>maxBytes)fail('ZIP 中缺少指定 BIN，或其长度超过设备 Flash。');
  // Convert only the selected headers to the established single-BIN decoder.
  // Its original compressed payload, flags, CRC and descriptor remain intact.
  // A constant filename avoids extending the legacy flat-name ZIP contract.
  const filename=new TextEncoder().encode('selected.bin'),lh=new Uint8Array(bytes.subarray(selected.local,selected.local+30)),ch=new Uint8Array(bytes.subarray(selected.central,selected.central+46));
  new DataView(lh.buffer).setUint16(26,filename.length,true);
  const cv=new DataView(ch.buffer);cv.setUint16(28,filename.length,true);cv.setUint32(42,0,true);
  const lx=bytes.subarray(selected.local+30+selected.nameSize,selected.start),payload=bytes.subarray(selected.start,selected.tail),cx=bytes.subarray(selected.central+46+selected.nameSize,selected.central+selected.length);
  const localLength=30+filename.length+lx.length+payload.length,centralLength=46+filename.length+cx.length,single=new Uint8Array(localLength+centralLength+22);
  let at=0;for(const part of [lh,filename,lx,payload,ch,filename,cx]){single.set(part,at);at+=part.length;}
  const trailer=new DataView(single.buffer,at,22);trailer.setUint32(0,0x06054b50,true);trailer.setUint16(8,1,true);trailer.setUint16(10,1,true);trailer.setUint32(12,centralLength,true);trailer.setUint32(16,localLength,true);
  const data=await decodeSingle(single,{...source,archive:{format:'zip',entry:'selected.bin'}},{maxBytes,signal});
  active(signal);
  if(await digest(data)!==source.sha256.toLowerCase())throw Object.assign(new Error('固件 BIN SHA-256 不匹配。'),{code:'HASH_MISMATCH'});
  active(signal);return data;
}
