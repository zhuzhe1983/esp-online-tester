#!/usr/bin/env node
// Read the real production loaders and picker from a caller-supplied checkout.
// No source geometry, renderer, product definition or source-tree file is changed.
import {readFile,writeFile,mkdir,access} from 'node:fs/promises';
import {resolve,join,relative,dirname} from 'node:path';
import {pathToFileURL,fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const args=process.argv.slice(2),option=(n,d)=>{const i=args.indexOf(n);return i<0?d:args[i+1];};
if(!option('--source-root'))throw Error('Usage: --source-root CHECKOUT --out JSON [--overlay-root CHECKOUT] [--ids id,id]');
const root=resolve(option('--source-root')),overlay=option('--overlay-root')?resolve(option('--overlay-root')):null,out=resolve(option('--out','m5-control-picking.json')),hash=b=>createHash('sha256').update(b).digest('hex');
const THREE=await import(pathToFileURL(join(root,'web/platform/vendor/three.module.min.js'))),{createManufacturerModel}=await import(pathToFileURL(join(root,'web/platform/manufacturer-model.js'))),{createPapermonoModel}=await import(pathToFileURL(join(root,'web/platform/papermono-model.js'))),{createStopwatchModel}=await import(pathToFileURL(join(root,'web/platform/stopwatch-model.js'))),{pickVisibleSurface}=await import(pathToFileURL(join(root,'web/platform/scene-interaction.js')));
const {applyShellAppearance}=await import(pathToFileURL(join(root,'web/platform/shell-appearance.js')));
const harnessSourceSha256=hash(await readFile(fileURLToPath(import.meta.url)));
const sourceStartCommit=execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim();
const sourceBytes=await readFile(await actualFile(join(root,'web/platform/catalog/products.json'))),products=JSON.parse(sourceBytes).items.filter(p=>p.manufacturer==='M5Stack'),ids=option('--ids')?.split(','),rows=[];
async function actualFile(file){if(overlay){const p=join(overlay,relative(root,file));try{await access(p);return p;}catch{}}return file;}
const canvas=()=>{const c={width:1,height:1,qaUniformAlpha:255,qaText:[]};const context={fillText(text,x,y,maxWidth){c.qaText.push({text,x,y,maxWidth,fillStyle:this.fillStyle,font:this.font});},fillRect(){},clearRect(){},drawImage(image){c.drawnImage=image;},getImageData(){const im=c.drawnImage||c;return{data:new Proxy({},{get:(_,key)=>typeof key==='string'&&/^\d+$/.test(key)?Number(key)%4===3?(im.qaAlpha?.[Math.floor(Number(key)/4)]??im.qaUniformAlpha??255):255:undefined})};}};c.getContext=()=>context;return c;};
const previous={fetch:globalThis.fetch,document:globalThis.document,load:THREE.TextureLoader.prototype.loadAsync};
function visible(o){for(let p=o;p;p=p.parent)if(!p.visible||p.userData.pickIgnore)return false;return true;}
function pickControl(hit){if(!hit)return null;const o=hit.object;if(o.userData.potentiometer)return null;if(o.userData.encoder){const p=o.userData.encoderPress;if(p&&o.userData.control&&hit.face?.normal.z>.7){const local=o.worldToLocal(hit.point.clone());if(Math.hypot(local.x-p.centerMm[0],local.y-p.centerMm[1])<=p.radiusMm)return o.userData.control;}return null;}return o.userData.control||null;}
function visualSignature(model){
 const meshes=[];
 model.group.traverse(mesh=>{
  if(!mesh.isMesh||!mesh.geometry||Array.isArray(mesh.material)||mesh.material.opacity<=.001)return;
  for(let p=mesh;p;p=p.parent)if(!p.visible)return;
  const attributes={};for(const name of ['position','normal','uv']){const attr=mesh.geometry.getAttribute(name);if(attr)attributes[name]={itemSize:attr.itemSize,count:attr.count,sha256:hash(Buffer.from(attr.array.buffer,attr.array.byteOffset,attr.array.byteLength))};}
  const index=mesh.geometry.index,m=mesh.material,t=m.map;if(t)t.updateMatrix();
  meshes.push({name:mesh.name,matrix:mesh.matrixWorld.elements,attributes,index:index?hash(Buffer.from(index.array.buffer,index.array.byteOffset,index.array.byteLength)):null,material:{type:m.type,color:m.color?.toArray(),emission:m.emissive?.toArray(),emissiveIntensity:m.emissiveIntensity,metalness:m.metalness,roughness:m.roughness,opacity:m.opacity,alphaTest:m.alphaTest,side:m.side,polygonOffset:m.polygonOffset,polygonOffsetFactor:m.polygonOffsetFactor,polygonOffsetUnits:m.polygonOffsetUnits,map:t?{file:t.userData.qaFile,sha256:t.userData.qaSha256,text:t.image?.qaText,size:t.image?[t.image.width,t.image.height]:null,matrix:t.matrix.elements,flipY:t.flipY,wrapS:t.wrapS,wrapT:t.wrapT,colorSpace:t.colorSpace}:null}});
 });
 return {meshCount:meshes.length,sha256:hash(JSON.stringify(meshes))};
}
function visualVariants(model){
 const result={original:visualSignature(model)},shells=model.group.userData.shellMeshes||Object.values(model.parts||{}).filter(mesh=>mesh?.isMesh&&mesh.userData.originalAppearance?.shell);
 for(const [name,preset]of Object.entries({gold:{color:'#d4af37',opacity:1,finish:'metal'},purple:{color:'#8054b3',opacity:.35,finish:'plastic'}})){
  for(const mesh of shells)applyShellAppearance(mesh,{shell:preset});result[name]=visualSignature(model);
 }
 return result;
}
function negativeRegionProbes(model,metadata,span){
 const rows=[];
 for(const region of metadata.presentation?.buttons||[]){
  const target=model.buttons.find(o=>o.name===`cover-key:${region.id}`);if(!target)continue;
  const center=target.localToWorld(new THREE.Vector3()),normal=new THREE.Vector3(0,0,1).transformDirection(target.matrixWorld),probes=[];
  // Probe outside each finite region in its own plane, not through an imagined cap.
  for(const sign of [-1,1]){const point=target.localToWorld(new THREE.Vector3(sign*(region.sizeMm[0]/2+.5),0,0)),eye=point.clone().addScaledVector(normal,span),ray=new THREE.Raycaster(eye,normal.clone().negate(),0,span*2),hit=pickVisibleSurface(ray,model.group);probes.push({eyeMm:eye.toArray(),direction:ray.ray.direction.toArray(),hitMesh:hit?.object.name??null,controlId:pickControl(hit),encoderId:hit?.object.userData.encoder??null,passed:pickControl(hit)!==region.id});}
  rows.push({controlId:region.id,regionCenterMm:center.toArray(),probes,passed:probes.every(p=>p.passed)});
 }
 return rows;
}
function regionSurfaceGates(model,metadata,span){
 const rows=[];
 for(const region of metadata.presentation?.buttons||[]){
  const target=model.buttons.find(o=>o.name===`cover-key:${region.id}`);if(!target)continue;
  const normal=new THREE.Vector3(0,0,1).transformDirection(target.matrixWorld),points=[];
  for(let u=-4;u<=4;u++)for(let v=-4;v<=4;v++){
   const point=target.localToWorld(new THREE.Vector3(u*region.sizeMm[0]/10,v*region.sizeMm[1]/10,0)),eye=point.clone().addScaledVector(normal,span),ray=new THREE.Raycaster(eye,normal.clone().negate(),0,span*2);
   if(!ray.intersectObject(target,false).length)continue;
   target.visible=false;const hit=pickVisibleSurface(ray,model.group);target.visible=true;
   points.push({pointMm:point.toArray(),physicalMesh:hit?.object.name??null,physicalPointMm:hit?.point.toArray()??null,gapMm:hit?point.distanceTo(hit.point):null});
  }
  rows.push({controlId:region.id,sampledPoints:points.length,maxGapMm:Math.max(...points.map(p=>p.gapMm??Infinity)),physicalMeshes:[...new Set(points.map(p=>p.physicalMesh))],points});
 }
 return rows;
}
function check(model,control,span,product,denseAngular=false){
 if(control.physical===false&&control.spatialInputMode==='touchscreen-shortcut'){
  const screen=model.screen||model.parts?.screen,result={id:control.id,type:control.type,transport:control.transport??null,physical:false,spatialInputMode:control.spatialInputMode,mappedMeshes:[],successfulRay:null,raysTried:0};
  if(!product.runtime?.touch||!product.controls?.some(c=>c.type==='touch'&&c.simulationSupported!==false)||!screen?.isMesh||!visible(screen)){result.status='virtual-without-real-touch-surface';return result;}
  screen.geometry.computeBoundingBox();const bounds=screen.geometry.boundingBox,width=product.display?.width,height=product.display?.height,pixel=control.touchscreenPoint;
  if(!Number.isInteger(width)||!Number.isInteger(height)||!Number.isInteger(pixel?.x)||!Number.isInteger(pixel?.y)||pixel.x<0||pixel.x>=width||pixel.y<0||pixel.y>=height){result.status='virtual-without-defined-touch-point';return result;}
  const point=screen.localToWorld(new THREE.Vector3(bounds.min.x+(pixel.x+.5)/width*(bounds.max.x-bounds.min.x),bounds.max.y-(pixel.y+.5)/height*(bounds.max.y-bounds.min.y),(bounds.min.z+bounds.max.z)/2)),normal=new THREE.Vector3(0,0,1).transformDirection(screen.matrixWorld),eye=point.clone().addScaledVector(normal,span),ray=new THREE.Raycaster(eye,normal.clone().negate(),0,span*2),hit=pickVisibleSurface(ray,model.group);result.raysTried=1;
  const native=hit?.uv?{x:Math.max(0,Math.min(width-1,Math.floor(hit.uv.x*width))),y:Math.max(0,Math.min(height-1,Math.floor((1-hit.uv.y)*height)))}:null;result.expectedNativePoint=pixel;result.actualNativePoint=native;
  if(hit?.object===screen&&hit.object.userData.screen&&native?.x===pixel.x&&native?.y===pixel.y){result.status='virtual-only-with-real-touch-surface';result.successfulRay={eyeMm:eye.toArray(),direction:ray.ray.direction.toArray(),hitMesh:hit.object.name,hitPointMm:hit.point.toArray(),dispatch:'touch; soft-button activation depends on guest firmware touch-zone configuration'};}else{result.status='virtual-touch-surface-occluded';result.blockedBy=hit?.object.name??null;}return result;
 }

 const targets=model.buttons.filter(o=>o.userData.control===control.id&&visible(o)),r={id:control.id,type:control.type,physical:control.physical??null,spatialInputMode:control.spatialInputMode??null,pin:control.pin??control.gpio??null,transport:control.transport??null,mappedMeshes:targets.map(o=>o.name),raysTried:0,successfulRay:null,blockedBy:[]};
 if(!targets.length){r.status='missing-mapping';return r;}
 const directions=[new THREE.Vector3(0,0,1),new THREE.Vector3(1,0,0),new THREE.Vector3(-1,0,0),new THREE.Vector3(0,1,0),new THREE.Vector3(0,-1,0),new THREE.Vector3(0,0,-1)];
 // Oblique camera views matter between exploded case layers; keep six axes first.
 for(const x of [-1,0,1])for(const y of [-1,0,1])for(const z of [-1,0,1])if((x!==0)+(y!==0)+(z!==0)>=2)directions.push(new THREE.Vector3(x,y,z).normalize());
 if(denseAngular)for(const elevation of [-.15,-.3,-.5,.15,.3,.5])for(let i=0;i<16;i++){const angle=i*Math.PI/8;directions.push(new THREE.Vector3(Math.cos(angle),Math.sin(angle),elevation).normalize());}
 const probe=(point,normal,target)=>{if(normal.lengthSq()<.1)return false;normal.normalize();const eye=point.clone().addScaledVector(normal,span),ray=new THREE.Raycaster(eye,normal.clone().negate(),0,span*2),hit=pickVisibleSurface(ray,model.group);r.raysTried++;if(pickControl(hit)===control.id){r.successfulRay={target:target.name,eyeMm:eye.toArray(),direction:ray.ray.direction.toArray(),hitMesh:hit.object.name,hitPointMm:hit.point.toArray(),controlId:pickControl(hit)};return true;}if(r.blockedBy.length<12)r.blockedBy.push({target:target.name,hit:hit?.object?.name??null,control:pickControl(hit)});return false;};
 for(const target of targets){target.geometry.computeBoundingBox();const center=target.localToWorld(target.geometry.boundingBox.getCenter(new THREE.Vector3()));const normal=new THREE.Vector3(0,0,1).transformDirection(target.matrixWorld);if(probe(center,normal,target)){r.status='passed';return r;}
  for(const d of directions)if(probe(center,d.clone(),target)){r.status='passed';return r;}
  // Curved/annular cap centers can miss their own visible surface. Sample the
  // largest actual triangle in each world-normal octant, never guessed positions.
  const g=target.geometry,p=g.getAttribute('position'),index=g.index,bins=new Map(),samples=[],sampleStride=Math.max(3,Math.floor((index?index.count:p.count)/768)*3),a=new THREE.Vector3(),b=new THREE.Vector3(),c=new THREE.Vector3();
  for(let i=0;i<(index?index.count:p.count);i+=3){a.fromBufferAttribute(p,index?index.getX(i):i).applyMatrix4(target.matrixWorld);b.fromBufferAttribute(p,index?index.getX(i+1):i+1).applyMatrix4(target.matrixWorld);c.fromBufferAttribute(p,index?index.getX(i+2):i+2).applyMatrix4(target.matrixWorld);const n=b.clone().sub(a).cross(c.clone().sub(a)),area=n.lengthSq(),key=[n.x>=0,n.y>=0,n.z>=0].join();const sample={area,point:a.clone().add(b).add(c).multiplyScalar(1/3),normal:n};if(area>(bins.get(key)?.area||0))bins.set(key,sample);if(i%sampleStride===0)samples.push(sample);}
  for(const sample of [...bins.values()].sort((a,b)=>b.area-a.area).concat(samples))if(probe(sample.point,sample.normal,target)){r.status='passed';return r;}
 }
 r.status='mapped-but-occluded';return r;
}
try{
 globalThis.fetch=async url=>{try{return new Response(await readFile(await actualFile(fileURLToPath(url))));}catch{return new Response('',{status:404});}};globalThis.document={createElement:canvas};
 THREE.TextureLoader.prototype.loadAsync=async url=>{const file=await actualFile(fileURLToPath(url));const raw=execFileSync('python',['-c',"import sys,struct;from PIL import Image;im=Image.open(sys.argv[1]);a=('A' in im.getbands())or('transparency' in im.info);sys.stdout.buffer.write(struct.pack('<IIB',*im.size,0 if a else 255));sys.stdout.buffer.write(im.convert('RGBA').getchannel('A').tobytes()if a else b'')",file],{maxBuffer:128*1024*1024});const image={width:raw.readUInt32LE(0),height:raw.readUInt32LE(4),qaUniformAlpha:raw[8]};if(raw.length>9)image.qaAlpha=new Uint8Array(raw.buffer,raw.byteOffset+9,raw.length-9);const texture=new THREE.Texture(image);texture.userData.qaFile=relative(root,fileURLToPath(url));texture.userData.qaSha256=hash(await readFile(file));return texture;};
 for(const p of products){if(ids&&!ids.includes(p.id))continue;const row={productId:p.id,name:p.name,board:p.runtime?.board,controls:[],matrixControls:[],errors:[]};rows.push(row);let m;
  try{m=p.id==='m5stack-papermono'?createPapermonoModel(new THREE.Texture()):p.id==='m5stack-stopwatch'?createStopwatchModel(new THREE.Texture()):createManufacturerModel(p,new THREE.Texture());const meta=await m.ready;m.group.updateMatrixWorld(true);const path=join(root,'web/platform',p.model?.url||`assets/${p.id}/model.json`);row.modelSha256=hash(await readFile(await actualFile(path)));row.loader=p.id==='m5stack-papermono'?'dedicated-papermono':p.id==='m5stack-stopwatch'?'dedicated-stopwatch':'manufacturer';row.modelEncoderMeshes=[];m.group.traverse(o=>{if(o.userData.encoder)row.modelEncoderMeshes.push({mesh:o.name,encoderId:o.userData.encoder,controlId:o.userData.control??null,encoderPress:o.userData.encoderPress??null});});row.modelButtons=m.buttons.map(o=>({mesh:o.name,control:o.userData.control}));row.sourceButtonParts=(meta.parts||[]).filter(o=>o.controlId||o.role==='button'||/button|key|cap|knob/i.test(o.name)).map(o=>({name:o.name,role:o.role,controlId:o.controlId??null,sourceComponent:o.sourceComponent??null,boundsMm:o.boundsMm,sourceIds:o.sourceIds??null,basis:o.basis??null}));row.presentationButtons=(meta.presentation?.buttons||[]).map(({id,shape,sizeMm,positionMm,rotation,sourceIds,basis,notes})=>({id,shape,sizeMm,positionMm,rotation,sourceIds,basis,notes}));row.derivedButtonParts=(meta.derivedParts||[]).filter(o=>o.controlId||/button|key|cap|knob/i.test(o.id)).map(o=>({id:o.id,kind:o.kind,controlId:o.controlId??null,geometry:o.geometry,sourceIds:o.sourceIds}));
   const span=Math.max(100,new THREE.Box3().setFromObject(m.group).getSize(new THREE.Vector3()).length()*2),supported=(p.controls||[]).filter(c=>c.type==='button'&&c.simulationSupported!==false&&(c.pin!==undefined||c.gpio!==undefined||c.transport==='board'));
   row.controls=supported.map(c=>check(m,c,span,p,args.includes('--inspection-missing')));row.matrixControls=(p.controls||[]).filter(c=>c.type==='matrix-key'&&c.simulationSupported!==false).map(c=>({...check(m,c,span,p),row:c.row,column:c.column}));row.inspectionError=m.group.userData.inspectionError??null;
   if(args.includes('--inspection-missing')&&m.group.userData.inspection){const failed=supported.filter(c=>row.controls.some(r=>r.id===c.id&&r.status==='mapped-but-occluded'));if(failed.length){try{m.group.userData.inspection.setProgress(1);m.group.updateMatrixWorld(true);row.inspectionControls=failed.map(c=>({...check(m,c,span,p,true),inspectionProgress:1,assembledStatus:'mapped-but-occluded'}));}catch(e){row.inspectionProbeError=e.message;}finally{m.group.userData.inspection.setProgress(0);m.group.updateMatrixWorld(true);for(const c of row.inspectionControls||[]){const saved=c.successfulRay;if(!saved)continue;const ray=new THREE.Raycaster(new THREE.Vector3().fromArray(saved.eyeMm),new THREE.Vector3().fromArray(saved.direction),0,span*2),hit=pickVisibleSurface(ray,m.group);c.assembledReplay={hitMesh:hit?.object.name??null,controlId:pickControl(hit),blocked:pickControl(hit)!==c.id};}}}}
   row.controlCoverage=row.controls.map(c=>{const inspected=(row.inspectionControls||[]).find(i=>i.id===c.id),category=c.status==='passed'?'assembled-physical':c.status==='virtual-only-with-real-touch-surface'?'virtual-touch-shortcut':inspected?.status==='passed'&&inspected.assembledReplay?.blocked===true?'inspection-only':'unclassified';return{controlId:c.id,category,assembledStatus:c.status,inspectionStatus:inspected?.status??null};});if(args.includes('--region-surface-gates'))row.regionSurfaceGates=regionSurfaceGates(m,meta,span);if(args.includes('--negative-region-probes'))row.regionNegativeProbes=negativeRegionProbes(m,meta,span);if(args.includes('--visual-signature'))row.visualSignatures=visualVariants(m);
  }catch(e){row.errors.push(e.message);}finally{m?.dispose();m?.group.traverse(o=>{o.geometry?.dispose();if(o.material)(Array.isArray(o.material)?o.material:[o.material]).forEach(v=>v.dispose());});}
  if(rows.length%10===0)console.log('Audited',rows.length,'/',products.length);
 }
}finally{globalThis.fetch=previous.fetch;globalThis.document=previous.document;THREE.TextureLoader.prototype.loadAsync=previous.load;}
const buttonFindings=rows.flatMap(r=>r.controls.filter(c=>c.status!=='passed'&&c.status!=='virtual-only-with-real-touch-surface').map(c=>({productId:r.productId,...c}))),matrixFindings=rows.flatMap(r=>r.matrixControls.filter(c=>c.status!=='passed').map(c=>({productId:r.productId,...c})));
const report={schemaVersion:1,checkedAt:new Date().toISOString(),sourceStartCommit,sourceCommit:execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),catalogSha256:hash(sourceBytes),harnessSha256:harnessSourceSha256,productionPickerSha256:hash(await readFile(join(root,'web/platform/scene-interaction.js'))),productionCodeHashes:Object.fromEntries(await Promise.all(['manufacturer-model.js','papermono-model.js','stopwatch-model.js','scene-view.js','shell-appearance.js'].map(async file=>[file,hash(await readFile(join(root,'web/platform',file)))]))),summary:{products:rows.length,buttonProducts:rows.filter(r=>r.controls.length).length,supportedButtons:rows.reduce((n,r)=>n+r.controls.length,0),classifiedSupportedButtons:rows.reduce((n,r)=>n+(r.controlCoverage||[]).filter(c=>c.category!=='unclassified').length,0),unclassifiedSupportedButtons:rows.reduce((n,r)=>n+(r.controlCoverage||[]).filter(c=>c.category==='unclassified').length,0),inspectionOnlyButtonPasses:rows.reduce((n,r)=>n+(r.inspectionControls||[]).filter(c=>c.status==='passed').length,0),physicalButtonPasses:rows.reduce((n,r)=>n+r.controls.filter(c=>c.status==='passed').length,0),virtualOnlyWithRealTouchSurface:rows.reduce((n,r)=>n+r.controls.filter(c=>c.status==='virtual-only-with-real-touch-surface').length,0),buttonFindings:buttonFindings.length,buttonFindingSkus:new Set(buttonFindings.map(c=>c.productId)).size,matrixControls:rows.reduce((n,r)=>n+r.matrixControls.length,0),matrixFindings:matrixFindings.length,matrixFindingSkus:new Set(matrixFindings.map(c=>c.productId)).size,loaderErrors:rows.filter(r=>r.errors.length).length},buttonFindings,matrixFindings,products:rows,scope:'Actual production loader and production pickVisibleSurface, assembled default appearance; optional inspection-only results stay separate and do not change default findings. Original photograph alpha is decoded. Canvas UNKNOWN text uses conservatively opaque alpha in Node; any such occluder is reported for review. No constructor-only success, browser pointer test, firmware execution or physical-position calibration claim.'};await mkdir(dirname(out),{recursive:true});await writeFile(out,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report.summary));
