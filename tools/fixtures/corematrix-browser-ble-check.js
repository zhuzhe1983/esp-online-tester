// Run after the matrix fixture in a task-owned browser with original UIFlow loaded.
async page => {
 const target=page.frames().find(f=>f.parentFrame()&&f.url().includes('/platform/embed.html'))||page;
 return target.evaluate(async()=>{
  const api=window.espTester,wait=async predicate=>{const end=Date.now()+120000;while(Date.now()<end){const s=await api.observe();if(s.status==='error')throw Error(JSON.stringify(s.lastError));if(predicate(s))return s;await new Promise(r=>setTimeout(r,25));}throw Error('BLE browser guest timeout');};
  const send=data=>api.input({type:'serial',port:'usb',data}),advance=async seconds=>{const at=(await api.observe()).stats.time;await wait(s=>s.stats.time>=at+seconds);};
  await api.start();let start=(await api.observe()).serial.length;await send('\x02\r\x03\x03');await advance(.15);await send('\x01');await wait(s=>s.serial.slice(start).includes('raw REPL; CTRL-B to exit\r\n>'));
  await api.configureBle({nearby:['name=Web-peer,addr=12:34:56:78:9a:bc,type=public,rssi=-42']});
  const source='import bluetooth,time,ujson\nb=bluetooth.BLE();b.active(True)\nscans=[]\ndef irq(event,data):\n if event==5:\n  kind,addr,typ,rssi,payload=data\n  scans.append([kind,bytes(addr).hex(),typ,rssi,bytes(payload).hex()])\nb.irq(irq)\nb.gap_scan(300,30000,30000,False)\ntime.sleep_ms(500)\nprint("CM_WEB_SCAN",ujson.dumps(scans))\nb.gap_advertise(100000,b"\\x02\\x01\\x06\\x04\\x09C61")\ntime.sleep_ms(250)\nb.gap_advertise(None);b.active(False)\n';
  start=(await api.observe()).serial.length;for(let i=0;i<source.length;i+=64){await send(source.slice(i,i+64));await advance(.01);}await send('\x04');
  const state=await wait(s=>/\x04[^\x04]*\x04>/.test(s.serial.slice(start))),out=state.serial.slice(start);if(out.includes('Traceback'))throw Error(out);
  const scans=JSON.parse(out.match(/CM_WEB_SCAN ([^\r\n]+)/)?.[1]||'null');if(!scans?.length||scans.some(s=>JSON.stringify(s)!=='[0,"123456789abc",2,-42,"02010609095765622d70656572"]'))throw Error('Incorrect guest scan callback');
  const radio=await api.readBle();if(radio.packets.length<6||radio.packets.some((p,i)=>p.channel!==37+i%3||!p.pduHex.endsWith('0201060409433631')))throw Error('Incorrect guest DMA advertisement');
  await api.configureBle({nearby:[]});await api.pause();
  return {productId:state.product.id,firmwareSha256:state.scene.firmware.sha256,firmwarePatched:false,transport:'production-browser-worker',scanCallbacks:scans.length,scanExample:scans[0],advertisementCount:radio.packets.length,channels:radio.packets.map(p=>p.channel),source:radio.source};
 });
}
