// Run with the Playwright CLI wrapper on an isolated local workbench.
async page => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.setViewportSize({width: 1420, height: 980});
  await page.locator('#product-family-select').selectOption('');
  await page.locator('#product-select').selectOption('m5stack-k034');
  try {
    const result = await page.evaluate(async () => {
      const api = window.espTester;
      const digest = async pixels => Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', pixels)),
          n => n.toString(16).padStart(2,'0')).join('');
      async function wait(label, predicate) {
        const end = Date.now() + 50000;
        do {
          const state = await api.observe();
          if (state.status === 'error') throw Error(JSON.stringify(state.lastError));
          const result = await predicate(state);
          if (result) return result === true ? state : result;
          await new Promise(resolve => setTimeout(resolve, 120));
        } while (Date.now() < end);
        const state = await api.observe();
        throw Error(label + ': ' + JSON.stringify({
          status:state.status,time:state.stats.time,frame:state.frame,
          detectsTough:state.serial.includes('[Autodetect] M5Tough'),board:state.product?.runtime?.board
        }));
      }
      function pixel(frame, x, y) {return Array.from(frame.pixels.slice((y*frame.width+x)*4,(y*frame.width+x)*4+4));}
      const opening = await wait('K034 catalog selection', state => state.product?.id === 'm5stack-k034');
      if (opening.scene.firmware?.sha256 !== '42198bed525475d134f1b92d71c7cc9208cdb8ff1581459cc2b8ca457635b2fb') {
        throw Error('Wrong default firmware for K034');
      }
      await api.configureNetwork({enabled:false});
      await api.loadFirmware();
      await api.start();
      const booted = await wait('native-sized Tough Tools menu', state => state.stats.time >= 4 &&
        state.frame?.width === 320 && state.frame.height === 240 && state.serial.includes('[Autodetect] M5Tough'));
      const menu = await api.capture({format:'rgba'});
      const menuColors = new Set(menu.pixels).size;
      if (menuColors < 40) throw Error('Menu is blank or incomplete');
      const down = await api.input({type:'button',id:'btn-a',pressed:true});
      let selected;
      try {
        selected = await wait('A front key selects TOUCH tool', async state => {
          if (state.stats.time <= down.guestTime || state.frame.frameNumber <= menu.frameNumber) return false;
          const frame = await api.capture({format:'rgba'}), [r,g,b] = pixel(frame,58,222);
          return r > 220 && g > 150 && b < 20 && frame;
        });
      } finally {
        await api.input({type:'button',id:'btn-a',pressed:false});
      }
      const pointer = await api.input({type:'touch',x:147,y:108,down:true});
      let pointed;
      try {
        pointed = await wait('TOUCH tool draws the guest point', async state => {
          if (state.stats.time <= pointer.guestTime || state.frame.frameNumber <= selected.frameNumber) return false;
          const frame = await api.capture({format:'rgba'}), [r,g,b] = pixel(frame,147,108);
          return (r < 30 && g < 30 && b > 230) && frame;
        });
      } finally {
        await api.input({type:'touch',x:147,y:108,down:false});
      }
      await api.pause();
      const after = await api.observe();
      return {
        board:after.product.runtime.board,romDetected:true,time:after.stats.time,width:menu.width,height:menu.height,
        firmware:booted.firmware?.artifacts?.map(({kind,sha256,bytes})=>({kind,sha256,bytes})),
        framebuffer:{
          menu:await digest(menu.pixels), touchTab:await digest(selected.pixels), touchPoint:await digest(pointed.pixels),
          menuColors,selectedIndicator:pixel(selected,58,222),point:pixel(pointed,147,108)
        },
        sdCard: {slot:after.sdCard?.slot, supported:after.sdCard?.supported},
        touchUpdateFailed: after.serial.includes('[tlsc6x] get cfg-ver fail'),
      };
    });
    for (const mode of ['2d','3d']) {
      const button = page.locator('#view-'+mode);
      await button.click();
      if (await button.getAttribute('aria-pressed') !== 'true') throw Error('View switch failed: '+mode);
    }
    if (errors.length) throw Error(errors.join('\n'));
    return {result,views:['2d','3d'],errors};
  } finally {
    await page.evaluate(async () => await window.espTester.dispose());
  }
}
