// Spark distributes one BIN per ZIP. SHA-256 on the source pins that BIN, not
// the container. Never choose an arbitrary file or guess a multi-image layout.
export const MAX_FIRMWARE_ZIP_BYTES = 64 * 1024 * 1024;
export const MAX_FIRMWARE_ZIP_ENTRIES = 64;
export const MAX_FIRMWARE_ZIP_DECLARED_BYTES = 128 * 1024 * 1024;
export const firmwareArchiveCacheKey = (source,url=source.url) => source.archive?`${url}#zip=${encodeURIComponent(source.archive.entry)}`:url;
const fail = message => { throw Object.assign(new Error(message), {code:'INVALID_FIRMWARE_ARCHIVE'}); };
export function validFirmwareArchive(source) {
  const a=source?.archive;
  return !!(a && typeof a==='object' && !Array.isArray(a) && Object.keys(a).length===2 && a.format==='zip'
    && typeof a.entry==='string' && a.entry.length<=180 && /^[A-Za-z0-9][A-Za-z0-9_. -]*\.bin$/i.test(a.entry) && !a.entry.includes('..')
    && source.url && !source.manifestUrl && /^[a-f0-9]{64}$/i.test(source.sha256||''));
}
// Manufacturer recovery packages may contain several explicit flash segments.
// This form is accepted only inside a pinned manifest, never as a guessed merged
// image. Pin the container AND the selected BIN; no files are written to disk.
export function validFirmwareZipEntry(source) {
  const a=source?.archive;
  return !!(a && Object.keys(a).length===3 && a.format==='zip-entry' && /^[a-f0-9]{64}$/i.test(a.sha256||'')
    && validFirmwareArchive({...source,archive:{format:'zip',entry:a.entry}}));
}
export async function extractFirmwareZipEntry(bytes,source,options={}) {
  const active=()=>{if(options.signal?.aborted)throw Object.assign(new Error('固件解压已取消。'),{code:'FETCH_ABORTED'});};active();
  if(!validFirmwareZipEntry(source)||!(bytes instanceof Uint8Array)||bytes.length<22||bytes.length>MAX_FIRMWARE_ZIP_BYTES)fail('分段 ZIP 必须固定压缩包、BIN 校验值及精确文件名。');
  const digest=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),b=>b.toString(16).padStart(2,'0')).join('');active();
  if(digest!==source.archive.sha256.toLowerCase())throw Object.assign(new Error('固件 ZIP SHA-256 不匹配。'),{code:'HASH_MISMATCH'});
  const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),u16=p=>view.getUint16(p,true),u32=p=>view.getUint32(p,true);
  let end=-1;
  for(let p=bytes.length-22;p>=Math.max(0,bytes.length-65557);p--)if(u32(p)===0x06054b50&&p+22+u16(p+20)===bytes.length){end=p;break;}
  if(end<0||u16(end+4)||u16(end+6)||u16(end+8)!==u16(end+10)||u16(end+10)<1||u16(end+10)>MAX_FIRMWARE_ZIP_ENTRIES)fail('分段 ZIP 必须是至多 64 个条目的普通单卷压缩包。');
  const count=u16(end+10),cd=u32(end+16),cdSize=u32(end+12),entries=[],names=new Set();let cursor=cd,total=0;
  if(cd+cdSize!==end)fail('ZIP 目录范围无效。');
  const decode=b=>new TextDecoder('utf-8',{fatal:true}).decode(b);
  for(let i=0;i<count;i++) {
    active();if(cursor+46>end||u32(cursor)!==0x02014b50)fail('ZIP 目录不完整。');
    const flags=u16(cursor+8),method=u16(cursor+10),packed=u32(cursor+20),size=u32(cursor+24),nameSize=u16(cursor+28),length=46+nameSize+u16(cursor+30)+u16(cursor+32),local=u32(cursor+42),type=(u32(cursor+38)>>>16)&0xf000;
    if(cursor+length>end||u16(cursor+34)||u16(cursor+6)>20||flags&~0x080e||![0,8].includes(method)||type&&type!==0x8000||packed>MAX_FIRMWARE_ZIP_BYTES||size>MAX_FIRMWARE_ZIP_BYTES)fail('ZIP 含不支持的条目、加密、ZIP64 或过大的文件。');
    const name=decode(bytes.subarray(cursor+46,cursor+46+nameSize));
    if(name.length>180||!/^[A-Za-z0-9][A-Za-z0-9_. -]*$/.test(name)||name.includes('..')||names.has(name))fail('ZIP 条目名称重复或包含不允许的路径。');names.add(name);total+=size;
    if(total>MAX_FIRMWARE_ZIP_DECLARED_BYTES||local+30>cd||u32(local)!==0x04034b50||u16(local+6)!==flags||u16(local+8)!==method||u16(local+26)!==nameSize)fail('ZIP 本地头、长度或总大小无效。');
    const start=local+30+nameSize+u16(local+28),finish=start+packed;
    if(start>cd||finish>cd||decode(bytes.subarray(local+30,local+30+nameSize))!==name)fail('ZIP 本地条目范围或名称不一致。');
    let tail=finish;
    if(flags&8){const descriptor=finish+((finish+4<=cd&&u32(finish)===0x08074b50)?4:0);tail=descriptor+12;if(tail>cd||u32(descriptor)!==u32(cursor+16)||u32(descriptor+4)!==packed||u32(descriptor+8)!==size)fail('ZIP 数据描述符无效。');}
    else if(u32(local+14)!==u32(cursor+16)||u32(local+18)!==packed||u32(local+22)!==size)fail('ZIP 条目校验信息不一致。');
    entries.push({name,local,tail,central:cursor,length});cursor+=length;
  }
  if(cursor!==end)fail('ZIP 目录长度不一致。');
  let next=0;for(const e of [...entries].sort((a,b)=>a.local-b.local)){if(e.local!==next)fail('ZIP 本地条目重叠或存在未声明数据。');next=e.tail;}if(next!==cd)fail('ZIP 数据和目录间存在未声明内容。');
  const selected=entries.find(e=>e.name===source.archive.entry);if(!selected)fail('ZIP 中找不到清单指定的 BIN。');
  // Reuse the original single-BIN decoder for bounded inflate, cancellation and
  // CRC. Only headers are normalized; payload and all checksums are unchanged.
  const local=bytes.subarray(selected.local,selected.tail),central=new Uint8Array(bytes.subarray(selected.central,selected.central+selected.length)),trailer=new Uint8Array(22);
  new DataView(central.buffer).setUint32(42,0,true);const v=new DataView(trailer.buffer);v.setUint32(0,0x06054b50,true);v.setUint16(8,1,true);v.setUint16(10,1,true);v.setUint32(12,central.length,true);v.setUint32(16,local.length,true);
  const single=new Uint8Array(local.length+central.length+22);single.set(local);single.set(central,local.length);single.set(trailer,local.length+central.length);
  return extractFirmwareZip(single,{...source,archive:{format:'zip',entry:source.archive.entry}},options);
}
const crcTable=Uint32Array.from({length:256},(_,n)=>{for(let i=0;i<8;i++)n=n&1?0xedb88320^(n>>>1):n>>>1;return n>>>0;});
function crc32(bytes) {let crc=0xffffffff;for(const b of bytes)crc=crcTable[(crc^b)&255]^(crc>>>8);return (crc^0xffffffff)>>>0;}
export async function extractFirmwareZip(bytes, source, {maxBytes=MAX_FIRMWARE_ZIP_BYTES,signal}={}) {
  const active=()=>{if(signal?.aborted)throw Object.assign(new Error('固件解压已取消。'),{code:'FETCH_ABORTED'});};
  active();
  if(!validFirmwareArchive(source)||!(bytes instanceof Uint8Array)||bytes.length<22||bytes.length>MAX_FIRMWARE_ZIP_BYTES)fail('固件压缩包或目标 BIN 信息无效。');
  const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),u16=p=>view.getUint16(p,true),u32=p=>view.getUint32(p,true);
  let end=-1;
  for(let p=bytes.length-22;p>=Math.max(0,bytes.length-65557);p--)if(u32(p)===0x06054b50&&p+22+u16(p+20)===bytes.length){end=p;break;}
  if(end<0||u16(end+4)||u16(end+6)||u16(end+8)!==1||u16(end+10)!==1)fail('仅支持包含一个 BIN 的普通 ZIP；多文件、分卷或 ZIP64 包请使用烧录清单。');
  const cd=u32(end+16),cdSize=u32(end+12);
  if(cd+cdSize!==end||cdSize<46||cd+46>end||u32(cd)!==0x02014b50||u32(cd+42)!==0||u16(cd+34))fail('ZIP 目录或文件偏移无效。');
  const flags=u16(cd+8),method=u16(cd+10),crc=u32(cd+16),packed=u32(cd+20),size=u32(cd+24),nameSize=u16(cd+28);
  if(flags&~0x080e||![0,8].includes(method)||u16(cd+6)>20||[packed,size].includes(0xffffffff))fail('不支持加密或此压缩格式的固件 ZIP。');
  if(size<1||size>maxBytes||packed<1||packed>MAX_FIRMWARE_ZIP_BYTES)fail('ZIP 中的固件超过设备或下载大小限制。');
  if(46+nameSize+u16(cd+30)+u16(cd+32)!==cdSize)fail('ZIP 目录长度无效。');
  const fileType=(u32(cd+38)>>>16)&0xf000;
  if(fileType&&fileType!==0x8000)fail('ZIP 条目必须是普通 BIN 文件。');
  const name=new TextDecoder('utf-8',{fatal:true}).decode(bytes.subarray(cd+46,cd+46+nameSize));
  if(name!==source.archive.entry)fail('ZIP 内文件名与所选固件不符。');
  if(cd<30||u32(0)!==0x04034b50||u16(6)!==flags||u16(8)!==method||u16(26)!==nameSize)fail('ZIP 本地头与目录不一致。');
  const start=30+u16(26)+u16(28),finish=start+packed;
  if(start>cd||finish>cd||new TextDecoder('utf-8',{fatal:true}).decode(bytes.subarray(30,30+nameSize))!==name)fail('ZIP 文件范围或名称不一致。');
  if(flags&8){
    const descriptor=finish+((finish+4<=cd&&u32(finish)===0x08074b50)?4:0);
    if(descriptor+12!==cd||u32(descriptor)!==crc||u32(descriptor+4)!==packed||u32(descriptor+8)!==size)fail('ZIP 数据描述符无效。');
  }else if(finish!==cd||u32(14)!==crc||u32(18)!==packed||u32(22)!==size)fail('ZIP 文件长度或校验信息不一致。');
  let data;
  if(method===0){if(packed!==size)fail('ZIP 未压缩条目长度无效。');data=bytes.slice(start,finish);}
  else {
    let decompressor;
    try{decompressor=new DecompressionStream('deflate-raw');}catch{fail('浏览器不支持 ZIP 解压，请更新浏览器或导入解压后的 BIN。');}
    const reader=new Blob([bytes.subarray(start,finish)]).stream().pipeThrough(decompressor).getReader();
    const cancel=()=>{void reader.cancel().catch(()=>{});};signal?.addEventListener('abort',cancel,{once:true});
    data=new Uint8Array(size);let written=0;
    try {
      for(;;){active();const {done,value}=await reader.read();active();if(done)break;if(written+value.length>size)fail('ZIP 解压数据超过声明长度。');data.set(value,written);written+=value.length;}
      if(written!==size)fail('ZIP 解压数据不完整。');
    }finally{signal?.removeEventListener('abort',cancel);await reader.cancel().catch(()=>{});reader.releaseLock();}
  }
  active();if(crc32(data)!==crc)fail('ZIP 数据 CRC 校验失败。');return data;
}
