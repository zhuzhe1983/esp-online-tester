import assert from 'node:assert/strict';
import {loadCatalog} from '../web/platform/catalog.js';

const empty = {schemaVersion:'1', items:[]};
const originalFetch = globalThis.fetch;
async function loadWith(documents) {
  globalThis.fetch = async url => new Response(documents[new URL(url).pathname.split('/').pop()] ?? JSON.stringify(empty));
  try { return await loadCatalog(); } finally { globalThis.fetch = originalFetch; }
}
const document = (bytes, id='catalog-product') => JSON.stringify({schemaVersion:'1',items:[{id,name:'Product',summary:'x'.repeat(bytes)}]});
const expanded = await loadWith({'products.json':document(600 * 1024)});
assert.equal(expanded.products[0].id,'catalog-product','Expanded product inventories load beyond the former 512 KiB limit');
await assert.rejects(() => loadWith({'products.json':document(1024 * 1024)}), {code:'INPUT_TOO_LARGE'});
await assert.rejects(() => loadWith({'accessories.json':document(600 * 1024)}), {code:'INPUT_TOO_LARGE'});
await assert.rejects(() => loadWith({'firmwares.json':document(600 * 1024)}), {code:'INPUT_TOO_LARGE'});
await assert.rejects(() => loadWith({'products.json':JSON.stringify({...empty,items:[{id:'same'},{id:'same'}]})}), {code:'CATALOG_SCHEMA'});
assert.equal(globalThis.fetch,originalFetch);
console.log('ok expanded product catalog bounds, unchanged accessory/firmware bounds and duplicate identity rejection');
