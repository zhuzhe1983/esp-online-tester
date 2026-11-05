// Caller opens the workbench and selects the hash-pinned K141 official merged BIN
// in #firmware-file. This check runs it in WASM; it does not flash hardware.
async page => {
 const errors=[];page.on('pageerror',error=>errors.push(error.message));
 await page.waitForFunction(()=>window.espTester);await page.setViewportSize({width:1440,height:1100});
 const source=await page.evaluate(async()=>{
  const file=document.querySelector('#firmware-file')?.files[0];if(!file)throw Error('Select the fixed K141 official merged BIN in #firmware-file first');
  const hash=async bytes=>[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(b=>b.toString(16).padStart(2,'0')).join('');
  const sha256=await hash(await file.arrayBuffer());if(sha256!=='abd1a4345353ce0c166fbed6b89c3feb7d65ca2a5f37a05a34d1271405aabf2e'||file.size!==5619252)throw Error('Unexpected official firmware bytes');
  const {loadCatalog,createDefaultScene}=await import(new URL('./catalog.js',location.href).href),catalog=await loadCatalog(),scene=createDefaultScene(catalog,'m5stack-k141');scene.firmware=null;await espTester.configure(scene);await espTester.loadFirmware({format:'merged',file,sha256});await espTester.start();
  return {url:'https://m5burner-cdn.m5stack.com/firmware/714fd486771a7a4aec72e54cb6031570.bin',sha256,bytes:file.size};
 });
 try{
  const end=Date.now()+120000;let state;
  while(Date.now()<end){state=await page.evaluate(()=>espTester.observe());if(state.status==='error')throw Error(JSON.stringify(state.lastError));if(state.stats.time>=10&&(state.serialByPort.uart0||'').includes('Dashboard onResume'))break;await page.waitForTimeout(100);}
  if(!state||(state.serialByPort.uart0||'').indexOf('Dashboard onResume')<0)throw Error('Official Dashboard did not render');
  await page.evaluate(()=>espTester.pause());await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));await page.waitForTimeout(100);
  const frame=await page.evaluate(async()=>{const f=await espTester.capture({format:'rgba'}),pixels=new Uint8Array(f.pixels),colors=new Set();for(let i=0;i<pixels.length;i+=4)colors.add(pixels.slice(i,i+3).join(','));return {width:f.width,height:f.height,colors:colors.size,sha256:[...new Uint8Array(await crypto.subtle.digest('SHA-256',pixels))].map(b=>b.toString(16).padStart(2,'0')).join('')};});
  if(frame.width!==240||frame.height!==135||frame.colors<4)throw Error('Expected actual nonblank 240x135 framebuffer');
  await page.locator('[data-model-status="ready"]').waitFor();await page.locator('#view-3d').click();
  const canvas=page.locator('#device-stage canvas').filter({visible:true}).first(),box=await canvas.boundingBox();
  if(!box)throw Error('Missing visible model canvas');await page.mouse.move(box.x+box.width*.5,box.y+box.height*.5);await page.mouse.wheel(0,-350);await page.waitForTimeout(100);await page.mouse.move(box.x+4,box.y+4);
  await page.locator('#device-stage').screenshot({path:'output/playwright/stamplc-screen-workbench-default.png'});
  // Rotate by dragging empty viewer space: framebuffer and physical opening must
  // remain together under the production viewer's own camera controls.
  await page.mouse.move(box.x+box.width*.85,box.y+box.height*.55);await page.mouse.down();await page.mouse.move(box.x+box.width*.85+70,box.y+box.height*.55+30,{steps:10});await page.mouse.up();await page.mouse.move(box.x+4,box.y+4);await page.waitForTimeout(100);
  await page.locator('#device-stage').screenshot({path:'output/playwright/stamplc-screen-workbench-oblique.png'});
  await page.locator('#view-2d').click();await page.locator('#device-stage').screenshot({path:'output/playwright/stamplc-screen-workbench-2d.png'});await page.locator('#view-3d').click();
  if(errors.length)throw Error(errors.join('\n'));
  return {productId:'m5stack-k141',source,guestSeconds:state.stats.time,guestEvidence:'Dashboard onResume',frame,pageErrors:errors,scope:'Fixed official firmware Dashboard framebuffer observed in actual workbench 3D and 2D. Screen position repair only; not whole firmware or hardware compatibility.'};
 }finally{await page.evaluate(()=>espTester.pause());}
}
