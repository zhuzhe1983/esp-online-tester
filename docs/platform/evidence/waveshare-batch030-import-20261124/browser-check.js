async page => {
 const ids=["waveshare-esp32-s3-nano", "waveshare-esp32-s3-nano-m", "waveshare-esp32-s3-zero", "waveshare-esp32-s3-zero-m", "waveshare-esp32-s3-zero-n8r8", "waveshare-esp32-s3-zero-n8r8-m", "waveshare-esp32-s3-dev-kit-n8r8", "waveshare-esp32-s3-dev-kit-n8r8-m", "waveshare-esp32-s3-dev-kit-n16r8", "waveshare-esp32-s3-dev-kit-n16r8-m", "waveshare-esp32-s3-dev-kit-n32r16v", "waveshare-esp32-s3-dev-kit-n32r16v-m", "waveshare-esp32-s3-dev-kit-n16r8-u", "waveshare-esp32-s3-dev-kit-n16r8-um", "waveshare-esp32-s3-pico", "waveshare-esp32-s3-pico-m"],errors=[],failedRequests=[],rows=[];
 const onError=e=>errors.push(e.message),onResponse=r=>{if(r.status()>=400&&!r.url().endsWith('/favicon.ico'))failedRequests.push({path:new URL(r.url()).pathname,status:r.status()});};
 page.on('pageerror',onError);page.on('response',onResponse);
 await page.setViewportSize({width:1440,height:1000});
 await page.waitForFunction(()=>!!window.espTester);
 const ready=async p=>{await p.waitForFunction(()=>['ready','error'].includes(document.querySelector('#scene-mount')?.dataset.modelStatus),null,{timeout:60000});if(await p.locator('#scene-mount').getAttribute('data-model-status')!=='ready')throw Error('model failed');};
 try {
  for(const id of ids){
   await page.evaluate(async id=>{const {loadCatalog,createDefaultScene}=await import('./catalog.js');await espTester.configure(createDefaultScene(await loadCatalog(),id));},id);await ready(page);
   const state=await page.evaluate(async()=>{const s=await espTester.observe();return {productId:s.product.id,status:s.status,firmware:s.scene.firmware,controls:s.product.controls?.map(c=>c.id),modelStatus:document.querySelector('#scene-mount').dataset.modelStatus};});
   if(state.productId!==id||state.firmware!==null)throw Error(id+' wrong exact scene or unexpected firmware');
   const canvas=page.locator('#scene-mount canvas[data-engine]');if(!await canvas.isVisible())throw Error(id+' WebGL canvas not visible');
   await page.locator('#reset-view').click();await page.mouse.move(0,0);await page.waitForTimeout(100);
   const before=await canvas.screenshot({path:'output/playwright/batch030/'+id+'-studio.png'});
   const rect=await canvas.boundingBox();await page.mouse.move(rect.x+rect.width*.55,rect.y+rect.height*.55);await page.mouse.down();await page.mouse.move(rect.x+rect.width*.55+100,rect.y+rect.height*.55+40,{steps:8});await page.mouse.up();await page.waitForTimeout(100);
   const rotated=await canvas.screenshot({path:'output/playwright/batch030/'+id+'-orbit.png'});if(before.equals(rotated))throw Error(id+' orbit did not change rendered model');
   for(const shell of [{color:'#d6ad45',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}]){await page.evaluate(async shell=>{const {scene}=await espTester.observe();await espTester.updateSceneMetadata({view:{...scene.view,appearance:{shell}}});},shell);await ready(page);}
   await page.evaluate(async()=>{const {scene}=await espTester.observe();const view={...scene.view};delete view.appearance;await espTester.updateSceneMetadata({view});});await ready(page);
   await page.locator('#view-2d').click();await page.waitForFunction(()=>document.querySelector('#view-2d').getAttribute('aria-pressed')==='true');
   await page.locator('#view-3d').click();await ready(page);
   const shared=await page.evaluate(()=>espTester.share({mode:'embed',autostart:false,autoplay:false}));
   const host=await page.context().newPage();let embed;
   try {
    host.on('pageerror',onError);host.on('response',onResponse);await host.setViewportSize({width:390,height:844});await host.goto(typeof shared==='string'?shared:shared.url);await host.waitForFunction(()=>!!window.espTester);await ready(host);
    embed=await host.evaluate(async()=>{const s=await espTester.observe();return {productId:s.product.id,firmware:s.scene.firmware,status:s.status,mode:s.scene.view.mode,overflow:document.documentElement.scrollWidth>innerWidth};});
    if(embed.productId!==id||embed.firmware!==null||embed.mode!=='embed'||embed.overflow)throw Error(id+' bad mobile shared embed '+JSON.stringify(embed));
    await host.screenshot({path:'output/playwright/batch030/'+id+'-embed.png',fullPage:true});
   }finally{await host.close();}
   rows.push({...state,webGLCanvas:true,orbitChangedPixels:true,appearanceModes:['gold','transparent-purple'],viewRoundTrip:true,embed});
  }
  if(errors.length||failedRequests.length)throw Error(JSON.stringify({errors,failedRequests}));
  return {scope:'Actual headless Chromium workbench WebGL, orbit, appearance controls,2D/3D and390px shared embed for16 exact profiles; no firmware loaded or hardware claim',results:rows,pageErrors:errors,failedRequests};
 }finally{page.off('pageerror',onError);page.off('response',onResponse);}
}
