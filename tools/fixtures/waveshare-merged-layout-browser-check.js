// Run on the workbench with Playwright CLI; uses the catalog's original remote images.
async page => {
  const results=[],errors=[];
  page.on('pageerror',error=>errors.push(error.message));
  for(const revision of ['v13','v32']){
    await page.evaluate(async revision=>{
      const api=espTester,{loadCatalog,createDefaultScene}=await import('./catalog.js');
      const productId=`waveshare-esp32-p4-wifi6-touch-lcd-3-5-${revision}`;
      const catalog=await loadCatalog(),scene=createDefaultScene(catalog,productId);
      const product=catalog.products.find(product=>product.id===productId);
      scene.firmware=catalog.firmwares.find(firmware=>firmware.id===product.defaultFirmwareId).source;
      await api.configure(scene);
    },revision);
    await page.getByRole('button',{name:'下载并尝试运行',exact:true}).click();
    await page.getByRole('button',{name:'继续运行',exact:true}).click();
    const result=await page.evaluate(async()=>{
      const api=espTester;
      const serial=state=>Object.values(state.serialByPort||{}).join('\n');
      const end=Date.now()+90000;let state;
      while(Date.now()<end){
        state=await api.observe();
        if(state.status==='error')throw Error(JSON.stringify(state.lastError));
        if(state.stats?.time>=5&&state.frame&&serial(state).includes('Starting Wi-Fi...'))break;
        await new Promise(resolve=>setTimeout(resolve,30));
      }
      if(!(state.stats?.time>=5&&state.frame&&serial(state).includes('Starting Wi-Fi...')))throw Error('Real factory boot output timed out');
      await api.pause();state=await api.observe();const frame=await api.capture({format:'rgba'});
      if(state.firmware.appDirect||state.firmware.flashLayout?.partitionTableOffset!==0x9000||state.firmware.flashLayout?.bootloaderOffset!==0x2000)throw Error('Original merged image did not retain its ROM boot layout');
      if(frame.width!==320||frame.height!==480)throw Error('Unexpected guest framebuffer dimensions');
      const colors=new Set();for(let i=0;i<frame.pixels.length;i+=4)colors.add(`${frame.pixels[i]},${frame.pixels[i+1]},${frame.pixels[i+2]}`);
      if(colors.size<10)throw Error('No actual factory UI rendered');
      const sha=async bytes=>[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(value=>value.toString(16).padStart(2,'0')).join('');
      return {productId:state.scene.productId,firmware:state.firmware,guestSeconds:state.stats.time,frame:{width:frame.width,height:frame.height,colors:colors.size,rgbaSha256:await sha(frame.pixels)},startupReachedWifi:true,guestFaultObserved:/Guru Meditation|Instruction access fault|panic'ed/.test(serial(state)),factoryFirmwareVerified:false};
    });
    await page.waitForFunction(()=>document.querySelector('#scene-mount').dataset.modelStatus==='ready');
    await page.screenshot({path:`output/playwright/waveshare-merged-layout-${revision}.png`,fullPage:true});
    results.push(result);
  }
  if(errors.length)throw Error(JSON.stringify(errors));
  return {results,pageErrors:errors};
}
