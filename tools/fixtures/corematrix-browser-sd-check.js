// Use a fresh task-owned browser profile and a loaded original CoreMatrix image.
// This fixture writes one test file to that profile's virtual SD card.
async page => {
 let child=page.frames().find(frame=>frame.parentFrame()&&frame.url().includes('/platform/embed.html'))||page;
 const phase=async(write)=>child.evaluate(async write=>{
  const api=window.espTester,wait=async predicate=>{const end=Date.now()+120000;while(Date.now()<end){const state=await api.observe();if(state.status==='error')throw Error(JSON.stringify(state.lastError));if(predicate(state))return state;await new Promise(r=>setTimeout(r,25));}throw Error('SD guest timeout');};
  const advance=async s=>{const at=(await api.observe()).stats.time;await wait(state=>state.stats.time>=at+s);},send=data=>api.input({type:'serial',port:'usb',data});
  await api.start();await wait(state=>state.stats.time>1);let begin=(await api.observe()).serial.length;
  await send('\x02\r\x03\x03');await advance(.15);await send('\x01');await wait(state=>state.serial.slice(begin).includes('raw REPL; CTRL-B to exit\r\n>'));
  const source='import M5,os\nfrom machine import I2C,Pin,SDCard\nfrom driver.m5ioe1 import M5IOE1\nM5.begin()\nbus=I2C(0,sda=Pin(0),scl=Pin(1),freq=400000)\nio=M5IOE1(bus,addr=0x4f)\nio.set_pin_mode(1,1)\nio.set_pin_drive(1,0)\nio.set_pin_value(1,True)\nsd=SDCard(slot=2,sck=Pin(25),miso=Pin(26),mosi=Pin(27),cs=Pin(28),freq=1000000)\nos.mount(sd,"/sd")\n'+(write?'with open("/sd/corematrix-browser.txt","w") as f:\n f.write("COREMATRIX_BROWSER_PERSISTENCE")\nos.sync()\n':'')+'with open("/sd/corematrix-browser.txt") as f:\n assert f.read()=="COREMATRIX_BROWSER_PERSISTENCE"\nprint("CM_BROWSER_SD_OK")\n';
  begin=(await api.observe()).serial.length;for(let i=0;i<source.length;i+=64){await send(source.slice(i,i+64));await advance(.01);}await send('\x04');
  const state=await wait(state=>state.serial.slice(begin).includes('\x04\x04>')),out=state.serial.slice(begin);if(out.includes('Traceback')||!out.includes('CM_BROWSER_SD_OK'))throw Error(out);
  await api.pause();await api.flushSdCard();const done=await api.observe();if(done.sdCard.backend!=='indexeddb')throw Error('Browser SD persistence must use IndexedDB');
  return {phase:write?'write':'reload-read',firmwareSha256:done.scene.firmware.sha256,sdCard:done.sdCard,guestSeconds:done.stats.time};
 },write);
 const first=await phase(true),url=new URL(child.url());url.searchParams.set('storage-check',String(Date.now()));await child.goto(url.href);await child.waitForFunction(async()=>window.espTester&&(await window.espTester.observe()).product?.id==='m5stack-c158',null,{timeout:60000});
 await child.evaluate(async()=>{const {configureFirmwareRelay}=await import('./firmware-relay.js');configureFirmwareRelay(new URL(location.href).searchParams.get('test-relay'));await window.espTester.loadFirmware();});
 const second=await phase(false);if(!second.sdCard.restored)throw Error('Reload did not restore the saved SD pages');
 return {productId:'m5stack-c158',firmwarePatched:false,backend:'production-browser-worker-WASM',pageReloadPersistence:true,phases:[first,second]};
}
