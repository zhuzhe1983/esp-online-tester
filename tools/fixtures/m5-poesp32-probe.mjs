// Original LX6 SRAM diagnostic bootloader, loaded by the real classic ESP32 ROM.
// MMIO/pad/descriptor values follow esp32/tests/unit_poesp32.rs and IDF v5.4.2.
// This is authored digital protocol coverage, not ESP-AT or physical PoE validation.
import {createHash} from 'node:crypto';
const IRAM=0x40080000,POOL=4096,ENTRY=IRAM+POOL,GPIO=0x3ff44000,MUX=0x3ff49000,MAC=0x3ff69000,DMA=0x3ff6a000,EXT=0x3ff6b000;
const TD=0x3ffb1000,RD=0x3ffb1020,TX=0x3ffb2000,RX=0x3ffb3000,HEX=0x3ffb4000,OWN=0x80000000;
const OFFSETS=[0x44,0x88,0x40,0x84,0x48,0x6c,0x60,0x64,0x68,0x54,0x58,0x5c,0x34,0x38,0x30,0x3c,0x4c,0x50,0x70,0x74,0x78,0x7c,0x80,0x8c,0x90,0x24,0x28,0x2c,0,0,0,0,0x1c,0x20,0x14,0x18,4,8,0x0c,0x10];
export const poesp32ArpRequest=()=>Buffer.from([255,255,255,255,255,255,2,0,0,0,0,3,8,6,0,1,8,0,6,4,0,1,2,0,0,0,0,3,10,0,2,15,0,0,0,0,0,0,10,0,2,2]);
export const poesp32ArpReply=()=>Buffer.from([2,0,0,0,0,3,2,0x53,0x49,0x4d,0,2,8,6,0,1,8,0,6,4,0,2,2,0x53,0x49,0x4d,0,2,10,0,2,2,2,0,0,0,0,3,10,0,2,15]);
export function ethernetCrc32(bytes){let c=0xffffffff;for(const b of bytes){c^=b;for(let bit=0;bit<8;bit++)c=(c>>>1)^((c&1)?0xedb88320:0);}return(~c)>>>0;}
export function m5PoEsp32Probe(){
 const code=[],labels=new Map(),fixups=[],literals=new Map();let unique=0;
 const emit=w=>code.push(w&0xffffff),rri=(r,s,t,n)=>emit(2|(t<<4)|(s<<8)|(r<<12)|((n&255)<<16));
 const label=n=>{if(labels.has(n))throw Error('Duplicate '+n);labels.set(n,ENTRY+code.length*3);};
 const ref=(type,name,args={})=>{fixups.push({at:code.length,type,name,...args});emit(0);};
 const li=(r,v)=>{v>>>=0;if(!literals.has(v))literals.set(v,literals.size*4);ref('literal',v,{r});};
 const load=(t,s,n=0)=>rri(2,s,t,n/4),store=(t,s,n=0)=>rri(6,s,t,n/4),addi=(t,s,n)=>rri(12,s,t,n),byte=(t,s,n=0)=>rri(0,s,t,n);
 const logic=(op,r,s,t)=>emit((t<<4)|(s<<8)|(r<<12)|(op<<20)),and=(r,s,t)=>logic(1,r,s,t),xor=(r,s,t)=>logic(3,r,s,t);
 const srli=(r,t,n)=>{if(n>15){srli(r,t,15);srli(r,r,n-15);}else emit((t<<4)|(n<<8)|(r<<12)|(1<<16)|(4<<20));};
 const add=(r,s,t)=>emit((t<<4)|(s<<8)|(r<<12)|(8<<20));
 const sub=(r,s,t)=>emit((t<<4)|(s<<8)|(r<<12)|(12<<20)),ccount=r=>emit((3<<16)|(234<<8)|(r<<4));
 const jump=n=>ref('jump',n),bz=(n,r,nonzero=false)=>ref('zero',n,{r,nonzero}),beq=(n,s,t)=>ref('equal',n,{s,t});
 const requireEqual=(r,v)=>{li(2,v);const ok='eq-'+unique++;beq(ok,r,2);jump('failed');label(ok);};
 const write=(a,v)=>{li(3,a);li(2,v);store(2,3);},read=(r,a)=>{li(3,a);load(r,3);};
 const mux=(pin,fn=2)=>write(MUX+OFFSETS[pin],(fn<<12)|(1<<9));
 const route=(pin,signal)=>{mux(pin);write(GPIO+0x530+pin*4,signal);if(signal===256)write(GPIO+0x24,1<<pin);};
 const reset=high=>write(GPIO+(high?8:12),1<<5);
 const uart=text=>{for(const c of text){li(2,c.charCodeAt(0));store(2,14);}};
 const hexReg=(r,nibbles)=>{for(let i=nibbles-1;i>=0;i--){if(i)srli(5,r,i*4);else addi(5,r,0);li(2,15);and(5,5,2);add(6,13,5);byte(5,6);store(5,14);}};
 const delay=()=>{li(7,20000);const loop='delay-'+unique++;label(loop);addi(7,7,-1);bz(loop,7,true);};
 const waitOwn=()=>{const poll='dma-'+unique++;label(poll);read(4,RD);li(2,OWN);and(4,4,2);bz(poll,4,true);};
 const mdio=(address,reg,expected,mask=0xffff)=>{
  ccount(15);write(MAC+0x10,1|(address<<11)|(reg<<6));read(4,MAC+0x10);li(2,1);and(4,4,2);requireEqual(4,1);
  const poll='mdio-'+unique++;label(poll);read(4,MAC+0x10);li(2,1);and(4,4,2);bz(poll,4,true);
  ccount(7);sub(7,7,15);read(4,MAC+0x14);li(2,mask);and(4,4,2);requireEqual(4,expected);
 };
 const words=(a,ws)=>ws.forEach((v,i)=>write(a+4*i,v));
 const bytes=(a,bs)=>{for(let i=0;i<bs.length;i+=4){let v=0;for(let j=0;j<4&&i+j<bs.length;j++)v|=bs[i+j]<<(8*j);write(a+i,v);}};
 const arm=()=>{
  write(DMA+0x18,0);bytes(TX,poesp32ArpRequest());for(let i=0;i<48;i+=4)write(RX+i,0xa5a5a5a5);
  words(TD,[OWN|(1<<28)|(1<<29)|(1<<30)|(1<<20),42,TX,TD,0,0,0,0]);words(RD,[OWN,(1<<14)|1536,RX,RD,0,0,0,0]);
  write(MAC+0x40,0x300);write(MAC+0x44,2);write(DMA,1<<7);write(DMA+0x10,TD);write(DMA+0x0c,RD);write(DMA+0x1c,(1<<16)|1|(1<<6));write(DMA+0x14,0xffffffff);write(MAC,12);write(DMA+0x18,(1<<13)|2);
 };
 const stalled=()=>{
  delay();read(4,TD);li(2,OWN);and(4,4,2);requireEqual(4,OWN);read(4,RD);requireEqual(4,OWN);
  for(let i=0;i<48;i+=4){read(4,RX+i);requireEqual(4,0xa5a5a5a5);}
 };
 const received=name=>{
  waitOwn();read(4,TD);li(2,OWN);and(4,4,2);requireEqual(4,0);read(4,RD);requireEqual(4,(46<<16)|(1<<9)|(1<<8));
  read(4,DMA+0x14);li(2,65);and(4,4,2);requireEqual(4,65);
  const reply=poesp32ArpReply(),crc=ethernetCrc32(reply),fcs=Buffer.alloc(4);fcs.writeUInt32LE(crc);const full=Buffer.concat([reply,fcs]);
  uart(name+' RXHEX ');for(let i=0;i<full.length;i++){li(3,RX+i);byte(4,3);hexReg(4,2);requireEqual(4,full[i]);}uart('\n');
  // Guest computes the reflected Ethernet CRC from actual RX RAM with LX6 shifts/XORs.
  li(8,0xffffffff);li(9,RX);li(10,42);li(11,0xedb88320);li(12,1);
  const next='crc-byte-'+unique++;label(next);byte(4,9);xor(8,8,4);
  for(let bit=0;bit<8;bit++){and(4,8,12);srli(8,8,1);const skip='crc-bit-'+unique++;bz(skip,4);xor(8,8,11);label(skip);}
  addi(9,9,1);addi(10,10,-1);bz(next,10,true);li(2,0xffffffff);xor(8,8,2);
  uart(name+' CRC32 ');hexReg(8,8);uart('\n');requireEqual(8,crc);uart(name+' OK\n');
 };
 write(0x3ff48070,1<<27);write(0x3ff0003c,2);li(14,0x3ff40000);li(13,HEX);bytes(HEX,Buffer.from('0123456789abcdef'));uart('U138 LX6 START\n');
 write(0x3ff000cc,1<<14);write(0x3ff000d0,0);route(5,256);reset(true);route(23,200);route(18,201);write(GPIO+0x130+4*201,0x80|18);
 for(const pin of[21,19,22,25,26,27,0])mux(pin,5);write(EXT+8,0x21);write(EXT+12,4<<13);
 mdio(1,2,0x0243);uart('MDIO CYCLES ');hexReg(7,8);uart('\n');mdio(1,3,0x0c54);mdio(1,1,4,4);mdio(2,2,0xffff);uart('PHY 0243/0c54 ADDR1 MDIO BUSY OK\n');arm();received('ARP');
 route(23,256);mdio(1,2,0xffff);route(23,200);mdio(1,2,0x0243);uart('WRONG MDC23 NO PHY OK\n');
 write(GPIO+0x130+4*201,0x80|19);mdio(1,2,0xffff);write(GPIO+0x130+4*201,0x80|18);mdio(1,2,0x0243);uart('WRONG MDIO18 NO PHY OK\n');
 mux(22,2);mdio(1,1,0,4);arm();stalled();uart('WRONG RMII22 NO DMA OK\n');mux(22,5);received('RMII RECOVER');
 mux(0,2);mdio(1,1,0,4);arm();stalled();uart('WRONG REFCLK0 NO DMA OK\n');mux(0,5);received('REFCLK RECOVER');
 write(0x3ff000cc,0);write(MAC+0x10,1|(1<<11)|(2<<6));arm();stalled();read(4,MAC+0x10);li(2,1);and(4,4,2);requireEqual(4,1);uart('CLOCK GATED BUSY NO DMA OK\n');write(0x3ff000cc,1<<14);
 const poll='clock-mdio';label(poll);read(4,MAC+0x10);li(2,1);and(4,4,2);bz(poll,4,true);read(4,MAC+0x14);requireEqual(4,0x0243);received('CLOCK RECOVER');
 reset(false);mdio(1,2,0xffff);arm();stalled();uart('PHY RESET5 NO PHY NO DMA OK\n');reset(true);mdio(1,2,0x0243);received('RESET RECOVER');
 write(DMA+0x14,0xffffffff);read(4,DMA+0x14);requireEqual(4,0);ccount(15);uart('GUEST CCOUNT ');hexReg(15,8);uart('\n');uart('U138 ALL CHECKS OK\n');label('done');jump('done');label('failed');uart('U138 FAIL\n');label('failed-loop');jump('failed-loop');
 if(literals.size*4>POOL)throw Error('Literal pool overflow');
 for(const f of fixups){const pc=ENTRY+f.at*3;if(f.type==='literal'){const d=IRAM+literals.get(f.name)-((pc+3)&~3);if(d%4||d<-262144||d>-4)throw Error('L32R range');code[f.at]=1|(f.r<<4)|(((d>>2)&65535)<<8);continue;}if(!labels.has(f.name))throw Error('Missing '+f.name);const d=labels.get(f.name)-(pc+4);if(f.type==='jump'){if(d<-131072||d>131071)throw Error('J range');code[f.at]=6|((d&0x3ffff)<<6);}else if(f.type==='equal'){if(d<-128||d>127)throw Error('BEQ range');code[f.at]=7|(f.t<<4)|(f.s<<8)|(1<<12)|((d&255)<<16);}else{if(d<-2048||d>2047)throw Error('BZ range');code[f.at]=0x16|(Number(f.nonzero)<<6)|(f.r<<8)|((d&4095)<<12);}}
 const data=Buffer.alloc((POOL+code.length*3+3)&~3);for(const[v,p]of literals)data.writeUInt32LE(v,p);code.forEach((w,i)=>data.writeUIntLE(w,POOL+i*3,3));
 const descriptor=Buffer.alloc(256);descriptor.writeUInt32LE(0xabcd5432);descriptor.write('diagnostic-1',16);descriptor.write('m5-poesp32-probe',48);descriptor.write('Nov 26 2026',96);descriptor.write('original LX6 SRAM diagnostic',112);createHash('sha256').update(data).digest().copy(descriptor,144);
 const end=296+data.length,checksumAt=Math.floor(end/16)*16+15,image=Buffer.alloc(checksumAt+33);if(image.length>0x7000)throw Error('Diagnostic bootloader overlaps partition table');image[0]=0xe9;image[1]=2;image[3]=0x20;image.writeUInt32LE(ENTRY,4);image[23]=1;image.writeUInt32LE(0x3ffb0000,24);image.writeUInt32LE(256,28);descriptor.copy(image,32);image.writeUInt32LE(IRAM,288);image.writeUInt32LE(data.length,292);data.copy(image,296);image[checksumAt]=data.reduce((s,b)=>s^b,descriptor.reduce((s,b)=>s^b,0xef));createHash('sha256').update(image.subarray(0,checksumAt+1)).digest().copy(image,checksumAt+1);return image;
}
