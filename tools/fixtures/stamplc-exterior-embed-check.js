async page=>{
 const base=new URL('./',page.url());await page.goto(new URL('index.html?autostart=0',base).href);await page.waitForFunction(()=>window.espTester);
 const shared=await page.evaluate(async()=>{const{loadCatalog,createDefaultScene}=await import('./catalog.js'),catalog=await loadCatalog(),scene=createDefaultScene(catalog,'m5stack-k141');scene.firmware=null;await espTester.configure(scene);return await espTester.share({mode:'embed',autostart:false});});
 const href=typeof shared==='string'?shared:shared.url;if(!href)throw Error('No share URL');
 await page.goto(href);await page.waitForFunction(()=>window.espTester);await page.waitForFunction(async()=>{const s=await espTester.observe();return s.scene?.productId==='m5stack-k141';});await page.locator('[data-model-status="ready"]').waitFor();
 const standalone=await page.evaluate(async()=>{const s=await espTester.observe();return {productId:s.scene.productId,status:s.status,canvas:!!document.querySelector('canvas'),model:document.querySelector('[data-model-status]')?.dataset.modelStatus};});
 await page.screenshot({path:'output/playwright/stamplc-refresh/embed-standalone.png'});
 await page.goto(new URL('index.html?autostart=0',base).href);await page.evaluate(href=>{const frame=document.createElement('iframe');frame.id='stamplc-check-frame';frame.src=href;frame.style='position:fixed;inset:0;width:100%;height:100%;z-index:10000;background:white';document.body.append(frame);},href);
 const f=page.frameLocator('#stamplc-check-frame');await f.locator('[data-model-status="ready"]').waitFor();
 const embedded=await page.locator('#stamplc-check-frame').evaluate(async element=>{const s=await element.contentWindow.espTester.observe();return{productId:s.scene.productId,status:s.status,model:element.contentDocument.querySelector('[data-model-status]')?.dataset.modelStatus};});
 await page.locator('#stamplc-check-frame').screenshot({path:'output/playwright/stamplc-refresh/embed-iframe.png'});
 if(standalone.productId!=='m5stack-k141'||embedded.productId!=='m5stack-k141'||standalone.model!=='ready'||embedded.model!=='ready')throw Error('Model share restore failed');
 return {standalone,embedded,scope:'Share restores the product and final manufacturer model in standalone mini and iframe; autostart disabled, no firmware or physical flashing performed here.'};
}
