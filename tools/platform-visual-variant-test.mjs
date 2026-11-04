import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createVisualVariantController} from '../web/platform/visual-variant-menu.js';
import {visualVariantsForProduct} from '../web/platform/catalog-selection.js';

const products=[{id:'base',name:'Base',model:{}},{id:'pin',name:'Pin',visualVariantOf:'base',pickerVisible:false,model:{}},{id:'kit',name:'Kit',visualVariantOf:'base',pickerVisible:false,model:{}},{id:'other',name:'Other',model:{}},{id:'missing',name:'No model',visualVariantOf:'base'}];
function fixture(){const renders=[];const c=createVisualVariantController({products,onSelect:(p,options)=>renders.push({id:p.id,...options})});const update=(status='configured',id='base',extra={})=>c.update({product:products.find(p=>p.id===id),status,mode:'3d',...extra});update();return{c,renders,update};}
test('only explicit child models are preview choices, with no implicit runtime inheritance',()=>{
  assert.deepEqual(visualVariantsForProduct(products,'base').map(p=>p.id),['kit','pin']);
  assert.deepEqual(visualVariantsForProduct(products,undefined),[]);
  assert.deepEqual(visualVariantsForProduct(products,'pin'),[]);
  const f=fixture(),before=structuredClone(products);assert.equal(f.c.select('other'),false);assert.equal(f.c.select('missing'),false);
  assert.equal(f.c.select('pin'),true);assert.equal(f.c.snapshot().product.id,'base');assert.equal(f.c.snapshot().selectedId,'pin');
  assert.deepEqual(f.renders,[{id:'pin',previewOnly:true}]);assert.deepEqual(products,before);
  f.c.select('base');assert.deepEqual(f.renders.at(-1),{id:'base',previewOnly:false});
});
test('running and loading restore base before renderer updates; pause requires a fresh explicit preview',()=>{
  for(const status of ['running','loading','disposed']){
    const f=fixture();f.c.select('pin');f.update(status);assert.deepEqual(f.renders.at(-1),{id:'base',previewOnly:false});
    assert.equal(f.c.snapshot().previewOnly,false);assert.equal(f.c.snapshot().disabled,true);assert.equal(f.c.select('pin'),false);
    f.update('paused');assert.equal(f.c.snapshot().selectedId,'base');assert.equal(f.c.select('kit'),true);
  }
});
test('product/mode changes reset previews, metadata refresh preserves an idle preview, and busy prevents selection',()=>{
  const f=fixture();f.c.select('pin');f.update();assert.equal(f.c.snapshot().selectedId,'pin','appearance/key metadata does not discard preview');
  f.update('configured','base',{busy:true});assert.equal(f.c.select('kit'),false);assert.equal(f.c.snapshot().selectedId,'pin');
  f.update('configured','base',{mode:'2d'});assert.equal(f.c.snapshot().visible,false);assert.deepEqual(f.renders.at(-1),{id:'base',previewOnly:false});
  f.update();f.c.select('pin');const count=f.renders.length;f.update('configured','other');assert.equal(f.c.snapshot().selectedId,'other');assert.equal(f.c.snapshot().visible,false);assert.equal(f.renders.length,count,'new scene renderer owns rendering the new base');
  f.update('configured','pin');assert.equal(f.c.snapshot().product.id,'pin');assert.equal(f.c.snapshot().visible,false,'legacy exact variant scene is not remapped to parent');
});
test('catalog parent links are complete, distinct, directly reference visible same-manufacturer models and preserve versions',async()=>{
  const catalog=JSON.parse(await readFile(new URL('../web/platform/catalog/products.json',import.meta.url),'utf8')).items;
  const expected={'m5stack-s007-pin127':'m5stack-s007','m5stack-s007-pin254':'m5stack-s007','m5stack-s007-pin127-v033':'m5stack-s007-v033','m5stack-s007-pin254-v033':'m5stack-s007-v033','m5stack-s015-dip':'m5stack-s015','m5stack-s016-dip':'m5stack-s016','m5stack-k051-b':'m5stack-c050-b','m5stack-k051':'m5stack-c050-b','m5stack-k056':'m5stack-c056-b','m5stack-k122':'m5stack-c122-b'};
  assert.deepEqual(Object.fromEntries(catalog.filter(p=>p.visualVariantOf).map(p=>[p.id,p.visualVariantOf])),expected);
  for(const [id,parentId] of Object.entries(expected)){const child=catalog.find(p=>p.id===id),parent=catalog.find(p=>p.id===parentId);assert.ok(parent?.model&&child.model);assert.equal(parent.visualVariantOf,undefined);assert.notEqual(parent.pickerVisible,false);assert.equal(parent.manufacturer,child.manufacturer);}
  for(const id of ['m5stack-k138','m5stack-k138-v11','m5stack-s007','m5stack-s007-v033'])assert.equal(catalog.find(p=>p.id===id).visualVariantOf,undefined);
});

test('disposed menu ignores late runtime events and never recreates renderer content',()=>{
  const f=fixture();f.c.select('pin');f.c.dispose();const count=f.renders.length;f.update('disposed');f.update('running');f.c.reset();assert.equal(f.c.select('kit'),false);assert.equal(f.renders.length,count);
});
