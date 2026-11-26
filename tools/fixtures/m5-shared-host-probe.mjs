// Original SRAM diagnostics, not manufacturer firmware. Actual LX7 instructions
// drive GPIO/I2C/SPI or LCD_CAM/GDMA; host input must change guest-rendered pixels.
// Encodings: xtensa-lx7/src/jit/wasm_tests/asm.rs. Register/pin sources and limits:
// docs/platform/evidence/m5-shared-host-runtime-20261126/sources.json.
import {createHash} from 'node:crypto';
const IRAM=0x40370000,POOL=4096,ENTRY=IRAM+POOL,GPIO=0x60004000,SPI=0x60024000;

export function m5SharedHostProbe(kind,{power=true,correctRoute=true}={}) {
 if(!['paperdiy','cores3'].includes(kind))throw Error('Unknown diagnostic');
 const code=[],labels=new Map(),fixups=[],literals=new Map();let unique=0;
 const emit=w=>code.push(w&0xffffff),rri=(r,s,t,n)=>emit(2|(t<<4)|(s<<8)|(r<<12)|((n&255)<<16));
 const label=n=>{if(labels.has(n))throw Error('Duplicate label');labels.set(n,ENTRY+code.length*3);};
 const ref=(type,name,args={})=>{fixups.push({at:code.length,type,name,...args});emit(0);};
 const li=(r,value)=>{value>>>=0;if(!literals.has(value))literals.set(value,literals.size*4);ref('literal',value,{r});};
 const store=(t,s,n=0)=>rri(6,s,t,n/4),load=(t,s,n=0)=>rri(2,s,t,n/4),addi=(t,s,n)=>rri(12,s,t,n);
 const logic=(op,r,s,t)=>emit((t<<4)|(s<<8)|(r<<12)|(op<<20));
 const jump=n=>ref('jump',n),bz=(n,r,nonzero=false)=>ref('zero',n,{r,nonzero}),beq=(n,s,t)=>ref('equal',n,{s,t});
 const write=(a,v)=>{li(3,a);li(2,v);store(2,3);};
 const output=(pin,high)=>write(GPIO+(pin<32?(high?8:12):(high?20:24)),1<<(pin%32));
 const mode=(pin,signal)=>{write(0x60009004+pin*4,(1<<12)|(1<<9));write(GPIO+0x554+pin*4,signal);if(signal===256)write(GPIO+(pin<32?0x24:0x30),1<<(pin%32));};
 const uart=text=>{for(const c of text)write(0x60000000,c.charCodeAt(0));};
 const sda=kind==='paperdiy'?41:12,scl=kind==='paperdiy'?42:11;
 const start=()=>{output(sda,true);output(scl,true);output(sda,false);output(scl,false);};
 const stop=()=>{output(sda,false);output(scl,true);output(sda,true);};
 const send=byte=>{for(let bit=7;bit>=0;bit--){output(sda,!!(byte&(1<<bit)));output(scl,true);output(scl,false);}output(sda,true);output(scl,true);output(scl,false);};
 const i2cWrite=(addr,reg,values)=>{start();for(const b of [addr<<1,reg,...values])send(b);stop();};
 const i2cRead=(addr,reg)=>{
  i2cWrite(addr,reg,[]);start();send((addr<<1)|1);li(7,0);output(sda,true);
  for(let bit=7;bit>=0;bit--){const skip='i2c-bit-'+unique++;output(scl,true);li(3,GPIO+(sda<32?0x3c:0x40));load(4,3);li(2,1<<(sda%32));logic(1,4,4,2);bz(skip,4);li(2,1<<bit);logic(2,7,7,2);label(skip);output(scl,false);}
  output(sda,true);output(scl,true);output(scl,false);stop();
 };
 for(const pin of [sda,scl]){mode(pin,256);output(pin,true);}
 if(kind==='paperdiy'){
  // M5PM1 GPIO2 push-pull EPD_PWR; analog boost and grayscale remain untested.
  if(power){i2cWrite(0x6e,0x10,[4,4]);i2cWrite(0x6e,0x13,[0x1b]);}
  for(const [i,pin]of [6,14,7,12,9,11,8,10].entries())mode(pin,133+i);
  mode(16,correctRoute?154:155);mode(13,132);
  for(const pin of [15,17,18,46]){mode(pin,256);output(pin,false);}output(46,true);
  for(const pin of [3,4])write(0x60009004+pin*4,(1<<12)|(1<<9));
  write(0x600c001c,1<<8);write(0x60041000,3<<29);write(0x6004101c,0);
  li(8,0xffffffff);uart('PAPERDIY HOST SRAM READY\n');label('poll');
  li(3,GPIO+0x3c);load(7,3);li(2,24);logic(1,7,7,2);beq('poll',7,8);addi(8,7,0);
  li(2,24);logic(3,4,7,2);bz('released',4);
  uart('BUTTON DOWN\n');li(10,0xaaaaaaaa);jump('draw');
  label('released');uart('BUTTON UP\n');li(10,0x55555555);label('draw');
  li(6,0x3fc8a000);for(let i=0;i<248;i+=4)store(10,6,i);
  output(17,true);output(17,false);output(18,true);li(9,540);label('row');output(15,false);
  const desc=0x3fc88000;
  write(desc,(1<<31)|(1<<30)|(248<<12)|248);write(desc+4,0x3fc8a000);write(desc+8,0);
  write(0x6003f0a8,5);write(0x6003f080,(desc&0xfffff)|(1<<21));
  write(0x60041014,(1<<27)|(1<<24)|(1<<13));output(15,true);addi(9,9,-1);bz('row',9,true);
  uart('FRAME COMPLETE\n');jump('poll');
 }else{
  if(power){i2cWrite(0x58,0x12,[255,255]);i2cWrite(0x58,2,[1,2,0x10,4]);i2cWrite(0x34,0x99,[28]);i2cWrite(0x34,0x90,[0x88]);}
  for(const [pin,signal]of [[36,correctRoute?101:102],[37,103],[3,110]])mode(pin,signal);
  mode(35,256);output(35,true);write(SPI+0x20,0x3e);li(6,SPI);li(13,1<<24);
  const transfer=bytes=>{if(!bytes.length)return;write(SPI+0x10,1<<27);write(SPI+0x1c,bytes.length*8-1);for(let i=0;i<bytes.length;i+=4){let value=0;for(let j=0;j<4&&i+j<bytes.length;j++)value|=bytes[i+j]<<(j*8);write(SPI+0x98+i,value);}store(13,6);const idle='spi-'+unique++;label(idle);load(4,6);logic(1,4,4,13);bz(idle,4,true);};
  const cmd=(c,data=[])=>{output(35,false);transfer([c]);output(35,true);transfer(data);};
  cmd(0x11);cmd(0x29);cmd(0x3a,[0x55]);
  uart('CORES3 HOST SRAM READY\n');li(8,0xffffffff);label('poll');i2cRead(0x38,2);beq('same',7,8);jump('changed');label('same');jump('poll');label('changed');addi(8,7,0);bz('released',7);
  uart('TOUCH DOWN\n');li(10,0xe007e007);jump('draw');label('released');uart('TOUCH UP\n');li(10,0x00f800f8);label('draw');
  cmd(0x2a,[0,0,0,15]);cmd(0x2b,[0,0,0,15]);cmd(0x2c);write(SPI+0x10,1<<27);write(SPI+0x1c,31);li(9,128);
  label('pixel');store(10,6,0x98);store(13,6);label('pixel-idle');load(4,6);logic(1,4,4,13);bz('pixel-idle',4,true);addi(9,9,-1);bz('pixel',9,true);uart('FRAME COMPLETE\n');jump('poll');
 }
 if(literals.size*4>POOL)throw Error('Literal pool overflow');
 for(const f of fixups){const pc=ENTRY+f.at*3;if(f.type==='literal'){const d=IRAM+literals.get(f.name)-((pc+3)&~3);if(d%4||d<-262144||d>-4)throw Error('L32R range');code[f.at]=1|(f.r<<4)|(((d>>2)&65535)<<8);continue;}if(!labels.has(f.name))throw Error('Missing label');const d=labels.get(f.name)-(pc+4);if(f.type==='jump'){if(d<-131072||d>131071)throw Error('J range');code[f.at]=6|((d&0x3ffff)<<6);}else if(f.type==='equal'){if(d<-128||d>127)throw Error('BEQ range');code[f.at]=7|(f.t<<4)|(f.s<<8)|(1<<12)|((d&255)<<16);}else{if(d<-2048||d>2047)throw Error('BZ range');code[f.at]=0x16|(Number(f.nonzero)<<6)|(f.r<<8)|((d&4095)<<12);}}
 const data=Buffer.alloc((POOL+code.length*3+3)&~3);for(const [v,p]of literals)data.writeUInt32LE(v,p);code.forEach((w,i)=>data.writeUIntLE(w&0xffffff,POOL+i*3,3));
 const descriptor=Buffer.alloc(256);descriptor.writeUInt32LE(0xabcd5432);descriptor.write('diagnostic-1',16);descriptor.write('m5-shared-host-probe',48);descriptor.write('Nov 26 2026',96);descriptor.write('original Xtensa SRAM probe',112);createHash('sha256').update(data).digest().copy(descriptor,144);
 const end=24+8+256+8+data.length,checksumAt=Math.floor(end/16)*16+15,image=Buffer.alloc(checksumAt+33);image[0]=0xe9;image[1]=2;image[3]=0x40;image.writeUInt32LE(ENTRY,4);image.writeUInt16LE(9,12);image[23]=1;image.writeUInt32LE(0x3fc90000,24);image.writeUInt32LE(256,28);descriptor.copy(image,32);image.writeUInt32LE(IRAM,288);image.writeUInt32LE(data.length,292);data.copy(image,296);image[checksumAt]=data.reduce((s,b)=>s^b,descriptor.reduce((s,b)=>s^b,0xef));createHash('sha256').update(image.subarray(0,checksumAt+1)).digest().copy(image,checksumAt+1);return image;
}
