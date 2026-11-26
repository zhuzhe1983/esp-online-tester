import assert from 'node:assert/strict';
import {partitionTextureTriangle} from '../web/platform/projected-texture.js';
const vertex=(x,y,z=2)=>[x,y,z,0,0,1];
const area=values=>{let total=0;for(let i=0;i<values.length;i+=18){const [ax,ay,az]=values.slice(i,i+3),[bx,by,bz]=values.slice(i+6,i+9),[cx,cy,cz]=values.slice(i+12,i+15);total+=Math.hypot((by-ay)*(cz-az)-(bz-az)*(cy-ay),(bz-az)*(cx-ax)-(bx-ax)*(cz-az),(bx-ax)*(cy-ay)-(by-ay)*(cx-ax))/2;}return total;};
for(const triangle of [[...vertex(-1,0),...vertex(2,0),...vertex(.5,2)],[...vertex(-2,-2),...vertex(3,-2),...vertex(.5,3)],[...vertex(-1,0),...vertex(0,0),...vertex(0,1)],[...vertex(.1,.1),...vertex(.9,.1),...vertex(.5,.9)],[...vertex(2,2),...vertex(3,2),...vertex(2,3)]]){
 const {inside,outside}=partitionTextureTriangle(triangle,[0,0,1,1]);
 assert.ok(Math.abs(area(inside)+area(outside)-area(triangle))<1e-12,'partition preserves CAD triangle area');
 for(let i=0;i<inside.length;i+=6){assert.ok(inside[i]>=0&&inside[i]<=1&&inside[i+1]>=0&&inside[i+1]<=1);assert.equal(inside[i+2],2);assert.deepEqual(inside.slice(i+3,i+6),[0,0,1]);}
}
const sloped=[...vertex(-1,0,0),...vertex(2,0,3),...vertex(.5,2,1.5)],split=partitionTextureTriangle(sloped,[0,0,1,1]);
for(let i=0;i<split.inside.length;i+=6)assert.ok(Math.abs(split.inside[i+2]-split.inside[i]-1)<1e-12,'new Z follows the source triangle rather than flattening it');
assert.ok(Math.abs(area(split.inside)+area(split.outside)-area(sloped))<1e-12);
console.log('ok projected texture partition: rectangle crossings, disjoint/boundary triangles, area and attribute preservation');

const {createManufacturerModel}=await import('../web/platform/manufacturer-model.js');
const THREE=await import('../web/platform/vendor/three.module.min.js');
const nativeFetch=globalThis.fetch,nativeLoad=THREE.TextureLoader.prototype.loadAsync;
try {
 THREE.TextureLoader.prototype.loadAsync=async()=>new THREE.Texture();
 for(const side of ['front','back'])for(const mode of [undefined,'decal']){
  const sign=side==='back'?-1:1,values=new Float32Array([...vertex(-1,0),...vertex(2,0),...vertex(.5,2)]);
  for(let i=5;i<values.length;i+=6)values[i]=sign;
  const metadata={unit:'mm',strideFloats:6,binary:'geometry.bin',parts:[{name:'case',role:'shell',byteOffset:0,vertexCount:3,texture:{file:'photo.png',projection:'xy',boundsMm:[0,0,1,1],side,mode}}]};
  globalThis.fetch=async url=>new Response(new URL(url).pathname.endsWith('model.json')?JSON.stringify(metadata):values.buffer);
  const model=createManufacturerModel({id:'projection-fixture',model:{url:'./assets/projection-fixture/model.json'}},null);await model.ready;
  const photo=model.group.getObjectByName('case-photo-surface'),uv=photo.geometry.getAttribute('uv'),painted=photo.geometry.getAttribute('position'),plain=model.parts.case.geometry.getAttribute('position');
  assert.ok(uv.count>3,'a crossing triangle is actually clipped into bounded fragments');
  for(let i=0;i<uv.count;i++){
   assert.ok(uv.getX(i)>=0&&uv.getX(i)<=1&&uv.getY(i)>=0&&uv.getY(i)<=1,'all projected texture coordinates stay within the source image');
   assert.ok(Math.abs(uv.getX(i)-(side==='back'?1-painted.getX(i):painted.getX(i)))<1e-7,'backside projection retains the documented X mirror');
  }
  const interleaved=attribute=>{const output=[];for(let i=0;i<attribute.count;i++)output.push(attribute.getX(i),attribute.getY(i),attribute.getZ(i),0,0,sign);return output;};
  const baseArea=area(interleaved(plain)),photoArea=area(interleaved(painted)),sourceArea=area(Array.from(values));
  assert.ok(Math.abs((mode==='decal'?baseArea:baseArea+photoArea)-sourceArea)<1e-6,'unpainted geometry is preserved, and decal bases remain complete');
  assert.ok(photoArea>0&&photoArea<sourceArea,'photo footprint cannot cover the entire crossing triangle');model.dispose();
 }
} finally {globalThis.fetch=nativeFetch;THREE.TextureLoader.prototype.loadAsync=nativeLoad;}
console.log('ok actual manufacturer loader: bounded front/back UVs and source-area preservation in replacement/decal modes');
const {readFile}=await import('node:fs/promises'),{fileURLToPath}=await import('node:url');
try {
 globalThis.fetch=async url=>{try{return new Response(await readFile(fileURLToPath(url)));}catch{return new Response('',{status:404});}};
 THREE.TextureLoader.prototype.loadAsync=async url=>{await readFile(fileURLToPath(url));return new THREE.Texture();};
 const product={id:'m5stack-k138',controls:[{id:'button',type:'button'}],model:{url:'./assets/m5stack-k138/model.json'}};
 const model=createManufacturerModel(product,null);const metadata=await model.ready,sourcePart=metadata.parts.find(p=>p.name==='pcb-main'),painted=model.group.getObjectByName('pcb-main-photo-surface');
 assert.equal(model.parts['pcb-main'].geometry.getAttribute('position').count,sourcePart.vertexCount,'StampFly retains the complete native PCB below its decal');
 assert.ok(painted);const uv=painted.geometry.getAttribute('uv');
 for(let i=0;i<uv.count;i++)assert.ok(uv.getX(i)>=0&&uv.getX(i)<=1&&uv.getY(i)>=0&&uv.getY(i)<=1,'actual StampFly texture has no ClampToEdge overrun');
 assert.equal(metadata.parts.find(p=>p.name==='pcb-main').texture.boundsMm[0],-20.233463035019454,'photo scale and footprint remain unchanged');model.dispose();
} finally {globalThis.fetch=nativeFetch;THREE.TextureLoader.prototype.loadAsync=nativeLoad;}
console.log('ok actual StampFly model: complete source PCB and bounded photograph coordinates');
