// Lossless transport for manufacturer geometry. Bound both the downloaded and
// expanded streams before collecting them; the mesh contract remains float32.
function restoreColumns(source,stride) {
  if(source.length%stride)throw Error('Invalid shuffled geometry stride');
  const restored=new Uint8Array(source.length),rows=source.length/stride;
  for(let column=0;column<stride;column++){
    let value=0;
    for(let row=0,at=column;row<rows;row++,at+=stride){value^=source[column*rows+row];restored[at]=value;}
  }
  return restored;
}
export async function readModelBinary(response, compression, maxBytes=64*1024*1024) {
  if(compression!==undefined&&!['gzip','gzip-xor-shuffle-24','gzip-split-indexed-12'].includes(compression))throw Error('Unsupported model geometry compression');
  if(!Number.isSafeInteger(maxBytes)||maxBytes<=0)throw Error('Invalid model geometry limit');
  const exceeded=()=>Error(`Model geometry exceeds ${maxBytes} bytes`);
  const length=Number(response.headers.get('content-length'));
  if(Number.isFinite(length)&&length>maxBytes)throw exceeded();
  if(!response.body)throw Error('Empty model geometry response');
  let downloaded=0,stream=response.body.pipeThrough(new TransformStream({transform(chunk,controller){
    downloaded+=chunk.byteLength;if(downloaded>maxBytes)throw exceeded();controller.enqueue(chunk);
  }}));
  if(compression!==undefined) {
    if(typeof DecompressionStream!=='function')throw Error('This browser cannot decompress gzip model geometry');
    stream=stream.pipeThrough(new DecompressionStream('gzip'));
  }
  const reader=stream.getReader(),chunks=[];let total=0;
  try {
    for(;;){const {value,done}=await reader.read();if(done)break;total+=value.byteLength;if(total>maxBytes)throw exceeded();chunks.push(value);}
  } catch(error) {await reader.cancel(error).catch(()=>{});throw error;}
  finally {reader.releaseLock();}
  const binary=new Uint8Array(total);let offset=0;
  for(const chunk of chunks){binary.set(chunk,offset);offset+=chunk.byteLength;}
  if(compression==='gzip-xor-shuffle-24')return restoreColumns(binary,24).buffer;
  if(compression!=='gzip-split-indexed-12')return binary.buffer;
  if(total<12)throw Error('Invalid indexed geometry header');
  const header=new DataView(binary.buffer),bytes=header.getUint32(0,true),positionCount=header.getUint32(4,true),normalCount=header.getUint32(8,true),rows=bytes/24;
  if(bytes>maxBytes)throw exceeded();
  if(bytes%24||positionCount>rows||normalCount>rows||rows>0&&(!positionCount||!normalCount)||12+(positionCount+normalCount)*12+rows*8!==total)throw Error('Invalid indexed geometry size');
  const normalStart=12+positionCount*12,indexStart=normalStart+normalCount*12;
  const positions=new Uint32Array(restoreColumns(binary.subarray(12,normalStart),12).buffer),normals=new Uint32Array(restoreColumns(binary.subarray(normalStart,indexStart),12).buffer);
  const positionIndices=new DataView(restoreColumns(binary.subarray(indexStart,indexStart+rows*4),4).buffer),normalIndices=new DataView(restoreColumns(binary.subarray(indexStart+rows*4),4).buffer),output=new Uint32Array(bytes/4);
  for(let i=0;i<rows;i++){
    const position=positionIndices.getUint32(i*4,true),normal=normalIndices.getUint32(i*4,true);
    if(position>=positionCount||normal>=normalCount)throw Error('Invalid geometry vertex index');
    for(let field=0;field<3;field++){output[i*6+field]=positions[position*3+field];output[i*6+3+field]=normals[normal*3+field];}
  }
  return output.buffer;
}
