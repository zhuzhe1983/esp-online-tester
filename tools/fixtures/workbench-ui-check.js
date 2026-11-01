// Headless CLI check: menus, locale persistence, error reporting and responsive
// layout. Downloads are intercepted and no hardware or GitHub issue is created.
async page => {
  const assert=(ok,message)=>{if(!ok)throw Error(message);},results=[];
  const root=new URL('./',page.url()).href;
  await page.route('**/platform/**',route=>route.continue());
  await page.evaluate(()=>localStorage.setItem('esp-online-tester.locale','zh-CN'));
  await page.setViewportSize({width:1440,height:1000});await page.goto(root);
  await page.waitForFunction(()=>Boolean(window.espTester)&&document.querySelector('#product-select')?.value);
  assert(await page.locator('#language-select').isVisible(),'Language switcher is restored');
  assert(await page.locator('#report-runtime-issue').count()===0,'No report action in the normal workbench');
  await page.locator('#repository-menu-button').click();
  const repos=await page.locator('#repository-menu a').evaluateAll(elements=>elements.map(el=>el.href));
  assert(repos.join('|')==='https://github.com/zhuzhe1983/esp-online-tester|https://github.com/mikroverk/esp32sim','Current and upstream repositories stay distinct');
  await page.locator('#config-menu-button').click();
  assert(await page.locator('#repository-menu').isHidden(),'Only one header menu can be open');
  await page.keyboard.press('ArrowDown');assert(await page.locator('#import-scene-trigger').evaluate(el=>el===document.activeElement),'ArrowDown focuses first menu item');
  await page.keyboard.press('ArrowDown');assert(await page.locator('#export-scene').evaluate(el=>el===document.activeElement),'ArrowDown advances');
  await page.keyboard.press('Escape');assert(await page.locator('#config-menu').isHidden(),'Escape closes menu');
  assert(await page.locator('#config-menu-button').evaluate(el=>el===document.activeElement),'Escape restores button focus');
  await page.locator('#config-menu-button').click();await page.locator('.brand').click();
  await page.waitForFunction(()=>Boolean(window.espTester)&&document.querySelector('#product-select')?.value);
  assert(await page.locator('#config-menu').isHidden(),'Outside click closes menu');
  const original=await page.evaluate(async()=>(await espTester.observe()).scene);
  await page.locator('#config-menu-button').click();
  const downloadPromise=page.waitForEvent('download');await page.locator('#export-scene').click();const download=await downloadPromise;
  assert(download.suggestedFilename().endsWith('.json'),'Config export still downloads JSON');
  await page.locator('#product-family-select').selectOption('');await page.locator('#product-select').selectOption('m5stack-u202');
  await page.locator('#config-menu-button').click();
  // The CLI pauses scripts on native choosers (also verified with its upload
  // command). Check the trigger and use its file input for this complete run.
  await page.evaluate(()=>{const input=document.querySelector('#import-scene');window.importChooserClicks=0;input.click=()=>{window.importChooserClicks++;};});
  await page.locator('#import-scene-trigger').click();assert(await page.evaluate(()=>importChooserClicks)===1,'Import action invokes the file chooser');
  await page.evaluate(()=>delete document.querySelector('#import-scene').click);
  await page.locator('#import-scene').setInputFiles({name:'scene.json',mimeType:'application/json',buffer:Buffer.from(JSON.stringify(original))});
  await page.waitForFunction(id=>document.querySelector('#product-select').value===id,original.productId);
  assert(await page.locator('#config-menu').isHidden(),'Import closes the menu and restores the exported device');
  results.push('Header menus: distinct repositories, keyboard/outside dismissal, config download and file-chooser import');

  const source=await page.evaluate(async()=>(await espTester.observe()).scene.firmware.url);
  await page.evaluate(async()=>espTester.clearFirmwareCache());
  await page.route(source,route=>route.fulfill({status:404,body:'Missing test firmware'}));
  await page.locator('#load-firmware').click();
  const dialog=page.locator('#workbench-error');await dialog.waitFor();
  assert(await page.locator('#workbench-error-title').innerText()==='设备启动失败','Startup failure has a dedicated error dialog');
  assert(await page.locator('dialog[open]').count()===1,'Runtime event and rejected action do not stack dialogs');
  const issue=await dialog.locator('#report-runtime-issue').getAttribute('href');
  assert(issue.startsWith('https://github.com/zhuzhe1983/esp-online-tester/issues/new?')&&decodeURIComponent(issue).includes('HTTP_ERROR'),'Report draft contains the actual failure');
  await dialog.getByRole('button',{name:'返回工作台'}).click();
  assert(await page.locator('#load-firmware').isEnabled(),'Failed startup leaves the workbench usable');
  await page.unroute(source);
  await page.evaluate(()=>window.dispatchEvent(new ErrorEvent('error',{error:Object.assign(new Error('UI smoke failure token=do-not-share'),{code:'PAGE_SMOKE'}),message:'UI smoke failure'})));
  await dialog.waitFor();assert(await page.locator('#workbench-error-title').innerText()==='页面发生错误','Unhandled page errors use the same modal');
  assert(!decodeURIComponent(await dialog.locator('#report-runtime-issue').getAttribute('href')).includes('do-not-share'),'Report redacts credentials');
  await dialog.getByRole('button',{name:'返回工作台'}).click();
  await page.evaluate(()=>window.dispatchEvent(new ErrorEvent('error',{error:new DOMException('A page node is missing','NotFoundError'),message:'A page node is missing'})));
  await dialog.waitFor();assert(await page.locator('#workbench-error-title').innerText()==='页面发生错误','DOM NotFoundError is not mistaken for a cancelled serial chooser');
  await dialog.getByRole('button',{name:'返回工作台'}).click();
  await page.evaluate(()=>window.dispatchEvent(new PromiseRejectionEvent('unhandledrejection',{promise:Promise.resolve(),reason:new DOMException('User cancelled','AbortError')})));
  assert(await page.locator('dialog[open]').count()===0,'Expected cancellation stays quiet');
  results.push('Startup and page failures: one modal, report draft with redaction, dismiss/retry, no cancellation modal');

  for(const lang of ['zh-CN','en']){
    if(lang==='en'){
      await page.locator('#language-select').selectOption('en');await page.waitForFunction(()=>document.documentElement.lang==='en'&&Boolean(window.espTester)&&document.querySelector('#product-select')?.value);
      await page.reload();await page.waitForFunction(()=>document.documentElement.lang==='en'&&Boolean(window.espTester)&&document.querySelector('#product-select')?.value);
      assert(await page.locator('#language-select').inputValue()==='en','Language choice survives reload');
      await page.locator('#config-menu-button').click();assert((await page.locator('#config-menu').innerText()).includes('Import config'),'Configuration menu is translated');await page.keyboard.press('Escape');
    }
    for(const width of [1440,1024,820,768,721,720,640,500,390,320]){
      await page.setViewportSize({width,height:900});
      const layout=await page.locator('header').evaluate(el=>({width:innerWidth,scroll:el.scrollWidth,items:[...el.querySelectorAll('.brand,#repository-menu-button,#config-menu-button,#language-select,#share-scene')].map(el=>{const r=el.getBoundingClientRect();return {left:r.left,right:r.right,top:r.top,bottom:r.bottom};})}));
      assert(layout.scroll<=width&&layout.items.every(r=>r.left>=0&&r.right<=width),`${lang} header fits ${width}px`);
      for(let a=0;a<layout.items.length;a++)for(let b=a+1;b<layout.items.length;b++){const x=layout.items[a],y=layout.items[b];assert(!(Math.min(x.right,y.right)-Math.max(x.left,y.left)>1&&Math.min(x.bottom,y.bottom)-Math.max(x.top,y.top)>1),`${lang} header items do not overlap at ${width}px`);}
      if([1440,500,390].includes(width))await page.locator('header').screenshot({path:`output/playwright/header-${lang}-${width}.png`});
    }
    await page.locator('#repository-menu-button').click();
    const rect=await page.locator('#repository-menu').boundingBox();assert(rect.x>=0&&rect.x+rect.width<=320,'Repository popup fits narrow screen');
    await page.keyboard.press('Escape');
  }
  await page.setViewportSize({width:1200,height:950});
  await page.locator('#product-family-select').selectOption('');await page.locator('#product-select').selectOption('m5stack-u202');
  await page.locator('#firmware-select').selectOption('m5stack-u202-uiflow2');await page.locator('#flash-device').click();
  const confirm=page.getByRole('dialog',{name:'Confirm hardware flashing'});await confirm.waitFor();
  assert((await confirm.innerText()).includes('third parties'),'New flashing disclaimer is translated');
  assert(!(await confirm.innerText()).match(/[\u4e00-\u9fff]/),'Flash confirmation has no untranslated UI copy');
  await page.keyboard.press('Escape');
  await page.evaluate(async()=>{
    const {M5BURNER_VERIFICATION_MESSAGE}=await import('./firmware-download-access.js');
    const catalog=await (await fetch('./catalog/firmwares.json')).json();const url=catalog.items.find(f=>f.id==='m5stack-u202-uiflow2').source.url;
    window.dispatchEvent(new ErrorEvent('error',{error:Object.assign(new Error(M5BURNER_VERIFICATION_MESSAGE),{code:'FIRMWARE_AUTH_REQUIRED',details:{url}}),message:M5BURNER_VERIFICATION_MESSAGE}));
  });
  await page.getByRole('dialog',{name:'Firmware download requires verification'}).waitFor();
  assert((await page.locator('#workbench-error-message').innerText()).includes('CAPTCHA'),'Authentication guidance is translated');
  await dialog.screenshot({path:'output/playwright/error-en.png'});await dialog.getByRole('button',{name:'Back to workbench'}).click();
  results.push('Chinese/English persisted selection, 320–1440px layouts, translated flash and verification dialogs');
  await page.route('**/platform/app.js',route=>route.abort('failed'));
  await page.goto(root);await page.getByRole('dialog',{name:'Something went wrong on this page'}).waitFor();
  assert((await page.locator('#workbench-error-message').innerText()).includes('Page scripts could not load'),'Entry-point failure is visible even before app initialization');
  assert(decodeURIComponent(await page.locator('#report-runtime-issue').getAttribute('href')).includes('PAGE_LOAD_FAILED'),'Module-loading failure has a report draft');
  await page.unroute('**/platform/app.js');await page.goto(root);
  await page.waitForFunction(()=>Boolean(window.espTester)&&document.querySelector('#product-select')?.value);
  results.push('Failed app module opens a localized error dialog before app handlers exist; reload recovers');
  return results;
}
