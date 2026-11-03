// A user-authorized firmware batch; no discovery event can authorize a write.
import {createFlasher,flasherCapabilities,prepareFlashSession,prepareFlashConfiguration,chooseFlashPort,activateFlashSession,releaseFlashSession} from './flasher.js';

const problem=(code,message)=>Object.assign(new Error(message),{code});
const check=(value,code,message)=>{if(!value)throw problem(code,message);};
const copy=value=>structuredClone(value);
const terminal=new Set(['done','error','cancelled']);
const replaceable=new Set(['queued','ready','error','cancelled']);
const waiting=item=>['queued','ready'].includes(item.status);
const deviceLabel=(id,port)=>{
  const info=port.getInfo?.() || {},usb=[info.usbVendorId,info.usbProductId].every(value=>Number.isInteger(value))?` · USB ${info.usbVendorId.toString(16).padStart(4,'0').toUpperCase()}:${info.usbProductId.toString(16).padStart(4,'0').toUpperCase()}`:'';
  return `设备 ${id.slice('device-'.length)}${usb}`;
};
export function createFlashBatch({onEvent,navigator:nav=globalThis.navigator,createDeviceFlasher=createFlasher,...options}={}) {
  const dependencies={...options,navigator:nav};
  let session=null,configuration=null,prepareAbort=null,status='idle',mode='parallel',running=false,choosing=false,replacingId=null,disposed=false,generation=0,nextId=1,concurrency=Infinity,error=null;
  const items=[],ports=new Map(),jobs=new Map(),pending=new Set();
  const capabilities=()=>flasherCapabilities(dependencies);
  const publicItem=item=>{const {port,configuration:config,...rest}=item;return {...copy(rest),configuration:config?copy(config):null};};
  const observe=()=>({status,mode,running,choosing,configuration:configuration?copy(configuration):null,product:session?copy(session.product):null,receipt:session?copy(session.receipt):null,error:copy(error),
    items:items.map(publicItem),counts:{total:items.length,queued:items.filter(waiting).length,active:jobs.size,done:items.filter(item=>item.status==='done').length,error:items.filter(item=>item.status==='error').length,cancelled:items.filter(item=>item.status==='cancelled').length}});
  const emit=()=>{try{onEvent?.({type:'state',state:observe()});}catch(e){console.error(e);}};
  const alive=()=>check(!disposed,'FLASH_SESSION_CLOSED','This flashing window has been closed.');
  const itemById=id=>{const item=items.find(item=>item.id===id);check(item,'FLASH_DEVICE_UNKNOWN','This device is not in the batch.');return item;};
  const deactivate=()=>{if(session)activateFlashSession(session,false,dependencies);};
  function settle() {
    if(status==='stopping' && !jobs.size)status='stopped';
    if(running && mode==='parallel' && !jobs.size && !items.some(item=>item.status==='queued')) {running=false;status=items.some(waiting)?'ready':'done';deactivate();}
    emit();
  }
  function launch(item) {
    item.status='connecting';item.progress=0;item.error=null;item.message='正在连接设备。';item.attempt++;item.mayBePartial=false;
    let disconnected=false;
    const flasher=createDeviceFlasher({...dependencies,onEvent(event){
      if(event.type==='state') {
        const state=event.state;
        Object.assign(item,{status:state.status==='choosing'?'connecting':state.status,progress:state.progress,message:state.message,error:state.error || null,mayBePartial:!!state.mayBePartial,flashVerified:!!state.flashVerified});
      } else if(event.type==='log') {
        item.logs.push(String(event.message).slice(-4096));if(item.logs.length>200)item.logs.splice(0,item.logs.length-200);
      }
      emit();
    }});
    jobs.set(item.id,{flasher,disconnect(){disconnected=true;flasher.cancel();}});
    const task=flasher.flash({prepared:session,configuration:item.configuration,port:item.port}).then(result=>{
      item.status='done';item.progress=1;item.message=result.message;item.error=null;
    },failure=>{
      item.status=failure.code==='FLASH_CANCELLED' && !disconnected?'cancelled':'error';
      item.error={code:disconnected?'DEVICE_DISCONNECTED':failure.code || 'FLASH_FAILED',message:disconnected?'设备已断开，请重新连接后重试。':failure.message};
      item.message=item.error.message;
    }).finally(()=>{
      jobs.delete(item.id);pending.delete(task);
      if(running)pump();
      settle();
    });
    pending.add(task);
  }
  function pump() {
    if(!running || disposed)return;
    const limit=mode==='sequential'?1:concurrency;
    for(const item of items) {
      if(jobs.size>=limit)break;
      if(item.status==='queued' && item.connected && item.id!==replacingId)launch(item);
    }
    settle();
  }
  const connection=connected=>event=>{
    // Web Serial events expose the port as event.target; older versions use port.
    const port=event.port || event.target,item=ports.get(port);if(!item || disposed)return;
    item.connected=connected;
    if(!connected) {
      // USB reset can re-enumerate after MD5 verification. Do not turn a verified
      // write into cancellation while the reset strategy is releasing the port.
      if(jobs.has(item.id) && !['resetting','closing','done'].includes(item.status))jobs.get(item.id).disconnect();
      else if(waiting(item)) {item.status='error';item.error={code:'DEVICE_DISCONNECTED',message:'设备已断开，请重新连接后重试。'};item.message=item.error.message;}
    }
    // Completed/reconnected devices never automatically get a second write.
    emit();
  };
  const connected=connection(true),disconnected=connection(false);
  nav?.serial?.addEventListener?.('connect',connected);nav?.serial?.addEventListener?.('disconnect',disconnected);
  const api={
    capabilities,observe,
    async prepare(input) {
      alive();check(!running && !jobs.size && !choosing && !prepareAbort,'FLASH_BUSY','Stop the current batch before preparing another firmware.');
      releaseFlashSession(session);session=null;configuration=null;items.length=0;ports.clear();error=null;status='preparing';
      const controller=new AbortController(),revision=++generation;prepareAbort=controller;emit();
      try {
        const prepared=await prepareFlashSession(input,{...dependencies,signal:controller.signal});
        if(disposed || revision!==generation) {releaseFlashSession(prepared);throw problem('FLASH_CANCELLED','Firmware preparation was cancelled.');}
        session=prepared;status='ready';
      } catch(failure) {
        if(!disposed && revision===generation) {status=failure.code==='FLASH_CANCELLED' || controller.signal.aborted?'stopped':'error';error={code:failure.code || 'FLASH_PREPARE_FAILED',message:failure.message};}
        throw failure;
      } finally {if(prepareAbort===controller)prepareAbort=null;emit();}
      return observe();
    },
    setConfiguration(profile) {
      alive();check(session,'FLASH_SESSION_INVALID','请先等待固件准备完成。');
      check(!jobs.size && !choosing && (!running || mode==='sequential'),'FLASH_BUSY','请等待当前设备写入完成后再切换配置。');
      configuration=prepareFlashConfiguration(session,profile);
      // Before Start, changes apply to waiting rows. Once continuous mode starts,
      // only subsequently selected devices receive the new configuration.
      if(!running)for(const item of items)if(waiting(item))item.configuration=configuration;
      emit();return observe();
    },
    async addDevice() {
      alive();check(session,'FLASH_SESSION_INVALID','Prepare the firmware before selecting devices.');
      check(!choosing && status!=='stopping','FLASH_BUSY','Wait for the current device selection or cancellation.');
      check(!running || mode==='sequential','FLASH_BUSY','Wait for this parallel batch to finish before adding devices.');
      choosing=true;const revision=generation;emit();
      try {
        const port=await chooseFlashPort(session,dependencies);
        check(!disposed && revision===generation,'FLASH_CANCELLED','Device selection was cancelled.');
        let item=ports.get(port);
        if(item) {item.connected=true;emit();return publicItem(item);}
        const id=`device-${nextId++}`;
        item={id,label:deviceLabel(id,port),port,configuration,status:'queued',progress:0,message:'等待烧录',logs:[],error:null,connected:true,attempt:0,mayBePartial:false};
        items.push(item);ports.set(port,item);if(!running)status='ready';else pump();
        emit();return publicItem(item);
      } finally {choosing=false;emit();}
    },
    async replaceDevice(id) {
      alive();const item=itemById(id);
      check(!jobs.has(id) && replaceable.has(item.status),'FLASH_BUSY','只能更换尚未开始、失败或已取消的设备。');
      check(!choosing && status!=='stopping','FLASH_BUSY','请等待设备选择或停止操作完成。');
      choosing=true;replacingId=id;const revision=generation,oldPort=item.port;emit();
      // Freeze a queued item while its chooser is open, including when another
      // parallel job completes. Selection alone must never authorize a write.
      try {
        const port=await chooseFlashPort(session,dependencies);
        check(!disposed && revision===generation,'FLASH_CANCELLED','Device selection was cancelled.');
        check(port!==oldPort,'FLASH_SAME_DEVICE','选择的仍是原设备。请选择其他设备，或取消后重试此设备。');
        check(!ports.has(port),'FLASH_DUPLICATE_DEVICE','这台设备已在本批队列中，请选择其他设备。');
        const logs=[...item.logs,`更换前：${item.label} · ${item.message}`,'已更换设备，等待点击烧录。'].slice(-200);
        ports.delete(oldPort);ports.set(port,item);
        Object.assign(item,{port,label:deviceLabel(id,port),status:'ready',progress:0,message:'已更换设备，确认后点击烧录。',logs,error:null,connected:true,attempt:0,mayBePartial:false,flashVerified:false});
        if(!running)status='ready';
        return publicItem(item);
      } finally {choosing=false;replacingId=null;if(running)pump();emit();}
    },
    start({mode:nextMode='parallel',concurrency:nextConcurrency=Infinity}={}) {
      alive();check(session,'FLASH_SESSION_INVALID','Prepare a firmware batch first.');
      check(!running && !jobs.size && !choosing,'FLASH_BUSY','A flashing batch is already active.');
      check(['parallel','sequential'].includes(nextMode),'FLASH_MODE_INVALID','Choose parallel or sequential flashing.');
      check(nextConcurrency===Infinity || Number.isSafeInteger(nextConcurrency) && nextConcurrency>=1,'FLASH_CONCURRENCY_INVALID','并行数量必须为正整数，默认不限制数量。');
      check(items.some(item=>waiting(item) && item.connected) || nextMode==='sequential','FLASH_NO_DEVICES','Choose at least one connected device.');
      activateFlashSession(session,true,dependencies);for(const item of items)if(item.status==='ready')item.status='queued';
      mode=nextMode;concurrency=nextConcurrency;running=true;status='running';pump();return observe();
    },
    retry(id) {
      alive();const item=itemById(id);check(!jobs.has(id) && (item.status==='ready' || terminal.has(item.status) && item.status!=='done'),'FLASH_BUSY','Only a waiting, failed or cancelled device can be started.');
      check(item.connected,'DEVICE_DISCONNECTED','Reconnect this device before retrying.');
      check(status!=='stopping' && !choosing,'FLASH_BUSY','Wait for cancellation or device selection to finish.');
      activateFlashSession(session,true,dependencies);item.status='queued';item.error=null;item.message='等待重试';item.progress=0;running=true;status='running';pump();return observe();
    },
    cancel(id) {
      alive();const item=itemById(id);
      if(jobs.has(id))jobs.get(id).flasher.cancel();
      else if(waiting(item)) {item.status='cancelled';item.message='已取消，未写入 Flash。';}
      settle();return observe();
    },
    stop() {
      if(disposed)return observe();
      running=false;generation++;deactivate();prepareAbort?.abort();
      for(const item of items) {
        if(jobs.has(item.id))jobs.get(item.id).flasher.cancel();
        else if(waiting(item)) {item.status='cancelled';item.message='已取消，未写入 Flash。';}
      }
      status=jobs.size?'stopping':'stopped';emit();return observe();
    },
    async dispose() {
      if(disposed)return;
      api.stop();disposed=true;releaseFlashSession(session);session=null;
      nav?.serial?.removeEventListener?.('connect',connected);nav?.serial?.removeEventListener?.('disconnect',disconnected);
      await Promise.allSettled([...pending]);ports.clear();
    },
  };
  return api;
}
