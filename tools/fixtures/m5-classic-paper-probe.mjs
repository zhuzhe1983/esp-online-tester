// Original LX6 SRAM diagnostic, not an M5Stack factory image. Every panel byte,
// BUSY read and key poll executes Xtensa MMIO instructions in production WASM.
// Encodings: xtensa-lx7/src/jit/wasm_tests/asm.rs. Classic ESP32 registers:
// ESP-IDF5.5.4 gpio_reg.h/io_mux_reg.h/spi_reg.h/clk_tree_ll.h; exact wiring and
// panel protocols: docs/platform/evidence/m5-classic-paper-runtime/receipt.json.
import {createHash} from 'node:crypto';
const IRAM=0x40080000,POOL=4096,ENTRY=IRAM+POOL,GPIO=0x3ff44000,MUX=0x3ff49000,SPI=0x3ff64000;
const OFFSETS=[0x44,0x88,0x40,0x84,0x48,0x6c,0x60,0x64,0x68,0x54,0x58,0x5c,0x34,0x38,0x30,0x3c,0x4c,0x50,0x70,0x74,0x78,0x7c,0x80,0x8c,0x90,0x24,0x28,0x2c,0,0,0,0,0x1c,0x20,0x14,0x18,4,8,0x0c,0x10];
export function m5ClassicPaperProbe(board,{touch=false}={}){
 if(['m5paper-original','m5paper-comm'].includes(board))board='m5paper-v11';
 if(!['m5paper-v11','coreink-d67'].includes(board))throw Error('Unknown classic e-paper diagnostic');
 const paper=board==='m5paper-v11',key=paper?38:5,busy=paper?27:4;
 const code=[],labels=new Map(),fixups=[],literals=new Map();let unique=0;
 const emit=w=>code.push(w&0xffffff),rri=(r,s,t,n)=>emit(2|(t<<4)|(s<<8)|(r<<12)|((n&255)<<16));
 const label=n=>{if(labels.has(n))throw Error('Duplicate label');labels.set(n,ENTRY+code.length*3);};
 const ref=(type,name,args={})=>{fixups.push({at:code.length,type,name,...args});emit(0);};
 const li=(r,value)=>{value>>>=0;if(!literals.has(value))literals.set(value,literals.size*4);ref('literal',value,{r});};
 const store=(t,s,n=0)=>rri(6,s,t,n/4),load=(t,s,n=0)=>rri(2,s,t,n/4),addi=(t,s,n)=>rri(12,s,t,n);
 const and=(r,s,t)=>emit((t<<4)|(s<<8)|(r<<12)|(1<<20));
 const jump=n=>ref('jump',n),bz=(n,r,nonzero=false)=>ref('zero',n,{r,nonzero}),beq=(n,s,t)=>ref('equal',n,{s,t});
 const write=(a,v)=>{li(3,a);li(2,v);store(2,3);};
 const mux=pin=>write(MUX+OFFSETS[pin],(2<<12)|(1<<9));
 const output=(pin,high)=>write(GPIO+(pin<32?(high?8:12):(high?20:24)),1<<(pin%32));
 const mode=(pin,signal)=>{mux(pin);write(GPIO+0x530+pin*4,signal);if(signal===256)write(GPIO+(pin<32?0x24:0x30),1<<(pin%32));};
 const uart=text=>{for(const c of text)write(0x3ff40000,c.charCodeAt(0));};
 const readPin=(r,pin)=>{li(3,GPIO+(pin<32?0x3c:0x40));load(r,3);li(2,1<<(pin%32));and(r,r,2);};
 const waitBusy=()=>{const idle='busy-'+unique++;label(idle);readPin(4,busy);bz(idle,4,!paper);};
 const transfer=(bytes,register=null)=>{
  if(register===null){for(let i=0;i<bytes.length;i+=4){let value=0;for(let j=0;j<4&&i+j<bytes.length;j++)value|=bytes[i+j]<<(j*8);write(SPI+0x80+i,value);}}
  else {li(3,SPI+0x80);store(register,3);}
  write(SPI+0x1c,(1<<27)|1);write(SPI+0x28,bytes.length*8-1);write(SPI,1<<18);
  const idle='spi-'+unique++;label(idle);li(3,SPI);load(4,3);li(2,1<<18);and(4,4,2);bz(idle,4,true);
 };
 const paperFrame=words=>{output(15,false);transfer(words.flatMap(w=>[w>>8,w&255]));output(15,true);};
 const paperCmd=(cmd,args=[])=>{waitBusy();paperFrame([0x6000,cmd]);for(const a of args)paperFrame([0,a]);};
 const inkCmd=(cmd,data=[])=>{output(9,false);output(15,false);transfer([cmd]);if(data.length){output(15,true);transfer(data);}output(9,true);};
 // PLL240MHz gives a1:1 reference-cycle basis; no host-time or calendar stubs.
 write(0x3ff48070,1<<27);write(0x3ff0003c,2);mux(key);mux(busy);
 if(paper){
  for(const[pin,sig]of[[14,8],[12,10]])mode(pin,sig);
  for(const pin of[15,4,23,2]){mode(pin,256);output(pin,true);}
  paperCmd(1);paperCmd(0x11,[4,1]);
 }else{
  for(const[pin,sig]of[[18,8],[23,10]])mode(pin,sig);
  for(const pin of[9,15,0,12]){mode(pin,256);output(pin,true);}
  inkCmd(0x12);waitBusy();inkCmd(0x01,[199,0,0]);inkCmd(0x11,[3]);inkCmd(0x21,[0]);
  inkCmd(0x22,[0xf8]);inkCmd(0x20);waitBusy();
 }
 const I2C=0x3ff53000;
 const transaction=(addr,data=[],n=0)=>{write(I2C+0x24,0xffffffff);write(I2C+0x18,3<<12);write(I2C+0x1c,(addr<<1)|Number(n>0));for(const v of data)write(I2C+0x1c,v);const commands=[0,(1<<11)|(1<<8)|(1+data.length),...(n?[(2<<11)|(1<<10)|n]:[]),3<<11];commands.forEach((v,i)=>write(I2C+0x58+i*4,v));write(I2C+4,1<<5);};
 if(touch){if(!paper)throw Error('Touch is Paper only');for(const[pin,sig]of[[21,30],[22,29]]){mux(pin);write(GPIO+0x130+sig*4,128|pin);write(GPIO+0x530+pin*4,sig);}}
 uart('CLASSIC PAPER SRAM READY\n');li(8,0xffffffff);label('poll');
 // Consume only ready reports. ACK leaves STATUS=0; that is not a new UP event.
 if(touch){transaction(0x14,[0x81,0x4e]);transaction(0x14,[],1);li(3,I2C+0x1c);load(7,3);li(2,128);and(4,7,2);bz('poll',4);transaction(0x14,[0x81,0x4e,0]);li(2,1);and(7,7,2);bz('no-touch',7);li(7,0);jump('touch-sampled');label('no-touch');li(7,1);label('touch-sampled');}
 else{readPin(7,key);}
 beq('same',7,8);jump('changed');label('same');jump('poll');label('changed');addi(8,7,0);bz('pressed',7);
 uart(touch?'TOUCH UP\n':'BUTTON UP\n');li(10,paper?0xffff0000:255);jump('draw');label('pressed');uart(touch?'TOUCH DOWN\n':'BUTTON DOWN\n');li(10,0);label('draw');
 if(paper){
  // Native x956..959,y0 maps to portrait x0,y3..0. White released, black pressed.
  paperCmd(0x21,[0x120,956,0,4,1]);output(15,false);transfer([0,0,0,0],10);output(15,true);paperCmd(0x22);
  paperCmd(0x37,[956,0,4,1,2,0x36e0,0x12]);if(touch){label('refresh-assert');readPin(4,busy);bz('refresh-assert',4,true);}waitBusy();
 }else{
  // A one-byte window touches only native row0,x0..7. No host pixel injection.
  inkCmd(0x44,[0,0]);inkCmd(0x45,[0,0,0,0]);inkCmd(0x4e,[0]);inkCmd(0x4f,[0,0]);
  output(9,false);output(15,false);transfer([0x24]);output(15,true);transfer([0],10);output(9,true);
  inkCmd(0x22,[0x14]);inkCmd(0x20);waitBusy();
 }
 uart('FRAME COMPLETE\n');jump('poll');
 if(literals.size*4>POOL)throw Error('Literal pool overflow');
 for(const f of fixups){const pc=ENTRY+f.at*3;if(f.type==='literal'){const d=IRAM+literals.get(f.name)-((pc+3)&~3);if(d%4||d<-262144||d>-4)throw Error('L32R range');code[f.at]=1|(f.r<<4)|(((d>>2)&65535)<<8);continue;}if(!labels.has(f.name))throw Error('Missing label');const d=labels.get(f.name)-(pc+4);if(f.type==='jump'){if(d<-131072||d>131071)throw Error('J range');code[f.at]=6|((d&0x3ffff)<<6);}else if(f.type==='equal'){if(d<-128||d>127)throw Error('BEQ range');code[f.at]=7|(f.t<<4)|(f.s<<8)|(1<<12)|((d&255)<<16);}else{if(d<-2048||d>2047)throw Error('BZ range');code[f.at]=0x16|(Number(f.nonzero)<<6)|(f.r<<8)|((d&4095)<<12);}}
 const data=Buffer.alloc((POOL+code.length*3+3)&~3);for(const[v,p]of literals)data.writeUInt32LE(v,p);code.forEach((w,i)=>data.writeUIntLE(w&0xffffff,POOL+i*3,3));
 const descriptor=Buffer.alloc(256);descriptor.writeUInt32LE(0xabcd5432);descriptor.write('diagnostic-1',16);descriptor.write('m5-classic-paper-probe',48);descriptor.write('Nov 26 2026',96);descriptor.write('original LX6 SRAM diagnostic',112);createHash('sha256').update(data).digest().copy(descriptor,144);
 const end=24+8+256+8+data.length,checksumAt=Math.floor(end/16)*16+15,image=Buffer.alloc(checksumAt+33);image[0]=0xe9;image[1]=2;image[3]=paper?0x40:0x20;image.writeUInt32LE(ENTRY,4);image.writeUInt16LE(0,12);image[23]=1;image.writeUInt32LE(0x3ffb0000,24);image.writeUInt32LE(256,28);descriptor.copy(image,32);image.writeUInt32LE(IRAM,288);image.writeUInt32LE(data.length,292);data.copy(image,296);image[checksumAt]=data.reduce((s,b)=>s^b,descriptor.reduce((s,b)=>s^b,0xef));createHash('sha256').update(image.subarray(0,checksumAt+1)).digest().copy(image,checksumAt+1);return image;
}
