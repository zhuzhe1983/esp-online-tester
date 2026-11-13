import assert from 'node:assert/strict';
import {loadCatalog,decodeScene,MAX_SCENE_BYTES} from '../web/platform/catalog.js';

const empty = {schemaVersion:'1', items:[]};
const originalFetch = globalThis.fetch;
async function loadWith(documents) {
  globalThis.fetch = async url => new Response(documents[new URL(url).pathname.split('/').pop()] ?? JSON.stringify(empty));
  try { return await loadCatalog(); } finally { globalThis.fetch = originalFetch; }
}
const document = (bytes, id='catalog-product') => JSON.stringify({schemaVersion:'1',items:[{id,name:'Product',summary:'x'.repeat(bytes)}]});
const expanded = await loadWith({'products.json':document(600 * 1024)});
assert.equal(expanded.products[0].id,'catalog-product','Expanded product inventories load beyond the former 512 KiB limit');
const releases = await loadWith({'firmwares.json':document(600 * 1024,'catalog-firmware')});
assert.equal(releases.firmwares[0].id,'catalog-firmware','Release metadata also grows beyond 512 KiB');
for(const [name,limit] of [['products',1024*1024],['accessories',512*1024],['firmwares',2*1024*1024]]){
  const padded=JSON.stringify(empty).padEnd(limit,' ');
  assert.deepEqual(await loadWith({[`${name}.json`]:padded}),{products:[],accessories:[],firmwares:[]});
  await assert.rejects(()=>loadWith({[`${name}.json`]:padded+' '}),{code:'INPUT_TOO_LARGE'});
}
const unicode=JSON.stringify({...empty,note:'纸'.repeat(700000)});
assert.ok(unicode.length<2*1024*1024);
await assert.rejects(()=>loadWith({'firmwares.json':unicode}),{code:'INPUT_TOO_LARGE'},'Limits count UTF-8 bytes');
assert.equal(MAX_SCENE_BYTES,64*1024);
const oversizedScene=Buffer.from(JSON.stringify({note:'x'.repeat(MAX_SCENE_BYTES)})).toString('base64url');
assert.throws(()=>decodeScene(oversizedScene),{code:'SCENE_ENCODING'},'Portable scenes keep their independent limit');
await assert.rejects(() => loadWith({'products.json':JSON.stringify({...empty,items:[{id:'same'},{id:'same'}]})}), {code:'CATALOG_SCHEMA'});
assert.equal(globalThis.fetch,originalFetch);
console.log('ok expanded product/firmware bounds, UTF-8 byte limits, independent scene limits and duplicate identity rejection');
