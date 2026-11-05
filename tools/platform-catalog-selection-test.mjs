import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {loadCatalog,createDefaultScene,validateScene,encodeScene,decodeScene} from '../web/platform/catalog.js';
import {manufacturerNames,productsForManufacturer,productFamilies,productFamilyGroups,productIdentityLabel,productOptionLabel,searchProducts,firmwareGroups,firstFirmware,filterAccessories,searchAccessories} from '../web/platform/catalog-selection.js';
const products=[{id:'m10',manufacturer:'M5Stack',name:'M5Stack Device 10',sku:'C010',chip:'esp32s3'},{id:'m2',manufacturer:'M5Stack',name:'M5Stack Device 2',sku:'C002',chip:'esp32s3'},{id:'a',manufacturer:'Arduino',name:'Arduino Nesso N1',sku:'DK001',chip:'esp32c6'}];
test('manufacturer selection filters while device names use natural order',()=>{
  assert.deepEqual(manufacturerNames(products),['Arduino','M5Stack']);
  assert.deepEqual(productsForManufacturer(products,'M5Stack').map(p=>p.id),['m2','m10']);
  assert.deepEqual(productsForManufacturer(products,'Arduino').map(p=>p.id),['a']);
  assert.equal(products[0].id,'m10','sorting does not mutate provenance catalog order');
});
test('compact search can find a SKU outside the currently selected manufacturer',()=>{
  assert.deepEqual(searchProducts(products,'dk001').map(p=>p.id),['a']);
  assert.deepEqual(searchProducts(products,'M5Stack Ｃ００２').map(p=>p.id),['m2']);
  assert.deepEqual(searchProducts(products,'arduino c6').map(p=>p.id),['a']);
  assert.equal(searchProducts(products,'missing').length,0);
});
test('firmware groups preserve separate vendor distribution and upstream versions',()=>{
  const entries=[{id:'upstream',productIds:['m2'],publisherGroup:'CrossPoint',revision:'1.6.5'},{id:'vendor',productIds:['m2'],publisherGroup:'官方',revision:'1.5.0'},{id:'other',productIds:['a'],publisher:'Other'}];
  assert.deepEqual(firmwareGroups(entries,'m2').map(([name,items])=>[name,items[0].id]),[['官方','vendor'],['CrossPoint','upstream']]);
});
test('factory firmware leads the menu and is selected even before compatibility verification',()=>{
  const entries=[{id:'community',productIds:['m2'],publisher:'CrossPoint',source:{url:'https://example.org/community.bin'}},{id:'official-alternative',productIds:['m2'],publisher:'M5Stack',role:'alternative'},{id:'factory',productIds:['m2'],publisher:'M5Stack',role:'factory',status:'trial',simulationVerified:false,source:{url:'https://example.org/factory.bin'}}];
  assert.equal(firstFirmware(entries,{id:'m2',defaultFirmwareId:'community'}).id,'factory');
  assert.equal(firmwareGroups(entries,'m2')[0][1][0].id,'factory');
  assert.equal(entries[0].id,'community');
  assert.equal(firstFirmware(entries,{id:'absent'}),null);
});
test('an explicit community default leads official alternatives when no factory entry exists',()=>{
  const entries=[{id:'official',productIds:['m2'],publisher:'M5Stack',role:'alternative',source:{url:'https://example.org/official.bin'}},{id:'demo',productIds:['m2'],publisherGroup:'社区',role:'demo',status:'trial',source:{url:'https://example.org/demo.bin'}}];
  const product={id:'m2',defaultFirmwareId:'demo'};
  assert.equal(firstFirmware(entries,product).id,'demo');
  assert.deepEqual(firmwareGroups(entries,product,product.defaultFirmwareId).map(([name,items])=>[name,items[0].id]),[['社区','demo'],['官方','official']]);
  assert.equal(firstFirmware(entries,{id:'m2',defaultFirmwareId:'missing'}).id,'official');
  assert.equal(entries[0].id,'official','selection does not change catalog order');
});
test('accessory filters combine brand and connector; search crosses filters and includes local definitions',()=>{
  const entries=[{id:'gps',name:'GPS',sku:'U032',manufacturer:'M5Stack',connector:{type:'HY2.0-4P'}},{id:'other',name:'Other sensor',manufacturer:'Acme',connector:{type:'USB-C'}},{id:'local:knob',name:'Private knob',connector:{type:'custom'}}];
  assert.deepEqual(filterAccessories(entries,{brand:'Acme',connector:'USB-C'}).map(x=>x.id),['other']);
  assert.equal(filterAccessories(entries,{brand:'M5Stack',connector:'USB-C'}).length,0);
  assert.deepEqual(searchAccessories(entries,'Ｕ０３２').map(x=>x.id),['gps']);
  assert.deepEqual(searchAccessories(entries,'diy knob').map(x=>x.id),['local:knob']);
});

