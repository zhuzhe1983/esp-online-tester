#!/usr/bin/env node
// Production Three loader, nearest physical rays and assembly-only visual digests.
// No browser, renderer, guest execution or power-domain emulation is claimed.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {pathToFileURL,fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const args=process.argv.slice(2),option=(name,fallback)=>{const i=args.indexOf(name);return i<0?fallback:args[i+1];};
const repo=resolve(option('--source-root',new URL('..',import.meta.url).pathname)),base=pathToFileURL(join(repo,'web/platform/'));
const contract=JSON.parse(await readFile(resolve(option('--contract',join(repo,'docs/platform/evidence/m5-classic-control-surfaces-20261126/stick-five-contract.json')))));
const ids=option('--ids',contract.products.map(p=>p.id).join(',')).split(',');
const THREE=await import(new URL('vendor/three.module.min.js',base)),{createManufacturerModel}=await import(new URL('manufacturer-model.js',base)),{applyShellAppearance}=await import(new URL('shell-appearance.js',base));
const hash=b=>createHash('sha256').update(b).digest('hex'),catalog=JSON.parse(await readFile(new URL('catalog/products.json',base))).items;
let override=null;
globalThis.fetch=async input=>{const url=new URL(input);url.search='';return new Response(override&&url.href===override.url?JSON.stringify(override.metadata):await readFile(fileURLToPath(url)));};
globalThis.document={createElement:()=>{const canvas={width:1,height:1,qaText:[]};canvas.getContext=()=>({fillStyle:'#fff',font:'20px monospace',textAlign:'left',textBaseline:'middle',fillText(text,x,y,maxWidth){canvas.qaText.push({text,x,y,maxWidth,color:this.fillStyle,font:this.font,align:this.textAlign,baseline:this.textBaseline});}});return canvas;}};
THREE.TextureLoader.prototype.loadAsync=async url=>{const raw=await readFile(fileURLToPath(url)),texture=new THREE.Texture({width:1,height:1});texture.userData.sourceSha256=hash(raw);return texture;};
const visibleMeshes=group=>{const rows=[];group.traverse(o=>{if(!o.isMesh)return;for(let p=o;p;p=p.parent)if(!p.visible)return;rows.push(o);});return rows;};
async function make(product,metadata){override={url:new URL(product.model.url,base).href,metadata};const m=createManufacturerModel(product,new THREE.Texture());await m.ready;override=null;m.group.updateMatrixWorld(true);assert.equal(m.group.userData.inspectionError,undefined);return m;}
function attribute(a){if(!a)return null;const values=new Float32Array(a.count*a.itemSize),getters=['getX','getY','getZ','getW'];for(let i=0;i<a.count;i++)for(let c=0;c<a.itemSize;c++)values[i*a.itemSize+c]=a[getters[c]](i);return{count:a.count,itemSize:a.itemSize,sha256:hash(Buffer.from(values.buffer))};}
function visualDigest(m){m.group.updateMatrixWorld(true);const meshes=visibleMeshes(m.group).filter(o=>o.material.opacity>.001).map(o=>{const g=o.geometry,mat=o.material,map=mat.map;if(map)map.updateMatrix();return{name:o.name,world:o.matrixWorld.elements,position:attribute(g.getAttribute('position')),normal:attribute(g.getAttribute('normal')),uv:attribute(g.getAttribute('uv')),index:g.index?hash(Buffer.from(Uint32Array.from(g.index.array).buffer)):null,material:{type:mat.type,color:mat.color?.toArray(),emission:mat.emissive?.toArray(),emissiveIntensity:mat.emissiveIntensity,visible:mat.visible,transparent:mat.transparent,depthTest:mat.depthTest,blending:mat.blending,colorWrite:mat.colorWrite,toneMapped:mat.toneMapped,flatShading:mat.flatShading,wireframe:mat.wireframe,premultipliedAlpha:mat.premultipliedAlpha,opacity:mat.opacity,roughness:mat.roughness,metalness:mat.metalness,alphaTest:mat.alphaTest,side:mat.side,depthWrite:mat.depthWrite,polygonOffset:mat.polygonOffset,polygonOffsetFactor:mat.polygonOffsetFactor,polygonOffsetUnits:mat.polygonOffsetUnits,map:map?{sourceSha256:map.userData.sourceSha256??null,text:map.image?.qaText??null,width:map.image?.width,height:map.image?.height,matrix:map.matrix.elements,flipY:map.flipY,wrapS:map.wrapS,wrapT:map.wrapT,colorSpace:map.colorSpace}:null}};});return hash(JSON.stringify(meshes));}
function hit(meshes,point,direction){return new THREE.Raycaster(point.clone().addScaledVector(direction,100),direction.clone().negate()).intersectObjects(meshes,false)[0]?.object;}
const rows=[];
function ray(model,target,normal){const origin=target.clone().addScaledVector(normal,100),direction=normal.clone().negate(),h=new THREE.Raycaster(origin,direction).intersectObjects(visibleMeshes(model.group),false)[0];return h?{mesh:h.object.name,controlId:h.object.userData.control??null,pointMm:h.point.toArray(),gapMm:h.point.distanceTo(target),object:h.object}:null;}
function summary(hit){if(!hit)return null;const {object,...result}=hit;return result;}

