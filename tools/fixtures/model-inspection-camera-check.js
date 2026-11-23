// Run through playwright_cli.sh run-code on the caller-supplied platform URL.
// Routes a two-box model and exposes renderer references only in this test response.
async page => {
  const base=new URL('./',page.url()),context=page.context(),result={fixture:'Two 20×20×2 mm boxes; declared +30 mm and +5000 mm Z viewing offsets; no product or firmware evidence',checks:[],extents:{}};
  let offsetMm=30;
  const scenePattern='**/platform/scene-view.js',modelPattern='**/assets/m5stack-s007/model.json';
  const check=(ok,message)=>{if(!ok)throw Error(message);result.checks.push(message);};
  await context.route(scenePattern,async route=>{
    const response=await route.fetch();let source=await response.text();
    const anchor='scene.add(orbitGroup);orbitGroup.add(flightGroup);flightGroup.add(group);';
    const viewAnchor='return {stickers,setProduct, setAccessories';
    if(!source.includes(anchor)||!source.includes(viewAnchor))throw Error('The scene-view camera fixture anchor changed; update the test before interpreting its result');
    source=source.replace(anchor,anchor+' window.__inspectionCameraTest={group:orbitGroup,camera};').replace(viewAnchor,'return window.__inspectionCameraTest.view = {stickers,setProduct, setAccessories');
    await route.fulfill({response,body:source});
  });
  await context.route(modelPattern,async route=>{
    const response=await route.fetch(),data=await response.json();
    data.parts=[];data.derivedParts=['base','cover'].map((id,i)=>({id,kind:'box',color:i?'#748a76':'#444444',geometry:{sizeMm:[20,20,2],centerMm:[0,0,0]}}));
    data.presentation={inspectionGroups:[{id:'cover',parts:['cover'],offsetMm:[0,0,offsetMm],sourceIds:[data.provenance.sources[0].id],notes:'Camera regression fixture only; Clearance is a viewing offset.'}]};
    await route.fulfill({response,json:data});
  });
  const extent=()=>page.evaluate(async()=>{
    const THREE=await import('./vendor/three.module.min.js'),{group,camera}=window.__inspectionCameraTest;group.updateMatrixWorld(true);camera.updateMatrixWorld();
    let x=0,y=0,zMin=Infinity,zMax=-Infinity;group.traverse(part=>{const a=part.geometry?.attributes.position;if(!part.isMesh||!a)return;for(let i=0;i<a.count;i++){const p=new THREE.Vector3().fromBufferAttribute(a,i).applyMatrix4(part.matrixWorld).project(camera);x=Math.max(x,Math.abs(p.x));y=Math.max(y,Math.abs(p.y));zMin=Math.min(zMin,p.z);zMax=Math.max(zMax,p.z);}});return {x,y,zMin,zMax,far:camera.far};
  });
  const fits=value=>value.x<1&&value.y<1&&value.zMin>-1&&value.zMax<1;
  try {
    await page.setViewportSize({width:1280,height:900});await page.goto(new URL('?lang=zh-CN',base).href);await page.waitForFunction(()=>!!window.espTester);
    await page.evaluate(async()=>{const m=await import('./catalog.js');await espTester.configure(m.createDefaultScene(await m.loadCatalog(),'m5stack-s007'));});
    await page.waitForFunction(()=>document.querySelector('#scene-mount').dataset.modelStatus==='ready');
    await page.addStyleTag({content:'#scene-mount{width:500px!important;height:450px!important;min-height:0!important;flex:none!important}'});await page.waitForTimeout(100);
    const slider=page.locator('#inspect-model'),track=await slider.boundingBox();
    await page.mouse.move(track.x+7,track.y+track.height/2);
    await page.mouse.down();
    await page.mouse.move(track.x+track.width*.53,track.y+track.height/2,{steps:6});
    await page.mouse.up();
    await page.waitForFunction(()=>{const input=document.getElementById('inspect-model');return input.valueAsNumber>0&&input.valueAsNumber<100&&input.getAttribute('aria-valuetext')===input.value+'%';});
    const partial=await page.evaluate(()=>({distance:window.__inspectionCameraTest.group.getObjectByName('cover').position.z,
      percent:document.getElementById('inspect-model').valueAsNumber}));
    check(Math.abs(partial.distance-30*partial.percent/100)<.001,'pointer drag translates the cover proportionally');
    await slider.press('End');await page.waitForTimeout(50);result.extents.initial=await extent();check(fits(result.extents.initial),'expanded fixture initially fits 500×450 viewport');
    await page.evaluate(async()=>{const THREE=await import('./vendor/three.module.min.js');window.__inspectionCameraTest.view.setOrientation(new THREE.Quaternion().setFromEuler(new THREE.Euler(-.13,-.25+Math.PI/2,.025)).toArray(),{notify:false});});
    await page.waitForTimeout(50);result.extents.orientation=await extent();check(fits(result.extents.orientation),'setOrientation reframes expanded parts after 90° Y turn');
    await page.locator('#reset-view').click();await slider.press('End');await page.waitForTimeout(50);
    const canvas=page.locator('#scene-mount canvas').first(),rect=await canvas.boundingBox(),before=await page.evaluate(()=>window.__inspectionCameraTest.view.getOrientation());
    await page.mouse.move(rect.x+10,rect.y+10);await page.mouse.down();await page.mouse.move(rect.x+10+Math.PI*50,rect.y+10,{steps:8});await page.mouse.up();await page.waitForTimeout(150);
    const after=await page.evaluate(()=>window.__inspectionCameraTest.view.getOrientation());check(JSON.stringify(before)!==JSON.stringify(after),'public canvas drag changes the model orientation');
    result.extents.orbit=await extent();check(fits(result.extents.orbit),'public 90° orbit keeps both separated boxes inside the camera');
    await canvas.screenshot({path:'output/playwright/inspection-camera-orbit.png'});await page.locator('#reset-view').click();check(await slider.inputValue()==='0','reset after orbit restores assembled state');
    offsetMm=5000;await page.reload();await page.waitForFunction(()=>!!window.espTester);
    await page.evaluate(async()=>{const m=await import('./catalog.js');await espTester.configure(m.createDefaultScene(await m.loadCatalog(),'m5stack-s007'));});await page.waitForFunction(()=>document.querySelector('#scene-mount').dataset.modelStatus==='ready');
    await page.addStyleTag({content:'#scene-mount{width:500px!important;height:450px!important;min-height:0!important;flex:none!important}'});await page.waitForTimeout(100);await slider.press('End');await page.waitForTimeout(50);
    result.extents.maximumOffset=await extent();check(fits(result.extents.maximumOffset)&&result.extents.maximumOffset.far>3000,'declared 5000 mm offset fits the camera depth range');
    await page.locator('#reset-view').click();await page.waitForTimeout(50);check((await extent()).far===3000,'assembled reset restores the ordinary camera depth range');
    return result;
  } finally {await context.unroute(scenePattern);await context.unroute(modelPattern);}
}
