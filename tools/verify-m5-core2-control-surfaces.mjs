// Source-bound control surfaces: actual production loader, appearance and picker.
// Caller supplies source/overlay trees; no browser, GPU or firmware claim.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {resolve,join,dirname} from 'node:path';
import {pathToFileURL,fileURLToPath} from 'node:url';
const args=process.argv.slice(2),arg=n=>{const i=args.indexOf(n);return i<0?null:args[i+1];};
assert(arg('--source-root')&&arg('--overlay-root')&&arg('--baseline')&&arg('--out'),'--source-root --overlay-root --baseline --out are required');
const root=resolve(arg('--source-root')),overlay=resolve(arg('--overlay-root')),baseline=arg('--baseline'),out=resolve(arg('--out'));
const ids=(arg('--ids')||'m5stack-k010,m5stack-k010-v11,m5stack-k010-aws,m5stack-k010-aws-v13,m5stack-k010-v13').split(',');
const sha=b=>createHash('sha256').update(b).digest('hex'),git=(...a)=>execFileSync('git',a,{cwd:root,encoding:null});
const sourceStartCommit=git('rev-parse','HEAD').toString().trim();
const THREE=await import(pathToFileURL(join(root,'web/platform/vendor/three.module.min.js')));
const {createManufacturerModel}=await import(pathToFileURL(join(root,'web/platform/manufacturer-model.js')));
const {pickVisibleSurface}=await import(pathToFileURL(join(root,'web/platform/scene-interaction.js')));
const {applyShellAppearance}=await import(pathToFileURL(join(root,'web/platform/shell-appearance.js')));
const catalogBytes=await readFile(join(root,'web/platform/catalog/products.json')),catalog=JSON.parse(catalogBytes).items;
let servedMetadata;
const canvas=()=>{const c={width:1,height:1,qaUniformAlpha:255};const ctx={fillText(){},fillRect(){},clearRect(){},drawImage(i){c.drawnImage=i;},getImageData(){const im=c.drawnImage||c;return{data:new Proxy({},{get:(_,k)=>typeof k==='string'&&/^\d+$/.test(k)?Number(k)%4===3?(im.qaAlpha?.[Math.floor(Number(k)/4)]??im.qaUniformAlpha??255):255:undefined})};}};c.getContext=()=>ctx;return c;};
const previous={fetch:globalThis.fetch,document:globalThis.document,load:THREE.TextureLoader.prototype.loadAsync};
const attribute=a=>a?{count:a.count,itemSize:a.itemSize,normalized:a.normalized,offset:a.offset??0,stride:a.data?.stride??null,sha256:sha(Buffer.from((a.array??a.data.array).buffer,(a.array??a.data.array).byteOffset,(a.array??a.data.array).byteLength))}:null;
function textureValues(map){map.updateMatrix();const values={sourceSha256:map.userData.sourceSha256??null,size:[map.image?.width??null,map.image?.height??null]};for(const[k,v]of Object.entries(map)){if(['id','uuid','version','_listeners','userData','source'].includes(k))continue;if(v===null||['string','number','boolean'].includes(typeof v))values[k]=v;else if((v.isVector2||v.isVector3||v.isVector4||v.isMatrix3||v.isMatrix4)&&v.toArray)values[k]=v.toArray();}return values;}
function materialValues(m){const values={};for(const[k,v]of Object.entries(m)){if(['id','uuid','version','_listeners','userData'].includes(k))continue;if(v===null||['string','number','boolean'].includes(typeof v))values[k]=v;else if(v.isTexture)values[k]=textureValues(v);else if((v.isColor||v.isVector2||v.isVector3||v.isVector4||v.isMatrix3||v.isMatrix4)&&v.toArray)values[k]=v.toArray();else if(Array.isArray(v)&&v.every(x=>x===null||['string','number','boolean'].includes(typeof x)))values[k]=v;else if(k==='defines')values[k]=v;}return values;}
function visualDigest(model){
 model.group.updateMatrixWorld(true);const meshes=[];
 model.group.traverse(o=>{if(!o.isMesh)return;const m=o.material;
  // These existing/added cover meshes only route input. Zero-opacity is
  // asserted separately; they contribute no surface to an exported image.
  if(m.opacity===0){assert(o.userData.inputOccluder);assert.equal(m.type,'MeshBasicMaterial');assert.equal(m.depthWrite,false);return;}
  const map=m.map;if(map)map.updateMatrix();const path=[];for(let p=o;p&&p!==model.group;p=p.parent)path.unshift(p.name);
  meshes.push({path,visible:o.visible,matrix:o.matrixWorld.elements,attributes:Object.fromEntries(Object.entries(o.geometry.attributes).map(([k,a])=>[k,attribute(a)])),index:attribute(o.geometry.getIndex()),material:materialValues(m)});;
 });return{sha256:sha(JSON.stringify(meshes)),meshCount:meshes.length};
}
const visible=o=>{for(let p=o;p;p=p.parent)if(!p.visible||p.userData.pickIgnore)return false;return true;};
function ray(model,point,normal){const eye=point.clone().addScaledVector(normal,150),direction=normal.clone().negate(),hit=pickVisibleSurface(new THREE.Raycaster(eye,direction,0,300),model.group);return{eyeMm:eye.toArray(),direction:direction.toArray(),hitMesh:hit?.object?.name??null,controlId:hit?.object?.userData.control??null,hitPointMm:hit?.point?.toArray()??null,distanceMm:hit?.distance??null};}
const presets={original:null,gold:{color:'#d4af37',opacity:1,finish:'metal'},purple:{color:'#8054b3',opacity:.35,finish:'plastic'}};
const rows=[];
try{
 globalThis.fetch=async url=>{const u=new URL(url);return new Response(u.pathname.endsWith('/model.json')?JSON.stringify(servedMetadata):await readFile(u));};globalThis.document={createElement:canvas};
 THREE.TextureLoader.prototype.loadAsync=async url=>{const file=fileURLToPath(url),raw=execFileSync('python',['-c',"import sys,struct;from PIL import Image;im=Image.open(sys.argv[1]);a=('A' in im.getbands())or('transparency' in im.info);sys.stdout.buffer.write(struct.pack('<IIB',*im.size,0 if a else 255));sys.stdout.buffer.write(im.convert('RGBA').getchannel('A').tobytes()if a else b'')",file],{maxBuffer:64*1024*1024});const image={width:raw.readUInt32LE(0),height:raw.readUInt32LE(4),qaUniformAlpha:raw[8]};if(raw.length>9)image.qaAlpha=new Uint8Array(raw.buffer,raw.byteOffset+9,raw.length-9);const t=new THREE.Texture(image);t.userData.sourceSha256=sha(await readFile(new URL(url)));return t;};
 for(const id of ids){
  const path=`web/platform/assets/${id}/model.json`,planPath=`web/platform/assets/${id}/model-plan.json`,beforeRaw=git('show',`${baseline}:${path}`),afterRaw=await readFile(join(overlay,path)),before=JSON.parse(beforeRaw),after=JSON.parse(afterRaw),planRaw=await readFile(join(overlay,planPath)),plan=JSON.parse(planRaw),product=catalog.find(p=>p.id===id);
  assert(product);assert.deepEqual(after.presentation,plan.presentation);assert.deepEqual(before.artifacts,after.artifacts);assert.deepEqual(before.parts.map(p=>[p.byteOffset,p.vertexCount,p.boundsMm,p.texture,p.material,p.color]),after.parts.map(p=>[p.byteOffset,p.vertexCount,p.boundsMm,p.texture,p.material,p.color]));assert.deepEqual(before.derivedParts,after.derivedParts);
  const verifiedArtifacts={};for(const[name,digest]of Object.entries(after.artifacts)){const actual=sha(await readFile(join(root,'web/platform/assets',id,name)));assert.equal(actual,digest);verifiedArtifacts[name]=actual;}
  const models=[];for(const metadata of [before,after]){servedMetadata=metadata;const m=createManufacturerModel(product,new THREE.Texture());await m.ready;m.group.updateMatrixWorld(true);assert.equal(m.group.userData.inspectionError,undefined);models.push(m);}
  const[oldModel,newModel]=models,row={productId:id,baselineRevision:baseline,previousModelSha256:sha(beforeRaw),finalModelSha256:sha(afterRaw),finalPlanSha256:sha(planRaw),artifactManifestSha256:sha(JSON.stringify(after.artifacts)),verifiedArtifacts,productionLoaderVisualDigests:{},surfaces:[],negativeRays:[],scope:'Visible geometry/indices/UVs/world transforms/materials/texture source bytes identical in the actual production loader. Zero-opacity input planes are excluded from the visible digest and asserted non-writing. No CPU rerender or GPU/browser pointer/guest execution claim.'};
  const removedJsonPointers=[];
  for(let i=before.presentation.buttons?.length||0;i<(after.presentation.buttons?.length||0);i++)removedJsonPointers.push(`/presentation/buttons/${i}`);
  for(let g=0;g<(before.presentation.inspectionGroups?.length||0);g++)for(let i=before.presentation.inspectionGroups[g].parts.length;i<after.presentation.inspectionGroups[g].parts.length;i++)removedJsonPointers.push(`/presentation/inspectionGroups/${g}/parts/${i}`);
  const restoredJsonValues=[];for(let i=0;i<before.parts.length;i++)if(before.parts[i].controlId!==after.parts[i].controlId)restoredJsonValues.push({pointer:`/parts/${i}/controlId`,value:before.parts[i].controlId});
  const restoredPreviousMetadataSha256=execFileSync('python',['-c',"import sys,json,hashlib;m=json.load(open(sys.argv[1]));ops=json.loads(sys.argv[2]);\nfor p in reversed(ops['remove']):\n a=m;ks=p.split('/')[1:];\n for k in ks[:-1]:a=a[int(k)]if isinstance(a,list)else a[k]\n k=ks[-1];a.pop(int(k))if isinstance(a,list)else a.pop(k)\nfor op in ops['restore']:\n a=m;ks=op['pointer'].split('/')[1:];\n for k in ks[:-1]:a=a[int(k)]if isinstance(a,list)else a[k]\n a[ks[-1]]=op['value']\nprint(hashlib.sha256((json.dumps(m,ensure_ascii=False,indent=2)+'\\n').encode()).hexdigest())",join(overlay,path),JSON.stringify({remove:removedJsonPointers,restore:restoredJsonValues})],{encoding:'utf8'}).trim();
  assert.equal(restoredPreviousMetadataSha256,sha(beforeRaw),id+' exact prior metadata reconstruction');
  Object.assign(row,{excludedJsonPointers:removedJsonPointers.concat(restoredJsonValues.map(v=>v.pointer)),removedJsonPointers,restoredJsonValues,restoredPreviousMetadataSha256,enginePath:'web/platform/manufacturer-model.js',engineSha256:sha(await readFile(join(root,'web/platform/manufacturer-model.js')))});
  const controls=id==='m5stack-k034'?['btn-a','btn-b','btn-c']:['power'];
  for(const preset of Object.keys(presets)){
   if(presets[preset])for(const m of models)for(const shell of m.group.userData.shellMeshes)applyShellAppearance(shell,{shell:presets[preset]});
   const oldDigest=visualDigest(oldModel),newDigest=visualDigest(newModel);assert.deepEqual(newDigest,oldDigest);row.productionLoaderVisualDigests[preset]={previous:oldDigest.sha256,final:newDigest.sha256,visibleMeshCount:newDigest.meshCount};
   if(preset!=='original')continue;
   for(const control of controls){const region=after.presentation.buttons.find(p=>p.id===control),target=newModel.parts[`cover-key:${control}`];assert(target&&visible(target));assert(newModel.buttons.includes(target));assert.equal(target.material.opacity,0);const normal=new THREE.Vector3(0,0,1).transformDirection(target.matrixWorld),samples=[];
    for(const[x,y]of [[0,0],[-.4,-.4],[.4,-.4],[-.4,.4],[.4,.4]]){const point=target.localToWorld(new THREE.Vector3(x*region.sizeMm[0],y*region.sizeMm[1],0)),prior=ray(oldModel,point,normal),final=ray(newModel,point,normal);assert.equal(final.controlId,control,id+' nearest control');assert(prior.hitPointMm,id+' prior visible supporting face');const distance=new THREE.Vector3(...prior.hitPointMm).distanceTo(point);assert(distance<.1,id+' plane stays within 0.1mm of native visible surface: '+distance);assert.equal(prior.hitMesh,id==='m5stack-k034'?'live-display':region.supportingPart,id+' source supporting surface');samples.push({regionFraction:[x,y],prior,final,supportingSurfaceDistanceMm:distance});}
    row.surfaces.push({controlId:control,region,nearestSamples:samples});
    const center=target.localToWorld(new THREE.Vector3()),reverse=ray(newModel,center,normal.clone().negate());assert.notEqual(reverse.controlId,control,id+' reverse view remains blocked');row.negativeRays.push({kind:'reverse-side-opaque-assembly',controlId:control,...reverse});
    const outside=target.localToWorld(new THREE.Vector3(region.sizeMm[0]*.75,0,0)),off=ray(newModel,outside,normal);assert.notEqual(off.controlId,control,id+' adjacent surface cannot press the button');row.negativeRays.push({kind:'outside-key-region',controlId:control,...off});
   }
  }
  row.inspectionRays=[];
  if(newModel.group.userData.inspection){assert(oldModel.group.userData.inspection);for(const progress of [.5,1,0]){for(const model of models)model.group.userData.inspection.setProgress(progress);newModel.group.updateMatrixWorld(true);oldModel.group.updateMatrixWorld(true);for(const control of controls){const target=newModel.parts[`cover-key:${control}`],point=target.localToWorld(new THREE.Vector3()),normal=new THREE.Vector3(0,0,1).transformDirection(target.matrixWorld),prior=ray(oldModel,point,normal),final=ray(newModel,point,normal);assert.equal(final.controlId,control,id+' expanded control follows shell');assert(prior.hitPointMm);assert(new THREE.Vector3(...prior.hitPointMm).distanceTo(point)<.1);row.inspectionRays.push({progress,controlId:control,prior,final});}}}
  if(id==='m5stack-k010'){const target=oldModel.parts['power-button-cap'];target.geometry.computeBoundingBox();const point=target.localToWorld(target.geometry.boundingBox.getCenter(new THREE.Vector3())),hit=ray(newModel,point,new THREE.Vector3(1,0,0));assert.notEqual(hit.controlId,'power');row.negativeRays.push({kind:'historically-misbound-internal-insert-is-not-power',...hit});}
  for(const m of models){m.dispose();m.group.traverse(o=>{o.geometry?.dispose();if(o.material)(Array.isArray(o.material)?o.material:[o.material]).forEach(v=>v.dispose());});}rows.push(row);
 }
}finally{globalThis.fetch=previous.fetch;globalThis.document=previous.document;THREE.TextureLoader.prototype.loadAsync=previous.load;}
const report={schemaVersion:1,sourceStartCommit,sourceCommit:git('rev-parse','HEAD').toString().trim(),catalogSha256:sha(catalogBytes),baselineRevision:baseline,harnessSha256:sha(await readFile(fileURLToPath(import.meta.url))),loaderSha256:sha(await readFile(join(root,'web/platform/manufacturer-model.js'))),pickerSha256:sha(await readFile(join(root,'web/platform/scene-interaction.js'))),appearanceSha256:sha(await readFile(join(root,'web/platform/shell-appearance.js'))),products:rows,passed:true};await mkdir(dirname(out),{recursive:true});await writeFile(out,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({passed:true,products:rows.length,surfaces:rows.reduce((n,r)=>n+r.surfaces.length,0)}));
