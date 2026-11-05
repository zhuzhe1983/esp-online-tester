// Runs against the real workbench and mini. Flash states and verification
// metadata below are synthetic UI inputs; no ports, writes or CAPTCHA solving.
async page => {
 const root=new URL('./',page.url()),errors=[],assert=(ok,message)=>{if(!ok)throw Error(message);};
 page.on('pageerror',error=>errors.push(error.message));
 await page.evaluate(()=>localStorage.removeItem('esp-online-tester.locale'));
 await page.setViewportSize({width:1440,height:1100});await page.goto(new URL('?lang=en',root).href);
 await page.waitForFunction(()=>document.documentElement.lang==='en'&&window.espTester);
 const settled=()=>page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
 const untranslated=async selector=>page.locator(selector).evaluate(root=>{
  const result=[],walker=document.createTreeWalker(root,NodeFilter.SHOW_ELEMENT|NodeFilter.SHOW_TEXT);
  const skipped=node=>{for(let el=node.nodeType===1?node:node.parentElement;el;el=el.parentElement)if(el.matches('pre,textarea,script,style,[data-i18n-skip]'))return true;return false;};
  for(let n=walker.nextNode();n;n=walker.nextNode()){
   if(skipped(n))continue;const el=n.nodeType===1?n:n.parentElement;if(!el?.checkVisibility())continue;
   if(n.nodeType===3&&/[\u4e00-\u9fff]/.test(n.nodeValue))result.push(n.nodeValue.trim());
   if(n.nodeType===1)for(const name of ['title','aria-label','placeholder','alt','label'])if(/[\u4e00-\u9fff]/.test(n.getAttribute(name)||''))result.push(name+':'+n.getAttribute(name));
  }return [...new Set(result)];
 });
 const boundary=await page.evaluate(async()=>{
  const {translateTree}=await import('./i18n.js'),root=document.createElement('div');root.id='i18n-boundary-fixture';
  root.innerHTML='<section data-i18n-skip><span title="启动" aria-label="暂停">启动</span></section><pre><span title="启动">暂停</span></pre><button title="暂停">启动</button><img alt="设备预览"><p>原适配型号说明：启动</p><textarea></textarea>';
  root.querySelector('textarea').value='启动';document.body.append(root);translateTree(root);
  await new Promise(resolve=>setTimeout(resolve,30));
  const opaque=root.querySelector('[data-i18n-skip] span'),pre=root.querySelector('pre span'),button=root.querySelector('button');
  const first=opaque.textContent==='启动'&&opaque.title==='启动'&&opaque.getAttribute('aria-label')==='暂停'&&pre.title==='启动'&&pre.textContent==='暂停'&&button.textContent==='Start'&&button.title==='Pause'&&root.querySelector('img').alt==='Device preview'&&root.querySelector('textarea').value==='启动'&&root.querySelector('p').textContent==='Notes for the original target: 启动';
  button.textContent='暂停';opaque.title='暂停';await new Promise(resolve=>setTimeout(resolve,30));
  const result=first&&button.textContent==='Pause'&&opaque.title==='暂停';root.remove();return result;
 });assert(boundary,'Inherited skip boundaries, opaque interpolation and dynamic nodes must be preserved');
 await page.evaluate(async()=>{const {loadCatalog,createDefaultScene}=await import('./catalog.js'),c=await loadCatalog(),scene=createDefaultScene(c,'m5stack-triviapod');scene.firmware=c.firmwares.find(item=>item.id==='m5stack-stopwatch-official').source;await espTester.configure(scene);});
 await page.waitForFunction(()=>document.querySelector('.firmware-update-identity')?.textContent.includes('Current model unverified'));
 assert((await page.locator('#firmware-select optgroup').first().getAttribute('label')).includes('Shared target'),'Shared firmware group is translated');
 await page.evaluate(async()=>{const {loadCatalog,createDefaultScene}=await import('./catalog.js'),c=await loadCatalog(),scene=createDefaultScene(c,'m5stack-s013');scene.firmware=c.firmwares.find(item=>item.productIds?.includes('m5stack-s013')&&item.source?.url).source;await espTester.configure(scene);});
 await settled();assert(await page.locator('#flash-device').isDisabled(),'P4 flashing support remains unchanged');
 assert((await page.locator('#flash-device').getAttribute('title')||'').includes('Online flashing currently supports'),'P4 disabled explanation is translated');
 await page.evaluate(async()=>{
  const {openFlashStation}=await import('./firmware-flash-ui.js');
  const receipt={artifacts:[{address:0,bytes:1024,sha256:'a'.repeat(64)}],configurationFormats:{uiflow1:true,uiflow2:true}};
  window.i18nStation=openFlashStation({product:{name:'UI fixture device',chip:'ESP32-C6'},source:{file:{name:'ui-fixture.bin'}},scene:{},catalog:{},prepareSource:async source=>source,createBatch:({onEvent})=>{
   let state={status:'ready',items:[],receipt,counts:{total:0,done:0,error:0,queued:0},running:false};
   window.i18nFlashEmit=change=>{state={...state,...change};onEvent({type:'state',state});};
   return {prepare:async()=>window.i18nFlashEmit({}),dispose:async()=>{},setConfiguration(){},stop:async()=>window.i18nFlashEmit({status:'stopped',running:false})};
  }});
 });
 const dialog=page.locator('.flash-station');await page.getByRole('dialog',{name:'Flash to device',exact:true}).waitFor();await settled();
 let missing=await untranslated('.flash-station');assert(!missing.length,'Flash dialog missing translations: '+missing.join(' | '));
 await dialog.locator('input[value="parallel"]').check();await settled();assert((await dialog.locator('.flash-station-actions').innerText()).includes('Flash 0 devices'),'Interpolated parallel count is translated');
 await dialog.getByRole('button',{name:'New configuration',exact:true}).click();await settled();
 assert(await dialog.getByRole('textbox',{name:'Configuration name',exact:true}).inputValue()==='New configuration','New input defaults follow the UI language');
 missing=await untranslated('.flash-station');assert(!missing.length,'Configuration missing translations: '+missing.join(' | '));
 await dialog.getByRole('button',{name:'Cancel editing',exact:true}).click();
 await page.evaluate(()=>i18nFlashEmit({status:'running',running:true,items:[{id:'fixture',label:'Test device',status:'writing',progress:.4,connected:true,logs:['启动\n暂停']}],counts:{total:1,queued:0,done:0,error:0,active:1}}));await settled();
 assert((await dialog.locator('progress').getAttribute('aria-label')).includes('Write progress for Test device'),'Progress accessibility label is translated');
 assert(await dialog.locator('pre').textContent()==='启动\n暂停','Flash diagnostics are not translated');
 missing=await untranslated('.flash-station');assert(!missing.length,'Progress missing translations: '+missing.join(' | '));
 await page.screenshot({path:'output/playwright/i18n-flash-en.png'});
 await page.setViewportSize({width:390,height:844});await settled();
 const bounds=await dialog.evaluate(el=>({width:el.scrollWidth,client:el.clientWidth,left:el.getBoundingClientRect().left,right:el.getBoundingClientRect().right}));
 assert(bounds.width<=bounds.client&&bounds.left>=0&&bounds.right<=390,'English flash dialog fits narrow screens');
 await page.screenshot({path:'output/playwright/i18n-flash-en-mobile.png'});
 await page.evaluate(()=>i18nStation.dispose());await page.setViewportSize({width:1440,height:1100});
 await page.evaluate(async()=>{
  const {requestBurnerVerification}=await import('./m5burner-captcha.js');
  const canvas=document.createElement('canvas');canvas.width=canvas.height=80;
  window.i18nVerification=requestBurnerVerification('https://burner.m5stack.com/api/v1/firmwares/1897200000000000718/versions/2098280161648791553/download',{client:{issue:async()=>({captchaToken:'synthetic_i18n_token_1234567890',width:80,height:80,targetCount:2,expiresInSeconds:60,instruction:'请按顺序点击：启动,暂停',captchaImage:canvas.toDataURL()}),verify:async()=>{throw Error('Verification must never be submitted by this check');}}}).catch(error=>error.code);
 });
 await page.locator('.burner-captcha-targets').waitFor();await settled();
 const words=await page.locator('.burner-captcha-targets strong').allTextContents();assert(words.join(',')==='启动,暂停','Verification target words remain original');
 const verify=page.locator('.burner-captcha').getByRole('button',{name:'Verify & continue',exact:true});
 assert(await verify.isDisabled()&&(await verify.getAttribute('title')).includes('requested characters'),'Incomplete manual selection is disabled with a clear reason');
 assert((await page.locator('#burner-captcha-submit-help').innerText()).includes('once'),'The manual single-click submission instruction is visible');
 missing=await untranslated('.burner-captcha');assert(!missing.length,'Verification UI missing translations: '+missing.join(' | '));
 await page.locator('.burner-captcha').getByRole('button',{name:'Cancel',exact:true}).click();
 await page.locator('#language-select').selectOption('zh-CN');await page.waitForFunction(()=>document.documentElement.lang==='zh-CN'&&window.espTester);await settled();
 assert((await page.locator('#flash-device').innerText()).includes('烧录到设备'),'Chinese source returns after switching language');
 await page.locator('#language-select').selectOption('en');await page.waitForFunction(()=>document.documentElement.lang==='en'&&window.espTester);
 const mini=await page.evaluate(()=>espTester.share({mode:'embed',autostart:false,autoplay:false}));await page.goto(mini);
 await page.waitForFunction(()=>document.documentElement.lang==='en'&&window.espTester);await settled();
 assert(await page.locator('#language-select').inputValue()==='en','Mini respects the saved language');
 assert((await page.locator('#flash-device').innerText()).includes('Flash to device'),'Mini translates the flashing entry');
 await page.goto(new URL('agents.html?lang=en',root).href);await page.waitForFunction(()=>document.documentElement.lang==='en'&&document.querySelector('#language-select'));await settled();
 missing=await untranslated('body');assert(!missing.length,'Agent guide missing translations: '+missing.join(' | '));
 assert(!/[\u4e00-\u9fff]/.test(await page.title()),'Guide title is translated');
 assert(!/[\u4e00-\u9fff]/.test(await page.locator('meta[name="description"]').getAttribute('content')),'Guide description is translated');
 assert(await page.locator('a[href="./skill/SKILL.md"]').count()===1&&await page.locator('a[href="./onboarding-skill/SKILL.md"]').count()===1,'Skill targets are unchanged');
 assert(await page.locator('a[href="https://github.com/zhuzhe1983"][rel="author"]').count()===1,'Author attribution is unchanged');
 await page.screenshot({path:'output/playwright/i18n-agents-en.png'});await page.setViewportSize({width:390,height:844});await settled();
 assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'English guide fits narrow screens');
 await page.screenshot({path:'output/playwright/i18n-agents-en-mobile.png'});
 await page.locator('#language-select').selectOption('zh-CN');await page.waitForFunction(()=>document.documentElement.lang==='zh-CN');
 assert((await page.locator('h1').innerText()).includes('用智能体开发和测试 ESP 产品'),'Guide can return to Chinese');
 assert(!errors.length,errors.join('\n'));
 return {localeRoundTrip:true,mini:true,agentGuide:true,sharedFirmware:true,p4DisabledReason:true,flashDialog:true,narrowLayout:true,configurationDefault:true,dynamicAttributes:true,skipInheritance:true,opaqueData:true,verificationWordsPreserved:true,serialOutputPreserved:true,hardwareWrites:0,verificationSubmissions:0,pageErrors:errors};
}
