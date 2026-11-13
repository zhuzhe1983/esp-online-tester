// Deterministic empty disk images generated entirely in the user's browser.
// FAT layout: Microsoft FAT32 File System Specification v1.03, BPB/FSInfo/FAT.
import {exfatUpcase} from './exfat-upcase.js';
export const DEFAULT_SD_BYTES = 64 * 1024 * 1024;
export const MAX_SD_BYTES = 128 * 1024 * 1024;
export const SD_FORMATS = Object.freeze(['fat32','exfat']);
export const SD_CAPACITIES = Object.freeze([64,128]);
const invalid = message => Object.assign(new Error(message),{code:'INVALID_SD_IMAGE'});
export function validateSdImage(value) {
  const bytes=value instanceof Uint8Array?value:value instanceof ArrayBuffer?new Uint8Array(value):null;
  if(!bytes||bytes.length<1024*1024||bytes.length>MAX_SD_BYTES||bytes.length%(512*1024))throw Object.assign(new Error('SD image must be 1–128 MiB and a multiple of 512 KiB.'),{code:'INVALID_SD_IMAGE'});
  return bytes;
}
function blankBytes(byteLength) {
  if(!SD_CAPACITIES.includes(byteLength/(1024*1024)))throw invalid('Empty cards must be 64 or 128 MiB.');
  return new Uint8Array(byteLength);
}
export function createFat32Image(byteLength=DEFAULT_SD_BYTES) {
  const bytes=blankBytes(byteLength),v=new DataView(bytes.buffer),enc=new TextEncoder();
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

// exFAT 1.00: §§3–7 of Microsoft's public file system specification.
// One MBR partition, 512-byte sectors, 4 KiB clusters, one FAT and bitmap.
// The recommended full Unicode table is fixed data, not host locale casing.
export function createExfatImage(byteLength=DEFAULT_SD_BYTES) {
  const bytes=blankBytes(byteLength),v=new DataView(bytes.buffer),enc=new TextEncoder();
  const u16=(at,n)=>v.setUint16(at,n,true),u32=(at,n)=>v.setUint32(at,n,true),u64=(at,n)=>v.setBigUint64(at,BigInt(n),true);
  const start=2048,sectors=bytes.length/512-start,b=start*512,spc=8,fatOffset=128;
  const fatLength=Math.ceil((Math.floor((sectors-fatOffset)/spc)+2)*4/512);
  const heap=Math.ceil((fatOffset+fatLength)/spc)*spc,clusters=Math.floor((sectors-heap)/spc);
  const upcase=exfatUpcase(),bitmapLength=Math.ceil(clusters/8),bitmapClusters=Math.ceil(bitmapLength/4096),upcaseClusters=Math.ceil(upcase.length/4096);
  const bitmap=2,upcaseFirst=bitmap+bitmapClusters,root=upcaseFirst+upcaseClusters,used=bitmapClusters+upcaseClusters+1;
  const clusterOffset=cluster=>(start+heap+(cluster-2)*spc)*512;
  bytes[450]=0x07;bytes.set([0xfe,0xff,0xff],447);bytes.set([0xfe,0xff,0xff],451);u32(454,start);u32(458,sectors);u16(510,0xaa55);
  bytes.set([0xeb,0x76,0x90],b);bytes.set(enc.encode('EXFAT   '),b+3);
  u64(b+64,start);u64(b+72,sectors);u32(b+80,fatOffset);u32(b+84,fatLength);u32(b+88,heap);u32(b+92,clusters);u32(b+96,root);u32(b+100,0x45535053);u16(b+104,0x100);
  bytes[b+108]=9;bytes[b+109]=3;bytes[b+110]=1;bytes[b+111]=0x80;bytes[b+112]=Math.floor(used*100/clusters);
  bytes.fill(0xf4,b+120,b+510);u16(b+510,0xaa55);
  for(let sector=1;sector<=8;sector++)u32(b+sector*512+508,0xaa550000);
  let checksum=0;for(let i=0;i<11*512;i++)if(i!==106&&i!==107&&i!==112)checksum=(((checksum&1)*0x80000000)+(checksum>>>1)+bytes[b+i])>>>0;
  for(let i=0;i<512;i+=4)u32(b+11*512+i,checksum);
  bytes.copyWithin(b+12*512,b,b+12*512);
  const fat=b+fatOffset*512;u32(fat,0xfffffff8);u32(fat+4,0xffffffff);
  for(const [first,count] of [[bitmap,bitmapClusters],[upcaseFirst,upcaseClusters],[root,1]])for(let i=0;i<count;i++)u32(fat+(first+i)*4,i===count-1?0xffffffff:first+i+1);
  const allocation=clusterOffset(bitmap);for(let i=0;i<used;i++)bytes[allocation+(i>>3)]|=1<<(i&7);
  bytes.set(upcase,clusterOffset(upcaseFirst));
  const directory=clusterOffset(root);bytes[directory]=0x83;bytes[directory+1]=10;
  for(let i=0;i<10;i++)u16(directory+2+i*2,'ESP TESTER'.charCodeAt(i));
  const bitmapEntry=directory+32;bytes[bitmapEntry]=0x81;u32(bitmapEntry+20,bitmap);u64(bitmapEntry+24,bitmapLength);
  const upcaseEntry=directory+64;bytes[upcaseEntry]=0x82;u32(upcaseEntry+4,0xe619d30d);u32(upcaseEntry+20,upcaseFirst);u64(upcaseEntry+24,upcase.length);
  return bytes;
}
export function createSdImage({format='fat32',byteLength=DEFAULT_SD_BYTES}={}) {
  if(!SD_FORMATS.includes(format))throw invalid('Choose FAT32 or exFAT for an empty TF card.');
  return format==='exfat'?createExfatImage(byteLength):createFat32Image(byteLength);
}
// Identify a boot-sector signature only; preserving an import never promises
// the filesystem is intact or supported by the selected firmware.
export function sdImageFormat(value) {
  const bytes=validateSdImage(value),view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);
  const starts=[0];if(view.getUint16(510,true)===0xaa55){const lba=view.getUint32(454,true);if(lba>0&&lba*512+512<=bytes.length)starts.push(lba*512);}
  const text=(at,length)=>String.fromCharCode(...bytes.subarray(at,at+length));
  for(const at of starts)if(view.getUint16(at+510,true)===0xaa55){if(text(at+3,8)==='EXFAT   ')return 'exfat';if(text(at+82,8)==='FAT32   ')return 'fat32';}
  return 'image';
}
