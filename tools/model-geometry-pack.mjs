import {gzipSync} from 'node:zlib';

function columns(raw,stride) {
  const rows=raw.length/stride,shuffled=new Uint8Array(raw.length);
  for(let column=0;column<stride;column++)for(let row=0,at=column;row<rows;row++,at+=stride)
    shuffled[column*rows+row]=raw[at]^(row?raw[at-stride]:0);
  return shuffled;
}

function dictionary(bytes,offset) {
  const count=bytes.length/24,lookup=new Map(),offsets=[],indices=Buffer.alloc(count*4);
  for(let i=0,at=offset;i<count;i++,at+=24){
    const key=bytes.toString('base64',at,at+12);let index=lookup.get(key);
    if(index===undefined){index=offsets.length;lookup.set(key,index);offsets.push(at);}
    indices.writeUInt32LE(index,i*4);
  }
  const unique=Buffer.alloc(offsets.length*12);
  for(let i=0;i<offsets.length;i++)bytes.copy(unique,i*12,offsets[i],offsets[i]+12);
  return {unique,indices,count:offsets.length};
}

function indexed(raw) {
  const bytes=Buffer.from(raw.buffer,raw.byteOffset,raw.byteLength),positions=dictionary(bytes,0),normals=dictionary(bytes,12);
  if(12+positions.unique.length+normals.unique.length+positions.indices.length+normals.indices.length>=Math.min(64*1024*1024,raw.length*.95))return null;
  const header=Buffer.alloc(12);header.writeUInt32LE(raw.length,0);header.writeUInt32LE(positions.count,4);header.writeUInt32LE(normals.count,8);
  return {data:gzipSync(Buffer.concat([header,columns(positions.unique,12),columns(normals.unique,12),columns(positions.indices,4),columns(normals.indices,4)]),{level:6}),compression:'gzip-split-indexed-12'};
}

// Lossless byte transport only: six float32 fields per vertex remain unchanged.
export function packModelGeometry(raw,gzipBaseline) {
  let result={data:gzipBaseline||gzipSync(raw,{level:6}),compression:'gzip'};
  if(!raw.length||raw.length%24)return result;
  if(raw.length>=96*1024){const packed=indexed(raw);if(packed&&packed.data.length<result.data.length){if(packed.data.length<result.data.length*.85)return packed;result=packed;}}
  const data=gzipSync(columns(raw,24),{level:6});
  if(data.length<result.data.length)result={data,compression:'gzip-xor-shuffle-24'};
  return result;
}
