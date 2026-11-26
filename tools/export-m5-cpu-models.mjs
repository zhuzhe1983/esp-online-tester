#!/usr/bin/env node
// Export actual production Three meshes, world transforms, UVs and materials for
// an independent CPU renderer. Reads source assets; never writes the source tree.
import {readFile,writeFile,mkdir,access} from 'node:fs/promises';
import {resolve,relative,dirname,join} from 'node:path';
import {readdir} from 'node:fs/promises';
import {pathToFileURL,fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const args=process.argv.slice(2),option=(name,fallback)=>{const i=args.indexOf(name);return i<0?fallback:args[i+1];};
const repo=resolve(option('--repo','.')),out=resolve(option('--out','/tmp/m5-cpu-qa'));
let ids=(option('--ids','m5stack-k001-v26,m5stack-k002,m5stack-k007-v26,m5stack-k010-v11,m5stack-k016-c,m5stack-k016-p,m5stack-k016-p2,m5stack-k016-t2,m5stack-c008,m5stack-c008-b,m5stack-c008-c,m5stack-k117,m5stack-k115,m5stack-k049-b,m5stack-k048,m5stack-c139,m5stack-u017,m5stack-u038,m5stack-u082-x,m5stack-u149,m5stack-u120')).split(',');
const overlay=option('--overlay-root')?resolve(option('--overlay-root')):null;
async function actualFile(file){if(overlay){const p=join(overlay,relative(repo,file));try{await access(p);return p;}catch{}}return file;}
const hash=b=>createHash('sha256').update(b).digest('hex');
const THREE=await import(pathToFileURL(join(repo,'web/platform/vendor/three.module.min.js'))),{createManufacturerModel}=await import(pathToFileURL(join(repo,'web/platform/manufacturer-model.js')));
const products=JSON.parse(await readFile(join(repo,'web/platform/catalog/products.json'))).items;
if(args.includes('--all'))ids=products.filter(p=>p.manufacturer==='M5Stack').map(p=>p.id);
const {createPapermonoModel}=await import(pathToFileURL(join(repo,'web/platform/papermono-model.js'))),{createStopwatchModel}=await import(pathToFileURL(join(repo,'web/platform/stopwatch-model.js'))),{applyShellAppearance}=await import(pathToFileURL(join(repo,'web/platform/shell-appearance.js')));
await mkdir(out,{recursive:true});
const previous={fetch:globalThis.fetch,document:globalThis.document,load:THREE.TextureLoader.prototype.loadAsync};
const rows=[];
try {
 globalThis.fetch=async url=>{try{return new Response(await readFile(await actualFile(fileURLToPath(url))));}catch{return new Response('',{status:404});}};
 globalThis.document={createElement:()=>{const canvas={width:1,height:1,qaText:[],getContext:()=>({fillStyle:'#ffffff',font:'20px monospace',textAlign:'left',textBaseline:'middle',fillText(text,x,y,maxWidth){canvas.qaText.push({text,x,y,maxWidth,fillStyle:this.fillStyle,font:this.font,textAlign:this.textAlign,textBaseline:this.textBaseline});}})};return canvas;}};
 THREE.TextureLoader.prototype.loadAsync=async url=>{const logical=fileURLToPath(url),file=await actualFile(logical);const bytes=await readFile(file);const size=JSON.parse(execFileSync('python',['-c','import sys,json;from PIL import Image;print(json.dumps(Image.open(sys.argv[1]).size))',file],{encoding:'utf8'}));const texture=new THREE.Texture({width:size[0],height:size[1]});texture.userData.qaFile=relative(repo,logical);texture.userData.qaSha256=hash(bytes);return texture;};
 for(const id of ids) {
  const product=products.find(p=>p.id===id);if(!product)throw Error('Unknown model '+id);
  let model;const row={id,name:product.name,errors:[]};rows.push(row);
  try {
   const screen=new THREE.Texture();screen.userData.qaConstantColor=[0,0,0,255];model=id==='m5stack-papermono'?createPapermonoModel(screen):id==='m5stack-stopwatch'?createStopwatchModel(screen):createManufacturerModel(product,screen);const meta=await model.ready;const inspectionProgress=Number(option('--inspection','0'));if(inspectionProgress){if(!model.group.userData.inspection)throw Error('No production inspection controller');model.group.userData.inspection.setProgress(inspectionProgress);}model.group.updateMatrixWorld(true);
   const modelFile=join(repo,'web/platform',product.model?.url||`./assets/${id}/model.json`),assetDir=dirname(modelFile),materials=[],materialIds=new Map(),materialMeshes=[],meshes=[],blobs=[];let bytes=0;
   const append=a=>{const b=Buffer.from(a.buffer,a.byteOffset,a.byteLength),offset=bytes;blobs.push(b);bytes+=b.length;return offset;};
   const materialId=(mat,mesh)=>{
    if(materialIds.has(mat.uuid))return materialIds.get(mat.uuid);let map=null;
    if(mat.map){mat.map.updateMatrix();map={file:mat.map.userData.qaFile||null,sha256:mat.map.userData.qaSha256||null,constant:mat.map.userData.qaConstantColor||null,text:mat.map.image?.qaText||null,size:mat.map.image?.qaText?[mat.map.image.width,mat.map.image.height]:null,matrix:mat.map.matrix.elements,flipY:mat.map.flipY,wrapS:mat.map.wrapS,wrapT:mat.map.wrapT,colorSpace:mat.map.colorSpace};}
    const record={type:mat.type,customShader:mat.isShaderMaterial===true||mat.onBeforeCompile!==THREE.Material.prototype.onBeforeCompile,polygonOffset:mat.polygonOffset===true,polygonOffsetFactor:mat.polygonOffsetFactor,polygonOffsetUnits:mat.polygonOffsetUnits,unsupportedFeatures:['normalMap','bumpMap','displacementMap','aoMap','envMap'].filter(key=>mat[key]).concat(['transmission','clearcoat','iridescence','anisotropy'].filter(key=>(mat[key]||0)>0)),name:mat.name||`material-${materials.length}`,color:[mat.color?.r??1,mat.color?.g??1,mat.color?.b??1],emission:[mat.emissive?.r??0,mat.emissive?.g??0,mat.emissive?.b??0],emissiveIntensity:mat.emissiveIntensity??0,metalness:mat.metalness??0,roughness:mat.roughness??1,opacity:mat.opacity??1,alphaTest:mat.alphaTest??0,unlit:mat.isMeshBasicMaterial===true,doubleSided:mat.side===THREE.DoubleSide,map};const index=materials.length;materials.push(record);materialMeshes.push(mesh);materialIds.set(mat.uuid,index);return index;
   };
   model.group.traverse(mesh=>{
    if(!mesh.isMesh||!mesh.geometry||Array.isArray(mesh.material)||mesh.material.opacity<=.001)return;
    for(let p=mesh;p;p=p.parent)if(!p.visible)return;
    const g=mesh.geometry,pos=g.getAttribute('position'),normal=g.getAttribute('normal'),uv=g.getAttribute('uv');if(!pos?.count)return;
    const cpuDepthOffsetMm=mesh.material.map&&mesh.material.polygonOffset&&(mesh.material.polygonOffsetFactor<0||mesh.material.polygonOffsetUnits<0)?.005:0;
    const p=new Float32Array(pos.count*3),n=new Float32Array(pos.count*3),u=uv?new Float32Array(pos.count*2):null,v=new THREE.Vector3(),nm=new THREE.Matrix3().getNormalMatrix(mesh.matrixWorld);
    for(let i=0;i<pos.count;i++){v.fromBufferAttribute(pos,i).applyMatrix4(mesh.matrixWorld);p.set(v.toArray(),i*3);if(normal){v.fromBufferAttribute(normal,i).applyNormalMatrix(nm);n.set(v.toArray(),i*3);if(cpuDepthOffsetMm){p[i*3]+=v.x*cpuDepthOffsetMm;p[i*3+1]+=v.y*cpuDepthOffsetMm;p[i*3+2]+=v.z*cpuDepthOffsetMm;}}if(u){u[i*2]=uv.getX(i);u[i*2+1]=uv.getY(i);}}
    const index=g.getIndex(),faces=index?Uint32Array.from(index.array):Uint32Array.from({length:pos.count},(_,i)=>i);
    meshes.push({name:mesh.name||'mesh',cpuDepthOffsetMm,vertices:pos.count,triangles:faces.length/3,positionOffset:append(p),normalOffset:normal?append(n):null,uvOffset:u?append(u):null,indexOffset:append(faces),material:materialId(mesh.material,mesh),sourceRole:mesh.userData.kind||null});
   });
   const binary=Buffer.concat(blobs);let photos=meta.referencePhotos||[];if(!photos.length)photos=(await readdir(assetDir)).filter(f=>/^reference-photo-.*\.(webp|png|jpg)$/.test(f)).map(file=>({file,sourceId:file.replace(/^reference-|\.[^.]+$/g,''),view:'retained-official-photo'}));const references=photos.map(photo=>({...photo,file:relative(repo,join(assetDir,photo.file)),exists:true}));
   for(const photo of references){try{photo.actualSha256=hash(await readFile(join(repo,photo.file)));}catch{photo.exists=false;}}
   const shellMeshes=model.group.userData.shellMeshes||Object.values(model.parts||{}).filter(mesh=>mesh?.isMesh&&mesh.userData.originalAppearance?.shell);const appearanceVariants={};for(const [name,preset]of Object.entries({gold:{color:'#d4af37',opacity:1,finish:'metal'},purple:{color:'#8054b3',opacity:.35,finish:'plastic'}})){for(const mesh of shellMeshes)applyShellAppearance(mesh,{shell:preset});appearanceVariants[name]=materialMeshes.map(mesh=>{const m=mesh.material;return{color:[m.color.r,m.color.g,m.color.b],roughness:m.roughness??1,metalness:m.metalness??0,opacity:m.opacity,doubleSided:m.side===THREE.DoubleSide};});}const data={schemaVersion:1,id,name:product.name,inspectionProgress,appearanceVariants,units:'mm',sourceModel:relative(repo,modelFile),sourceModelSha256:hash(await readFile(await actualFile(modelFile))),meshes,materials,references,screenPolicy:'Blank black framebuffer texture; no static firmware photograph.',geometryFile:id+'.bin',geometrySha256:hash(binary),sourceLimitations:meta.limitations||[],cpuDepthBiasPolicy:'Only textured meshes with negative Three polygonOffset get0.005mm displacement along their world normals in this CPU export. This approximates GPU depth bias; source assets are unchanged.',materialScope:'MeshBasic/Standard/Physical RGB, map UV transform, alpha test, side, opacity, roughness and metalness. customShader and unsupportedFeatures are explicitly recorded; private user stickers and non-mesh glows are not loaded.',exportScope:'Actual production loader output, all world transforms baked; original UV/texture transforms and alpha tests retained. Invisible hit planes omitted.'};
   await writeFile(join(out,id+'.bin'),binary);await writeFile(join(out,id+'.json'),JSON.stringify(data,null,2)+'\n');Object.assign(row,{inspectionProgress,meshes:meshes.length,triangles:meshes.reduce((n,m)=>n+m.triangles,0),materialTypes:[...new Set(materials.map(m=>m.type))],customShaderMaterials:materials.filter(m=>m.customShader).map(m=>m.name),unsupportedMaterialFeatures:materials.flatMap(m=>m.unsupportedFeatures),cpuDepthBiasedMeshes:meshes.filter(m=>m.cpuDepthOffsetMm).map(m=>m.name),textureFiles:materials.filter(m=>m.map?.file).map(m=>m.map.file),metadataSha256:data.sourceModelSha256,geometrySha256:data.geometrySha256});
   console.log(JSON.stringify(row));
  }catch(error){row.errors.push(error.message);console.error(id+': '+error.stack);}finally{model?.dispose();}
 }
}finally{globalThis.fetch=previous.fetch;globalThis.document=previous.document;THREE.TextureLoader.prototype.loadAsync=previous.load;}
await writeFile(join(out,'export-receipt.json'),JSON.stringify({checkedAt:new Date().toISOString(),sourceRepoHead:execFileSync('git',['rev-parse','HEAD'],{cwd:repo,encoding:'utf8'}).trim(),scope:'CPU export of actual Three model output. Not browser GPU, calibrated photography, or factory firmware acceptance.',products:rows},null,2)+'\n');
if(rows.some(r=>r.errors.length))process.exitCode=1;
