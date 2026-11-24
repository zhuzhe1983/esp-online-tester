// The caller supplies ?firmware= with the original repository SRAM diagnostic.
async page => {
  const firmware=new URL(page.url()).searchParams.get('firmware');
  if(!firmware)throw Error('Supply ?firmware= with waveshare-c5-lcd147-probe.bin.');
  const result=await page.evaluate(async firmware=>{
    const api=espTester,{loadCatalog,createDefaultScene}=await import('./catalog.js');
    await api.configure(createDefaultScene(await loadCatalog(),'waveshare-esp32-c5-lcd-1-47'));
    await api.loadFirmware({url:new URL(firmware,location.href).href,bootMode:'app-direct',sha256:'89bb4d4c0c38c165c341fab0f3dcacce051a524641066de5a06d03443b99a754'});await api.start();
    const serial=s=>s.serialByPort?.uart0||'',wait=async fn=>{const end=Date.now()+90000;while(Date.now()<end){const s=await api.observe();if(s.status==='error')throw Error(JSON.stringify(s.lastError));if(fn(s))return s;await new Promise(r=>setTimeout(r,20));}throw Error('Guest output timed out');};
    let state=await wait(s=>serial(s).includes('C5 LCD147 READY SD PASS\n')&&s.frame);const ready=state.stats.time;
    await wait(s=>s.stats.time>=ready+.05);const initial=await api.capture({format:'rgba'});
    if(initial.width!==172||initial.height!==320)throw Error('Wrong physical framebuffer');
    const changes=(a,b)=>{let n=0;for(let i=0;i<a.length;i+=4)if(a[i]!==b[i]||a[i+1]!==b[i+1]||a[i+2]!==b[i+2]||a[i+3]!==b[i+3])n++;return n;};
    const capture=async changed=>{const end=Date.now()+10000;while(Date.now()<end){const frame=await api.capture({format:'rgba'});if(changes(initial.pixels,frame.pixels)===changed)return frame;await new Promise(r=>setTimeout(r,20));}throw Error('Guest pixels not delivered');};
    let mark=serial(await api.observe()).length;await api.input({type:'button',id:'boot',pressed:true});
    await wait(s=>serial(s).slice(mark).includes('C5 BOOT DOWN\n'));const pressed=await capture(140*256);
    mark=serial(await api.observe()).length;await api.input({type:'button',id:'boot',pressed:false});
    await wait(s=>serial(s).slice(mark).includes('C5 BOOT UP\n'));const released=await capture(0);await api.pause();state=await api.observe();
    const sha=async bytes=>[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(v=>v.toString(16).padStart(2,'0')).join('');
    return {productId:state.scene.productId,width:initial.width,height:initial.height,initialSha256:await sha(initial.pixels),pressedSha256:await sha(pressed.pixels),releasedSha256:await sha(released.pixels),changedPixels:140*256,releaseRestoredExactly:true,guestSdBlockRestoreMarker:serial(state).includes('C5 SD ORIGINAL BLOCK RESTORED\n'),factoryFirmwareVerified:false};
  },firmware);
  await page.waitForFunction(()=>document.querySelector('#scene-mount').dataset.modelStatus==='ready');
  await page.screenshot({path:'output/playwright/waveshare-c5-packed-real-frame.png',fullPage:true});
  return result;
}
