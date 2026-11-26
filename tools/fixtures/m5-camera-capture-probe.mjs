// Original LX7 SRAM program. The host only supplies an image to the sensor.
// Executed GPIO/I2C/LCD_CAM/GDMA instructions configure/capture it, then the
// guest reads RAM and emits its first 16 bytes and full standard CRC32 on UART0.
// ISA: xtensa-lx7/src/decode.rs and jit/wasm_tests/asm.rs. Hardware sources:
// docs/platform/research/{m5-cams3-runtime,atoms3r-capture}-20261126/sources.json.
import {createHash} from 'node:crypto';
export const M5_CAMERA_PROFILES=Object.freeze([
 {id:'m5stack-u174',board:'unit-cams3',sensor:'ov2640',width:8,height:4,sda:17,scl:41,xclk:11,reset:21,data:[6,15,16,7,5,10,4,13,12,18,42]},
 {id:'m5stack-u174-b',board:'unit-cams3-5mp',sensor:'py260',width:96,height:96,sda:17,scl:41,xclk:11,reset:21,data:[6,15,16,7,5,10,4,13,12,18,42]},
 {id:'m5stack-c126-cam',board:'atoms3r-cam',sensor:'gc0308',width:8,height:4,sda:12,scl:9,xclk:21,power:18,data:[3,42,46,48,4,17,11,13,40,14,10]},
 {id:'m5stack-k147-cam',board:'atoms3r-cam-chatbot',sensor:'gc0308',width:8,height:4,sda:12,scl:9,xclk:21,power:18,data:[3,42,46,48,4,17,11,13,40,14,10]},
 {id:'m5stack-c126-m12',board:'atoms3r-m12',sensor:'ov3660',width:8,height:4,sda:12,scl:9,xclk:21,power:18,data:[3,42,46,48,4,17,11,13,40,14,10]},
 {id:'m5stack-d062-m12',board:'atoms3r-m12-volcengine',sensor:'ov3660',width:8,height:4,sda:12,scl:9,xclk:21,power:18,data:[3,42,46,48,4,17,11,13,40,14,10]},
].map(p=>Object.freeze({...p,data:Object.freeze(p.data)})));
const IRAM=0x40370000,POOL=4096,ENTRY=IRAM+POOL,GPIO=0x60004000,I2C=0x60013000,LCD=0x60041000,GDMA=0x6003f000,DESC=0x3fc88000,DATA=0x3fc90000;
export function m5CameraCaptureProbe(id,{format='rgb565',jpegBytes=0,fault=null,timeoutCycles=60_000_000}={}) {
 const p=M5_CAMERA_PROFILES.find(p=>p.id===id);if(!p)throw Error('Unknown exact camera SKU');
 if(!['rgb565','jpeg'].includes(format)||format==='jpeg'&&(p.sensor==='gc0308'||!Number.isInteger(jpegBytes)||jpegBytes<16||jpegBytes>65536))throw Error('Unsupported diagnostic format/size');
 if(!Number.isInteger(timeoutCycles)||timeoutCycles<1||timeoutCycles>240_000_000)throw Error('Invalid guest timeout');
 if(![null,'route','clock','power','owner'].includes(fault))throw Error('Unknown diagnostic fault');
 const length=format==='jpeg'?jpegBytes:p.width*p.height*2;
 const code=[],labels=new Map(),fixups=[],literals=new Map();let unique=0;
 const emit=w=>code.push(w&0xffffff),rri=(r,s,t,n)=>emit(2|(t<<4)|(s<<8)|(r<<12)|((n&255)<<16));
 const label=n=>{if(labels.has(n))throw Error('Duplicate label '+n);labels.set(n,ENTRY+code.length*3);};
 const ref=(type,name,args={})=>{fixups.push({at:code.length,type,name,...args});emit(0);};
 const li=(r,v)=>{v>>>=0;if(!literals.has(v))literals.set(v,literals.size*4);ref('literal',v,{r});};
 const store=(t,s,n=0)=>rri(6,s,t,n/4),load=(t,s,n=0)=>rri(2,s,t,n/4),load8=(t,s,n=0)=>rri(0,s,t,n),addi=(t,s,n)=>rri(12,s,t,n);
 const logic=(op,r,s,t)=>emit((t<<4)|(s<<8)|(r<<12)|(op<<20));
 const srli=(r,t,n)=>{if(n>15){srli(r,t,15);srli(r,r,n-15);}else emit((4<<20)|(1<<16)|(r<<12)|(n<<8)|(t<<4));};
 const jump=n=>ref('jump',n),bz=(n,r,nonzero=false)=>ref('zero',n,{r,nonzero}),bltu=(n,s,t)=>ref('unsigned',n,{s,t});
 const ccount=r=>emit((3<<16)|(234<<8)|(r<<4));
 const write=(a,v)=>{li(3,a);li(2,v);store(2,3);};
 const high=(pin,v)=>write(GPIO+(pin<32?(v?8:12):(v?20:24)),1<<(pin%32));
 const route=(pin,s)=>{write(0x60009004+pin*4,(1<<12)|(1<<9));write(GPIO+0x554+pin*4,s);if(s===256)write(GPIO+(pin<32?0x24:0x30),1<<(pin%32));};
 const uart=text=>{for(const c of text)write(0x60000000,c.charCodeAt(0));};
 // The printer and standard polynomial run entirely in the guest. Node's
 // independent zlib.crc32 validates the printed value against expected bytes.
 const hex=(r,digits)=>{for(let n=digits-1;n>=0;n--){const digit='digit-'+unique++,out='digit-out-'+unique++;if(n)srli(4,r,n*4);else addi(4,r,0);li(2,15);logic(1,4,4,2);li(5,10);bltu(digit,4,5);addi(4,4,87);jump(out);label(digit);addi(4,4,48);label(out);li(3,0x60000000);store(4,3);}};
 const sensorWrite=(reg,value)=>{
  const address={ov2640:0x30,py260:0x1f,gc0308:0x21,ov3660:0x3c}[p.sensor];
  const bytes=p.sensor==='ov2640'||p.sensor==='gc0308'?[address<<1,reg,value]:[address<<1,reg>>8,reg&255,value];
  write(I2C+0x24,0xffffffff);write(I2C+0x18,3<<12);for(const b of bytes)write(I2C+0x1c,b);
  for(const[i,v]of[6<<11,(1<<11)|(1<<8)|bytes.length,2<<11].entries())write(I2C+0x58+4*i,v);
  write(I2C+4,(1<<4)|(1<<5));const wait='i2c-'+unique++;label(wait);li(3,I2C+0x20);load(4,3);li(2,(1<<7)|(1<<10));logic(1,4,4,2);bz(wait,4);
 };
 if(p.power!==undefined){route(p.power,256);high(p.power,fault==='power');}
 if(p.reset!==undefined){route(p.reset,256);high(p.reset,fault!=='power');}
 for(const[pin,s]of[[p.sda,90],[p.scl,89]]){route(pin,s);write(GPIO+0x154+4*s,0x80|pin);}
 const regs=p.sensor==='ov2640'?[[0xff,0],[0x5a,p.width/4],[0x5b,p.height/4],[0x5c,0],[0xda,format==='jpeg'?0x12:8]]
  :p.sensor==='py260'?[[0x121,7],[0x120,format==='jpeg'?1:0x10]]
  :p.sensor==='gc0308'?[[0xfe,1],[0x54,0x11],[0xfe,0],[0x25,15],[0x24,0xa6],[0x46,0x80],[0x49,0],[0x4a,p.height],[0x4b,0],[0x4c,p.width]]
  :[[0x3808,0],[0x3809,p.width],[0x380a,0],[0x380b,p.height],[0x4300,format==='jpeg'?0x30:0x61],[0x3821,format==='jpeg'?0x20:0],[0x3008,2]];
 for(const[r,v]of regs)sensorWrite(r,v);
 for(const[i,pin]of p.data.entries()){
  route(pin,256);write(GPIO+(pin<32?0x28:0x34),1<<(pin%32));const s=i<8?133+i:[149,150,152][i-8];write(GPIO+0x154+4*s,0x80|(fault==='route'&&i===0?0:pin));
 }
 route(p.xclk,fault==='clock'?256:149);if(fault==='clock')high(p.xclk,true);
 write(0x600c001c,1<<8);write(LCD+4,(3<<29)|(8<<9));write(LCD+0x64,1<<2);
 uart('CAMERA SRAM READY '+p.sensor+' '+format+'\n');
 label('capture');write(LCD+8,0);write(GDMA+0x20,1<<21);write(GDMA+0x14,0xffffffff);
 // Fill a memory sentinel before every new receive. Byte length can be odd for
 // JPEG, so round only this fill up; the armed descriptor length stays exact.
 li(6,DATA);li(7,Math.ceil(length/4));li(10,0xa5a5a5a5);label('fill');store(10,6);addi(6,6,4);addi(7,7,-1);bz('fill',7,true);
 for(let at=0,index=0;at<length;index++){
  const n=Math.min(length-at,4092),next=at+n===length?0:DESC+12*(index+1);write(DESC+12*index,(fault==='owner'?0:1<<31)|n);write(DESC+12*index+4,DATA+at);write(DESC+12*index+8,next);at+=n;
 }
 write(GDMA+4,1<<12);write(GDMA+0x48,5);write(GDMA+0x20,(DESC&0xfffff)|(1<<22));write(LCD+8,(1<<29)|(length-1));
 ccount(8);li(9,timeoutCycles);label('receive');li(7,512);label('receive-pace');addi(7,7,-1);bz('receive-pace',7,true);li(3,GDMA+8);load(4,3);li(2,2);logic(1,4,4,2);bz('received',4,true);ccount(5);logic(12,5,5,8);bltu('receive',5,9);li(11,0);jump('report');label('received');li(11,1);
 label('report');uart('CAPTURE EOF=');li(3,0x60000000);addi(4,11,48);store(4,3);uart(' HEAD=');li(6,DATA);for(let i=0;i<16;i++){load8(12,6,i);hex(12,2);}
 li(6,DATA);li(7,length);li(8,1);li(9,0xedb88320);li(10,0xffffffff);label('crc-byte');load8(4,6);logic(3,10,10,4);li(5,8);label('crc-bit');logic(1,12,10,8);srli(10,10,1);bz('crc-no-xor',12);logic(3,10,10,9);label('crc-no-xor');addi(5,5,-1);bz('crc-bit',5,true);addi(6,6,1);addi(7,7,-1);bz('crc-byte',7,true);li(2,0xffffffff);logic(3,10,10,2);
 uart(' CRC=');hex(10,8);uart(' END\n');jump('capture');
 if(literals.size*4>POOL)throw Error('Literal pool overflow');
 for(const f of fixups){const pc=ENTRY+f.at*3;if(f.type==='literal'){const d=IRAM+literals.get(f.name)-((pc+3)&~3);if(d%4||d<-262144||d>-4)throw Error('L32R range');code[f.at]=1|(f.r<<4)|(((d>>2)&65535)<<8);continue;}if(!labels.has(f.name))throw Error('Missing label '+f.name);const d=labels.get(f.name)-(pc+4);if(f.type==='jump'){if(d<-131072||d>131071)throw Error('J range');code[f.at]=6|((d&0x3ffff)<<6);}else if(f.type==='unsigned'){if(d<-128||d>127)throw Error('BLTU range');code[f.at]=7|(f.t<<4)|(f.s<<8)|(3<<12)|((d&255)<<16);}else{if(d<-2048||d>2047)throw Error('BZ range');code[f.at]=0x16|(Number(f.nonzero)<<6)|(f.r<<8)|((d&4095)<<12);}}
 const data=Buffer.alloc((POOL+code.length*3+3)&~3);for(const[v,at]of literals)data.writeUInt32LE(v,at);code.forEach((w,i)=>data.writeUIntLE(w&0xffffff,POOL+i*3,3));
 const descriptor=Buffer.alloc(256);descriptor.writeUInt32LE(0xabcd5432);descriptor.write('diagnostic-1',16);descriptor.write('m5-camera-capture-probe',48);descriptor.write('Nov 26 2026',96);descriptor.write('original LX7 SRAM MMIO guest',112);createHash('sha256').update(data).digest().copy(descriptor,144);
 const end=24+8+256+8+data.length,checksumAt=Math.floor(end/16)*16+15,image=Buffer.alloc(checksumAt+33);image[0]=0xe9;image[1]=2;image[3]=p.power===undefined?0x40:0x30;image.writeUInt32LE(ENTRY,4);image.writeUInt16LE(9,12);image[23]=1;image.writeUInt32LE(0x3fc9f000,24);image.writeUInt32LE(256,28);descriptor.copy(image,32);image.writeUInt32LE(IRAM,288);image.writeUInt32LE(data.length,292);data.copy(image,296);image[checksumAt]=data.reduce((s,b)=>s^b,descriptor.reduce((s,b)=>s^b,0xef));createHash('sha256').update(image.subarray(0,checksumAt+1)).digest().copy(image,checksumAt+1);return image;
}
