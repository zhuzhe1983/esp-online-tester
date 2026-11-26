// Actual K151 loader, articulation, picking and material checks. No guest execution.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {dirname,resolve} from 'node:path';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyServoValues} from '../web/platform/servo-view.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
import {pickVisibleSurface} from '../web/platform/scene-interaction.js';
const root=new URL('../',import.meta.url),assets=new URL('web/platform/assets/m5stack-k151/',root),sha=b=>createHash('sha256').update(b).digest('hex');
const raw=await readFile(new URL('model.json',assets)),metadata=JSON.parse(raw),planRaw=await readFile(new URL('model-plan.json',assets)),plan=JSON.parse(planRaw);
const product=JSON.parse(await readFile(new URL('web/platform/catalog/products.json',root))).items.find(p=>p.id==='m5stack-k151');
const previousModelSha256='7f0dd1c0f2afac0ee19afd71358ace53e86ff431c413a05943f9696ed37aaf60',withoutInspection=structuredClone(metadata);delete withoutInspection.presentation.inspectionGroups;
const assembledMetadataSha256=sha(JSON.stringify(withoutInspection));assert.equal(assembledMetadataSha256,'5d18ff18cbfebdc22d3816d9fe5de8376455d71e0546028166538ad5196bc717');
const restoredPreviousMetadataSha256=execFileSync('python',['-c','import json,hashlib,sys;m=json.load(open(sys.argv[1]));del m["presentation"]["inspectionGroups"];print(hashlib.sha256((json.dumps(m,ensure_ascii=False,indent=2)+"\\n").encode()).hexdigest())',fileURLToPath(new URL('model.json',assets))],{encoding:'utf8'}).trim();assert.equal(restoredPreviousMetadataSha256,previousModelSha256);
assert.deepEqual(metadata.presentation,plan.presentation);assert.equal(metadata.artifacts['geometry.bin'],'43a3f731ea8f12b7071f75e5c8d2a325ab876737164906f7993bfd7765ad1e29');
for(const[file,digest]of Object.entries(metadata.artifacts))assert.equal(sha(await readFile(new URL(file,assets))),digest,file);
const old={fetch:globalThis.fetch,document:globalThis.document,load:THREE.TextureLoader.prototype.loadAsync},texts=[],models=[];let assembledReference=false;
globalThis.fetch=async url=>{const value=new URL(url);if(value.pathname.endsWith('/model.json')){const copy=structuredClone(metadata);if(assembledReference)delete copy.presentation.inspectionGroups;return new Response(JSON.stringify(copy));}return new Response(await readFile(value));};
globalThis.document={createElement:()=>({getContext:()=>({fillText(text){assert.equal(text,'UNKNOWN');texts.push(text);}})})};
THREE.TextureLoader.prototype.loadAsync=async url=>{const map=new THREE.Texture();map.userData.sourceSha256=sha(await readFile(new URL(url)));return map;};
const world=o=>o.getWorldPosition(new THREE.Vector3()),near=(a,b,label)=>assert(a.distanceTo(b)<1e-8,label);
const attribute=a=>a?{count:a.count,itemSize:a.itemSize,normalized:a.normalized,offset:a.offset??0,stride:a.data?.stride??null,sha256:sha(Buffer.from((a.array??a.data.array).buffer,(a.array??a.data.array).byteOffset,(a.array??a.data.array).byteLength))}:null;
function visualDigest(model){model.group.updateMatrixWorld(true);const meshes=[];model.group.traverse(o=>{if(!o.isMesh)return;const m=o.material,map=m.map;if(map)map.updateMatrix();const path=[];for(let p=o;p&&p!==model.group;p=p.parent)path.unshift(p.name);meshes.push({path,visible:o.visible,matrix:o.matrixWorld.elements,attributes:Object.fromEntries(Object.entries(o.geometry.attributes).map(([name,a])=>[name,attribute(a)])),index:attribute(o.geometry.getIndex()),material:{type:m.type,color:m.color?.toArray(),emissive:m.emissive?.toArray(),emissiveIntensity:m.emissiveIntensity,opacity:m.opacity,roughness:m.roughness,metalness:m.metalness,side:m.side,alphaTest:m.alphaTest,transparent:m.transparent,depthWrite:m.depthWrite,map:map?{sourceSha256:map.userData.sourceSha256??null,size:[map.image?.width??null,map.image?.height??null],matrix:map.matrix.elements,flipY:map.flipY,colorSpace:map.colorSpace}:null}});});return sha(JSON.stringify(meshes));}
try{
 for(const reference of [false,true]){assembledReference=reference;const m=createManufacturerModel(product,new THREE.Texture());await m.ready;assert.equal(m.group.userData.inspectionError,undefined);models.push(m);}
 const[model,reference]=models,inspection=model.group.userData.inspection,joints=model.group.userData.servoJoints;assert(inspection);assert.equal(joints.length,2);assert.equal(joints[1].parent,joints[0]);
 const loaderDigests={original:{previous:visualDigest(reference),final:visualDigest(model)}};assert.equal(loaderDigests.original.final,loaderDigests.original.previous);
 const nodes=[];model.group.traverse(o=>nodes.push({o,parent:o.parent,position:o.position.clone(),quaternion:o.quaternion.clone(),scale:o.scale.clone(),material:o.material,geometry:o.geometry}));
 const groups=metadata.presentation.inspectionGroups;assert.equal(groups.length,4);assert(groups.some(g=>g.nested));const base=new Map(nodes.map(n=>[n.o.name,n.position]));
 const poseCases=[[0,0],[-30,25],[55,-20],[0,0]],progresses=[0,.25,.5,1,0],poseResults=[];
 const photo=model.parts['pcb-power-back-photo'],board=model.parts['pcb-power'],photoOffset=photo.position.clone().sub(board.position);
 for(const[pan,tilt]of poseCases){
  const values=[{id:'pan',positionDegrees:pan},{id:'tilt',positionDegrees:tilt}];applyServoValues(joints,values);applyServoValues(reference.group.userData.servoJoints,values);reference.group.updateMatrixWorld(true);
  const quaternions=joints.map(j=>j.quaternion.toArray());
  for(const progress of progresses){
   inspection.setProgress(progress);
   for(const g of groups)for(const name of g.parts){const o=model.group.getObjectByName(name);near(o.position,base.get(name).clone().addScaledVector(new THREE.Vector3(...g.offsetMm),progress),name+' source viewing offset');}
   near(world(board),world(reference.parts['pcb-power']),'Power board stays in the existing tilt layer');near(photo.position.clone().sub(board.position),photoOffset,'Rear photograph stays on its source board');
   for(let i=0;i<2;i++)assert.deepEqual(joints[i].quaternion.toArray(),quaternions[i]);
   for(const n of nodes){assert(n.o.parent===n.parent);assert.deepEqual(n.o.scale.toArray(),n.scale.toArray());assert(n.o.material===n.material);assert(n.o.geometry===n.geometry);if(!joints.includes(n.o))assert.deepEqual(n.o.quaternion.toArray(),n.quaternion.toArray());}
   const screen=model.parts.screen,normal=new THREE.Vector3(0,0,1).transformDirection(screen.matrixWorld),point=world(screen),ray=new THREE.Raycaster(point.clone().addScaledVector(normal,100),normal.clone().negate());assert.equal(pickVisibleSurface(ray,model.group)?.object?.name,'live-display','Frame-buffer hit follows actual servo and inspection transforms');
   if(progress===0){for(const name of Object.keys(model.parts)){const a=model.parts[name],b=reference.parts[name];if(!a?.isObject3D||!b?.isObject3D)continue;near(world(a),world(b),name+' reassembles at current servo pose');}}
  }
  poseResults.push({panDegrees:pan,tiltDegrees:tilt,progresses,servoQuaternionsUntouched:true,screenNearestHit:true,reassembledAtCurrentPose:true});
 }
 inspection.setProgress(.6);const liveValues=[{id:'pan',positionDegrees:35},{id:'tilt',positionDegrees:15}];applyServoValues(joints,liveValues);applyServoValues(reference.group.userData.servoJoints,liveValues);model.group.updateMatrixWorld(true);reference.group.updateMatrixWorld(true);
 const delta=new THREE.Vector3(72,0,38).multiplyScalar(.6).applyQuaternion(reference.group.userData.servoJoints[1].getWorldQuaternion(new THREE.Quaternion()));near(world(model.parts.screen),world(reference.parts.screen).add(delta),'Inspection offset follows new feedback while already expanded');inspection.setProgress(0);near(world(model.parts.screen),world(reference.parts.screen),'Reassembly preserves new feedback pose');
 applyServoValues(joints,[]);applyServoValues(reference.group.userData.servoJoints,[]);
 inspection.setProgress(1);model.group.updateMatrixWorld(true);
 const visible=[];model.group.traverse(o=>{if(!o.isMesh||o.material.opacity<=.001)return;for(let p=o;p;p=p.parent)if(!p.visible)return;visible.push(o);});
 const directions=[[0,0,1],[0,0,-1],[0,1,0],[0,-1,0],[1,0,1],[-1,0,1],[1,1,2],[-1,1,2],[1,1,-2],[-1,1,-2],[0,2,1],[0,-2,1],[2,1,1],[-2,1,1],[1,-1,-2],[-1,-1,-2]];
 const boardIds=['pcb-main','pcb-touch','pcb-power','pcb-adapter','pcb-ring'],unknownVisibility=[];
 for(const id of boardIds){
  const markers=[];model.parts[id].traverse(o=>{if(o.name.startsWith(id+'-unknown-'))markers.push(o);});assert(markers.length);
  let best={boardId:id,visiblePoints:0};
  for(const marker of markers)for(const xyz of directions){const direction=new THREE.Vector3(...xyz).normalize(),normal=new THREE.Vector3(0,0,1).transformDirection(marker.matrixWorld);if(normal.dot(direction)<.1)continue;
   const{width,height}=marker.geometry.parameters,samples=[[0,0],[-.4*width,-.4*height],[.4*width,-.4*height],[-.4*width,.4*height],[.4*width,.4*height]],hits=[];
   for(const[x,y]of samples){const at=marker.localToWorld(new THREE.Vector3(x,y,0)),hit=new THREE.Raycaster(at.clone().addScaledVector(direction,180),direction.clone().negate()).intersectObjects(visible,false)[0];hits.push(hit?.object?.name??null);}
   const count=hits.filter(name=>name===marker.name).length;if(count>best.visiblePoints)best={boardId:id,marker:marker.name,viewDirection:xyz,visiblePoints:count,sampledNearestMeshes:hits};if(count===5)break;
  }
  unknownVisibility.push(best);
 }
 const shells=new Set(model.group.userData.shellMeshes),fixed=visible.filter(o=>!shells.has(o)).map(o=>({o,material:o.material,color:o.material.color?.getHex(),opacity:o.material.opacity,map:o.material.map}));inspection.setProgress(0);
 for(const[name,shell]of Object.entries({gold:{color:'#d4af37',opacity:1,finish:'metal'},purple:{color:'#8054b3',opacity:.35,finish:'plastic'}})){for(const m of [model,reference])for(const mesh of m.group.userData.shellMeshes)applyShellAppearance(mesh,{shell});for(const n of fixed){assert(n.o.material===n.material);assert.equal(n.o.material.color?.getHex(),n.color);assert.equal(n.o.material.opacity,n.opacity);assert(n.o.material.map===n.map);}loaderDigests[name]={previous:visualDigest(reference),final:visualDigest(model)};assert.equal(loaderDigests[name].final,loaderDigests[name].previous);}
 inspection.setProgress(0);for(const n of nodes)assert.deepEqual(n.o.position.toArray(),n.position.toArray());assert(texts.length&&texts.every(t=>t==='UNKNOWN'));
 const receipt={productId:product.id,modelSha256:sha(raw),planSha256:sha(planRaw),nativeGeometrySha256:metadata.artifacts['geometry.bin'],verifiedArtifacts:metadata.artifacts,engineSha256:sha(await readFile(new URL('web/platform/model-inspection.js',root))),verifierSha256:sha(await readFile(new URL(import.meta.url))),groups,actualServoParentageRetained:true,poseResults,liveFeedbackWhileExpanded:{panDegrees:35,tiltDegrees:15,progress:.6,offsetFollowsJoint:true,restoreAtCurrentFeedbackPose:true},photoBoardRelativeTransformUnchanged:true,unknownVisibility,allUnknownMarkersVisible:unknownVisibility.every(v=>v.visiblePoints===5),nonShellMaterialIsolation:['original','gold','translucent-purple'],resetLocalPositionsExact:true,assembledEquivalence:{previousModelSha256,finalModelSha256:sha(raw),excludedJsonPointer:'/presentation/inspectionGroups',restoredPreviousMetadataSha256,restoredEncoding:'UTF-8 Python json.dumps ensure_ascii=False indent=2 plus final newline',parsedPreviousContractSha256:assembledMetadataSha256,parsedFinalWithoutInspectionSha256:assembledMetadataSha256,productionLoaderVisualDigests:loaderDigests,scope:'Actual independent old-contract/new-contract production loader meshes, full world matrices, geometry attributes/indices, UV/texture matrices and source image hashes, material parameters and hierarchy at inspection=0. Existing original/gold/purple CPU receipts remain bound to previous7f metadata; they are not rewritten.'},scope:'Actual production K151 loader, source artifacts, servo feedback poses and inspection transforms. Texture pixels are not decoded here; independent CPU image checks appearance. No guest, physical fit or manufacturer firmware validation.'};
 const out=process.argv[2];if(out){await mkdir(dirname(resolve(out)),{recursive:true});await writeFile(out,JSON.stringify(receipt,null,2)+'\n');}
 console.log(JSON.stringify({modelSha256:receipt.modelSha256,poseCases:poseResults.length,unknownVisibility},null,2));
 assert(receipt.allUnknownMarkersVisible,'Every UNKNOWN board must have an unobscured center and four inset corners in an accessible inspection view');
}finally{for(const m of models)m.dispose();globalThis.fetch=old.fetch;globalThis.document=old.document;THREE.TextureLoader.prototype.loadAsync=old.load;}
