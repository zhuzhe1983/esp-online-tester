async (page) => {
 const requests=[];const listener=request=>{if(/\/versions\/\d+\/download/.test(request.url()))requests.push(request.url());};page.on('request',listener);
 const checks=[];
 await page.locator('#product-select').selectOption('m5stack-stopwatch');await page.waitForFunction(()=>document.querySelector('#firmware-select').value==='m5stack-stopwatch-official');
 try {
  const selected=await page.locator('#firmware-select').inputValue();if(selected!=='m5stack-stopwatch-official')throw Error('factory default changed');
  for(const family of ['uiflow2','xiaozhi']){
   await page.locator('#firmware-select').selectOption('m5stack-stopwatch-'+family);
   await page.locator('#load-firmware').click();await page.getByRole('dialog').waitFor();
   const title=await page.getByRole('dialog').innerText();if(!title.includes('尚未确认完整适配'))throw Error('missing trial confirmation');
   await page.getByRole('button',{name:'取消',exact:true}).click();checks.push({product:'m5stack-stopwatch',family,confirmation:true,cancelled:true});
  }
  await page.screenshot({path:'docs/platform/evidence/m5-official-firmware-catalog/stopwatch-library.png'});
  await page.locator('#product-select').selectOption('m5stack-c126');await page.waitForFunction(()=>document.querySelector('#firmware-select').value==='m5stack-c126-factory-reference');
  const atom=await page.locator('#firmware-select option').allTextContents();if(!atom.some(x=>x.includes('NiHaoXiaoZhi'))||atom.some(x=>x.includes('v2.2.6-AtomS3')))throw Error('AtomS3/R edition crossed');checks.push({product:'m5stack-c126',options:atom});
  await page.locator('#product-select').selectOption('m5stack-k146');await page.waitForFunction(()=>document.querySelector('#firmware-select').value==='m5stack-k146-official-reference');
  if(await page.locator('#load-firmware').isVisible())throw Error('unimplemented board exposed run');if(!(await page.locator('body').innerText()).includes('尚未完成适配'))throw Error('missing board label');checks.push({product:'m5stack-k146',factoryDefault:true,runHidden:true});
  if(requests.length)throw Error('confirmation cancel downloaded firmware');
  return {checks,firmwareDownloadRequests:requests.length,firmwaresExecuted:0};
 }finally{page.off('request',listener);}
}
