// Run against the local workbench with the Playwright CLI wrapper.
async page => {
  const base=new URL('./',page.url()).href,errors=[],checks=[];
  const assert=(value,message)=>{if(!value)throw Error(message);};
  page.on('pageerror',error=>errors.push(error.message));
  await page.setViewportSize({width:1440,height:1000});
  await page.goto(base);await page.waitForFunction(()=>!!window.espTester);
  const configure=async id=>{
    await page.evaluate(async id=>{const {loadCatalog,createDefaultScene}=await import('./catalog.js');await espTester.configure(createDefaultScene(await loadCatalog(),id));},id);
    await page.waitForFunction(id=>document.querySelector('#product-select')?.value===id,id);
  };
  const ports=()=>page.locator('#accessory-interface option').evaluateAll(options=>options.map(o=>o.value));
  await configure('m5stack-stopwatch');
  assert(JSON.stringify(await ports())===JSON.stringify(['HY2.0-4P','USB-C','pin-header','*']),'StopWatch external connectors exclude its internal buses');
  assert(await page.locator('#accessory-interface').inputValue()==='HY2.0-4P','Default interface has catalog accessories');
  await page.locator('#accessory-interface').selectOption('USB-C');
  assert(await page.locator('#add-accessory').isDisabled(),'No USB-C catalog choice means no add action');
  await page.locator('#accessory-interface').selectOption('*');
  assert(!(await page.locator('#add-accessory').isDisabled()),'Explicit browsing restores catalog choices');
  await page.locator('#product-select').selectOption('m5stack-triviapod');
  await page.waitForFunction(()=>document.querySelector('#accessory-interface')?.value==='USB-C');
  await page.waitForFunction(()=>document.querySelector('[data-model-status="ready"]'));
  assert(JSON.stringify(await ports())===JSON.stringify(['USB-C','*']),'TriviaPOD independently exposes only its verified charging socket');
  assert(await page.locator('#add-accessory').isDisabled(),'Device change resets explicit all-interface browsing');
  assert(await page.locator('#visual-variant-menu').isHidden(),'TriviaPOD is not a StopWatch preview');
  assert((await page.evaluate(async()=>(await espTester.observe()).scene)).productId==='m5stack-triviapod','Standalone selection updates the scene identity');
  await page.screenshot({path:'output/playwright/triviapod-independent-interfaces.png'});
  checks.push('Real product picker: StopWatch → TriviaPOD, connector defaults reset, USB charging only, no inherited variants or accessory choices');
  await configure('lilygo-t-higrow');
  assert(JSON.stringify(await ports())===JSON.stringify(['','*']),'Unknown ports are not invented');
  assert(await page.locator('#add-accessory').isDisabled(),'Unknown ports do not default to Grove');
  assert((await page.locator('#accessory-filter-note').innerText()).includes('待核对'),'Missing evidence is visible');
  checks.push('Unknown LILYGO interfaces remain explicit and all-interface browsing remains available');
  await configure('m5stack-s016');
  assert((await ports()).includes('FPC-pad')&&!(await ports()).includes('FPC-0.5mm-12P'),'Bare C5 solder pads do not become a fitted DIP connector');
  await configure('m5stack-s007');await page.waitForFunction(()=>document.querySelector('[data-model-status="ready"]'));
  const order=async()=>{const variant=await page.locator('#visual-variant-menu').boundingBox(),shell=await page.locator('#shell-settings').boundingBox();assert(variant&&shell&&variant.x<shell.x,'Variant menu belongs left of shell');return {variantX:variant.x,shellX:shell.x};};
  const workbenchOrder=await order();
  await page.locator('#visual-variant-menu summary').click();await page.locator('#visual-variant-select').selectOption('m5stack-s007-pin127');
  await page.waitForFunction(()=>document.querySelector('[data-model-status="ready"]'));
  assert((await page.evaluate(async()=>(await espTester.observe()).scene)).productId==='m5stack-s007','Derivative selection is preview-only');
  assert((await ports()).includes('USB-C'),'Preview keeps parent interfaces');
  await page.screenshot({path:'output/playwright/variant-left-of-shell.png'});
  const hash=await page.evaluate(async()=>{const {encodeScene}=await import('./catalog.js');return '#scene='+encodeScene((await espTester.observe()).scene);});
  await page.goto(base+'embed.html'+hash);await page.waitForFunction(()=>document.querySelector('[data-model-status="ready"]'));
  const miniOrder=await order();
  checks.push({variantMenuOrder:{workbench:workbenchOrder,mini:miniOrder},previewKeepsParentProduct:true});
  assert(!errors.length,errors.join('\n'));return checks;
}
