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
 console.log('ok manufacturer loader: bare-PCB material isolation, source-verified connector boundary, asset cache identity and invalid label rejection');
} finally {globalThis.fetch=nativeFetch;}
