import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const root=process.env.M5_VAMETER_SOURCE_ROOT?pathToFileURL(resolve(process.env.M5_VAMETER_SOURCE_ROOT)+'/web/platform/'):new URL('../web/platform/',import.meta.url);
const THREE=await import(new URL('vendor/three.module.min.js',root));
const {createManufacturerModel}=await import(new URL('manufacturer-model.js',root));
const {applyShellAppearance}=await import(new URL('shell-appearance.js',root));
const {pickVisibleSurface,createScenePointerRouter}=await import(new URL('scene-interaction.js',root));
const products=JSON.parse(await readFile(new URL('catalog/products.json',root))).items;
test('VAMeter radial press, axial/rim dragging and internal BOOT retain real surface occlusion',async()=>{
 const previous={fetch:globalThis.fetch,load:THREE.TextureLoader.prototype.loadAsync,document:globalThis.document};let model;
 try{
  globalThis.fetch=async value=>{const url=new URL(value);url.search='';return new Response(await readFile(url));};
  THREE.TextureLoader.prototype.loadAsync=async()=>new THREE.Texture();
  globalThis.document={createElement:()=>({width:1,height:1,getContext:()=>({fillText(){},fillRect(){},clearRect(){}})})};
  const product=products.find(p=>p.id==='m5stack-k136');model=createManufacturerModel(product,null);await model.ready;model.group.updateMatrixWorld(true);
  const [plan,metadata]=await Promise.all(['model-plan.json','model.json'].map(name=>readFile(new URL(`assets/m5stack-k136/${name}`,root)).then(JSON.parse)));
  assert.deepEqual(plan.derivedParts,metadata.derivedParts);assert.deepEqual(plan.presentation.buttons,metadata.presentation.buttons,'offline plan retains both source-bound press regions');
  const wheel=model.parts['encoder-thumbwheel'],press=model.parts['cover-key:encoder-press'],boot=model.parts['cover-key:boot'];
  assert(wheel.isMesh&&press.isMesh&&boot.isMesh);assert.equal(wheel.userData.encoder,'encoder');
  assert.equal(wheel.userData.control,undefined,'hidden axial ends must not impersonate a press button');assert.equal(wheel.userData.encoderPress,undefined);
  assert.equal(press.userData.control,'encoder-press');assert.equal(boot.userData.control,'boot');assert.equal(press.material.opacity,0);assert.equal(boot.material.opacity,0);
  assert(model.buttons.includes(press)&&model.buttons.includes(boot));assert(!model.buttons.includes(wheel));
  const bootControl=product.controls.find(c=>c.id==='boot');assert.equal(bootControl.pin,0);assert.equal(bootControl.physical,true);assert.equal(bootControl.spatialInputMode,'inspection-only');
  assert.equal(model.group.userData.inspectionError,undefined);const inspection=model.group.userData.inspection;assert(inspection);
  const pick=(eye,direction)=>pickVisibleSurface(new THREE.Raycaster(eye,direction.clone().normalize(),0,400),model.group);
  const aimed=(point,outward)=>pick(point.clone().addScaledVector(outward,160),outward.clone().negate());
  assert.equal(product.runtime.encoder,true);
  const events=[],router=createScenePointerRouter({onInput:e=>events.push(e)}),encoder=product.controls.find(c=>c.id==='encoder');
  const surface=hit=>hit?.object.userData.encoder?{kind:'encoder',id:hit.object.userData.encoder,detents:encoder.detents}:hit?.object.userData.control?{kind:'button',id:hit.object.userData.control}:{kind:'orbit'};
  const tap=(hit,id)=>{events.length=0;assert.equal(router.down({pointerId:1,x:0,y:0,surface:surface(hit)}),'button');router.up(1);assert.deepEqual(events,[{type:'button',id,pressed:true},{type:'button',id,pressed:false}]);};
  const wheelColor=wheel.material.color.getHexString(),center=wheel.getWorldPosition(new THREE.Vector3());
  const pressPoint=press.getWorldPosition(new THREE.Vector3()),pressNormal=new THREE.Vector3(0,0,1).transformDirection(press.matrixWorld);
  const bootPoint=boot.getWorldPosition(new THREE.Vector3()),insideView=new THREE.Vector3(1,0,-.15).normalize();
  for(const appearance of [null,{color:'#d4af37',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}]){
   if(appearance)for(const shell of model.group.userData.shellMeshes)applyShellAppearance(shell,{shell:appearance});
   assert.equal(wheel.material.color.getHexString(),wheelColor);assert.equal(wheel.material.opacity,1);assert.equal(press.material.opacity,0);assert.equal(boot.material.opacity,0);
   inspection.setProgress(0);model.group.updateMatrixWorld(true);
   const pressHit=aimed(pressPoint,pressNormal);assert.equal(pressHit?.object,press);tap(pressHit,'encoder-press');
   press.visible=false;const physical=aimed(pressPoint,pressNormal);press.visible=true;
   assert.equal(physical?.object,wheel,'radial press region sits on the actual exposed wheel');assert(physical.point.distanceTo(pressPoint)<.03);
   const axial=pick(new THREE.Vector3(100,-24,18.1),new THREE.Vector3(-1,0,0));assert.equal(axial?.object,wheel);assert(axial.face.normal.z>.7);assert.equal(surface(axial).kind,'encoder','an axial wheel face is not a synthetic press cap');
   for(const offset of [-2,2]){
    const rim=pick(new THREE.Vector3(center.x+offset,center.y-100,center.z),new THREE.Vector3(0,1,0));assert.equal(rim?.object,wheel);assert.equal(surface(rim).kind,'encoder');
    events.length=0;router.down({pointerId:2,x:0,y:0,surface:surface(rim),point:{angle:0}});router.move({pointerId:2,x:10,y:0,point:{angle:-1.5*2*Math.PI/encoder.detents}});router.up(2);
    assert.deepEqual(events,[{type:'encoder',id:'encoder',steps:1}],'rim gesture sends quadrature steps without button events');
   }
   for(const direction of [[1,0,0],[-1,0,0],[0,1,0],[0,-1,0],[0,0,1],[0,0,-1]])assert.notEqual(aimed(bootPoint,new THREE.Vector3(...direction))?.object.userData.control,'boot','assembled shell occludes internal G0');
   const closed=aimed(bootPoint,insideView);assert.equal(closed?.object.name,'base-shell');events.length=0;router.down({pointerId:3,x:0,y:0,surface:surface(closed)});router.up(3);assert.deepEqual(events,[]);
   inspection.setProgress(1);model.group.updateMatrixWorld(true);assert(boot.getWorldPosition(new THREE.Vector3()).distanceTo(bootPoint)<1e-9);
   const opened=aimed(bootPoint,insideView);assert.equal(opened?.object,boot);tap(opened,'boot');
   for(const side of [-1,1]){const outside=boot.localToWorld(new THREE.Vector3(side*1.7,0,0));assert.notEqual(aimed(outside,insideView)?.object.userData.control,'boot','only the finite native tongue region presses G0');}
   inspection.setProgress(0);model.group.updateMatrixWorld(true);assert.equal(aimed(bootPoint,insideView)?.object.name,'base-shell','the identical successful open-case ray is blocked again when reassembled');
  }
  const card=products.find(p=>p.id==='m5stack-k146');assert.equal(card.runtime.board,'xiaozhi-card');assert.equal(card.runtime.touch,true);assert(card.controls.some(c=>c.id==='wake'&&c.pin===21));
 }finally{model?.dispose();globalThis.fetch=previous.fetch;THREE.TextureLoader.prototype.loadAsync=previous.load;globalThis.document=previous.document;}
});
test('derived input metadata cannot create nonexistent buttons or encoders',async()=>{
 const oldFetch=globalThis.fetch;
 const geometry=new Float32Array([-1,-1,0,0,0,1,1,-1,0,0,0,1,0,1,0,0,0,1]);
 const base={id:'derived-input-fixture',controls:[{id:'encoder',type:'encoder'},{id:'press',type:'button'}],model:{url:'./assets/derived-input-fixture/model.json'}};
 try{for(const change of [{encoderId:'absent'},{controlId:'encoder'},{encoderPress:{centerMm:[0,0,0],radiusMm:0}},{encoderId:undefined,encoderPress:{centerMm:[0,0,0],radiusMm:1}},{encoderPress:{centerMm:[0,0],radiusMm:1}}]){
  const part={id:'wheel',kind:'cylinder',geometry:{radiusMm:2,depthMm:4},encoderId:'encoder',controlId:'press',encoderPress:{centerMm:[0,0,0],radiusMm:1},...change};
  const metadata={unit:'mm',strideFloats:6,binary:'geometry.bin',parts:[{name:'base',byteOffset:0,vertexCount:3}],derivedParts:[part]};globalThis.fetch=async value=>new Response(String(value).endsWith('model.json')?JSON.stringify(metadata):geometry.buffer);
  const model=createManufacturerModel(base,null);await assert.rejects(model.ready,/Invalid derived/);model.dispose();
 }}finally{globalThis.fetch=oldFetch;}
});
