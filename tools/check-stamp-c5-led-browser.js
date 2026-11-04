// Playwright CLI run-code --filename from a public platform page. The caller
// supplies ?c5ProbeUrl=<served firmware/c5-uart-probe/probe.bin>&c5ProbeSha=<sha>.
// This uses the real Worker/WASM and public UART API; never overrides LED output.
async page => {
  const query=new URL(page.url()).searchParams,probeUrl=query.get('c5ProbeUrl'),sha256=query.get('c5ProbeSha');
  if(!probeUrl||!/^[a-f0-9]{64}$/i.test(sha256||''))throw Error('Supply the source-built C5 UART probe URL and SHA-256.');
  const errors=[],results=[];const onError=error=>errors.push(error.message);page.on('pageerror',onError);
  const assert=(value,message)=>{if(!value)throw Error(message);};
  try {
    await page.setViewportSize({width:1440,height:1050});await page.waitForFunction(()=>window.espTester);
    const client=await page.context().newCDPSession(page);await client.send('Network.setCacheDisabled',{cacheDisabled:true});await client.send('Network.clearBrowserCache');
    for(const productId of ['m5stack-s016','m5stack-s016-dip']) {
      await page.evaluate(async({productId,probeUrl,sha256})=>{
        const{loadCatalog,createDefaultScene}=await import('./catalog.js'),scene=createDefaultScene(await loadCatalog(),productId);
        scene.firmware={url:probeUrl,sha256,bootMode:'app-direct'};scene.view.deviceView='3d';await espTester.configure(scene);
      },{productId,probeUrl,sha256});
      await page.waitForFunction(()=>document.querySelector('#scene-mount')?.dataset.modelStatus==='ready');
      await page.locator('#reset-view').click();await page.mouse.move(0,0);
      const projection=await page.evaluate(async()=>{
        const THREE=await import('./vendor/three.module.min.js'),{createManufacturerModel}=await import('./manufacturer-model.js');
        const model=createManufacturerModel((await espTester.observe()).product,null);await model.ready;
        try {
          model.group.rotation.set(-.13,-.25,.025);model.group.updateMatrixWorld(true);
          const bounds=new THREE.Box3().setFromObject(model.group),size=bounds.getSize(new THREE.Vector3()),center=bounds.getCenter(new THREE.Vector3()),rect=document.querySelector('#scene-mount canvas[data-engine]').getBoundingClientRect();
          const camera=new THREE.PerspectiveCamera(32,rect.width/rect.height,2,3000),tangent=Math.tan(THREE.MathUtils.degToRad(16)),distance=Math.max(20,Math.max(size.y/(2*tangent),size.x/(2*tangent*camera.aspect))*1.32+size.z/2);
          camera.position.set(center.x,center.y,center.z+distance);camera.lookAt(center);camera.updateProjectionMatrix();camera.updateMatrixWorld();
          const lens=model.parts['status-blue-led'],positions=lens.geometry.attributes.position,points=[];
          for(let i=0;i<positions.count;i++){const p=new THREE.Vector3().fromBufferAttribute(positions,i).applyMatrix4(lens.matrixWorld).project(camera);points.push([(p.x+1)*rect.width/2,(1-p.y)*rect.height/2]);}
          return {method:'Reconstruct the public reset-view camera from the same native model and canvas dimensions; no change to the running scene.',canvasCssSize:[rect.width,rect.height],devicePixelRatio:devicePixelRatio,emittingPart:'status-blue-led',emittingFaceCanvasVertices:points};
        } finally {model.dispose();model.group.traverse(m=>{m.geometry?.dispose();for(const material of[].concat(m.material||[]))material.dispose();});}
      });
      await page.evaluate(async()=>{await espTester.loadFirmware();await espTester.start();await espTester.waitFor({port:'uart0',serialIncludes:'C5 UART IRQ READY\n',timeoutMs:20000});});
      const stages=[];
      const capture=async(name,expected)=>{
        let state;const deadline=Date.now()+10000;
        do {state=await page.evaluate(async()=>{const s=await espTester.observe();return {status:s.status,leds:s.leds,serial:s.serialByPort?.uart0,stats:s.stats,lastError:s.lastError};});if(JSON.stringify(state.leds)===JSON.stringify([expected]))break;await page.waitForTimeout(20);}while(Date.now()<deadline);
        assert(JSON.stringify(state.leds)===JSON.stringify([expected]),`${productId}/${name}: expected ${JSON.stringify([expected])}, got ${JSON.stringify(state.leds)}`);
        assert(state.status==='running'&&!state.lastError,`${productId}/${name}: guest not running`);
        await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
        await page.locator('#scene-mount canvas[data-engine]').screenshot({path:`output/playwright/${productId}-uart-led-${name}.png`});stages.push({name,...state});
      };
      const command=async(data,name,expected)=>{
        const receipt=await page.evaluate(async data=>{const fresh=espTester.waitFor({port:'uart0',serialIncludes:data,fromNow:true,timeoutMs:10000});const receipt=await espTester.input({type:'serial',port:'uart0',data});await fresh;return receipt;},data);
        await capture(name,expected);stages.at(-1).input={data,port:'uart0',receipt};
      };
      await capture('ready-off',[0,0,0]);
      await command('1','on',[0,0,255]);await command('0','off',[0,0,0]);await command('W','wrong-g27',[0,0,0]);
      await command('1','before-reset-on',[0,0,255]);
      await page.evaluate(async()=>{await espTester.reset();await espTester.waitFor({port:'uart0',serialIncludes:'C5 UART IRQ READY\n',timeoutMs:20000});});
      await capture('reset-off',[0,0,0]);await command('1','after-reset-on',[0,0,255]);await command('0','final-off',[0,0,0]);
      await page.evaluate(()=>espTester.pause());results.push({productId,firmwareSha256:sha256,projection,stages});
    }
    assert(!errors.length,errors.join('; '));return {results,pageErrors:errors,scope:'Actual source-built guest UART IRQ -> GPIO28 -> board LED index0 -> public 3D model. No photometric or physical hardware validation.'};
  } finally {page.off('pageerror',onError);await page.evaluate(()=>window.espTester?.dispose()).catch(()=>{});}
}
