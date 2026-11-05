// Real public UserDemo bytes, executed in WASM with TriviaPOD's 32 MB profile.
// Runs on either the local or published /platform/ workbench; no hardware writes.
async page => {
 const errors=[],assert=(ok,message)=>{if(!ok)throw Error(message);},root=new URL('./',page.url());
 page.on('pageerror',error=>errors.push(error.message));
 await page.evaluate(()=>localStorage.setItem('esp-online-tester.locale','zh-CN'));
 await page.setViewportSize({width:1440,height:1100});await page.goto(new URL('?lang=zh-CN',root).href);
 await page.waitForFunction(()=>window.espTester&&!document.querySelector('#product-select')?.disabled);
 const waitState=async predicate=>{const end=Date.now()+120000;while(Date.now()<end){const state=await page.evaluate(()=>window.espTester?.observe());if(state?.status==='error')throw Error(JSON.stringify(state.lastError));if(state&&predicate(state))return state;await page.waitForTimeout(100);}throw Error('Timed out waiting for the configured firmware state');};
 await page.locator('#product-select').selectOption('m5stack-triviapod');await waitState(state=>state.product?.id==='m5stack-triviapod');
 assert(!/尚未适配/.test(await page.locator('#product-select option:checked').innerText()),'TriviaPOD is a runnable product');
 assert(await page.locator('#firmware-select').inputValue()==='m5stack-stopwatch-official','Shared UserDemo is selected first');
 const firmwareIds=await page.locator('#firmware-select option').evaluateAll(options=>options.map(option=>option.value));
 assert(['m5stack-stopwatch-official','m5stack-stopwatch-uiflow2','m5stack-stopwatch-xiaozhi'].every(id=>firmwareIds.includes(id)),'Shared firmware library is complete');
 assert(await page.locator('#load-firmware').isEnabled(),'Shared firmware can run');
 assert(await page.locator('#accessory-interface').inputValue()==='USB-C','Accessory filter uses TriviaPOD connectors');
 assert((await page.locator('#accessory-interface option').allTextContents()).every(value=>!value.includes('HY2.0')&&!value.includes('pin-header')),'No inherited expansion connector');
 await page.locator('#load-firmware').click();await page.getByRole('button',{name:'继续运行',exact:true}).click();
 let state;
 try {
  state=await waitState(state=>state.status==='running'&&state.stats.time>=6&&state.serial.includes('[Launcher] on open')&&state.frame?.frameNumber>0);
  const artifact=state.firmware.artifacts.find(item=>item.kind==='flash');
  assert(artifact.sha256==='6395799089341e0ae96072c2a3418d469d6797b2ab8ee0cab1c9ed24a42acbbe','Exact official V0.5 image was executed');
  assert(state.product.runtime.flashMb===32&&state.storage.scope.flashBytes===33554432,'Runtime and persistent flash stay 32 MB');
  assert(state.storage.scope.productId==='m5stack-triviapod','Writable state remains isolated from StopWatch');
  const pressed=await page.evaluate(async()=>{await espTester.input({type:'button',id:'button-a',pressed:true});return espTester.input({type:'button',id:'button-b',pressed:true});});
  try {await waitState(state=>state.stats.time>=pressed.guestTime+1.5);}
  finally {await page.evaluate(async()=>{await espTester.input({type:'button',id:'button-a',pressed:false});await espTester.input({type:'button',id:'button-b',pressed:false});});}
  await waitState(state=>state.stats.time>=pressed.guestTime+3);await page.evaluate(()=>espTester.pause());
  await page.locator('#scene-mount[data-model-status="ready"]').waitFor();await page.locator('#view-3d').click();await page.waitForTimeout(100);
  const frame=await page.evaluate(async()=>{const f=await espTester.capture({format:'rgba'}),pixels=new Uint8Array(f.pixels),colors=new Set();for(let i=0;i<pixels.length;i+=4)colors.add(pixels.slice(i,i+3).join(','));return {width:f.width,height:f.height,colors:colors.size,sha256:[...new Uint8Array(await crypto.subtle.digest('SHA-256',pixels))].map(b=>b.toString(16).padStart(2,'0')).join('')};});
  assert(frame.width===468&&frame.height===466&&frame.colors>16,'Actual firmware pixels reach the workbench');
  await page.locator('#device-stage').screenshot({path:'output/playwright/triviapod-official-3d.png'});
  await page.locator('#view-2d').click();await page.locator('#device-stage').screenshot({path:'output/playwright/triviapod-official-2d.png'});
  const share=await page.evaluate(()=>espTester.share({mode:'embed',autostart:false,autoplay:false}));await page.goto(share);
  await page.waitForFunction(()=>window.espTester);await waitState(state=>state.product?.id==='m5stack-triviapod');
  const restored=await page.evaluate(()=>espTester.observe());assert(restored.product.runtime.flashMb===32&&restored.scene.firmware.url===artifact.url,'Mini preserves the real product and shared source');
  assert(await page.locator('#start-runtime').isEnabled(),'Mini offers the same firmware run action');
  assert(!errors.length,errors.join('\n'));
  return {productId:state.product.id,board:state.product.runtime.board,flashMb:32,psramMb:8,storageScope:state.storage.scope,firmware:artifact,firmwareIds,frame,mini:true,physicalExpansion:false,hardwareWrites:0,pageErrors:errors};
 } finally {await page.evaluate(async()=>{if((await window.espTester?.observe())?.status==='running')await window.espTester.pause();});}
}
