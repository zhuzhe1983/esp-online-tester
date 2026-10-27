import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createPcbPlaceholderGeometry} from '../web/platform/pcb-placeholder.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';

function clip(poly,z,sign) {
  const out=[];
  for(let i=0;i<poly.length;i++) {
    const a=poly[i],b=poly[(i+1)%poly.length],insideA=sign*(a[2]-z)>=0,insideB=sign*(b[2]-z)>=0;
    if(insideA)out.push(a);
    if(insideA!==insideB)out.push(a.map((x,k)=>x+(b[k]-x)*(z-a[2])/(b[2]-a[2])));
  }
  return out;
}
function radius(poly) {
  if(!poly.length)return Infinity;
  let minimum=Infinity,positive=false,negative=false,area=0;
  for(let i=0;i<poly.length;i++) {
    const a=poly[i],b=poly[(i+1)%poly.length],x=b[0]-a[0],y=b[1]-a[1],squared=x*x+y*y;
    const t=squared?Math.max(0,Math.min(1,-(a[0]*x+a[1]*y)/squared)):0;
    minimum=Math.min(minimum,Math.hypot(a[0]+t*x,a[1]+t*y));
    const cross=a[0]*b[1]-a[1]*b[0];positive ||= cross>1e-9;negative ||= cross < -1e-9;area+=cross;
  }
  return Math.abs(area)>1e-9 && !(positive&&negative)?0:minimum;
}

test('both Dial UNKNOWN proxies clear every CAD triangle in their full thickness',async()=>{
  for(const id of ['m5stack-k130','m5stack-k130-v11']) {
    const root=new URL(`../web/platform/assets/${id}/`,import.meta.url),meta=JSON.parse(await readFile(new URL('model.json',root))),plan=JSON.parse(await readFile(new URL('model-plan.json',root)));
    const pcb=meta.derivedParts.find(p=>p.id==='pcb-main');assert.equal(pcb.kind,'pcb-placeholder');assert.equal(pcb.label,'UNKNOWN');assert.equal(pcb.basis,'estimated');assert.deepEqual(pcb,plan.derivedParts.find(p=>p.id==='pcb-main'));
    const shape=createPcbPlaceholderGeometry(pcb.geometry.sizeMm,pcb.geometry.shape),position=shape.getAttribute('position');let maxRadius=0,minZ=Infinity,maxZ=-Infinity;
    for(let i=0;i<position.count;i++){maxRadius=Math.max(maxRadius,Math.hypot(position.getX(i),position.getY(i)));minZ=Math.min(minZ,position.getZ(i));maxZ=Math.max(maxZ,position.getZ(i));}
    assert.ok(maxRadius<15.00001);assert.ok(Math.abs(maxZ-minZ-1)<1e-6);shape.dispose();
    const bytes=await readFile(new URL(meta.binary,root)),data=new Float32Array(bytes.buffer,bytes.byteOffset,bytes.byteLength/4);let gap=Infinity;
    for(const part of meta.parts.filter(p=>p.sourceComponent!==null)) {
      for(let offset=part.byteOffset/4;offset<(part.byteOffset/4+part.vertexCount*6);offset+=18) {
        const triangle=[0,6,12].map(k=>[data[offset+k]-pcb.geometry.centerMm[0],data[offset+k+1]-pcb.geometry.centerMm[1],data[offset+k+2]]);
        const slice=clip(clip(triangle,minZ+pcb.geometry.centerMm[2],1),maxZ+pcb.geometry.centerMm[2],-1);
        gap=Math.min(gap,radius(slice)-maxRadius);
      }
    }
    assert.ok(gap>=.67,`${id}: CAD radial clearance ${gap}mm`);
    assert.ok(meta.parts.some(p=>p.sourceComponent===10),'The real mounting nut remains present');
    const carrier=meta.parts.find(p=>p.sourceComponent===5);assert.ok(carrier.boundsMm[0][2]-(pcb.geometry.centerMm[2]+maxZ)>=.4999);
    if(id==='m5stack-k130'){assert.equal(meta.parts.find(p=>p.name==='grip-ring').encoderId,'encoder');assert.equal(meta.parts.find(p=>p.name==='front-bezel').controlId,'button');}
  }
});

test('renderer keeps UNKNOWN disc and legacy rectangular placeholders separate from shell material',async()=>{
  const nativeFetch=globalThis.fetch;
  try {
    const vertices=new Float32Array([-1,-1,0,0,0,1,1,-1,0,0,0,1,0,1,0,0,0,1]);
    const meta={unit:'mm',strideFloats:6,binary:'geometry.bin',parts:[{name:'shell',role:'shell',byteOffset:0,vertexCount:3}],derivedParts:[{id:'pcb-main',kind:'pcb-placeholder',geometry:{shape:'disc',sizeMm:[30,30,1],centerMm:[0,0,-4.16]}},{id:'legacy-pcb',kind:'pcb-placeholder',dimensionsMm:[20,10,1]}]};
    globalThis.fetch=async url=>new Response(String(url).includes('model.json')?JSON.stringify(meta):vertices.buffer);
    const model=createManufacturerModel({id:'test',model:{url:'./assets/test/model.json'}},null);
    try {await model.ready;const pcb=model.parts['pcb-main'];assert.equal(pcb.userData.label,'UNKNOWN');assert.equal(pcb.children[0].geometry.type,'CylinderGeometry');assert.equal(model.parts['legacy-pcb'].children[0].geometry.type,'BoxGeometry');assert.ok(!model.group.userData.shellMeshes.includes(pcb.children[0]));assert.deepEqual(pcb.position.toArray(),[0,0,-4.16]);}finally{model.dispose();}
  }finally{globalThis.fetch=nativeFetch;}
  for(const args of [[[0,2,1]],[[20,10,1],'disc'],[[20,20,1],'cone']])assert.throws(()=>createPcbPlaceholderGeometry(...args),/Invalid PCB placeholder/);
});
