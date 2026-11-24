async page => {
 const ids=["waveshare-esp32-s3-nano", "waveshare-esp32-s3-nano-m", "waveshare-esp32-s3-zero", "waveshare-esp32-s3-zero-m", "waveshare-esp32-s3-zero-n8r8", "waveshare-esp32-s3-zero-n8r8-m", "waveshare-esp32-s3-dev-kit-n8r8", "waveshare-esp32-s3-dev-kit-n8r8-m", "waveshare-esp32-s3-dev-kit-n16r8", "waveshare-esp32-s3-dev-kit-n16r8-m", "waveshare-esp32-s3-dev-kit-n32r16v", "waveshare-esp32-s3-dev-kit-n32r16v-m", "waveshare-esp32-s3-dev-kit-n16r8-u", "waveshare-esp32-s3-dev-kit-n16r8-um", "waveshare-esp32-s3-pico", "waveshare-esp32-s3-pico-m"],base=new URL('./',page.url()),rows=[],errors=[];
 await page.waitForFunction(()=>!!window.espTester);
 const hostUrl=new URL('batch030-iframe-qa.html',base).href,context=page.context();
 await context.route(hostUrl,r=>r.fulfill({contentType:'text/html',body:'<!doctype html><html><body style="margin:0"><iframe id="player" style="width:390px;height:844px;border:0" title="ESP player"></iframe></body></html>'}));
 const host=await context.newPage();host.on('pageerror',e=>errors.push(e.message));
 try {
  for(const id of ids){
   await page.evaluate(async id=>{const {loadCatalog,createDefaultScene}=await import('./catalog.js');await espTester.configure(createDefaultScene(await loadCatalog(),id));},id);
   const shared=await page.evaluate(()=>espTester.share({mode:'embed',autostart:false,autoplay:false}));
   await host.goto(hostUrl);await host.locator('#player').evaluate((f,url)=>{f.src=url;},shared);
   const frame=await (async()=>{for(let i=0;i<100;i++){const f=host.frames().find(f=>f.url()===shared);if(f)return f;await host.waitForTimeout(50);}throw Error('iframe missing');})();
   await frame.waitForFunction(()=>!!window.espTester&&document.querySelector('#scene-mount')?.dataset.modelStatus==='ready',null,{timeout:60000});
   const result=await host.evaluate(async()=>{
    const frame=document.querySelector('#player'),origin=new URL(frame.src).origin,session='batch030_qa';
    async function request(type,requestId,method,args=[]){return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{window.removeEventListener('message',receive);reject(Error('RPC timeout'));},15000);function receive(event){const d=event.data;if(event.source!==frame.contentWindow||event.origin!==origin||d?.channel!=='esp-workbench'||d.version!=='1'||d.session!==session||d.requestId!==requestId)return;clearTimeout(timer);window.removeEventListener('message',receive);d.error?reject(Error(JSON.stringify(d.error))):resolve(d.result);}window.addEventListener('message',receive);frame.contentWindow.postMessage({channel:'esp-workbench',version:'1',type,session,requestId,method,args},origin);});}
    const hello=await request('hello','hello'),s=await request('call','observe','observe');return {apiVersion:hello.version,productId:s.product.id,firmware:s.scene.firmware,status:s.status,mode:s.scene.view.mode,lastError:s.lastError};
   });
   if(result.productId!==id||result.firmware!==null||result.lastError||result.mode!=='embed')throw Error(id+' unexpected iframe result '+JSON.stringify(result));
   rows.push(result);
  }
  if(errors.length)throw Error(JSON.stringify(errors));return {scope:'16 exact Pages-packed scenes in390px actual iframes; origin/session-bound hello+observe; no firmware loaded',results:rows,pageErrors:errors};
 }finally{await host.close();await context.unroute(hostUrl);}
}
