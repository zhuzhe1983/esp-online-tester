// Authored LX7 SRAM image for exact K152 public-runtime execution evidence.
// It reads real TCA8418 FIFO bytes through I2C0 and prints them on guest UART0;
// GPIO-matrix/SPI3 writes place the corresponding RGB565 pixel on ST7789 glass.
// Register/pad/scan sources: docs/platform/evidence/m5-basic-mesh-runtime-20261126/README.md.
import {createHash} from 'node:crypto';
const IRAM=0x40370000,POOL=4096,ENTRY=IRAM+POOL,GPIO=0x60004000,SPI=0x60025000,I2C=0x60013000;
export function m5CardputerMeshInputProbe({correctDisplayRoute=true,correctKeyboardRoute=true}={}){
 const code=[],labels=new Map(),fixups=[],literals=new Map();let unique=0;
 const emit=w=>code.push(w&0xffffff),rri=(r,s,t,n)=>emit(2|(t<<4)|(s<<8)|(r<<12)|((n&255)<<16));
 const label=n=>{if(labels.has(n))throw Error('Duplicate '+n);labels.set(n,ENTRY+code.length*3);};
 const ref=(type,name,args={})=>{fixups.push({at:code.length,type,name,...args});emit(0);};
 const li=(r,v)=>{v>>>=0;if(!literals.has(v))literals.set(v,literals.size*4);ref('literal',v,{r});};
 const store=(t,s,n=0)=>rri(6,s,t,n/4),load=(t,s,n=0)=>rri(2,s,t,n/4),addi=(t,s,n)=>rri(12,s,t,n);
 const logic=(op,r,s,t)=>emit((t<<4)|(s<<8)|(r<<12)|(op<<20));
 const srli=(r,t,n)=>emit((4<<20)|(1<<16)|(r<<12)|(n<<8)|(t<<4));
 const jump=n=>ref('jump',n),bz=(n,r,nonzero=false)=>ref('zero',n,{r,nonzero}),bltu=(n,s,t)=>ref('unsigned',n,{s,t});
 const write=(a,v)=>{li(3,a);li(2,v);store(2,3);};
 const output=(pin,high)=>write(GPIO+(pin<32?(high?8:12):(high?20:24)),1<<(pin%32));
 const mode=(pin,signal)=>{write(0x60009004+pin*4,(1<<12)|(1<<9));write(GPIO+0x554+pin*4,signal);if(signal===256)write(GPIO+(pin<32?0x24:0x30),1<<(pin%32));};
 const uart=text=>{for(const c of text)write(0x60000000,c.charCodeAt(0));};
 const hex=r=>{for(const n of[1,0]){const digit='digit-'+unique++,out='digit-out-'+unique++;if(n)srli(4,r,4);else addi(4,r,0);li(2,15);logic(1,4,4,2);li(5,10);bltu(digit,4,5);addi(4,4,87);jump(out);label(digit);addi(4,4,48);label(out);li(3,0x60000000);store(4,3);}};
 const transfer=bytes=>{if(!bytes.length)return;write(SPI+0x10,1<<27);write(SPI+0x1c,bytes.length*8-1);for(let i=0;i<bytes.length;i+=4){let v=0;for(let j=0;j<4&&i+j<bytes.length;j++)v|=bytes[i+j]<<(8*j);write(SPI+0x98+i,v);}store(13,6);const idle='spi-'+unique++;label(idle);load(4,6);logic(1,4,4,13);bz(idle,4,true);};
 const cmd=(c,data=[])=>{output(34,false);transfer([c]);output(34,true);transfer(data);};
 const draw=()=>{cmd(0x2a,[0,186,0,186]);cmd(0x2b,[0,40,0,40]);cmd(0x2c);write(SPI+0x10,1<<27);write(SPI+0x1c,15);store(10,6,0x98);store(13,6);const idle='pixel-'+unique++;label(idle);load(4,6);logic(1,4,4,13);bz(idle,4,true);uart('FRAME COMPLETE\n');};
 const i2cTransaction=(bytes,commands)=>{write(I2C+0x24,0xffffffff);write(I2C+0x18,3<<12);for(const b of bytes)write(I2C+0x1c,b);for(const[i,c]of commands.entries())write(I2C+0x58+4*i,c);write(I2C+4,(1<<4)|(1<<5));const wait='i2c-'+unique++;label(wait);li(3,I2C+0x20);load(4,3);li(2,(1<<7)|(1<<10));logic(1,4,4,2);bz(wait,4);};
 const regWrite=(r,v)=>i2cTransaction([0x68,r,v],[6<<11,(1<<11)|(1<<8)|3,2<<11]);
 const regRead=r=>{i2cTransaction([0x68,r,0x69],[6<<11,(1<<11)|(1<<8)|2,6<<11,(1<<11)|(1<<8)|1,(3<<11)|(1<<10)|1,2<<11]);li(3,I2C+0x1c);load(7,3);li(2,255);logic(1,7,7,2);};
 for(const[pin,signal]of[[correctDisplayRoute?36:40,66],[35,68],[37,71]])mode(pin,signal);
 for(const pin of[33,34,38]){mode(pin,256);output(pin,true);}output(33,false);output(33,true);write(SPI+0x20,0x3e);li(6,SPI);li(13,1<<24);
 for(const[pin,signal]of[[8,90],[9,89]]){mode(pin,signal);write(GPIO+0x154+signal*4,128|pin);}write(0x60009004+11*4,(1<<12)|(1<<9));
 for(const[r,v]of[[0x1d,0x7f],[0x1e,0xff],[1,1]])regWrite(r,v);
 if(!correctKeyboardRoute){mode(10,90);write(GPIO+0x154+90*4,128|10);uart('KEYBOARD SDA10 WRONG ROUTE\n');}
 cmd(0x11);cmd(0x29);cmd(0x3a,[0x55]);cmd(0x36,[0]);uart('K152 AUTHORED SRAM READY\nKEY NONE\n');li(10,0x00f8);draw();
 label('poll');regRead(3);
 if(!correctKeyboardRoute){li(2,1<<10);logic(1,4,4,2);bz('poll',4);uart('KEYBOARD I2C NACK\n');label('nack-poll');regRead(3);jump('nack-poll');}
 li(2,15);logic(1,7,7,2);bz('poll',7);regRead(4);addi(12,7,0);regWrite(2,1);
 uart('KEY EVENT ');hex(12);uart('\n');li(2,128);logic(1,7,12,2);bz('released',7);uart('KEY DOWN\n');li(10,0xe007);jump('draw');label('released');uart('KEY UP\n');li(10,0x00f8);label('draw');draw();jump('poll');
 if(literals.size*4>POOL)throw Error('Literal pool overflow');
 for(const f of fixups){const pc=ENTRY+f.at*3;if(f.type==='literal'){const d=IRAM+literals.get(f.name)-((pc+3)&~3);if(d%4||d<-262144||d>-4)throw Error('L32R range');code[f.at]=1|(f.r<<4)|(((d>>2)&65535)<<8);continue;}if(!labels.has(f.name))throw Error('Missing '+f.name);const d=labels.get(f.name)-(pc+4);if(f.type==='jump'){if(d<-131072||d>131071)throw Error('J range');code[f.at]=6|((d&0x3ffff)<<6);}else if(f.type==='unsigned'){if(d<-128||d>127)throw Error('BLTU range');code[f.at]=7|(f.t<<4)|(f.s<<8)|(3<<12)|((d&255)<<16);}else{if(d<-2048||d>2047)throw Error('BZ range');code[f.at]=0x16|(Number(f.nonzero)<<6)|(f.r<<8)|((d&4095)<<12);}}
 const data=Buffer.alloc((POOL+code.length*3+3)&~3);for(const[v,p]of literals)data.writeUInt32LE(v,p);code.forEach((w,i)=>data.writeUIntLE(w&0xffffff,POOL+i*3,3));
 const descriptor=Buffer.alloc(256);descriptor.writeUInt32LE(0xabcd5432);descriptor.write('diagnostic-1',16);descriptor.write('k152-authored-input-probe',48);descriptor.write('Nov 26 2026',96);descriptor.write('original LX7 SRAM diagnostic',112);createHash('sha256').update(data).digest().copy(descriptor,144);
 const end=24+8+256+8+data.length,checksumAt=Math.floor(end/16)*16+15,image=Buffer.alloc(checksumAt+33);image[0]=0xe9;image[1]=2;image[3]=0x30;image.writeUInt32LE(ENTRY,4);image.writeUInt16LE(9,12);image[23]=1;image.writeUInt32LE(0x3fc90000,24);image.writeUInt32LE(256,28);descriptor.copy(image,32);image.writeUInt32LE(IRAM,288);image.writeUInt32LE(data.length,292);data.copy(image,296);image[checksumAt]=data.reduce((s,b)=>s^b,descriptor.reduce((s,b)=>s^b,0xef));createHash('sha256').update(image.subarray(0,checksumAt+1)).digest().copy(image,checksumAt+1);return image;
}
