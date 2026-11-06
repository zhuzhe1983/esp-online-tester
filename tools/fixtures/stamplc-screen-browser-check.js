// Run on any platform page. These colored pixels test geometry only; the separate
// workbench check loads the fixed official firmware for actual framebuffer evidence.
async page => {
 const base=new URL('./',page.url()),errors=[];page.on('pageerror',error=>errors.push(error.message));
 await page.goto(new URL('review-lane-01.html?id=m5stack-k141',base).href);
 await page.waitForFunction(()=>document.body.dataset.ready==='true');await page.setViewportSize({width:1000,height:1000});
 await page.addStyleTag({content:'#render,#render canvas{width:800px!important;height:800px!important}#render{position:fixed;left:0;top:0;z-index:999}'});
 const receipt=await page.evaluate(async()=>{
  const {pickVisibleSurface}=await import('./scene-interaction.js'),THREE=await import('./vendor/three.module.min.js'),{current}=laneReview,screen=current.model.screen,meta=current.meta;
  const hash=async bytes=>[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(b=>b.toString(16).padStart(2,'0')).join('');
  const modelBytes=await(await fetch(new URL('model.json',current.base),{cache:'no-store'})).arrayBuffer(),binary=await(await fetch(new URL(meta.binary,current.base),{cache:'no-store'})).arrayBuffer();
  if(await hash(binary)!==meta.artifacts[meta.binary])throw Error('Geometry hash mismatch');
  const part=meta.parts.find(p=>p.name==='shell-front'),data=new Float32Array(binary,part.byteOffset,part.vertexCount*6),lip=[],innerLip=[];
  // Extract the original CAD bevel triangles around the window, independently
  // of presentation.screen. Their Z=14 vertices define the opening's outer lip.
  for(let i=0;i<data.length;i+=18){const v=[0,6,12].map(j=>[...data.slice(i+j,i+j+3)]);if(v.every(([x,y,z])=>x>-23&&x<4&&y>7&&y<25&&z>13.4&&z<14.001)&&v.some(p=>p[2]<13.9)){innerLip.push(...v.map(p=>p[2]));for(const p of v)if(Math.abs(p[2]-14)<.001)lip.push(p);}}
  const low=[0,1,2].map(i=>Math.min(...lip.map(p=>p[i]))),high=[0,1,2].map(i=>Math.max(...lip.map(p=>p[i]))),center=low.map((v,i)=>(v+high[i])/2),position=screen.position.toArray(),size=meta.presentation.screen.sizeMm;
  if(!lip.length||Math.abs(position[0]-center[0])>.001||Math.abs(position[1]-center[1])>.001)throw Error('Display is not centered on the native CAD opening');
  if(position[2]>=low[2]||position[2]<Math.min(...innerLip))throw Error('Display must sit between the native outer and inner lip planes');
  if(screen.rotation.toArray().slice(0,3).some(v=>Math.abs(v)>.000001))throw Error('Display is tilted relative to the native front plane');
  current.group.updateMatrixWorld(true);
  const edgePoints=[];for(let i=0;i<=32;i++){const t=i/32;for(const [u,v] of [[t,0],[t,1],[0,t],[1,t]])edgePoints.push(new THREE.Vector3(position[0]+((.5+u*239)/240-.5)*size[0],position[1]+((.5+v*134)/135-.5)*size[1],position[2]));}
  const cameras=[[-9.7,16,250],[42,36,100],[-62,36,100]],visibility=[];
  for(const origin of cameras){let visible=0;for(const point of edgePoints){const start=new THREE.Vector3(...origin),ray=new THREE.Raycaster(start,point.clone().sub(start).normalize()),hit=pickVisibleSurface(ray,current.group);if(hit?.object===screen)visible++;}visibility.push({cameraPosition:origin,edgeSamples:edgePoints.length,visible});if(visible!==edgePoints.length)throw Error('Display edge hidden by model at '+origin);}
  window.screenReviewOriginal={position:screen.position.clone(),texture:screen.material.map};
  const canvas=document.createElement('canvas');canvas.width=240;canvas.height=135;const ctx=canvas.getContext('2d');for(const [color,x,y,w,h]of[['#e63e44',0,0,120,67],['#40cf7a',120,0,120,67],['#3c75f4',0,67,120,68],['#f0cd38',120,67,120,68]]){ctx.fillStyle=color;ctx.fillRect(x,y,w,h);}ctx.fillStyle='#ffffff';ctx.fillRect(0,0,240,1);ctx.fillRect(0,134,240,1);ctx.fillRect(0,0,1,135);ctx.fillRect(239,0,1,135);ctx.fillStyle='#111111';ctx.font='16px monospace';ctx.fillText('GEOMETRY',72,73);
  const texture=new THREE.CanvasTexture(canvas);texture.colorSpace=THREE.SRGBColorSpace;texture.magFilter=THREE.NearestFilter;texture.minFilter=THREE.NearestFilter;screen.material.map=texture;screen.material.needsUpdate=true;window.screenReviewTexture=texture;
  return {productId:current.id,modelSha256:await hash(modelBytes),geometrySha256:await hash(binary),cadLipBoundsMm:[low,high],cadLipCenterMm:center,oldPositionMm:[-10.45,16.3,14.035],newPositionMm:position,sizeMm:size,rotationRadians:screen.rotation.toArray().slice(0,3),recessMm:low[2]-position[2],edgeVisibility:visibility,scope:'Native CAD aperture alignment and synthetic pixel-border visibility only; not firmware execution or optical calibration.'};
 });
 const views=[['front',{span:44,position:[-9.7,16,250],target:[-9.7,16,0]}],['right',{span:48,position:[42,36,100],target:[-9.7,16,10]}],['left',{span:48,position:[-62,36,100],target:[-9.7,16,10]}]];
 for(const [view,options] of views){await page.evaluate(({view,options})=>laneReview.show(view,options),{view,options});await page.locator('#render canvas').screenshot({path:`output/playwright/stamplc-screen-${view}.png`});}
 await page.evaluate(()=>laneReview.current.model.screen.position.set(-10.45,16.3,14.035));for(const [view,options]of views){await page.evaluate(({view,options})=>laneReview.show(view,options),{view,options});await page.locator('#render canvas').screenshot({path:`output/playwright/stamplc-screen-before-${view}.png`});}
 await page.evaluate(()=>{const screen=laneReview.current.model.screen;screen.position.copy(screenReviewOriginal.position);screen.material.map=screenReviewOriginal.texture;screen.material.needsUpdate=true;screenReviewTexture.dispose();});
 for(const [name,shell]of[['original',null],['gold',{color:'#c7a148',opacity:1,finish:'metal'}],['purple',{color:'#9c72cb',opacity:.4,finish:'plastic'}]]){await page.evaluate(({shell,options})=>{if(shell)laneReview.appearance({shell});laneReview.show('right',options);},{shell,options:views[1][1]});await page.locator('#render canvas').screenshot({path:`output/playwright/stamplc-screen-${name}.png`});}
 if(errors.length)throw Error(errors.join('\n'));return {...receipt,pageErrors:errors};
}
