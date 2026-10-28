import assert from 'node:assert/strict';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
const nativeFetch=globalThis.fetch;
const vertices=new Float32Array([-1,-1,0,0,0,1,1,-1,0,0,0,1,0,1,0,0,0,1]);
const fixture={unit:'mm',strideFloats:6,binary:'geometry.bin',artifacts:{'geometry.bin':'a'.repeat(64)},parts:[{name:'main-board',role:'pcb',byteOffset:0,vertexCount:3,color:'#225522'}]};
const product={id:'test-bare-board',model:{url:'./assets/test-bare-board/model.json'}};
try {
 let metadata=fixture,requests=[];
 globalThis.fetch=async(url,options)=>{requests.push({url:new URL(url),options});return new Response(new URL(url).pathname.endsWith('model.json')?JSON.stringify(metadata):vertices.buffer);};
 const bare=createManufacturerModel(product,null);await bare.ready;
 assert.deepEqual(bare.group.userData.shellMeshes,[],'Explicit PCB roles must not turn into a recolorable case');assert.equal(bare.parts.shell,undefined);
 assert.equal(requests[0].options.cache,'no-store');assert.equal(requests[1].url.searchParams.get('sha256'),'a'.repeat(64),'Changed geometry gets an independent cache identity');bare.dispose();
 const port={id:'grove',kind:'connector-visual',connectorType:'HY2.0-4P',geometry:{centerMm:[0,2,0]},sourceIds:['fixture'],pinOrder:['signal2','signal1','vcc','gnd']};
 metadata={...fixture,derivedParts:[port]};const unknown=createManufacturerModel(product,null);await unknown.ready;assert.ok(unknown.parts.grove);assert.equal(unknown.connectors.grove,undefined,'Visible socket is not automatically connectable');unknown.dispose();
 metadata={...fixture,derivedParts:[{...port,allowConnections:true}]};const verified=createManufacturerModel(product,null);await verified.ready;assert.deepEqual(verified.connectors.grove.pinOrder,port.pinOrder);verified.dispose();
 metadata={...fixture,derivedParts:[{...port,housingColor:'#b5212d'}]};const red=createManufacturerModel(product,null);await red.ready;assert.equal(red.parts.grove.getObjectByName('socket-side').material.color.getHexString(),'b5212d');assert.equal(red.parts.grove.getObjectByName('socket-pin-0').material.color.getHexString(),'b8a572');assert.deepEqual(red.group.userData.shellMeshes,[]);red.dispose();
 metadata={...fixture,derivedParts:[{kind:'label',id:'bad-label',texture:'anything.png',geometry:{sizeMm:[-1,1]}}]};const bad=createManufacturerModel(product,null);await assert.rejects(bad.ready,/Invalid label dimensions/);bad.dispose();
 const profile=[[0,0],[4.65,0],[4.65,103.35],[0,108]];
 metadata={...fixture,derivedParts:[{id:'antenna',kind:'lathe',geometry:{profileMm:profile,centerMm:[34,36,-17]}}]};const antenna=createManufacturerModel(product,null);await antenna.ready;
 const shape=antenna.parts.antenna.geometry;shape.computeBoundingBox();assert.equal(shape.boundingBox.min.y,0);assert.equal(shape.boundingBox.max.y,108);assert.ok(Math.abs(shape.boundingBox.max.x-shape.boundingBox.min.x-9.3)<.0001);assert.deepEqual(antenna.parts.antenna.position.toArray(),[34,36,-17]);assert.deepEqual(antenna.group.userData.shellMeshes,[],'Antenna rubber is not recoloured with the shell');antenna.dispose();
 for(const invalid of [[[1,0],[2,-1]],[[0,0],[0,2]],[[1,0],[1,6000]],Array.from({length:129},(_,i)=>[1,i])]) {
  metadata={...fixture,derivedParts:[{id:'invalid',kind:'lathe',geometry:{profileMm:invalid}}]};const malformed=createManufacturerModel(product,null);await assert.rejects(malformed.ready,/Invalid lathe profile/);malformed.dispose();
 }
 const lens={id:'rgb-window',kind:'box',color:'#69746a',ledIndex:0,geometry:{sizeMm:[2.8,.4,.1],centerMm:[0,0,4.64]}};
 metadata={...fixture,derivedParts:[lens]};const rgb=createManufacturerModel(product,null);await rgb.ready;
 assert.equal(rgb.group.userData.rgbLedMeshes[0],rgb.parts['rgb-window']);assert.equal(rgb.parts['rgb-window'].userData.unlitColor,'#69746a');assert.deepEqual(rgb.group.userData.shellMeshes,[]);rgb.dispose();
 for(const ledIndex of [-1,256,1.5,'0']){metadata={...fixture,derivedParts:[{...lens,ledIndex}]};const invalid=createManufacturerModel(product,null);await assert.rejects(invalid.ready,/Invalid derived LED window/);invalid.dispose();}
 const button={id:'cover',shape:'rectangle',sizeMm:[19.2,19.2],positionMm:[0,0,6.98]},buttonProduct={...product,controls:[{id:'cover',type:'button'}]};
 for(const dragToOrbit of [undefined,true,false]) {
  metadata={...fixture,presentation:{buttons:[{...button,dragToOrbit}]}};const cover=createManufacturerModel(buttonProduct,null);await cover.ready;
  assert.equal(cover.parts['cover-key:cover'].userData.dragToOrbit,dragToOrbit===true);assert.equal(cover.parts['cover-key:cover'].userData.control,'cover');assert.equal(cover.parts['cover-key:cover'].userData.screen,undefined);cover.dispose();
 }
 metadata={...fixture,presentation:{buttons:[{...button,dragToOrbit:'yes'}]}};const invalidCover=createManufacturerModel(buttonProduct,null);await assert.rejects(invalidCover.ready,/Invalid cover button drag behavior/);invalidCover.dispose();
 for(const value of [null,[],[0],[0,100],[NaN,0],'bad']) {
  metadata={...fixture,derivedParts:[{id:'pcb-main',kind:'pcb-placeholder',geometry:{sizeMm:[20,16,1],labelOffsetMm:value}}]};const invalid=createManufacturerModel(product,null);await assert.rejects(invalid.ready,/Invalid PCB placeholder label offset/);invalid.dispose();
 }
 metadata={...fixture,derivedParts:[{...port,housingVariant:'open-roof'},{id:'pcb-main',kind:'pcb-placeholder',geometry:{sizeMm:[20,16,1],labelOffsetMm:[0,-5]}}]};const light=createManufacturerModel(product,null);await light.ready;assert.equal(light.parts.grove.getObjectByName('socket-top'),undefined);assert.ok(light.parts.grove.getObjectByName('socket-top-rear'));light.dispose();
 console.log('ok manufacturer loader: bare-PCB material isolation, source-verified connector boundary, asset cache identity and invalid label rejection');
} finally {globalThis.fetch=nativeFetch;}
