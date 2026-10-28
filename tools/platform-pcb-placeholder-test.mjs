import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createPcbPlaceholderGeometry} from '../web/platform/pcb-placeholder.js';
const model=JSON.parse(await readFile(new URL('../web/platform/assets/lilygo-t-embed-cc1101/model.json',import.meta.url)));
const pcb=model.derivedParts.find(p=>p.id==='pcb-main');const geometry=createPcbPlaceholderGeometry(pcb.geometry.sizeMm,pcb.geometry.shape),p=geometry.getAttribute('position');
// Manufacturer case's 98 × 39 mm stadium envelope. The previous rectangular
// board's four corners were outside it even though its AABB looked smaller.
const caseClearance=(x,y)=>19.5-Math.hypot(Math.max(0,Math.abs(x)-29.5),y);
assert.ok(caseClearance(42,16)<0,'Old rectangular corner is outside the official rounded shell');
for(let i=0;i<p.count;i++){assert.ok(caseClearance(p.getX(i),p.getY(i))>3,'Every PCB vertex stays inside the case outline');assert.ok(Math.abs(p.getZ(i))<=.60001);}
geometry.computeBoundingBox();assert.ok(Math.abs(geometry.boundingBox.max.x-42)<.0001);assert.ok(Math.abs(geometry.boundingBox.max.y-16)<.0001);geometry.dispose();
for(const shape of ['box','disc']){const g=createPcbPlaceholderGeometry([20,20,1],shape);g.computeBoundingBox();assert.equal(g.boundingBox.max.x,10);g.dispose();}
assert.throws(()=>createPcbPlaceholderGeometry([10,20,1],'capsule'));
console.log('ok rounded UNKNOWN PCB stays within the official case outline; box/disc behavior retained');
