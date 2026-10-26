#!/usr/bin/env node
// Fixed source identity + independently published dimensions; no browser or Python dependencies.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
const root=new URL('../',import.meta.url),base=new URL('web/platform/assets/m5stack-stopwatch/',root);
const digest=b=>createHash('sha256').update(b).digest('hex');
const modelBytes=await readFile(new URL('model.json',base)),model=JSON.parse(modelBytes);
for(const [name,hash] of Object.entries(model.artifacts))assert.equal(digest(await readFile(new URL(name,base))),hash,`${name} changed without updating source receipt`);
assert.equal(model.provenance.sourceSha256,'48b292c76f929acb75868cba47ccda86800fbff7cfff172161df64ff87f220d5');
const geometry=await readFile(new URL('geometry.bin',base)),view=new DataView(geometry.buffer,geometry.byteOffset,geometry.byteLength);
assert.equal(model.unit,'mm');assert.equal(model.sourceTransform.scale,1);
let byteLength=0;
for(const part of model.parts){
 assert.equal(part.byteOffset,byteLength);assert.equal(part.vertexCount%3,0);
 const low=[Infinity,Infinity,Infinity],high=[-Infinity,-Infinity,-Infinity];
 for(let i=0;i<part.vertexCount;i++)for(let axis=0;axis<3;axis++){const value=view.getFloat32(part.byteOffset+(i*6+axis)*4,true);assert.ok(Number.isFinite(value));low[axis]=Math.min(low[axis],value);high[axis]=Math.max(high[axis],value);}
 for(let j=0;j<3;j++){assert.ok(Math.abs(low[j]-part.boundsMm[0][j])<.001);assert.ok(Math.abs(high[j]-part.boundsMm[1][j])<.001);}
 if(part.name==='shell'){assert.ok(Math.abs(high[0]-low[0]-51.95)<.05);assert.ok(Math.abs(high[1]-low[1]-51.95)<.05);assert.ok(Math.abs(high[2]-low[2]-15)<.001);}
 byteLength+=part.vertexCount*6*4;
}
assert.equal(byteLength,geometry.byteLength);assert.deepEqual(model.parts.map(p=>p.name),['shell','power-button','button-b','button-a']);
const report=JSON.parse(await readFile(new URL('docs/platform/evidence/stopwatch-model/verification.json',root)));
for(const [key,file] of [['rendererSha256','web/platform/stopwatch-model.js'],['modelReviewSha256','web/platform/model-review.js'],['verificationScriptSha256','tools/verify-stopwatch-model.py']])assert.equal(report[key],digest(await readFile(new URL(file,root))),`Re-run model verification for changed ${file}`);
assert.equal(report.modelSha256,digest(modelBytes));assert.equal(report.geometrySha256,digest(geometry));
assert.ok(report.dimensions.every(v=>v.pass));assert.equal(report.photos.length,4);assert.ok(report.photos.every(v=>v.pass));
console.log('ok pinned manufacturer CAD, component bounds in mm, binary integrity, drawing dimensions and current four-view verification receipt');
