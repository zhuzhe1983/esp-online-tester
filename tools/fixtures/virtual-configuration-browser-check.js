// Original firmware URL is caller-supplied in ?firmware=. Run in a task-owned browser.
async page => {
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  const original=new URL(page.url()),firmware=original.searchParams.get('firmware');
  if(!firmware)throw Error('Supply ?firmware= with the original CoreMatrix UIFlow image.');
  await page.reload();await page.setViewportSize({width:1440,height:1050});
  const setup=async(clear=false)=>page.evaluate(async({firmware,clear})=>{
    const api=window.espTester,{loadCatalog,createDefaultScene}=await import('./catalog.js');
    await api.configure(createDefaultScene(await loadCatalog(),'m5stack-c158'));
    await api.loadFirmware({url:firmware,sha256:'8d5b8d4e22ebc293937086b2f9f218aa5a5ebda27b5ccb32b3f8f580ee02939b'});
    if(clear)await api.clearDeviceStorage();
    const wait=async fn=>{const end=Date.now()+150000;while(Date.now()<end){const s=await api.observe();if(s.status==='error')throw Error(JSON.stringify(s.lastError));if(fn(s))return s;await new Promise(r=>setTimeout(r,20));}throw Error('Guest timeout');};
    const advance=async delta=>{const at=(await api.observe()).stats.time;return wait(s=>s.stats.time>=at+delta);};
    const send=data=>api.input({type:'serial',port:'usb',data});
    window.__configProbe={wait,async read(){let start=(await api.observe()).serial.length;await send('\r\x03\x03');await advance(.15);await send('\x01');await wait(s=>s.serial.slice(start).includes('raw REPL; CTRL-B to exit\r\n>'));
      const source=`import esp32,network,hashlib,binascii,json,time\nn=esp32.NVS('uiflow')\nassert n.get_str('ssid0')=='esp32_online_tester'\nassert n.get_str('pswd0')=='rilakkuma_cool'\nassert n.get_u8('boot_option')==2\nassert n.get_str('net_mode')=='WIFI'\nassert n.get_str('protocol')=='DHCP'\nassert binascii.hexlify(hashlib.sha256(open('/flash/boot.py','rb').read()).digest()).decode()=='dfc76de173d2afee2bc5a94a293346b898a76e5ee6e544f6f7f3edac14657efb'\nw=network.WLAN(network.STA_IF)\nfor _ in range(100):\n if w.isconnected(): break\n time.sleep_ms(100)\nassert w.isconnected()\nprint('VIRTUAL_CONFIG',json.dumps({'connected':w.isconnected(),'ip':w.ifconfig()[0],'bootOption':n.get_u8('boot_option')}))\n`;
      start=(await api.observe()).serial.length;for(let i=0;i<source.length;i+=64){await send(source.slice(i,i+64));await advance(.005);}await send('\x04');
      const s=await wait(s=>/\x04[^\x04]*\x04>/.test(s.serial.slice(start))),out=s.serial.slice(start);if(out.includes('Traceback'))throw Error(out);return JSON.parse(out.split('VIRTUAL_CONFIG ')[1].split('\r\n')[0]);
    }};
    await api.start();return (await api.observe()).firmware.artifacts;
  },{firmware,clear});
  // Deliberate test-only erase of this task browser's isolated original-image scope.
  await page.waitForFunction(()=>!!window.espTester);const artifacts=await setup(true);
  await page.evaluate(()=>__configProbe.wait(s=>s.serial.includes('>>>')&&s.stats.time>1));
  if(await page.evaluate(async()=>!!(await espTester.observe()).firmwareConfiguration))throw Error('Configuration was written automatically');
  const section=page.locator('.virtual-configuration');await section.locator('summary').click();
  const write=section.getByRole('button',{name:'写入并重启',exact:true});
  await page.waitForFunction(()=>!document.querySelector('.virtual-configuration [data-write]').disabled);
  if(await section.locator('select').inputValue()!=='builtin-uiflow2-simulator')throw Error('Missing built-in profile');
  await write.click();
  const boot=await page.evaluate(async()=>{const s=await __configProbe.wait(s=>s.serial.includes('WiFi initialized')&&s.serial.includes('>>>')&&s.stats.time>1);return {serial:s.serial,receipt:s.firmwareConfiguration,artifacts:s.firmware.artifacts,router:s.network.router};});
  if(boot.serial.includes("no module named 'M5Things'")||boot.serial.includes('Traceback'))throw Error('Network-only startup failed');
  if(JSON.stringify(artifacts)!==JSON.stringify(boot.artifacts))throw Error('Original source identity changed');
  if(!boot.receipt||JSON.stringify(boot.receipt).includes('rilakkuma'))throw Error('Missing or credential-bearing receipt');
  const guest=await page.evaluate(()=>__configProbe.read());if(guest.ip!=='10.0.2.15')throw Error('Guest did not obtain virtual DHCP address');
  if(!(await section.locator('[data-result]').innerText()).includes('配置已写入'))throw Error('Missing UI write result');
  await page.evaluate(()=>scrollTo(0,0));await page.screenshot({path:'output/playwright/virtual-config-written.png',fullPage:true});
  await page.evaluate(()=>espTester.reset());
  await page.evaluate(()=>__configProbe.wait(s=>s.serial.includes('WiFi initialized')&&s.serial.includes('>>>')));
  const reset=await page.evaluate(()=>__configProbe.read());await page.evaluate(()=>espTester.pause());
  await page.reload();await page.waitForFunction(()=>!!window.espTester);await setup();
  await page.evaluate(()=>__configProbe.wait(s=>s.serial.includes('WiFi initialized')&&s.serial.includes('>>>')));
  const reload=await page.evaluate(()=>__configProbe.read());await page.evaluate(()=>espTester.pause());
  await page.setViewportSize({width:390,height:844});await page.locator('.virtual-configuration summary').click();
  await page.screenshot({path:'output/playwright/virtual-config-mobile.png',fullPage:true});
  if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth))throw Error('Mobile overflow');
  const embedded=await page.context().newPage();try{
    await embedded.goto(new URL('embed.html?lang=en',original).href);await embedded.waitForFunction(()=>!!window.espTester);
    const panel=embedded.locator('.virtual-configuration');await panel.locator('summary').click();
    if(!await panel.locator('[data-write]').isDisabled())throw Error('Unloaded embed allows write');
    if((await panel.innerText()).includes('[object Object]'))throw Error('Unusable error rendering');
  }finally{await embedded.close();}
  if(errors.length)throw Error(errors.join('\n'));
  return {guest,reset,reload,defaultWrite:false,uiWrite:true,originalSourceUnchanged:true,originalBootScriptUnchanged:true,credentialFreeReceipt:true,embedUnloadedDisabled:true,mobileOverflow:false,pageErrors:errors};
}
