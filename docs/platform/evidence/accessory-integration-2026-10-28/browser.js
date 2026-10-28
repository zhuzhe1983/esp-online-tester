// Open the caller-selected static repository origin before running this callback.
async page => {
 const receipt={checkedAt:new Date().toISOString().replace(/\.\d{3}Z$/,'Z'),cases:[]};
 const poll=async(p,test)=>{for(let n=0;n<700;n++){const s=await p.evaluate(async()=>{const s=await espTester.observe();return {...s,serial:s.serialByPort.uart0||''};});if(s.status==='error')throw Error(JSON.stringify(s.lastError));if(test(s))return s;await new Promise(r=>setTimeout(r,80));}throw Error('Actual firmware output timeout');};
 const query=async(p,marker)=>{if(marker==='ENV4_SAMPLE'){const before=(await p.evaluate(()=>espTester.observe())).serialByPort.uart0.length;await p.evaluate(()=>espTester.input({type:'serial',port:'uart0',data:'p'}));await poll(p,s=>/ENV4_PROBE[^\n]+\n/.test(s.serial.slice(before)));const tick=await p.evaluate(()=>espTester.input({type:'serial',port:'uart0',data:''}));await poll(p,s=>s.stats.time>tick.guestTime+.003);}const at=(await p.evaluate(()=>espTester.observe())).serialByPort.uart0.length;await p.evaluate(()=>espTester.input({type:'serial',port:'uart0',data:'q'}));return poll(p,s=>s.serial.slice(at).includes(marker)&&s.serial.slice(at).trimEnd().includes(marker));};
 const configurations=[{key:'env',id:'m5stack-unit-env4-u001d',ready:'ENV4_BOOT_READY',marker:'ENV4_SAMPLE',parameters:{temperatureC:25,humidityPercent:50,pressureHpa:1013.25}},{key:'dlight',id:'m5stack-unit-dlight-u136',ready:'DLIGHT_READY',marker:'DLIGHT_SAMPLE',parameters:{lux:120}},{key:'pir',id:'m5stack-unit-pir-u004',ready:'U004_POWER_ON',marker:'U004_READ',parameters:{motion:false}}];
 for(const c of configurations){
  await page.goto(new URL('/web/platform/',page.url()).href);await page.waitForFunction(()=>!!window.espTester);
  await page.evaluate(async c=>{const {loadCatalog,createDefaultScene}=await import('./catalog.js');const s=createDefaultScene(await loadCatalog(),'m5stack-stopwatch');const url=new URL(`/output/accessory-integration/fixture-${c.key}/manifest.json`,location.href).href;const bytes=await(await fetch(url)).arrayBuffer();const sha256=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),b=>b.toString(16).padStart(2,'0')).join('');s.firmware={manifestUrl:url,sha256};s.accessories=[{id:'sensor',catalogId:c.id,parameters:c.parameters,wiring:{mode:'physical',interfaceId:'grove'}}];await espTester.configure(s);await espTester.loadFirmware(s.firmware);await espTester.start();},c);
  await poll(page,s=>s.status==='running'&&s.serial.includes(c.ready));
  if(c.key==='env'){
   await page.getByLabel('ENV-IV 温度 (°C)',{exact:true}).fill('22.5');await page.getByLabel('ENV-IV 相对湿度 (%)',{exact:true}).fill('61.2');await page.getByLabel('ENV-IV 气压 (hPa)',{exact:true}).fill('990.1');await page.getByRole('button',{name:'设置环境',exact:true}).click();
   await query(page,c.marker);await poll(page,s=>/ENV4_SAMPLE shtT=22\.[^\n]+\n/.test(s.serial));
  } else if(c.key==='dlight'){
   await page.getByLabel('Unit DLight 照度 lx',{exact:true}).fill('4321.5');await page.getByRole('button',{name:'设置照度',exact:true}).click();const tick=await page.evaluate(()=>espTester.input({type:'serial',port:'uart0',data:''}));await poll(page,s=>s.stats.time>tick.guestTime+.4);await query(page,c.marker);await poll(page,s=>/DLIGHT_SAMPLE[^\n]+lux=4321\.[^\n]+\n/.test(s.serial));
  } else {
   await page.getByRole('button',{name:'Unit PIR 模拟移动',exact:true}).click();await poll(page,s=>/U004_READ G11=1 G10=1 US=\d+\r?\n/.test(s.serial));await page.getByRole('button',{name:'Unit PIR 模拟移动',exact:true}).click();await poll(page,s=>/U004_READ G11=1 G10=1 US=\d+\r?\n[\s\S]*U004_READ G11=0 G10=1 US=\d+\r?\n/.test(s.serial));
  }
  await page.evaluate(()=>espTester.pause());const s=await page.evaluate(()=>espTester.observe());
  const item={key:c.key,productId:s.product.id,studio:{status:s.status,inputs:s.accessoryInputs,serial:s.serialByPort.uart0.split(/\r?\n/).filter(l=>l.startsWith(c.marker))}};
  if(!item.studio.serial.length)throw Error('Missing firmware evidence');
  await page.screenshot({path:`output/playwright/integrated-${c.key}-studio.png`,fullPage:true});
  const share=await page.evaluate(()=>espTester.share({mode:'embed',autostart:true,includeRecording:false}));
  const mini=await page.context().newPage();try{
   await mini.goto(share);await mini.waitForFunction(()=>!!window.espTester);await mini.getByRole('button',{name:'继续运行',exact:true}).click();await poll(mini,s=>s.status==='running'&&s.serial.includes(c.ready));
   if(c.key==='env')await mini.evaluate(()=>espTester.input({type:'accessory-environment',accessoryId:'sensor',temperatureC:13,humidityPercent:41,pressureHpa:851}));
   else if(c.key==='dlight')await mini.evaluate(()=>espTester.input({type:'accessory-illuminance',accessoryId:'sensor',lux:777}));
   else await mini.evaluate(()=>espTester.input({type:'accessory-motion',accessoryId:'sensor',motion:true}));
   const tick=await mini.evaluate(()=>espTester.input({type:'serial',port:'uart0',data:''}));await poll(mini,s=>s.stats.time>tick.guestTime+.4);await query(mini,c.marker);
   await poll(mini,s=>(c.key==='env'?/ENV4_SAMPLE shtT=13\.[^\n]+\n/:c.key==='dlight'?/DLIGHT_SAMPLE[^\n]+lux=776\.[^\n]+\n/:/U004_READ G11=1 G10=1 US=\d+\r?\n/).test(s.serial));
   await mini.evaluate(()=>espTester.pause());const m=await mini.evaluate(()=>espTester.observe());item.mini={automaticStartAfterUnverifiedConfirmation:true,status:m.status,inputs:m.accessoryInputs,serial:m.serialByPort.uart0.split(/\r?\n/).filter(l=>l.startsWith(c.marker))};
   await mini.getByText('配件与 GPS 数据',{exact:true}).click();await mini.screenshot({path:`output/playwright/integrated-${c.key}-mini.png`,fullPage:true});
   await mini.evaluate(()=>espTester.dispose());
  }finally{await mini.close();}
  await page.evaluate(()=>espTester.dispose());receipt.cases.push(item);
 }
 return receipt;
}
