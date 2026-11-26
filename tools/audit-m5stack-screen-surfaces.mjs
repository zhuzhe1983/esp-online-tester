// CPU geometry and actual alpha decoding; deliberately no WebGL acceptance.
import {readFile,writeFile} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {pickVisibleSurface} from '../web/platform/scene-interaction.js';

if(!process.argv[2])throw Error('Usage: node tools/audit-m5stack-screen-surfaces.mjs <receipt.json>');
const root=new URL('../web/platform/',import.meta.url);
const products=JSON.parse(await readFile(new URL('catalog/products.json',root))).items.filter(p=>p.manufacturer==='M5Stack');
const old={fetch:globalThis.fetch,document:globalThis.document,load:THREE.TextureLoader.prototype.loadAsync};
const rows=[];
try{
 globalThis.fetch=async url=>new Response(await readFile(fileURLToPath(url)));
 // This implements the same decoded-alpha access used by pickVisibleSurface.
 // It does not emulate DOM layout, text rasterization or a GPU renderer.
 globalThis.document={createElement:()=>{let source;return {width:1,height:1,getContext:()=>({fillText(){},drawImage(image){source=image;},getImageData(){return {data:source?.data||new Uint8ClampedArray([255,255,255,255])};}})};}};
 THREE.TextureLoader.prototype.loadAsync=async url=>{
  const filename=fileURLToPath(url),bytes=await readFile(filename);
  const alpha=filename.endsWith('.webp')||(filename.endsWith('.png')&&([4,6].includes(bytes[25])||bytes.includes(Buffer.from('tRNS'))));
  if(!alpha)return new THREE.Texture({width:1,height:1,data:new Uint8ClampedArray([255,255,255,255])});
  const data=execFileSync('python',['-c','import sys,struct;from PIL import Image;i=Image.open(sys.argv[1]).convert("RGBA");sys.stdout.buffer.write(struct.pack("<II",*i.size)+i.tobytes())',filename],{maxBuffer:128*1024*1024});
  return new THREE.Texture({width:data.readUInt32LE(0),height:data.readUInt32LE(4),data:new Uint8ClampedArray(data.subarray(8))});
 };
 for(const product of products){
  if(!product.display||!product.model?.url||product.id==='m5stack-papermono')continue;
  const row={id:product.id,rays:[],errors:[]};rows.push(row);let model;
  try{
   model=createManufacturerModel(product,new THREE.Texture());const meta=await model.ready;model.group.updateMatrixWorld(true);
   if(!model.screen&&/led-matrix/.test(product.display.type)){row.scope='Per-LED matrix; no rectangular framebuffer screen';row.skipped=true;continue;}
   if(!model.screen){row.errors.push('Declared display has no generic screen mesh');continue;}
   const screen=model.screen,normal=new THREE.Vector3(0,0,1).transformDirection(screen.matrixWorld),[w,h]=meta.presentation.screen.sizeMm;
   for(const x of [-.35,0,.35])for(const y of [-.35,0,.35]){
    const point=screen.localToWorld(new THREE.Vector3(x*w,y*h,0));
    const hit=pickVisibleSurface(new THREE.Raycaster(point.clone().addScaledVector(normal,1000),normal.clone().negate(),0,2000),model.group);
    row.rays.push({normalized:[x,y],firstSurface:hit?.object.name||null,screenVisible:hit?.object===screen});
   }
  }catch(error){row.errors.push(error.message);}finally{model?.dispose();}
 }
}finally{globalThis.fetch=old.fetch;globalThis.document=old.document;THREE.TextureLoader.prototype.loadAsync=old.load;}
const result={scope:'Nine CPU rays across each generic display, using production pickVisibleSurface and decoded PNG/WebP alpha. Obstruction is a review candidate, not a calibrated photo or browser render verdict. Dedicated renderers omitted.',products:rows.length,ledMatrixNotExercised:rows.filter(r=>r.skipped).length,rays:rows.reduce((n,r)=>n+r.rays.length,0),obstructed:rows.filter(r=>r.rays.some(x=>!x.screenVisible)),errors:rows.filter(r=>r.errors.length),results:rows};
await writeFile(process.argv[2],JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify({products:result.products,rays:result.rays,obstructed:result.obstructed.map(r=>({id:r.id,surfaces:[...new Set(r.rays.filter(x=>!x.screenVisible).map(x=>x.firstSurface))]})),errors:result.errors}));
