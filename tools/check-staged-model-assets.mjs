#!/usr/bin/env node
// Compare actual source and published model dependencies after transport packing.
import assert from 'node:assert/strict';
import {readFile,readdir,writeFile} from 'node:fs/promises';
import {resolve,join,relative,dirname} from 'node:path';
import {createHash} from 'node:crypto';
import {readModelBinary} from '../web/platform/model-binary.js';
const [sourceArg,publishedArg,outputArg]=process.argv.slice(2);
if(!sourceArg||!publishedArg)throw Error('Usage: node tools/check-staged-model-assets.mjs <source-web> <published-web> [receipt.json]');
const source=resolve(sourceArg,'platform/assets'),published=resolve(publishedArg,'platform/assets');
const walk=async function*(dir){for(const entry of await readdir(dir,{withFileTypes:true})){const path=join(dir,entry.name);if(entry.isDirectory())yield*walk(path);else if(entry.isFile())yield path;}};
const hash=data=>createHash('sha256').update(data).digest('hex'),rows=[];
let textures=0,references=0;
for await(const file of walk(source)){
  if(!file.endsWith('/model.json'))continue;
  const path=relative(source,file),target=join(published,path),original=JSON.parse(await readFile(file,'utf8')),staged=JSON.parse(await readFile(target,'utf8'));
  const {binary:_oldBinary,binaryCompression:_oldCompression,artifacts:oldArtifacts,...oldRest}=original;
  const {binary:_newBinary,binaryCompression:_newCompression,artifacts:newArtifacts,...newRest}=staged;
  assert.deepEqual(newRest,oldRest,path+' model geometry, transforms and provenance');
  for(const [name,digest]of Object.entries(oldArtifacts||{}))assert.equal(newArtifacts?.[name],digest,path+' original source digest');
  const read=async(base,model)=>readModelBinary(new Response(await readFile(join(dirname(base),model.binary))),model.binaryCompression);
  const a=Buffer.from(await read(file,original)),b=Buffer.from(await read(target,staged));
  assert.deepEqual(b,a,path+' decoded vertex bytes');
  const declared=[...(original.parts||[]).map(part=>part.texture?.file),...(original.derivedParts||[]).filter(part=>['label','photo-surface'].includes(part.kind)).map(part=>part.texture),...(original.textures||[]).map(texture=>texture.file)].filter(Boolean);
  for(const name of new Set(declared)){assert.deepEqual(await readFile(join(dirname(target),name)),await readFile(join(dirname(file),name)),path+' texture '+name);textures++;}
  for(const photo of original.referencePhotos||[])if(photo?.file){
    let bytes;
    try{bytes=await readFile(join(dirname(file),photo.file));}
    catch(error){if(error.code!=='ENOENT'||!photo.sharedAssetProductId)throw error;bytes=await readFile(join(source,photo.sharedAssetProductId,photo.file));assert.equal(hash(bytes),photo.sourceSha256);}
    assert.deepEqual(await readFile(join(dirname(target),photo.file)),bytes,path+' reference '+photo.file);references++;
  }
  const packed=await readFile(join(dirname(target),staged.binary));
  if(newArtifacts?.[staged.binary])assert.equal(hash(packed),newArtifacts[staged.binary],path+' published geometry hash');
  rows.push({path,expandedSha256:hash(a),sourceMetadataSha256:hash(await readFile(file)),publishedMetadataSha256:hash(await readFile(target))});
}
rows.sort((a,b)=>a.path.localeCompare(b.path));
const result={models:rows.length,textures,references,allDecodedGeometryBytesIdentical:true,metadataAndProvenancePreserved:true,sourceAndPublishedManifestSha256:hash(JSON.stringify(rows)),source:sourceArg,published:publishedArg};
if(outputArg)await writeFile(outputArg,JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify(result,null,2));
