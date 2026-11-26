import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {chipForBoard,chipIdForBoard,validateAppImage} from '../web/platform/app-image.js';
import {M5_CLASSIC_CAMERA_BOARDS,isM5ClassicCameraBoard} from '../web/platform/m5-camera-profiles.js';
import {firmwareBoardMatches,hasSdCard} from '../web/platform/board-compatibility.js';
const read=async p=>JSON.parse(await readFile(new URL('../'+p,import.meta.url),'utf8'));
const products=(await read('web/platform/catalog/products.json')).items;
const firmwares=(await read('web/platform/catalog/firmwares.json')).items;
const report=await read('docs/platform/research/m5-classic-camera-runtime.json');

test('fourteen exact M5 camera SKUs select classic LX6 and retain separate factory identities',()=>{
 assert.equal(report.products.length,14);assert.equal(M5_CLASSIC_CAMERA_BOARDS.length,15);
 for(const r of report.products){const p=products.find(p=>p.id===r.id);assert.equal(p.runtime.board,r.id);assert.equal(p.runtime.status,'partial');assert.equal(chipForBoard(p.runtime.board),'esp32');assert.equal(chipIdForBoard(p.runtime.board),0);assert.equal(p.display,null);assert.equal(hasSdCard(p.runtime.board),false);
  assert.equal(p.runtime.flashMb,r.flashMiB);assert.equal(p.runtime.psramMb,r.configuredPsramMiB);assert.equal(p.defaultFirmwareId,r.defaultFirmwareId);
  const f=firmwares.find(f=>f.id===p.defaultFirmwareId);assert.deepEqual(f.productIds,[r.id]);assert.equal(f.simulationVerified,false);assert.equal(f.hardwareValidated,false);assert.notEqual(f.status,'verified');assert.equal(p.firmwareTargetId,undefined,'shared sensor code must not borrow a sibling firmware target');
  const k=report.products.filter(x=>x.id!==r.id);for(const other of k)assert.equal(firmwareBoardMatches(r.board,other.board),false);
 }
 assert.equal(chipForBoard('m5stack-u017-b'),'esp32');assert.equal(isM5ClassicCameraBoard('m5stack-u174'),false);assert.equal(isM5ClassicCameraBoard('m5stack-u017-random'),false);
});

test('classic camera selection accepts actual LX6 app and rejects an S3 app before execution',async()=>{
 const classic=new Uint8Array(await readFile(new URL('../esp32/tests/fixtures/classic-camera/classic_camera.bin',import.meta.url)));
 const s3=new Uint8Array(await readFile(new URL('../web/wasm/fw/public/hello_world.bin',import.meta.url)));
 for(const board of M5_CLASSIC_CAMERA_BOARDS){assert.equal((await validateAppImage(classic,board,4<<20)).chipId,0);await assert.rejects(()=>validateAppImage(s3,board,4<<20),{code:'CHIP_MISMATCH'});}
});

test('source audit preserves revision and unknown-capacity boundaries',async()=>{
 for(const id of ['m5stack-u007','m5stack-u109','m5stack-u109-x','m5stack-u109-b']){const r=report.products.find(p=>p.id===id);assert.equal(r.physicalPsramMiB,null);assert.equal(r.configuredPsramMiB,0);assert.ok(products.find(p=>p.id===id).limitations.some(s=>s.includes('PSRAM容量')));}
 const get=id=>report.products.find(p=>p.id===id);assert.equal(get('m5stack-u007').dvpD0D7PclkHrefVsync[0],17);assert.deepEqual(get('m5stack-u017').sccbSdaScl,[25,23]);assert.deepEqual(get('m5stack-u017-pcba').sccbSdaScl,[22,23]);assert.equal(get('m5stack-u121-b').sensor,'OV2640');assert.equal(get('m5stack-u121-b-v11').sensor,'OV3660');
 assert.equal(report.sources.length,15);for(const source of report.sources){assert.match(source.url,/^https:\/\//);assert.match(source.sha256,/^[a-f0-9]{64}$/);assert.ok(source.bytes>1000);assert.ok(Number.isFinite(Date.parse(source.fetchedAt)));assert.equal(source.retained,false,'metadata receipts are not claimed to retain original source bytes');}
});
