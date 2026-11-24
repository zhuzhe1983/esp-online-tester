#!/usr/bin/env node
// Tests use the repository's fixtures; Pages receives a separate public tree.
import {cp, mkdir, readdir, rm, access, readFile, writeFile, stat} from 'node:fs/promises';
import {resolve, join, relative, extname, dirname, sep} from 'node:path';
import {createHash} from 'node:crypto';
import {gunzipSync} from 'node:zlib';
import {packModelGeometry} from './model-geometry-pack.mjs';
import {execFileSync} from 'node:child_process';
const hash=data=>createHash('sha256').update(data).digest('hex');
const [sourceArg = 'web', destinationArg = 'target/pages'] = process.argv.slice(2);
const source = resolve(sourceArg), destination = resolve(destinationArg);
if (destination === source || !relative(source, destination).startsWith('..')) throw new Error('Publish destination must be outside the source tree.');
let exists = false;
try { await access(destination); exists = true; } catch (error) { if (error.code !== 'ENOENT') throw error; }
if (exists) throw new Error('Use a new staging directory; this command does not overwrite an existing tree.');
for (const [name, forbidden] of [['products', ['generic-esp32s3']], ['firmwares', ['platform-stopwatch-diagnostic', 'esp32sim-s3-hello']]]) {
  const catalog = JSON.parse(await readFile(join(source, 'platform/catalog', `${name}.json`), 'utf8'));
  if (catalog.items.some(item => forbidden.includes(item.id))) throw new Error(`Internal test fixtures are still listed in public ${name}.`);
}
// Rebuilding inputs stay in Git. Publish all model receipts plus actual geometry,
// textures and the explicit review-page downloads, rather than source archives.
const sourceFormats=new Set(['.step','.stp','.stl','.kicad_pcb','.kicad_sch','.kicad_mod','.wrl','.3mf']);
// These two existing model-review pages offer the original STL as a download.
// Preserve those exact user-facing targets, not every source mesh in Git.
const retainedSourceDownloadPaths=new Set(['platform/assets/m5stack-stopwatch/source.stl','platform/assets/m5stack-papermono/source.stl']);
const walk=async function*(directory){for(const entry of await readdir(directory,{withFileTypes:true})){const path=join(directory,entry.name);if(entry.isDirectory())yield*walk(path);else if(entry.isFile())yield path;}};
const assetsRoot=join(source,'platform/assets'),liveAssets=new Set(),assetFiles=new Set(),models=[],sharedReferencePhotos=[];
const reviewImages=new Set(['m5stack-stopwatch','m5stack-papermono','m5stack-gps-u032-v11','m5stack-mosaico']);
for(const name of await readdir(join(source,'platform')))if(/(?:model-review|review-lane-\d+)\.js$/.test(name)){
  const text=await readFile(join(source,'platform',name),'utf8');
  for(const match of text.matchAll(/['"](m5stack-[a-z0-9-]+)['"]/g))reviewImages.add(match[1]);
}
const assetPath=(base,name)=>{
  const path=resolve(base,name);
  if(!path.startsWith(assetsRoot+sep))throw new Error('Model dependency must stay inside platform/assets.');
  return path;
};
for await(const path of walk(assetsRoot)){
  assetFiles.add(path);
  if(!path.endsWith('/model.json'))continue;
  const model=JSON.parse(await readFile(path,'utf8'));models.push({path,model});liveAssets.add(path);
  // Public review pages can inspect any catalog model, including its references.
  for(const photo of model.referencePhotos||[])if(photo?.file){
    const asset=assetPath(dirname(path),photo.file);
    try{await access(asset);liveAssets.add(asset);}
    catch(error){
      if(error.code!=='ENOENT'||!/^[a-z0-9][a-z0-9-]*$/.test(photo.sharedAssetProductId||''))throw error;
      const original=assetPath(join(assetsRoot,photo.sharedAssetProductId),photo.file),bytes=await readFile(original),sha256=hash(bytes);
      if(photo.sourceSha256!==sha256)throw new Error('Shared reference photograph SHA-256 mismatch: '+relative(source,asset));
      liveAssets.add(original);sharedReferencePhotos.push({path:relative(source,asset),sourcePath:relative(source,original),sha256});
    }
  }
  for(const name of [model.binary,...(model.parts||[]).map(part=>part.texture?.file),...(model.derivedParts||[]).filter(part=>['label','photo-surface'].includes(part.kind)).map(part=>part.texture),...(model.textures||[]).map(texture=>texture.file)].filter(Boolean)){
    const asset=assetPath(dirname(path),name);
    if(sourceFormats.has(extname(asset).toLowerCase()))throw new Error(`A live model dependency uses an excluded source format: ${relative(source,asset)}`);
    await access(asset);liveAssets.add(asset);
  }
}
// Catalogs may declare local images independently from model textures.
const collect=value=>{
  if(typeof value==='string'&&/^(?:\.\/)?assets\//.test(value)){
    const path=resolve(source,'platform',value.split(/[?#]/)[0]);
    if(path.startsWith(assetsRoot+sep))liveAssets.add(path);
  }else if(Array.isArray(value))value.forEach(collect);
  else if(value&&typeof value==='object')Object.values(value).forEach(collect);
};
for(const name of await readdir(join(source,'platform/catalog')))if(name.endsWith('.json'))collect(JSON.parse(await readFile(join(source,'platform/catalog',name),'utf8')));
const sourceArchive=path=>{
  const name=relative(source,path),asset=relative(assetsRoot,path);
  if(!name.startsWith('platform/assets/'))return false;
  if(!assetFiles.has(path))return false;
  if(retainedSourceDownloadPaths.has(name))return false;
  if(sourceFormats.has(extname(path).toLowerCase()))return true;
  if(path.endsWith('/model-plan.json'))return true;
  if(liveAssets.has(path)||/\.(?:json(?:\.gz)?|md|txt|svg)$/i.test(path)||/license/i.test(path.split(sep).at(-1)))return false;
  if(reviewImages.has(asset.split(sep)[0])&&/\.(?:png|jpe?g|webp|svg)$/i.test(path))return false;
  return true;
};
const omittedSourceArchives=[];
const retainedSourceDownloads=[];
let sourceArchiveBytes=0,retainedSourceDownloadBytes=0;
for await(const path of walk(source)){
  if(retainedSourceDownloadPaths.has(relative(source,path))){const bytes=(await stat(path)).size;retainedSourceDownloads.push({path:relative(source,path),bytes});retainedSourceDownloadBytes+=bytes;}
  if(sourceArchive(path)){omittedSourceArchives.push(relative(source,path));sourceArchiveBytes+=(await stat(path)).size;}
}
await mkdir(destination, {recursive: true});
await cp(source, destination, {recursive: true,filter:path=>!sourceArchive(path)});
for(const photo of sharedReferencePhotos)await cp(join(source,photo.sourcePath),join(destination,photo.path));
// The generic loader supports bounded gzip and lossless vertex-byte transport;
// the three older specialized loaders still need their original arrayBuffer.
const rawLoaderDirectories=new Set(['m5stack-stopwatch','m5stack-papermono','m5stack-gps-u032-v11']);
const compressedGeometry=[],compressedPaths=new Map(),rawStillNeeded=new Set(),packedCache=new Map();let packedCacheBytes=0;
const modelJson=model=>{
  const text=JSON.stringify(model);let marker='__model_negative_zero__';while(text.includes(marker))marker+='0';
  return JSON.stringify(model,(_key,value)=>Object.is(value,-0)?marker:value,2).replaceAll(JSON.stringify(marker),'-0')+'\n';
};
for(const {path,model}of models){
  if(!model.binary)continue;
  const raw=assetPath(dirname(path),model.binary);
  if(!/\.bin(?:\.gz)?$/.test(model.binary)||model.binaryCompression&&model.binaryCompression!=='gzip'||rawLoaderDirectories.has(relative(assetsRoot,path).split(sep)[0])){rawStillNeeded.add(raw);continue;}
  const name=model.binary+'.pages.gz',published=join(destination,relative(source,raw))+'.pages.gz';
  let record=compressedPaths.get(raw);
  if(!record){
    const sourceData=await readFile(raw),data=model.binaryCompression==='gzip'?gunzipSync(sourceData,{maxOutputLength:64*1024*1024}):sourceData;
    const expandedSha256=hash(data);let packed=packedCache.get(expandedSha256);
    if(!packed){packed=packModelGeometry(data,model.binaryCompression==='gzip'?sourceData:undefined);while(packedCache.size&&packedCacheBytes+packed.data.length>64*1024*1024){const key=packedCache.keys().next().value;packedCacheBytes-=packedCache.get(key).data.length;packedCache.delete(key);}if(packed.data.length<=64*1024*1024){packedCache.set(expandedSha256,packed);packedCacheBytes+=packed.data.length;}}
    const compressed=packed.data;
    if(model.binaryCompression==='gzip'&&compressed.length>=sourceData.length){rawStillNeeded.add(raw);continue;}
    await writeFile(published,compressed);
    record={path:relative(source,raw),publishedPath:relative(destination,published),sourceBytes:sourceData.length,sourceSha256:hash(sourceData),expandedBytes:data.length,expandedSha256,publishedBytes:compressed.length,publishedSha256:hash(compressed),compression:packed.compression};
    compressedPaths.set(raw,record);compressedGeometry.push(record);
  }
  model.binary=name;model.binaryCompression=record.compression;model.artifacts={...model.artifacts,[name]:record.publishedSha256};
  await writeFile(join(destination,relative(source,path)),modelJson(model));
}
for(const raw of compressedPaths.keys())if(!rawStillNeeded.has(raw))await rm(join(destination,relative(source,raw)));
const excluded = ['wasm/fw/demo-stopwatch.json'];
for (const name of await readdir(join(destination, 'platform/firmware')).catch(error => { if (error.code === 'ENOENT') return []; throw error; })) if (/^waveshare-c5-.*-probe\.bin$/.test(name)) excluded.push(`platform/firmware/${name}`);
for (const name of await readdir(join(destination, 'wasm/fw/public'))) if (name.startsWith('platform-stopwatch')) excluded.push(`wasm/fw/public/${name}`);
for (const path of excluded) await rm(join(destination, path), {force: true});
for (const path of excluded) {
  let present = false;
  try { await access(join(destination, path)); present = true; } catch (error) { if (error.code !== 'ENOENT') throw error; }
  if (present) throw new Error(`Test fixture leaked into public output: ${path}`);
}
// A new metadata transport must reach the same revision of its decoder, even
// when the previous deployment's module or WASM URLs remain in browser caches.
let cacheVersion;
try{cacheVersion=execFileSync('git',['-C',source,'rev-parse','--short=12','HEAD'],{encoding:'utf8',stdio:['ignore','pipe','ignore']}).trim();}
catch{cacheVersion=hash(await readFile(new URL(import.meta.url))).slice(0,12);}
for await(const path of walk(destination)){
  const name=relative(destination,path);
  if(name.startsWith('platform/assets/')||/(?:^|\/)vendor\//.test(name)||!['.js','.mjs','.html'].includes(extname(path)))continue;
  const text=await readFile(path,'utf8');
  const versioned=(all,start,quote,url)=>/(?:^|\/)vendor\//.test(url)?all:`${start}${quote}${url}?v=${cacheVersion}${quote}`;
  let updated=text.replace(/(\bfrom\s*|\bimport\s*(?:\(\s*)?)(['"])(\.{1,2}\/[a-zA-Z0-9_.\/-]+\.(?:js|mjs))\2/g,versioned);
  updated=updated.replace(/(\b(?:new\s+(?:URL|Worker)|fetch)\s*\(\s*)(['"])((?:\.{1,2}\/)?[a-zA-Z0-9][a-zA-Z0-9_.\/-]*\.(?:js|mjs|wasm))\2/g,versioned);
  if(path.endsWith('.html'))updated=updated.replace(/(\b(?:src|href)=["'])([^"':?#]+\.(?:js|mjs|css))(["'])/g,(all,start,url,end)=>/(?:^|\/)vendor\//.test(url)?all:`${start}${url}?v=${cacheVersion}${end}`);
  if(updated!==text)await writeFile(path,updated);
}
let publishedBytes=0;
for await(const path of walk(destination))publishedBytes+=(await stat(path)).size;
if(publishedBytes>1_000_000_000)throw new Error(`Published site exceeds the conservative 1 GB Pages budget: ${publishedBytes} bytes.`);
console.log(JSON.stringify({publicSite: destinationArg,cacheVersion,excludedInternalFixtures: excluded,omittedSourceArchives,sourceArchiveBytes,retainedSourceDownloads,retainedSourceDownloadBytes,sharedReferencePhotos,compressedGeometry,publishedBytes,publishedByteLimit:1_000_000_000}, null, 2));
