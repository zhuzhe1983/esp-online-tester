// Run against a task-owned local workbench with playwright_cli.sh run-code --filename.
// The port chooser is replaced with cancellation; no physical hardware is touched.
async page => {
  const assert=(condition,message)=>{if(!condition)throw Error(message);};
  const root=new URL('./',page.url()).href,results=[];
  // Repeated CLI runs must exercise the current static files, not the HTTP cache.
  await page.route('**/platform/**',route=>route.continue());
  const fail=[];page.on('pageerror',error=>fail.push(error.message));
  await page.setViewportSize({width:1440,height:1000});
  await page.evaluate(()=>localStorage.setItem('esp-online-tester.locale','zh-CN'));
  await page.goto(root);await page.waitForFunction(()=>Boolean(window.espTester));
  await page.locator('#product-family-select').selectOption('');
  await page.locator('#product-select').selectOption('m5stack-u202');
  await page.waitForFunction(()=>document.querySelector('#firmware-select').value==='m5stack-u202-official-factory-reference');
  assert(await page.locator('#flash-device').isDisabled(),'Reference-only default has no file');
  await page.locator('#firmware-select').selectOption('m5stack-u202-meshtastic-stable');
  await page.waitForFunction(()=>!document.querySelector('#flash-device').disabled);
  assert(await page.locator('#load-firmware').isHidden(),'C6L simulator remains unavailable');
  assert(await page.locator('.firmware-actions #flash-device').count()===1,'Flash action is beside Run');
  const error=await page.evaluate(async()=>{try{await espTester.loadFirmware();}catch(error){return error.code;}});
  assert(error==='UNSUPPORTED_PRODUCT','Reproduce missing board');
  if(await page.locator('#workbench-error').count())await page.locator('#workbench-error').getByRole('button',{name:'返回工作台'}).click();
  assert(await page.locator('#flash-device').isEnabled(),'Simulator failure cannot disable physical flashing');
  const installChooser=()=>page.evaluate(()=>{
    window.flashTestChooserCalls=0;window.flashTestActivation=false;
    Object.defineProperty(navigator,'serial',{configurable:true,value:{requestPort(){
      window.flashTestChooserCalls++;window.flashTestActivation=navigator.userActivation.isActive;
      return Promise.reject(new DOMException('Test chooser cancelled','NotFoundError'));
    }}});
  });
  await installChooser();
  await page.locator('#flash-device').click();
  const dialog=page.getByRole('dialog',{name:'确认刷写实体设备'}),proceed=dialog.getByRole('button',{name:'确认并选择设备'});
  await dialog.waitFor();assert(await proceed.isDisabled(),'Acknowledgement is required');
  const text=await dialog.innerText();
  for(const value of ['第三方','用户自行制作','备份','自行承担','分区','merged'])assert(text.includes(value),`Missing confirmation text: ${value}`);
  assert(await page.evaluate(()=>flashTestChooserCalls)===0,'Opening confirmation must not choose a port');
  await page.screenshot({path:'output/playwright/flash-confirmation.png',fullPage:true});
  await dialog.getByRole('button',{name:'取消',exact:true}).click();
  assert(await page.evaluate(()=>flashTestChooserCalls)===0,'Cancelling confirmation must not choose a port');
  await page.locator('#flash-device').click();await dialog.getByRole('checkbox').check();await proceed.click();
  await page.waitForFunction(()=>window.flashTestChooserCalls===1&&!document.querySelector('#flash-device').disabled);
  assert(await page.evaluate(()=>flashTestActivation),'Final confirmation must retain Web Serial user activation');
  await page.locator('#flash-device').click();assert(await proceed.isDisabled(),'Every attempt needs a new acknowledgement');
  await page.keyboard.press('Escape');
  results.push('C6L unavailable board: independent button, fresh confirmation, cancel/escape, user-gesture chooser');

  await page.locator('#firmware-select').selectOption('m5stack-u202-uiflow2');
  await page.waitForFunction(()=>document.querySelector('#flash-availability').textContent.includes('验证码'));
  assert(await page.locator('#flash-availability a').getAttribute('href')==='https://burner.m5stack.com/firmware/1897200000000002059','Known verification requirement links to the project page');
  const burnerUrl=await page.evaluate(async()=>(await (await fetch('./catalog/firmwares.json')).json()).items.find(f=>f.id==='m5stack-u202-uiflow2').source.url);
  let downloads=0;
  await page.route(burnerUrl,route=>{downloads++;return route.fulfill({status:400,contentType:'application/json',body:JSON.stringify({code:400,data:{errorCode:'BURN_CAPTCHA_INVALID'}})});});
  await page.evaluate(()=>{
    window.testPortOpened=0;
    Object.defineProperty(navigator,'serial',{configurable:true,value:{requestPort(){return Promise.resolve({open(){window.testPortOpened++;throw Error('No hardware should be opened');}});}}});
  });
  await page.locator('#flash-device').click();await dialog.getByRole('checkbox').check();await proceed.click();
  const manualDownload=page.locator('#workbench-error .error-source-link');
  await manualDownload.waitFor();
  assert(await manualDownload.getAttribute('href')==='https://burner.m5stack.com/firmware/1897200000000002059','CAPTCHA recovery must open the official page, not a GitHub issue');
  assert(downloads===1&&await page.evaluate(()=>testPortOpened)===0,'JSON error stops before opening or writing a serial port');
  assert(await page.locator('#flash-device').isEnabled(),'A download failure permits retry or local import');
  await page.screenshot({path:'output/playwright/flash-captcha.png',fullPage:true});
  await page.locator('#workbench-error').getByRole('button',{name:'返回工作台'}).click();
  await page.unroute(burnerUrl);await installChooser();
  results.push('M5Burner CAPTCHA: visible source warning, bounded download failure, manual import link, no hardware opened');

  const localBytes=new Uint8Array(0x10040).fill(255),view=new DataView(localBytes.buffer);
  for(const at of [0,0x10000]){localBytes.fill(0,at,at+64);localBytes[at]=0xe9;localBytes[at+1]=1;view.setUint16(at+12,13,true);view.setUint32(at+28,4,true);}
  view.setUint16(0x8000,0x50aa,true);localBytes[0x8002]=0;localBytes[0x8003]=0;view.setUint32(0x8004,0x10000,true);view.setUint32(0x8008,0x100000,true);
  await page.getByRole('button',{name:'本地文件',exact:true}).click();
  await page.locator('#firmware-file').setInputFiles({name:'merged.bin',mimeType:'application/octet-stream',buffer:Buffer.from(localBytes)});
  await page.waitForFunction(()=>document.querySelector('#local-build-status').textContent.includes('已识别完整合并镜像'));
  assert(await page.locator('#flash-device').isEnabled(),'Local merged source requires no simulator');
  await page.locator('#flash-device').click();assert((await dialog.innerText()).includes('merged.bin'),'Confirmation identifies local file');
  await dialog.getByRole('button',{name:'取消',exact:true}).click();
  await page.locator('#clear-local-files').click();
  const table=localBytes.slice(0x8000,0x9000);new DataView(table.buffer).setUint32(4,0x20000,true);
  const config=JSON.stringify({flash_files:{'0x0':'boot.bin','0x9000':'table.bin','0x20000':'app.bin'},extra_esptool_args:{chip:'esp32c6'}});
  await page.locator('#firmware-file').setInputFiles([
    {name:'flasher_args.json',mimeType:'application/json',buffer:Buffer.from(config)},
    ...[['boot.bin',localBytes.slice(0,64)],['table.bin',table],['app.bin',localBytes.slice(0x10000)]].map(([name,bytes])=>({name,mimeType:'application/octet-stream',buffer:Buffer.from(bytes)})),
  ]);
  await page.waitForFunction(()=>document.querySelector('#local-build-status').textContent.includes('0x20000'));
  assert(await page.locator('#flash-device').isEnabled(),'Partition build is independently selectable');
  await page.locator('#flash-device').click();await dialog.waitFor();
  assert((await dialog.innerText()).includes('4 个文件'),'Partition confirmation identifies source');await page.keyboard.press('Escape');
  results.push('Local merged and partition files both reach confirmation without running');

  const links=await page.locator('header .repository-link').evaluateAll(elements=>elements.map(el=>({text:el.textContent,url:el.href})));
  assert(links.length===2&&links[0].url==='https://github.com/zhuzhe1983/esp-online-tester'&&links[1].url==='https://github.com/mikroverk/esp32sim','Distinct current/upstream header links');
  await page.screenshot({path:'output/playwright/flash-workbench.png',fullPage:true});
  await page.setViewportSize({width:390,height:844});
  await page.evaluate(()=>window.scrollTo(0,0));
  const header=await page.locator('header').evaluate(el=>({width:el.scrollWidth,viewport:innerWidth,links:[...el.querySelectorAll('.repository-link')].map(a=>({left:a.getBoundingClientRect().left,right:a.getBoundingClientRect().right}))}));
  assert(header.width<=header.viewport&&header.links.every(a=>a.left>=0&&a.right<=header.viewport),'Both repository links fit mobile viewport');
  await page.screenshot({path:'output/playwright/flash-mobile.png',fullPage:true});
  results.push('Current/upstream links and 390px header layout');

  const mini=await page.evaluate(async()=>{
    const {loadCatalog,createDefaultScene,encodeScene}=await import('./catalog.js'),catalog=await loadCatalog();
    const scene=createDefaultScene(catalog,'m5stack-u202');scene.firmware=catalog.firmwares.find(f=>f.id==='m5stack-u202-meshtastic-stable').source;
    return new URL('embed.html#autostart=0&scene='+encodeScene(scene),location.href).href;
  });
  await page.setViewportSize({width:920,height:900});await page.goto(mini);
  await page.waitForFunction(()=>Boolean(window.espTester)&&Boolean(document.querySelector('#flash-device')));
  assert(await page.locator('.transport-main #flash-device').isEnabled(),'Mini flash action is beside Start');
  await installChooser();await page.locator('#flash-device').click();await dialog.waitFor();
  assert(await proceed.isDisabled(),'Mini requires the same confirmation');
  await dialog.getByRole('checkbox').check();await proceed.click();
  await page.waitForFunction(()=>window.flashTestChooserCalls===1&&!document.querySelector('#flash-device').disabled);
  assert(await page.evaluate(()=>flashTestActivation),'Mini final click retains activation');
  await page.screenshot({path:'output/playwright/flash-mini.png',fullPage:true});
  await page.evaluate(async()=>{Object.defineProperty(navigator,'serial',{configurable:true,value:undefined});await espTester.configure((await espTester.observe()).scene);});
  assert(await page.locator('#flash-device').isDisabled(),'Unsupported Web Serial is still disabled');
  assert((await page.locator('#flash-availability').innerText()).includes('Web Serial'),'Browser limitation is visible next to action');
  results.push('Mini confirmation and explicit unsupported-browser reason');
  assert(!fail.length,JSON.stringify(fail));
  return results;
}
