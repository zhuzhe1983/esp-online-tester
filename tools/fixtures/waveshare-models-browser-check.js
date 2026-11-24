// Model and embedding checks only. Use the C5 fixture separately for actual guest execution.
async page => {
  const errors=[], failures=[], result={models:[],pageErrors:errors,failedRequests:failures};
  page.on('pageerror',e=>errors.push(e.message));
  page.on('response',r=>{if(r.status()>=400)failures.push({status:r.status(),path:new URL(r.url()).pathname});});
  await page.setViewportSize({width:1440,height:1000});
  await page.waitForFunction(()=>window.espTester);
  for(const id of ['waveshare-esp32-c5-lcd-1-47','waveshare-esp32-c6-zero','waveshare-esp32-c3-lcd-0-71','waveshare-esp32-s3-touch-lcd-5','waveshare-esp32-dev-kit-wroom-32e-n4','waveshare-esp32-s2-pico','waveshare-esp32-h2-zero','waveshare-esp32-s31-zero-n16r16','waveshare-esp32-p4-core']) {
    const state=await page.evaluate(async id=>{
      const {loadCatalog,createDefaultScene}=await import('./catalog.js');
      await espTester.configure(createDefaultScene(await loadCatalog(),id));
      const s=await espTester.observe();
      return {productId:id,status:s.status};
    },id);
    await page.waitForFunction(()=>document.querySelector('#scene-mount')?.dataset.modelStatus==='ready',null,{timeout:60000});
    if(await page.locator('#scene-mount canvas').count()===0)throw Error(id+': missing canvas');
    result.models.push({...state,modelStatus:'ready'});
    if(result.models.length<=3)await page.screenshot({path:'output/playwright/'+id+'.png',fullPage:true});
  }
  await page.getByRole('button',{name:'2D',exact:true}).click();
  await page.getByRole('button',{name:'3D',exact:true}).click();
  result.viewModeRoundTrip=true;
  const link=await page.evaluate(()=>espTester.share({mode:'embed',autostart:false}));
  const mini=await page.context().newPage();
  try {
    mini.on('pageerror',e=>errors.push(e.message));
    await mini.setViewportSize({width:390,height:844});
    await mini.goto(typeof link==='string'?link:link.url);
    await mini.waitForFunction(()=>window.espTester&&document.querySelector('#scene-mount')?.dataset.modelStatus==='ready',null,{timeout:60000});
    result.embed=await mini.evaluate(async()=>({productId:(await espTester.observe()).scene.productId,horizontalOverflow:document.documentElement.scrollWidth>innerWidth,modelStatus:document.querySelector('#scene-mount').dataset.modelStatus}));
    await mini.screenshot({path:'output/playwright/waveshare-embed-mobile.png',fullPage:true});
    if(result.embed.horizontalOverflow)throw Error('Mobile embed overflow');
  } finally {await mini.close();}
  if(errors.length||failures.length)throw Error(JSON.stringify(result));
  return result;
}
