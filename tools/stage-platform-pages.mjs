#!/usr/bin/env node
// Tests use the repository's fixtures; Pages receives a separate public tree.
import {cp, mkdir, readdir, rm, access, readFile, stat} from 'node:fs/promises';
import {resolve, join, relative, extname, dirname} from 'node:path';
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
// These are retained, hash-pinned rebuilding inputs in Git. The browser reads
// exported geometry.bin and textures, never STEP/STL/KiCad source archives.
// Excluding only these known source formats keeps the published site below
// GitHub Pages' 1 GB limit without dropping model metadata or live assets.
const sourceFormats=new Set(['.step','.stp','.stl','.kicad_pcb','.kicad_sch','.kicad_mod','.wrl','.3mf']);
// These two existing model-review pages offer the original STL as a download.
// Preserve those exact user-facing targets, not every source mesh in Git.
const retainedSourceDownloadPaths=new Set(['platform/assets/m5stack-stopwatch/source.stl','platform/assets/m5stack-papermono/source.stl']);
const sourceArchive=path=>relative(source,path).startsWith(`platform/assets/`)&&sourceFormats.has(extname(path).toLowerCase())&&!retainedSourceDownloadPaths.has(relative(source,path));
const walk=async function*(directory){for(const entry of await readdir(directory,{withFileTypes:true})){const path=join(directory,entry.name);if(entry.isDirectory())yield*walk(path);else if(entry.isFile())yield path;}};
const omittedSourceArchives=[];
const retainedSourceDownloads=[];
let sourceArchiveBytes=0,retainedSourceDownloadBytes=0;
for await(const path of walk(source)){
  if(retainedSourceDownloadPaths.has(relative(source,path))){const bytes=(await stat(path)).size;retainedSourceDownloads.push({path:relative(source,path),bytes});retainedSourceDownloadBytes+=bytes;}
  if(sourceArchive(path)){omittedSourceArchives.push(relative(source,path));sourceArchiveBytes+=(await stat(path)).size;}
  if(relative(source,path).startsWith('platform/assets/')&&path.endsWith('/model.json')){
    const model=JSON.parse(await readFile(path,'utf8'));
    for(const name of [model.binary,...(model.parts||[]).map(part=>part.texture?.file),...(model.derivedParts||[]).filter(part=>['label','photo-surface'].includes(part.kind)).map(part=>part.texture),...(model.textures||[]).map(texture=>texture.file)].filter(Boolean)){
      const asset=resolve(dirname(path),name);
      if(sourceArchive(asset))throw new Error(`A live model dependency uses an excluded source format: ${relative(source,asset)}`);
    }
  }
}
await mkdir(destination, {recursive: true});
await cp(source, destination, {recursive: true,filter:path=>!sourceArchive(path)});
const excluded = ['wasm/fw/demo-stopwatch.json'];
for (const name of await readdir(join(destination, 'wasm/fw/public'))) if (name.startsWith('platform-stopwatch')) excluded.push(`wasm/fw/public/${name}`);
for (const path of excluded) await rm(join(destination, path), {force: true});
for (const path of excluded) {
  let present = false;
  try { await access(join(destination, path)); present = true; } catch (error) { if (error.code !== 'ENOENT') throw error; }
  if (present) throw new Error(`Test fixture leaked into public output: ${path}`);
}
let publishedBytes=0;
for await(const path of walk(destination))publishedBytes+=(await stat(path)).size;
if(publishedBytes>1_000_000_000)throw new Error(`Published site exceeds the conservative 1 GB Pages budget: ${publishedBytes} bytes.`);
console.log(JSON.stringify({publicSite: destinationArg, excludedInternalFixtures: excluded,omittedSourceArchives,sourceArchiveBytes,retainedSourceDownloads,retainedSourceDownloadBytes,publishedBytes,publishedByteLimit:1_000_000_000}, null, 2));
