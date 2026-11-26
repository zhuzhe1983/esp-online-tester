import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
const root=pathToFileURL(process.argv[2].replace(/\/$/,'')+'/'),base=new URL('web/platform/',root);
const THREE=await import(new URL('vendor/three.module.min.js',base)),{createManufacturerModel}=await import(new URL('manufacturer-model.js',base)),{applyShellAppearance}=await import(new URL('shell-appearance.js',base));
const product=JSON.parse(await readFile(new URL('catalog/products.json',base),'utf8')).items.find(p=>p.id==='m5stack-k014-b');
globalThis.document={createElement:()=>({getContext:()=>({fillText(){}})})};globalThis.fetch=async url=>{const u=new URL(url);u.search='';return new Response(await readFile(u));};
const tex=new THREE.DataTexture(new Uint8Array([0,0,0,255]),1,1),model=createManufacturerModel(product,tex);await model.ready;
try{
 assert.ok(model.screen.material.map===tex);const held=[];for(const id of ['button-a','button-b','button-c','left-light-diffuser','right-light-diffuser','front-bezel','usb-c','grove-a'])model.parts[id].traverse(p=>{if(p.material)held.push({id,p,color:p.material.color.getHex(),opacity:p.material.opacity,map:p.material.map});});
 const rays=[];model.group.updateMatrixWorld(true);
 for(const id of ['button-a','button-b','button-c']){const b=new THREE.Box3().setFromObject(model.parts[id]),center=b.getCenter(new THREE.Vector3()),origin=new THREE.Vector3(center.x,center.y,100);const first=new THREE.Raycaster(origin,new THREE.Vector3(0,0,-1)).intersectObject(model.group,true).find(h=>h.object.material?.opacity>=.9)?.object;assert.ok(first===model.parts[id],id+' occluded by '+first?.name);rays.push({part:id,firstOpaqueHit:first.name,origin:origin.toArray()});}
 for(const shell of [{color:'#d4ac36',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}]){for(const mesh of model.group.userData.shellMeshes)applyShellAppearance(mesh,{shell});for(const x of held){assert.equal(x.p.material.color.getHex(),x.color,x.id);assert.equal(x.p.material.opacity,x.opacity,x.id);assert.equal(x.p.material.map,x.map,x.id);}}
 const result={productId:product.id,keyCenterRays:rays,protectedMaterialMeshes:held.length,protectedParts:[...new Set(held.map(x=>x.id))],goldAndPurpleShellMaterialIsolation:'passed',framebufferMapBinding:'passed',scope:'Actual Three geometry/ray/material checks only. No browser rendering, text legibility, physical fit or firmware execution.'};await writeFile(process.argv[3],JSON.stringify(result,null,2)+'\n');console.log(result);
}finally{model.dispose();tex.dispose();}
