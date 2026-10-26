// Run with the tracked Playwright CLI run-code command, on a local repository server.
async (page) => {
  const origin=new URL(page.url()).origin,dbName=`storage-browser-${Date.now()}`,second=await page.context().newPage();
  const setup=async target=>target.evaluate(async ({origin,dbName})=>{
    const {createDeviceStorage,deviceStorageScope}=await import(origin+'/web/platform/device-storage.js');
    window.store=createDeviceStorage({dbName});window.scope=deviceStorageScope({productId:'browser-test',board:'none',chip:'esp32s3',flashBytes:8192,firmwareSha256:'e'.repeat(64)});window.changes=[];store.subscribe(e=>changes.push(e));
    return store.read(scope);
  },{origin,dbName});
  const check=(condition,message)=>{if(!condition)throw Error(message);};
  try {
    await second.goto(page.url());const initial=await setup(page);check(initial.persistent,'real IndexedDB opens');await setup(second);
    await page.evaluate(async()=>{const data=new Uint8Array(4096);data[10]=78;await store.save(scope,{epoch:0,revision:0,pages:[{offset:4096,data:data.buffer}]});store.close();});
    await page.reload();const restored=await setup(page);check(restored.row.revision===1&&await page.evaluate(async()=>new Uint8Array((await store.read(scope)).row.pages[0].data)[10])===78,'refresh restores exact bytes');
    const cleared=await page.evaluate(()=>store.clear(scope));check(cleared.row.epoch===1,'clear advances epoch');
    await second.waitForFunction(()=>changes.some(change=>change.epoch===1));
    const stale=await second.evaluate(async()=>{try{await store.save(scope,{epoch:0,revision:1,pages:[]});return null;}catch(e){return e.code;}});check(stale==='STORAGE_CONFLICT','stale tab cannot overwrite clear');
    const events=await second.evaluate(()=>changes);check(events.every(event=>!('pages'in event)&&!('data'in event)),'broadcast has no guest contents');
    return {checkedAt:new Date().toISOString(),backend:'Chromium IndexedDB + BroadcastChannel',checks:['durable-refresh-exact-bytes','cross-tab-clear-notification','stale-write-rejected','no-guest-bytes-in-broadcast'],status:'pass'};
  } finally {
    await Promise.all([page.evaluate(()=>window.store?.close()),second.evaluate(()=>window.store?.close())]);await second.close();
    await page.evaluate(name=>new Promise((resolve,reject)=>{const r=indexedDB.deleteDatabase(name);r.onsuccess=resolve;r.onerror=()=>reject(r.error);}),dbName);
  }
}
