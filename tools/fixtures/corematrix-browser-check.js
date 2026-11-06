// Run through the repository Playwright wrapper on the workbench or embed page.
// Supply ?test-relay=http://127.0.0.1:<task-owned firmware relay port>.
// The relay streams the catalog-pinned original image; no test firmware replaces it.
async page => {
  const hostPage=page,child=page.frames().find(frame=>frame.parentFrame()&&frame.url().includes('/platform/embed.html'));
  if(child)page=child;
  const relay=new URL(page.url()).searchParams.get('test-relay');
  if(!relay)throw Error('Supply the task-owned firmware relay with ?test-relay=');
  const errors=[];hostPage.on('pageerror',error=>errors.push(error.message));
  await page.waitForFunction(()=>window.espTester,null,{timeout:60000});
  const surface=child?'iframe':new URL(page.url()).pathname.endsWith('embed.html')?'embed':'workbench';
  await hostPage.setViewportSize({width:1440,height:1100});
  const cdp=await hostPage.context().newCDPSession(hostPage);await cdp.send('Network.setCacheDisabled',{cacheDisabled:true});
  const receipt=await page.evaluate(async relay=>{
    const {loadCatalog,createDefaultScene}=await import('./catalog.js');
    const {configureFirmwareRelay}=await import('./firmware-relay.js');configureFirmwareRelay(relay);
    const wasmBytes=await (await fetch('../wasm/esp32sim.wasm',{cache:'no-store'})).arrayBuffer();
    const wasmSha256=[...new Uint8Array(await crypto.subtle.digest('SHA-256',wasmBytes))].map(x=>x.toString(16).padStart(2,'0')).join('');
    const api=window.espTester,catalog=await loadCatalog(),firmware=catalog.firmwares.find(f=>f.id==='m5stack-c158-uiflow2-github');
    if(location.hash.includes('scene=')){const shared=await api.observe();if(shared.product?.id!=='m5stack-c158'||shared.scene?.firmware?.sha256!==firmware.source.sha256)throw Error('Shared CoreMatrix source was not restored');}
    else await api.configure(createDefaultScene(catalog,'m5stack-c158'));
    await api.loadFirmware(firmware.source);await api.start();
    const wait=async predicate=>{const end=Date.now()+180000;while(Date.now()<end){const s=await api.observe();if(s.status==='error')throw Error(JSON.stringify(s.lastError));if(predicate(s))return s;await new Promise(resolve=>setTimeout(resolve,30));}throw Error('CoreMatrix browser guest timeout');};
    const advance=async delta=>{const at=(await api.observe()).stats.time;await wait(s=>s.stats.time>=at+delta);};
    const send=data=>api.input({type:'serial',port:'usb',data});
    await wait(s=>s.serial.includes('MicroPython')&&s.stats.time>1);
    const startup=(await api.observe()).serial.includes("no module named 'M5Things'");
    let start=(await api.observe()).serial.length;await send('\r\x03\x03');await advance(.15);await send('\x01');await wait(s=>s.serial.slice(start).includes('raw REPL; CTRL-B to exit\r\n>'));
    const source='import M5\nM5.begin()\nM5.Display.setRotation(0)\nM5.Display.setBrightness(255)\nM5.Display.clear(0)\nfor x,y in [(0,0),(3,2),(4,3),(7,4),(8,5),(11,6),(12,7),(15,15)]:\n M5.Display.drawPixel(x,y,0xffffff)\nprint("CM_BROWSER",M5.getBoard(),M5.Display.width(),M5.Display.height())\n';
    start=(await api.observe()).serial.length;
    for(let i=0;i<source.length;i+=64){await send(source.slice(i,i+64));await advance(.01);}await send('\x04');
    await wait(s=>s.serial.slice(start).includes('\x04\x04>'));const output=(await api.observe()).serial.slice(start);
    if(!output.includes('CM_BROWSER 152 16 16')||output.includes('Traceback'))throw Error(output);
    const expected=[0,35,52,71,88,107,124,255];
    const state=await wait(s=>s.leds.length===256&&s.leds.filter(rgb=>rgb.some(v=>v>0)).length===8);
    const lit=state.leds.flatMap((rgb,i)=>rgb.some(v=>v>0)?[i]:[]);
    if(JSON.stringify(lit)!==JSON.stringify(expected)||state.leds.some(rgb=>rgb[0]!==rgb[1]||rgb[0]!==rgb[2]))throw Error('Matrix position/white channel mismatch');
    const frame=await api.capture({format:'rgba'}),bytes=new Uint8Array(frame.pixels);
    if(frame.width!==16||frame.height!==16)throw Error('Incorrect framebuffer size');
    const frameHash=[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(x=>x.toString(16).padStart(2,'0')).join('');
    await api.pause();
    return {wasmSha256,productId:state.product.id,firmwareId:firmware.id,sha256:firmware.source.sha256,source:state.firmware,firmwarePatched:false,guestProgram:'RAM-only USB raw REPL',startupMissingM5Things:startup,guestSeconds:state.stats.time,litIndices:lit,frame:{width:frame.width,height:frame.height,sha256:frameHash},hardwareDiagnostics:state.hardwareDiagnostics,capabilities:await api.capabilities(),share:await api.share({mode:'embed',autostart:false})};
  },relay);
  await page.locator('#view-3d').click();await hostPage.screenshot({path:`output/playwright/corematrix-${surface}-3d.png`,fullPage:true});
  await page.locator('#view-2d').click();
  const flat=await page.locator('.flat-display').boundingBox();if(!flat||flat.width<200||Math.abs(flat.width-flat.height)>2)throw Error('16×16 matrix preview must be legible and square');
  await hostPage.screenshot({path:`output/playwright/corematrix-${surface}-2d.png`,fullPage:true});
  await page.locator('#view-3d').click();
  if(errors.length)throw Error(errors.join('\n'));await cdp.detach();
  return {...receipt,surface,flatDisplay:flat,pageErrors:errors};
}
