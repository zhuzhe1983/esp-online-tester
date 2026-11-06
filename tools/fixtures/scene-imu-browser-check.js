// Run with the task-owned Playwright wrapper. Serve the unmodified official
// image at the caller's ?firmware= URL; expected SHA stays pinned below.
async page => {
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.setViewportSize({width:1440,height:1050});
  await page.waitForFunction(()=>window.espTester,null,{timeout:60000});
  const firmware=new URL(page.url()).searchParams.get('firmware');if(!firmware)throw Error('Supply ?firmware= with the original CoreMatrix UIFlow image URL.');
  const setup=await page.evaluate(async firmware=>{
    const api=window.espTester,{loadCatalog,createDefaultScene}=await import('./catalog.js'),catalog=await loadCatalog();
    await api.configure(createDefaultScene(catalog,'m5stack-c158'));
    await api.loadFirmware({url:firmware,sha256:'8d5b8d4e22ebc293937086b2f9f218aa5a5ebda27b5ccb32b3f8f580ee02939b'});await api.start();
    const wait=async fn=>{const end=Date.now()+180000;while(Date.now()<end){const s=await api.observe();if(s.status==='error')throw Error(JSON.stringify(s.lastError));if(fn(s))return s;await new Promise(r=>setTimeout(r,25));}throw Error('Guest timeout');};
    const advance=async delta=>{const at=(await api.observe()).stats.time;return wait(s=>s.stats.time>=at+delta);};
    const send=data=>api.input({type:'serial',port:'usb',data});
    await wait(s=>s.serial.includes('MicroPython')&&s.stats.time>1);
    let start=(await api.observe()).serial.length;await send('\r\x03\x03');await advance(.15);await send('\x01');await wait(s=>s.serial.slice(start).includes('raw REPL; CTRL-B to exit\r\n>'));
    const exec=async source=>{const mark=(await api.observe()).serial.length;for(let i=0;i<source.length;i+=64){await send(source.slice(i,i+64));await advance(.005);}await send('\x04');const s=await wait(s=>s.serial.slice(mark).includes('\x04\x04>'));const output=s.serial.slice(mark);if(output.includes('Traceback'))throw Error(output);return output;};
    await exec('import M5,time,json\nfrom machine import I2C,Pin\nM5.begin()\nb=I2C(0,sda=Pin(0),scl=Pin(1),freq=400000)\nprint("SCENE_IMU_READY")\n');
    window.__imuProbe={advance,exec,async read(){const out=await exec('time.sleep_ms(30)\nprint("SCENE_IMU",json.dumps({"raw":list(b.readfrom_mem(0x68,0x0c,12)),"range":b.readfrom_mem(0x68,0x41,1)[0]&3}))\n');return JSON.parse(out.split('SCENE_IMU ')[1].split('\r\n')[0]);}};
    const wasm=await (await fetch('../wasm/esp32sim.wasm')).arrayBuffer();
    return {capabilities:(await api.capabilities()).imu,wasmSha256:[...new Uint8Array(await crypto.subtle.digest('SHA-256',wasm))].map(v=>v.toString(16).padStart(2,'0')).join('')};
  },firmware);
  const control=page.locator('#imu-scene-link');if(await control.getAttribute('aria-pressed')!=='false')throw Error('Link must start off');
  await page.locator('#view-3d').click();
  const drag=async(dx,dy)=>{const r=await page.locator('#scene-mount canvas:not(.flat-display)').boundingBox();await page.mouse.move(r.x+20,r.y+30);await page.mouse.down();await page.mouse.move(r.x+20+dx,r.y+30+dy,{steps:14});await page.mouse.up();await page.evaluate(()=>window.__imuProbe.advance(.15));};
  const read=()=>page.evaluate(()=>window.__imuProbe.read());
  const before=await read();await drag(75,55);const disabled=await read();
  if(JSON.stringify(before)!==JSON.stringify(disabled))throw Error('Off drag changed hardware registers');
  await control.click();await page.waitForFunction(()=>document.querySelector('#imu-scene-link').getAttribute('aria-pressed')==='true');
  const linkedBefore=await read();await drag(-95,65);const linked=await read();
  const input=await page.evaluate(async()=>(await window.espTester.observe()).imu);
  const signed=(lo,hi)=>((hi<<8)|lo)<<16>>16, scale=32768/(2<<linked.range);
  const actual=linked.raw.slice(0,6).reduce((v,_,i,a)=>(i%2===0&&v.push(signed(a[i],a[i+1])/scale),v),[]);
  if(actual.some((v,i)=>Math.abs(v-input.accelG[i])>1/scale))throw Error('Guest BMI acceleration differs from the scene input');
  if(JSON.stringify(linkedBefore.raw.slice(0,6))===JSON.stringify(linked.raw.slice(0,6)))throw Error('On drag did not change actual BMI registers');
  if(linked.raw.slice(6).some(v=>v!==0)||input.gyroDps.some(v=>v!==0))throw Error('Pointer release did not clear angular velocity');
  await page.screenshot({path:'output/playwright/scene-imu-linked.png',fullPage:true});
  await control.click();const off=await read();await drag(85,-50);const frozen=await read();if(JSON.stringify(off)!==JSON.stringify(frozen))throw Error('Turning off failed to freeze IMU');
  await control.click();await page.evaluate(()=>window.espTester.setImu({quaternion:[0,0,0,1],angularVelocityDps:[12,0,0]}));
  const agent=await page.evaluate(async()=>(await window.espTester.observe()).imu);
  if(await control.getAttribute('aria-pressed')!=='false'||agent.gyroDps[0]!==12)throw Error('Agent input lost ownership or explicit gyro');
  await control.click();await page.evaluate(()=>window.espTester.pause());
  if(await control.getAttribute('aria-pressed')!=='false'||!await control.isDisabled())throw Error('Pause did not disarm link');
  await page.evaluate(()=>window.espTester.start());await control.click();await page.locator('#view-2d').click();
  if(await control.getAttribute('aria-pressed')!=='false'||!await control.isDisabled())throw Error('2D did not disarm link');
  await page.locator('#view-3d').click();await control.click();await page.evaluate(()=>window.espTester.reset());
  if(await control.getAttribute('aria-pressed')!=='false')throw Error('Rebuild retained link');
  await page.evaluate(()=>window.espTester.pause());
  // A fresh embed page must not inherit the local toolbar choice.
  const embed=await page.context().newPage();
  try {await embed.goto(new URL('embed.html?lang=en',page.url()).href);await embed.waitForFunction(()=>window.espTester);await embed.evaluate(async()=>{const {loadCatalog,createDefaultScene}=await import('./catalog.js');await window.espTester.configure(createDefaultScene(await loadCatalog(),'m5stack-c158'));});
    if(await embed.locator('#imu-scene-link').getAttribute('aria-pressed')!=='false')throw Error('Embed inherited link');
    await embed.setViewportSize({width:390,height:844});await embed.screenshot({path:'output/playwright/scene-imu-embed-mobile.png',fullPage:true});
    const overflow=await embed.evaluate(()=>document.documentElement.scrollWidth>innerWidth);if(overflow)throw Error('Mobile toolbar overflow');
    await embed.evaluate(async()=>{const {loadCatalog,createDefaultScene}=await import('./catalog.js');const s=createDefaultScene(await loadCatalog(),'m5stack-k138');s.flight={enabled:true,model:'stampfly-rigid-body-v1'};await window.espTester.configure(s);});
    if(!await embed.locator('#imu-scene-link').isDisabled()||!/flight/i.test(await embed.locator('#imu-scene-link').getAttribute('title')))throw Error('Flight ownership needs a disabled control and explanation');
    const shared=await page.evaluate(async()=>{const {encodeScene}=await import('./catalog.js');return encodeScene((await window.espTester.observe()).scene);});
    const iframeUrl=new URL('embed.html',page.url());iframeUrl.hash=new URLSearchParams({scene:shared,autostart:'0'}).toString();
    await embed.goto(new URL('../',page.url()).href);await embed.setContent('<iframe title="Shared device" style="width:800px;height:700px"></iframe>');
    await embed.locator('iframe').evaluate((node,src)=>{node.src=src;},iframeUrl.href);
    const frame=embed.frameLocator('iframe');await frame.locator('#imu-scene-link').waitFor({state:'visible',timeout:60000});
    if(!(await frame.locator('#stage-title').textContent()).includes('CoreMatrix'))throw Error('Shared iframe restored the wrong device');
    if(await frame.locator('#imu-scene-link').getAttribute('aria-pressed')!=='false')throw Error('Shared iframe retained the local IMU choice');
  }finally{await embed.close();}
  if(errors.length)throw Error(errors.join('\n'));
  return {firmwareSha256:'8d5b8d4e22ebc293937086b2f9f218aa5a5ebda27b5ccb32b3f8f580ee02939b',wasmSha256:setup.wasmSha256,capabilities:setup.capabilities,before,disabled,linkedBefore,linked,hostInput:input,guestAccelG:actual,off,frozen,pauseDisarms:true,twoDDisarms:true,rebuildDisarms:true,embedStartsOff:true,sharedIframeStartsOff:true,agentOwnsExplicitGyro:true,flightDisablesControl:true,pageErrors:errors};
}