for(const id of ids){
 const expected=contract.products.find(p=>p.id===id);assert.ok(expected);const product=catalog.find(p=>p.id===id);assert.ok(product);
 const raw=await readFile(new URL(product.model.url,base)),metadata=JSON.parse(raw);assert.equal(hash(raw),expected.finalModelSha256);assert.equal(hash(await readFile(new URL('model-plan.json',new URL(product.model.url,base)))),expected.finalPlanSha256);
 const priorRaw=execFileSync('git',['-C',repo,'show',expected.baselineRevision+':web/platform/assets/'+id+'/model.json']);assert.equal(hash(priorRaw),expected.previousModelSha256);const previous=JSON.parse(priorRaw);assert.deepEqual(metadata.artifacts,previous.artifacts);
 const normalized=execFileSync(option('--python','python'),['-c',"import json,sys,hashlib\nm=json.load(sys.stdin)\nps=sorted(json.loads(sys.argv[1]),key=lambda p:(p.count('/'),p.rsplit('/',1)[0],int(p.rsplit('/',1)[1]) if p.rsplit('/',1)[1].isdigit() else -1),reverse=True)\nfor p in ps:\n ks=p.split('/')[1:];c=m\n for k in ks[:-1]:c=c[int(k)] if isinstance(c,list) else c[k]\n if isinstance(c,list):del c[int(ks[-1])]\n else:del c[ks[-1]]\nprint(hashlib.sha256((json.dumps(m,ensure_ascii=False,indent=2)+chr(10)).encode()).hexdigest())\n",JSON.stringify(expected.removedJsonPointers)],{input:raw,encoding:'utf8'}).trim();assert.equal(normalized,expected.previousModelSha256);
 const old=await make(product,previous),model=await make(product,metadata);
 try{
  const surfaces=[],negatives=[],inspection=[];
  for(const change of expected.mutations){
   const definition=product.controls.find(c=>c.id===change.controlId);assert.ok(definition);assert.notEqual(definition.simulationSupported,false);if(change.controlId==='power'){assert.equal(definition.transport,'board');assert.equal(definition.pin,undefined);}
   const part=model.parts[change.kind==='input-region'?'cover-key:'+change.controlId:change.part];assert.ok(part);assert.equal(part.userData.control,change.controlId);assert.ok(model.buttons.includes(part));let width,height;
   if(change.kind==='input-region'){assert.equal(part.material.opacity,0);assert.equal(part.material.depthWrite,false);({width,height}=part.geometry.parameters);}
   else{const b=new THREE.Box3().setFromObject(part),size=b.getSize(new THREE.Vector3());width=size.x;height=size.y;}
   const probes=[];
   for(const [fx,fy]of[[0,0],[-.4,-.4],[.4,-.4],[-.4,.4],[.4,.4]]){
    let target,normal;if(change.kind==='input-region'){target=part.localToWorld(new THREE.Vector3(fx*width,fy*height,0));normal=new THREE.Vector3(0,0,1).applyQuaternion(part.getWorldQuaternion(new THREE.Quaternion()));}
    else{const b=new THREE.Box3().setFromObject(part),c=b.getCenter(new THREE.Vector3());target=new THREE.Vector3(c.x+fx*width,c.y+fy*height,b.max.z);normal=new THREE.Vector3(0,0,1);}
    const before=ray(old,target,normal),after=ray(model,target,normal);assert.ok(after?.object===part,change.controlId+' nearest visible surface: '+after?.mesh);
    if(change.kind==='input-region'){assert.equal(before?.mesh,change.supportingPart);assert.ok(before.gapMm<=change.maxPriorSurfaceGapMm,change.controlId+' prior native supporting surface: '+before.gapMm);}
    else assert.equal(before?.mesh,change.part);
    assert.ok(ray(model,target,normal.clone().negate())?.object!==part,'Opposite shell cannot click through');probes.push({fraction:[fx,fy],previous:summary(before),final:summary(after),eyeMm:target.clone().addScaledVector(normal,100).toArray(),normal:normal.toArray()});
   }
   if(change.kind==='input-region'){
    for(const side of[-1,1]){const target=part.localToWorld(new THREE.Vector3(0,side*.8*height,0)),normal=new THREE.Vector3(0,0,1).applyQuaternion(part.getWorldQuaternion(new THREE.Quaternion())),h=ray(model,target,normal);assert.notEqual(h?.controlId,change.controlId);negatives.push({controlId:change.controlId,kind:'outside-region',side,hit:summary(h)});}
   }
   surfaces.push({controlId:change.controlId,kind:change.kind,transport:definition.transport??'GPIO',pin:definition.pin??null,region:change.kind==='input-region'?metadata.presentation.buttons[change.regionIndex]:null,nearestSamples:probes});
  }
  for(const progress of [0.5,1,0]){for(const m of [old,model])m.group.userData.inspection?.setProgress(progress);for(const change of expected.mutations.filter(c=>c.kind==='input-region')){const part=model.parts['cover-key:'+change.controlId],target=part.localToWorld(new THREE.Vector3()),normal=new THREE.Vector3(0,0,1).applyQuaternion(part.getWorldQuaternion(new THREE.Quaternion())),before=ray(old,target,normal),after=ray(model,target,normal);assert.ok(after?.object===part,change.controlId+' inspection nearest '+after?.mesh);assert.equal(before?.mesh,change.supportingPart);assert.ok(before.gapMm<=change.maxPriorSurfaceGapMm);inspection.push({controlId:change.controlId,progress,previous:summary(before),final:summary(after)});}}
  const digests={};for(const [preset,shell]of Object.entries({original:null,gold:{color:'#d4af37',opacity:1,finish:'metal'},purple:{color:'#8054b3',opacity:.35,finish:'plastic'}})){if(shell)for(const m of [old,model])for(const object of m.group.userData.shellMeshes)applyShellAppearance(object,{shell});const a=visualDigest(old),b=visualDigest(model);assert.equal(a,b,'Visible contract in '+preset);digests[preset]={previous:a,final:b};}
  const verifiedArtifacts={};for(const [file,sha]of Object.entries(metadata.artifacts)){const actual=hash(await readFile(new URL(file,new URL(product.model.url,base))));assert.equal(actual,sha);verifiedArtifacts[file]=sha;}
  const receipt={schemaVersion:1,harnessPath:'tools/verify-m5-stick-control-surfaces.mjs',harnessSha256:hash(await readFile(fileURLToPath(import.meta.url))),productId:id,baselineRevision:expected.baselineRevision,previousModelSha256:expected.previousModelSha256,finalModelSha256:hash(raw),excludedJsonPointers:expected.removedJsonPointers,restoredPreviousMetadataSha256:normalized,finalPlanSha256:hash(await readFile(new URL('model-plan.json',new URL(product.model.url,base)))),verifiedArtifacts,productionLoaderVisualDigests:digests,enginePath:'web/platform/manufacturer-model.js',engineSha256:hash(await readFile(new URL('manufacturer-model.js',base))),inspectionEnginePath:'web/platform/model-inspection.js',inspectionEngineSha256:hash(await readFile(new URL('model-inspection.js',base))),surfaces,negativeRays:negatives,inspectionRays:inspection,passed:true,scope:'Actual production loader source-bound input surfaces and metadata-only visual equivalence. Source-photo estimates remain explicit, native internal pieces are not made exterior controls. Physical power-off/wake, guest execution and new rendering are not claimed.'};
  rows.push(receipt);if(option('--output-dir'))await writeFile(join(option('--output-dir'),id+'-assembled-equivalence.json'),JSON.stringify(receipt,null,2)+'\n');
 }finally{old.dispose();model.dispose();}
}
console.log(JSON.stringify({passed:true,products:rows.map(r=>({id:r.productId,previous:r.previousModelSha256,final:r.finalModelSha256,nearestSamples:r.surfaces.reduce((n,s)=>n+s.nearestSamples.length,0),inspectionRays:r.inspectionRays.length}))},null,2));
