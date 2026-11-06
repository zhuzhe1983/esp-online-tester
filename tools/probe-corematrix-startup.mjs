#!/usr/bin/env node
// Original UIFlow startup selection, with configuration written by its own NVS API.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createCoreMatrixRepl} from './fixtures/corematrix-uiflow/raw-repl.mjs';

const [firmwarePath,wasmPath,romPath,receiptPath]=process.argv.slice(2);
if(!receiptPath)throw Error('Usage: probe-corematrix-startup.mjs ORIGINAL_BIN PRODUCTION_WASM C61_ROM_ELF RECEIPT_JSON');
const originalSha='8d5b8d4e22ebc293937086b2f9f218aa5a5ebda27b5ccb32b3f8f580ee02939b';
const bootSha='dfc76de173d2afee2bc5a94a293346b898a76e5ee6e544f6f7f3edac14657efb';
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const [firmware,wasm,rom]=await Promise.all([firmwarePath,wasmPath,romPath].map(path=>readFile(path)));
assert.equal(hash(firmware),originalSha,'Only the pinned unchanged original image is accepted');
const partitions=[];
for(let at=0x8000;at<0x9000&&firmware.readUInt16LE(at)===0x50aa;at+=32)partitions.push({name:firmware.subarray(at+12,at+28).toString().replace(/\0.*$/,''),type:firmware[at+2],subtype:firmware[at+3],offset:firmware.readUInt32LE(at+4),size:firmware.readUInt32LE(at+8)});
assert.deepEqual(partitions.find(p=>p.name==='nvs'),{name:'nvs',type:1,subtype:2,offset:0x9000,size:0x6000});
const root=new URL('../',import.meta.url),origin='https://corematrix-startup.invalid';
const catalog={};for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root))).items;
const fetchImpl=async value=>{
 const u=new URL(value);assert.equal(u.origin,origin);
 if(u.pathname==='/wasm/esp32sim.wasm')return new Response(wasm);
 if(u.pathname==='/wasm/fw/esp32c61_rev100_rom.elf')return new Response(rom);
 try{return new Response(await readFile(new URL('web'+u.pathname,root)));}catch{return new Response('',{status:404});}
};
const workerFactory=url=>{
 const real=new URL('web'+url.pathname,root);
 const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));
 const adapter={postMessage:(message,...args)=>thread.postMessage(message.op==='create'?{...message,jit:false}:message,...args),terminate:()=>thread.terminate()};
 thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',error=>adapter.onerror?.(error));return adapter;
};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'corematrix-startup-isolated'})});
const repl=createCoreMatrixRepl(runtime),ssid='corematrix-nvs-test',password='public-test-pass';
const receipt={sourceSha256:originalSha,wasmSha256:hash(wasm),romSha256:hash(rom),bootScriptSha256:bootSha,partitions,firmwareCodePatched:false,configurationWrittenBy:'original guest esp32.NVS',bootOption:2,checks:[],limitations:['Network-only startup does not supply the absent M5Things cloud module.','The original I2C port-not-initialized startup warning remains.','Virtual WPA2 AP and DHCP, no physical RF or cloud service.','The separate upstream RTC epoch issue #120 is not changed or tested here.']};
let failure;
try {
 const scene=createDefaultScene(catalog,'m5stack-c158');scene.firmware=null;
 await runtime.configure(scene);await runtime.configureNetwork({autoProvision:false,ssid,password});
 await runtime.loadFirmware({file:new File([firmware],'original-corematrix.bin')});await runtime.start();
 const baseline=await repl.wait(s=>s.serial.includes('>>>')&&s.stats.time>1);
 assert.match(baseline.serial,/ImportError: no module named 'M5Things'/);receipt.checks.push('original-default-cloud-menu-missing-module-reproduced');
 await repl.enter();
 const write=await repl.exec(`import esp32,hashlib,binascii\nn=esp32.NVS('uiflow')\nassert n.get_u8('boot_option')==1\nassert binascii.hexlify(hashlib.sha256(open('/flash/boot.py','rb').read()).digest()).decode()=='${bootSha}'\nn.set_str('ssid0',${JSON.stringify(ssid)})\nn.set_str('pswd0',${JSON.stringify(password)})\nn.set_u8('boot_option',2)\nn.commit()\nprint('CM_NVS_CONFIG_WRITTEN',n.get_u8('boot_option'))`);
 assert.match(write,/CM_NVS_CONFIG_WRITTEN 2/);
 const start=await repl.submit("import machine\nprint('CM_NVS_RESET')\nmachine.reset()");
 const after=await repl.wait(s=>{const out=s.serial.slice(start);return out.includes('ESP-ROM:esp32c61')&&out.includes('WiFi initialized')&&out.includes('>>>');});
 await repl.advance(3);
 const boot=after.serial.slice(start);assert.ok(!boot.includes('Traceback'),'Official network-only startup must not raise');assert.ok(!boot.includes("no module named 'M5Things'"));
 receipt.checks.push('guest-reset-original-ROM-boot-network-only-without-M5Things-import');
 await repl.enter();
 const result=await repl.exec(`import esp32,network,hashlib,binascii,json\nn=esp32.NVS('uiflow')\nassert n.get_str('ssid0')==${JSON.stringify(ssid)}\nassert n.get_str('pswd0')==${JSON.stringify(password)}\nassert n.get_str('net_mode')=='WIFI'\nassert n.get_u8('boot_option')==2\nassert binascii.hexlify(hashlib.sha256(open('/flash/boot.py','rb').read()).digest()).decode()=='${bootSha}'\nw=network.WLAN(network.STA_IF)\nassert w.isconnected()\nprint('CM_NVS_STARTUP',json.dumps({'connected':w.isconnected(),'status':w.status(),'ifconfig':w.ifconfig(),'bootOption':n.get_u8('boot_option')}))\ntry:\n import M5Things\n print('CM_M5THINGS_PRESENT')\nexcept ImportError:\n print('CM_M5THINGS_STILL_ABSENT')`);
 assert.match(result,/CM_M5THINGS_STILL_ABSENT/);receipt.guest=JSON.parse(result.match(/CM_NVS_STARTUP ([^\r\n]+)/)[1]);
 assert.deepEqual(receipt.guest.ifconfig,['10.0.2.15','255.255.255.0','10.0.2.2','10.0.2.3']);
 receipt.checks.push('original-startup-NVS-WPA2-association-and-DHCP','guest-NVS-typed-readback-after-reset','original-boot.py-hash-preserved','cloud-module-absence-not-hidden');
 receipt.startupSerial=boot.split(/\r?\n/).filter(s=>/Startup with|WiFi initialized|Traceback|M5Things|I2C|i2c/.test(s));
 receipt.network=await runtime.readNetwork();await runtime.pause();
} catch(error){failure=String(error.stack||error);process.exitCode=1;}
finally {
 const state=await runtime.observe().catch(()=>({}));receipt.network=await runtime.readNetwork().catch(()=>null);
 receipt.status=failure?'failed':'pass';receipt.checkedAt=new Date().toISOString();receipt.guestSeconds=state.stats?.time;
 if(failure){receipt.failure=failure;receipt.serialTail=state.serial?.slice(-4000);}
 await runtime.dispose();await writeFile(receiptPath,JSON.stringify(receipt,null,2)+'\n');
 console.log(JSON.stringify({status:receipt.status,checks:receipt.checks,guest:receipt.guest,failure}));
}
