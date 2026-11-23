// Run with the Playwright CLI wrapper on an open local Core2 workbench.
async page => {
  await page.waitForFunction(() => !!window.espTester);
  const id = 'm5stack-k010';
  const ready = await page.evaluate(async id => {
    const {loadCatalog, createDefaultScene} = await import('./catalog.js');
    const catalog = await loadCatalog();
    const scene = createDefaultScene(catalog, id);
    const product = catalog.products.find(item => item.id === id);
    const fw = catalog.firmwares.find(item => item.id === product.defaultFirmwareId);
    scene.firmware = fw.source;
    await espTester.configure(scene);
    await espTester.loadFirmware();
    await espTester.start();
    return {firmware: fw.id, source: scene.firmware};
  }, id);
  const result = await page.evaluate(async () => {
    function require(condition, label) { if (!condition) throw Error(label); }
    async function until(predicate, label) {
      const end = performance.now() + 90000;
      while (performance.now() < end) {
        const state = await espTester.observe();
        if (state.status === 'error') throw Error(JSON.stringify(state.lastError));
        if (predicate(state)) return state;
        await new Promise(resolve => setTimeout(resolve, 25));
      }
      const state = await espTester.observe();
      throw Error(label + ' @ ' + JSON.stringify(state.stats) + ' ' + state.serial.slice(-1300));
    }
    const hash = async bytes => [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(v => v.toString(16).padStart(2,'0')).join('');
    async function frame() {
      const f = await espTester.capture({format:'rgba'});
      const area = (left, top, width, height) => {
        const values=new Uint8Array(width*height*4);
        for (let y=0; y<height; y++) {
          const start=((top+y)*f.width+left)*4;
          values.set(f.pixels.subarray(start,start+width*4),y*width*4);
        }
        return values;
      };
      return {sha256:await hash(f.pixels),touch:await hash(area(4,160,62,26)),width:f.width,height:f.height,channels:new Set(f.pixels).size};
    }
    const boot = await until(s => s.stats.time >= 6 && s.serial.includes('SDCard Size = 64'), 'Dashboard/TF mount');
    require(boot.sdCard.mounted && boot.sdCard.present && boot.sdCard.format==='fat32','Runtime card must be attached.');
    require(boot.serial.includes('FT6336 ready') && boot.serial.includes('Name:IMU6886') && boot.serial.includes('Name:BM8563'),'Core2 guest I2C devices must be observed.');
    const pre=await frame();
    require(pre.width===320 && pre.height===240 && pre.channels>15, 'Real factory dashboard dimensions/contents');
    const time=boot.stats.time;
    await espTester.input({type:'touch',x:160,y:120,down:true});
    await until(s=>s.stats.time>=time+.4,'FT6336 contact hold');
    await espTester.input({type:'touch',x:160,y:120,down:false});
    const release=(await espTester.observe()).stats.time;
    await until(s=>s.stats.time>=release+.25,'Guest received touch release');
    await espTester.pause();
    const post=await frame();
    require(pre.touch!==post.touch,'Guest dashboard TOUCH coordinate region does not reflect the touch');
    const paused=await espTester.observe();
    return {boot:{elapsed:boot.stats.time,serial:boot.serial.split(/\r?\n/).filter(line=>/FT6336 ready|Name:(Axp|CST Touch|IMU6886|BM8563)|SDCard (Type|Size) =/.test(line)),sdCard:{mounted:boot.sdCard.mounted,format:boot.sdCard.format,bytes:boot.sdCard.bytes}},touch:{x:160,y:120,before:pre,after:post,elapsed:paused.stats.time},guestPanic:/Guru Meditation|panic.ed/.test(paused.serial)};
  });
  if (result.guestPanic) throw Error('Official factory panicked.');
  return {ready,...result};
}
