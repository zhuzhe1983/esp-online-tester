// Run through the task-owned Playwright wrapper; ?firmware= supplies the pinned first-party probe.
async page => {
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  const firmware=new URL(page.url()).searchParams.get('firmware');
  if(!firmware)throw Error('Supply ?firmware= with the original CloudZao first-party diagnostic URL.');
  await page.setViewportSize({width:1440,height:1000});
  await page.waitForFunction(()=>window.espTester);
  const proof=await page.evaluate(async firmware=>{
    const api=espTester,{loadCatalog,createDefaultScene}=await import('./catalog.js');
    await api.configure(createDefaultScene(await loadCatalog(),'cloudzao-claw-154'));
    const url=new URL(firmware,location.href).href;
    await api.loadFirmware({url,bootMode:'app-direct',sha256:'666d7b2e553d6788aafc48282ebfec1fb121b76849295437d098b8f55d4ca4f8'});
    await api.start();
    const serial=s=>s.serialByPort?.uart0||'';
    const wait=async fn=>{const deadline=Date.now()+60000;while(Date.now()<deadline){const s=await api.observe();if(s.status==='error')throw Error(JSON.stringify(s.lastError));if(fn(s))return s;await new Promise(r=>setTimeout(r,20));}throw Error('Real guest output timeout');};
    const frame=async color=>{const deadline=Date.now()+8000;while(Date.now()<deadline){const f=await api.capture({format:'rgba'});if(f.width===240&&f.height===240&&f.pixels.every((v,i)=>v===[...color,255][i%4]))return {width:f.width,height:f.height,sha256:[...new Uint8Array(await crypto.subtle.digest('SHA-256',f.pixels))].map(v=>v.toString(16).padStart(2,'0')).join('')};await new Promise(r=>setTimeout(r,20));}throw Error('Expected guest pixels not received');};
    await wait(s=>serial(s).includes('CLOUDZAO CLAW154 SRAM READY\n')&&serial(s).includes('FRAME COMPLETE\n')&&s.frame);
    const initial=await frame([255,0,0]);
    let mark=serial(await api.observe()).length;
    await api.input({type:'button',id:'boot',pressed:true});
    await wait(s=>serial(s).slice(mark).includes('BUTTON DOWN\n')&&serial(s).slice(mark).includes('FRAME COMPLETE\n'));
    const pressed=await frame([0,255,0]);
    mark=serial(await api.observe()).length;
    await api.input({type:'button',id:'boot',pressed:false});
    await wait(s=>serial(s).slice(mark).includes('BUTTON UP\n')&&serial(s).slice(mark).includes('FRAME COMPLETE\n'));
    const released=await frame([255,0,0]);
    await api.pause();
    if(initial.sha256!==released.sha256||initial.sha256===pressed.sha256)throw Error('Frame/input mismatch');
    const state=await api.observe();
    return {productId:state.scene.productId,status:state.status,initial,pressed,released,guestInputAndFrameVerified:true,firmwareRole:'original first-party diagnostic; not factory firmware',sourceSha256:'666d7b2e553d6788aafc48282ebfec1fb121b76849295437d098b8f55d4ca4f8'};
  },firmware);
  await page.waitForFunction(()=>document.querySelector('#scene-mount').dataset.modelStatus==='ready');
  await page.screenshot({path:'output/playwright/cloudzao-claw154-real-frame.png',fullPage:true});
  const share=await page.evaluate(()=>espTester.share({mode:'embed',autostart:false}));
  const mini=await page.context().newPage();
  try{
    await mini.setViewportSize({width:390,height:844});await mini.goto(typeof share==='string'?share:share.url);
    await mini.waitForFunction(()=>window.espTester&&document.querySelector('#scene-mount').dataset.modelStatus==='ready');
    proof.embed=await mini.evaluate(async()=>({productId:(await espTester.observe()).scene.productId,overflow:document.documentElement.scrollWidth>innerWidth,modelStatus:document.querySelector('#scene-mount').dataset.modelStatus}));
    if(proof.embed.overflow)throw Error('Mobile embed overflow');
    await mini.screenshot({path:'output/playwright/cloudzao-claw154-mobile.png',fullPage:true});
  }finally{await mini.close();}
  if(errors.length)throw Error(errors.join('\n'));
  return {...proof,pageErrors:errors};
}
