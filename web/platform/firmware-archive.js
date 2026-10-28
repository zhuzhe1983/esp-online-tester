// Spark distributes one BIN per ZIP. SHA-256 on the source pins that BIN, not
// the container. Never choose an arbitrary file or guess a multi-image layout.
export const MAX_FIRMWARE_ZIP_BYTES = 64 * 1024 * 1024;
export const firmwareArchiveCacheKey = (source,url=source.url) => source.archive?`${url}#zip=${encodeURIComponent(source.archive.entry)}`:url;
const fail = message => { throw Object.assign(new Error(message), {code:'INVALID_FIRMWARE_ARCHIVE'}); };
export function validFirmwareArchive(source) {
  const a=source?.archive;
  return !!(a && typeof a==='object' && !Array.isArray(a) && Object.keys(a).length===2 && a.format==='zip'
    && typeof a.entry==='string' && a.entry.length<=180 && /^[A-Za-z0-9][A-Za-z0-9_. -]*\.bin$/i.test(a.entry) && !a.entry.includes('..')
    && source.url && !source.manifestUrl && /^[a-f0-9]{64}$/i.test(source.sha256||''));
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
