// Run with playwright_cli.sh run-code --filename against a task-owned local page.
// Serial ports and esptool are test doubles; this fixture never touches hardware.
async page => {
  const assert=(condition,message)=>{if(!condition)throw Error(message);},root=new URL('./',page.url()).href,results=[];
  await page.route('**/platform/**',route=>route.continue());
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.addInitScript(()=>{
    if(window.top!==window)return;
    localStorage.setItem('esp-online-tester.locale','zh-CN');
    const serial=new EventTarget(),ports=new Map();
    window.flashTest={chooserCalls:0,activation:[],next:null,active:0,maxActive:0,writes:[],closed:[],delay:20};
    serial.requestPort=()=>{
      flashTest.chooserCalls++;flashTest.activation.push(navigator.userActivation.isActive);
      if(!flashTest.next)return Promise.reject(new DOMException('Test chooser cancelled','NotFoundError'));
      const id=flashTest.next;flashTest.next=null;
      if(!ports.has(id))ports.set(id,{id,getInfo:()=>({usbVendorId:0x303a,usbProductId:0x1001})});
      return Promise.resolve(ports.get(id));
    };
    window.flashTestConnection=(id,connected)=>{const event=new Event(connected?'connect':'disconnect');Object.defineProperty(event,'port',{value:ports.get(id)});serial.dispatchEvent(event);};
    Object.defineProperty(navigator,'serial',{configurable:true,value:serial});
  });
  await page.route('**/vendor/esptool-js-0.7.0/bundle.js',route=>route.fulfill({contentType:'text/javascript',body:`
    export class Transport {constructor(port){this.port=port;}async disconnect(){flashTest.closed.push(this.port.id);}async setDTR(){}async setRTS(){}}
    export class ESPLoader {
      constructor(options){this.options=options;this.port=options.transport.port;this.chip={CHIP_NAME:this.port.id==='wrong-chip'?'ESP32-S31':'ESP32-C6'};}
      async main(){this.options.terminal.writeLine('Connected to test ESP32-C6');}
      async getSecurityInfo(){return {flashCryptCnt:0,parsedFlags:{}};}
      async detectFlashSize(){return '16MB';}flashSizeBytes(){return 16*1024*1024;}
      async writeFlash(options){
        if(options.eraseAll||options.flashMode!=='keep')throw Error('Unsafe test options');
        flashTest.active++;flashTest.maxActive=Math.max(flashTest.maxActive,flashTest.active);
        try{for(let step=1;step<=5;step++){await new Promise(r=>setTimeout(r,flashTest.delay));options.reportProgress(0,step,5);}
          if(this.port.id==='fail-once'&&!this.port.failed){this.port.failed=true;throw Error('Injected write verification failure');}
          flashTest.writes.push({id:this.port.id,hash:options.calculateMD5Hash(options.fileArray[0].data)});
        }finally{flashTest.active--;}
      }
      async after(){await this.options.resetConstructors.hardReset(this.options.transport,false,async()=>{}).reset();}
    }
  `}));
  await page.setViewportSize({width:1440,height:1000});await page.goto(root);await page.waitForFunction(()=>Boolean(window.espTester));
  await page.locator('#product-family-select').selectOption('');await page.locator('#product-select').selectOption('m5stack-u202');
  await page.waitForFunction(()=>document.querySelector('#firmware-select').value==='m5stack-u202-official-factory-reference');
  assert(await page.locator('#flash-device').isDisabled(),'Reference-only source cannot be written');
  await page.locator('#firmware-select').selectOption('m5stack-u202-meshtastic-stable');
  await page.waitForFunction(()=>!document.querySelector('#flash-device').disabled);
  assert(await page.locator('.firmware-actions #flash-device').count()===1,'Independent entry beside Run');
  assert(await page.locator('#load-firmware').isHidden(),'Unavailable simulation does not gate physical flashing');
  const localBytes=new Uint8Array(0x10040).fill(255),view=new DataView(localBytes.buffer);
  for(const at of [0,0x10000]){localBytes.fill(0,at,at+64);localBytes[at]=0xe9;localBytes[at+1]=1;view.setUint16(at+12,13,true);view.setUint32(at+28,4,true);}
  view.setUint16(0x8000,0x50aa,true);localBytes[0x8002]=0;localBytes[0x8003]=0;view.setUint32(0x8004,0x10000,true);view.setUint32(0x8008,0x100000,true);
  await page.getByRole('button',{name:'本地文件',exact:true}).click();
  const selectLocal=async name=>{await page.locator('#clear-local-files').click();await page.locator('#firmware-file').setInputFiles({name,mimeType:'application/octet-stream',buffer:Buffer.from(localBytes)});await page.waitForFunction(()=>document.querySelector('#local-build-status').textContent.includes('已识别完整合并镜像'));};
  await selectLocal('batch-A.bin');
  const dialog=page.getByRole('dialog',{name:'烧录到设备'}),add=dialog.getByRole('button',{name:'添加设备',exact:true});
  await page.locator('#flash-device').click();await add.waitFor();await page.waitForFunction(()=>document.querySelector('.flash-queue-heading button')&&!document.querySelector('.flash-queue-heading button').disabled);
  assert(await dialog.getByRole('button',{name:'开始烧录',exact:true}).isDisabled(),'Start requires devices and acknowledgement');
  assert(await page.evaluate(()=>flashTest.chooserCalls)===0,'Opening a window does not request a port');
  assert(!(await page.locator('.session-details').innerText()).includes('尚未连接硬件'),'Hardware progress is outside details');
  await add.click();await page.waitForFunction(()=>flashTest.chooserCalls===1&&!document.querySelector('.flash-queue-heading button').disabled);
  assert(await dialog.locator('.flash-device').count()===0,'Cancelling chooser creates no device');
  await page.keyboard.press('Escape');assert(await dialog.isHidden(),'Escape hides the window');
  await page.locator('#flash-device').click();await dialog.getByRole('radio',{name:'多台并行'}).check();
  const choose=async id=>{await page.evaluate(id=>{flashTest.next=id;},id);await dialog.locator('.flash-queue-heading button').click();await page.waitForFunction(()=>!document.querySelector('.flash-station-status').textContent.includes('请在浏览器中'));};
  await choose('first');await choose('fail-once');await choose('first');
  assert(await dialog.locator('.flash-device').count()===2,'Duplicate chooser port is not duplicated');
  await dialog.getByRole('checkbox').check();await page.screenshot({path:'output/playwright/flash-station-ready.png',fullPage:false});
  await page.evaluate(()=>flashTest.delay=350);await dialog.getByRole('button',{name:'开始烧录 2 台'}).click();
  await page.waitForFunction(()=>flashTest.active===2);await page.screenshot({path:'output/playwright/flash-station-progress.png',fullPage:false});
  await page.keyboard.press('Escape');assert((await page.locator('#flash-device').innerText()).includes('查看进度'),'Hidden running batch has a visible return entry');
  await selectLocal('batch-B.bin');await page.locator('#flash-device').click();
  assert((await dialog.locator('.flash-source').innerText()).includes('batch-A.bin'),'Scene changes preserve current batch source');
  await page.waitForFunction(()=>document.querySelector('.flash-counts').textContent.includes('成功 1 · 失败 1'));
  assert(await page.evaluate(()=>flashTest.maxActive)===2,'Separate ports wrote concurrently');
  await dialog.getByRole('button',{name:'重试此设备',exact:true}).click();await page.waitForFunction(()=>document.querySelector('.flash-counts').textContent.includes('成功 2 · 失败 0'));
  assert(await page.evaluate(()=>new Set(flashTest.writes.map(row=>row.hash)).size)===1,'Retry keeps the prepared firmware bytes');
  assert(await page.evaluate(()=>flashTest.closed.length)===3,'Every completed/failed device closes its port');
  results.push('Independent modal, chooser activation/cancellation, acknowledgement, duplicate ports, parallel isolation/retry, hidden status, frozen firmware');

  await dialog.getByRole('button',{name:'新建批次',exact:true}).click();await page.waitForFunction(()=>document.querySelector('.flash-queue-heading button')&&!document.querySelector('.flash-queue-heading button').disabled);
  assert((await dialog.locator('.flash-source').innerText()).includes('batch-B.bin'),'New batch reads current selection');
  assert(!(await dialog.getByRole('checkbox').isChecked()),'New batch requires its own acknowledgement');
  await dialog.getByRole('radio',{name:'逐台连续'}).check();await dialog.getByRole('checkbox').check();
  await dialog.getByRole('button',{name:'开启连续烧录'}).click();await page.evaluate(()=>flashTest.delay=20);
  await choose('sequential-a');await page.waitForFunction(()=>document.querySelector('.flash-counts').textContent.includes('成功 1'));
  await page.evaluate(()=>{flashTestConnection('sequential-a',false);flashTestConnection('sequential-a',true);});
  await choose('sequential-a');assert(await dialog.locator('.flash-device').count()===1,'Completed reinserted device is not written twice');
  await choose('sequential-b');await page.waitForFunction(()=>document.querySelector('.flash-counts').textContent.includes('成功 2'));
  assert(await page.evaluate(()=>flashTest.writes.filter(row=>row.id==='sequential-a').length)===1,'Reinserted success has one write');
  await page.setViewportSize({width:390,height:844});await page.screenshot({path:'output/playwright/flash-station-mobile.png',fullPage:false});
  const bounds=await dialog.evaluate(el=>({width:el.scrollWidth,client:el.clientWidth,left:el.getBoundingClientRect().left,right:el.getBoundingClientRect().right}));
  assert(bounds.width<=bounds.client&&bounds.left>=0&&bounds.right<=390,'Modal fits 390px without horizontal overflow');
  await dialog.getByRole('button',{name:'停止本批',exact:true}).click();await page.waitForFunction(()=>document.querySelector('.flash-station-status').textContent.includes('本批已停止'));
  assert(await dialog.getByRole('button',{name:'新建批次',exact:true}).isEnabled(),'Stopped batch can be replaced');
  results.push('Sequential session, same-source replacement, completed-device deduplication, stop, mobile layout');

  await dialog.getByRole('button',{name:'新建批次',exact:true}).click();await page.waitForFunction(()=>document.querySelector('.flash-queue-heading button')&&!document.querySelector('.flash-queue-heading button').disabled);
  await choose('cancel-active');await dialog.getByRole('checkbox').check();await page.evaluate(()=>flashTest.delay=200);
  await dialog.getByRole('button',{name:'开始烧录',exact:true}).click();await page.waitForFunction(()=>flashTest.active===1);
  await dialog.getByRole('button',{name:'停止本批',exact:true}).click();await page.waitForFunction(()=>document.querySelector('.flash-counts').textContent.includes('已取消 1'));
  assert(await page.evaluate(()=>flashTest.active===0&&flashTest.closed.includes('cancel-active')),'Cancel waits for active device cleanup');
  assert(!(await page.evaluate(()=>flashTest.writes.some(row=>row.id==='cancel-active'))),'Cancelled device does not report success');
  await dialog.getByRole('button',{name:'新建批次',exact:true}).click();await page.waitForFunction(()=>document.querySelector('.flash-queue-heading button')&&!document.querySelector('.flash-queue-heading button').disabled);
  await choose('cancel-queued');await dialog.getByRole('button',{name:'取消排队',exact:true}).click();
  assert((await dialog.locator('.flash-mode-hint').innerText()).includes('更换设备'),'Cancelled single-device selection explains how to change the device');
  assert(await dialog.getByRole('button',{name:'新建批次',exact:true}).isEnabled(),'Cancelled selection permits a new batch');
  await dialog.getByRole('checkbox').check();await dialog.getByRole('button',{name:'重试此设备',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('.flash-counts').textContent.includes('成功 1'));
  assert(await page.evaluate(()=>flashTest.writes.filter(row=>row.id==='cancel-queued').length)===1,'A cancelled queued device can be retried with confirmation');
  await page.keyboard.press('Escape');
  assert(await page.evaluate(()=>flashTest.activation.every(Boolean)),'Every chooser retained a user activation');
  results.push('Mid-write batch cancellation and port cleanup');

  await page.locator('#flash-device').click();
  await dialog.getByRole('button',{name:'新建批次',exact:true}).click();await page.waitForFunction(()=>!document.querySelector('.flash-queue-heading button').disabled);
  await choose('wrong-chip');await dialog.getByRole('checkbox').check();await dialog.getByRole('button',{name:'开始烧录',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('.flash-counts').textContent.includes('失败 1'));
  const failed=dialog.locator('.flash-device'),replace=failed.getByRole('button',{name:'更换设备',exact:true});
  assert((await failed.innerText()).includes('ESP32-S31')&&(await failed.innerText()).includes('未写入 Flash'),'Chip mismatch explains the wrong selection and blocked write');
  assert(await replace.isEnabled()&&await failed.getByRole('button',{name:'重试此设备',exact:true}).isEnabled(),'Failed single-device batch offers retry and replacement');
  assert(!(await page.evaluate(()=>flashTest.writes.some(row=>row.id==='wrong-chip'))),'Wrong chip receives no firmware write');
  await page.screenshot({path:'output/playwright/flash-station-replace-error.png',fullPage:false});
  const beforeReplacement=await page.evaluate(()=>({writes:flashTest.writes.length,closed:flashTest.closed.length})),pinned=await dialog.locator('.flash-fingerprint').innerText();
  await replace.click();await page.waitForFunction(()=>!document.querySelector('.flash-station-status').textContent.includes('请在浏览器中'));
  assert((await failed.innerText()).includes('ESP32-S31'),'Cancelling replacement preserves the original failure');
  await page.evaluate(()=>flashTest.next='wrong-chip');await replace.click();
  await page.waitForFunction(()=>!document.querySelector('.flash-station-error').hidden);
  assert((await dialog.getByRole('alert').innerText()).includes('仍是原设备'),'Choosing the same port explains how to recover');
  await page.evaluate(()=>flashTest.next='corrected-device');await replace.click();
  await failed.getByRole('button',{name:'烧录此设备',exact:true}).waitFor();
  assert(await failed.count()===1&&(await dialog.locator('.flash-counts').innerText()).includes('等待 1'),'Replacement reuses the row and returns it to waiting');
  assert(await dialog.locator('.flash-fingerprint').innerText()===pinned,'Replacement preserves the pinned firmware');
  assert(await page.evaluate(before=>flashTest.writes.length===before.writes&&flashTest.closed.length===before.closed,beforeReplacement),'Choosing replacement alone does not connect or write');
  assert(await dialog.getByRole('alert').isHidden(),'A successful selection clears the previous chooser error');
  await page.screenshot({path:'output/playwright/flash-station-replace-ready.png',fullPage:false});
  const replacementBounds=await dialog.evaluate(el=>({width:el.scrollWidth,client:el.clientWidth}));
  assert(replacementBounds.width<=replacementBounds.client,'Replacement actions fit the mobile modal');
  await failed.getByRole('button',{name:'烧录此设备',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('.flash-counts').textContent.includes('成功 1 · 失败 0'));
  assert(await page.evaluate(()=>flashTest.writes.filter(row=>row.id==='corrected-device').length)===1,'Explicit action writes the replacement once');
  await page.keyboard.press('Escape');
  assert(await page.evaluate(()=>flashTest.activation.every(Boolean)),'Replacement chooser retains user activation');
  results.push('Single-device chip mismatch recovery, cancelled/same-port chooser, explicit replacement write, pinned firmware and mobile layout');

  // Remote local-server fixture makes the same UI reachable in the embedded player.
  await page.route('**/__flash-test.bin',route=>route.fulfill({contentType:'application/octet-stream',body:Buffer.from(localBytes)}));
  const mini=await page.evaluate(async()=>{const {loadCatalog,createDefaultScene,encodeScene}=await import('./catalog.js'),catalog=await loadCatalog(),scene=createDefaultScene(catalog,'m5stack-u202');scene.firmware={url:new URL('./__flash-test.bin',location.href).href};return new URL('embed.html#autostart=0&scene='+encodeScene(scene),location.href).href;});
  await page.setViewportSize({width:920,height:900});await page.goto(mini);await page.waitForFunction(()=>Boolean(window.espTester)&&Boolean(document.querySelector('#flash-device')));
  assert(await page.locator('.transport-main #flash-device').isEnabled(),'Embedded entry remains beside Start');
  await page.locator('#flash-device').click();await page.waitForFunction(()=>document.querySelector('.flash-queue-heading button')&&!document.querySelector('.flash-queue-heading button').disabled);await dialog.waitFor();
  assert(await dialog.getByRole('radio',{name:'多台并行'}).count()===1,'Embedded player uses the same batch window');
  await page.screenshot({path:'output/playwright/flash-station-mini.png',fullPage:false});await page.keyboard.press('Escape');
  results.push('Embedded player shares station UI');

  await page.goto(root);await page.waitForFunction(()=>Boolean(window.espTester));
  await page.evaluate(async()=>{
    const {openFlashStation}=await import('./firmware-flash-ui.js');window.delayedPrepareCalls=0;
    window.delayedStation=openFlashStation({product:{name:'Delayed preparation fixture'},source:{url:'https://firmware.invalid/test.bin'},scene:{},catalog:{},
      prepareSource:()=>new Promise(resolve=>{window.releaseDelayedSource=resolve;}),
      createBatch:({onEvent})=>({prepare(){delayedPrepareCalls++;},stop(){onEvent({type:'state',state:{status:'stopped',items:[],counts:{},running:false,choosing:false}});},dispose(){}}),
    });
  });
  await dialog.getByRole('button',{name:'取消准备',exact:true}).click();
  await page.evaluate(()=>releaseDelayedSource({url:'https://firmware.invalid/test.bin'}));
  await page.waitForFunction(()=>!delayedStation.observe().busy);
  assert(await page.evaluate(()=>delayedPrepareCalls)===0,'A source that resolves after cancellation never starts batch.prepare');
  assert(await dialog.getByRole('button',{name:'添加设备',exact:true}).isDisabled(),'Cancelled preparation cannot select or write devices');
  await page.evaluate(()=>delayedStation.dispose());
  results.push('Cancelled single-device queue can retry; delayed source preparation is discarded after cancellation');
  assert(!errors.length,JSON.stringify(errors));return results;
}
