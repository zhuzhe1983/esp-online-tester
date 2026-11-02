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
  await dialog.getByRole('button',{name:'验证并继续',exact:true}).dblclick();
  await page.waitForFunction(async()=>{const s=await espTester.observe();return s.status==='running'&&s.serial.includes('PLATFORM_READY stopwatch')&&s.frame?.frameNumber>0;},{},{timeout:60000});
  assert(verifies===1,'Double submit sends exactly one verification');
  await page.locator('#pause-runtime').click();await page.waitForFunction(async()=>(await espTester.observe()).status==='paused');
  checks.push('Manual verification resumes the same startup once and executes real StopWatch firmware');

  const openCase=async(kind,timeout=80)=>{
    await page.evaluate(async({fixture,kind,timeout})=>{
      const {requestBurnerVerification}=await import('./m5burner-captcha.js');
      window.verificationTest={issues:0,reads:0,verifies:0,aborted:0,late:null,outcome:null};
      const t=verificationTest;
      const client={issue:async()=>{t.issues++;return {...fixture.challenge,captchaToken:'local-challenge-token-'+String(t.issues).padStart(10,'0')};},verify:async(c,p,s)=>{t.verifies++;if(kind==='rate')throw Object.assign(Error('请求过于频繁，请稍后再试。'),{code:'CAPTCHA_RATE_LIMIT'});return c.captchaToken;}};
      const readPoints=(c,{signal,onProgress})=>{
        t.reads++;signal.addEventListener('abort',()=>t.aborted++,{once:true});
        if(kind==='timeout'&&t.reads===1||kind==='cancel'||kind==='manual')return new Promise(resolve=>{t.late=()=>{onProgress('stale-result');resolve({complete:true,points:[{x:1,y:1},{x:2,y:2},{x:3,y:3}]});};});
        if(kind==='incomplete')return Promise.resolve({complete:false,points:[]});
        return Promise.resolve({complete:true,points:[{x:1,y:1},{x:2,y:2},{x:3,y:3}]});
      };
      void requestBurnerVerification(fixture.url,{client,readPoints,attemptTimeoutMs:timeout}).then(()=>t.outcome='success',e=>t.outcome=e.code);
    },{fixture,kind,timeout});
    await dialog.waitFor();await page.waitForFunction(()=>document.querySelector('.burner-captcha-image')?.hidden===false);
  };
  await openCase('timeout',10000);await dialog.getByRole('button',{name:'自动验证',exact:true}).click();
  await page.waitForFunction(()=>verificationTest.outcome==='success');
  assert(await page.evaluate(()=>verificationTest.issues===2&&verificationTest.reads===2&&verificationTest.verifies===1&&verificationTest.aborted>=1),'Timed-out attempt aborts, refreshes and retries once');
  await page.evaluate(()=>verificationTest.late());
  assert(await page.locator('dialog[open]').count()===0,'Late OCR result cannot resurrect a closed dialog');
  checks.push('Deadline abort, next image retry, successful continuation and stale result disposal');
  await openCase('incomplete');await dialog.getByRole('button',{name:'自动验证',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('.burner-captcha .form-error').textContent.includes('已尝试 3 张'));
  assert(await page.evaluate(()=>verificationTest.issues===3&&verificationTest.reads===3&&verificationTest.verifies===0),'Only three automatic attempts');
  await page.setViewportSize({width:390,height:844});await page.screenshot({path:'output/playwright/startup-verification-fallback.png'});
  assert(await dialog.evaluate(e=>e.scrollWidth<=e.clientWidth),'Verification controls fit mobile width');
  await dialog.getByRole('button',{name:'取消',exact:true}).click();await page.waitForFunction(()=>verificationTest.outcome==='FETCH_ABORTED');
  await openCase('cancel',10000);await dialog.getByRole('button',{name:'自动验证',exact:true}).click();await page.waitForFunction(()=>verificationTest.reads===1);await dialog.getByRole('button',{name:'取消',exact:true}).click();
  await page.waitForFunction(()=>verificationTest.outcome==='FETCH_ABORTED');await page.evaluate(()=>verificationTest.late());
  assert(await page.evaluate(()=>verificationTest.issues===1&&verificationTest.verifies===0&&verificationTest.aborted===1),'Cancellation stops retry and stale verification submission');
  await openCase('manual',10000);await dialog.getByRole('button',{name:'自动验证',exact:true}).click();await page.waitForFunction(()=>verificationTest.reads===1);
  await dialog.getByRole('button',{name:'改为手动',exact:true}).click();await page.evaluate(()=>verificationTest.late());
  assert((await dialog.locator('.form-error').innerText()).includes('已停止自动验证'),'Late progress cannot overwrite manual mode');
  assert(await page.evaluate(()=>verificationTest.verifies===0),'Switching to manual does not submit stale OCR');await dialog.getByRole('button',{name:'取消',exact:true}).click();
  await openCase('rate');await dialog.getByRole('button',{name:'自动验证',exact:true}).click();await page.waitForFunction(()=>document.querySelector('.burner-captcha .form-error').textContent.includes('请求过于频繁'));
  assert(await page.evaluate(()=>verificationTest.issues===1&&verificationTest.verifies===1),'Rate limit stops automatic retry');await dialog.getByRole('button',{name:'取消',exact:true}).click();
  checks.push('Three-attempt cap, mobile layout, cancel/manual takeover, stale progress protection, rate-limit stop');
  // Exercise the shipped reader and its real nested Tesseract worker as well.
  const actual=await page.evaluate(async()=>{
    const {readImagePoints}=await import('./m5burner-image-points.js'),{captchaAttempt}=await import('./m5burner-captcha.js');
    const canvas=document.createElement('canvas');canvas.width=520;canvas.height=300;const c=canvas.getContext('2d');c.fillStyle='white';c.fillRect(0,0,520,300);c.font='bold 32px Arial';
    for(const [i,text] of ['PaperS3','Cardputer','StickS3'].entries()){c.fillStyle=['rgb(214,167,255)','rgb(255,156,125)','rgb(143,211,255)'][i];c.fillText(text,40,60+i*90);}
    window.readerFixture={captchaImage:canvas.toDataURL(),width:520,height:300,instruction:'Click in order: PaperS3, Cardputer, StickS3'};
    return captchaAttempt(signal=>readImagePoints(readerFixture,{signal}));
  });
  assert(actual.complete&&actual.points.length===3,'Production OCR worker reads the synthetic three-label image');
  const cdp=await page.context().newCDPSession(page);
  const assertNoReaderWorkers=async()=>{
    let remaining=[];for(let i=0;i<40;i++){remaining=(await cdp.send('Target.getTargets')).targetInfos.filter(t=>t.type==='worker'&&t.url.startsWith(base)&&/m5burner-image-worker|tesseract/.test(t.url));if(!remaining.length)return;await new Promise(r=>setTimeout(r,50));}
    assert(false,'Reader worker tree remained alive: '+remaining.map(t=>t.url).join(','));
  };
  await assertNoReaderWorkers();
  try {
    for(const phase of ['准备验证','处理图片']){
      const code=await page.evaluate(async phase=>{
        const {readImagePoints}=await import('./m5burner-image-points.js'),{captchaAttempt}=await import('./m5burner-captcha.js');
        const aborter=new AbortController();
        try {await captchaAttempt(signal=>readImagePoints(readerFixture,{signal,onProgress:text=>{if(text.includes(phase))aborter.abort();}}),{signal:aborter.signal});return 'unexpected-success';}
        catch(e){return e.code;}
      },phase);
      assert(code==='FETCH_ABORTED','Reader cancels during '+phase);await assertNoReaderWorkers();
    }
  }finally{await cdp.detach();}
  checks.push('Real synthetic OCR and termination of parent/nested workers, during initialization and recognition');
  assert(!errors.length,errors.join('\n'));return checks;
}
