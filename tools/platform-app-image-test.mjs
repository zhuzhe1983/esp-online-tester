import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {validateAppImage} from '../web/platform/app-image.js';
import {createFirmwareHistory} from '../web/platform/firmware-history.js';
import {burnerHistoryFirmware} from '../web/platform/m5burner.js';
import {validateScene,createDefaultScene,encodeScene,decodeScene} from '../web/platform/catalog.js';
import {prepareFlashPlan} from '../web/platform/flasher.js';

const app=new Uint8Array(await readFile(new URL('../web/wasm/fw/public/hello_world.bin',import.meta.url)));
const good=await validateAppImage(app,'stopwatch',16*1024*1024);assert.equal(good.chipId,9);
const reject=(bytes,code,board='stopwatch',size=16*1024*1024)=>assert.rejects(()=>validateAppImage(bytes,board,size),{code});
await reject(app,'CHIP_MISMATCH','esp32c3');await reject(app,'CHIP_MISMATCH','atom-lite');await reject(app,'CHIP_MISMATCH','atom-matrix');await reject(app,'CHIP_MISMATCH','atom-matrix-v11');await reject(app,'FLASH_OVERFLOW','stopwatch',65536);
await reject(app.subarray(0,app.length-40),'APP_IMAGE_INVALID');
const damaged=app.slice();damaged[300]^=1;await reject(damaged,'APP_IMAGE_CHECKSUM');
const digest=app.slice();digest[good.imageBytes-1]^=1;await reject(digest,'APP_IMAGE_DIGEST');
const descriptor=app.slice();descriptor[32]=0;await reject(descriptor,'APP_IMAGE_INVALID');
const boot=new Uint8Array(await readFile(new URL('../web/wasm/fw/public/hello-bootloader.bin',import.meta.url)));await reject(boot,'APP_IMAGE_INVALID');
const trailer=new Uint8Array(app.length+10);trailer.set(app);trailer[app.length]=0x42;await reject(trailer,'APP_IMAGE_INVALID');

const c6app=new Uint8Array(await readFile(new URL('../web/wasm/fw/public/c6-hello_world.bin',import.meta.url)));
assert.equal((await validateAppImage(c6app,'nesso-n1',16*1024*1024)).chipId,13);
await reject(app,'CHIP_MISMATCH','nesso-n1');

const memory=new Map(),storage={getItem:k=>memory.get(k),setItem:(k,v)=>memory.set(k,v)};
const h=createFirmwareHistory(()=>storage),url='https://burner.m5stack.com/api/v1/firmwares/123/versions/456/download';
const item={url,provider:'m5burner',deviceId:'789',kind:'merged',productId:'m5stack-stopwatch',name:'App',version:'1.0',lastUsedAt:1};
h.remember(item);h.remember({...item,lastUsedAt:2,sha256:'a'.repeat(64),bootMode:'app-direct'});
const restored=createFirmwareHistory(()=>storage).list();assert.equal(restored.length,1);assert.equal(restored[0].bootMode,'app-direct');
const source=burnerHistoryFirmware(restored[0]).source;assert.equal(source.bootMode,'app-direct');
assert.equal(h.remember({...item,bootMode:'unknown'}),false);
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${key}.json`,import.meta.url))).items;
const scene=createDefaultScene(catalog);scene.firmware=source;
assert.equal(validateScene(scene,catalog).valid,true);assert.equal(decodeScene(encodeScene(scene)).firmware.bootMode,'app-direct');
await assert.rejects(()=>prepareFlashPlan({scene,catalog,fetchImpl:()=>{throw Error('must not fetch');}}),{code:'FLASH_LAYOUT'});
scene.firmware={manifestUrl:'https://example.com/plan.json',bootMode:'app-direct'};assert.equal(validateScene(scene,catalog).valid,false);
console.log('ok app-only checksum/digest/chip/layout checks, pinned mode history, portable source, hardware flashing refusal');