const lilygo=[
  {id:'local-deck-plus',name:'LILYGO T-Deck Plus',manufacturer:'LILYGO',sku:'T-Deck Plus',family:{id:'t-deck-series',name:'T-Deck 系列'},hardwareRevision:'Plus',providers:{'lilygo-spark':{productId:'spark-deck-plus'}},runtime:{board:null,status:'unavailable'}},
  {id:'local-deck-original',name:'LILYGO T-Deck',manufacturer:'LILYGO',sku:'T-Deck',family:{id:'t-deck-series',name:'T-Deck 系列'},hardwareRevision:'original',runtime:{board:null,status:'unavailable'}},
  {id:'local-display',name:'LILYGO T-Display',manufacturer:'LILYGO',family:{id:'t-display-series',name:'T-Display 系列'},hardwareRevision:'unspecified',runtime:{board:'ttgo-tdisplay',status:'partial'}},
  {id:'local-uncategorized',name:'LILYGO Board',manufacturer:'LILYGO',runtime:{board:null,status:'unavailable'}},
];
test('families describe concrete manufacturer models without creating selectable series products',()=>{
  const all=[...products,...lilygo,{id:'other-deck',manufacturer:'Other',name:'Another Deck',family:{id:'t-deck-series',name:'Other family'}}];
  assert.deepEqual(productFamilies(all,'LILYGO'),[{id:'t-deck-series',name:'T-Deck 系列',count:2},{id:'t-display-series',name:'T-Display 系列',count:1}]);
  assert.deepEqual(productFamilies(all,'M5Stack'),[]);
  assert.deepEqual(productsForManufacturer(all,'LILYGO','t-deck-series').map(p=>p.id),['local-deck-original','local-deck-plus']);
  assert.deepEqual(productsForManufacturer(all,'LILYGO','missing'),[]);
  const groups=productFamilyGroups(productsForManufacturer(all,'LILYGO'));
  assert.deepEqual(groups.map(g=>[g.name,g.products.map(p=>p.id)]),[['T-Deck 系列',['local-deck-original','local-deck-plus']],['T-Display 系列',['local-display']],['未分类',['local-uncategorized']]]);
  assert.equal(lilygo[0].id,'local-deck-plus','provenance order is unchanged');
});
test('cross-family search returns exact local product IDs and reports unavailable siblings',()=>{
  assert.deepEqual(searchProducts(lilygo,'LILYGO T-Deck 系列').map(p=>p.id),['local-deck-original','local-deck-plus']);
  assert.deepEqual(searchProducts(lilygo,'spark-deck-plus').map(p=>p.id),['local-deck-plus']);
  assert.deepEqual(searchProducts(lilygo,'T-Deck original').map(p=>p.id),['local-deck-original']);
  assert.equal(productOptionLabel(lilygo[0]),'LILYGO T-Deck Plus × 尚未适配');
  assert.equal(productOptionLabel(lilygo[2]),'LILYGO T-Display');
  assert.equal(productIdentityLabel(lilygo[0]),'LILYGO / T-Deck 系列 / T-Deck Plus / 硬件：Plus');
  assert.equal(productIdentityLabel(lilygo[2]),'LILYGO / T-Display 系列');
  assert.equal(productIdentityLabel({...lilygo[2],hardwareRevision:'unknown; Spark product listing'}),'LILYGO / T-Display 系列');
});


