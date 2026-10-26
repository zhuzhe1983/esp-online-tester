// User-device writable Flash, separate from downloaded firmware artifacts.
// A transaction revision/epoch prevents a stale tab from resurrecting cleared data.
const memoryRows = new Map(), memoryListeners = new Set();
const clone = value => structuredClone(value);
const err = (code,message) => Object.assign(new Error(message),{code});
const PAGE = 4096, MAX_FLASH = 32*1024*1024;
function validScope(scope) {
  if (!scope || !/^[a-f0-9]{64}$/.test(scope.firmwareSha256) || !Number.isSafeInteger(scope.flashBytes) || scope.flashBytes < PAGE || scope.flashBytes > MAX_FLASH || !['productId','board','chip'].every(k=>typeof scope[k]==='string' && /^[a-zA-Z0-9_.-]{1,128}$/.test(scope[k]))) throw err('INVALID_STORAGE_SCOPE','Invalid loaded device storage identity.');
  const key=`v1:${scope.productId}:${scope.board}:${scope.chip}:${scope.flashBytes}:${scope.firmwareSha256}`;
  if(scope.key!==key)throw err('INVALID_STORAGE_SCOPE','Storage key does not match its device identity.');
  return key;
}
export function deviceStorageScope({productId,board,chip,flashBytes,firmwareSha256}) {
  const scope={productId,board,chip,flashBytes,firmwareSha256};scope.key=`v1:${productId}:${board}:${chip}:${flashBytes}:${firmwareSha256}`;validScope(scope);return scope;
}
function empty(scope) {return {key:scope.key,scope:clone(scope),epoch:0,revision:0,pages:[],savedAt:null};}
function validateRow(row,scope) {
  if(!row)return empty(scope);
  if(row.key!==scope.key || !Object.keys(scope).every(key=>row.scope?.[key]===scope[key]) || !Number.isSafeInteger(row.epoch)||row.epoch<0||!Number.isSafeInteger(row.revision)||row.revision<0||!Array.isArray(row.pages)||row.pages.length>scope.flashBytes/PAGE)throw err('STORAGE_CORRUPT','Stored device metadata does not match the loaded firmware.');
  const offsets=new Set();
  for(const page of row.pages){if(!Number.isSafeInteger(page.offset)||page.offset<0||page.offset%PAGE||page.offset+PAGE>scope.flashBytes||offsets.has(page.offset)||!(page.data instanceof ArrayBuffer)||page.data.byteLength!==PAGE)throw err('STORAGE_CORRUPT','Stored Flash page has invalid bounds.');offsets.add(page.offset);}
  return row;
}
export function createDeviceStorage({indexedDB=globalThis.indexedDB,BroadcastChannel=globalThis.BroadcastChannel,dbName='esp-online-tester-device-v1'}={}) {
  let connection,opening,channel,closed=false,memoryOnly=false,backend=indexedDB?'indexeddb':'memory',warning=indexedDB?null:'IndexedDB is unavailable; device writes survive only this page session.';
  const writer=globalThis.crypto?.randomUUID?.() || `${Date.now()}-${Math.random()}`,listeners=new Set();
  const deliver=message=>{if(closed||message.dbName!==dbName||message.writer===writer)return;for(const fn of listeners)fn(clone(message));};
  memoryListeners.add(deliver);
  function notify(row){const message={dbName,key:row.key,epoch:row.epoch,revision:row.revision,writer};for(const fn of memoryListeners)fn(message);channel?.postMessage(message);}
  function open(){
    if(connection)return Promise.resolve(connection);if(!indexedDB||closed)return Promise.resolve(null);
    if(!opening)opening=new Promise(resolve=>{
      let finished=false;const done=db=>{if(finished||closed){db?.close();if(!finished){finished=true;clearTimeout(timer);resolve(null);}return;}finished=true;clearTimeout(timer);connection=db;if(!db){backend='memory';warning='IndexedDB could not be opened; device writes are only in memory.';}else if(BroadcastChannel&&!channel){try{channel=new BroadcastChannel(dbName);channel.onmessage=e=>deliver(e.data);}catch{/* Revision/epoch transactions still reject stale writes. */}}resolve(db);};
      const timer=setTimeout(()=>done(null),2000);
      try{const r=indexedDB.open(dbName,1);r.onupgradeneeded=()=>r.result.createObjectStore('devices',{keyPath:'key'});r.onerror=r.onblocked=()=>done(null);r.onsuccess=()=>{r.result.onversionchange=()=>{r.result.close();connection=null;opening=null;};done(r.result);};}catch{done(null);}
    });return opening;
  }
  async function transaction(scope,mode,update){
    validScope(scope);const db=await open();if(!db)return null;
    return new Promise((resolve,reject)=>{
      let tx,value,cause;
      try{tx=db.transaction('devices',mode);const store=tx.objectStore('devices'),request=store.get(scope.key);
        request.onsuccess=()=>{try{const row=validateRow(request.result,scope);value=update(row);if(mode==='readwrite')store.put(value);}catch(e){cause=e;tx.abort();}};
        tx.oncomplete=()=>resolve(value);tx.onabort=tx.onerror=()=>reject(cause||err('STORAGE_WRITE_FAILED','Browser device-storage transaction failed (quota or database error).'));
      }catch(e){try{tx?.abort();}catch{}reject(e.code?e:err('STORAGE_WRITE_FAILED','Browser device-storage transaction could not start.'));}
    });
  }
  const memory=scope=>validateRow(memoryRows.get(dbName+':'+scope.key),scope);
  const result=row=>({row:clone(row),backend,warning,persistent:backend==='indexeddb'});
  return {
    subscribe(fn){listeners.add(fn);return()=>listeners.delete(fn);},
    async read(scope){
      let row;
      try{row=memoryOnly?null:await transaction(scope,'readonly',r=>r);if(row){backend='indexeddb';warning=null;}}
      catch(e){if(e.code==='STORAGE_CORRUPT')throw e;memoryOnly=true;backend='memory';warning=e.message;}
      row=row||memory(scope);memoryRows.set(dbName+':'+scope.key,clone(row));return result(row);
    },
    async save(scope,{epoch,revision,pages}){
      const update=row=>{if(row.epoch!==epoch||row.revision!==revision)throw err('STORAGE_CONFLICT','Another tab saved or cleared this device. Reload its stored state before continuing.');return validateRow({...row,epoch,revision:revision+1,pages:clone(pages),savedAt:new Date().toISOString()},scope);};
      let row;
      try{row=memoryOnly?null:await transaction(scope,'readwrite',update);if(row){backend='indexeddb';warning=null;}}
      catch(e){if(['STORAGE_CONFLICT','STORAGE_CORRUPT'].includes(e.code))throw e;memoryOnly=true;backend='memory';warning=e.message;}
      row=row||update(memory(scope));memoryRows.set(dbName+':'+scope.key,clone(row));notify(row);return result(row);
    },
    async clear(scope){
      const update=row=>({...empty(scope),epoch:row.epoch+1,revision:row.revision+1,savedAt:new Date().toISOString()});
      let row;
      try{row=await transaction(scope,'readwrite',update);if(row){memoryOnly=false;backend='indexeddb';warning=null;}}catch(e){throw err('STORAGE_CLEAR_FAILED',e.message);}
      if(!row&&indexedDB)throw err('STORAGE_CLEAR_FAILED','Browser storage is unavailable; durable device data could not be cleared.');
      row=row||update(memory(scope));memoryRows.set(dbName+':'+scope.key,clone(row));notify(row);return result(row);
    },
    close(){closed=true;channel?.close();connection?.close();listeners.clear();memoryListeners.delete(deliver);},
  };
}
