// Headless UI regression. All downloads and Serial APIs are injected fixtures.
async page => {
  const assert=(value,message)=>{if(!value)throw Error(message);},results=[],errors=[];
  page.on('pageerror',error=>errors.push(error.message));
  await page.goto(new URL('index.html',page.url()).href);
  await page.evaluate(async()=>{
    const {openFlashStation}=await import('./firmware-flash-ui.js'),{createFlashBatch}=await import('./flash-batch.js'),{loadCatalog,createDefaultScene}=await import('./catalog.js');
    const {prepareBurnerSource}=await import('./m5burner-download-ui.js'),{createM5BurnerClient}=await import('./m5burner.js');
    const catalog=await loadCatalog(),product=catalog.products.find(p=>p.id==='m5stack-stopwatch'),scene=createDefaultScene(catalog,product.id);
    const bytes=new Uint8Array(0x10040).fill(255),view=new DataView(bytes.buffer);
    for(const at of [0,0x10000]){bytes.fill(0,at,at+64);bytes[at]=0xe9;bytes[at+1]=1;view.setUint16(at+12,9,true);view.setUint32(at+28,4,true);}
    view.setUint16(0x8000,0x50aa,true);bytes[0x8002]=0;bytes[0x8003]=0;view.setUint32(0x8004,0x10000,true);view.setUint32(0x8008,0x100000,true);
    const file=new File([bytes],'recovery-fixture.bin'),serial=new EventTarget();
    window.recoveryTest={usb:0,calls:[],metadata:0,prompts:0,attempts:0};serial.requestPort=()=>{recoveryTest.usb++;throw Error('No hardware in this test');};
    const options={product,scene,catalog,createBatch:options=>createFlashBatch({...options,navigator:{serial}})};
    window.openRecovery=async kind=>{
      await window.recoveryStation?.dispose();
      if(kind==='retry') {
        window.recoveryStation=openFlashStation({...options,source:{file},prepareSource:async source=>{if(++recoveryTest.attempts===1)throw Error('Fixture download unavailable');return source;}});return;
      }
      if(kind==='cancel') {
        window.recoveryStation=openFlashStation({...options,source:{file},prepareSource:source=>++recoveryTest.attempts===1?new Promise(resolve=>{window.releaseLateSource=()=>resolve(source);}):Promise.resolve(source)});return;
      }
      if(kind==='cancel-batch') {
        const delayed={name:'delayed.bin',size:bytes.length,arrayBuffer:()=>++recoveryTest.attempts===1?new Promise(resolve=>{window.releaseLateFile=()=>resolve(bytes.slice().buffer);}):Promise.resolve(bytes.slice().buffer)};
        window.recoveryStation=openFlashStation({...options,source:{file:delayed}});return;
      }
      const firmwareId='1897200000000002172',old='2116093081161424897',fresh='2117443003011928065',deviceId='1897100000000000022';
      const endpoint=id=>`https://burner.m5stack.com/api/v1/firmwares/${firmwareId}/versions/${id}/download`;
      const fetchImpl=async url=>{recoveryTest.calls.push(url);return url===endpoint(old)?Response.json({data:{errorCode:'VERSION_NOT_FOUND'}},{status:404}):new Response(bytes);};
      const burnerClient=createM5BurnerClient(async url=>{
        recoveryTest.metadata++;
        if(window.deferRecoveryMetadata)return new Promise(resolve=>{window.releaseLateMetadata=()=>resolve(Response.json({code:200,data:[{deviceId,deviceName:'StopWatch'}]}));});
        return Response.json({code:200,data:url.endsWith('/devices')?[{deviceId,deviceName:'StopWatch'}]:[
          {versionId:fresh,versionName:'v2.5.5',publishTime:'2026-11-03',visibility:'PUBLIC',status:'PUBLISHED',supportedDevices:[{deviceId}]},
          {versionId:'999',versionName:'wrong device',visibility:'PUBLIC',status:'PUBLISHED',supportedDevices:[{deviceId:'998'}]},
          {versionId:'997',versionName:'private',visibility:'PRIVATE',status:'PUBLISHED',supportedDevices:[{deviceId}]},
        ]});
      });
      window.recoveryStation=openFlashStation({...options,source:{url:endpoint(old),sha256:'a'.repeat(64)},burnerClient,prepareSource:(source,{signal})=>prepareBurnerSource(source,{signal,fetchImpl,verify:()=>{recoveryTest.prompts++;throw Error('Unexpected CAPTCHA');}})});
    };
  });
  const dialog=page.getByRole('dialog',{name:'烧录到设备'}),retry=dialog.getByRole('button',{name:'重试准备固件',exact:true}),refresh=dialog.getByRole('button',{name:'刷新版本列表',exact:true});
  await page.evaluate(()=>openRecovery('retry'));await retry.waitFor();
  assert(!await page.evaluate(()=>recoveryStation.observe().busy),'Source failure clears busy state');
  assert(await dialog.getByRole('button',{name:'新建批次',exact:true}).isEnabled(),'Failure permits new batch');
  assert(!(await dialog.innerText()).includes('不符合已支持'),'Unprepared source is not called unsupported');
  assert(!(await dialog.innerText()).includes('本批原始固件已固定'),'Unprepared bytes are not claimed pinned');
  await retry.click();await page.waitForFunction(()=>recoveryStation.observe().status==='ready');
  assert(await dialog.getByRole('button',{name:'添加设备',exact:true}).isEnabled(),'Same-window retry reaches prepared state');
  results.push('Preparation failure releases controls; retry succeeds; layout and pin claims wait for receipt');
  await page.evaluate(()=>openRecovery('removed'));await refresh.waitFor();
  assert((await dialog.getByRole('alert').innerText()).includes('版本已不存在'),'Missing release is explained');
  assert(await page.evaluate(()=>recoveryTest.prompts)===0,'404 does not prompt for CAPTCHA');
  await refresh.click();await dialog.getByRole('combobox',{name:'可用固件版本'}).waitFor();
  assert(await dialog.getByRole('combobox',{name:'可用固件版本'}).locator('option').count()===2,'Only matching public version and placeholder');
  const use=dialog.getByRole('button',{name:'使用所选版本并重新准备',exact:true});assert(await use.isDisabled(),'Refreshing never selects or downloads a replacement automatically');
  assert(await page.evaluate(()=>recoveryTest.calls.length)===1,'Version metadata does not download another binary');
  await page.setViewportSize({width:390,height:844});await page.screenshot({path:'output/playwright/flash-version-recovery-mobile.png'});
  const dimensions=await dialog.evaluate(el=>({width:el.scrollWidth,client:el.clientWidth}));assert(dimensions.width<=dimensions.client,'Version recovery fits mobile width');
  await dialog.getByRole('combobox',{name:'可用固件版本'}).selectOption('2117443003011928065');await use.click();await page.waitForFunction(()=>recoveryStation.observe().status==='ready');
  assert((await dialog.locator('.flash-source').innerText()).includes('2117443003011928065'),'Explicit selection replaces URL');
  assert(await page.evaluate(()=>recoveryTest.calls.length)===2,'Explicit choice downloads only its selected version');
  assert(!(await dialog.locator('.flash-fingerprint').innerText()).includes('aaaaaaaa'),'Old source hash is not inherited');
  assert(await page.evaluate(()=>recoveryTest.usb)===0,'Recovery never selects or writes a USB device');
  results.push('Removed pinned release: no CAPTCHA, current public/device-filtered choices, explicit replacement, fresh hash, no USB, mobile fit');
  await page.evaluate(()=>{recoveryTest.attempts=0;return openRecovery('cancel');});
  await dialog.getByRole('button',{name:'取消准备',exact:true}).click();await retry.click();await page.waitForFunction(()=>recoveryStation.observe().status==='ready');
  const fingerprint=await dialog.locator('.flash-fingerprint').innerText();await page.evaluate(()=>releaseLateSource());
  assert(await dialog.locator('.flash-fingerprint').innerText()===fingerprint,'Late cancelled attempt cannot replace retry receipt');
  await page.evaluate(()=>{recoveryTest.attempts=0;return openRecovery('cancel-batch');});await page.waitForFunction(()=>Boolean(window.releaseLateFile));
  await dialog.getByRole('button',{name:'取消准备',exact:true}).click();
  assert(await retry.isDisabled(),'Retry waits for the cancelled batch preparation to release its lock');
  assert((await dialog.locator('.flash-station-status').innerText()).includes('等待当前校验结束'),'Cancellation wait is explained');
  await page.evaluate(()=>releaseLateFile());await page.waitForFunction(()=>!recoveryStation.observe().busy);
  await retry.click();await page.waitForFunction(()=>recoveryStation.observe().status==='ready');
  assert(await dialog.getByRole('alert').isHidden(),'Immediate retry after cancellation does not hit FLASH_BUSY');
  await page.evaluate(()=>{window.deferRecoveryMetadata=true;return openRecovery('removed');});await refresh.click();await page.waitForFunction(()=>Boolean(window.releaseLateMetadata));
  await dialog.getByRole('button',{name:'取消准备',exact:true}).click();await page.evaluate(()=>releaseLateMetadata());
  assert(await dialog.getByRole('combobox',{name:'可用固件版本'}).isHidden(),'Cancelled metadata result is discarded');
  assert(!await page.evaluate(()=>recoveryStation.observe().busy),'Cancelled metadata releases controls');
  await page.evaluate(()=>recoveryStation.dispose());assert(!errors.length,JSON.stringify(errors));
  results.push('Cancelled preparation and metadata discard late responses across retries');return results;
}
