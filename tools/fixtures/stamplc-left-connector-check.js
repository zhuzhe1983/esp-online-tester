// Run with the task-owned Playwright CLI on a local platform page. No firmware
// output is synthesized: these assertions concern the rendered physical assembly.
async page => {
 const base=new URL('./',page.url()),errors=[];
 page.on('pageerror',error=>errors.push(error.message));
 const cdp=await page.context().newCDPSession(page);await cdp.send('Network.setCacheDisabled',{cacheDisabled:true});
 await page.goto(new URL('review-lane-01.html?id=m5stack-k141',base).href);
 await page.waitForFunction(()=>document.body.dataset.ready==='true');
 await page.setViewportSize({width:1100,height:1000});
 await page.addStyleTag({content:'#render,#render canvas{width:800px!important;height:800px!important}#render{position:fixed;left:0;top:0;z-index:999}'});
 const receipt=await page.evaluate(async()=>{
  const T=await import('./vendor/three.module.min.js'),{pickVisibleSurface}=await import('./scene-interaction.js'),{current}=laneReview,{model,meta,group}=current;
  group.updateMatrixWorld(true);
  const hash=async bytes=>[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(x=>x.toString(16).padStart(2,'0')).join('');
  const bytes=await(await fetch(new URL('model.json',current.base),{cache:'no-store'})).arrayBuffer();
  const geometryHash=await hash(await(await fetch(new URL('geometry.bin',current.base),{cache:'no-store'})).arrayBuffer());
  if(geometryHash!=='8eada24face950933c7ee7d2d649a1bcc2caf48c74402dbde4e583ad48565e31')throw Error('Native CAD changed; re-audit cut-outs');
  const cast=(y,z)=>pickVisibleSurface(new T.Raycaster(new T.Vector3(-100,y,z),new T.Vector3(1,0,0)),group);
  const inserts=[];
  for(const id of ['pwr-485','pwr-can','bus-switch-0','bus-switch-1']){
   const part=model.parts[id],box=new T.Box3().setFromObject(part),center=box.getCenter(new T.Vector3()),normal=new T.Vector3(0,0,1).applyQuaternion(part.getWorldQuaternion(new T.Quaternion())),localX=new T.Vector3(1,0,0).applyQuaternion(part.getWorldQuaternion(new T.Quaternion()));
   if(normal.x>-.999||localX.y>-.999)throw Error('Insert normal/keyed orientation changed: '+id);
   // Inspect four real rim surfaces, including the end previously hidden by
   // the native shell. A metadata-only check would miss rendered obstruction.
   const rim=[[box.min.y+.15,center.z],[box.max.y-.15,center.z],[center.y,box.min.z+.15],[center.y,box.max.z-.15]].map(([y,z])=>{
    const hit=cast(y,z);if(hit?.object!==part)throw Error('Native shell obstructs '+id+' rim at '+[y,z]+'; first hit '+hit?.object.name);
    return {pointMm:hit.point.toArray(),firstHit:hit.object.name};
   });
   inserts.push({id,centerMm:center.toArray(),mouthPlaneXmm:box.min.x,outwardNormal:normal.toArray(),localX:localX.toArray(),boundsMm:{min:box.min.toArray(),max:box.max.toArray()},rim});
  }
  const contacts=[];
  for(const id of meta.derivedParts.map(p=>p.id).filter(id=>/^pwr-485-pin-|^pwr-can-contact-|^pwr-can-signal-/.test(id))){
   const part=model.parts[id],b=new T.Box3().setFromObject(part),c=b.getCenter(new T.Vector3());
   // A hollow power contact's center sees its rear; the annular edge sees metal.
   const y=c.y+(id.startsWith('pwr-can-contact-')?.8:0),hit=cast(y,c.z);
   if(hit?.object!==part)throw Error('Contact displaced/occluded: '+id+'; first hit '+hit?.object.name);
   contacts.push({id,visiblePointMm:hit.point.toArray()});
  }
  const keys=['pwr-485-key-0','pwr-485-key-1'].map(id=>{
   const b=new T.Box3().setFromObject(model.parts[id]),body=new T.Box3().setFromObject(model.parts['pwr-485']);
   if(Math.abs(b.min.z-body.max.z)>1e-5)throw Error('Detached PWR485 key tab');
   return {id,bottomZmm:b.min.z,bodyTopZmm:body.max.z};
  });
  const grove=['port-a','port-c'].map((id,i)=>{
   const part=model.parts[id],spec=meta.derivedParts.find(p=>p.id===id),mouth=part.getObjectByName('connector:'+id+':mouth'),position=mouth.getWorldPosition(new T.Vector3()).toArray(),normal=new T.Vector3(0,0,1).applyQuaternion(mouth.getWorldQuaternion(new T.Quaternion()));
   if(position.some((v,j)=>Math.abs(v-[[13,-.35,13.8],[25,-.35,13.8]][i][j])>1e-5)||normal.z<.999||spec.pinOrder.join(',')!=='signal2,signal1,vcc,gnd')throw Error('Existing Grove cable anchor changed');
   return {id,mouthMm:position,outwardNormal:normal.toArray(),localPinOrder:spec.pinOrder};
  });
  const physical=new T.Box3();group.traverse(o=>{if(o.isMesh&&!o.name.startsWith('cover-key:')&&!o.name.includes('-unknown-'))physical.union(new T.Box3().setFromObject(o));});
  const envelope=physical.getSize(new T.Vector3()).toArray();if(envelope.some((v,i)=>Math.abs(v-[72,80,33.4][i])>.1))throw Error('Complete assembly exceeds drawing tolerance');
  return {productId:current.id,modelSha256:await hash(bytes),geometrySha256:geometryHash,inserts,contacts,keys,groveAnchors:grove,envelopeMm:envelope,scope:'Physical insert seating, rim/contact visibility and existing Grove anchors only; PWR485/PWR-CAN remain visual-only.'};
 });
 const images=[];
 for(const [view,position]of [['left',[-250,13,0]],['left-front',[-160,-80,150]],['left-back',[-160,80,-150]]]){
  const camera=await page.evaluate(({view,position})=>laneReview.show(view,{position,target:[-34,13,0],up:[0,0,1],span:45}),{view,position});
  await page.locator('#render canvas').screenshot({path:'output/playwright/stamplc-left/final-'+view+'.png'});images.push({view,camera});
 }
 if(errors.length)throw Error(errors.join('\n'));await cdp.detach();return {...receipt,images,pageErrors:errors};
}
