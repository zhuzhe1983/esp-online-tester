// Metadata doubles only; this check never downloads or flashes firmware.
async page => {
  const base=new URL('./',page.url()).href,errors=[],requests=[],assert=(ok,message)=>{if(!ok)throw Error(message);};
  page.on('pageerror',error=>errors.push(error.message));
  const observeProduct=async id=>page.evaluate(async id=>{for(let i=0;i<200;i++){const s=await window.espTester?.observe();if(s?.scene&&(!id||s.scene.productId===id))return s;await new Promise(resolve=>setTimeout(resolve,50));}throw Error('Product configuration did not settle');},id);
  const device={deviceId:'1897100000000000005',deviceName:'StopWatch',productUrl:'https://docs.m5stack.com/en/core/StopWatch'};
  const other={deviceId:'1897100000000000006',deviceName:'TriviaPOD',productUrl:'https://docs.m5stack.com/en/core/TriviaPOD'};
  const firmwareId='1897200000000000718',versionId='2098280161648791553';
  const project={firmwareId,firmwareName:'StopWatch metadata fixture',sourceType:'OFFICIAL',supportedDevices:[device],currentVersion:{versionId}};
  const version={versionId,versionName:'v1.2.3',visibility:'PUBLIC',status:'PUBLISHED',supportedDevices:[device],current:true};
  await page.route('https://burner.m5stack.com/api/v1/**',async route=>{
    const url=new URL(route.request().url());requests.push(url.pathname+url.search);
    assert(!url.pathname.endsWith('/download'),'Metadata checks cannot download firmware');
    await route.fulfill({json:url.pathname.endsWith('/devices')?{code:200,data:[device,other]}:url.pathname.endsWith('/versions')?{code:200,data:[version]}:{code:200,total:1,rows:[project]}});
  });
  await page.route('https://m5burner-cdn.m5stack.com/**',async route=>{
    assert(route.request().method()==='HEAD','Version checking must not download firmware');
    await route.fulfill({status:200,body:''});
  });
  await page.setViewportSize({width:1440,height:1000});await page.goto(base);await observeProduct();
  await page.evaluate(async()=>{const {loadCatalog,createDefaultScene}=await import('./catalog.js'),c=await loadCatalog(),s=createDefaultScene(c,'m5stack-stopwatch');s.firmware=c.firmwares.find(f=>f.id==='m5stack-stopwatch-official').source;await espTester.configure(s);});
  await page.locator('#product-select').selectOption('m5stack-triviapod');
  await page.waitForFunction(()=>document.querySelector('#firmware-select')?.value==='m5stack-stopwatch-official');
  const state=await observeProduct('m5stack-triviapod');
  assert(state.scene.productId==='m5stack-triviapod'&&state.product.runtime.flashMb===32&&state.product.runtime.board===null,'Firmware target leaves the physical product and runtime unchanged');
  const options=await page.locator('#firmware-select option').evaluateAll(items=>items.map(o=>o.value));
  assert(options.length===3&&options.every(id=>id.startsWith('m5stack-stopwatch-')),'TriviaPOD discovers the three StopWatch catalog candidates');
  assert((await page.locator('#firmware-select optgroup').first().getAttribute('label')).startsWith('共享目标'),'Shared entries have their own group');
  assert((await page.locator('.firmware-update-identity').innerText()).includes('当前型号待验证'),'Version panel does not inherit factory or verification status');
  assert(await page.locator('#firmware-note').getByRole('button',{name:'已验证操作',exact:true}).count()===0,'Source product validation is not relabeled as current-product validation');
  assert(await page.locator('#flash-device').isDisabled(),'A discovery pointer cannot authorize hardware flashing');
  await page.getByRole('button',{name:'检查版本',exact:true}).click();
  await page.locator('.watch-version').waitFor();assert((await page.locator('.watch-version').innerText()).includes('V0.5'),'Version lookup follows the canonical firmware target');
  await page.locator('[data-model-status="ready"]').waitFor();
  await page.screenshot({path:'output/playwright/firmware-target-triviapod.png'});
  const links=await page.evaluate(async()=>({studio:await espTester.share({mode:'studio',autostart:false,autoplay:false}),mini:await espTester.share({mode:'embed',autostart:false,autoplay:false})}));
  await page.locator('[data-firmware-tab="remote"]').click();await page.locator('#firmware-kind').selectOption('m5burner');
  await page.waitForFunction(()=>document.querySelector('#burner-version')?.value==='2098280161648791553');
  assert(requests.some(url=>url.includes('deviceId='+device.deviceId))&&!requests.some(url=>url.includes('deviceId='+other.deviceId)),'M5Burner queries the target device, not the product identity');
  assert((await page.locator('#firmware-burner').innerText()).includes('TriviaPOD')&&(await page.locator('#firmware-burner').innerText()).includes('兼容性待验证'),'Shared identity remains visible after a successful remote query');
  assert(await page.locator('#flash-device').isDisabled(),'Remote shared firmware follows the same flashing boundary');
  const historyIsolation=await page.evaluate(async({deviceId,firmwareId,versionId})=>{
    const {mountM5Burner}=await import('./m5burner-ui.js'),{loadCatalog}=await import('./catalog.js'),catalog=await loadCatalog();
    const actual=catalog.products.find(p=>p.id==='m5stack-triviapod'),target=catalog.products.find(p=>p.id==='m5stack-stopwatch');
    const hub=mountM5Burner({mount:document.createElement('section')}),entry={provider:'m5burner',productId:actual.id,firmwareTargetId:target.id,deviceId,url:`https://burner.m5stack.com/api/v1/firmwares/${firmwareId}/versions/${versionId}/download`,name:'Metadata fixture',version:'v1.2.3'};
    try {
      hub.setProduct(actual,{firmwareProduct:target});
      const accepted=hub.restore(entry),shared=hub.current()?.sharedTargetId===target.id;
      const staleRejected=!hub.restore({...entry,firmwareTargetId:actual.id});
      hub.setProduct(target);const isolated=!hub.restore(entry);
      return accepted&&shared&&staleRejected&&isolated;
    }finally{hub.deactivate();}
  },{deviceId:device.deviceId,firmwareId,versionId});
  assert(historyIsolation,'Remote history belongs to the actual product and rejects a stale target');
  for(const [mode,url] of Object.entries(links)) {
    await page.goto(url);
    const restored=await observeProduct('m5stack-triviapod');
    assert(restored.scene.productId==='m5stack-triviapod'&&restored.scene.firmware.url===state.scene.firmware.url,`${mode} restores the actual product and selected source`);
    assert(await page.locator('#flash-device').isDisabled(),`${mode} cannot bypass shared firmware flashing policy`);
    if(mode==='studio')assert((await page.locator('.firmware-update-identity').innerText()).includes('当前型号待验证'),'Shared source restore retains trial status');
  }
  assert(!errors.length,errors.join('\n'));
  return {catalogCandidates:options,productId:state.scene.productId,flashMb:state.product.runtime.flashMb,runtimeBoard:state.product.runtime.board,version:'V0.5',m5burnerTarget:device.deviceName,sharedTrial:true,historyIsolation,studioAndMiniRestore:true,hardwareFlashing:false,pageErrors:errors};
}
