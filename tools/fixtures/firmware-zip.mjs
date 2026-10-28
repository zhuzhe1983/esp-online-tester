import {deflateRawSync} from 'node:zlib';
// Independent test ZIP writer, including data-descriptor archives used by Spark.
export function firmwareZip(data,{name='firmware.bin',method=8,descriptor=false}={}) {
 data=Buffer.from(data);const filename=Buffer.from(name),payload=method===8?deflateRawSync(data):data;
 let crc=0xffffffff;for(const b of data){crc^=b;for(let bit=0;bit<8;bit++)crc=crc&1?(crc>>>1)^0xedb88320:crc>>>1;}crc=(crc^0xffffffff)>>>0;
 const header=Buffer.alloc(30);header.writeUInt32LE(0x04034b50);header.writeUInt16LE(20,4);header.writeUInt16LE(descriptor?8:0,6);header.writeUInt16LE(method,8);header.writeUInt16LE(filename.length,26);
 if(!descriptor){header.writeUInt32LE(crc,14);header.writeUInt32LE(payload.length,18);header.writeUInt32LE(data.length,22);}
 const dd=Buffer.alloc(descriptor?16:0);if(descriptor){dd.writeUInt32LE(0x08074b50);dd.writeUInt32LE(crc,4);dd.writeUInt32LE(payload.length,8);dd.writeUInt32LE(data.length,12);}
 const central=Buffer.alloc(46);central.writeUInt32LE(0x02014b50);central.writeUInt16LE(20,4);central.writeUInt16LE(20,6);central.writeUInt16LE(descriptor?8:0,8);central.writeUInt16LE(method,10);central.writeUInt32LE(crc,16);central.writeUInt32LE(payload.length,20);central.writeUInt32LE(data.length,24);central.writeUInt16LE(filename.length,28);
 const end=Buffer.alloc(22);end.writeUInt32LE(0x06054b50);end.writeUInt16LE(1,8);end.writeUInt16LE(1,10);end.writeUInt32LE(central.length+filename.length,12);end.writeUInt32LE(header.length+filename.length+payload.length+dd.length,16);
 return Buffer.concat([header,filename,payload,dd,central,filename,end]);
}
