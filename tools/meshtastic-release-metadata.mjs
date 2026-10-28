// Only target manifests are downloaded here; firmware bodies remain upstream.
import {createHash} from 'node:crypto';
import {releaseMatchesChannel} from './firmware-release-source.mjs';

export async function enrichMeshtasticReleases(entry,releases,{fetchImpl=fetch}={}) {
  if(entry.assetMetadata!=='meshtastic-targets')return releases;
  if(entry.upstreamRepo!=='meshtastic/firmware')throw Error('Unexpected Meshtastic repository');
  const result=structuredClone(releases),cache=new Map();
  for(const target of entry.catalogTargets||[]) {
    if(!/^[a-z0-9-]+$/.test(target.boardTarget||'')||!['esp32','esp32c6','esp32s3'].includes(target.targetChip)||target.format!=='merged')throw Error('Invalid Meshtastic target');
    for(const release of result.filter(r=>releaseMatchesChannel(r,target.channel))) {
      const version=release.tag_name.replace(/^v/,'');
      if(!/^[0-9][a-zA-Z0-9.-]{0,79}$/.test(version))throw Error('Invalid Meshtastic version');
      const base=`https://release.meshtastic.org/${version}/`,name=`firmware-${target.boardTarget}-${version}`,url=base+name+'.mt.json';
      if(!cache.has(url))cache.set(url,(async()=>{
        const response=await fetchImpl(url,{headers:{Accept:'application/json'},redirect:'error',signal:AbortSignal.timeout(30000)});
        if(response.status===404){await response.body?.cancel();return null;}
        if(!response.ok){await response.body?.cancel();throw Error('Meshtastic target metadata unavailable');}
        if(!response.body)throw Error('Meshtastic metadata has no stream');
        const reader=response.body.getReader(),chunks=[];let length=0;
        try{while(true){const {done,value}=await reader.read();if(done)break;length+=value.byteLength;if(length>256*1024)throw Error('Meshtastic metadata exceeds limit');chunks.push(value);}}
        finally{await reader.cancel();}
        const bytes=Buffer.concat(chunks),data=JSON.parse(bytes.toString('utf8'));
        return {data,sha256:createHash('sha256').update(bytes).digest('hex'),bytes:bytes.length};
      })());
      const saved=await cache.get(url);if(!saved)continue;
      const {data}=saved;
      if(data.version!==version||data.platformioTarget!==target.boardTarget||data.mcu!==target.targetChip||data.repo!=='meshtastic/firmware'||!Array.isArray(data.files))throw Error('Meshtastic manifest identity mismatch');
      const files=data.files.filter(file=>file.name===name+'.factory.bin');
      if(files.length!==1||!Number.isSafeInteger(files[0].bytes)||files[0].bytes<=0||files[0].bytes>256*1024*1024)throw Error('Meshtastic factory artifact missing or ambiguous');
      const file=files[0],digest=/^[a-f0-9]{64}$/i.test(file.sha256||'')?`sha256:${file.sha256.toLowerCase()}`:null;
      // MD5 is retained as publisher metadata, never presented as SHA-256.
      const asset={name:file.name,browser_download_url:base+file.name,size:file.bytes,digest,format:'merged',boardTarget:target.boardTarget,chip:target.targetChip,
        ...( /^[a-f0-9]{32}$/i.test(file.md5||'')?{publisherMd5:file.md5.toLowerCase()}:{}),
        metadata:{url,sha256:saved.sha256,bytes:saved.bytes}};
      if(!release.assets.some(a=>a.name===asset.name))release.assets.push(asset);
      break;
    }
  }
  return result;
}
