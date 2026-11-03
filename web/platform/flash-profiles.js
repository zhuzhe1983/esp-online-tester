import {normalizeFlashConfiguration} from './flash-configuration.js';
const key='esp-online-tester.flash-profiles.v1';
// Page-memory by default. Persist credentials only through the explicit UI choice.
export function createFlashProfiles({storage}={}) {
  const profiles=new Map();let loaded=false;
  const load=()=>{
    if(loaded)return;loaded=true;
    try{const values=JSON.parse(storage?.getItem(key)||'[]');if(Array.isArray(values))for(const item of values){try{if(typeof item.id==='string')profiles.set(item.id,{...normalizeFlashConfiguration(item),id:item.id,persistent:true});}catch{}}}catch{}
  };
  const save=values=>{if(storage)storage.setItem(key,JSON.stringify([...values.values()].filter(p=>p.persistent)));};
  return {
    list(){load();return structuredClone([...profiles.values()]);},
    put(profile,{id=globalThis.crypto.randomUUID(),persistent=false}={}) {
      load();const value={...normalizeFlashConfiguration(profile),id,persistent:!!persistent};
      if(persistent&&!storage)throw Error('此浏览器不能保存配置；可取消“保存到此浏览器”后使用。');
      const next=new Map(profiles);next.set(id,value);if(persistent||profiles.get(id)?.persistent)save(next);profiles.set(id,value);return structuredClone(value);
    },
    remove(id){load();const next=new Map(profiles);next.delete(id);if(profiles.get(id)?.persistent)save(next);profiles.delete(id);},
  };
}
let shared;
export function browserFlashProfiles() {
  if(!shared){let storage;try{storage=globalThis.localStorage;}catch{}shared=createFlashProfiles({storage});}
  return shared;
}
