// Task-owned browser only; all serial ports and the loader are protocol doubles.
async page => {
  const assert=(value,message)=>{if(!value)throw Error(message);},root=new URL('./',page.url()).href,errors=[];
  page.on('pageerror',error=>errors.push(error.message));
  // Clear only this fixture's explicit credential store in the task-owned browser.
  await page.evaluate(()=>localStorage.removeItem('esp-online-tester.flash-profiles.v1'));
  await page.addInitScript(()=>{
    if(window.top!==window)return;
    window.configTest={next:null,writes:[],active:0,maxActive:0,delay:100,closed:0};
    const serial=new EventTarget(),ports=new Map();serial.requestPort=async()=>{if(!navigator.userActivation.isActive)throw Error('Missing click');const id=configTest.next;if(!id)throw new DOMException('Cancelled','NotFoundError');configTest.next=null;if(!ports.has(id))ports.set(id,{id,getInfo:()=>({usbVendorId:0x303a,usbProductId:0x1001})});return ports.get(id);};
    Object.defineProperty(navigator,'serial',{configurable:true,value:serial});
  });
  await page.route('**/vendor/esptool-js-0.7.0/bundle.js',route=>route.fulfill({contentType:'text/javascript',body:`
    export class Transport {constructor(port){this.port=port;}async disconnect(){configTest.closed++;}async setDTR(){}async setRTS(){}}
    export class ESPLoader {
      constructor(options){this.options=options;this.port=options.transport.port;this.chip={CHIP_NAME:'ESP32-C6'};}
      async main(){}async getSecurityInfo(){return {flashCryptCnt:0,parsedFlags:{SECURE_BOOT_EN:false,SECURE_DOWNLOAD_ENABLE:false}};}
      async detectFlashSize(){return '16MB';}flashSizeBytes(){return 16*1024*1024;}
      async writeFlash(options){
        configTest.active++;configTest.maxActive=Math.max(configTest.maxActive,configTest.active);
        try{const nvs=options.fileArray.find(f=>f.address===0x9000),values={};
          if(nvs){const b=nvs.data,v=new DataView(b.buffer),decoder=new TextDecoder();for(let slot=0;slot<126;){if(((b[32+Math.floor(slot/4)]>>((slot%4)*2))&3)!==2){slot++;continue;}const at=64+slot*32,span=b[at+2],key=decoder.decode(b.subarray(at+8,at+24)).replace(/\\0.*$/s,'');if(b[at]===1&&b[at+1]===33)values[key]=decoder.decode(b.subarray(at+32,at+32+v.getUint16(at+24,true)-1));slot+=span;}}
          await new Promise(r=>setTimeout(r,configTest.delay));options.reportProgress(options.fileArray.length-1,1,1);configTest.writes.push({id:this.port.id,server:values.server,ssid:values.ssid0,password:values.pswd0});
        }finally{configTest.active--;}
      }
      async after(){await this.options.resetConstructors.hardReset(this.options.transport,false,async()=>{}).reset();}
    }
  `}));
  await page.setViewportSize({width:1280,height:1000});await page.goto(root+'?autostart=0');await page.waitForFunction(()=>!!window.espTester);
  await page.evaluate(async()=>{
    const {openFlashStation}=await import('./firmware-flash-ui.js'),{createDefaultScene}=await import('./catalog.js'),catalog={};
    for(const name of ['products','accessories','firmwares'])catalog[name]=(await (await fetch(`./catalog/${name}.json`)).json()).items;
    const product=catalog.products.find(p=>p.id==='m5stack-u202'),scene=createDefaultScene(catalog,product.id),b=new Uint8Array(0x11000).fill(255),v=new DataView(b.buffer);
    for(const at of [0,0x10000]){b.fill(0,at,at+64);b[at]=0xe9;b[at+1]=1;v.setUint16(at+12,13,true);v.setUint32(at+28,4,true);}
    for(const [i,type,subtype,at,size,name]of[[0,1,2,0x9000,0x6000,'nvs'],[1,1,1,0xf000,0x1000,'phy_init'],[2,0,0,0x10000,0x1000,'factory']]){const off=0x8000+i*32;b.fill(0,off,off+32);v.setUint16(off,0x50aa,true);b[off+2]=type;b[off+3]=subtype;v.setUint32(off+4,at,true);v.setUint32(off+8,size,true);b.set(new TextEncoder().encode(name),off+12);}
    window.newConfigStation=async()=>{await window.configStation?.dispose();window.configStation=openFlashStation({product,scene,catalog,source:{file:new File([b],'uiflow-protocol-fixture.bin')},prepareSource:async s=>s});};await newConfigStation();
  });
  const dialog=page.getByRole('dialog',{name:'烧录到设备'}),section=dialog.getByRole('region',{name:'随固件写入配置'});
  await section.getByRole('button',{name:'新建配置',exact:true}).waitFor();await page.waitForFunction(()=>!document.querySelector('.flash-config-toolbar button').disabled);
  assert(await section.getByRole('combobox',{name:'使用配置',exact:true}).inputValue()==='','Configuration is opt-in');
  const profileSelect=section.getByRole('combobox',{name:'使用配置',exact:true});
  await profileSelect.selectOption('builtin-uiflow2-simulator');
  assert(await section.getByRole('button',{name:'删除配置',exact:true}).isHidden(),'Built-in profile cannot be deleted');
  await section.getByRole('button',{name:'复制并编辑',exact:true}).click();
  assert(await section.getByRole('textbox',{name:'Wi-Fi 名称（SSID）',exact:true}).inputValue()==='esp32_online_tester','Built-in SSID');
  assert(await section.getByLabel('Wi-Fi 密码',{exact:true}).inputValue()==='rilakkuma_cool','Built-in password');
  assert(await section.getByRole('combobox',{name:'UIFlow2 启动方式',exact:true}).inputValue()==='2','Official network-only boot mode');
  await section.getByRole('button',{name:'取消编辑',exact:true}).click();await profileSelect.selectOption('');
  await section.getByRole('button',{name:'新建配置',exact:true}).click();
  const fill=async(name,server)=>{await section.getByRole('textbox',{name:'配置名称',exact:true}).fill(name);await section.getByRole('combobox',{name:'固件格式',exact:true}).selectOption('uiflow2');await section.getByRole('textbox',{name:'数据服务器',exact:true}).fill(server);await section.getByRole('textbox',{name:'Wi-Fi 名称（SSID）',exact:true}).fill('Production-Test');await section.getByLabel('Wi-Fi 密码',{exact:true}).fill('fixture-secret-123');};
  await fill('产线 A','flow.example');
  assert(await section.getByLabel('Wi-Fi 密码',{exact:true}).getAttribute('type')==='password','Password starts masked');
  assert(await dialog.getByRole('button',{name:'添加设备',exact:true}).isDisabled(),'Unapplied edit cannot add a device');
  await page.screenshot({path:'output/playwright/flash-config-edit.png'});
  await section.getByRole('button',{name:'应用此配置',exact:true}).click();
  assert(!(await page.evaluate(()=>JSON.stringify(localStorage))).includes('fixture-secret-123'),'Default profile never persists password');
  await dialog.getByRole('radio',{name:'多台并行'}).check();
  const choose=async id=>{await page.evaluate(id=>configTest.next=id,id);await dialog.getByRole('button',{name:/^(添加设备|选择下一台)$/}).click();};
  for(let i=0;i<7;i++)await choose('parallel-'+i);
  assert(await dialog.locator('.flash-device').count()===7,'Seven independently authorized ports');
  await dialog.locator('.flash-acknowledgement input').check();await page.evaluate(()=>configTest.delay=1500);await dialog.getByRole('button',{name:'开始烧录 7 台',exact:true}).click();
  await page.waitForFunction(()=>configTest.active===7);assert(await section.getByRole('button',{name:'编辑配置',exact:true}).isDisabled(),'Active profile is locked');
  await page.screenshot({path:'output/playwright/flash-config-parallel.png'});
  await page.waitForFunction(()=>configTest.writes.length===7&&configTest.closed===7);
  const writes=await page.evaluate(()=>configTest.writes);assert(writes.every(w=>w.server==='flow.example'&&w.ssid==='Production-Test'&&w.password==='fixture-secret-123'),'Actual emitted NVS contains selected custom server and credentials on every port');
  assert(!(await page.evaluate(()=>JSON.stringify(configStation.observe()))).includes('fixture-secret-123'),'Public station observation excludes credentials');
  await page.evaluate(()=>newConfigStation());await page.waitForFunction(()=>!document.querySelector('.flash-config-toolbar button').disabled);
  assert(await section.getByRole('combobox',{name:'使用配置'}).locator('option').count()===3,'Memory profile reusable in next batch');
  const id=await section.getByRole('combobox',{name:'使用配置'}).locator('option').filter({hasText:'产线 A'}).getAttribute('value');await section.getByRole('combobox',{name:'使用配置'}).selectOption(id);
  await dialog.getByRole('radio',{name:'逐台连续'}).check();await dialog.locator('.flash-acknowledgement input').check();await dialog.getByRole('button',{name:'开启连续烧录'}).click();
  await page.evaluate(()=>configTest.delay=50);await choose('continuous-a');await page.waitForFunction(()=>configTest.writes.length===8&&configTest.closed===8);
  await section.getByRole('button',{name:'编辑配置',exact:true}).click();await fill('产线 B','custom.example');
  await section.getByRole('checkbox',{name:'保存到此浏览器（包含 Wi-Fi 密码）',exact:true}).check();await section.getByRole('button',{name:'应用此配置',exact:true}).click();await choose('continuous-b');await page.waitForFunction(()=>configTest.writes.length===9&&configTest.closed===9);
  const next=await page.evaluate(()=>configTest.writes.slice(-2));assert(next[0].server==='flow.example'&&next[1].server==='custom.example','Changing continuous profile applies only to next device');
  assert((await dialog.locator('.flash-device-configuration').nth(0).innerText()).includes('产线 A'),'Completed row retains prior profile');
  await page.setViewportSize({width:390,height:844});await page.screenshot({path:'output/playwright/flash-config-mobile.png'});
  assert(await dialog.evaluate(el=>el.scrollWidth<=el.clientWidth),'No mobile horizontal overflow');
  await dialog.getByRole('button',{name:'停止本批',exact:true}).click();await page.evaluate(()=>configStation.dispose());
  assert(errors.length===0,'No browser errors: '+errors.join(';'));
  return {checks:'custom server, masked password, memory/persist profiles, 7 concurrent writes, emitted NVS, continuous profile switch, immutability, secret-free observation, mobile layout',maxActive:await page.evaluate(()=>configTest.maxActive),errors};
}
