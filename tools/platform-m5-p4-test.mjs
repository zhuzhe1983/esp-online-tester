import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {tab5ProbeImage} from './fixtures/tab5-guest.mjs';
import {chipForBoard,validateAppImage,validateBoardRevision} from '../web/platform/app-image.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const catalog={};for(const k of ['products','accessories','firmwares'])catalog[k]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${k}.json`,import.meta.url))).items;
test('M5 P4 image identity and exact virtual revision reject sibling SoCs and excluded die families',async()=>{
 const image=tab5ProbeImage();for(const board of ['m5stack-tab5','m5stack-poe-p4']){assert.equal(chipForBoard(board),'esp32p4');await validateAppImage(image,board,16<<20);const rev=image.slice();new DataView(rev.buffer).setUint16(15,300,true);assert.throws(()=>validateBoardRevision(rev,board),{code:'CHIP_REVISION_UNSUPPORTED'});const max=image.slice();new DataView(max.buffer).setUint16(17,99,true);assert.throws(()=>validateBoardRevision(max,board),{code:'CHIP_REVISION_UNSUPPORTED'});const other=image.slice();new DataView(other.buffer).setUint16(12,13,true);await assert.rejects(validateAppImage(other,board,16<<20),{code:'CHIP_MISMATCH'});}
});
test('Tab5 and PoE-P4 expose distinct touch/network capabilities and retain unverified factory identity',async()=>{
 for(const [id,board,touch,wired]of [['m5stack-c145','m5stack-tab5',true,false],['m5stack-u213','m5stack-poe-p4',false,true]]){
  const p=catalog.products.find(p=>p.id===id);assert.equal(p.runtime.board,board);assert.equal(p.runtime.status,'partial');assert.equal(!!p.runtime.touch,touch);const factory=catalog.firmwares.find(f=>f.id===p.defaultFirmwareId);assert.equal(factory.role,'factory');assert.equal(factory.simulationVerified,false);
  const runtime=createPlatformRuntime({catalog});try{await runtime.configure(createDefaultScene(catalog,id));const cap=await runtime.capabilities();assert.equal(cap.input.touch,touch);assert.equal(cap.network.available,wired);assert.equal(cap.network.softAp,false);if(wired){assert.equal(cap.network.transport,'wired-ethernet-websocket');assert.equal(cap.network.primaryInterface,'native-rmii');assert.deepEqual(cap.network.automaticProvisioning,[]);}}finally{await runtime.dispose();}
 }
});
