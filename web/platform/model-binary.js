// Lossless transport for manufacturer geometry. Bound both the downloaded and
// expanded streams before collecting them; the mesh contract remains float32.
export async function readModelBinary(response, compression, maxBytes=64*1024*1024) {
  if(compression!==undefined&&compression!=='gzip')throw Error('Unsupported model geometry compression');
  if(!Number.isSafeInteger(maxBytes)||maxBytes<=0)throw Error('Invalid model geometry limit');
  const exceeded=()=>Error(`Model geometry exceeds ${maxBytes} bytes`);
  const length=Number(response.headers.get('content-length'));
  if(Number.isFinite(length)&&length>maxBytes)throw exceeded();
  if(!response.body)throw Error('Empty model geometry response');
  let downloaded=0,stream=response.body.pipeThrough(new TransformStream({transform(chunk,controller){
    downloaded+=chunk.byteLength;if(downloaded>maxBytes)throw exceeded();controller.enqueue(chunk);
  }}));
  if(compression==='gzip') {
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
  return binary.buffer;
}
