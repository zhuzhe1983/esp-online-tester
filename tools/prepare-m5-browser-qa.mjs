#!/usr/bin/env node
// Generates authored browser diagnostics without editing the product app.
// Origin and output are caller-supplied; no physical-device or factory claim.
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
const args=process.argv.slice(2),options={};for(let i=0;i<args.length;i+=2)options[args[i]]=args[i+1];
if(!options['--repo']||!options['--out']||!options['--origin'])throw Error('Use --repo CHECKOUT --out STATIC_QA_DIR --origin HTTP_OR_HTTPS_ORIGIN');
const repo=resolve(options['--repo']),out=resolve(options['--out']),origin=new URL(options['--origin']);
if(!['http:','https:'].includes(origin.protocol)||origin.pathname!=='/'||origin.search||origin.hash||origin.username||origin.password)throw Error('Supply a bare HTTP(S) origin');
const base=origin.origin,hash=b=>createHash('sha256').update(b).digest('hex'),relativeOut=options['--path']||'/qa-m5/';
if(!relativeOut.startsWith('/')||!relativeOut.endsWith('/'))throw Error('--path must be an absolute path ending /');
await mkdir(out,{recursive:true});
const profiles=JSON.parse(await readFile(join(repo,'docs/platform/research/m5-classic-camera-runtime.json')));
const source=await readFile(join(repo,'tools/fixtures/m5-classic-camera-probe.mjs'),'utf8');
const expected="arm();waitEof();pixels(0x00f80000,'red RGB565 DMA');";
if(source.split(expected).length!==2)throw Error('The diagnostic changed; review the bounded red-frame wait adaptation');
const adapted=source.replace(expected,"label('red-frame');arm();waitEof();read(4,B);li(5,0x00f80000);bcc('red-seen',4,5);jump('red-frame');label('red-seen');pixels(0x00f80000,'red RGB565 DMA');");
// Only the diagnostic waits for a human to choose the first picture. Its checks
// still read real descriptor/pixel RAM after guest SCCB/LEDC/GPIO/I2S execution.
const {m5ClassicCameraProbe}=await import('data:text/javascript;base64,'+Buffer.from(adapted).toString('base64'));
const {m5UnitControllerProbe}=await import(pathToFileURL(join(repo,'tools/fixtures/m5-unit-controller-probe.mjs')));
const {poeP4ProbeImage}=await import(pathToFileURL(join(repo,'tools/fixtures/poep4-guest.mjs')));
const table=Buffer.alloc(0xc00,255);table.writeUInt16LE(0x50aa);table[2]=0;table[3]=0;table.writeUInt32LE(0x10000,4);table.writeUInt32LE(0x10000,8);table.fill(0,12,32);table.write('camera-probe',12);
await writeFile(join(out,'partitions.bin'),table);
const rom=await readFile(join(repo,'web/wasm/fw/esp32_rev300_rom.elf'));
const rows=[];
for(const [name,board,image,psram] of [
 ['u017-wait-picture','m5stack-u017',m5ClassicCameraProbe(profiles.products.find(p=>p.id==='m5stack-u017')),4],
 ['u149','unit-thermal2',m5UnitControllerProbe('unit-thermal2'),0],
]){
 const manifest={board,flash_mb:4,psram_mb:psram,app_direct:false,stubs:[],files:{rom:{url:base+'/wasm/fw/esp32_rev300_rom.elf',sha256:hash(rom)},bootloader:{url:base+relativeOut+name+'.bin',sha256:hash(image)},ptable:{url:base+relativeOut+'partitions.bin',sha256:hash(table)},app:{url:base+relativeOut+name+'.bin',sha256:hash(image)}}};
 const bytes=JSON.stringify(manifest);await writeFile(join(out,name+'.bin'),image);await writeFile(join(out,name+'.json'),bytes);
 rows.push({name,imageBytes:image.length,imageSha256:hash(image),manifestSha256:hash(bytes)});
}
const p4=poeP4ProbeImage();await writeFile(join(out,'u213.bin'),p4);rows.push({name:'u213',imageBytes:p4.length,imageSha256:hash(p4),bootMode:'app-direct-original-RV32-diagnostic'});
const shared={accessories:[],firmware:{manifestUrl:base+relativeOut+'u017-wait-picture.json',sha256:rows[0].manifestSha256},keyMapping:{},productId:'m5stack-u017',schemaVersion:'1',view:{deviceView:'3d',mode:'embed'}};
const scene=Buffer.from(JSON.stringify(shared)).toString('base64url');
const iframe=`<iframe src="${base}/platform/embed.html#scene=${scene}&autostart=1&autoplay=1" title="M5Stack M5Camera · U017 · ESP Player" width="800" height="640" allow="serial; fullscreen; microphone; autoplay" loading="lazy"></iframe>`;
await writeFile(join(out,'iframe.html'),'<!doctype html><meta charset="utf-8"><title>M5 camera iframe QA</title><h1>Actual shared M5Camera player</h1>'+iframe);
const receipt={scope:'Authored diagnostics and synthetic public-colour picture stimuli only; no manufacturer binaries.',originalCameraSourceSha256:hash(source),adaptedCameraSourceSha256:hash(adapted),romSha256:hash(rom),files:rows};await writeFile(join(out,'fixture-receipt.json'),JSON.stringify(receipt,null,2)+'\n');console.log(JSON.stringify(receipt,null,2));
