#!/usr/bin/env node
// Offline audit of actual factory captures against fixed PNG assets and M5GFX LUTs.
// This checks the digital endpoint only; it does not infer physical grayscale.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {join} from 'node:path';
import {inflateSync} from 'node:zlib';
import {createHash} from 'node:crypto';
const args=process.argv.slice(2),arg=k=>args[args.indexOf(k)+1];
for(const k of ['--assets','--panel-source','--frames'])assert(args.includes(k)&&arg(k),`Required ${k}`);
const hash=b=>createHash('sha256').update(b).digest('hex');
const sources=JSON.parse(await readFile(new URL('../docs/platform/evidence/papers3-display/sources.json',import.meta.url)));
async function source(id,path){const b=await readFile(path),s=sources.sources.find(s=>s.id===id);assert.equal(hash(b),s.sha256,`Changed source ${id}`);return b;}
// Fixed public assets are noninterlaced 8-bit RGB or palette PNGs.
function decodePng(b){
 assert.equal(b.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
 let w,h,type,palette,alpha;const chunks=[];
 for(let at=8;at<b.length;){const n=b.readUInt32BE(at),name=b.toString('ascii',at+4,at+8),d=b.subarray(at+8,at+8+n);at+=n+12;
  if(name==='IHDR'){w=d.readUInt32BE(0);h=d.readUInt32BE(4);assert.equal(d[8],8);assert.equal(d[12],0);type=d[9];}
  if(name==='PLTE')palette=d;if(name==='tRNS')alpha=d;if(name==='IDAT')chunks.push(d);
 }
 const channels=({2:3,3:1})[type];assert(channels);const stride=w*channels,raw=inflateSync(Buffer.concat(chunks)),pixels=Buffer.alloc(stride*h);assert.equal(raw.length,(stride+1)*h);
 for(let y=0;y<h;y++){const filter=raw[y*(stride+1)];assert(filter<=4);for(let x=0;x<stride;x++){
  const a=x>=channels?pixels[y*stride+x-channels]:0,c=y&&x>=channels?pixels[(y-1)*stride+x-channels]:0,u=y?pixels[(y-1)*stride+x]:0,p=a+u-c,pa=Math.abs(p-a),pb=Math.abs(p-u),pc=Math.abs(p-c);
  const predict=filter===0?0:filter===1?a:filter===2?u:filter===3?Math.floor((a+u)/2):pa<=pb&&pa<=pc?a:pb<=pc?u:c;
  pixels[y*stride+x]=(raw[y*(stride+1)+1+x]+predict)&255;
 }}
 return {width:w,height:h,gray(x,y){assert(x>=0&&x<w&&y>=0&&y<h);const i=(y*w+x)*channels,c=pixels[i],rgb=type===3?palette.subarray(c*3,c*3+3):pixels.subarray(i,i+3);assert.equal(rgb[0],rgb[1]);assert.equal(rgb[1],rgb[2]);assert.equal(type===3?(alpha?.[c]??255):255,255);return rgb[0];}};
}
const panel=(await source('m5gfx-panel',arg('--panel-source'))).toString();
const bayer=panel.match(/Bayer\[16\]\s*=\s*\{([^}]+)\}/)[1].split(',').map(Number);assert.equal(bayer.length,16);
function endpoint(mode){const block=panel.match(new RegExp(`lut_${mode}\\[\\]\\s*=\\s*\\{([\\s\\S]*?)\\};`))[1],levels=Array(16).fill(null);
 for(const row of block.matchAll(/LUT_MAKE\(([^)]+)\)/g)){const commands=row[1].split(',').map(Number);assert.equal(commands.length,16);commands.forEach((c,i)=>{if(c===1)levels[i]=0;if(c===2)levels[i]=255;});}
 if(mode==='fastest'){assert.notEqual(levels[0],null);assert.notEqual(levels[15],null);}else assert(levels.every(v=>v!==null));return levels;
}
const endpoints={quality:endpoint('quality'),fastest:endpoint('fastest')},images={};
for(const name of ['img_bg.png','img_icon_mute_on.png','img_icon_mute_off.png'])images[name]=decodePng(await source(name,join(arg('--assets'),name)));
const w=88,h=85,x0=298,y0=380;assert.deepEqual([images['img_icon_mute_on.png'].width,images['img_icon_mute_on.png'].height],[w,h]);
const frames={};for(const n of ['first','buzzer','returned']){frames[n]=await readFile(join(arg('--frames'),n+'.rgba'));assert.equal(frames[n].length,540*960*4);}
function expected(asset,mode){const pixels=Buffer.alloc(w*h),image=images[asset];for(let j=0;j<h;j++)for(let i=0;i<w;i++){
 const x=x0+i,y=y0+j,gray=image.gray(asset==='img_bg.png'?x:i,asset==='img_bg.png'?y:j),d=bayer[(y&3)*4+(x&3)],level=mode==='fastest'?(gray+(d<<4)<248?0:15):Math.min(15,Math.max(0,(gray+d-8)>>4));pixels[j*w+i]=endpoints[mode][level];
}return pixels;}
function actual(name){const pixels=Buffer.alloc(w*h),frame=frames[name];for(let j=0;j<h;j++)for(let i=0;i<w;i++){const at=((x0+i)*540+539-(y0+j))*4;assert.equal(frame[at],frame[at+1]);assert.equal(frame[at],frame[at+2]);assert.equal(frame[at+3],255);pixels[j*w+i]=frame[at];}return pixels;}
const count=(a,b)=>a.reduce((sum,v,i)=>sum+(v!==b[i]),0),comparisons=[];
for(const [frame,asset,mode] of [['first','img_bg.png','quality'],['buzzer','img_icon_mute_off.png','fastest'],['returned','img_icon_mute_on.png','fastest']]){
 const a=actual(frame),e=expected(asset,mode),mismatches=count(a,e);comparisons.push({frame,asset,mode,pixels:w*h,mismatches,frameRgbaSha256:hash(frames[frame]),regionGraySha256:hash(a),expectedGraySha256:hash(e)});assert.equal(mismatches,0,`Unexpected ${frame} pixels`);
}
const before=actual('first'),after=actual('returned');let sourceDifference=0;for(let j=0;j<h;j++)for(let i=0;i<w;i++)sourceDifference+=images['img_bg.png'].gray(x0+i,y0+j)!==images['img_icon_mute_on.png'].gray(i,j);
console.log(JSON.stringify({status:'pass',method:'PNG decode plus fixed M5GFX Bayer quantization and last non-hold drive endpoint; compared to actual WASM physical framebuffer.',nativeRegion:[x0,y0,w,h],sourceGrayDifferencePixels:sourceDifference,actualChangedPixels:count(before,after),blackToWhite:before.reduce((n,v,i)=>n+(v===0&&after[i]===255),0),whiteToBlack:before.reduce((n,v,i)=>n+(v===255&&after[i]===0),0),comparisons,limitations:['quality is the current uncalibrated black/white endpoint, not the physical 16-gray result.','Exact equality covers the 88x85 mute icon region and these three actual captures; it is not a whole-screen or EDCBook correctness claim.']},null,2));
