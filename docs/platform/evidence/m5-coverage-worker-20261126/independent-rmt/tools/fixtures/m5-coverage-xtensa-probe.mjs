// Newly authored LX6/LX7 SRAM diagnostics. Every input read and output is guest MMIO.
// Source contracts: exact board files in esp32{,s3}/src/board and IDF GPIO/RMT/SPI.
// Classic images are ordinary mask-ROM-loaded diagnostic bootloaders; S3 uses explicit SRAM app-direct.
import {createHash} from 'node:crypto';
const OFFSETS=[0x44,0x88,0x40,0x84,0x48,0x6c,0x60,0x64,0x68,0x54,0x58,0x5c,0x34,0x38,0x30,0x3c,0x4c,0x50,0x70,0x74,0x78,0x7c,0x80,0x8c,0x90,0x24,0x28,0x2c,0,0,0,0,0x1c,0x20,0x14,0x18,4,8,0x0c,0x10];
export function m5CoverageXtensaProbe(board,{wrongRoute=false,flashMb=8}={}){
 const classic=['station-bat','station-485','stickc'].includes(board),station=board.startsWith('station-'),rgb=board.startsWith('capsule')||board==='atoms3u';
 if(!classic&&!rgb&&!['powerhub','atoms3r-ext'].includes(board))throw Error('Unknown coverage board '+board);
 const IRAM=classic?0x40080000:0x40370000,POOL=4096,ENTRY=IRAM+POOL,GPIO=classic?0x3ff44000:0x60004000,SPI=0x3ff64000;
 const code=[],labels=new Map(),fixups=[],literals=new Map();let unique=0;
 const emit=w=>code.push(w&0xffffff),rri=(r,s,t,n)=>emit(2|(t<<4)|(s<<8)|(r<<12)|((n&255)<<16));
 const label=n=>{if(labels.has(n))throw Error('Duplicate '+n);labels.set(n,ENTRY+code.length*3);},ref=(type,name,args={})=>{fixups.push({at:code.length,type,name,...args});emit(0);};
 const li=(r,v)=>{v>>>=0;if(!literals.has(v))literals.set(v,literals.size*4);ref('literal',v,{r});};
 const store=(t,s,n=0)=>rri(6,s,t,n/4),load=(t,s,n=0)=>rri(2,s,t,n/4),addi=(t,s,n)=>rri(12,s,t,n),and=(r,s,t)=>emit((t<<4)|(s<<8)|(r<<12)|(1<<20));
 const jump=n=>ref('jump',n),bz=(n,r,nonzero=false)=>ref('zero',n,{r,nonzero}),beq=(n,s,t)=>ref('equal',n,{s,t});
 const write=(a,v)=>{li(3,a);li(2,v);store(2,3);},read=(r,a)=>{li(3,a);load(r,3);};
 const mux=pin=>write(classic?0x3ff49000+OFFSETS[pin]:0x60009004+pin*4,((classic?2:1)<<12)|(1<<9));
 const mode=(pin,signal)=>{mux(pin);write(GPIO+(classic?0x530:0x554)+pin*4,signal);if(signal===256)write(GPIO+(pin<32?0x24:0x30),1<<(pin%32));};
 const output=(pin,high)=>write(GPIO+(pin<32?(high?8:12):(high?20:24)),1<<(pin%32));
 li(14,classic?0x3ff40000:0x60000000);const uart=s=>{for(const c of s){li(2,c.charCodeAt(0));store(2,14);}};
 const delay=()=>{li(7,10000);const loop='delay-'+unique++;label(loop);addi(7,7,-1);bz(loop,7,true);};
 const rmt=green=>{const pin=board==='atoms3u'?35:21;mode(wrongRoute?pin-1:pin,81);let i=0;for(const b of green?[255,0,0]:[0,255,0])for(let bit=7;bit>=0;bit--)write(0x60016800+4*i++,0x8000|((b&(1<<bit))?64|(32<<16):32|(64<<16)));write(0x60016800+4*i,0);write(0x60016020,(1<<16)|(1<<8)|1);delay();read(4,0x60016070);li(2,1);and(4,4,2);const done='rmt-ok-'+unique++,end='rmt-end-'+unique++;bz(done,4,true);uart('RMT IRQ MISSING\n');jump(end);label(done);uart('RMT IRQ DONE\n');label(end);};
 const transfer=(data,reg=null)=>{if(reg===null){for(let i=0;i<data.length;i+=4){let v=0;for(let j=0;j<4&&i+j<data.length;j++)v|=data[i+j]<<(8*j);write(SPI+0x80+i,v);}}else{li(3,SPI+0x80);store(reg,3);}write(SPI+0x1c,1<<27);write(SPI+0x28,data.length*8-1);write(SPI,1<<18);const loop='spi-'+unique++;label(loop);read(4,SPI);li(2,1<<18);and(4,4,2);bz(loop,4,true);};
 const cs=station?5:5,dc=station?19:23,rst=station?15:18;
 const cmd=(c,data=[])=>{output(cs,false);output(dc,false);transfer([c]);if(data.length){output(dc,true);transfer(data);}output(cs,true);};
 if(classic){
  write(0x3ff48070,1<<27);write(0x3ff0003c,2);
  // AXP192 register0x12 LDO2+LDO3 through actual SDA21/SCL22 hardware I2C.
  for(const[pin,sig]of[[21,30],[22,29]]){mux(pin);write(GPIO+0x130+4*sig,0x80|pin);write(GPIO+0x530+4*pin,sig);}
  const I2C=0x3ff53000;write(I2C+0x24,0xffffffff);write(I2C+0x18,3<<12);for(const b of[0x68,0x12,0x4d])write(I2C+0x1c,b);[0,(1<<11)|(1<<8)|3,3<<11].forEach((v,i)=>write(I2C+0x58+4*i,v));write(I2C+4,(1<<4)|(1<<5));delay();
  for(const[pin,sig]of[[station?18:13,8],[wrongRoute?22:station?23:15,10]])mode(pin,sig);
  for(const pin of[cs,dc,rst]){mode(pin,256);output(pin,true);}output(rst,false);output(rst,true);
  cmd(0x11);cmd(0x29);cmd(0x3a,[0x55]);cmd(0x36,[station?0x68:0xc8]);
 }else if(board==='capsule-v11'){mode(38,256);output(38,true);}
 const realPin=classic?37:board==='powerhub'?11:board.startsWith('capsule')?42:41,key=wrongRoute&&!classic&&!rgb?realPin-1:realPin;
 mux(key);if(wrongRoute&&!classic&&!rgb){mode(key,256);output(key,true);}
 li(8,0xffffffff);uart('COVERAGE '+board+' READY\n');label('poll');read(9,GPIO+(key<32?0x3c:0x40));li(2,1<<(key%32));and(9,9,2);beq('poll',9,8);addi(8,9,0);bz('down',9);
 uart('KEY UP\n');li(10,0x00f8);if(rgb)rmt(false);jump('draw');label('down');uart('KEY DOWN\n');li(10,0xe007);if(rgb)rmt(true);label('draw');
 if(classic){const x=station?40:26,y=station?53:1;cmd(0x2a,[0,x,0,x]);cmd(0x2b,[0,y,0,y]);cmd(0x2c);output(cs,false);output(dc,true);transfer([0,0],10);output(cs,true);uart('PIXEL COMPLETE\n');}
 uart('OUTPUT COMPLETE\n');jump('poll');
 if(literals.size*4>POOL)throw Error('Literal pool overflow');
 for(const f of fixups){const pc=ENTRY+f.at*3;if(f.type==='literal'){const d=IRAM+literals.get(f.name)-((pc+3)&~3);if(d%4||d<-262144||d>-4)throw Error('L32R range');code[f.at]=1|(f.r<<4)|(((d>>2)&65535)<<8);continue;}const d=labels.get(f.name)-(pc+4);if(f.type==='jump'){if(d<-131072||d>131071)throw Error('J range');code[f.at]=6|((d&0x3ffff)<<6);}else if(f.type==='equal'){if(d<-128||d>127)throw Error('BEQ range');code[f.at]=7|(f.t<<4)|(f.s<<8)|(1<<12)|((d&255)<<16);}else{if(d<-2048||d>2047)throw Error('BZ range');code[f.at]=0x16|(Number(f.nonzero)<<6)|(f.r<<8)|((d&4095)<<12);}}
 const data=Buffer.alloc((POOL+code.length*3+3)&~3);for(const[v,p]of literals)data.writeUInt32LE(v,p);code.forEach((w,i)=>data.writeUIntLE(w,POOL+i*3,3));
 const descriptor=Buffer.alloc(256);descriptor.writeUInt32LE(0xabcd5432);descriptor.write('diagnostic-1',16);descriptor.write('m5-coverage-probe',48);descriptor.write('Nov 26 2026',96);createHash('sha256').update(data).digest().copy(descriptor,144);
 const end=296+data.length,check=Math.floor(end/16)*16+15,image=Buffer.alloc(check+33);if(classic&&image.length>0x7000)throw Error('Bootloader overlaps partition table');image[0]=0xe9;image[1]=2;image[3]=({4:2,8:3,16:4}[flashMb])<<4;image.writeUInt32LE(ENTRY,4);image.writeUInt16LE(classic?0:9,12);image[23]=1;image.writeUInt32LE(classic?0x3ffb0000:0x3fc90000,24);image.writeUInt32LE(256,28);descriptor.copy(image,32);image.writeUInt32LE(IRAM,288);image.writeUInt32LE(data.length,292);data.copy(image,296);image[check]=data.reduce((a,b)=>a^b,descriptor.reduce((a,b)=>a^b,0xef));createHash('sha256').update(image.subarray(0,check+1)).digest().copy(image,check+1);return image;
}
