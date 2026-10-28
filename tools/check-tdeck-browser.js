// Playwright CLI run-code --filename. Open a local workbench first. The checked
// upstream image must already be cached at the path below; runtime validates SHA.
async (page) => {
  const check=(ok,message)=>{if(!ok)throw Error(message);},errors=[];
  const onError=e=>errors.push(e.message);page.on('pageerror',onError);
  const cache='output/tdeck-source/firmware/tdeck-sx1262-factory-20261024-0x0.bin';
  let source,route;
  try {
    await page.setViewportSize({width:1440,height:1050});
    await page.waitForFunction(()=>window.espTester);
    source=await page.evaluate(async()=>{
      const {loadCatalog,createDefaultScene}=await import('./catalog.js'),catalog=await loadCatalog(),scene=createDefaultScene(catalog,'lilygo-t-deck');
      await espTester.configure(scene);return catalog.firmwares.find(f=>f.id==='lilygo-t-deck-factory').source;
    });
    await page.waitForFunction(()=>document.querySelector('#scene-mount').dataset.modelStatus==='ready');
    const geometry=await page.evaluate(async()=>{
      const THREE=await import('./vendor/three.module.min.js'),{createManufacturerModel}=await import('./manufacturer-model.js'),{pickVisibleSurface}=await import('./scene-interaction.js');
      const product=(await (await fetch('./catalog/products.json')).json()).items.find(p=>p.id==='lilygo-t-deck');
      const texture=new THREE.DataTexture(new Uint8Array([0,0,0,255]),1,1),model=createManufacturerModel(product,texture),metadata=await model.ready,regions=metadata.presentation.buttons;
      const results=[];
      for(const [view,position] of [['front',[0,0,250]],['oblique',[90,60,220]],['back',[0,0,-250]]]){
        const camera=new THREE.PerspectiveCamera(32,1,2,3000);camera.position.set(...position);camera.lookAt(0,0,0);camera.updateProjectionMatrix();camera.updateMatrixWorld();const ray=new THREE.Raycaster();
        for(const r of regions){const point=new THREE.Vector3(...r.positionMm);if(r.shape==='circle')point.x+=.01; // Sample triangle interior, avoiding the shared centre vertex.
          point.project(camera);ray.setFromCamera(new THREE.Vector2(point.x,point.y),camera);const hit=pickVisibleSurface(ray,model.group)?.object.userData.control||null;results.push({view,id:r.id,hit,pass:view==='back'?!hit:hit===r.id});}
      }
      // Match the workbench's unmodified reset-view camera to click an actual
      // photo-aligned key through its production pointer controller.
      model.group.rotation.set(-.13,-.25,.025);model.group.updateMatrixWorld(true);
      const box=new THREE.Box3().setFromObject(model.group),size=box.getSize(new THREE.Vector3()),center=box.getCenter(new THREE.Vector3());
      const rect=document.querySelector('#scene-mount canvas[data-engine]').getBoundingClientRect(),camera=new THREE.PerspectiveCamera(32,rect.width/rect.height,2,3000),tangent=Math.tan(THREE.MathUtils.degToRad(16));
      const distance=Math.max(20,Math.max(size.y/(2*tangent),size.x/(2*tangent*camera.aspect))*1.32+size.z/2);camera.position.set(center.x,center.y,center.z+distance);camera.lookAt(center);camera.updateProjectionMatrix();camera.updateMatrixWorld();
      const r=regions.find(r=>r.id==='key-q'),p=new THREE.Vector3(...r.positionMm).applyMatrix4(model.group.matrixWorld).project(camera);
      const keyPoint={x:rect.x+(p.x+1)*rect.width/2,y:rect.y+(1-p.y)*rect.height/2};
      model.dispose();texture.dispose();model.group.traverse(m=>{m.geometry?.dispose();for(const mat of [].concat(m.material||[]))mat.dispose();});
      return {regions:regions.length,results,keyPoint};
    });
    check(geometry.regions===40 && geometry.results.every(r=>r.pass),`35 keyboard and 5 trackball regions must pick their visible fronts and stay occluded from the back: ${JSON.stringify(geometry.results.filter(r=>!r.pass))}`);
    await page.mouse.move(geometry.keyPoint.x,geometry.keyPoint.y);
    check(await page.locator('#scene-mount').getAttribute('data-hover')==='button','Actual 3D Q key is a button hit.');
    route=r=>r.fulfill({path:cache,contentType:'application/octet-stream',headers:{'Access-Control-Allow-Origin':'*'}});
    await page.route(source.url,route);
    await page.evaluate(async source=>{await espTester.loadFirmware(source);await espTester.start();},source);
    const waitGuest=async time=>{const deadline=Date.now()+180000;while(Date.now()<deadline){const s=await page.evaluate(()=>espTester.observe());if(s.status==='error')throw Error(JSON.stringify(s.lastError));if(s.stats.time>=time)return;await page.waitForTimeout(100);}throw Error(`Guest clock timeout at ${time}`);};
    await waitGuest(15);
    const input=async(event,hold=.15)=>{const r=await page.evaluate(e=>espTester.input(e),event);await waitGuest(r.guestTime+hold);};
    const tap=async(x,y)=>{await input({type:'touch',x,y,down:true});await input({type:'touch',x,y,down:false},1);};
    const swipe=async()=>{for(const x of [280,200,100,40])await input({type:'touch',x,y:120,down:true},.08);await input({type:'touch',x:40,y:120,down:false},1);};
    await tap(90,164);await swipe();await swipe();await tap(262,55);
    // A DOM button or direct matrix API is not sufficient here: this click
    // crosses the actual CAD region, pointer routing, control mapping and I2C.
    await page.mouse.click(geometry.keyPoint.x,geometry.keyPoint.y);
    const afterClick=await page.evaluate(()=>espTester.observe());await waitGuest(afterClick.stats.time+1);
    const keyboard=await page.evaluate(async()=>{
      const f=await espTester.capture({format:'rgba'}),bytes=[];for(let y=80;y<160;y++)bytes.push(...f.pixels.subarray((y*f.width+20)*4,(y*f.width+300)*4));
      const hash=await crypto.subtle.digest('SHA-256',new Uint8Array(bytes));return {width:f.width,height:f.height,frameNumber:f.frameNumber,sha256:[...new Uint8Array(hash)].map(b=>b.toString(16).padStart(2,'0')).join('')};
    });
    check(keyboard.sha256==='ce149a5a3fd8fcab2069b29341a173fc363e7bf0ec1c7ab2a8a0cceabd152fd1','3D Q key must produce the visually verified q glyph in official Keyboard.');
    await page.screenshot({path:'output/tdeck-browser-keyboard.png'});
    await page.evaluate(()=>espTester.pause());await page.unroute(source.url,route);route=null;
    const shared=await page.evaluate(()=>espTester.share({mode:'embed',autostart:false}));await page.goto(shared);
    await page.waitForFunction(()=>window.espTester);check((await page.evaluate(()=>espTester.observe())).scene.productId==='lilygo-t-deck','Embed restores exact product.');
    await page.waitForFunction(()=>document.querySelector('#scene-mount').dataset.modelStatus==='ready');
    const embedded=await page.evaluate(async()=>({productId:(await espTester.observe()).scene.productId,keyboard:(await espTester.capabilities()).input.keyboard}));
    check(embedded.keyboard.columns===10&&embedded.keyboard.model==='esp32c3-i2c-raw','Embed restores exact matrix model.');
    await page.screenshot({path:'output/tdeck-browser-embed.png'});
    check(!errors.length,`Page errors: ${errors.join('; ')}`);
    return {geometry,keyboard,embedded,pageErrors:errors,firmwareDelivery:'Cached byte-identical upstream image fulfilled at its exact catalog URL; runtime SHA/layout checks retained.'};
  } finally {if(source&&route)await page.unroute(source.url,route);page.off('pageerror',onError);await page.evaluate(()=>window.espTester?.dispose()).catch(()=>{});}
}
