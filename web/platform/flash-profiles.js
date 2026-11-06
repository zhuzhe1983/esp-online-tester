import {normalizeFlashConfiguration} from './flash-configuration.js';
import {SIMULATOR_UIFLOW2_PROFILE} from './simulation-network.js';
const key='esp-online-tester.flash-profiles.v1';
// Page-memory by default. Persist credentials only through the explicit UI choice.
export function createFlashProfiles({storage,builtins=[]}={}) {
  const fixed=new Map(builtins.map(p=>[p.id,{...normalizeFlashConfiguration(p),id:p.id,builtin:true,persistent:false}]));
  const profiles=new Map();let loaded=false;
  const load=()=>{
    if(loaded)return;loaded=true;
    try{const values=JSON.parse(storage?.getItem(key)||'[]');if(Array.isArray(values))for(const item of values){try{if(typeof item.id==='string'&&!fixed.has(item.id))profiles.set(item.id,{...normalizeFlashConfiguration(item),id:item.id,persistent:true});}catch{}}}catch{}
  };
  const save=values=>{if(storage)storage.setItem(key,JSON.stringify([...values.values()].filter(p=>p.persistent)));};
  return {
    list(){load();return structuredClone([...fixed.values(),...profiles.values()]);},
    put(profile,{id=globalThis.crypto.randomUUID(),persistent=false}={}) {
      load();if(fixed.has(id))throw Error('内置配置不可覆盖，请另存为新配置。');const value={...normalizeFlashConfiguration(profile),id,persistent:!!persistent};
      if(persistent&&!storage)throw Error('此浏览器不能保存配置；可取消“保存到此浏览器”后使用。');
      const next=new Map(profiles);next.set(id,value);if(persistent||profiles.get(id)?.persistent)save(next);profiles.set(id,value);return structuredClone(value);
    },
    remove(id){load();if(fixed.has(id))throw Error('内置配置不可删除。');const next=new Map(profiles);next.delete(id);if(profiles.get(id)?.persistent)save(next);profiles.delete(id);},
  };
}
let shared;
export function browserFlashProfiles() {
  if(!shared){let storage;try{storage=globalThis.localStorage;}catch{}shared=createFlashProfiles({storage,builtins:[SIMULATOR_UIFLOW2_PROFILE]});}
  return shared;
}