test('picker visibility filters brands, families, groups, search and counts without mutating entries',()=>{
  const entries=[
    {...products[0],family:{id:'main',name:'Main'}},
    {...products[1],pickerVisible:true,family:{id:'main',name:'Main'}},
    {id:'header',manufacturer:'M5Stack',name:'Header',pickerVisible:false,family:{id:'main',name:'Main'}},
    {id:'hidden-only',manufacturer:'Hidden brand',name:'Hidden',pickerVisible:false,family:{id:'hidden',name:'Hidden'}},
  ];
  const before=structuredClone(entries);
  assert.deepEqual(manufacturerNames(entries),['M5Stack']);
  assert.deepEqual(productFamilies(entries,'M5Stack'),[{id:'main',name:'Main',count:2}]);
  assert.deepEqual(productFamilies(entries,'Hidden brand'),[]);
  assert.deepEqual(productsForManufacturer(entries,'M5Stack','main').map(p=>p.id),['m2','m10']);
  assert.deepEqual(productFamilyGroups(entries).map(group=>group.products.map(p=>p.id)),[['m10','m2']]);
  assert.deepEqual(searchProducts(entries,'').map(p=>p.id),['m2','m10']);
  assert.deepEqual(searchProducts(entries,'Header'),[]);
  assert.deepEqual(entries,before);
});

test('hidden visual variants remain exact catalog and portable scene identities',async()=>{
  const fetchOriginal=globalThis.fetch;
  let catalog;
  try { globalThis.fetch=async url=>new Response(await readFile(url,'utf8'));catalog=await loadCatalog(); }
  finally { globalThis.fetch=fetchOriginal; }
  const variants=['m5stack-s007-pin127','m5stack-s007-pin254','m5stack-s007-pin127-v033','m5stack-s007-pin254-v033','m5stack-s015-dip','m5stack-s016-dip','m5stack-k051-b','m5stack-k051','m5stack-k056','m5stack-k122'];
  assert.deepEqual(catalog.products.filter(p=>p.pickerVisible===false).map(p=>p.id).sort(),[...variants].sort());
  assert.equal(searchProducts(catalog.products,'').length,catalog.products.length-variants.length);
  for(const id of variants){
    const product=catalog.products.find(p=>p.id===id),scene=createDefaultScene(catalog,id);
    assert.ok(product.model.url.includes(id),'each assembly keeps its own model');
    assert.ok(product.knowledgeUrl.includes(id));
    assert.equal(validateScene(scene,catalog).valid,true);
    assert.equal(decodeScene(encodeScene(scene,catalog),catalog).productId,id);
    assert.deepEqual(searchProducts(catalog.products,id),[]);
    assert.equal(productsForManufacturer(catalog.products,product.manufacturer,product.family.id).some(p=>p.id===id),false);
  }
  for(const id of ['m5stack-s007','m5stack-s007-v033','m5stack-s015','m5stack-s016','m5stack-c050-b','m5stack-c056-b','m5stack-c122-b','m5stack-k138','m5stack-k138-v11','m5stack-stopwatch','m5stack-triviapod']){
    assert.ok(searchProducts(catalog.products,'').some(p=>p.id===id),id);
  }
});

test('TriviaPOD is discoverable as its own runnable product and preserves its scene identity',async()=>{
  const [products,accessories,firmwares]=await Promise.all(['products','accessories','firmwares'].map(async name=>JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url),'utf8')).items));
  const catalog={products,accessories,firmwares},trivia=products.find(item=>item.id==='m5stack-triviapod');
  assert.deepEqual(searchProducts(products,'TP-01-A').map(item=>item.id),[trivia.id]);
  assert.ok(productsForManufacturer(products,'M5Stack','m5-other').some(item=>item.id===trivia.id));
  assert.doesNotMatch(productOptionLabel(trivia),/尚未适配$/);
  const candidate=firstFirmware(firmwares,trivia);
  assert.equal(candidate.sharedTargetId,'m5stack-stopwatch');assert.equal(candidate.simulationVerified,false);assert.equal(candidate.role,undefined,'shared firmware is not the TriviaPOD factory image');
  const scene=createDefaultScene(catalog,trivia.id);
  assert.equal(validateScene(scene,catalog).valid,true);
  assert.equal(scene.firmware,null);
  assert.equal(decodeScene(encodeScene(scene,catalog),catalog).productId,trivia.id);
});
