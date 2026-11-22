import test from 'node:test';
import assert from 'node:assert/strict';
import {productsForManufacturer,searchProducts,productChronologyLabel} from '../web/platform/catalog-selection.js';

const device=(id,date,basis='official-announcement',family='display')=>({id,name:`LILYGO ${id}`,manufacturer:'LILYGO',family:{id:family,name:family},chronology:{date,basis}});
test('LilyGO pickers and search use newest evidence first across families, with stable unknowns last',()=>{
  const entries=[device('A-old','2022-07-15'),device('Z-new','2026-01-28','storefront-publication','c5'),device('B-middle','2025-08-01'),device('C-unknown',null,'unknown'),device('D-invalid','2026-02-30')];
  const before=structuredClone(entries);
  assert.deepEqual(productsForManufacturer(entries,'LILYGO').map(x=>x.id),['Z-new','B-middle','A-old','C-unknown','D-invalid']);
  assert.deepEqual(searchProducts(entries,'LILYGO').map(x=>x.id),['Z-new','B-middle','A-old','C-unknown','D-invalid']);
  assert.deepEqual(productsForManufacturer(entries,'LILYGO','display').map(x=>x.id),['B-middle','A-old','C-unknown','D-invalid']);
  assert.deepEqual(entries,before,'picker order never rewrites source history');
  assert.match(productChronologyLabel(entries[1]),/上架日期（代用）/);
  assert.equal(productChronologyLabel(entries[4]),'');
});
test('ties use natural names and unrelated manufacturers retain natural ordering',()=>{
  const entries=[device('Device 10','2026-01-28'),device('Device 2','2026-01-28'),{id:'m10',name:'M5Stack Device 10',manufacturer:'M5Stack'},{id:'m2',name:'M5Stack Device 2',manufacturer:'M5Stack'}];
  assert.deepEqual(productsForManufacturer(entries,'LILYGO').map(x=>x.id),['Device 2','Device 10']);
  assert.deepEqual(productsForManufacturer(entries,'M5Stack').map(x=>x.id),['m2','m10']);
});
