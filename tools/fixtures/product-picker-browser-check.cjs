// Run via the tracked Playwright CLI run-code command on the platform workbench.
async (page) => {
  const hidden=['m5stack-s007-pin127','m5stack-s007-pin254','m5stack-s007-pin127-v033','m5stack-s007-pin254-v033','m5stack-s015-dip','m5stack-s016-dip','m5stack-k051-b','m5stack-k051','m5stack-k056','m5stack-k122'];
  const retained=['m5stack-s007','m5stack-s007-v033','m5stack-s015','m5stack-s016','m5stack-c050-b','m5stack-c056-b','m5stack-c122-b','m5stack-k138','m5stack-k138-v11'];
  const errors=[],onError=error=>errors.push(error.message),onConsole=message=>{if(message.type()==='error')errors.push(message.text());};
  page.on('pageerror',onError);page.on('console',onConsole);
  const check=(value,message)=>{if(!value)throw Error(message);};
  const ready=()=>page.waitForFunction(()=>window.espTester&&!document.querySelector('#product-select').disabled);
  const current=()=>page.evaluate(async()=>{const state=await espTester.observe();return {id:state.scene.productId,model:state.product.model?.url,selected:document.querySelector('#product-select').value,options:[...document.querySelector('#product-select').options].map(o=>o.value),count:document.querySelector('#product-count').textContent};});
  const assertCurrent=async id=>{await page.waitForFunction(id=>document.querySelector('#product-select').value===id,id);const state=await current();check(state.id===id&&state.selected===id,`exact scene and picker identity: ${id}`);return state;};
  try {
    await ready();
    const catalog=await page.evaluate(async()=>{const {loadCatalog}=await import('./catalog.js');return loadCatalog();});
    const visible=catalog.products.filter(p=>!hidden.includes(p.id)),stamp=visible.filter(p=>p.family?.id==='m5-stamp');
    check(catalog.products.filter(p=>p.pickerVisible===false).length===hidden.length,'only reviewed variants hidden');
    await page.getByRole('combobox',{name:'厂家',exact:true}).selectOption('M5Stack');
    await page.getByRole('combobox',{name:'选择产品系列',exact:true}).selectOption('m5-stamp');
    let state=await current();
    check(state.options.length===stamp.length&&hidden.every(id=>!state.options.includes(id)),'ordinary Stamp dropdown omits variants');
    check(retained.every(id=>state.options.includes(id)),'distinct main boards and versions remain');
    const familyLabel=await page.locator('#product-family-select option[value="m5-stamp"]').textContent();
    check(familyLabel===`Stamp 系列 · ${stamp.length} 款`,'family count matches visible products');
    check(state.count===`${stamp.filter(p=>p.runtime?.board).length} / ${stamp.length} 款`,'device count matches visible products');
    await page.getByRole('button',{name:'搜索所有设备',exact:true}).click();
    const search=page.getByRole('searchbox',{name:'跨品牌搜索设备'});
    check(await page.locator('#device-search-dialog .device-search-result').count()===visible.length,'global search count');
    for(const id of hidden){await search.fill(id);check(await page.locator('#device-search-dialog .device-search-result').count()===0,`hidden search ${id}`);}
    await search.fill('Arduino');check(await page.locator('#device-search-dialog .device-search-result').count()>0,'unrelated manufacturer remains searchable');
    await page.getByRole('button',{name:'关闭设备搜索'}).click();
    const restored=[];
    for(const id of hidden){
      const product=catalog.products.find(p=>p.id===id),modelUrl=new URL(product.model.url,page.url());
      const response=page.waitForResponse(res=>new URL(res.url()).pathname===modelUrl.pathname&&res.status()===200);
      await page.evaluate(async id=>{const {loadCatalog,createDefaultScene}=await import('./catalog.js');await espTester.configure(createDefaultScene(await loadCatalog(),id));},id);
      await response;await page.waitForFunction(()=>document.querySelector('#scene-mount').dataset.modelStatus==='ready');
      state=await assertCurrent(id);
      check(state.model===product.model.url&&state.options.filter(value=>hidden.includes(value)).join()===id,`only exact current variant is temporarily listed: ${id}`);
      check(state.count===`${stamp.filter(p=>p.runtime?.board).length} / ${stamp.length} 款`,'current hidden variant does not inflate count');
      await page.getByRole('combobox',{name:'选择产品系列',exact:true}).selectOption('');await assertCurrent(id);
      await page.getByRole('combobox',{name:'选择产品系列',exact:true}).selectOption('m5-stamp');await assertCurrent(id);
      const share=await page.evaluate(()=>espTester.share({mode:'studio',autostart:false}));
      await page.goto(share);await ready();await assertCurrent(id);
      await page.waitForFunction(()=>document.querySelector('#scene-mount').dataset.modelStatus==='ready');
      check((await current()).model===product.model.url,`shared variant keeps model: ${id}`);
      restored.push({id,model:product.model.url,api:true,share:true,familyFilter:true,modelReady:true});
    }
    await page.getByRole('combobox',{name:/^具体型号/}).selectOption('m5stack-c050-b');
    state=await assertCurrent('m5stack-c050-b');check(hidden.every(id=>!state.options.includes(id)),'temporary option disappears after choosing ordinary product');
    await page.getByRole('button',{name:'搜索所有设备',exact:true}).click();
    await search.fill('m5stack-c122-b');await page.locator('#device-search-dialog [data-product-id="m5stack-c122-b"]').click();await assertCurrent('m5stack-c122-b');
    await page.getByRole('combobox',{name:'厂家',exact:true}).selectOption('LILYGO');
    await page.waitForFunction(()=>document.querySelector('#manufacturer-select').value==='LILYGO'&&document.querySelector('#stage-title').textContent.includes('LILYGO'));
    check((await current()).options.every(id=>!hidden.includes(id)),'manufacturer switch uses ordinary choices');
    check(errors.length===0,`browser errors: ${errors.join('; ')}`);
    return {status:'pass',checkedAt:new Date().toISOString(),browser:page.context().browser().version(),catalogCount:catalog.products.length,searchCount:visible.length,stampCount:stamp.length,restored,errors};
  } finally {page.off('pageerror',onError);page.off('console',onConsole);}
}
