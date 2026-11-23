// Loopback-only preview. Public upstream image/ROM stay outside Git.
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {resolve,extname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {createDefaultScene,encodeScene} from '../web/platform/catalog.js';
export async function startStickT2Preview({port=8792, useCatalogSource=false}={}) {
 const root=resolve(fileURLToPath(new URL('../web',import.meta.url))),sha=bytes=>createHash('sha256').update(bytes).digest('hex');
 const bin=await readFile(process.env.STICKT2_FACTORY_BIN||'target/stickt2-research/factory-original.bin');
 if(sha(bin)!=='bb33ffd41f9c6f9235b2fb44eac893b7ce6f70879f48eed9cb1ff75ef9419cd7')throw Error('Original StickT2 factory SHA256 mismatch; never patch its bootloader.');
 if(!process.env.ESP32SIM_ROM_DIR)throw Error('Set ESP32SIM_ROM_DIR to the existing public ROM directory.');
 const rom=await readFile(resolve(process.env.ESP32SIM_ROM_DIR,'esp32_rev300_rom.elf'));
 if(sha(rom)!=='920b70635440517866aab2230964a570d2cf2b676658d93c52fbac108c1cca31')throw Error('Public ROM hash mismatch');
 const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(resolve(root,`platform/catalog/${key}.json`),'utf8')).items;
 const scene=createDefaultScene(catalog,'m5stack-k016-t2');
 const factory=catalog.firmwares.find(f=>f.id==='m5stack-k016-t2-official-factory');
 if(factory?.source.sha256!==sha(bin))throw Error('StickT2 catalog factory pin does not match the fixture.');
 const types={'.js':'text/javascript','.mjs':'text/javascript','.html':'text/html','.json':'application/json','.wasm':'application/wasm','.css':'text/css','.png':'image/png','.svg':'image/svg+xml'};
 const server=createServer(async(req,res)=>{try{const pathname=decodeURIComponent(new URL(req.url,'http://127.0.0.1').pathname);let bytes;
 if(pathname==='/__stickt2_factory.bin')bytes=bin;
 else if(pathname.endsWith('/esp32_rev300_rom.elf'))bytes=rom;
 else {const path=resolve(root,'.'+(pathname==='/'?'/platform/index.html':pathname));if(!path.startsWith(root+'/'))throw Error('Forbidden path');bytes=await readFile(path);}
 res.writeHead(200,{'Content-Type':types[extname(pathname)]||'application/octet-stream','Cache-Control':'no-store'});res.end(bytes);
 }catch{res.writeHead(404);res.end('Not found');}});
 await new Promise((ok,fail)=>{server.once('error',fail);server.listen(port,'127.0.0.1',ok);});
 const origin=`http://127.0.0.1:${server.address().port}`;
 scene.firmware=useCatalogSource?factory.source:{url:origin+'/__stickt2_factory.bin',sha256:factory.source.sha256};scene.view.deviceView='2d';
 const url=origin+'/platform/index.html#'+new URLSearchParams({scene:encodeScene(scene),autostart:'0'});
 return {origin,url,scene,bin,close:()=>new Promise(ok=>server.close(ok))};
}
if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
 const preview=await startStickT2Preview({port:Number(process.argv[process.argv.indexOf('--port')+1])||8792});console.log(preview.url);console.log('Loopback-only; synthetic Raw14 counts are NOT Celsius. Ctrl-C stops this preview.');
 for(const signal of ['SIGINT','SIGTERM'])process.once(signal,async()=>{await preview.close();process.exit();});
}
