import test from 'node:test';
import assert from 'node:assert/strict';
import {createFat32Image,createExfatImage,createSdImage,sdImageFormat,validateSdImage} from '../web/platform/sd-card-image.js';

for(const mib of [64,128])test(`empty ${mib} MiB exFAT contains independently readable boot regions, allocation and Unicode metadata`,()=>{
 const bytes=createExfatImage(mib*1024*1024),view=new DataView(bytes.buffer),u32=at=>view.getUint32(at,true),u64=at=>Number(view.getBigUint64(at,true));
 const start=u32(454)*512,volumeSectors=u32(458),fat=start+u32(start+80)*512,heap=start+u32(start+88)*512,clusterBytes=512*(1<<bytes[start+109]),count=u32(start+92);
 assert.equal(bytes.length,mib*1024*1024);assert.equal(start+volumeSectors*512,bytes.length);assert.equal(bytes[450],7);assert.equal(u64(start+64)*512,start);assert.equal(u64(start+72),volumeSectors);
 assert.equal(bytes[start+108],9);assert.equal(bytes[start+110],1);assert.equal(view.getUint16(start+104,true),0x100);assert.ok(count>0);assert.ok((count+2)*4<=u32(start+84)*512);assert.ok(heap+count*clusterBytes<=bytes.length);
 assert.deepEqual(bytes.subarray(start,start+12*512),bytes.subarray(start+12*512,start+24*512));
 let bootChecksum=0;for(let offset=0;offset<11*512;offset++)if(![106,107,112].includes(offset))bootChecksum=((bootChecksum>>>1)|(bootChecksum<<31))+bytes[start+offset]>>>0;
 for(let offset=0;offset<512;offset+=4)assert.equal(u32(start+11*512+offset),bootChecksum);
 const cluster=number=>heap+(number-2)*clusterBytes,rootCluster=u32(start+96),root=cluster(rootCluster);
 assert.equal(bytes[root],0x83);assert.equal(bytes[root+32],0x81);assert.equal(bytes[root+64],0x82);assert.equal(bytes[root+96],0,'No file entries exist in the new root');
 const bitmap=cluster(u32(root+32+20)),bitmapBytes=u64(root+32+24),upcase=cluster(u32(root+64+20)),upcaseBytes=u64(root+64+24);
 assert.equal(bitmapBytes,Math.ceil(count/8));assert.equal(upcaseBytes,5836);
 let used=0;for(let index=0;index<count;index++)used+=(bytes[bitmap+(index>>3)]>>(index&7))&1;
 assert.equal(used,Math.ceil(bitmapBytes/clusterBytes)+Math.ceil(upcaseBytes/clusterBytes)+1);
 assert.equal(u32(fat+rootCluster*4),0xffffffff);assert.ok(bytes[bitmap+((rootCluster-2)>>3)]&(1<<((rootCluster-2)&7)));
 let upcaseChecksum=0;for(const byte of bytes.subarray(upcase,upcase+upcaseBytes))upcaseChecksum=((upcaseChecksum>>>1)|(upcaseChecksum<<31))+byte>>>0;
 assert.equal(upcaseChecksum,0xe619d30d);assert.equal(u32(root+64+4),upcaseChecksum);
 assert.equal(sdImageFormat(bytes),'exfat');assert.equal(validateSdImage(bytes),bytes);
});
test('FAT32 capacities stay genuine FAT32 and formatting rejects unsupported sizes and formats',()=>{
 for(const mib of [64,128]){const data=createFat32Image(mib*1024*1024),view=new DataView(data.buffer),at=view.getUint32(454,true)*512;const sectors=view.getUint32(at+32,true),fat=view.getUint32(at+36,true),reserved=view.getUint16(at+14,true);assert.ok(sectors-reserved-2*fat>=65525);assert.equal(sdImageFormat(data),'fat32');}
 for(const options of [{format:'ntfs'},{format:'exfat',byteLength:1024*1024},{byteLength:256*1024*1024},{byteLength:NaN}])assert.throws(()=>createSdImage(options),{code:'INVALID_SD_IMAGE'});
});
test('import format sniffing handles typed-array offsets and preserves unrecognized bytes',()=>{
 const original=createExfatImage(),wrapped=new Uint8Array(original.length+12);wrapped.set(original,12);const offset=wrapped.subarray(12);assert.equal(sdImageFormat(offset),'exfat');assert.equal(validateSdImage(offset),offset);
 const unknown=new Uint8Array(1024*1024);new DataView(unknown.buffer).setUint32(454,0xffffffff,true);assert.equal(sdImageFormat(unknown),'image');assert.equal(unknown[454],255);
});
