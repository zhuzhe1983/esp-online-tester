#!/usr/bin/env node
// Read the real production loaders and picker from a caller-supplied checkout.
// No source geometry, renderer, product definition or source-tree file is changed.
import {readFile,writeFile,mkdir,access} from 'node:fs/promises';
import {resolve,join,relative,dirname} from 'node:path';
import {pathToFileURL,fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {gunzipSync} from 'node:zlib';
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
const args=process.argv.slice(2),option=(n,d)=>{const i=args.indexOf(n);return i<0?d:args[i+1];};
if(!option('--source-root'))throw Error('Usage: --source-root CHECKOUT --out JSON [--overlay-root CHECKOUT] [--ids id,id]');
const root=resolve(option('--source-root')),overlay=option('--overlay-root')?resolve(option('--overlay-root')):null,out=resolve(option('--out','m5-control-picking.json')),hash=b=>createHash('sha256').update(b).digest('hex');
let baselineView=false;
const THREE=await import(pathToFileURL(join(root,'web/platform/vendor/three.module.min.js'))),{createManufacturerModel}=await import(pathToFileURL(join(root,'web/platform/manufacturer-model.js'))),{createPapermonoModel}=await import(pathToFileURL(join(root,'web/platform/papermono-model.js'))),{createStopwatchModel}=await import(pathToFileURL(join(root,'web/platform/stopwatch-model.js'))),{pickVisibleSurface,createScenePointerRouter}=await import(pathToFileURL(join(root,'web/platform/scene-interaction.js')));
const {applyShellAppearance}=await import(pathToFileURL(join(root,'web/platform/shell-appearance.js')));
const sourceStartCommit=execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim();
const sourceBytes=await readFile(join(root,'web/platform/catalog/products.json')),products=JSON.parse(sourceBytes).items.filter(p=>p.manufacturer==='M5Stack'),ids=option('--ids','m5stack-k049-b,m5stack-k049,m5stack-k049-c,m5stack-k048,m5stack-k146').split(','),rows=[];
async function actualFile(file){if(overlay){const p=join(overlay,relative(root,file));try{await access(p);return p;}catch{}}return file;}
const canvas=()=>{const c={width:1,height:1,qaUniformAlpha:255,qaText:[]};const context={fillText(text,x,y,maxWidth){c.qaText.push({text,x,y,maxWidth,fillStyle:this.fillStyle,font:this.font});},fillRect(){},clearRect(){},drawImage(image){c.drawnImage=image;},getImageData(){const im=c.drawnImage||c;return{data:new Proxy({},{get:(_,key)=>typeof key==='string'&&/^\d+$/.test(key)?Number(key)%4===3?(im.qaAlpha?.[Math.floor(Number(key)/4)]??im.qaUniformAlpha??255):255:undefined})};}};c.getContext=()=>context;return c;};
const previous={fetch:globalThis.fetch,document:globalThis.document,load:THREE.TextureLoader.prototype.loadAsync};
function visible(o){for(let p=o;p;p=p.parent)if(!p.visible||p.userData.pickIgnore)return false;return true;}
function pickControl(hit){if(!hit)return null;const o=hit.object;if(o.userData.encoder){const p=o.userData.encoderPress;if(p&&o.userData.control&&hit.face?.normal.z>.7){const local=o.worldToLocal(hit.point.clone());if(Math.hypot(local.x-p.centerMm[0],local.y-p.centerMm[1])<=p.radiusMm)return o.userData.control;}return null;}return o.userData.control||null;}
function check(model,control,span){
 const targets=model.buttons.filter(o=>o.userData.control===control.id&&visible(o)),r={id:control.id,type:control.type,pin:control.pin??control.gpio??null,transport:control.transport??null,mappedMeshes:targets.map(o=>o.name),raysTried:0,successfulRay:null,blockedBy:[]};
 if(!targets.length){r.status='missing-mapping';return r;}
 const directions=[new THREE.Vector3(0,0,1),new THREE.Vector3(1,0,0),new THREE.Vector3(-1,0,0),new THREE.Vector3(0,1,0),new THREE.Vector3(0,-1,0),new THREE.Vector3(0,0,-1)];
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
function assembly(model,excludeCaps=false){
 const triangles=[];model.group.updateMatrixWorld(true);
 model.group.traverse(o=>{if(!o.isMesh||!visible(o)||o.material.opacity<=.001||excludeCaps&&o.name.startsWith('rotary-cap-'))return;
  const g=o.geometry,p=g.getAttribute('position'),n=g.getAttribute('normal'),uv=g.getAttribute('uv'),idx=g.index,m=o.material;if(Array.isArray(m))throw Error('Unexpected material array');
  const mat=JSON.stringify([m.type,m.color?.toArray(),m.roughness,m.metalness,m.opacity,m.transparent,m.depthWrite,m.side,m.alphaTest,m.polygonOffset,m.polygonOffsetFactor,m.polygonOffsetUnits,m.map?.userData.qaSha256??null,m.map?.matrix.elements??null]);
  for(let i=0;i<(idx?idx.count:p.count);i+=3){const v=[];for(let j=0;j<3;j++){const k=idx?idx.getX(i+j):i+j;v.push(p.getX(k),p.getY(k),p.getZ(k),n?.getX(k)??0,n?.getY(k)??0,n?.getZ(k)??0,uv?.getX(k)??null,uv?.getY(k)??null);}triangles.push(hash(mat+JSON.stringify(v)+JSON.stringify(o.matrixWorld.elements)));}
 });triangles.sort();return{triangles:triangles.length,sha256:hash(triangles.join('\n'))};
}
async function verifyAssembly(product,model,row){
 baselineView=true;const baseline=createManufacturerModel(product,new THREE.Texture());try{await baseline.ready;}finally{baselineView=false;}
 try{
  const colors=[],caps=Object.values(model.parts).filter(o=>o?.name?.startsWith('rotary-cap-'));
  for(const [name,preset]of [['original',null],['gold',{color:'#d4af37',opacity:1,finish:'metal'}],['purple',{color:'#8054b3',opacity:.35,finish:'plastic'}]]){
   if(preset)for(const value of [baseline,model])for(const mesh of value.group.userData.shellMeshes||[])applyShellAppearance(mesh,{shell:preset});
   const original=assembly(baseline),retained=assembly(model,true),complete=assembly(model);assert.deepEqual(retained,original,product.id+' retained triangles/materials changed in '+name);
   for(const cap of caps)assert.equal(cap.material.color.getHexString(),'292a28');colors.push({appearance:name,original,retained,newAssembly:complete,retainedAssemblyExactlyEquivalent:true,capMaterialsIsolated:true});
  }
  // Restore original shell appearance before geometric negative tests.
  for(const mesh of model.group.userData.shellMeshes||[])applyShellAppearance(mesh,{shell:{color:'#e0e0d9',opacity:1,finish:'plastic'}});
  const negative=[],events=[],router=createScenePointerRouter({onInput:e=>events.push(e)});let pointer=1;
  for(const control of row.controls){assert.equal(control.status,'passed');const ray=control.successfulRay,hit=pickVisibleSurface(new THREE.Raycaster(new THREE.Vector3(...ray.eyeMm),new THREE.Vector3(...ray.direction),0,1000),model.group);assert.equal(pickControl(hit),control.id);router.down({pointerId:pointer,x:0,y:0,surface:{kind:'button',id:pickControl(hit)}});router.up(pointer++);
   const target=new THREE.Vector3(...ray.hitPointMm),direction=new THREE.Vector3(...ray.direction),opposite=target.clone().addScaledVector(direction,500),back=pickVisibleSurface(new THREE.Raycaster(opposite,direction.clone().negate(),0,1000),model.group);assert.notEqual(pickControl(back),control.id,'Opposite side penetrates cap '+control.id);negative.push({id:control.id,kind:'opposite-side-occlusion',firstMesh:back?.object?.name??null,firstControl:pickControl(back),passed:true});
  }
  for(let i=0;i<row.controls.length;i++)assert.deepEqual(events.slice(i*2,i*2+2),[{type:'button',id:row.controls[i].id,pressed:true},{type:'button',id:row.controls[i].id,pressed:false}]);
  for(const [name,eye,direction]of [['front-center',[0,0,500],[0,0,-1]],['rear-corner',[23,32,-500],[0,0,1]],...(product.id==='m5stack-k048'?[['physical-RST',[-14,10,-500],[0,0,1]]]:product.id.startsWith('m5stack-k049')?[['physical-RST',[27,24,-500],[0,0,1]]]:[])]){const hit=pickVisibleSurface(new THREE.Raycaster(new THREE.Vector3(...eye),new THREE.Vector3(...direction),0,1000),model.group);assert.equal(pickControl(hit),null);negative.push({kind:'non-button-case-or-screen',name,firstMesh:hit?.object?.name??null,passed:true});}
  return{colors,visibleGeometryDelta:caps.length>0,capMeshes:caps.map(c=>c.name),negative,pointerEvents:events,inspectionError:model.group.userData.inspectionError??null};
 }finally{baseline.dispose();}
}

try{
 globalThis.fetch=async url=>{try{const file=fileURLToPath(url);return new Response(baselineView&&file.endsWith('/model.json')?gunzipSync(await readFile(await actualFile(join(dirname(file),'control-base-model.json.gz')))):await readFile(await actualFile(file)));}catch{return new Response('',{status:404});}};globalThis.document={createElement:canvas};
 THREE.TextureLoader.prototype.loadAsync=async url=>{const file=await actualFile(fileURLToPath(url));const raw=execFileSync('python',['-c',"import sys,struct;from PIL import Image;im=Image.open(sys.argv[1]);a=('A' in im.getbands())or('transparency' in im.info);sys.stdout.buffer.write(struct.pack('<IIB',*im.size,0 if a else 255));sys.stdout.buffer.write(im.convert('RGBA').getchannel('A').tobytes()if a else b'')",file],{maxBuffer:128*1024*1024});const image={width:raw.readUInt32LE(0),height:raw.readUInt32LE(4),qaUniformAlpha:raw[8]};if(raw.length>9)image.qaAlpha=new Uint8Array(raw.buffer,raw.byteOffset+9,raw.length-9);const texture=new THREE.Texture(image);texture.userData.qaFile=relative(root,fileURLToPath(url));texture.userData.qaSha256=hash(await readFile(file));return texture;};
 for(const p of products){if(ids&&!ids.includes(p.id))continue;const row={productId:p.id,name:p.name,board:p.runtime?.board,controls:[],matrixControls:[],errors:[]};rows.push(row);let m;
  try{m=p.id==='m5stack-papermono'?createPapermonoModel(new THREE.Texture()):p.id==='m5stack-stopwatch'?createStopwatchModel(new THREE.Texture()):createManufacturerModel(p,new THREE.Texture());const meta=await m.ready;m.group.updateMatrixWorld(true);const path=join(root,'web/platform',p.model?.url||`assets/${p.id}/model.json`);row.modelSha256=hash(await readFile(await actualFile(path)));row.loader=p.id==='m5stack-papermono'?'dedicated-papermono':p.id==='m5stack-stopwatch'?'dedicated-stopwatch':'manufacturer';row.modelButtons=m.buttons.map(o=>({mesh:o.name,control:o.userData.control}));row.sourceButtonParts=(meta.parts||[]).filter(o=>o.controlId||o.role==='button'||/button|key|cap|knob/i.test(o.name)).map(o=>({name:o.name,role:o.role,controlId:o.controlId??null,sourceComponent:o.sourceComponent??null,boundsMm:o.boundsMm}));row.derivedButtonParts=(meta.derivedParts||[]).filter(o=>o.controlId||/button|key|cap|knob/i.test(o.id)).map(o=>({id:o.id,kind:o.kind,controlId:o.controlId??null,geometry:o.geometry,sourceIds:o.sourceIds}));
   const span=Math.max(100,new THREE.Box3().setFromObject(m.group).getSize(new THREE.Vector3()).length()*2),supported=(p.controls||[]).filter(c=>c.type==='button'&&c.simulationSupported!==false&&(c.pin!==undefined||c.gpio!==undefined||c.transport==='board'));
   row.controls=supported.map(c=>check(m,c,span));row.matrixControls=(p.controls||[]).filter(c=>c.type==='matrix-key'&&c.simulationSupported!==false).map(c=>({...check(m,c,span),row:c.row,column:c.column}));row.inspectionError=m.group.userData.inspectionError??null;row.assembly=await verifyAssembly(p,m,row);
  }catch(e){row.errors.push(e.message);}finally{m?.dispose();m?.group.traverse(o=>{o.geometry?.dispose();if(o.material)(Array.isArray(o.material)?o.material:[o.material]).forEach(v=>v.dispose());});}
  if(rows.length%10===0)console.log('Audited',rows.length,'/',products.length);
 }
}finally{globalThis.fetch=previous.fetch;globalThis.document=previous.document;THREE.TextureLoader.prototype.loadAsync=previous.load;}
const buttonFindings=rows.flatMap(r=>r.controls.filter(c=>c.status!=='passed').map(c=>({productId:r.productId,...c}))),matrixFindings=rows.flatMap(r=>r.matrixControls.filter(c=>c.status!=='passed').map(c=>({productId:r.productId,...c})));
const report={schemaVersion:1,checkedAt:new Date().toISOString(),sourceStartCommit,sourceCommit:execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),catalogSha256:hash(sourceBytes),harnessSha256:hash(await readFile(fileURLToPath(import.meta.url))),productionPickerSha256:hash(await readFile(join(root,'web/platform/scene-interaction.js'))),productionCodeHashes:Object.fromEntries(await Promise.all(['manufacturer-model.js','papermono-model.js','stopwatch-model.js','scene-view.js','shell-appearance.js'].map(async file=>[file,hash(await readFile(join(root,'web/platform',file)))]))),summary:{products:rows.length,buttonProducts:rows.filter(r=>r.controls.length).length,supportedButtons:rows.reduce((n,r)=>n+r.controls.length,0),buttonFindings:buttonFindings.length,buttonFindingSkus:new Set(buttonFindings.map(c=>c.productId)).size,matrixControls:rows.reduce((n,r)=>n+r.matrixControls.length,0),matrixFindings:matrixFindings.length,matrixFindingSkus:new Set(matrixFindings.map(c=>c.productId)).size,loaderErrors:rows.filter(r=>r.errors.length).length},buttonFindings,matrixFindings,products:rows,scope:'Actual production loader and production pickVisibleSurface, assembled default appearance. Original photograph alpha is decoded. Canvas UNKNOWN text uses conservatively opaque alpha in Node; any such occluder is reported for review. No constructor-only success, browser pointer test, firmware execution or physical-position calibration claim.'};await mkdir(dirname(out),{recursive:true});await writeFile(out,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report.summary));

if(report.summary.loaderErrors||report.summary.buttonFindings)process.exitCode=1;
