import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {test} from 'node:test';
import {chipForBoard,chipIdForBoard} from '../web/platform/app-image.js';
import {hasSdCard,firmwareBoardMatches} from '../web/platform/board-compatibility.js';
const root=new URL('../',import.meta.url);
const read=async p=>JSON.parse(await readFile(new URL(p,root),'utf8'));
const products=(await read('web/platform/catalog/products.json')).items;
const firmwares=(await read('web/platform/catalog/firmwares.json')).items;
const report=await read('docs/platform/research/m5-classic-paper-runtime.json');

test('two exact board identities select classic LX6 without inheriting factory runs',()=>{
 for(const item of report.products){
  const p=products.find(p=>p.id===item.id);assert.equal(p.runtime.board,item.board);assert.equal(p.runtime.status,'partial');
  assert.equal(chipForBoard(item.board),'esp32');assert.equal(chipIdForBoard(item.board),0);assert.equal(hasSdCard(item.board),item.board==='m5paper-v11');
  const f=firmwares.find(f=>f.id===p.defaultFirmwareId);assert.deepEqual(f.productIds,[item.id]);assert.equal(f.simulationVerified,false);assert.equal(f.role,'factory');
  assert.equal(firmwares.filter(f=>f.productIds?.includes(item.id)&&f.simulationVerified===true).length,0);
 }
 assert.equal(firmwareBoardMatches('m5paper-v11','coreink-d67'),false);
 assert.equal(firmwareBoardMatches('papers3','m5paper-v11'),false);
 const c=products.find(p=>p.id==='m5stack-k048');assert.equal(c.display.controller,'GDEY0154D67 / SSD1681');assert.equal(c.runtime.psramMb,0);
 assert.equal(c.controls.find(c=>c.id==='button-power').pin,27);assert.ok(c.limitations.some(x=>x.includes('GDEW0154M09')));
});

test('offline pinned driver evidence has exact source byte lengths and SHA-256',async()=>{
 const r=await read('docs/platform/evidence/m5-classic-paper-runtime/receipt.json');let checked=0;
 for(const source of r.sources.filter(s=>s.retained)){
  const b=await readFile(new URL(`docs/platform/evidence/m5-classic-paper-runtime/${source.localFile}`,root));
  assert.equal(b.length,source.bytes,source.localFile);assert.equal(createHash('sha256').update(b).digest('hex'),source.sha256,source.localFile);
  assert.match(source.url,/^https:\/\/raw\.githubusercontent\.com\/(m5stack|lovyan03)\//);assert.match(source.fetchedAt,/Z$/);checked++;
 }
 assert.ok(checked>=12);assert.equal(report.virtualPolicies.busyMilliseconds,20);
});
