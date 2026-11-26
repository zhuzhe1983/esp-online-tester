// Source-backed cover regions must sit directly on an exposed native flexure.
// A constructor-only control mapping, slot or occluded interior cannot pass.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {pickVisibleSurface} from '../web/platform/scene-interaction.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
import * as THREE from '../web/platform/vendor/three.module.min.js';

const base=new URL('../web/platform/',import.meta.url);
const catalog=JSON.parse(await readFile(new URL('catalog/products.json',base),'utf8'));
const ids=['m5stack-u121','m5stack-u121-b','m5stack-u121-b-v11','m5stack-u149','m5stack-u213'];
const original={fetch:globalThis.fetch,document:globalThis.document,load:THREE.TextureLoader.prototype.loadAsync};
function canvas(){const c={width:1,height:1,qaUniformAlpha:255};const context={fillText(){},fillRect(){},clearRect(){},drawImage(image){c.drawnImage=image;},getImageData(){const im=c.drawnImage||c;return{data:new Proxy({},{get:(_,key)=>typeof key==='string'&&/^\d+$/.test(key)?Number(key)%4===3?(im.qaAlpha?.[Math.floor(Number(key)/4)]??im.qaUniformAlpha??255):255:undefined})};}};c.getContext=()=>context;return c;}
const rows=[];
try {
  globalThis.fetch=async input=>new Response(await readFile(new URL(input)));
  globalThis.document={createElement:canvas};
  THREE.TextureLoader.prototype.loadAsync=async url=>{
    const raw=execFileSync('python',['-c',"import sys,struct;from PIL import Image;im=Image.open(sys.argv[1]);a=('A' in im.getbands())or('transparency' in im.info);sys.stdout.buffer.write(struct.pack('<IIB',*im.size,0 if a else 255));sys.stdout.buffer.write(im.convert('RGBA').getchannel('A').tobytes()if a else b'')",fileURLToPath(url)],{maxBuffer:128*1024*1024});
    const image={width:raw.readUInt32LE(0),height:raw.readUInt32LE(4),qaUniformAlpha:raw[8]};if(raw.length>9)image.qaAlpha=new Uint8Array(raw.buffer,raw.byteOffset+9,raw.length-9);return new THREE.Texture(image);
  };
  for(const id of ids){
    const product=catalog.items.find(p=>p.id===id),model=createManufacturerModel(product,new THREE.Texture());
    try {
      const meta=await model.ready,plan=JSON.parse(await readFile(new URL('model-plan.json',new URL(product.model.url,base))));
      const region=meta.presentation.buttons.find(b=>b.id===(id==='m5stack-u213'?'user':'button'));
      assert.ok(region,`${id} source-backed region`);
      const control=product.controls.find(c=>c.id===region.id);
      assert.equal(control?.pin??control?.gpio,id==='m5stack-u213'?45:id==='m5stack-u149'?39:37,`${id} catalog GPIO identity`);
      assert.deepEqual(plan.presentation.buttons,meta.presentation.buttons,`${id} offline plan matches`);
      assert.ok(region.sourceIds.includes('manufacturer-cad')&&region.sourceIds.length>=2,`${id} CAD and exact-SKU photograph`);
      const mesh=model.parts[`cover-key:${region.id}`],native=model.parts[region.sourceSurface.part];
      assert.ok(native?.isMesh&&meta.parts.some(p=>p.name===native.name),`${id} retained native support`);
      assert.equal(mesh.material.opacity,0);assert.equal(mesh.userData.inputOccluder,true);
      model.group.updateMatrixWorld(true);
      const normal=new THREE.Vector3(0,0,1).transformDirection(mesh.matrixWorld),samples=[];
      // Cover the region up to 0.00002 mm inside all four edges/corners, avoiding
      // Float32 ray/triangle boundary rounding. Removing the region must reveal
      // its native exterior within 0.04 mm, never a hole.
      for(let x=0;x<=8;x++)for(let y=0;y<=8;y++){
        const point=mesh.localToWorld(new THREE.Vector3((x/8-.5)*region.sizeMm[0]*.99999,(y/8-.5)*region.sizeMm[1]*.99999,0));
        const ray=new THREE.Raycaster(point.clone().addScaledVector(normal,100),normal.clone().negate());
        mesh.visible=false;const hit=pickVisibleSurface(ray,model.group);mesh.visible=true;
        assert.equal(hit?.object?.name,native.name,`${id} native exterior at ${x},${y}`);
        const gap=point.clone().sub(hit.point).dot(normal);
        assert.ok(gap>=.002&&gap<=.04,`${id} source surface gap ${gap}`);
        assert.ok(hit.face.normal.clone().transformDirection(native.matrixWorld).dot(normal)>.99,`${id} native face orientation`);
        assert.equal(pickVisibleSurface(ray,model.group)?.object?.name,mesh.name,`${id} actual nearest control at ${x},${y}`);
        samples.push({gap,faceIndex:hit.faceIndex});
      }
      const center=mesh.localToWorld(new THREE.Vector3()),ray=new THREE.Raycaster(center.clone().addScaledVector(normal,100),normal.clone().negate());
      const blocker=new THREE.Mesh(new THREE.PlaneGeometry(...region.sizeMm),new THREE.MeshBasicMaterial({color:0x222222}));
      blocker.position.copy(center).addScaledVector(normal,1);blocker.quaternion.copy(mesh.getWorldQuaternion(new THREE.Quaternion()));model.group.add(blocker);
      try{assert.equal(pickVisibleSurface(ray,model.group)?.object?.uuid,blocker.uuid,`${id} opaque foreground blocks button`);}finally{blocker.removeFromParent();blocker.geometry.dispose();blocker.material.dispose();}
      for(const shell of [{color:'#d4af37',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}]){
        for(const surface of model.group.userData.shellMeshes)applyShellAppearance(surface,{shell});
        assert.equal(pickVisibleSurface(ray,model.group)?.object?.name,mesh.name,`${id} actual control survives shell preset`);
      }
      const group=meta.presentation.inspectionGroups.find(g=>g.parts.includes(region.sourceSurface.part));
      assert.ok(group.parts.includes(mesh.name),`${id} hotspot moves with native cover`);
      assert.equal(model.group.userData.inspectionError,undefined,`${id} production inspection accepted`);
      model.group.userData.inspection.setProgress(1);model.group.updateMatrixWorld(true);
      const inspectionPoint=mesh.localToWorld(new THREE.Vector3()),inspectionRay=new THREE.Raycaster(inspectionPoint.clone().addScaledVector(normal,100),normal.clone().negate());
      assert.equal(pickVisibleSurface(inspectionRay,model.group)?.object?.name,mesh.name,`${id} inspection control remains on its cover`);
      mesh.visible=false;assert.equal(pickVisibleSurface(inspectionRay,model.group)?.object?.name,native.name,`${id} inspection exposes native support`);mesh.visible=true;
      if(id==='m5stack-u149'){
        model.group.userData.inspection.setProgress(0);model.group.updateMatrixWorld(true);
        const aperture=pickVisibleSurface(new THREE.Raycaster(new THREE.Vector3(0,5,100),new THREE.Vector3(0,0,-1)),model.group);
        assert.notEqual(aperture?.object?.userData.control,'button','The RGB aperture must not become a button surface');
      }
      rows.push({productId:id,controlId:region.id,nativePart:native.name,gridRays:samples.length,minimumGapMm:Math.min(...samples.map(s=>s.gap)),maximumGapMm:Math.max(...samples.map(s=>s.gap)),nativeFaceIndices:[...new Set(samples.map(s=>s.faceIndex))].sort((a,b)=>a-b),opaqueForegroundRejected:true,inspectionFollowsNativeCover:true,shellPresets:['original','gold','translucent-purple']});
    }finally{model.dispose();}
  }
  console.log(JSON.stringify({products:rows,scope:'Actual production alpha-aware picking of native exterior plus invisible source-bound input region. No new visible geometry, GPU rendering, firmware or physical calibration claim.'},null,2));
}finally{globalThis.fetch=original.fetch;globalThis.document=original.document;THREE.TextureLoader.prototype.loadAsync=original.load;}
