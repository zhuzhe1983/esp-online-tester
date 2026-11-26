// Authored LX6 SRAM diagnostics, loaded by the real ESP32 mask ROM as a
// diagnostic bootloader. Not manufacturer firmware, app-direct, or ROM stubs.
// GPIO/SPI/I2C/RMT/PSRAM constants follow esp32/tests/m5_units.rs and the pinned
// source receipts in docs/platform/evidence/m5-unit-runtime-20261126/.
import {createHash} from 'node:crypto';
const IRAM=0x40080000,POOL=4096,ENTRY=IRAM+POOL,GPIO=0x3ff44000,MUX=0x3ff49000,SPI=0x3ff64000,I2C=0x3ff53000;
const OFFSETS=[0x44,0x88,0x40,0x84,0x48,0x6c,0x60,0x64,0x68,0x54,0x58,0x5c,0x34,0x38,0x30,0x3c,0x4c,0x50,0x70,0x74,0x78,0x7c,0x80,0x8c,0x90,0x24,0x28,0x2c,0,0,0,0,0x1c,0x20,0x14,0x18,4,8,0x0c,0x10];
export function m5UnitControllerProbe(board,{wrongRoute=false}={}) {
 if(!['atomu','unit-lcd','unit-thermal2','atom-psram-display'].includes(board))throw Error('Unknown exact unit probe');
 const code=[],labels=new Map(),fixups=[],literals=new Map();let unique=0;
 const emit=w=>code.push(w&0xffffff),rri=(r,s,t,n)=>emit(2|(t<<4)|(s<<8)|(r<<12)|((n&255)<<16));
 const label=n=>{if(labels.has(n))throw Error('Duplicate label '+n);labels.set(n,ENTRY+code.length*3);};
 const ref=(type,name,args={})=>{fixups.push({at:code.length,type,name,...args});emit(0);};
 const li=(r,value)=>{value>>>=0;if(!literals.has(value))literals.set(value,literals.size*4);ref('literal',value,{r});};
 const store=(t,s,n=0)=>rri(6,s,t,n/4),load=(t,s,n=0)=>rri(2,s,t,n/4),addi=(t,s,n)=>rri(12,s,t,n);
 const and=(r,s,t)=>emit((t<<4)|(s<<8)|(r<<12)|(1<<20));
 const jump=n=>ref('jump',n),bz=(n,r,nonzero=false)=>ref('zero',n,{r,nonzero}),beq=(n,s,t)=>ref('equal',n,{s,t});
 const eqLong=(target,s,t)=>{const yes='eq-yes-'+unique++,no='eq-no-'+unique++;beq(yes,s,t);jump(no);label(yes);jump(target);label(no);};
 const write=(a,v)=>{li(3,a);li(2,v);store(2,3);};
 const read=(r,a)=>{li(3,a);load(r,3);};
 const mux=pin=>write(MUX+OFFSETS[pin],(2<<12)|(1<<9));
 const output=(pin,high)=>write(GPIO+(pin<32?(high?8:12):(high?20:24)),1<<(pin%32));
 const mode=(pin,signal)=>{mux(pin);write(GPIO+0x530+pin*4,signal);if(signal===256)write(GPIO+(pin<32?0x24:0x30),1<<(pin%32));};
 const uart=text=>{for(const c of text)write(0x3ff40000,c.charCodeAt(0));};
 const readKey=r=>{read(r,GPIO+0x40);li(2,0x80);and(r,r,2);};
 const transfer=bytes=>{for(let i=0;i<bytes.length;i+=4){let v=0;for(let j=0;j<4&&i+j<bytes.length;j++)v|=bytes[i+j]<<(j*8);write(SPI+0x80+i,v);}write(SPI+0x1c,1<<27);write(SPI+0x28,bytes.length*8-1);write(SPI,1<<18);const idle='spi-'+unique++;label(idle);read(4,SPI);li(2,1<<18);and(4,4,2);bz(idle,4,true);};
 const rmt=(red,green,blue)=>{const base=0x3ff56000;write(0x3ff000c0,1<<9);mode(wrongRoute?26:27,87);let index=0;for(const b of[green,red,blue])for(let i=7;i>=0;i--){const hi=b&(1<<i)?64:32;write(base+0x800+4*index++,hi|0x8000|((100-hi)<<16));}write(base+0x800+4*index,0);write(base+0x20,(1<<24)|1);write(base+0x24,(1<<17)|(1<<19)|(1<<3)|1);};
 const lcdCmd=(c,p=[])=>{output(5,false);output(23,false);transfer([c]);if(p.length){output(23,true);transfer(p);}output(5,true);};
 const lcdPixel=color=>{lcdCmd(0x2a,[0,52,0,52]);lcdCmd(0x2b,[0,40,0,40]);lcdCmd(0x2c,color);};
 const fpga=bytes=>{output(33,false);transfer(bytes);output(33,true);};
 const transact=(bytes,n=0)=>{write(I2C+0x24,0xffffffff);write(I2C+0x18,3<<12);for(const b of bytes)write(I2C+0x1c,b);const cmds=[0,(1<<11)|(1<<8)|bytes.length,...(n?[(2<<11)|(1<<10)|n]:[]),3<<11];for(const[i,c]of cmds.entries())write(I2C+0x58+4*i,c);write(I2C+4,(1<<4)|(1<<5));const done='i2c-'+unique++;label(done);read(4,I2C+0x20);li(2,0x480);and(4,4,2);bz(done,4);};
 write(0x3ff48070,1<<27);write(0x3ff0003c,2);
 if(board==='unit-lcd') {
  for(const[pin,sig]of[[wrongRoute?12:13,8],[15,10]])mode(pin,sig);
  for(const pin of[5,18,4,23])mode(pin,256);output(5,true);output(18,true);output(4,false);
  lcdCmd(0x11);lcdCmd(0x29);lcdCmd(0x3a,[0x55]);lcdCmd(0x36,[8]);lcdPixel([0xf8,0]);uart('UNIT LCD RED\n');
  label('uart-poll');read(7,0x3ff4001c);li(2,255);and(7,7,2);bz('uart-poll',7);read(7,0x3ff40000);li(2,103);eqLong('green',7,2);lcdPixel([0xf8,0]);uart('UNIT LCD RED\n');jump('uart-poll');label('green');lcdPixel([0x07,0xe0]);uart('UNIT LCD GREEN\n');jump('uart-poll');
 } else if(board==='unit-thermal2') {
  for(const[pin,sig]of[[wrongRoute?22:21,30],[wrongRoute?21:22,29]]){mux(pin);write(GPIO+0x130+4*sig,0x80|pin);write(GPIO+0x530+4*pin,sig);}
  mux(39);uart('THERMAL SENSOR POLL\n');li(8,0xffffffff);li(9,0xffffffff);
  label('sensor-poll');transact([0x66,4,0]);transact([0x67],2);read(10,I2C+0x1c);read(11,I2C+0x1c);beq('same-high',10,8);jump('changed');label('same-high');eqLong('sensor-poll',11,9);label('changed');addi(8,10,0);addi(9,11,0);
  // Exact expected wire-byte cases, not a host temperature or framebuffer write.
  li(2,0xe8);eqLong('raw1000',11,2);li(2,0xe1);eqLong('raw4321',11,2);li(2,0xd2);eqLong('raw1234',11,2);uart('THERMAL RAW OTHER\n');jump('sensor-poll');
  label('raw1000');li(2,3);eqLong('is1000',10,2);jump('sensor-poll');label('is1000');uart('THERMAL RAW1000\n');jump('sensor-poll');
  label('raw4321');li(2,16);eqLong('is4321',10,2);jump('sensor-poll');label('is4321');uart('THERMAL RAW4321\n');jump('sensor-poll');
  label('raw1234');li(2,4);eqLong('is1234',10,2);jump('sensor-poll');label('is1234');uart('THERMAL RAW1234\n');jump('sensor-poll');
 } else {
  mux(39);
  if(board==='atom-psram-display') {
   for(const[off,v]of[[0x34,5],[0x1c,1<<31],[0x24,0x70000035],[0,1<<18]])write(0x3ff42000+off,v);
   for(const[off,v]of[[0x34,0],[0x54,(23<<22)|(1<<28)|0x34],[0x5c,0x700000eb],[0x60,0x70000038]])write(0x3ff43000+off,v);
   for(const base of[0x3ff00040,0x3ff00058]){write(base,8);write(base+4,0);}for(const base of[0x3ff11200,0x3ff13200]){write(base,0);write(base+63*4,63);}
   write(0x3f800000,0x12345678);write(0x3f9ffffc,0xaabbccdd);read(10,0x3f800000);read(11,0x3f9ffffc);li(2,0x12345678);beq('psram-first-ok',10,2);jump('psram-fail');label('psram-first-ok');li(2,0xaabbccdd);beq('psram-both-ok',11,2);jump('psram-fail');label('psram-fail');uart('PSRAM FAIL\n');label('failed');jump('failed');label('psram-both-ok');uart('PSRAM2MB OK\n');
   for(const[pin,sig]of[[wrongRoute?23:5,8],[19,10]])mode(pin,sig);mode(33,256);output(33,true);fpga([0x18,1,1,0,4,0,3,0xde]);
  }
  li(8,0xffffffff);label('key-poll');readKey(7);beq('same',7,8);jump('change');label('same');jump('key-poll');label('change');addi(8,7,0);bz('down',7);
  if(board==='atomu')rmt(255,0,0);else fpga([0x62,0,0,0,0,0xf8,0]);uart('BUTTON UP RED\n');jump('key-poll');label('down');if(board==='atomu')rmt(0,255,0);else fpga([0x62,0,0,0,0,0x07,0xe0]);uart('BUTTON DOWN GREEN\n');jump('key-poll');
 }
 if(literals.size*4>POOL)throw Error('Literal pool overflow');
 for(const f of fixups){const pc=ENTRY+f.at*3;if(f.type==='literal'){const d=IRAM+literals.get(f.name)-((pc+3)&~3);if(d%4||d<-262144||d>-4)throw Error('L32R range');code[f.at]=1|(f.r<<4)|(((d>>2)&65535)<<8);continue;}if(!labels.has(f.name))throw Error('Missing label '+f.name);const d=labels.get(f.name)-(pc+4);if(f.type==='jump'){if(d<-131072||d>131071)throw Error('J range');code[f.at]=6|((d&0x3ffff)<<6);}else if(f.type==='equal'){if(d<-128||d>127)throw Error('BEQ range '+f.name);code[f.at]=7|(f.t<<4)|(f.s<<8)|(1<<12)|((d&255)<<16);}else{if(d<-2048||d>2047)throw Error('BZ range '+f.name);code[f.at]=0x16|(Number(f.nonzero)<<6)|(f.r<<8)|((d&4095)<<12);}}
 const data=Buffer.alloc((POOL+code.length*3+3)&~3);for(const[v,p]of literals)data.writeUInt32LE(v,p);code.forEach((w,i)=>data.writeUIntLE(w&0xffffff,POOL+i*3,3));
 const descriptor=Buffer.alloc(256);descriptor.writeUInt32LE(0xabcd5432);descriptor.write('diagnostic-1',16);descriptor.write('m5-unit-probe',48);descriptor.write('Nov 26 2026',96);descriptor.write('original LX6 SRAM diagnostic',112);createHash('sha256').update(data).digest().copy(descriptor,144);
 const end=24+8+256+8+data.length,checksumAt=Math.floor(end/16)*16+15,image=Buffer.alloc(checksumAt+33);image[0]=0xe9;image[1]=2;image[3]=board==='atom-psram-display'?0x30:0x20;image.writeUInt32LE(ENTRY,4);image.writeUInt16LE(0,12);image[23]=1;image.writeUInt32LE(0x3ffb0000,24);image.writeUInt32LE(256,28);descriptor.copy(image,32);image.writeUInt32LE(IRAM,288);image.writeUInt32LE(data.length,292);data.copy(image,296);image[checksumAt]=data.reduce((s,b)=>s^b,descriptor.reduce((s,b)=>s^b,0xef));createHash('sha256').update(image.subarray(0,checksumAt+1)).digest().copy(image,checksumAt+1);return image;
}
