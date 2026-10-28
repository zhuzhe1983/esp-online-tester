#!/usr/bin/env node
// Recheck the onboarding receipt without retaining any firmware body on disk.
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {validateMergedImage} from '../web/platform/runtime.js';

const receipt=JSON.parse(await readFile(new URL('../docs/platform/research/lilygo-catalog-audit-2026-10-28.json',import.meta.url),'utf8'));
const artifacts=[...receipt.products.map(product=>({...product.artifact,id:product.productId+'-factory'})),...receipt.openFirmwareArtifacts.map(artifact=>({...artifact,id:artifact.project+'-'+artifact.key}))];
for(const artifact of artifacts){
  const response=await fetch(artifact.url,{credentials:'omit',redirect:'follow',signal:AbortSignal.timeout(90000)});
  if(!response.ok)throw Error(`${artifact.id}: HTTP ${response.status}`);
  if(!response.body)throw Error(`${artifact.id}: no response stream`);
  const chunks=[];let length=0;const reader=response.body.getReader();
  try{while(true){const {done,value}=await reader.read();if(done)break;length+=value.byteLength;if(length>artifact.bytes)throw Error(`${artifact.id}: size exceeds receipt`);chunks.push(value);}}
  finally{await reader.cancel();}
  const bytes=Buffer.concat(chunks),sha256=createHash('sha256').update(bytes).digest('hex');
  if(length!==artifact.bytes||sha256!==artifact.sha256)throw Error(`${artifact.id}: size or SHA-256 differs from receipt`);
  validateMergedImage(bytes,artifact.board,artifact.flashMb*1024*1024);
  console.log(`${artifact.id}: ${length} bytes, SHA-256 and complete flash layout passed`);
}
console.log(`${artifacts.length} remote images checked in memory; no execution or hardware flashing.`);
