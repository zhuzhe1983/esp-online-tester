// User images stay in this browser. They are deliberately absent from portable scene JSON.
export const STICKER_LIMITS = Object.freeze({count:32, fileBytes:12*1024*1024, storedBytes:48*1024*1024, edge:1024});
const memory=new Map(),volatileProducts=new Set();let database;
function openDatabase(){
  if(database)return database;
  database=new Promise((resolve,reject)=>{
    if(!globalThis.indexedDB){reject(new Error('此浏览器无法保存贴纸，当前页面内仍可使用。'));return;}
    const request=indexedDB.open('esp-online-tester:stickers:v1',1);
    request.onupgradeneeded=()=>request.result.createObjectStore('products');
    request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error);
  });return database;
}
export function validateStickerCollection(items){
  if(!Array.isArray(items)||items.length>STICKER_LIMITS.count)throw Error(`每款设备最多保存 ${STICKER_LIMITS.count} 张贴纸。`);
  let bytes=0;const ids=new Set();
  for(const item of items){
    if(typeof item.id!=='string'||item.id.length>80||ids.has(item.id)||!(item.blob instanceof Blob)||!['image/png','image/jpeg','image/webp'].includes(item.blob.type))throw Error('贴纸数据无效。');
    ids.add(item.id);bytes+=item.blob.size;
    if(typeof item.name!=='string'||item.name.length>120||!Number.isFinite(item.aspect)||item.aspect<=0||item.aspect>100)throw Error('贴纸尺寸无效。');
    if(!Number.isFinite(item.size)||item.size<.02||item.size>1||!Number.isFinite(item.rotation)||Math.abs(item.rotation)>180||!Number.isFinite(item.x)||Math.abs(item.x)>1||!Number.isFinite(item.y)||Math.abs(item.y)>1)throw Error('贴纸变换无效。');
    if(item.anchor){const a=item.anchor;if(typeof a.part!=='string'||a.part.length>240||!Array.isArray(a.point)||!Array.isArray(a.normal)||a.point.length!==3||a.normal.length!==3||![...a.point,...a.normal].every(Number.isFinite)||a.point.some(n=>Math.abs(n)>10000)||Math.hypot(...a.normal)<.9||Math.hypot(...a.normal)>1.1)throw Error('贴纸位置无效。');}
  }
  if(bytes>STICKER_LIMITS.storedBytes)throw Error('这款设备的贴纸总量不能超过 48 MiB。');return items;
}
export async function readStickers(productId){
  if(volatileProducts.has(productId))return {items:structuredClone(memory.get(productId)||[]),persistent:false};
  try{const db=await openDatabase(),items=await new Promise((resolve,reject)=>{const r=db.transaction('products').objectStore('products').get(productId);r.onsuccess=()=>resolve(r.result||[]);r.onerror=()=>reject(r.error);});return {items:validateStickerCollection(items),persistent:true};}
  catch{return {items:memory.get(productId)||[],persistent:false};}
}
export async function saveStickers(productId,items){
  validateStickerCollection(items);memory.set(productId,structuredClone(items));
  try{const db=await openDatabase();await new Promise((resolve,reject)=>{const tx=db.transaction('products','readwrite');tx.objectStore('products').put(items,productId);tx.oncomplete=resolve;tx.onerror=()=>reject(tx.error);tx.onabort=()=>reject(tx.error);});volatileProducts.delete(productId);return true;}
  catch{volatileProducts.add(productId);return false;}
}
export async function prepareSticker(file){
  if(!['image/png','image/jpeg','image/webp'].includes(file.type))throw Error('贴纸支持 PNG、JPEG、WebP 图片。');
  if(file.size>STICKER_LIMITS.fileBytes)throw Error('每张原图不能超过 12 MiB。');
  const bitmap=await createImageBitmap(file);
  try{
    if(!bitmap.width||!bitmap.height||bitmap.width*bitmap.height>64*1024*1024||Math.max(bitmap.width/bitmap.height,bitmap.height/bitmap.width)>100)throw Error('图片尺寸过大或比例无效。');
    const scale=Math.min(1,STICKER_LIMITS.edge/Math.max(bitmap.width,bitmap.height)),canvas=document.createElement('canvas');
    canvas.width=Math.max(1,Math.round(bitmap.width*scale));canvas.height=Math.max(1,Math.round(bitmap.height*scale));canvas.getContext('2d').drawImage(bitmap,0,0,canvas.width,canvas.height);
    const blob=await new Promise((resolve,reject)=>canvas.toBlob(value=>value?resolve(value):reject(Error('无法读取这张图片。')),'image/webp',.9));
    return {id:crypto.randomUUID(),name:(file.name||'贴纸').slice(0,120),blob,aspect:canvas.width/canvas.height,size:.22,rotation:0,x:0,y:0,anchor:null};
  }finally{bitmap.close();}
}
