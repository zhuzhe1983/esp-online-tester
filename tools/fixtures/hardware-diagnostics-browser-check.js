// Run via playwright_cli.sh run-code --filename. Worker data is a structural
// test double; this checks UI/API behavior, not firmware or physical hardware.
async page => {
  const root=new URL('./',page.url()).href,errors=[],results=[];
  const assert=(ok,message)=>{if(!ok)throw Error(message);};page.on('pageerror',error=>errors.push(error.message));
  await page.addInitScript(()=>{
    if(window.top!==window)return;
    localStorage.setItem('esp-online-tester.locale','zh-CN');window.diagnosticWorkers=[];
    window.Worker=class {
      constructor(){window.diagnosticWorkers.push(this);}
      terminate(){}
      postMessage(m){let reply;
        if(m.op==='init')reply={ready:true};if(m.op==='create')reply={created:true,flashStorage:false};
        if(m.op==='load')reply={loaded:m.at===undefined?m.kind:`at${m.at}`,ok:true};
        if(m.op==='start')reply={started:true};if(m.op==='resume')reply={resumed:true};if(m.op==='stop')reply={stopped:0};
        if(m.op==='imu')reply={imuApplied:m.inputId,ok:true,time:0};
        if(reply)queueMicrotask(()=>this.onmessage?.({data:reply}));
      }
      emit(value){this.onmessage?.({data:{text:JSON.stringify({...value,t:'hardware-diagnostics'})}});}
    };
  });
  await page.route('**/wasm/esp32sim.wasm',route=>route.fulfill({body:Buffer.from([1,2,3])}));
  await page.route('**/wasm/fw/*_rom.elf',route=>route.fulfill({body:Buffer.from([1,2,3])}));
  const prepare=async()=>{
    await page.waitForFunction(()=>Boolean(window.espTester));
    await page.evaluate(async()=>{
      const api=window.espTester;
      for(let attempt=0;!(await api.observe()).scene;attempt++){if(attempt===500)throw Error('Initial scene was not configured');await new Promise(resolve=>setTimeout(resolve,10));}
      const c=await import('./catalog.js');await api.configure(c.createDefaultScene(await c.loadCatalog(),'m5stack-stopwatch'));
    });
  };
  const load=async()=>page.evaluate(async()=>{
    const bytes=new Uint8Array(0x10100),view=new DataView(bytes.buffer);
    for(const at of [0,0x10000]){bytes[at]=0xe9;bytes[at+1]=1;bytes[at+12]=9;view.setUint32(at+28,4,true);}
    bytes[0x8000]=0xaa;bytes[0x8001]=0x50;view.setUint32(0x8004,0x10000,true);view.setUint32(0x8008,0x100000,true);
    await espTester.loadFirmware({file:new Blob([bytes])});await espTester.start();
  });
  const value={schemaVersion:1,board:'stopwatch',state:{powerLedEnabled:false,downloadLocked:true,pm1ConfiguredHz:100000,pm1LastBusHz:400000,
    display:{sleepOut:true,displayOn:true,brightness:160,brightnessControlEnabled:null,visible:true}},
    findings:[{code:'PM1_I2C_CLOCK_MISMATCH',severity:'warning',count:2,summary:'Clock mismatch',detail:'Host 400000 Hz, PM1 100000 Hz'},
      {code:'PM1_DOWNLOAD_LOCK',severity:'warning',count:1,summary:'Download locked',detail:'DL_LOCK bit 7 set'},
      {code:'PM1_POWER_LED_DISABLED',severity:'info',count:1,summary:'Power LED disabled',detail:'LED_EN cleared'}],limits:['Wake timing and physical tolerances are not fully modeled.']};
  const push=async(v=value)=>page.evaluate(data=>diagnosticWorkers.at(-1).emit(data),v);
  await page.setViewportSize({width:1440,height:1000});await page.goto(new URL('?hardware-diagnostics-test=1#autostart=0',root).href);await prepare();
  const card=page.locator('.hardware-diagnostics');await card.waitFor();
  assert((await card.innerText()).includes('等待固件运行'),'No initial pass claim');await load();await push();
  await page.waitForFunction(()=>document.querySelector('.hardware-check-status').textContent==='2 项需检查');
  await card.locator('summary').click();
  assert(await card.locator('.hardware-check-finding.warning').count()===2,'Two warning findings');assert(await card.locator('.hardware-check-finding.info').count()===1,'LED disable is information');
  assert((await card.innerText()).includes('未明确配置'),'Unknown brightness control stays unknown');
  assert((await card.innerText()).includes('仅覆盖已建模检查，仍需实机验证'),'Limited scope remains visible');
  assert((await page.evaluate(()=>espTester.observe())).serial==='','No injected guest UART logs');
  await push();assert(await card.locator('details').getAttribute('open')!==null,'Duplicate snapshot preserves expansion');
  await page.screenshot({path:'output/playwright/hardware-diagnostics-desktop.png'});results.push('workbench findings, info/warning split, scope, unknown state, duplicate stability');
  await page.setViewportSize({width:390,height:844});
  await card.scrollIntoViewIfNeeded();assert(await card.evaluate(n=>n.getBoundingClientRect().right<=innerWidth),'Mobile card fits viewport');
  await page.screenshot({path:'output/playwright/hardware-diagnostics-mobile.png'});results.push('390px layout');
  await page.evaluate(async()=>{window.oldDiagnosticWorker=diagnosticWorkers.at(-1);await espTester.reset();});
  assert((await page.evaluate(()=>espTester.observe())).hardwareDiagnostics===null,'Reset clears data');await page.evaluate(data=>oldDiagnosticWorker.emit(data),value);
  assert((await page.evaluate(()=>espTester.observe())).hardwareDiagnostics===null,'Retired worker cannot restore diagnostics');
  await push();await page.evaluate(async()=>{const c=await import('./catalog.js');await espTester.configure(c.createDefaultScene(await c.loadCatalog(),'m5stack-k134'));});
  assert(await card.isHidden(),'Unsupported board hides check card');results.push('reset and product lifecycle');
  await page.goto(new URL('embed.html#autostart=0',root).href);await prepare();await load();await push();
  await card.waitFor();assert(await page.locator('.embed-console').getAttribute('open')===null,'Mini console starts collapsed');
  assert(await card.isVisible(),'Mini hardware warning is visible outside collapsed serial console');await card.locator('summary').click();
  await card.scrollIntoViewIfNeeded();await page.screenshot({path:'output/playwright/hardware-diagnostics-mini.png'});results.push('mini warning visible outside serial disclosure');
  const hostile={...value,findings:[{code:'UNKNOWN_CHECK',severity:'warning',count:1,summary:'<img src=x onerror=alert(1)>',detail:'<script>alert(1)</script>'}]};await push(hostile);
  assert(await card.locator('img,script').count()===0,'Worker strings render as text');assert((await card.innerText()).includes('<script>'),'Evidence text stays literal');
  await push({...value,findings:[]});assert((await card.innerText()).includes('当前未发现已覆盖的问题'),'Empty findings do not claim hardware compatibility');
  await page.evaluate(()=>espTester.dispose());assert(errors.length===0,JSON.stringify(errors));return results;
}
