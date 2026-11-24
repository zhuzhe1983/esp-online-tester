// Project-authored acceptance SD, not a manufacturer disk image. Original
// upstream 1.bmp is consumed by the unmodified guest through real SDMMC/FAT.
// Config intentionally contains no service endpoint or credential. The public
// upstream config is not copied because it contains a credential-like value.
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createFat32Image} from '../../../web/platform/sd-card-image.js';
const [input,output]=process.argv.slice(2);
if(!input||!output)throw new Error('Usage: node make-sd-card.mjs <original-1.bmp> <output.img>');
const bmp=await readFile(input),hash=b=>createHash('sha256').update(b).digest('hex');
if(hash(bmp)!=='6ea4d6ac8bc6850ce272baa0cad9bb2d098f3212cccc824be3103a3a324fae1a')throw new Error('Original BMP SHA-256 mismatch');
const bytes=createFat32Image(),v=new DataView(bytes.buffer),enc=new TextEncoder();
const start=v.getUint32(454,true),boot=start*512,reserved=v.getUint16(boot+14,true),fat=v.getUint32(boot+36,true);
const data=(start+reserved+2*fat)*512,clusterAt=n=>data+(n-2)*512;
let next=3;
function allocate(body){
 const first=next,count=Math.max(1,Math.ceil(body.length/512));next+=count;
 if(clusterAt(next)>bytes.length)throw new Error('SD capacity exceeded');
 for(let f=0;f<2;f++)for(let i=0;i<count;i++)v.setUint32((start+reserved+f*fat)*512+(first+i)*4,i===count-1?0x0fffffff:first+i+1,true);
 bytes.set(body,clusterAt(first));return first;
}
function entry(at,name,cluster,length,dir=false){
 const [base,ext='']=name.split('.');if(base.length>8||ext.length>3)throw new Error('8.3 name required');
 bytes.set(enc.encode(base.toUpperCase().padEnd(8)+ext.toUpperCase().padEnd(3)),at);bytes[at+11]=dir?0x10:0x20;bytes[at+12]=0x18;
 v.setUint16(at+20,cluster>>>16,true);v.setUint16(at+26,cluster&65535,true);v.setUint32(at+28,length,true);
}
const dir=allocate(new Uint8Array(512));entry(data+32,'img',dir,0,true);
// FAT directory dot entries are conventional literal short names.
for(const [offset,name,cluster] of [[0,'.          ',dir],[32,'..         ',0]]){const at=clusterAt(dir)+offset;bytes.set(enc.encode(name),at);bytes[at+11]=0x10;v.setUint16(at+26,cluster,true);}
const files=[['1.bmp',bmp],['fileList.txt',enc.encode('1.bmp\n')],['index.txt',enc.encode('0')],['config.txt',enc.encode(JSON.stringify({timer:1200,ai_model:'',ai_url:'',ai_key:''})+'\n')]];
for(const [i,[name,body]] of files.entries()){entry(clusterAt(dir)+(i+2)*32,name,allocate(body),body.length);}
const used=next-3;for(const offset of [512,7*512]){v.setUint32(boot+offset+488,v.getUint32(boot+offset+488,true)-used,true);v.setUint32(boot+offset+492,next,true);}
await writeFile(output,bytes);
const receipt={kind:'project-authored-acceptance-sd',format:'fat32',bytes:bytes.length,sha256:hash(bytes),files:files.map(([name,body])=>({path:`/img/${name}`,bytes:body.length,sha256:hash(body),role:name==='1.bmp'?'unmodified-official-test-image':'project-generated-test-configuration'})),notes:['No firmware bytes changed.','No service requests or credentials are configured.','Only the BMP is original upstream content; this filesystem and its other files are a test fixture.']};
await writeFile(output+'.json',JSON.stringify(receipt,null,2)+'\n');console.log(JSON.stringify(receipt));
