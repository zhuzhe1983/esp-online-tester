// Load every new LilyGO declarative model through the production loader.
// Verifies finite geometry, retained-source hashes, live-screen aspect and the
// honest preview mode. It does not claim a manufactured dimensional inspection.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';

const root=new URL('../',import.meta.url),patches=JSON.parse(await readFile(new URL('docs/platform/research/lilygo-model-catalog-patches-2026-11-22.json',root),'utf8'));
const nativeFetch=globalThis.fetch,counts={},failures=[];let triangles=0,bytes=0;
globalThis.fetch=async value=>{const url=new URL(value);url.search='';try{return new Response(await readFile(fileURLToPath(url)));}catch{return new Response('',{status:404});}};
try{
  for(const item of patches.items){
    let model;
    try{
      const folder=new URL('web/platform/assets/'+item.id+'/',root),plan=JSON.parse(await readFile(new URL('model-plan.json',folder),'utf8'));
      assert.equal(plan.id,item.id);assert.equal(plan.mode,item.model.basis);assert.ok(['manufacturer-cad','photo-derived','screen-only','debug-placeholder'].includes(plan.mode));
      const product={id:item.id,model:item.model,display:item.displayHint||null,controls:[...new Set((plan.parts||[]).map(p=>p.controlId).filter(Boolean).concat((plan.presentation?.buttons||[]).map(p=>p.id)))].map(id=>({id,type:'button'}))};
      model=createManufacturerModel(product,null);const metadata=await model.ready;
      for(const [file,expected]of Object.entries(metadata.artifacts||{})){const raw=await readFile(new URL(file,folder));assert.equal(createHash('sha256').update(raw).digest('hex'),expected,`${item.id}/${file}: source hash`);if(file===metadata.binary)bytes+=raw.byteLength;}
      assert.ok(!model.group.userData.inspectionError,model.group.userData.inspectionError);
      if(item.displayHint){assert.ok(model.screen,`${item.id}: live display surface missing`);const [w,h]=metadata.presentation.screen.sizeMm;assert.ok(Math.abs(w/h-item.displayHint.width/item.displayHint.height)<1e-7,`${item.id}: native framebuffer aspect mismatch`);}
      triangles+=metadata.parts.reduce((sum,p)=>sum+p.vertexCount/3,0);counts[plan.mode]=(counts[plan.mode]||0)+1;
    }catch(error){failures.push({id:item.id,error:error.stack});}finally{model?.dispose();}
  }
}finally{globalThis.fetch=nativeFetch;}
console.log(JSON.stringify({count:patches.items.length,counts,triangles,geometryBytes:bytes,failures},null,2));
assert.equal(failures.length,0,'LilyGO model asset verification failed');
