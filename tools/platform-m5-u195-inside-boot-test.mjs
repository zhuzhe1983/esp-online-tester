// BOOT is a real internal tact switch; assembled-case API input is not a click-through.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {pickVisibleSurface} from '../web/platform/scene-interaction.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
import * as T from '../web/platform/vendor/three.module.min.js';
const base=new URL('../web/platform/',import.meta.url),catalog=JSON.parse(await readFile(new URL('catalog/products.json',base))),product=catalog.items.find(p=>p.id==='m5stack-u195');
const old={fetch:globalThis.fetch,document:globalThis.document,load:T.TextureLoader.prototype.loadAsync};
function canvas(){const c={width:1,height:1,qaUniformAlpha:255};c.getContext=()=>({fillText(){},drawImage(image){c.image=image;},getImageData(){const im=c.image||c;return{data:new Proxy({},{get:(_,k)=>typeof k==='string'&&/^\d+$/.test(k)?Number(k)%4===3?(im.qaAlpha?.[Math.floor(Number(k)/4)]??im.qaUniformAlpha??255):255:undefined})};}});return c;}
let model;
try{
 globalThis.fetch=async u=>new Response(await readFile(new URL(u)));globalThis.document={createElement:canvas};
 T.TextureLoader.prototype.loadAsync=async url=>{const raw=execFileSync('python',['-c',"import sys,struct;from PIL import Image;im=Image.open(sys.argv[1]);a=('A'in im.getbands())or('transparency'in im.info);sys.stdout.buffer.write(struct.pack('<IIB',*im.size,0 if a else 255));sys.stdout.buffer.write(im.convert('RGBA').getchannel('A').tobytes()if a else b'')",fileURLToPath(url)]);const im={width:raw.readUInt32LE(0),height:raw.readUInt32LE(4),qaUniformAlpha:raw[8]};if(raw.length>9)im.qaAlpha=new Uint8Array(raw.buffer,raw.byteOffset+9,raw.length-9);return new T.Texture(im);};
 model=createManufacturerModel(product,new T.Texture());const meta=await model.ready;model.group.updateMatrixWorld(true);
 const cap=model.parts['inside-boot'],body=model.parts['inside-boot-body'],spec=meta.derivedParts.find(p=>p.id===cap.name),center=cap.getWorldPosition(new T.Vector3()),target=center.clone().add(new T.Vector3(0,0,.2));
 assert.equal(product.controls.find(c=>c.id==='boot').pin,9);assert.equal(cap.userData.control,'boot');assert.ok(spec.sourceIds.includes('photo-3'));assert.ok(!model.group.userData.shellMeshes.includes(cap)&&!model.group.userData.shellMeshes.includes(body));assert.equal(model.group.userData.inspectionError,undefined);
 const directions=[[0,0,1],[0,0,-1],[1,0,0],[-1,0,0],[0,1,0],[0,-1,0],[-.97,0,.25]],closed=[];
 for(const d of directions){const dir=new T.Vector3(...d).normalize(),hit=pickVisibleSurface(new T.Raycaster(target.clone().addScaledVector(dir,100),dir.clone().negate()),model.group);assert.notEqual(hit?.object?.userData.control,'boot','Opaque native closed case blocks internal BOOT');assert.ok(['shell-front','shell-back','front-paper','rear-paper'].includes(hit?.object?.name));closed.push({direction:d,occluder:hit.object.name,hitPointMm:hit.point.toArray()});}
 model.group.userData.inspection.setProgress(1);model.group.updateMatrixWorld(true);
 let openRays=0;const direction=new T.Vector3(-.97,0,.25).normalize();
 for(let x=0;x<5;x++)for(let y=0;y<5;y++){
  // Avoid exact radial fan-triangle edges of the Three cylinder cap.
  const point=target.clone().add(new T.Vector3((x-2)*.2+.031,(y-2)*.2+.037,0)),ray=new T.Raycaster(point.clone().addScaledVector(direction,100),direction.clone().negate());
  const hit=pickVisibleSurface(ray,model.group);
  assert.equal(hit?.object?.name,'inside-boot',`Inspection gap reaches real internal cap at ${x},${y}: ${JSON.stringify({point:point.toArray(),hit:hit?.point.toArray()})}`);openRays++;
 }
 const colors=[cap,body].map(m=>m.material.color.getHexString()),ray=new T.Raycaster(target.clone().addScaledVector(direction,100),direction.clone().negate());
 for(const shell of [{color:'#d4af37',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}]){
  for(const surface of model.group.userData.shellMeshes)applyShellAppearance(surface,{shell});
  assert.deepEqual([cap,body].map(m=>m.material.color.getHexString()),colors);assert.equal(pickVisibleSurface(ray,model.group)?.object?.name,'inside-boot');
  model.group.userData.inspection.setProgress(0);model.group.updateMatrixWorld(true);assert.notEqual(pickVisibleSurface(ray,model.group)?.object?.userData.control,'boot','Even recolored/translucent native shell blocks the closed cap');model.group.userData.inspection.setProgress(1);model.group.updateMatrixWorld(true);
 }
 console.log(JSON.stringify({productId:product.id,controlId:'boot',gpio:9,closedCaseOccluders:closed,inspectionCapRays:openRays,capAndBodyMaterialIsolated:true,shellPresets:['original','gold','translucent-purple'],scope:'Source-photo-estimated local PCB switch. Existing native enclosure retained and blocks assembled-case 3D input. Inspection-only oblique access and API input do not establish physical service access, measured PCB layout, firmware or hardware acceptance.'},null,2));
}finally{model?.dispose();globalThis.fetch=old.fetch;globalThis.document=old.document;T.TextureLoader.prototype.loadAsync=old.load;}
