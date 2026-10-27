// Diagnostic only: caller supplies an extracted official package and a pinned remote firmware.
// Firmware bytes stay in RAM. No product runtime, board peripheral or ROM patch is installed.
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
const [pkg,url,expectedHash,out='output/s31-official-wasm',seconds='10']=process.argv.slice(2);
if(!pkg||!/^https:\/\//.test(url||'')||!/^[a-f0-9]{64}$/.test(expectedHash||''))throw Error('Supply extracted official pkg path, HTTPS firmware URL, SHA256, output path, and guest seconds.');
const target=Number(seconds);if(!(target>0&&target<=120))throw Error('Invalid guest time');
await mkdir(out,{recursive:true});const hash=b=>createHash('sha256').update(b).digest('hex');
const packageInfo=JSON.parse(await readFile(resolve(pkg,'package.json'),'utf8'));
const wasmBytes=await readFile(resolve(pkg,'esp_emu_bg.wasm')),jsBytes=await readFile(resolve(pkg,'esp_emu.js'));
const {default:init,WasmEmulator}=await import(pathToFileURL(resolve(pkg,'esp_emu.js')));await init({module_or_path:wasmBytes});
const response=await fetch(url,{cache:'no-store'});if(!response.ok)throw Error('Firmware HTTP '+response.status);
const firmware=new Uint8Array(await response.arrayBuffer());if(hash(firmware)!==expectedHash)throw Error('Firmware SHA mismatch');
const receipt={chip:'esp32s31',backend:`esp-emulator ${packageInfo.version} WASM (Node)`,networkBridge:false,additionalRomPatches:false,wasmSha256:hash(wasmBytes),jsSha256:hash(jsBytes),firmware:{url,finalUrl:response.url,fetchedAt:new Date().toISOString().replace(/\.\d{3}Z$/,'Z'),mediaType:response.headers.get('content-type'),bytes:firmware.length,sha256:expectedHash,storage:'RAM-only'},snapshots:[]};
const e=new WasmEmulator('esp32s31');let serial='',requested=0,stopped='budget';const start=performance.now();
const snap=()=>({pc:e.pc(),pcHex:e.pc().toString(16),hart1Pc:e.hart_pc(1),harts:e.num_harts(),cycles:String(e.cycles()),cyclesPerUs:e.cycles_per_us(),registers:Array.from(e.registers()),instructionBytes:Array.from(e.read_memory(e.pc(),16)),needsRestart:e.needs_restart()});
try {
 receipt.hasDefaultRom=e.has_default_rom();if(!receipt.hasDefaultRom)throw Error('No embedded default S31 ROM');e.load_default_rom();e.set_boot_from_rom(true);e.load_firmware(firmware);receipt.loaded=snap();
 let last=-1;
 while(requested<2e9&&performance.now()-start<120000){serial+=e.run_batch(50000);requested+=50000;const guest=Number(e.cycles())/(e.cycles_per_us()*1e6);const sec=Math.floor(guest);if(sec!==last){last=sec;receipt.snapshots.push(snap());process.stderr.write(JSON.stringify({guestSeconds:guest,pc:e.pc().toString(16),requested})+'\n');}if(e.needs_restart()){stopped='guest-restart-request';break;}if(guest>=target){stopped='guest-time';break;}}
 receipt.stop=stopped;receipt.final=snap();
} catch(error){receipt.error=String(error);receipt.final=snap();process.exitCode=1;}
finally {receipt.requestedInstructions=requested;receipt.wallMs=performance.now()-start;receipt.serialSha256=hash(serial);receipt.serialBytes=Buffer.byteLength(serial);await writeFile(out+'/serial.log',serial);await writeFile(out+'/receipt.json',JSON.stringify(receipt,null,2)+'\n');e.free();}
