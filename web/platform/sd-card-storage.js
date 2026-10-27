// Browser-only SD media. Base images and sector-page overlays use separate stores so
// autosaving a few written pages never rewrites an entire imported image.
const memory=new Map(),listeners=new Set(),copy=x=>structuredClone(x);
const error=(code,message)=>Object.assign(new Error(message),{code});
const blank=key=>({key,epoch:0,revision:0,pages:[],media:null,savedAt:null});
export function createSdCardStorage({indexedDB=globalThis.indexedDB,BroadcastChannel=globalThis.BroadcastChannel,dbName='esp-online-tester-sd-v1'}={}) {
  let db,opening,closed=false,channel,failed=false;
  const own=new Set(),writer=globalThis.crypto?.randomUUID?.()||String(Math.random());
  const deliver=m=>{if(m.dbName===dbName&&m.writer!==writer)for(const fn of own)fn(m);};listeners.add(deliver);
  const notify=row=>{const m={dbName,key:row.key,epoch:row.epoch,revision:row.revision,writer};for(const fn of listeners)fn(m);channel?.postMessage(m);};
  const result=(row,image=null)=>({row:copy(row),image,backend:db&&!failed?'indexeddb':'memory',persistent:!!db&&!failed});
  const open=()=>opening ||= new Promise(resolve=>{
    if(!indexedDB||closed){resolve(null);return;}
    let ended=false;const done=value=>{if(ended||closed){value?.close();if(!ended){ended=true;clearTimeout(timer);resolve(null);}return;}ended=true;clearTimeout(timer);db=value;if(value&&BroadcastChannel){try{channel=new BroadcastChannel(dbName);channel.onmessage=e=>deliver(e.data);}catch{}}resolve(value);};
    const timer=setTimeout(()=>done(null),2000);
    try{const r=indexedDB.open(dbName,1);r.onupgradeneeded=()=>{r.result.createObjectStore('cards',{keyPath:'key'});r.result.createObjectStore('images');};r.onerror=r.onblocked=()=>done(null);r.onsuccess=()=>done(r.result);}catch{done(null);}
  });
  const keyCheck=key=>{if(typeof key!=='string'||!/^v1:[A-Za-z0-9_.:-]+:[a-f0-9]{64}$/.test(key))throw error('INVALID_SD_SCOPE','Invalid SD device/firmware identity.');};
  const checked=(row,key)=>{if(!row)return blank(key);if(row.key!==key||!Number.isSafeInteger(row.epoch)||row.epoch<0||!Number.isSafeInteger(row.revision)||row.revision<0||!Array.isArray(row.pages)||row.pages.length>32768)throw error('STORAGE_CORRUPT','Invalid saved SD metadata.');const seen=new Set();for(const p of row.pages){if(!Number.isSafeInteger(p.offset)||p.offset<0||p.offset%4096||p.offset+4096>128*1024*1024||seen.has(p.offset)||!(p.data instanceof ArrayBuffer)||p.data.byteLength!==4096)throw error('STORAGE_CORRUPT','Invalid saved SD page.');seen.add(p.offset);}return row;};
  const mem=key=>memory.get(dbName+':'+key)||{row:blank(key),image:null};
  async function transaction(key,mode,update,imageUpdate){
    keyCheck(key);await open();if(!db||failed)return null;
    return new Promise((resolve,reject)=>{const tx=db.transaction(['cards','images'],mode),cards=tx.objectStore('cards'),images=tx.objectStore('images');let row,image,cause;
      const r=cards.get(key);r.onsuccess=()=>{try{row=update(checked(r.result,key));if(mode==='readwrite')cards.put(row);if(imageUpdate!==undefined){if(imageUpdate===null)images.delete(key);else images.put(imageUpdate,key);image=imageUpdate;}else if(mode==='readonly'){const q=images.get(key);q.onsuccess=()=>{image=q.result||null;};}else image=mem(key).image;}catch(e){cause=e;tx.abort();}};
      tx.oncomplete=()=>resolve({row,image});tx.onerror=tx.onabort=()=>reject(cause||error('STORAGE_WRITE_FAILED','SD storage transaction failed.'));
    });
  }
  async function update(key,fn,imageUpdate,strict=false){
    let value;try{value=await transaction(key,'readwrite',fn,imageUpdate);}catch(e){if(strict||e.code==='STORAGE_CONFLICT'||e.code==='STORAGE_CORRUPT')throw e;failed=true;}
    if(!value&&strict&&indexedDB)throw error('STORAGE_CLEAR_FAILED','Browser storage is unavailable; the durable SD card was not replaced.');
    if(!value){const old=mem(key);value={row:fn(old.row),image:imageUpdate===undefined?old.image:imageUpdate};}
    // Keep the same owned image reference in memory; only deltas are copied on autosave.
    memory.set(dbName+':'+key,{row:copy(value.row),image:value.image});notify(value.row);return result(value.row);
  }
  return {
    subscribe(fn){own.add(fn);return()=>own.delete(fn);},
    async read(key){let value;try{value=await transaction(key,'readonly',r=>r);}catch(e){if(e.code==='STORAGE_CORRUPT')throw e;failed=true;}value ||= mem(key);memory.set(dbName+':'+key,{row:copy(value.row),image:value.image});return result(value.row,value.image?.slice(0)||null);},
    save(key,{epoch,revision,pages}){return update(key,row=>{if(row.epoch!==epoch||row.revision!==revision)throw error('STORAGE_CONFLICT','Another tab changed this SD card. Reload the firmware before continuing.');return {...row,pages:copy(pages),revision:revision+1,savedAt:new Date().toISOString()};});},
    replace(key,{image=null,media=null}={}){return update(key,row=>({...blank(key),epoch:row.epoch+1,revision:row.revision+1,media,savedAt:new Date().toISOString()}),image,true);},
    close(){closed=true;db?.close();channel?.close();own.clear();listeners.delete(deliver);}
  };
}
