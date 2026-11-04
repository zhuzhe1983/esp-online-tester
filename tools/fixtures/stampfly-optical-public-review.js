async(page)=>{
 if(await page.evaluate(()=>!!document.fullscreenElement)){await page.evaluate(()=>document.exitFullscreen());await page.waitForFunction(()=>!document.fullscreenElement);}
 await page.locator('#product-family-select').selectOption('');const records=[];
 for(const sku of ['m5stack-k138','m5stack-k138-v11']){
  await page.locator('#product-select').selectOption(sku);await page.waitForFunction(()=>document.querySelector('#scene-mount').dataset.modelStatus==='ready');await page.waitForTimeout(500);await page.locator('#inspect-model').waitFor({state:'visible'});await page.locator('#reset-view').click();await page.waitForTimeout(500);await page.locator('#inspect-model').click();await page.waitForFunction(()=>document.querySelector('#inspect-model').getAttribute('aria-pressed')==='true');
  if(await page.locator('#inspect-model').getAttribute('aria-pressed')!=='true')throw Error('Inspection not expanded');await page.locator('#fullscreen').click();await page.waitForFunction(()=>!!document.fullscreenElement);await page.waitForTimeout(300);
  const canvas=page.locator('#scene-mount canvas[data-engine]');const b=await canvas.boundingBox();const targetX=sku==='m5stack-k138'?.58:.52;await page.mouse.move(b.x+b.width*.5,b.y+b.height*.65);await page.mouse.down();await page.mouse.move(b.x+b.width*targetX,b.y+b.height*.48,{steps:20});await page.mouse.up();await page.mouse.move(b.x+b.width*.5,b.y+b.height*.55);await page.mouse.wheel(0,-450);await page.waitForTimeout(250);if(await page.locator('#inspect-model').getAttribute('aria-pressed')!=='true')throw Error('Inspection lost during orbit');
  await canvas.screenshot({path:`output/playwright/optical-detail/${sku}/public-unknown-readable.png`});await page.evaluate(()=>document.exitFullscreen());await page.waitForFunction(()=>!document.fullscreenElement);await page.locator('#inspect-model').click();if(await page.locator('#inspect-model').getAttribute('aria-pressed')!=='false')throw Error('Public restore failed');
  records.push({productId:sku,control:'#inspect-model',fullscreen:true,gesture:{dragFrom:[.5,.65],dragTo:[targetX,.48],wheelDeltaY:-450},unknownReadable:"requires manual image review",restored:true,firmwareStarted:false});
 }
 return records;
}
