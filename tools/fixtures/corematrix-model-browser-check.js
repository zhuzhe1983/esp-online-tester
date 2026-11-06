// Headless visual/geometry probe; deliberately separate from real firmware tests.
async page => {
 const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto(new URL('corematrix-model-review.html',page.url()).href);await page.setViewportSize({width:1000,height:1000});
 await page.waitForFunction(()=>document.body.dataset.ready==='true'||document.body.dataset.error);
 const receipt=await page.evaluate(async()=>{
  if(document.body.dataset.error)throw Error(document.body.dataset.error);
  const THREE=await import('../../web/platform/vendor/three.module.min.js'),{pickVisibleSurface}=await import('../../web/platform/scene-interaction.js');
  const data=await(await fetch('../../web/platform/assets/m5stack-c158/model.json',{cache:'no-store'})).arrayBuffer(),meta=JSON.parse(new TextDecoder().decode(data));
  const hash=async v=>[...new Uint8Array(await crypto.subtle.digest('SHA-256',v))].map(x=>x.toString(16).padStart(2,'0')).join('');
  const binary=await(await fetch('../../web/platform/assets/m5stack-c158/geometry.bin',{cache:'no-store'})).arrayBuffer();if(await hash(binary)!==meta.artifacts['geometry.bin'])throw Error('Unexpected geometry bytes');
  const group=review.model.group;group.updateMatrixWorld(true);if(group.userData.inspectionError)throw Error(group.userData.inspectionError);
  const bounds=review.bounds();if(bounds.size.some((v,i)=>Math.abs(v-[54,54,11.6][i])>.05))throw Error('Complete assembly exceeds independent dimensions');
  const leds=group.userData.rgbLedMeshes;if(leds.length!==256||leds.some((m,i)=>m.userData.ledIndex!==i))throw Error('Missing matrix optical windows');
  for(let i=0;i<256;i++){if(leds[i].position.x!==meta.derivedParts.find(p=>p.ledIndex===i).geometry.centerMm[0])throw Error('Moved emitter');}
  const hits=[];
  for(const key of meta.presentation.buttons){const mesh=review.model.parts['cover-key:'+key.id],target=mesh.getWorldPosition(new THREE.Vector3()),normal=new THREE.Vector3(0,0,1).transformDirection(mesh.matrixWorld),ray=new THREE.Raycaster(target.clone().addScaledVector(normal,100),normal.clone().negate()),hit=pickVisibleSurface(ray,group);hits.push({id:key.id,hit:hit?.object.userData.control,object:hit?.object.name});if(hit?.object.userData.control!==key.id)throw Error('Occluded control '+key.id);}
  const faceRay=new THREE.Raycaster(new THREE.Vector3(-13,27,100),new THREE.Vector3(0,0,-1)),frontHit=pickVisibleSurface(faceRay,group);if(frontHit?.object.userData.control)throw Error('Side key clickable through front shell');
  const probe=Array.from({length:256},()=>[0,0,0]);probe[0]=[64,64,64];probe[15]=[128,128,128];probe[240]=[192,192,192];probe[255]=[255,255,255];review.leds(probe);const corners=[0,15,240,255].map(i=>({index:i,position:leds[i].position.toArray(),rgb:leds[i].userData.rgb}));
  const before=group.children.map(o=>[o.name,o.position.toArray(),o.visible]);review.inspection(true);if(!review.model.parts['pcb-main'].getObjectByName('pcb-main-unknown-back'))throw Error('Missing UNKNOWN');review.inspection(false);const after=group.children.map(o=>[o.name,o.position.toArray(),o.visible]);if(JSON.stringify(before)!==JSON.stringify(after))throw Error('Inspection restore drift');
  window.corematrixProbe=probe;return {productId:meta.id,modelSha256:await hash(data),geometrySha256:await hash(binary),boundsMm:bounds,opticalWindowCount:256,corners,controlHits:hits,frontCannotPressSideKey:true,inspectionRestore:true,scope:'Production renderer geometry, LED value presentation and control visibility only. Explicit fixture LED values are not firmware evidence; white optical diffusion is uncalibrated.'};
 });
 await page.evaluate(()=>{review.leds(corematrixProbe);review.show('front')});await page.locator('canvas').screenshot({path:'output/playwright/m5stack-c158-led-corners.png'});
 await page.evaluate(()=>review.leds(Array.from({length:256},()=>[255,255,255])));
 for(const v of ['front','back','left','right','top','bottom','oblique','back-oblique']){await page.evaluate(v=>review.show(v),v);await page.locator('canvas').screenshot({path:`output/playwright/m5stack-c158-${v}.png`});await page.evaluate(()=>review.mask());await page.locator('canvas').screenshot({path:`output/playwright/m5stack-c158-${v}-mask.png`});}
 const unchanged=await page.evaluate(()=>{const p=review.model.parts;return {label:p['rear-label-print'].material.map.uuid,button:p['button-a'].material.color.getHexString(),led:p['matrix-led-00-00'].material.color.getHexString(),socket:p['grove-a'].children[0].material.color.getHexString()}});
 for(const[name,shell]of[['gold',{color:'#c7a148',opacity:1,finish:'metal'}],['purple',{color:'#9c72cb',opacity:.4,finish:'plastic'}]]){await page.evaluate(shell=>review.appearance({shell}),shell);for(const v of['front','back','left','back-oblique']){await page.evaluate(v=>review.show(v),v);await page.locator('canvas').screenshot({path:`output/playwright/m5stack-c158-${name}-${v}.png`});}}
 const after=await page.evaluate(()=>{const p=review.model.parts;return {label:p['rear-label-print'].material.map.uuid,button:p['button-a'].material.color.getHexString(),led:p['matrix-led-00-00'].material.color.getHexString(),socket:p['grove-a'].children[0].material.color.getHexString()}});if(JSON.stringify(unchanged)!==JSON.stringify(after))throw Error('Shell appearance modified independent components');
 await page.evaluate(()=>review.inspection(true));await page.locator('canvas').screenshot({path:'output/playwright/m5stack-c158-inspection.png'});await page.evaluate(()=>review.inspection(false));
 if(errors.length)throw Error(errors.join('\n'));return {...receipt,appearanceIndependent:true,pageErrors:errors};
}
