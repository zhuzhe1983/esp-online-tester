// Actual side-actuator geometry, cut opening and production occlusion; no GPU.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {pickVisibleSurface} from '../web/platform/scene-interaction.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
import * as T from '../web/platform/vendor/three.module.min.js';
const base=new URL('../web/platform/',import.meta.url),catalog=JSON.parse(await readFile(new URL('catalog/products.json',base))),product=catalog.items.find(p=>p.id==='m5stack-k140');
const previous={fetch:globalThis.fetch,document:globalThis.document,load:T.TextureLoader.prototype.loadAsync};
function canvas(){const c={width:1,height:1,qaUniformAlpha:255};c.getContext=()=>({fillText(){},drawImage(image){c.image=image;},getImageData(){const im=c.image||c;return{data:new Proxy({},{get:(_,k)=>typeof k==='string'&&/^\d+$/.test(k)?Number(k)%4===3?(im.qaAlpha?.[Math.floor(Number(k)/4)]??im.qaUniformAlpha??255):255:undefined})};}});return c;}
let model;
try{
 globalThis.fetch=async u=>new Response(await readFile(new URL(u)));globalThis.document={createElement:canvas};
 T.TextureLoader.prototype.loadAsync=async url=>{const raw=execFileSync('python',['-c',"import sys,struct;from PIL import Image;im=Image.open(sys.argv[1]);a=('A' in im.getbands())or('transparency'in im.info);sys.stdout.buffer.write(struct.pack('<IIB',*im.size,0 if a else 255));sys.stdout.buffer.write(im.convert('RGBA').getchannel('A').tobytes()if a else b'')",fileURLToPath(url)]);const im={width:raw.readUInt32LE(0),height:raw.readUInt32LE(4),qaUniformAlpha:raw[8]};if(raw.length>9)im.qaAlpha=new Uint8Array(raw.buffer,raw.byteOffset+9,raw.length-9);return new T.Texture(im);};
 model=createManufacturerModel(product,new T.Texture());const meta=await model.ready;model.group.updateMatrixWorld(true);
 const cap=model.parts['user-button-cap'],spec=meta.derivedParts.find(p=>p.id===cap.name);
 assert.equal(product.controls.find(c=>c.id==='button').pin,7);assert.equal(cap.userData.control,'button');assert.ok(spec.sourceIds.includes('button-dimension-drawing')&&spec.sourceIds.includes('photo-3'));assert.equal(cap.material.opacity,1);assert.ok(!model.group.userData.shellMeshes.includes(cap),'Orange cap is not shell material');
 let rays=0;for(let x=0;x<7;x++)for(let z=0;z<5;z++){
  const ray=new T.Raycaster(new T.Vector3(-3+x,100,-6.3+z*.6),new T.Vector3(0,-1,0)),hit=pickVisibleSurface(ray,model.group);
  assert.equal(hit?.object?.name,cap.name,`Actual side cap at ${x},${z}`);assert.equal(hit.object.userData.control,'button');
  cap.visible=false;const behind=pickVisibleSurface(ray,model.group);cap.visible=true;
  assert.ok(!behind||behind.point.y<14.5,`Actual side wall opening at ${x},${z}`);rays++;
 }
 const front=pickVisibleSurface(new T.Raycaster(new T.Vector3(0,15.8,100),new T.Vector3(0,0,-1)),model.group);
 assert.equal(front?.object?.userData.control,undefined,'The front wall must occlude the side cap');assert.ok(front.point.z>0);
 const rear=pickVisibleSurface(new T.Raycaster(new T.Vector3(0,-100,-5.1),new T.Vector3(0,1,0)),model.group);
 assert.equal(rear?.object?.userData.control,undefined,'The opposite wall must occlude the cap');assert.ok(rear.point.y<0);
 const notch=pickVisibleSurface(new T.Raycaster(new T.Vector3(0,14,100),new T.Vector3(0,0,-1)),model.group);
 assert.ok(!notch||notch.point.z<12.6,'Front U-notch is genuinely cut, including paper alpha');
 const color=cap.material.color.getHexString(),ray=new T.Raycaster(new T.Vector3(0,100,-5.1),new T.Vector3(0,-1,0));
 for(const shell of [{color:'#d4af37',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}]){
  for(const surface of model.group.userData.shellMeshes)applyShellAppearance(surface,{shell});
  assert.equal(cap.material.color.getHexString(),color);assert.equal(cap.material.opacity,1);assert.equal(pickVisibleSurface(ray,model.group)?.object?.name,cap.name);
 }
 console.log(JSON.stringify({productId:product.id,controlId:'button',gpio:7,actualSideCapRays:rays,actualWallOpeningRays:rays,frontWallOccludes:true,oppositeWallOccludes:true,frontUNotchCut:true,orangeMaterialIsolated:true,shellPresets:['original','gold','translucent-purple'],scope:'Production picker with real source-image alpha and reconstructed approximate exterior cap/opening. Not official CAD, hardware calibration, firmware or GPU acceptance.'},null,2));
}finally{model?.dispose();globalThis.fetch=previous.fetch;globalThis.document=previous.document;T.TextureLoader.prototype.loadAsync=previous.load;}
