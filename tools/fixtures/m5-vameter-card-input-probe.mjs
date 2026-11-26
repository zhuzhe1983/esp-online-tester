// Original LX7 SRAM diagnostics, never manufacturer firmware. Source contracts:
// docs/platform/vameter-xiaozhi-card-runtime.md and its retained official sources.
// Instruction encodings follow xtensa-lx7/src/jit/wasm_tests/asm.rs and the existing
// m5-shared-host-probe assembler. All input decisions and pixel writes run in guest.
import {createHash} from 'node:crypto';
const IRAM=0x40370000,POOL=4096,ENTRY=IRAM+POOL,GPIO=0x60004000,SPI=0x60025000;

export function m5VameterCardInputProbe(kind){
 if(!['vameter','xiaozhi-card'].includes(kind))throw Error('Unknown diagnostic');
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
 const dc=kind==='vameter'?34:41;
 for(const [pin,signal]of kind==='vameter'?[[36,66],[35,68],[37,71]]:[[45,66],[46,68],[42,71]])mode(pin,signal);
 for(const pin of kind==='vameter'?[34,33,38]:[41,47]){mode(pin,256);output(pin,true);}
 write(SPI+0x20,0x3e);li(6,SPI);li(13,1<<24);
 const waitSpi=()=>{const idle='spi-'+unique++;label(idle);load(4,6);logic(1,4,4,13);bz(idle,4,true);};
 const transfer=bytes=>{if(!bytes.length)return;write(SPI+0x10,1<<27);write(SPI+0x1c,bytes.length*8-1);for(let i=0;i<bytes.length;i+=4){let v=0;for(let j=0;j<4&&i+j<bytes.length;j++)v|=bytes[i+j]<<(8*j);write(SPI+0x98+i,v);}store(13,6);waitSpi();};
 const cmd=(c,data=[])=>{output(dc,false);transfer([c]);output(dc,true);transfer(data);};
 const transferRegister=(r,bits)=>{write(SPI+0x10,1<<27);write(SPI+0x1c,bits-1);store(r,6,0x98);store(13,6);waitSpi();};
 if(kind==='vameter'){
  for(const pin of [1,2,3])write(0x60009004+pin*4,(1<<12)|(1<<9));
  cmd(0x11);cmd(0x29);cmd(0x3a,[0x55]);cmd(0x36,[0]);
  li(8,14);li(12,0);li(7,14);uart('VAMETER INPUT SRAM READY\n');jump('draw');
  label('poll');li(3,GPIO+0x3c);load(7,3);li(2,14);logic(1,7,7,2);beq('same',7,8);jump('changed');label('same');jump('poll');
  label('changed');logic(3,4,7,8);li(2,10);logic(1,4,4,2);bz('no-edge',4);addi(12,12,1);uart('ENCODER EDGE\n');label('no-edge');addi(8,7,0);
  label('draw');li(2,4);logic(1,4,7,2);bz('pressed',4);li(10,0x00f8);uart('PRESS UP\n');jump('press-color');
  label('pressed');li(10,0xe007);uart('PRESS DOWN\n');label('press-color');
  // Each actual A3/B1 transition increments the guest counter. The second
  // pixel stays yellow after the complete quadrature cycle returns to idle.
  li(11,0x1f00);bz('encoder-color',12);li(11,0xe0ff);label('encoder-color');
  cmd(0x2a,[0,0,0,1]);cmd(0x2b,[0,0,0,0]);cmd(0x2c);transferRegister(10,16);transferRegister(11,16);
  uart('FRAME COMPLETE\n');jump('poll');
 }else{
  const sda=2,scl=1;
  // BUSY48 must be an enabled physical GPIO input before guest polling. Without
  // this, an unreadable default IO_MUX input can make a probe skip the deadline.
  write(0x60009004+48*4,(1<<12)|(1<<9));
  for(const pin of [sda,scl]){mode(pin,256);output(pin,true);}
  const start=()=>{output(sda,true);output(scl,true);output(sda,false);output(scl,false);};
  const stop=()=>{output(sda,false);output(scl,true);output(sda,true);};
  const send=byte=>{for(let bit=7;bit>=0;bit--){output(sda,!!(byte&(1<<bit)));output(scl,true);output(scl,false);}output(sda,true);output(scl,true);output(scl,false);};
  const touchCount=()=>{
   start();for(const b of [0x70,2])send(b);stop();start();send(0x71);li(7,0);output(sda,true);
   for(let bit=7;bit>=0;bit--){const skip='i2c-bit-'+unique++;output(scl,true);li(3,GPIO+0x3c);load(4,3);li(2,4);logic(1,4,4,2);bz(skip,4);li(2,1<<bit);logic(2,7,7,2);label(skip);output(scl,false);}
   output(sda,true);output(scl,true);output(scl,false);stop();
  };
  cmd(0x11,[3]);cmd(0x44,[21,21]);cmd(0x45,[7,1,7,1]);
  li(8,0xffffffff);uart('XIAOZHI CARD INPUT SRAM READY\n');
  label('poll');touchCount();beq('same',7,8);jump('changed');label('same');jump('poll');label('changed');addi(8,7,0);
  bz('released',7);li(10,0xfe);uart('TOUCH DOWN\n');jump('draw');label('released');li(10,0xff);uart('TOUCH UP\n');label('draw');
  cmd(0x4e,[21]);cmd(0x4f,[7,1]);cmd(0x24);transferRegister(10,8);cmd(0x22,[0xf7]);cmd(0x20);
  label('epd-busy');li(3,GPIO+0x40);load(4,3);li(2,1<<16);logic(1,4,4,2);bz('epd-busy',4,true);
  uart('FRAME COMPLETE\n');jump('poll');
 }
 if(literals.size*4>POOL)throw Error('Literal pool overflow');
 for(const f of fixups){const pc=ENTRY+f.at*3;if(f.type==='literal'){const d=IRAM+literals.get(f.name)-((pc+3)&~3);if(d%4||d<-262144||d>-4)throw Error('L32R range');code[f.at]=1|(f.r<<4)|(((d>>2)&65535)<<8);continue;}if(!labels.has(f.name))throw Error('Missing label');const d=labels.get(f.name)-(pc+4);if(f.type==='jump'){if(d<-131072||d>131071)throw Error('J range');code[f.at]=6|((d&0x3ffff)<<6);}else if(f.type==='equal'){if(d<-128||d>127)throw Error('BEQ range');code[f.at]=7|(f.t<<4)|(f.s<<8)|(1<<12)|((d&255)<<16);}else{if(d<-2048||d>2047)throw Error('BZ range');code[f.at]=0x16|(Number(f.nonzero)<<6)|(f.r<<8)|((d&4095)<<12);}}
 const data=Buffer.alloc((POOL+code.length*3+3)&~3);for(const [v,p]of literals)data.writeUInt32LE(v,p);code.forEach((w,i)=>data.writeUIntLE(w&0xffffff,POOL+i*3,3));
 const descriptor=Buffer.alloc(256);descriptor.writeUInt32LE(0xabcd5432);descriptor.write('diagnostic-1',16);descriptor.write('m5-vameter-card-input-probe',48);descriptor.write('Nov 26 2026',96);descriptor.write('original Xtensa SRAM probe',112);createHash('sha256').update(data).digest().copy(descriptor,144);
 const end=24+8+256+8+data.length,checksumAt=Math.floor(end/16)*16+15,image=Buffer.alloc(checksumAt+33);image[0]=0xe9;image[1]=2;image[3]=0x40;image.writeUInt32LE(ENTRY,4);image.writeUInt16LE(9,12);image[23]=1;image.writeUInt32LE(0x3fc90000,24);image.writeUInt32LE(256,28);descriptor.copy(image,32);image.writeUInt32LE(IRAM,288);image.writeUInt32LE(data.length,292);data.copy(image,296);image[checksumAt]=data.reduce((s,b)=>s^b,descriptor.reduce((s,b)=>s^b,0xef));createHash('sha256').update(image.subarray(0,checksumAt+1)).digest().copy(image,checksumAt+1);return image;
}
