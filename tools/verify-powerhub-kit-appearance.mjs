// Independent K148 catalog-instance verification of the explicitly shared
// PowerHub host model. No battery geometry or firmware execution is inferred.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
const root=new URL('../',import.meta.url),hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const attributeValues=a=>a?Array.from(a.array??a.data.array):null;
const catalog=JSON.parse(await readFile(new URL('web/platform/catalog/products.json',root))).items;
const kit=catalog.find(p=>p.id==='m5stack-k148'),host=catalog.find(p=>p.id==='m5stack-c148');
assert.equal(kit.name,'M5Stack PowerHub Kit · K148');assert.notEqual(kit.id,host.id);
assert.equal(kit.visualVariantOf,host.id);assert.equal(kit.firmwareTargetId,host.id);
assert.equal(kit.model.url,host.model.url);assert.equal(kit.model.url,'./assets/m5stack-c148/model.json');
const knowledge=JSON.parse(await readFile(new URL('web/platform/knowledge/m5stack-k148.json',root)));
const sourceReceipt=JSON.parse(await readFile(new URL('docs/platform/evidence/m5stack-k148/host-identity-source-check.json',root)));
for(const recorded of sourceReceipt.sourceRecordsUnchanged)assert.deepEqual(knowledge.sources.find(s=>s.id===recorded.id),recorded,'Preserve each original fetched-source identity');
const sourceOption=process.argv.indexOf('--source-html');
if(sourceOption>=0){
 assert(process.argv[sourceOption+1],'--source-html requires the retained official page');
 const source=knowledge.sources.find(s=>s.id==='official-docs-20261126'),raw=await readFile(process.argv[sourceOption+1]),html=raw.toString('utf8');
 assert.equal(hash(raw),source.sha256);assert.equal(raw.length,source.bytes);
 assert.match(html,/SKU:C148\/K148/);
 const includes=html.match(/<h2[^>]*id="includes"[^>]*>[\s\S]*?<h2[^>]*id="applications"/)[0];
 assert.match(includes,/id="powerhub-skuc148"[\s\S]*?1 x PowerHub[\s\S]*?id="powerhub-kit-skuk148"[\s\S]*?1 x PowerHub[\s\S]*?NP[‑-]F550 2000mAh/);
 sourceReceipt.sourceByteValidation={verifiedBy:'tools/verify-powerhub-kit-appearance.mjs --source-html SOURCE',sha256:hash(raw),bytes:raw.length,originalFetchedAtPreserved:source.fetchedAt,skuHeadingAndSeparateIncludesChecked:true};
 await writeFile(new URL('docs/platform/evidence/m5stack-k148/host-identity-source-check.json',root),JSON.stringify(sourceReceipt,null,2)+'\n');
}
const contents=knowledge.facts.find(f=>f.id==='kit-contents').value;
assert.deepEqual(contents,{controller:'PowerHub',battery:'NP-F550 2000mAh',batteryIncluded:true});
const identity=knowledge.facts.find(f=>f.id==='host-appearance-reference').value;
assert.equal(identity.kitBatteryInstalledInPreview,false);assert.equal(identity.kitBatteryGeometryVerified,false);
const base=new URL('web/platform/assets/m5stack-c148/',root),bytes=await readFile(new URL('model.json',base)),meta=JSON.parse(bytes);
assert.equal(meta.artifacts['geometry.bin'],'1a83db6a0d3bc223455d88b57fc42212f6b0235456245c0a2f37ea4adede7826');
assert.equal(meta.artifacts['source.stl'],'ae845a13e4b9160249eece2351720bd76f951cc617f63bc765ac7f8a0b8a3ec9');
for(const[file,sha256]of Object.entries(meta.artifacts))assert.equal(hash(await readFile(new URL(file,base))),sha256,file);
const rearPaper=meta.derivedParts.filter(p=>p.kind==='label'&&p.sourceIds?.includes('product-photo-c8fad993ac'));
assert.equal(rearPaper.length,1,'The shared host has the sourced rear printed layer');
assert(meta.omittedComponents.some(p=>p.index===6&&/battery/i.test(p.reason)));
assert(!meta.parts.some(p=>p.sourceComponent===6),'The unverified native battery envelope stays omitted');
const old={fetch:globalThis.fetch,document:globalThis.document,load:THREE.TextureLoader.prototype.loadAsync};
globalThis.fetch=async url=>new Response(await readFile(fileURLToPath(url)));
const canvasTexts=[];globalThis.document={createElement:()=>({width:1,height:1,getContext:()=>({fillText(text){assert.equal(text,'UNKNOWN');canvasTexts.push(text);}})})};
THREE.TextureLoader.prototype.loadAsync=async url=>{const texture=new THREE.Texture();texture.userData.sourceSha256=hash(await readFile(fileURLToPath(url)));return texture;};
const models=[];
try{
 for(const product of [host,kit]){const m=createManufacturerModel(product,new THREE.Texture());await m.ready;m.group.updateMatrixWorld(true);models.push(m);assert.equal(m.group.userData.inspectionError,undefined);}
 const [original,model]=models;
 assert.deepEqual(Object.keys(original.parts).sort(),Object.keys(model.parts).sort());
 for(const[name,mesh]of Object.entries(original.parts)){
  const twin=model.parts[name];assert.deepEqual(mesh.position.toArray(),twin.position.toArray());assert.deepEqual(mesh.quaternion.toArray(),twin.quaternion.toArray());
  if(mesh.isMesh){assert.deepEqual(attributeValues(mesh.geometry.getAttribute('position')),attributeValues(twin.geometry.getAttribute('position')));assert.equal(mesh.material.color.getHex(),twin.material.color.getHex());}
 }
 const meshSignature=m=>{const result=[];m.group.traverse(o=>{if(o.isMesh)result.push({name:o.name,matrix:o.matrixWorld.elements,positions:attributeValues(o.geometry.getAttribute('position')),normals:attributeValues(o.geometry.getAttribute('normal')),uv:attributeValues(o.geometry.getAttribute('uv')),indices:attributeValues(o.geometry.getIndex()),texture:o.material.map?.userData.sourceSha256??null});});return result;};
 assert.deepEqual(meshSignature(original),meshSignature(model),'All production child meshes, world matrices, UVs and source textures match the explicitly shared host');
 const paper=model.parts[rearPaper[0].id],point=paper.getWorldPosition(new THREE.Vector3()),normal=new THREE.Vector3(0,0,1).transformDirection(paper.matrixWorld);
 const hit=new THREE.Raycaster(point.clone().addScaledVector(normal,100),normal.clone().negate()).intersectObjects(model.group.children,true).find(h=>h.object.userData.inputOccluder);
 assert.equal(hit?.object?.name,paper.name,'K148 rear printed layer is visible on its actual outward face');
 const fixed=[],shells=new Set(model.group.userData.shellMeshes),markers=[];
 const surface=m=>({color:m.color.toArray(),opacity:m.opacity,roughness:m.roughness,metalness:m.metalness,alphaTest:m.alphaTest,side:m.side,transparent:m.transparent});
 model.group.traverse(o=>{if(o.name.includes('-unknown-'))markers.push(o.name);if(o.isMesh&&!shells.has(o))fixed.push([o,surface(o.material),o.material,o.material.map]);});assert.deepEqual(markers,['pcb-main-unknown-front']);assert(shells.size>0);
 const marker=model.group.getObjectByName(markers[0]);assert.equal(marker.material.side,THREE.FrontSide,'No rear-facing UNKNOWN canvas may leak text through opaque rear keyholes');
 const rearPads=[];for(const sx of [-1,1])for(const sy of [-1,1]){const id=`rear-rubber-pad-${sx}-${sy}`,pad=model.parts[id];assert(pad?.isMesh,id);assert(!shells.has(pad));assert.equal(pad.material.color.getHexString(),'222522');const at=pad.getWorldPosition(new THREE.Vector3()),hit=new THREE.Raycaster(at.clone().setZ(-70),new THREE.Vector3(0,0,1)).intersectObjects(model.group.children,true).find(h=>h.object.material?.opacity>.001);assert.equal(hit?.object?.name,pad.name,'Independent rear pad is the outward opaque hit');rearPads.push(id);}
 for(const shell of [{color:'#d4af37',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}]){for(const m of shells)applyShellAppearance(m,{shell});for(const[m,properties,material,map]of fixed){assert.deepEqual(surface(m.material),properties);assert(m.material===material,'Non-shell material identity preserved: '+m.name);assert(m.material.map===map,'Non-shell texture identity preserved: '+m.name);}}
 const poses=model.group.children.map(o=>[o,o.position.clone(),o.quaternion.clone(),o.visible]),inspection=model.group.userData.inspection;
 const rearGroup=meta.presentation.inspectionGroups.find(g=>g.parts.includes(rearPaper[0].id));assert(rearGroup,'Rear paper participates in the declared rear housing inspection group');
 const paperBase=poses.find(([o])=>o===paper)[1],board=model.parts['pcb-main'],boardBase=board.position.clone(),padBases=rearPads.map(id=>[model.parts[id],model.parts[id].position.clone()]);for(const id of rearPads)assert(rearGroup.parts.includes(id),'Rear pad belongs to rear housing inspection group');
 const unknownInspectionRayPoints=[],inspectionDirection=[2,3,1];
 for(const progress of [.25,.5,1]){inspection.setProgress(progress);assert(paper.position.equals(paperBase.clone().add(new THREE.Vector3(...rearGroup.offsetMm).multiplyScalar(progress))));assert(board.position.equals(boardBase));for(const[pad,base]of padBases)assert(pad.position.equals(base.clone().add(new THREE.Vector3(...rearGroup.offsetMm).multiplyScalar(progress))),'Rear pad follows rear housing: '+pad.name);if(progress===1){model.group.updateMatrixWorld(true);const direction=new THREE.Vector3(...inspectionDirection).normalize();for(const[x,y]of[[0,0],[-14,-2.8],[14,-2.8],[-14,2.8],[14,2.8]]){const at=marker.localToWorld(new THREE.Vector3(x,y,0)),hit=new THREE.Raycaster(at.clone().addScaledVector(direction,100),direction.clone().negate()).intersectObjects(model.group.children,true).find(h=>h.object.material?.opacity>.001);assert.equal(hit?.object?.name,marker.name,'UNKNOWN remains unobscured during public inspection at '+[x,y]);unknownInspectionRayPoints.push(at.toArray());}}inspection.setProgress(0);for(const[o,p,q,visible]of poses){assert(o.position.equals(p));assert(o.quaternion.equals(q));assert.equal(o.visible,visible);}}
 model.group.updateMatrixWorld(true);const bounds=new THREE.Box3();model.group.traverse(o=>{if(!o.isMesh||o.material.opacity<=.001)return;for(let p=o;p;p=p.parent)if(!p.visible)return;o.geometry.computeBoundingBox();bounds.union(o.geometry.boundingBox.clone().applyMatrix4(o.matrixWorld));});
 const receipt={productId:kit.id,hostProductId:host.id,modelUrl:kit.model.url,productRecordSha256:hash(JSON.stringify(kit)),knowledgeRecordSha256:hash(await readFile(new URL('web/platform/knowledge/m5stack-k148.json',root))),verifierSha256:hash(await readFile(fileURLToPath(import.meta.url))),modelSha256:hash(bytes),nativeGeometrySha256:meta.artifacts['geometry.bin'],nativeSourceSha256:meta.artifacts['source.stl'],allArtifactHashesVerified:meta.artifacts,sharedHostMeshesIdentical:true,independentlyLoadedMeshCount:meshSignature(model).length,rearPaperId:rearPaper[0].id,rearPaperOutwardRayVisible:true,rayScope:'Nearest geometric input-occluder hit at the rear-paper center; Three Raycaster does not evaluate texture alpha. Actual alpha/UV appearance requires the independent CPU render.',unknownMarkers:markers,materialIsolation:['original','gold','translucent-purple'],inspectionProgressChecked:[.25,.5,1],rearPaperFollowsDeclaredRearHousing:true,unknownBoardStaysInElectronicsLayer:true,inspectionRestoreExact:true,batteryIncludedInKit:true,batteryInstalledInPreview:false,batteryGeometryVerified:false,omittedBatterySourceComponent:6,firmwareTargetId:kit.firmwareTargetId,factoryFirmwareValidation:false,physicalFitValidation:false,scope:'Actual independent K148 and C148 production Three instances, native artifact hashes, geometry/material/inspection/ray checks. Image appearance is reviewed separately through CPU renders, not by this texture stub.'};
 receipt.visibleHostPreviewBoundsMm=[bounds.min.toArray(),bounds.max.toArray()];receipt.visibleHostPreviewSizeMm=bounds.getSize(new THREE.Vector3()).toArray();receipt.dimensionScope='All visible production meshes, including exterior estimates and paper; transparent input hit planes omitted. This is an uninstalled-battery host envelope, not a complete Kit or manufacturing tolerance pass.';
 assert.deepEqual(canvasTexts,['UNKNOWN','UNKNOWN']);receipt.rearPads={ids:rearPads,color:'#222522',separateFromShell:true,nearestOutwardRayHits:4,inspectionFollowsRearHousing:true};receipt.unknownPlacement={frontOnly:true,rearCanvasAbsent:true,actualProductionCanvasText:'UNKNOWN',inspectionViewDirection:inspectionDirection,inspectionNearestRayPoints:unknownInspectionRayPoints,rayScope:'The actual UNKNOWN plane is the nearest visible geometric hit at center and four inset corners after production inspection=1. CPU rendering separately checks text pixels.'};
 const out=new URL('docs/platform/evidence/m5stack-k148/host-appearance-loader.json',root);await mkdir(new URL('.',out),{recursive:true});await writeFile(out,JSON.stringify(receipt,null,2)+'\n');console.log('K148 exact Kit identity and independent shared-host loader verification passed');
}finally{for(const m of models)m.dispose();globalThis.fetch=old.fetch;globalThis.document=old.document;THREE.TextureLoader.prototype.loadAsync=old.load;}
