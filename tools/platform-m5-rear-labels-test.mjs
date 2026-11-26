// Actual loader, original asset hashes and fixed paper material in three shell presets.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';
import {Texture,TextureLoader} from '../web/platform/vendor/three.module.min.js';
const root=new URL('../web/platform/',import.meta.url),products=JSON.parse(await readFile(new URL('catalog/products.json',root))).items;
const sha=b=>createHash('sha256').update(b).digest('hex');
for(const id of ['m5stack-k001-v26','m5stack-k002','m5stack-k010-v11','m5stack-k001-v27','m5stack-k001','m5stack-k001-b','m5stack-k128-lite'])test(id+' rear paper is a source-masked fixed material attached to its original cover',async()=>{
 const p=products.find(p=>p.id===id),url=new URL(p.model.url,root),meta=JSON.parse(await readFile(url)),plan=JSON.parse(await readFile(new URL('model-plan.json',url))),spec=plan.rearLabel;
 assert.equal(sha(await readFile(new URL(meta.binary,url))),spec.originalGeometrySha256);
 for(const[file,digest]of Object.entries(meta.artifacts))assert.equal(sha(await readFile(new URL(file,url))),digest,id+' '+file);
 assert.equal(spec.texture.sourceSha256,sha(await readFile(new URL(spec.texture.sourceFile,url))));
 const original={fetch:globalThis.fetch,load:TextureLoader.prototype.loadAsync};
 try{globalThis.fetch=async u=>new Response(await readFile(new URL(u)));TextureLoader.prototype.loadAsync=async()=>new Texture();
  const model=createManufacturerModel(p,null);await model.ready;const label=model.parts['rear-paper'];assert.ok(label?.material.map,'production loader must actually bind the new image');assert.equal(label.material.alphaTest,.5);assert.equal(label.userData.stickerSurface,true);
  const labels=[spec.part,...(spec.additionalParts||[]).filter(p=>p.kind==='label')].map(p=>model.parts[p.id]);for(const paper of labels){assert.ok(paper.material.map);assert.equal(paper.material.alphaTest,.5);assert.equal(paper.userData.stickerSurface,true);}const fixed=labels.map(p=>({p,map:p.material.map,color:p.material.color.getHex()}));
  const map=label.material.map,color=label.material.color.getHex(),before=label.position.clone(),carrier=model.parts[id==='m5stack-k010-v11'?'shell-back':id==='m5stack-k001-v27'?'shell-back-128':id==='m5stack-k128-lite'?'rear-shell':'photo-derived-bottom-cover'],carrierBefore=carrier.position.clone();
  for(const shell of [{color:'#d5ad40',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}]){for(const mesh of model.group.userData.shellMeshes)applyShellAppearance(mesh,{shell});assert.equal(label.material.color.getHex(),color);assert.equal(label.material.opacity,1);assert.equal(label.material.map,map);for(const f of fixed){assert.equal(f.p.material.color.getHex(),f.color);assert.equal(f.p.material.map,f.map);assert.equal(f.p.material.opacity,1);}}
  model.group.userData.inspection.setExpanded(true);assert.ok(label.position.clone().sub(before).distanceTo(carrier.position.clone().sub(carrierBefore))<1e-9);model.group.userData.inspection.setExpanded(false);assert.deepEqual(label.position.toArray(),before.toArray());model.dispose();
 }finally{globalThis.fetch=original.fetch;TextureLoader.prototype.loadAsync=original.load;}
});
