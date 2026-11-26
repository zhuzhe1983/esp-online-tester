// Actual local meshes and production picking with source-image alpha; no GPU rendering.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
import {pickVisibleSurface} from '../web/platform/scene-interaction.js';
import {Raycaster,Vector3,Texture,TextureLoader,Mesh,PlaneGeometry,MeshBasicMaterial} from '../web/platform/vendor/three.module.min.js';
const base=new URL('../web/platform/',import.meta.url);
const catalog=JSON.parse(await readFile(new URL('catalog/products.json',base),'utf8'));
const product=id=>catalog.items.find(p=>p.id===id);
const originalFetch=globalThis.fetch,originalLoad=TextureLoader.prototype.loadAsync,originalDocument=globalThis.document;
// Same PIL alpha decoding and canvas boundary as the full production-picking audit.
// The production picker, rather than this test, applies map UV transforms/alphaTest.
function canvas(){const c={width:1,height:1,qaUniformAlpha:255};const context={fillText(){},fillRect(){},clearRect(){},drawImage(image){c.drawnImage=image;},getImageData(){const im=c.drawnImage||c;return{data:new Proxy({},{get:(_,key)=>typeof key==='string'&&/^\d+$/.test(key)?Number(key)%4===3?(im.qaAlpha?.[Math.floor(Number(key)/4)]??im.qaUniformAlpha??255):255:undefined})};}};c.getContext=()=>context;return c;}
const rows=[];
try {
  globalThis.fetch=async input=>new Response(await readFile(new URL(input)));
  globalThis.document={createElement:canvas};
  TextureLoader.prototype.loadAsync=async url=>{
    const raw=execFileSync('python',['-c',"import sys,struct;from PIL import Image;im=Image.open(sys.argv[1]);a=('A' in im.getbands())or('transparency' in im.info);sys.stdout.buffer.write(struct.pack('<IIB',*im.size,0 if a else 255));sys.stdout.buffer.write(im.convert('RGBA').getchannel('A').tobytes()if a else b'')",fileURLToPath(url)],{maxBuffer:128*1024*1024});
    const image={width:raw.readUInt32LE(0),height:raw.readUInt32LE(4),qaUniformAlpha:raw[8]};
    if(raw.length>9)image.qaAlpha=new Uint8Array(raw.buffer,raw.byteOffset+9,raw.length-9);
    return new Texture(image);
  };
  for(const id of ['m5stack-k129','m5stack-k129-v11','m5stack-k125','m5stack-k152','m5stack-k146']){
    const p=product(id),meta=JSON.parse(await readFile(new URL(p.model.url,base),'utf8'));
    for(const name of [meta.binary,...meta.parts.map(p=>p.texture?.file),...meta.derivedParts.filter(p=>['label','photo-surface'].includes(p.kind)).map(p=>p.texture)].filter(Boolean)){
      const bytes=await readFile(new URL(name,new URL(p.model.url,base)));
      assert.equal(createHash('sha256').update(bytes).digest('hex'),meta.artifacts[name],`${id} ${name} actual source bytes`);
    }
    const model=createManufacturerModel(p,new Texture());await model.ready;model.group.updateMatrixWorld(true);
    if(id==='m5stack-k152'){
      const keys=meta.parts.filter(part=>part.controlId?.startsWith('key-r'));
      assert.equal(keys.length,56);assert.equal(new Set(keys.map(k=>k.controlId)).size,56);
      assert.equal(p.controls.filter(c=>c.type==='matrix-key').length,56);assert.equal(p.runtime.board,'cardputer-adv');
      for(const key of keys){
        const [min,max]=key.boundsMm,x=(min[0]+max[0])/2,y=(min[1]+max[1])/2;
        const ray=new Raycaster(new Vector3(x,y,100),new Vector3(0,0,-1));
        const first=pickVisibleSurface(ray,model.group);
        assert.equal(first?.object.userData.control,key.controlId,`${key.controlId} nearest actual visible mesh`);
        if(key.controlId==='key-r0-c0'){
          const raw=ray.intersectObject(model.group,true).find(h=>h.object.userData.inputOccluder);
          assert.equal(raw?.object.name,'front-mesh-paper','Retain the raw-ray transparent-paper false negative');
          assert.equal(raw.object.userData.control,undefined,'Product-name paper must not acquire a key identity');
          // An opaque cover in the same ray must still block the real key. No
          // control-only raycast or blanket paper/occluder skipping can pass this.
          const blocker=new Mesh(new PlaneGeometry(4,4),new MeshBasicMaterial({color:0x222222}));
          blocker.name='opaque-key-occlusion-negative-control';blocker.position.set(x,y,30);blocker.userData.inputOccluder=true;model.group.add(blocker);
          try {assert.equal(pickVisibleSurface(ray,model.group)?.object,blocker,'Opaque foreground must reject the key');}
          finally {blocker.removeFromParent();blocker.geometry.dispose();blocker.material.dispose();}
        }
      }
      const paper=meta.derivedParts.find(part=>part.id==='front-mesh-paper');
      const photo=meta.textures.find(texture=>texture.file===paper.texture);
      const [left,top,right,bottom]=photo.cropXYXY,[width,height]=paper.geometry.sizeMm,[cx,cy]=paper.geometry.centerMm;
      const printedPoint=new Vector3(cx+((290.5-left)/(right-left)-.5)*width,cy+(.5-(503.5-top)/(bottom-top))*height,100);
      const printedHit=pickVisibleSurface(new Raycaster(printedPoint,new Vector3(0,0,-1)),model.group);
      assert.equal(printedHit?.object.name,'front-mesh-paper','Opaque source-paper pixels remain real occluders');
      assert.equal(printedHit.object.userData.control,undefined,'Printed housing paper is not a key');
      for(const shell of model.group.userData.shellMeshes)applyShellAppearance(shell,{shell:{color:'#8054b3',opacity:.35,finish:'plastic'}});
      for(const key of keys)assert.equal(model.parts[key.name].material.color.getHexString(),key.color.slice(1),'Purple shell preserves key material');
      for(const shell of model.group.userData.shellMeshes)applyShellAppearance(shell,{shell:{color:'#d6ae52',opacity:1,finish:'metal'}});
      for(const key of keys)assert.equal(model.parts[key.name].material.color.getHexString(),key.color.slice(1),'Gold shell preserves key material');
    } else if(id.startsWith('m5stack-k129')){
      const cap=model.parts[id.endsWith('v11')?'button-cap':'wake-button-shell'];
      assert.equal(cap.userData.control,'wake');assert.ok(!model.group.userData.shellMeshes.includes(cap));
      assert.deepEqual(p.controls.map(c=>[c.id,c.pin]),[['wake',42],['boot',0]]);
    } else if(id==='m5stack-k125'){
      const first=pickVisibleSurface(new Raycaster(new Vector3(0,2,100),new Vector3(0,0,-1)),model.group);
      assert.equal(first?.object.userData.control,'button-a','Photo-bounded upper flexure is exposed');
    } else {
      assert.equal(p.display.touch,true);assert.equal(p.display.touchController,'FT6336U');assert.equal(p.controls[0].pin,21);
      assert.equal(p.runtime.board,'xiaozhi-card');assert.equal(p.runtime.status,'partial');
    }
    rows.push({id,meshLoaded:true,assetHashesVerified:true});model.dispose();
  }
  console.log(JSON.stringify({products:rows,meshRaycastKeyCaps:56,productionPicker:true,sourceImageAlphaDecoded:true,transparentPaperFalseNegativeRetained:true,opaqueSourcePaperRetained:true,opaqueForegroundRejected:true,scope:'Node/Three geometry, source bytes, production alpha-aware picking and material isolation; no GPU, firmware or physical hardware acceptance.'},null,2));
} finally {globalThis.fetch=originalFetch;TextureLoader.prototype.loadAsync=originalLoad;globalThis.document=originalDocument;}
