// Install in an isolated workbench page. This diagnoses real guest output; it is
// not a throughput benchmark. Inputs use guest-time receipts, never wall holds.
export function installStopwatchTearingProbe(api = window.espTester) {
  const probe = {phase:'installed',frames:[],inputs:[],checkpoints:[],stats:{},markers:{},error:null};
  let latest=null, recording=false, status='configured';
  const canvas=()=>document.querySelector('canvas.flat-display');
  const takeFrame=event=>({number:event.frameNumber,width:event.width,height:event.height,
    pixels:event.pixels,stats:{...probe.stats},canvasPng:canvas()?.toDataURL('image/png')||null});
  const unsubscribe=api.subscribe(event=>{
    if(event.type==='stats')probe.stats={...event.stats};
    if(event.type==='state')status=event.state.status;
    if(event.type==='serial')for(const marker of ['[Launcher] on open','Guru Meditation Error'])
      if(event.data.includes(marker))probe.markers[marker]=true;
    if(event.type==='frame'){
      latest=event;
      if(recording&&probe.frames.length<240)probe.frames.push(takeFrame(event));
    }
  });
  const waitGuest=target=>new Promise((resolve,reject)=>{
    if(Number(probe.stats.time)>=target){resolve();return;}
    const timer=setTimeout(()=>{off();reject(Error(`Guest-time timeout at ${target}`));},120000);
    const off=api.subscribe(event=>{
      if(event.type==='stats'&&Number(event.stats.time)>=target){clearTimeout(timer);off();resolve();}
      else if(event.type==='state'&&['error','disposed'].includes(event.state.status)){clearTimeout(timer);off();reject(Error(`Runtime ${event.state.status}`));}
    });
  });
  const input=async event=>{const receipt=await api.input(event);probe.inputs.push({event,receipt});return receipt;};
  const checkpoint=name=>{if(!latest)throw Error('No guest frame');const frame=takeFrame(latest);probe.checkpoints.push({name,...frame});};
  probe.prepare=async()=>{
    try {
      probe.phase='loading';
      const state=await api.observe();
      if(state.scene.productId!=='m5stack-stopwatch')throw Error('Select StopWatch first.');
      const catalog=await(await fetch(new URL('catalog/firmwares.json',location.href))).json();
      const firmware=catalog.items.find(item=>item.id==='m5stack-stopwatch-official');
      probe.firmware={id:firmware.id,revision:firmware.revision,source:firmware.source};
      await api.configure({...state.scene,firmware:firmware.source,accessories:[],view:{mode:'studio',deviceView:'2d'}});
      await api.loadFirmware(firmware.source);
      const loaded=await api.observe();
      probe.storage={pristine:loaded.storage.pristine,restored:loaded.storage.restored};
      if(!probe.storage.pristine||probe.storage.restored)throw Error('Use an isolated browser context with pristine Flash.');
      probe.phase='booting';await api.start();await waitGuest(15);checkpoint('boot');
      probe.phase='entering-menu';let receipt;
      try {for(const id of ['button-a','button-b'])receipt=await input({type:'button',id,pressed:true});await waitGuest(receipt.guestTime+1.5);}
      finally {for(const id of ['button-a','button-b'])await input({type:'button',id,pressed:false});}
      await waitGuest(Number(probe.stats.time)+1.5);await api.pause();checkpoint('before-swipe');probe.phase='ready';
    }catch(error){probe.error=error.message;probe.phase='failed';if(status==='running')await api.pause();}
  };
  probe.adoptPaused=async()=>{
    const state=await api.observe();
    if(state.status!=='paused'||state.scene.productId!=='m5stack-stopwatch')throw Error('Pause the isolated StopWatch first.');
    probe.stats={...state.stats};probe.firmware={source:state.scene.firmware};
    latest=await api.capture({format:'rgba'});checkpoint('before-swipe');probe.phase='ready';
  };
  probe.swipe=async(points=[[234,15],[234,50],[234,100],[234,150],[234,220],[234,300]])=>{
    try {
      if(probe.phase!=='ready')throw Error('Wait for the launcher checkpoint.');
      if(!Array.isArray(points)||points.length<2||points.length>30||!points.every(p=>Array.isArray(p)&&p.length===2&&p.every(Number.isInteger)&&p[0]>=0&&p[0]<468&&p[1]>=0&&p[1]<466))throw Error('Supply 2..30 native StopWatch touch points.');
      probe.phase='swiping';recording=true;await api.start();
      const [x0,y0]=points[0], [x1,y1]=points.at(-1);
      let receipt=await input({type:'touch',x:x0,y:y0,down:true});
      try {
        for(const [x,y] of points.slice(1)){await waitGuest(receipt.guestTime+0.12);receipt=await input({type:'touch',x,y,down:true});}
        await waitGuest(receipt.guestTime+0.15);
      }finally{await input({type:'touch',x:x1,y:y1,down:false});}
      await waitGuest(Number(probe.stats.time)+1.5);await api.pause();recording=false;checkpoint('after-swipe');probe.phase='done';
    }catch(error){recording=false;probe.error=error.message;probe.phase='failed';if(status==='running')await api.pause();}
  };
  probe.export=async()=>{
    const hash=async pixels=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',pixels)),b=>b.toString(16).padStart(2,'0')).join('');
    const serialize=async f=>{
      const image=document.createElement('canvas');image.width=f.width;image.height=f.height;
      image.getContext('2d').putImageData(new ImageData(f.pixels,f.width,f.height),0,0);
      return {name:f.name,number:f.number,width:f.width,height:f.height,
        stats:Object.fromEntries(['time','insns','frames','speed','behind','resyncs','mips'].map(k=>[k,f.stats[k]])),
        sha256:await hash(f.pixels),rawPng:image.toDataURL('image/png'),canvasPng:f.canvasPng};
    };
    return {phase:probe.phase,error:probe.error,firmware:probe.firmware,storage:probe.storage,
      inputs:probe.inputs,markers:probe.markers,frames:await Promise.all(probe.frames.map(serialize)),
      checkpoints:await Promise.all(probe.checkpoints.map(serialize))};
  };
  probe.dispose=()=>{unsubscribe();};
  return probe;
}
