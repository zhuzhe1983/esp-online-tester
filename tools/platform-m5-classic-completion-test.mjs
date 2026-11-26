import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {chipForBoard,chipIdForBoard} from '../web/platform/app-image.js';
import {hasSdCard,firmwareBoardMatches} from '../web/platform/board-compatibility.js';
const catalog=JSON.parse(await readFile(new URL('../web/platform/catalog/products.json',import.meta.url))).items;
const firmware=JSON.parse(await readFile(new URL('../web/platform/catalog/firmwares.json',import.meta.url))).items;
const profiles=[['m5stack-k016-p-se','stickc-plus-se',4,0],['m5stack-k016-p2','stickc-plus2',8,2],['m5stack-k016-b','m5stick-oled',4,0],['m5stack-k049','m5paper-original',16,8],['m5stack-k049-c','m5paper-comm',16,8],['m5stack-k016-t','stickt-original',4,0],['m5stack-k126','tlite',4,0],['m5stack-k014-b','bala-fire-original',16,4]];
test('exact classic identities retain capacities, product firmware identity and explicit partial scope',()=>{
 for(const[id,board,flash,psram]of profiles){const p=catalog.find(p=>p.id===id);assert(p);assert.equal(p.runtime.board,board);assert.equal(p.runtime.status,'partial');assert.deepEqual([p.runtime.flashMb,p.runtime.psramMb],[flash,psram]);assert.equal(chipForBoard(board),'esp32');assert.equal(chipIdForBoard(board),0);assert.equal(hasSdCard(board),board.startsWith('m5paper-')||board==='bala-fire-original');const f=firmware.find(f=>p.defaultFirmwareId?f.id===p.defaultFirmwareId:f.productIds?.includes(id));assert(f.productIds.includes(id));assert.notEqual(f.simulationVerified,true);assert(p.limitations.some(x=>x.includes('未实机验证')));}
 assert.equal(firmwareBoardMatches('stickc-plus','stickc-plus-se'),false);assert.equal(firmwareBoardMatches('stickc-plus','stickc-plus2'),false);assert.equal(firmwareBoardMatches('m5paper-v11','m5paper-original'),false);assert.equal(firmwareBoardMatches('m5paper-v11','m5paper-comm'),false);
 assert.deepEqual(catalog.find(p=>p.id==='m5stack-k016-p2').controls.map(c=>c.pin),[37,39,35]);
 assert.equal(catalog.find(p=>p.id==='m5stack-k016-b').controls[0].pin,35);
});
