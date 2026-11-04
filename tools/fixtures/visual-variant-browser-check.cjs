// Via tracked Playwright CLI run-code, on the workbench. Caller must first set
// window.visualVariantTestFirmware={url,sha256} to a locally served source-built
// tools/fixtures/stamps3-firmware merged image; never use manufacturer firmware as this fixture.
async (page) => {
  const check=(value,message)=>{if(!value)throw Error(message);};
  const source=await page.evaluate(()=>window.visualVariantTestFirmware);
  check(source?.url&&/^[a-f0-9]{64}$/.test(source.sha256),'Supply visualVariantTestFirmware URL + SHA-256');
  const errors=[],onError=e=>errors.push(e.message),onConsole=e=>{if(e.type()==='error')errors.push(e.text());};page.on('pageerror',onError);page.on('console',onConsole);
  const ready=()=>page.waitForFunction(()=>window.espTester&&!document.querySelector('#product-select').disabled);
  const modelReady=()=>page.waitForFunction(()=>document.querySelector('#scene-mount').dataset.modelStatus==='ready');
  const observe=()=>page.evaluate(async()=>{const s=await espTester.observe(),mount=document.querySelector('#scene-mount');return {scene:s.scene,product:s.product,status:s.status,leds:s.leds,motors:s.motors,accessoryInputs:s.accessoryInputs,accessoryOutputs:s.accessoryOutputs,serial:s.serial,visualId:mount.dataset.visualProductId,preview:mount.dataset.previewOnly,menuHidden:document.querySelector('#visual-variant-menu').hidden,menuDisabled:document.querySelector('#visual-variant-select').disabled,selected:document.querySelector('#visual-variant-select').value};});
  const waitLed=expected=>page.evaluate(async expected=>{const until=performance.now()+15000;while(performance.now()<until){const s=await espTester.observe();if(JSON.stringify(s.leds[0])===JSON.stringify(expected))return;if(s.status==='error')throw Error(s.lastError?.message);await new Promise(resolve=>setTimeout(resolve,30));}throw Error(`LED output not observed: ${expected}`);},expected);
  const configure=async(id,firmware=null)=>{await page.evaluate(async({id,firmware})=>{const {loadCatalog,createDefaultScene}=await import('./catalog.js');const scene=createDefaultScene(await loadCatalog(),id);scene.firmware=firmware;await espTester.configure(scene);},{id,firmware});await modelReady();};
  const choose=async id=>{const summary=page.locator('#visual-variant-menu summary');if(await page.locator('#visual-variant-menu').getAttribute('open')===null)await summary.click();await page.getByRole('combobox',{name:'衍生模型',exact:true}).selectOption(id);await modelReady();};
  const base=(s,id)=>check(s.scene.productId===id&&s.visualId===id&&s.preview==='false'&&s.selected===id,`base model restored: ${id}`);
  try {
    await ready();
    const catalog=await page.evaluate(async()=>{const {loadCatalog}=await import('./catalog.js');return loadCatalog();});
    const variants=catalog.products.filter(p=>p.visualVariantOf),previews=[];
    for(const variant of variants){
      await configure(variant.visualVariantOf);const before=await observe();await choose(variant.id);const after=await observe();
      check(after.scene.productId===variant.visualVariantOf&&after.visualId===variant.id&&after.preview==='true','variant is a renderer-only product');
      for(const key of ['scene','product','status','leds','motors','accessoryInputs','accessoryOutputs'])check(JSON.stringify(before[key])===JSON.stringify(after[key]),`preview leaves ${key} unchanged: ${variant.id}`);
      const share=await page.evaluate(()=>espTester.share({mode:'studio',autostart:false}));
      const encoded=new URL(share).hash.slice(1);check(new URLSearchParams(encoded).has('scene'),'share remains a scene');
      const sharedId=await page.evaluate(async url=>{const {decodeScene}=await import('./catalog.js');return decodeScene(new URLSearchParams(new URL(url).hash.slice(1)).get('scene')).productId;},share);
      check(sharedId===variant.visualVariantOf,'share never records preview as runtime product');
      previews.push({parent:variant.visualVariantOf,variant:variant.id,unchangedRuntime:true,modelReady:true,shareProductId:sharedId});
    }
    // A legacy scene is still the exact SKU, and does not become a parent alias.
    await configure('m5stack-s015-dip');let state=await observe();base(state,'m5stack-s015-dip');check(state.menuHidden,'legacy variant has no parent menu remapping');
    const legacyShare=await page.evaluate(()=>espTester.share({mode:'studio',autostart:false}));await page.goto(legacyShare);await ready();await modelReady();base(await observe(),'m5stack-s015-dip');
    // Human product changes and mode changes reset the temporary visual selection.
    await configure('m5stack-s007');await choose('m5stack-s007-pin254');
    await page.getByRole('combobox',{name:/^具体型号/}).selectOption('m5stack-c050-b');await modelReady();base(await observe(),'m5stack-c050-b');
    await choose('m5stack-k051-b');await page.getByRole('button',{name:'2D',exact:true}).click();state=await observe();check(state.preview==='false'&&state.menuHidden,'2D restores runtime model');await page.getByRole('button',{name:'3D',exact:true}).click();
    // Real guest execution: loading, human start, API resume, API reset, human reset.
    await configure('m5stack-s007',source);await choose('m5stack-s007-pin127');
    await page.evaluate(()=>espTester.loadFirmware());base(await observe(),'m5stack-s007');
    await choose('m5stack-s007-pin127');await page.locator('#start-runtime').click();
    if(await page.locator('.firmware-run-confirm').isVisible())await page.getByRole('button',{name:'继续运行',exact:true}).click();
    await page.evaluate(()=>espTester.waitFor({status:'running',serialIncludes:'STAMPS3_BUTTON 1',timeoutMs:30000}));await waitLed([255,0,0]);state=await observe();base(state,'m5stack-s007');check(state.menuDisabled,'running locks visual menu');check(state.leds[0]?.join(',')==='255,0,0','real RMT red after human start');
    const guest=[];guest.push({action:'human-start',status:state.status,led:state.leds[0]});
    await page.evaluate(()=>espTester.pause());state=await observe();base(state,'m5stack-s007');check(!state.menuDisabled,'pause unlocks without restoring old preview');
    await choose('m5stack-s007-pin254');const paused=await observe();
    await page.evaluate(()=>{window.variantInputCount=0;window.variantStopInputs=espTester.subscribe(event=>{if(event.type==='input')window.variantInputCount++;});});
    if(await page.locator('#visual-variant-menu').getAttribute('open')!==null)await page.locator('#visual-variant-menu summary').click();
    const deviceButton=page.locator('#device-buttons [data-control="button"]');check(await deviceButton.isDisabled(),'preview bottom control is disabled');
    const buttonBox=await deviceButton.boundingBox();await page.mouse.click(buttonBox.x+buttonBox.width/2,buttonBox.y+buttonBox.height/2);
    const box=await page.locator('#scene-mount canvas[data-engine]').boundingBox();await page.mouse.click(box.x+box.width/2,box.y+box.height/2);await page.keyboard.press('a');check(await page.evaluate(()=>window.variantInputCount)===0,'preview pointer, bottom button and mapped key send no runtime input');
    await page.evaluate(()=>espTester.start());state=await observe();base(state,'m5stack-s007');check(state.leds[0]?.join(',')===paused.leds[0]?.join(','),'resume preserves guest LED state');
    await page.evaluate(async()=>{const fresh=espTester.waitFor({serialIncludes:'STAMPS3_RMT 0 255 0',fromNow:true,timeoutMs:15000});await espTester.press('button',{durationMs:180});await fresh;});await waitLed([0,255,0]);state=await observe();guest.push({action:'api-start-and-button',status:state.status,led:state.leds[0]});check(state.leds[0]?.join(',')==='0,255,0','real GPIO press drives RMT after model restoration');
    await page.evaluate(()=>espTester.pause());await choose('m5stack-s007-pin127');await page.evaluate(()=>espTester.reset());await page.evaluate(()=>espTester.waitFor({serialIncludes:'STAMPS3_BUTTON 1',timeoutMs:30000}));await waitLed([255,0,0]);state=await observe();base(state,'m5stack-s007');guest.push({action:'api-reset',status:state.status,led:state.leds[0]});check(state.leds[0]?.join(',')==='255,0,0','API reset returns guest initial output');
    await page.evaluate(()=>espTester.pause());await choose('m5stack-s007-pin254');await page.locator('#reset-runtime').click();await page.evaluate(()=>espTester.waitFor({status:'running',serialIncludes:'STAMPS3_BUTTON 1',timeoutMs:30000}));await waitLed([255,0,0]);state=await observe();base(state,'m5stack-s007');guest.push({action:'human-reset',status:state.status,led:state.leds[0]});
    await page.evaluate(()=>espTester.pause());check(errors.length===0,errors.join('; '));
    return {status:'pass',checkedAt:new Date().toISOString(),browser:page.context().browser().version(),fixtureSha256:source.sha256,previews,legacyShare:true,productSwitch:true,modeSwitch:true,previewHumanInputs:0,bottomControlDisabled:true,guest,errors};
  } finally {
    await page.evaluate(async()=>{window.variantStopInputs?.();if((await espTester.observe()).status==='running')await espTester.pause();}).catch(()=>{});
    page.off('pageerror',onError);page.off('console',onConsole);
  }
}
