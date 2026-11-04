// Apply the separately reviewed candidate to a caller-selected integration tree.
// No implicit working-directory target; no runtime, CAD or firmware binary changes.
import {readFile,writeFile} from 'node:fs/promises';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
const args=process.argv.slice(2),root=args[0];
if(!root||args.some((a,i)=>i>0&&a!=='--default'&&a!=='--check'))throw Error('usage: node apply-dip-firmware.mjs /path/to/integration [--default] [--check]');
const record=JSON.parse(await readFile(new URL('./dip-firmware-candidate.json',import.meta.url),'utf8'));
const names=['products','firmwares','firmware-watchlist'];
const files=names.map(n=>path.join(root,'web/platform/catalog',`${n}.json`));
const [products,firmwares,watchlist]=await Promise.all(files.map(async file=>JSON.parse(await readFile(file,'utf8'))));
const product=products.items.find(p=>p.id===record.productId);
if(!product)throw Error('S016-DIP product must be integrated before its candidate firmware mapping');
const insert=(list,item)=>{const old=list.find(x=>x.id===item.id);if(old&&JSON.stringify(old)!==JSON.stringify(item))throw Error(`Existing ${item.id} differs; review before replacing`);if(!old)list.push(item);};
insert(firmwares.items,record.firmware);insert(watchlist.entries,record.watchEntry);
product.firmwareReferenceIds=[...new Set([...(product.firmwareReferenceIds||[]),record.productReferenceId])];
if(args.includes('--default')){
  if(product.defaultFirmwareId&&product.defaultFirmwareId!==record.firmware.id)throw Error('Product already has a different default; preserve it and review explicitly');
  product.defaultFirmwareId=record.firmware.id;
}
const {validateWatchlist}=await import(pathToFileURL(path.resolve(root,'web/platform/watchlist.js')));
validateWatchlist(watchlist);
if(!args.includes('--check'))for(const [i,value]of [products,firmwares,watchlist].entries())await writeFile(files[i],JSON.stringify(value,null,2)+'\n');
console.log(JSON.stringify({productId:record.productId,firmwareId:record.firmware.id,watchlistId:record.watchEntry.id,defaultSelected:args.includes('--default'),mode:args.includes('--check')?'validated-no-writes':'applied'}));
