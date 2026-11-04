async(page)=>{
  // Caller loads the independently built local fixture into this SKU first.
  const poll=async(label,predicate)=>{const end=Date.now()+60000;while(Date.now()<end){const state=await page.evaluate(()=>espTester.observe());state.serial=state.serialByPort.uart0||'';if(state.status==='error')throw Error(label+': '+JSON.stringify(state.lastError));if(predicate(state))return state;await page.waitForTimeout(50);}throw Error(label+' timed out');};
  await page.evaluate(async()=>{const s=await espTester.observe();if(s.scene.productId!=='m5stack-s015-dip')throw Error('Expected S015-DIP with local fixture');await espTester.reset();if((await espTester.observe()).status!=='running')await espTester.start();});
  let state=await poll('fresh fixture boot',s=>s.serial.includes('S3BAT_READY')&&s.serial.includes('S3BAT_BUTTON 0 FIRST 00 SECOND 00')&&JSON.stringify(s.leds)==='[[255,0,0]]');
  const receipt={productId:'m5stack-s015-dip',firmware:'internal ESP-IDF 5.5.1 stamps3bat fixture',before:{serialLength:state.serial.length,guestSeconds:state.stats.time,leds:state.leds},serialPort:'uart0',pollMethod:'Explicit awaited observe calls and host50ms sleep; each acceptance requires fresh guest serial and matching LED values in the same snapshot.',inputs:[],scope:'Actual workbench 3D PWR pointer hit and keyboard mapping, real WASM PM1 polling and guest RGB output; no factory or physical power claim.'};
  for(const [kind,step,color] of [['3d-pointer',1,[0,255,0]],['keyboard',2,[0,0,255]]]){
    const at=state.serial.length;
    if(kind==='3d-pointer'){await page.mouse.move(638,309);await page.mouse.down();}else await page.keyboard.down('KeyA');
    try{state=await poll(kind+' press',s=>s.serial.slice(at).includes('S3BAT_BUTTON 1 FIRST 81 SECOND 01')&&s.serial.slice(at).includes('S3BAT_OUTPUT '+step)&&JSON.stringify(s.leds)===JSON.stringify([color]));receipt.inputs.push({kind,coordinates:kind==='3d-pointer'?[638,309]:null,key:kind==='keyboard'?'KeyA':null,guestSeconds:state.stats.time,leds:state.leds,serial:state.serial.slice(at).split(/\r?\n/).filter(x=>/^S3BAT_(BUTTON|OUTPUT)/.test(x))});}
    finally{if(kind==='3d-pointer')await page.mouse.up();else await page.keyboard.up('KeyA');}
    const releaseAt=state.serial.length;state=await poll(kind+' release',s=>s.serial.slice(releaseAt).includes('S3BAT_BUTTON 0 FIRST 00 SECOND 00'));receipt.inputs.at(-1).release=state.serial.slice(releaseAt).split(/\r?\n/).filter(x=>/^S3BAT_BUTTON/.test(x));
    await page.locator('#device-stage').screenshot({path:'docs/platform/evidence/m5stack-s015-dip/workbench-'+(kind==='3d-pointer'?'pointer-green':'keyboard-blue')+'.png'});
  }
  await page.evaluate(async receipt=>{const digest=async b=>[...new Uint8Array(await crypto.subtle.digest('SHA-256',b))].map(x=>x.toString(16).padStart(2,'0')).join('');receipt.wasmSha256=await digest(await(await fetch('../wasm/esp32sim.wasm',{cache:'no-store'})).arrayBuffer());receipt.modelSha256=await digest(await(await fetch('./assets/m5stack-s015-dip/model.json',{cache:'no-store'})).arrayBuffer());receipt.checkedAt=new Date().toISOString();receipt.status='passed';window.poweredInputReceipt=receipt;await espTester.pause();},receipt);
}
