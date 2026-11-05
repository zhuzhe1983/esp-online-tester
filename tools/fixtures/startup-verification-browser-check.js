// Headless UI regression with local challenge/transport doubles, never a live CAPTCHA.
async page => {
  const base=new URL('./',page.url()).href,checks=[],errors=[];
  const assert=(v,message)=>{if(!v)throw Error(message);};
  page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(()=>{
    if(window.top!==window)return;
    localStorage.setItem('esp-online-tester.locale','zh-CN');
    window.maxRunDialogs=0;
    new MutationObserver(()=>{maxRunDialogs=Math.max(maxRunDialogs,document.querySelectorAll('dialog.firmware-run-confirm[open]').length);}).observe(document,{subtree:true,childList:true,attributes:true,attributeFilter:['open']});
  });
  await page.goto(base);await page.waitForFunction(()=>!!window.espTester);
  const fixture=await page.evaluate(async()=>{
    const {loadCatalog,createDefaultScene,encodeScene}=await import('./catalog.js'),catalog=await loadCatalog();
    const url='https://burner.m5stack.com/api/v1/firmwares/1897200000000001943/versions/1897300000000004110/download';
    const scene=createDefaultScene(catalog,'m5stack-stopwatch');scene.firmware={url};
    const canvas=document.createElement('canvas');canvas.width=520;canvas.height=300;const ctx=canvas.getContext('2d');ctx.fillStyle='#e8eef7';ctx.fillRect(0,0,520,300);ctx.fillStyle='#253c51';ctx.font='30px sans-serif';ctx.fillText('Local verification fixture',50,150);
    return {url,hash:'#scene='+encodeScene(scene)+'&autostart=1&autoplay=1',challenge:{captchaToken:'local-fixture-token-00000000001',captchaImage:canvas.toDataURL(),width:520,height:300,targetCount:3,instruction:'Click in order: PaperS3, Cardputer, StickS3',expiresInSeconds:120}};
  });
  let issues=0,verifies=0,downloads=0;
  await page.route('https://burner.m5stack.com/api/v1/firmware-burn/captcha**',async route=>{
    if(route.request().url().endsWith('/verify')){verifies++;await route.fulfill({json:{code:200,data:{captchaToken:fixture.challenge.captchaToken}}});}
    else {issues++;await route.fulfill({json:{code:200,data:fixture.challenge}});}
  });
  const merged=new Uint8Array(16*1024*1024).fill(255);
  for(const [at,name] of [[0,'bootloader'],[0x8000,'ptable'],[0x10000,'']]){
    const response=await page.request.get(new URL('../wasm/fw/public/platform-stopwatch'+(name?'-'+name:'')+'.bin',base).href);
    assert(response.ok(),'Missing local StopWatch firmware fixture');merged.set(await response.body(),at);
  }
  await page.route(fixture.url+'**',route=>{
    downloads++;
    return route.request().url().includes('captchaToken=')?route.fulfill({contentType:'application/octet-stream',body:Buffer.from(merged)}):route.fulfill({status:400,json:{code:400,data:{errorCode:'BURN_CAPTCHA_INVALID'}}});
  });
  for(const file of ['index.html','embed.html']){
    await page.goto(base+file+fixture.hash);const confirmation=page.getByRole('dialog',{name:'尚未确认完整适配'});await confirmation.waitFor();
    assert(await page.evaluate(()=>maxRunDialogs)===1,'Autostart opens one confirmation');
    await confirmation.getByRole('button',{name:'继续运行',exact:true}).dblclick();
    const dialog=page.getByRole('dialog',{name:'完成 M5Burner 下载验证'});await dialog.waitFor();await page.waitForFunction(()=>document.querySelector('.burner-captcha-image')?.hidden===false);
    assert(await page.locator('dialog[open]').count()===1,'Confirmation closes before verification opens');
    assert(await dialog.getByRole('button',{name:'验证并继续',exact:true}).isDisabled(),'Fast submit cannot verify an incomplete challenge');
    await dialog.getByRole('button',{name:'取消',exact:true}).click();
    await page.waitForFunction(()=>!document.querySelector('dialog[open]'));
    assert(!(await page.evaluate(async()=> (await espTester.observe()).firmware)),'Cancellation never loads firmware');
    assert((await page.locator('#empty-description').innerText()).includes('已取消自动启动'),'Cancellation is not reported as startup failure');
    assert(await page.evaluate(()=>maxRunDialogs)===1,'No secondary error dialog or overlap');
  }
  assert(issues===2&&verifies===0&&downloads===2,'Rapid continue/cancel starts one request per flow');
  checks.push('Workbench and mini autoplay: one ordered confirmation/verification, rapid continue guarded, cancel has no error popup');

  // Exercise the successful human path all the way to guest serial/frame output.
  await page.locator('#start-runtime').click();await page.getByRole('button',{name:'继续运行',exact:true}).click();
  const dialog=page.getByRole('dialog',{name:'完成 M5Burner 下载验证'});await page.waitForFunction(()=>document.querySelector('.burner-captcha-image')?.hidden===false);
  for(const position of [{x:50,y:50},{x:130,y:90},{x:220,y:120}])await dialog.locator('.burner-captcha-image').click({position});
  await dialog.getByRole('button',{name:'验证并继续',exact:true}).click();
  const execution=await page.evaluate(async()=>{
    await espTester.waitFor({status:'running',serialIncludes:'PLATFORM_READY stopwatch',timeoutMs:60000});
    let s=await espTester.observe();
    for(let attempt=0;!s.frame&&attempt<100;attempt++){await new Promise(resolve=>setTimeout(resolve,50));s=await espTester.observe();}
    return {manualSubmissionStartsFirmware:true,status:s.status,productId:s.product.id,frame:s.frame,readySerial:s.serial.includes('PLATFORM_READY stopwatch')};
  });
  assert(execution.readySerial&&execution.frame?.frameNumber>0,'Manual submission executes firmware through serial and display output');
  assert(verifies===1,'One manual click sends exactly one verification');
  await page.evaluate(()=>espTester.pause());
  checks.push(execution);

  const openCase=async(kind)=>{
    await page.evaluate(async({fixture,kind})=>{
      const {requestBurnerVerification}=await import('./m5burner-captcha.js');
      window.verificationTest={issues:0,verifies:0,aborted:0,late:null,outcome:null};
      const t=verificationTest;
      const client={issue:async()=>{t.issues++;return fixture.challenge;},verify:async(c,p,s)=>{
        t.verifies++;s.addEventListener('abort',()=>t.aborted++,{once:true});
        if(kind==='rate')throw Object.assign(Error('请求过于频繁，请稍后再试。'),{code:'CAPTCHA_RATE_LIMIT'});
        return new Promise(resolve=>{t.late=()=>resolve(c.captchaToken);});
      }};
      void requestBurnerVerification(fixture.url,{client,attemptTimeoutMs:kind==='timeout'?80:10000}).then(()=>t.outcome='success',e=>t.outcome=e.code);
    },{fixture,kind});
    await dialog.waitFor();await page.waitForFunction(()=>document.querySelector('.burner-captcha-image')?.hidden===false);
    assert(await dialog.getByRole('button',{name:'自动验证',exact:true}).count()===0,'No automatic verification control');
  };
  const selectPoints=async()=>{for(const position of [{x:50,y:50},{x:130,y:90},{x:220,y:120}])await dialog.locator('.burner-captcha-image').click({position});};
  await openCase('timeout');await selectPoints();await dialog.getByRole('button',{name:'验证并继续',exact:true}).click();
  await page.waitForFunction(()=>verificationTest.aborted===1&&document.querySelector('.burner-captcha .form-error').textContent.includes('超过'));
  assert(await page.evaluate(()=>verificationTest.issues===1&&verificationTest.verifies===1&&verificationTest.aborted===1),'Timeout aborts one manual submission without issuing another challenge');
  await page.evaluate(()=>verificationTest.late());
  assert(await dialog.isVisible(),'A late response cannot complete a timed-out submission');
  await dialog.getByRole('button',{name:'换一张',exact:true}).click();await page.waitForFunction(()=>verificationTest.issues===2);
  assert(await dialog.getByRole('button',{name:'验证并继续',exact:true}).isDisabled(),'Manual refresh clears previous selections');
  await dialog.getByRole('button',{name:'取消',exact:true}).click();await page.waitForFunction(()=>verificationTest.outcome==='FETCH_ABORTED');
  await openCase('cancel');await selectPoints();await dialog.getByRole('button',{name:'验证并继续',exact:true}).click();
  await page.waitForFunction(()=>verificationTest.verifies===1);await dialog.getByRole('button',{name:'取消',exact:true}).click();
  await page.waitForFunction(()=>verificationTest.outcome==='FETCH_ABORTED');await page.evaluate(()=>verificationTest.late());
  assert(await page.evaluate(()=>verificationTest.issues===1&&verificationTest.verifies===1&&verificationTest.aborted===1),'Cancellation aborts the pending request without retries');
  assert(await page.locator('dialog[open]').count()===0,'A late response cannot reopen a cancelled dialog');
  await openCase('rate');await selectPoints();await dialog.getByRole('button',{name:'验证并继续',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('.burner-captcha .form-error').textContent.includes('请求过于频繁'));
  assert(await page.evaluate(()=>verificationTest.issues===1&&verificationTest.verifies===1),'Rate limit does not trigger retries');
  await page.setViewportSize({width:390,height:844});await page.screenshot({path:'output/playwright/startup-verification-manual.png'});
  assert(await dialog.evaluate(e=>e.scrollWidth<=e.clientWidth),'Verification controls fit mobile width');
  await dialog.getByRole('button',{name:'取消',exact:true}).click();
  checks.push('Manual submission timeout/cancel, stale response disposal, explicit refresh, no retries, mobile layout');
  assert(!errors.length,errors.join('\n'));return checks;
}
