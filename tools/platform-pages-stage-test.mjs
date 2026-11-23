#!/usr/bin/env node
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,readFile,access,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {execFileSync} from 'node:child_process';
import {test} from 'node:test';
const script=new URL('./stage-platform-pages.mjs',import.meta.url);

test('Pages retains live model bytes and source receipts while leaving large CAD inputs in Git',async()=>{
  const root=await mkdtemp(join(tmpdir(),'platform-pages-stage-')),source=join(root,'web'),destination=join(root,'public');
  const put=async(name,body)=>{const file=join(source,name);await mkdir(join(file,'..'),{recursive:true});await writeFile(file,body);};
  try{
    for(const name of ['products','firmwares'])await put(`platform/catalog/${name}.json`,JSON.stringify({items:[]}));
    await put('wasm/fw/demo-stopwatch.json','internal');await put('wasm/fw/public/platform-stopwatch.bin','internal');await put('wasm/fw/public/public-example.bin','public');
    await put('platform/assets/device/model.json',JSON.stringify({binary:'geometry.bin',parts:[{texture:{file:'surface.png'}}],derivedParts:[{kind:'label',texture:'label.webp'},{kind:'photo-surface',texture:'board.png'}]}));
    for(const [name,body]of [['geometry.bin','live geometry'],['surface.png','live texture'],['source-manifest.json','source provenance'],['source.STEP','source CAD'],['board.kicad_pcb','source PCB'],['source.stl','source mesh']])await put(`platform/assets/device/${name}`,body);
    await put('platform/assets/device/label.webp','live label');await put('platform/assets/device/board.png','live board photo');
    const retainedDownloads=[['platform/assets/m5stack-stopwatch/source.stl','downloadable StopWatch CAD'],['platform/assets/m5stack-papermono/source.stl','downloadable PaperMono CAD']];
    for(const [name,body]of retainedDownloads)await put(name,body);
    const receipt=JSON.parse(execFileSync(process.execPath,[script.pathname,source,destination],{encoding:'utf8'}));
    assert.equal(receipt.omittedSourceArchives.length,3);assert.equal(receipt.sourceArchiveBytes,31);assert.ok(receipt.publishedBytes>0&&receipt.publishedBytes<receipt.publishedByteLimit);
    assert.deepEqual(receipt.retainedSourceDownloads.slice().sort((a,b)=>a.path.localeCompare(b.path)),retainedDownloads.map(([path,body])=>({path,bytes:Buffer.byteLength(body)})).sort((a,b)=>a.path.localeCompare(b.path)));
    assert.equal(receipt.retainedSourceDownloadBytes,retainedDownloads.reduce((sum,[,body])=>sum+Buffer.byteLength(body),0));
    for(const [name]of retainedDownloads)assert.deepEqual(await readFile(join(destination,name)),await readFile(join(source,name)));
    for(const name of ['geometry.bin','surface.png','label.webp','board.png','source-manifest.json'])assert.deepEqual(await readFile(join(source,'platform/assets/device',name)),await readFile(join(destination,'platform/assets/device',name)));
    for(const name of ['source.STEP','board.kicad_pcb','source.stl']){await access(join(source,'platform/assets/device',name));await assert.rejects(access(join(destination,'platform/assets/device',name)),{code:'ENOENT'});}
    for(const name of ['wasm/fw/demo-stopwatch.json','wasm/fw/public/platform-stopwatch.bin'])await assert.rejects(access(join(destination,name)),{code:'ENOENT'});
    assert.equal(await readFile(join(destination,'wasm/fw/public/public-example.bin'),'utf8'),'public');
    assert.throws(()=>execFileSync(process.execPath,[script.pathname,source,destination],{stdio:'pipe'}),/Command failed/,'staging still refuses to overwrite an existing tree');
  }finally{await rm(root,{recursive:true,force:true});}
});

test('Pages rejects an excluded file used by any actual loader dependency shape',async()=>{
  const root=await mkdtemp(join(tmpdir(),'platform-pages-dependency-')),source=join(root,'web');
  try{
    await mkdir(join(source,'platform/catalog'),{recursive:true});await mkdir(join(source,'platform/assets/device'),{recursive:true});await mkdir(join(source,'wasm/fw/public'),{recursive:true});
    for(const name of ['products','firmwares'])await writeFile(join(source,`platform/catalog/${name}.json`),JSON.stringify({items:[]}));
    await writeFile(join(source,'platform/assets/device/source.stl'),'needed');
    await writeFile(join(source,'platform/assets/device/geometry.bin'),'live geometry');
    const cases=[
      ['binary',{binary:'source.stl'}],
      ['part-texture',{parts:[{texture:{file:'source.stl'}}]}],
      ['derived-label',{derivedParts:[{kind:'label',texture:'source.stl'}]}],
      ['derived-photo',{derivedParts:[{kind:'photo-surface',texture:'source.stl'}]}],
    ];
    for(const [name,patch]of cases){
      await writeFile(join(source,'platform/assets/device/model.json'),JSON.stringify({binary:'geometry.bin',...patch}));
      assert.throws(()=>execFileSync(process.execPath,[script.pathname,source,join(root,`public-${name}`)],{stdio:'pipe'}),error=>String(error.stderr).includes('live model dependency'),name);
    }
  }finally{await rm(root,{recursive:true,force:true});}
});
