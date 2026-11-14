import assert from 'node:assert/strict';
import {test} from 'node:test';
import {readFile} from 'node:fs/promises';
import {releaseRequest,publicReleases} from './firmware-release-source.mjs';
import {syncReleaseMetadata} from './sync-firmware-release-metadata.mjs';
import {updateFirmwareCatalog} from './sync-firmware-catalog.mjs';
import {validateWatchlist} from '../web/platform/watchlist.js';
const read=async name=>JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url)));
const watchlist=await read('firmware-watchlist'),entry=watchlist.entries.find(e=>e.id==='espressif-mosaico-official');
const metadata={id:4,version:'Vibe Mode - v0.3.0',title:'Mosaico Hello v0.1.3 + Vibe Mode v0.3.0',target:'ESP32-S31',build_date:'2026-11-12',file_url:'https://mosaico.ai/api/v1/firmware/4/download',file_name:'hello-mosaico-complete_0.1.3.bin',file_size:16777216,notes:'publisher markup',download_count:87};
test('Mosaico metadata has a fixed public endpoint and strict target/artifact identity',()=>{
  validateWatchlist(watchlist);
  const request=releaseRequest(entry,'private-github-token');
  assert.equal(request.url,'https://mosaico.ai/api/v1/firmware');assert.equal(request.headers.Authorization,undefined);assert.equal(request.maxBytes,65536);
  const [release]=publicReleases(entry,metadata);
  assert.equal(release.tag_name,entry.version);assert.equal(release.assets[0].digest,null);
  assert.equal('notes' in release,false);assert.equal('download_count' in release,false);
  for(const change of [{target:'ESP32-S3'},{id:'4'},{file_url:'https://example.com/fw.bin'},{file_name:'../fw.bin'},{file_size:0},{build_date:'unknown'}])assert.throws(()=>publicReleases(entry,{...metadata,...change}));
  assert.throws(()=>releaseRequest({...entry,metadataUrl:'https://example.com/api'}));
  assert.throws(()=>validateWatchlist({...watchlist,entries:[{...entry,catalogTargets:[{}]}]}));
});
test('Mosaico watches version metadata without downloading or replacing pinned firmware',async()=>{
  const list={...watchlist,entries:[entry]},calls=[];
  const snapshot=await syncReleaseMetadata(list,{token:'private-github-token',fetchImpl:async(url,options)=>{
    calls.push(url);assert.equal(options.headers.Authorization,undefined);return new Response(JSON.stringify(metadata));
  },now:'2026-11-14T10:00:00Z'});
  assert.deepEqual(calls,[entry.metadataUrl]);assert.equal(snapshot.entries[0].error,null);
  const firmwares=await read('firmwares'),result=updateFirmwareCatalog({watchlist:list,products:await read('products'),firmwares,snapshot});
  assert.deepEqual(result.firmwares,firmwares);assert.equal(result.report.entries[0].reason,'no-explicit-device-targets');
  const previous=snapshot,failed=await syncReleaseMetadata(list,{previous,fetchImpl:async()=>new Response('{}'),now:'2026-11-15T10:00:00Z'});
  assert.deepEqual(failed.entries[0].release,previous.entries[0].release);assert.ok(failed.entries[0].error);
});
