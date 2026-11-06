// Run through the task-owned Playwright CLI on a local platform page.
// Geometry checks use the production loader; pixels here are a test pattern.
// The separate workbench fixture runs the fixed official firmware in WASM.
async page => {
 const base=new URL('./',page.url()),errors=[];
 page.on('pageerror',error=>errors.push(error.message));
 const cdp=await page.context().newCDPSession(page);await cdp.send('Network.setCacheDisabled',{cacheDisabled:true});
 await page.goto(new URL('review-lane-01.html?id=m5stack-k141',base).href);
 await page.waitForFunction(()=>document.body.dataset.ready==='true');
 await page.setViewportSize({width:1100,height:1000});
 await page.addStyleTag({content:'#render,#render canvas{width:800px!important;height:800px!important}#render{position:fixed;left:0;top:0;z-index:999}'});
 const receipt=await page.evaluate(async()=>{
  const THREE=await import('./vendor/three.module.min.js'),{pickVisibleSurface}=await import('./scene-interaction.js');
  const {current}=laneReview,{model,meta,group}=current;group.updateMatrixWorld(true);
  const hash=async bytes=>[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(x=>x.toString(16).padStart(2,'0')).join('');
  const bytes=await(await fetch(new URL('model.json',current.base),{cache:'no-store'})).arrayBuffer(),artifacts={};
  for(const [name,expected]of Object.entries(meta.artifacts)){
   const actual=await hash(await(await fetch(new URL(name,current.base),{cache:'no-store'})).arrayBuffer());if(actual!==expected)throw Error('Artifact hash mismatch: '+name);artifacts[name]=actual;
  }
  const bounds=part=>{const b=new THREE.Box3().setFromObject(part);return {min:b.min.toArray(),max:b.max.toArray(),size:b.getSize(new THREE.Vector3()).toArray()};};
  const physical=new THREE.Box3();group.traverse(o=>{if(o.isMesh&&!o.name.startsWith('cover-key:')&&!o.name.includes('-unknown-'))physical.union(new THREE.Box3().setFromObject(o));});
  const envelope=physical.getSize(new THREE.Vector3()).toArray();if(envelope.some((x,i)=>Math.abs(x-[72,80,33.4][i])>.1))throw Error('Complete assembly exceeds drawing tolerance');
  const cast=(origin,point)=>pickVisibleSurface(new THREE.Raycaster(new THREE.Vector3(...origin),new THREE.Vector3(...point).sub(new THREE.Vector3(...origin)).normalize()),group);
  const screws=[];
  for(const part of meta.derivedParts.filter(p=>/^terminal-(in|out)-\d+$/.test(p.id))){
   const [x,y]=part.geometry.centerMm,b=bounds(model.parts[part.id]),hit=cast([x,y,100],[x,y,0]);
   if(b.max[2]>=9.5||b.min[2]<=7.8||!hit||!/^terminal-(slot-)?(in|out)-/.test(hit.object.name))throw Error('Screw not recessed/visible: '+part.id);
   screws.push({id:part.id,boundsMm:b,frontHit:hit.object.name,frontHitZ:hit.point.z,recessFromNativeFaceMm:10.4-hit.point.z});
  }
  if(screws.length!==23)throw Error('Expected 11 input and 12 relay screw heads');
  const usb=bounds(model.parts['stamp-usb-c']);if(usb.size[1]/usb.size[2]<2.7||usb.size[0]>4)throw Error('USB long axis does not follow Y');
  const usbHit=cast([42,13,12.8],[30,13,12.8]);if(!usbHit||usbHit.point.x>34.8)throw Error('USB aperture is blocked at its lip');
  const wireEntries=[];
  for(const part of meta.derivedParts.filter(p=>/^terminal-(in|out)-\d+$/.test(p.id))){const top=part.id.startsWith('terminal-in-'),x=part.geometry.centerMm[0],hit=cast([x,top?48:-48,-3.1],[x,0,-3.1]);if(!hit)throw Error('Wire cavity has no visible rear');const depth=top?37.75-hit.point.y:hit.point.y+33.8;if(depth<1.5)throw Error('Blocked wire entry '+part.id);wireEntries.push({id:part.id,firstHit:hit.object.name,depthMm:depth});}
  const pinOrders=['port-a','port-c'].map(id=>{const part=model.parts[id],spec=meta.derivedParts.find(p=>p.id===id),normal=new THREE.Vector3(0,0,1).applyQuaternion(part.getWorldQuaternion(new THREE.Quaternion()));const localX=new THREE.Vector3(1,0,0).applyQuaternion(part.getWorldQuaternion(new THREE.Quaternion()));if(normal.z<.99||localX.x>-.99||spec.pinOrder.join(',')!=='signal2,signal1,vcc,gnd')throw Error('Grove keyed orientation/wire order changed');return{id,normal:normal.toArray(),localX:localX.toArray(),worldLeftToRight:[...spec.pinOrder].reverse()};});
  const screen=model.screen,pos=screen.position,size=meta.presentation.screen.sizeMm,edgeVisibility=[];
  for(const origin of [[-9.7,16,250],[42,36,100],[-62,36,100]]){
   let edge=0,inner=0,total=0;
   for(let i=0;i<=32;i++)for(const [u,v]of [[i/32,0],[i/32,1],[0,i/32],[1,i/32]]){
    for(const margin of [.5,4]){const q=new THREE.Vector3(pos.x+((margin+u*(240-2*margin))/240-.5)*size[0],pos.y+((margin+v*(135-2*margin))/135-.5)*size[1],pos.z),start=new THREE.Vector3(...origin),hit=pickVisibleSurface(new THREE.Raycaster(start,q.sub(start).normalize()),group);if(hit?.object===screen){if(margin===.5)edge++;else inner++;}}
    total++;
   }
   edgeVisibility.push({origin,samples:total,edgeVisible:edge,fourPixelInsetVisible:inner});
   if(inner!==total||(origin[2]===250&&edge!==total))throw Error('Display image clipped in front view or inside the four-pixel oblique margin');
  }
  if(14-pos.z<.4||Math.abs(size[0]/size[1]-240/135)>1e-8)throw Error('LCD seating/aspect mismatch');
  const materialState=()=>{const result={};group.traverse(o=>{if(o.material&&!group.userData.shellMeshes.includes(o))result[o.uuid]=[].concat(o.material).map(m=>({color:m.color?.getHexString(),opacity:m.opacity,map:m.map?.uuid}));});return JSON.stringify(result);};
  window.stamplcMaterialState=materialState;window.stamplcOriginalMaterials=materialState();
  return {productId:current.id,modelSha256:await hash(bytes),artifacts,envelopeMm:envelope,sourceTriangleCount:meta.parts.reduce((n,p)=>n+p.vertexCount/3,0),screws,usb:{boundsMm:usb,openingFirstHit:usbHit.object.name,openingDepthMm:35.7-usbHit.point.x},wireEntries,pinOrders,screen:{positionMm:pos.toArray(),sizeMm:size,edgeVisibility},scope:'Exterior geometry, source hashes and production loader only. No hardware or electrical/firmware compatibility claim.'};
 });
 const images=[];
 for(const view of ['front','back','right','left','top','bottom']){
  const camera=await page.evaluate(view=>laneReview.show(view),view);
  await page.locator('#render canvas').screenshot({path:`output/playwright/stamplc-refresh/final-${view}.png`});
  await page.evaluate(()=>laneReview.mask());await page.locator('#render canvas').screenshot({path:`output/playwright/stamplc-refresh/final-${view}-mask.png`});images.push({view,camera});
 }
 for(const [view,options]of [['front-left',{position:[-160,-140,210],span:110}],['front-right',{position:[160,120,210],span:110}],['back-oblique',{position:[160,100,-210],span:110}],['usb-close',{position:[250,13,12],target:[30,13,12],up:[0,0,1],span:23}],['screen-close',{position:[35,30,110],target:[-9.7,16,10],span:42}]]){
  const camera=await page.evaluate(({view,options})=>laneReview.show(view,options),{view,options});await page.locator('#render canvas').screenshot({path:`output/playwright/stamplc-refresh/final-${view}.png`});images.push({view,camera});
 }
 for(const [name,shell]of [['gold',{color:'#c7a148',opacity:1,finish:'metal'}],['purple',{color:'#9c72cb',opacity:.32,finish:'plastic'}]]){
  await page.evaluate(shell=>{laneReview.appearance({shell});laneReview.show('materials',{position:[150,-140,210],span:110});if(stamplcMaterialState()!==stamplcOriginalMaterials)throw Error('Shell recoloring modified paper/connector/display materials');},shell);
  await page.locator('#render canvas').screenshot({path:`output/playwright/stamplc-refresh/final-${name}.png`});
 }
 await page.evaluate(()=>laneReview.show('unknown',{position:[100,40,-230],span:95}));await page.locator('#render canvas').screenshot({path:'output/playwright/stamplc-refresh/final-unknown.png'});
 if(errors.length)throw Error(errors.join('\n'));await cdp.detach();
 return {...receipt,images,nonShellMaterialIsolation:true,pageErrors:errors};
}
