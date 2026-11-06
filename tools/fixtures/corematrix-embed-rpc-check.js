// Run after corematrix-browser-check.js inside an iframe. Only the public
// parent postMessage bridge is used to observe and capture the running guest.
async page => page.evaluate(async()=>{
 const frame=document.querySelector('iframe'),origin=new URL(frame.src).origin;
 const session=crypto.randomUUID();let sequence=0;
 const request=(type,method,args=[])=>new Promise((resolve,reject)=>{
  const requestId='check-'+(++sequence),timer=setTimeout(()=>{window.removeEventListener('message',receive);reject(Error('Embed RPC timed out'));},10000);
  function receive(event){const m=event.data;if(event.source!==frame.contentWindow||event.origin!==origin||m?.channel!=='esp-workbench'||m.session!==session||m.requestId!==requestId)return;clearTimeout(timer);window.removeEventListener('message',receive);m.error?reject(Error(m.error.message)):resolve(m.result);}
  window.addEventListener('message',receive);frame.contentWindow.postMessage({channel:'esp-workbench',version:'1',session,requestId,type,method,args},origin);
 });
 const hello=await request('hello'),state=await request('call','observe'),capture=await request('call','capture',[{format:'rgba'}]);
 const bleConfig=await request('call','configureBle',[{nearby:[]}]),ble=await request('call','readBle');
 if(bleConfig.nearbyCount!==0||ble.source!=='guest-controller-DMA')throw Error('BLE methods are not exposed through the parent bridge');
 const indices=state.leds.flatMap((rgb,i)=>rgb.some(v=>v>0)?[i]:[]);
 if(state.product.id!=='m5stack-c158'||JSON.stringify(indices)!=='[0,35,52,71,88,107,124,255]'||capture.width!==16||capture.height!==16)throw Error('Embedded guest output differs');
 return {productId:state.product.id,bridgeVersion:hello.version,display:[capture.width,capture.height],litIndices:indices,sourceSha256:state.scene.firmware.sha256,originAndSessionHandshake:true,bleMethods:true};
})
