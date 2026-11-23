// Run on a local workbench in a fresh task-owned browser profile. The K128
// test imports a writable virtual FAT32 image into that profile's storage.
async page => {
  if(!['127.0.0.1','localhost'].includes(new URL(page.url()).hostname))throw Error('Use a local workbench for this storage fixture');
  await page.waitForFunction(()=>!!window.espTester,null,{timeout:60000});
  const results=[];
  for(const id of ['m5stack-k128','m5stack-k128-se','m5stack-k128-lite','m5stack-k149']){
    results.push(await page.evaluate(async id=>{
      const api=window.espTester,{loadCatalog,createDefaultScene}=await import('./catalog.js'),catalog=await loadCatalog();
      const product=catalog.products.find(p=>p.id===id);
      const firmware=catalog.firmwares.find(f=>f.id===product.defaultFirmwareId);
      const scene=createDefaultScene(catalog,id);scene.firmware=firmware.source;
      await api.configure(scene);
      await api.configureNetwork({enabled:false,autoProvision:false});
      await api.loadFirmware();
      if(id==='m5stack-k128'){
        // Add a zero-byte 8.3 file to the existing FAT32 root (cluster 2).
        const {createFat32Image}=await import('./sd-card-image.js'),bytes=createFat32Image();
        const view=new DataView(bytes.buffer),boot=view.getUint32(454,true)*512;
        const root=boot+(view.getUint16(boot+14,true)+view.getUint32(boot+36,true)*bytes[boot+16])*512;
        bytes.set(new TextEncoder().encode('PROOF   TXT'),root+32);bytes[root+32+11]=0x20;
        await api.attachSdCard({bytes});
      }
      const capture=async()=>{
        const frame=await api.capture({format:'rgba'});
        if(frame.width!==320||frame.height!==240||new Set(frame.pixels).size<16)throw Error('Expected a nonblank 320×240 guest frame');
        const digest=await crypto.subtle.digest('SHA-256',frame.pixels);
        return [...new Uint8Array(digest)].map(v=>v.toString(16).padStart(2,'0')).join('');
      };
      const advance=async seconds=>{
        const deadline=Date.now()+45000;
        while(Date.now()<deadline){
          const state=await api.observe();
          if(state.status==='error')throw Error(state.lastError?.message||'Guest failed');
          if(state.stats.time>=seconds)return state;
          await new Promise(resolve=>setTimeout(resolve,30));
        }
        throw Error('Guest time did not advance');
      };
      const tap=async (x,y)=>{
        const down=await api.input({type:'touch',x,y,down:true});await advance(down.guestTime+.2);
        const up=await api.input({type:'touch',x,y,down:false});await advance(up.guestTime+.65);
      };
      await api.start();
      const boot=await advance(4);
      if(!boot.serial.includes('ESP-ROM')||boot.firmware.artifacts.find(a=>a.kind==='flash')?.sha256!==firmware.source.sha256){
        throw Error('Expected unchanged pinned image booted via ROM');
      }
      const result={id,board:product.runtime.board,firmwareSha256:firmware.source.sha256,startupFrameSha256:await capture()};
      if(id==='m5stack-k149'){
        if(!boot.serial.includes('esp_ot_br')||!boot.serial.includes('Wait for response timeout'))throw Error('Expected original Thread BR host to wait for the unmodeled RCP');
        result.guestEvidence='esp_ot_br starts; Spinel Wait for response timeout';
      }else{
        await tap(160,120);
        result.menuFrameSha256=await capture();
        if(result.menuFrameSha256===result.startupFrameSha256)throw Error('Touch did not enter the factory menu');
        await tap(120,190);
        await api.waitFor({serialIncludes:'HomeMenu -> AppSD',timeoutMs:12000});
        result.sdFrameSha256=await capture();
        if(result.sdFrameSha256===result.menuFrameSha256)throw Error('Touch did not reach AppSD');
        result.guestEvidence='HomeMenu -> AppSD';
        // Exact guest frame reviewed on the pinned v0.12 image with PROOF.TXT.
        if(id==='m5stack-k128'&&result.sdFrameSha256!=='b48fbd13e7d76796af845ee7d6cce32bb7d801faa7dfb0143c6bdc81aa3f97b7'){
          throw Error('The original SD file list did not render PROOF.TXT');
        }
      }
      await api.pause();
      return result;
    },id));
  }
  return results;
}
