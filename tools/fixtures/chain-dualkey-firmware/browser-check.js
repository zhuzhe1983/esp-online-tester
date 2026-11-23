async page => {
  const origin = new URL(page.url()).origin;
  const factoryId = 'm5stack-c147-userdemo-v0-8';
  const errors = [];
  page.on('pageerror', error=>errors.push(error.message));
  const must = (result, label) => { if (!result) throw Error(label); };
  await page.setViewportSize({width:1280,height:720});
  await page.waitForFunction(()=>window.espTester);
  await page.evaluate(async () => {
    const {loadCatalog,createDefaultScene}=await import('./catalog.js');
    await espTester.configure(createDefaultScene(await loadCatalog(),'m5stack-c147'));
  });
  await page.locator('#product-select').selectOption('m5stack-c147');
  await page.waitForFunction(id => document.querySelector('#firmware-select')?.value===id,factoryId);
  const defaultSelected = await page.evaluate(() => document.querySelector('#firmware-select').value);
  await page.evaluate(async () => {
    const res = await fetch('/c147-fixture.bin');
    if (!res.ok) throw Error('Local diagnostic HTTP '+res.status);
    await espTester.loadFirmware({file:new File([await res.arrayBuffer()],'c147-fixture.bin')});
    await espTester.start();
  });
  const expect = async (line,leds,after=0) => {
    const end=Date.now()+30000;
    while(Date.now()<end) {
      const result=await page.evaluate(async ({line,leds,after}) => {
        const s=await espTester.observe();
        const text=s.serialByPort.uart0;
        return text?.slice(after).includes(line) && JSON.stringify(s.leds)===JSON.stringify(leds) ? text.length : 0;
      },{line,leds,after});
      if(result) return result;
      await new Promise(resolve=>setTimeout(resolve,50));
    }
    throw Error('Missing observation: '+line+' '+JSON.stringify(leds));
  };
  const off=[[0,0,0],[0,0,0]];
  const red=[[0,0,0],[127,0,0]];
  const green=[[0,191,0],[0,0,0]];
  let start=await expect('C147 KEYS 0 0',off);
  await page.evaluate(() => document.activeElement.blur());
  await page.keyboard.down('a');
  start=await expect('C147 KEYS 1 0',red,start);
  await page.keyboard.up('a');
  start=await expect('C147 KEYS 0 0',off,start);
  const canvas=page.locator('#scene-mount canvas:visible');
  const box=await canvas.boundingBox();
  const key1={x:box.x+box.width*.63,y:box.y+box.height*.46};
  await page.mouse.move(key1.x,key1.y);
  must(await page.locator('#scene-mount').getAttribute('data-hover')==='button','No 3D key hit');
  await page.mouse.down();
  start=await expect('C147 KEYS 1 0',red,start);
  await page.mouse.up();
  start=await expect('C147 KEYS 0 0',off,start);
  const key2=await page.locator('#device-buttons [data-control="key2"]').boundingBox();
  await page.mouse.move(key2.x+key2.width/2,key2.y+key2.height/2);
  await page.mouse.down();
  start=await expect('C147 KEYS 0 1',green,start);
  await page.mouse.up();
  await expect('C147 KEYS 0 0',off,start);
  await page.locator('#view-2d').click();
  await page.locator('#view-3d').click();
  await page.getByRole('button',{name:'Inspect parts'}).click();
  await page.getByRole('button',{name:'Reassemble'}).click();
  const shared=await page.evaluate(async () => {
    const {createDefaultScene,encodeScene,loadCatalog}=await import('./catalog.js');
    return encodeScene(createDefaultScene(await loadCatalog(),'m5stack-c147'));
  });
  const embed=await page.context().newPage();
  try {
    await embed.goto(origin+'/platform/embed.html?lang=en#scene='+shared+'&autostart=0');
    await embed.waitForFunction(()=>window.espTester);
    await embed.evaluate(()=>espTester.waitFor({status:'configured',timeoutMs:12000}));
    const state=await embed.evaluate(async()=>await espTester.observe());
    must(state.product.runtime.board==='chain-dualkey','Shared board missing');
    must(state.product.display===null,'C147 must remain displayless');
  } finally {await embed.close();}
  must(errors.length===0,errors.join('; '));
  return {productId:'m5stack-c147',defaultSelected,keyboard:'KEY1 to UART/RGB',
    modelPress:'KEY1 to UART/RGB',panelPress:'KEY2 to UART/RGB',viewModes:['2d','3d'],
    inspection:true,independentEmbed:true,errors};
}
