// A deterministic blank FAT32 SDHC image, generated in the user's browser.
// FAT layout: Microsoft FAT32 File System Specification v1.03, BPB/FSInfo/FAT.
export const DEFAULT_SD_BYTES = 64 * 1024 * 1024;
export const MAX_SD_BYTES = 128 * 1024 * 1024;
export function validateSdImage(value) {
  const bytes=value instanceof Uint8Array?value:value instanceof ArrayBuffer?new Uint8Array(value):null;
  if(!bytes||bytes.length<1024*1024||bytes.length>MAX_SD_BYTES||bytes.length%(512*1024))throw Object.assign(new Error('SD image must be 1–128 MiB and a multiple of 512 KiB.'),{code:'INVALID_SD_IMAGE'});
  return bytes;
}
export function createFat32Image() {
  const bytes=new Uint8Array(DEFAULT_SD_BYTES),v=new DataView(bytes.buffer),enc=new TextEncoder();
  const start=2048,sectors=bytes.length/512-start,reserved=32,fat=Math.ceil((sectors-reserved+2)/130),clusters=sectors-reserved-fat*2,b=start*512;
  const text=(at,s)=>bytes.set(enc.encode(s),at),u16=(at,n)=>v.setUint16(at,n,true),u32=(at,n)=>v.setUint32(at,n,true);
  // One FAT32 LBA primary partition, beginning at the standard 1 MiB boundary.
  bytes[446+4]=0x0c;bytes.set([0xfe,0xff,0xff],447);bytes.set([0xfe,0xff,0xff],451);u32(454,start);u32(458,sectors);u16(510,0xaa55);
  bytes.set([0xeb,0x58,0x90],b);text(b+3,'ESPSIM  ');u16(b+11,512);bytes[b+13]=1;u16(b+14,reserved);bytes[b+16]=2;bytes[b+21]=0xf8;
  u16(b+24,63);u16(b+26,255);u32(b+28,start);u32(b+32,sectors);u32(b+36,fat);u32(b+44,2);u16(b+48,1);u16(b+50,6);
  bytes[b+64]=0x80;bytes[b+66]=0x29;u32(b+67,0x45535053);text(b+71,'ESP TESTER ');text(b+82,'FAT32   ');u16(b+510,0xaa55);
  const info=b+512;u32(info,0x41615252);u32(info+484,0x61417272);u32(info+488,clusters-1);u32(info+492,3);u32(info+508,0xaa550000);
  bytes.copyWithin(b+6*512,b,b+512);bytes.copyWithin(b+7*512,info,info+512);
  for(let i=0;i<2;i++){const at=(start+reserved+i*fat)*512;u32(at,0x0ffffff8);u32(at+4,0xffffffff);u32(at+8,0x0fffffff);}
  const root=(start+reserved+2*fat)*512;text(root,'ESP TESTER ');bytes[root+11]=8;
  return bytes;
}
