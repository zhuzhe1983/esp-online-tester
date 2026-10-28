// Playwright CLI run-code --filename, from this checkout. Cache the exact
// upstream image at output/tembed/factory.bin; its catalog SHA remains enforced.
async (page) => {
  const check=(ok,message)=>{if(!ok)throw Error(message);},errors=[];
  const onError=e=>errors.push(e.message);page.on('pageerror',onError);
  let source,route;
  try {
    await page.setViewportSize({width:1440,height:1050});
    await page.waitForFunction(()=>window.espTester);
    source=await page.evaluate(async()=>{
      const {loadCatalog,createDefaultScene}=await import('./catalog.js'),catalog=await loadCatalog();
      await espTester.configure(createDefaultScene(catalog,'lilygo-t-embed-cc1101'));
      return catalog.firmwares.find(f=>f.id==='lilygo-t-embed-cc1101-factory').source;
    });
    await page.waitForFunction(()=>document.querySelector('#scene-mount').dataset.modelStatus==='ready');
    const geometry=await page.evaluate(async()=>{
      const THREE=await import('./vendor/three.module.min.js'),{createManufacturerModel}=await import('./manufacturer-model.js'),{pickVisibleSurface}=await import('./scene-interaction.js');
      const product=(await (await fetch('./catalog/products.json')).json()).items.find(p=>p.id==='lilygo-t-embed-cc1101');
      const texture=new THREE.DataTexture(new Uint8Array([0,0,0,255]),1,1),model=createManufacturerModel(product,texture);await model.ready;
      const results=[];
      for(const [view,position] of [['front',[0,0,250]],['oblique',[90,60,220]],['back',[0,0,-250]]]) {
        const camera=new THREE.PerspectiveCamera(32,1,2,3000);camera.position.set(...position);camera.lookAt(0,0,0);camera.updateProjectionMatrix();camera.updateMatrixWorld();
        for(const [role,coords] of [['button',[29.5,0,17]],['encoder',[39.5,0,16.5]],...[-1,1].flatMap(x=>[-1,1].map(y=>['screen',[-10.075+x*21.25,y*11.24,13.45]]))]) {
          const point=new THREE.Vector3(...coords).project(camera),ray=new THREE.Raycaster();ray.setFromCamera(new THREE.Vector2(point.x,point.y),camera);
          const object=pickVisibleSurface(ray,model.group)?.object,data=object?.userData||{},hit=data.control?'button':data.encoder?'encoder':data.screen?'screen':null;
          results.push({view,role,hit,object:object?.name,pass:view==='back'?!hit:hit===role});
        }
      }
      model.group.rotation.set(-.13,-.25,.025);model.group.updateMatrixWorld(true);
      const box=new THREE.Box3().setFromObject(model.group),size=box.getSize(new THREE.Vector3()),center=box.getCenter(new THREE.Vector3()),rect=document.querySelector('#scene-mount canvas[data-engine]').getBoundingClientRect();
      const camera=new THREE.PerspectiveCamera(32,rect.width/rect.height,2,3000),tangent=Math.tan(THREE.MathUtils.degToRad(16)),distance=Math.max(20,Math.max(size.y/(2*tangent),size.x/(2*tangent*camera.aspect))*1.32+size.z/2);
      camera.position.set(center.x,center.y,center.z+distance);camera.lookAt(center);camera.updateProjectionMatrix();camera.updateMatrixWorld();
      const project=coords=>{const p=new THREE.Vector3(...coords).applyMatrix4(model.group.matrixWorld).project(camera);return {x:rect.x+(p.x+1)*rect.width/2,y:rect.y+(1-p.y)*rect.height/2};};
      const press=project([29.5,0,17]),arc=Array.from({length:7},(_,i)=>{const angle=-i*5*Math.PI/180;return project([29.5+10*Math.cos(angle),10*Math.sin(angle),16.5]);});
      model.dispose();texture.dispose();model.group.traverse(m=>{m.geometry?.dispose();for(const mat of [].concat(m.material||[]))mat.dispose();});
      return {results,press,arc};
    });
    check(geometry.results.every(r=>r.pass),`CAD controls must be visible and occluded from behind: ${JSON.stringify(geometry.results)}`);
    route=r=>r.fulfill({path:'output/tembed/factory.bin',contentType:'application/octet-stream',headers:{'Access-Control-Allow-Origin':'*'}});await page.route(source.url,route);
    await page.evaluate(async source=>{await espTester.loadFirmware(source);await espTester.start();},source);
    const waitGuest=async seconds=>{const end=Date.now()+180000;while(Date.now()<end){const s=await page.evaluate(()=>espTester.observe());if(s.status==='error')throw Error(JSON.stringify(s.lastError));if(s.stats.time>=seconds)return;await page.waitForTimeout(100);}throw Error(`Guest did not reach ${seconds}`);};
    const advance=async seconds=>{const s=await page.evaluate(()=>espTester.observe());await waitGuest(s.stats.time+seconds);};
    const key=async code=>{await page.keyboard.down(code);await advance(.12);await page.keyboard.up(code);await advance(.15);};
    const hash=()=>page.evaluate(async()=>{const f=await espTester.capture({format:'rgba'});let h=0xcbf29ce484222325n;for(let i=0;i<f.pixels.length;i+=4){const v=((f.pixels[i]>>3)<<11)|((f.pixels[i+1]>>2)<<5)|(f.pixels[i+2]>>3);for(const b of [v&255,v>>8])h=BigInt.asUintN(64,(h^BigInt(b))*0x100000001b3n);}return h.toString(16).padStart(16,'0');});
    await waitGuest(2);await page.locator('#scene-mount canvas[data-engine]').focus();await key('Space');
    check(await hash()==='61fc3a6e3c6aff88','Keyboard-mapped GPIO6 enters the factory menu.');
    const drag=async points=>{await page.mouse.move(points[0].x,points[0].y);check(await page.locator('#scene-mount').getAttribute('data-hover')==='encoder','Actual ring is an encoder target.');await page.mouse.down();for(const p of points.slice(1))await page.mouse.move(p.x,p.y);await page.mouse.up();await advance(.3);};
    await drag(geometry.arc);check(await hash()==='df3395d50c34b146','CAD ring drag reaches guest quadrature and changes selection.');
    await drag([...geometry.arc].reverse());check(await hash()==='61fc3a6e3c6aff88','Reverse CAD drag restores original selection.');
    for(let i=0;i<8;i++)await key('Space');
    await page.mouse.move(geometry.press.x,geometry.press.y);check(await page.locator('#scene-mount').getAttribute('data-hover')==='button','Center is a press target.');
    await page.mouse.down();await advance(.12);await page.mouse.up();await advance(.15);
    check(await hash()==='b1bdc75bf4e7ed76','CAD center press opens the real factory TFT page.');
    await page.screenshot({path:'output/tembed/browser-tft.png'});await page.evaluate(()=>espTester.pause());
    const shared=await page.evaluate(()=>espTester.share({mode:'embed',autostart:false}));await page.goto(shared);
    await page.waitForFunction(()=>window.espTester);check((await page.evaluate(()=>espTester.observe())).scene.productId==='lilygo-t-embed-cc1101','Embed restores the exact product.');
    await page.waitForFunction(()=>document.querySelector('#scene-mount').dataset.modelStatus==='ready');
    const embedded=await page.evaluate(async()=>({productId:(await espTester.observe()).scene.productId,encoders:(await espTester.capabilities()).input.encoders}));
    check(embedded.encoders[0].id==='encoder','Embed restores encoder capability.');await page.screenshot({path:'output/tembed/browser-embed.png'});
    check(!errors.length,`Page errors: ${errors.join('; ')}`);
    return {geometry,embedded,pageErrors:errors,firmwareDelivery:'Exact catalog URL fulfilled with cached upstream bytes; SHA and layout checks retained.'};
  } finally {if(source&&route)await page.unroute(source.url,route);page.off('pageerror',onError);await page.evaluate(()=>window.espTester?.dispose()).catch(()=>{});}
}
