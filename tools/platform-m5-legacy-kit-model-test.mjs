import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile,mkdtemp,rm} from 'node:fs/promises';
import {existsSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {execFileSync} from 'node:child_process';
const root=new URL('../',import.meta.url),sourceRoot=process.env.M5_LEGACY_SOURCE_ROOT?pathToFileURL(process.env.M5_LEGACY_SOURCE_ROOT.replace(/\/$/,'')+'/'):root,platform=new URL('web/platform/',sourceRoot);
const THREE=await import(new URL('web/platform/vendor/three.module.min.js',sourceRoot)),{createManufacturerModel}=await import(new URL('web/platform/manufacturer-model.js',sourceRoot)),{applyShellAppearance}=await import(new URL('web/platform/shell-appearance.js',sourceRoot));
const products=JSON.parse(await readFile(new URL('catalog/products.json',platform))).items;
const ids=['m5stack-k016-b','m5stack-k016-t','m5stack-k022','m5stack-k006','m5stack-k007'];
test('all five actual old-SKU assets load and inspection restores source transforms',async()=>{
 const fetch=globalThis.fetch,load=THREE.TextureLoader.prototype.loadAsync;
 try{
  globalThis.fetch=async value=>{const url=new URL(value);url.search='';return new Response(await readFile(url));};
  THREE.TextureLoader.prototype.loadAsync=async()=>new THREE.Texture();
  for(const id of ids){
   const product=products.find(p=>p.id===id);assert(product);const model=createManufacturerModel(product,null);
   try{await model.ready;assert.equal(model.group.userData.inspectionError,undefined,id);model.group.updateMatrixWorld(true);
    const inspection=model.group.userData.inspection;assert(inspection);const board=model.parts['pcb-main'];const initial=model.group.children.map(p=>[p,p.position.clone()]);
    const glass=model.group.getObjectByName('live-display');assert(glass,'live framebuffer surface exists');const glassMaterial=glass.material;
    for(const color of ['#d7aa31','#8054b3'])for(const shell of model.group.userData.shellMeshes)applyShellAppearance(shell,{shell:{color,opacity:color==='#8054b3'?.35:1,finish:'plastic'}});
    assert.equal(glass.material,glassMaterial);assert.equal(glass.material.opacity,1);
    inspection.setExpanded(true);assert.deepEqual(board.position.toArray(),initial.find(([p])=>p===board)[1].toArray());inspection.setExpanded(false);
    for(const[p,v]of initial)assert.deepEqual(p.position.toArray(),v.toArray(),id+' restores exact assembled coordinates');
    if(['m5stack-k006','m5stack-k022','m5stack-k007'].includes(id))for(const key of model.buttons){
     const at=key.getWorldPosition(new THREE.Vector3()),ray=new THREE.Raycaster(new THREE.Vector3(at.x,at.y,100),new THREE.Vector3(0,0,-1));
     assert.equal(ray.intersectObject(model.group,true)[0].object,key,'visible native key receives the nearest hit');
    }
   }finally{model.dispose();}
  }
 }finally{globalThis.fetch=fetch;THREE.TextureLoader.prototype.loadAsync=load;}
});
test('offline public builder reproduces every geometry byte from fixed inputs',async()=>{
 const scratch=await mkdtemp(join(tmpdir(),'m5-legacy-rebuild-'));
 try{for(const id of ids){const output=join(scratch,id),args=[fileURLToPath(new URL('tools/build-m5-legacy-kit-models.py',root)),id,'--output',output,'--source-root',fileURLToPath(sourceRoot)],override=process.env.M5_LEGACY_PLAN_ROOT&&join(process.env.M5_LEGACY_PLAN_ROOT,id,'model-plan.json');if(override&&existsSync(override))args.push('--plan',override);execFileSync('python',args);
   const base=new URL('assets/'+id+'/',platform);for(const file of ['geometry.bin','model.json'])assert.deepEqual(await readFile(join(output,file)),await readFile(new URL(file,base)),id+' '+file);
  }}finally{await rm(scratch,{recursive:true,force:true});}
});
test('related K001 top preserves every vertex with only the declared rigid translation',async()=>{
 const base=JSON.parse(await readFile(new URL('assets/m5stack-k001/model.json',platform))),before=await readFile(new URL('assets/m5stack-k001/geometry.bin',platform));
 for(const id of ['m5stack-k022','m5stack-k006','m5stack-k007']){
  const model=JSON.parse(await readFile(new URL('assets/'+id+'/model.json',platform))),after=await readFile(new URL('assets/'+id+'/geometry.bin',platform));const shift=model.dimensions.depth/2-5.632286529541016;
  for(const source of base.parts){const part=model.parts.find(p=>p.name===source.name);assert.equal(part.vertexCount,source.vertexCount);
   for(let i=0;i<source.vertexCount*6;i++){const a=before.readFloatLE(source.byteOffset+i*4),b=after.readFloatLE(part.byteOffset+i*4);assert(Math.abs(b-a-(i%6===2?shift:0))<.000002,id+' never stretches manufacturer geometry');}
  }
 }
});
