async page => {
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.waitForFunction(()=>Boolean(window.espTester));
  const checks=await page.evaluate(async()=>{
    const {loadCatalog,createDefaultScene,decodeScene}=await import('./catalog.js');const catalog=await loadCatalog(),checks=[];
    for(const id of ['m5stack-papermono','m5stack-c153-lite']) {
      const scene=createDefaultScene(catalog,id);scene.firmware=null;await espTester.configure(scene);
      const state=await espTester.observe(),share=await espTester.share({autostart:false});
      if(state.scene.productId!==id)throw Error('wrong product');
      const decoded=decodeScene(new URL(share.url||share).hash.match(/scene=([^&]+)/)[1]);if(decoded.productId!==id)throw Error('share lost product variant');
      const plan=await (await fetch('../../output/papermono-lora-build/flasher_args.json')).json();const files=[];
      for(const path of ['flasher_args.json',...Object.values(plan.flash_files)]){const bytes=await (await fetch('../../output/papermono-lora-build/'+path)).arrayBuffer();const file=new File([bytes],path.split('/').at(-1));Object.defineProperty(file,'webkitRelativePath',{value:path});files.push(file);}
      await espTester.loadFirmware({format:'build',files});await espTester.start();
      if((await espTester.capabilities()).lora.available !== (id==='m5stack-papermono'))throw Error('Wrong LoRa capability');
      if(id==='m5stack-papermono') {
        await espTester.waitFor({port:'uart0',serialIncludes:'LORA_FIXTURE WAIT_RX',timeoutMs:90000});
        const radio=await espTester.readLoRa();if(radio.txCount!==1||radio.rxCount!==0||radio.transmitted[0].payloadHex!=='48454c4c4f')throw Error('guest TX mismatch');
        const packet={config:{frequency:0x36400000,sf:7,bandwidth:4,codingRate:1,syncWord:0x3444,implicitHeader:false,crc:true,invertIq:false},payloadHex:'50454552',rssiDbm:-70,snrQuarterDb:24,crcError:false};
        let rejected=false;try {await espTester.receiveLoRa({...packet,config:{...packet.config,frequency:packet.config.frequency+1}});}catch(e){rejected=e.code==='LORA_PACKET_REJECTED';}if(!rejected)throw Error('wrong peer accepted');
        await espTester.receiveLoRa(packet);
      } else {let unsupported=false;try{await espTester.readLoRa();}catch(e){unsupported=e.code==='LORA_UNSUPPORTED';}if(!unsupported)throw Error('Lite exposed radio');}
      const wanted=id==='m5stack-papermono'?'PASS_FULL':'LITE_ABSENT';const end=Date.now()+90000;let run;
      do{run=await espTester.observe();if(run.serial.includes('LORA_FIXTURE '+wanted))break;if(run.status==='error')throw Error(JSON.stringify(run.lastError));await new Promise(r=>setTimeout(r,50));}while(Date.now()<end);
      if(!run.serial.includes('LORA_FIXTURE '+wanted))throw Error('fixture failed '+run.serial.slice(-2000));await espTester.pause();
      checks.push({productId:id,board:catalog.products.find(p=>p.id===id).runtime.board,status:run.status,sharedProductId:decoded.productId,serial:(run.serialByPort.uart0||'').split(/\r?\n/).filter(l=>l.startsWith('LORA_FIXTURE '))});
    }
    return checks;
  });
  if(errors.length)throw Error(JSON.stringify(errors));return {checks,pageErrors:errors};
}
