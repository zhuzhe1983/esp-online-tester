// Original LX6 SRAM program, not a manufacturer image or an SDK substitute.
// Each SCCB/LEDC/GPIO/I2S/DMA/W5500 operation executes in the production CPU.
// Encodings: xtensa-lx7/src/jit/wasm_tests/asm.rs and decode.rs; register sources:
// docs/platform/research/m5-classic-camera-runtime.json (ESP-IDF4.4.7 / W5500 1.1.0).
import {createHash} from 'node:crypto';
const IRAM=0x40080000,POOL=4096,ENTRY=IRAM+POOL,GPIO=0x3ff44000,MUX=0x3ff49000,I2C=0x3ff53000,I=0x3ff4f000,D=0x3ffb1000,B=0x3ffb2000,SPI=0x3ff64000;
const OFFSETS=[0x44,0x88,0x40,0x84,0x48,0x6c,0x60,0x64,0x68,0x54,0x58,0x5c,0x34,0x38,0x30,0x3c,0x4c,0x50,0x70,0x74,0x78,0x7c,0x80,0x8c,0x90,0x24,0x28,0x2c,0,0,0,0,0x1c,0x20,0x14,0x18,4,8,0x0c,0x10];
export function m5ClassicCameraProbe(profile){
 const {board,sensor,sccbSdaScl:[sda,scl],dvpD0D7PclkHrefVsync:pins,xclk,reset}=profile,ov2640=sensor==='OV2640',poe=board.startsWith('m5stack-u121'),address=ov2640?0x30:0x3c;
 if(!board.startsWith('m5stack-')||![sensor].every(s=>['OV2640','OV3660'].includes(s))||pins.length!==11)throw Error('Exact camera profile required');
 const code=[],labels=new Map(),fixups=[],literals=new Map(),failures=[];let unique=0;
 const emit=w=>code.push(w&0xffffff),rri=(r,s,t,n)=>emit(2|(t<<4)|(s<<8)|(r<<12)|((n&255)<<16));
 const label=n=>{if(labels.has(n))throw Error('Duplicate label '+n);labels.set(n,ENTRY+code.length*3);};
 const ref=(type,name,args={})=>{fixups.push({at:code.length,type,name,...args});emit(0);};
 const li=(r,value)=>{value>>>=0;if(!literals.has(value))literals.set(value,literals.size*4);ref('literal',value,{r});};
 const store=(t,s,n=0)=>rri(6,s,t,n/4),load=(t,s,n=0)=>rri(2,s,t,n/4),logic=(op,r,s,t)=>emit((t<<4)|(s<<8)|(r<<12)|(op<<20));
 const jump=n=>ref('jump',n),bz=(n,r,nonzero=false)=>ref('zero',n,{r,nonzero}),bcc=(n,s,t,condition=1)=>ref('condition',n,{s,t,condition});
 const write=(a,v)=>{li(3,a);li(2,v);store(2,3);},read=(r,a)=>{li(3,a);load(r,3);};
 const uart=text=>{for(const c of text)write(0x3ff40000,c.charCodeAt(0));};
 const check=(r,value,tag)=>{const okay='eq-'+unique++;li(5,value);bcc(okay,r,5);li(9,failures.length);jump('failure');label(okay);failures.push(tag);};
 const masked=(r,mask,value,tag)=>{li(5,mask);logic(1,r,r,5);check(r,value,tag);};
 const mux=pin=>write(MUX+OFFSETS[pin],(2<<12)|(1<<9)|(1<<8));
 const output=(pin,high)=>write(GPIO+(pin<32?(high?8:12):(high?20:24)),1<<(pin%32));
 const mode=(pin,signal)=>{mux(pin);write(GPIO+0x530+pin*4,signal);if(signal===256)write(GPIO+(pin<32?0x24:0x30),1<<(pin%32));};
 const delay=cycles=>{const loop='delay-'+unique++;emit((3<<16)|(234<<8)|(10<<4));li(12,cycles);label(loop);emit((3<<16)|(234<<8)|(11<<4));logic(12,11,11,10);bcc(loop,11,12,3);};
 const routeSccb=(data,clock)=>{for(const[pin,signal]of[[data,30],[clock,29]]){mux(pin);write(GPIO+0x130+4*signal,0x80|pin);write(GPIO+0x530+4*pin,signal);}};
 const transaction=(bytes,readCount=0,nack=false)=>{
  write(I2C+0x24,0xffffffff);write(I2C+0x18,3<<12);write(I2C+0x1c,(address<<1)|Number(readCount>0));for(const b of bytes)write(I2C+0x1c,b);
  const commands=[0,(1<<11)|(1<<8)|(bytes.length+1),...(readCount?[(2<<11)|(1<<10)|readCount]:[]),3<<11];commands.forEach((c,i)=>write(I2C+0x58+i*4,c));write(I2C+4,1<<5);read(4,I2C+0x20);masked(4,1<<10,nack?1<<10:0,'SCCB NACK expectation');
 };
 const sensorWrite=(reg,value)=>transaction(ov2640?[reg,value]:[reg>>8,reg&255,value]);
 const sensorConfig=()=>{if(ov2640){sensorWrite(0xff,0);for(const[r,v]of[[0x5a,1],[0x5b,1],[0x5c,0],[0xda,8],[0xe0,0]])sensorWrite(r,v);}else{for(const[r,v]of[[0x3808,0],[0x3809,4],[0x380a,0],[0x380b,4],[0x4300,0x61],[0x3821,0],[0x3008,2]])sensorWrite(r,v);}};
 const arm=()=>{write(D,0x80000040);write(D+4,B);write(D+8,0);for(let n=0;n<16;n++)write(B+n*4,0xa5a5a5a5);write(I+0x18,0xffffffff);write(I+0x24,16);write(I+0x60,1<<12);write(I+0x14,(1<<9)|(1<<8)|(1<<13));write(I+0x34,(D&0xfffff)|(1<<29));};
 const waitEof=()=>{const wait='eof-'+unique++;label(wait);read(4,I+0xc);li(5,1<<9);logic(1,4,4,5);bz(wait,4);read(4,D);masked(4,1<<31,0,'DMA ownership');};
 const pixels=(word,tag)=>{for(let n=0;n<16;n++){read(4,B+n*4);check(4,word,tag+' pixel '+n);}};
 const spi=bytes=>{for(let n=0;n<bytes.length;n+=4){let word=0;for(let j=0;j<4&&n+j<bytes.length;j++)word|=bytes[n+j]<<(j*8);write(SPI+0x80+n,word);}write(SPI+0x1c,(1<<27)|(1<<28)|1);write(SPI+0x28,bytes.length*8-1);write(SPI+0x2c,bytes.length*8-1);write(SPI,1<<18);};
 const spiByte=(at,value,tag)=>{read(4,SPI+0x80+(at&~3));masked(4,(255<<((at%4)*8))>>>0,(value<<((at%4)*8))>>>0,tag);};
 const ethWrite=(block,reg,data)=>{output(4,false);spi([reg>>8,reg&255,(block<<3)|4,...data]);output(4,true);};
 const ethRead=(block,reg,len)=>{output(4,false);spi([reg>>8,reg&255,block<<3,...Array(len).fill(0)]);output(4,true);};
 write(0x3ff48070,1<<27);write(0x3ff0003c,2);uart('CAMERA SRAM '+board+'\n');
 if(poe){
  for(const[p,s]of[[23,8],[13,10]])mode(p,s);mux(38);write(GPIO+0x130+4*9,0x80|38);mode(4,256);output(4,true);mux(2);write(SPI+0x34,7);write(SPI+8,0);
  ethRead(0,0x39,1);spiByte(3,4,'W5500 full-duplex VERSIONR');uart('W5500 FIFO PASS\n');
  mode(4,11);write(SPI+0x34,6);write(SPI+0x1c,(1<<31)|(1<<30)|(1<<28));write(SPI+0x24,(15<<28)|0x3900);write(SPI+0x20,7<<26);write(SPI+4,0);write(SPI+0x30,0);write(SPI+0x2c,7);write(SPI,1<<18);spiByte(0,4,'W5500 command/address phases');uart('W5500 PHASE PASS\n');
  mode(4,256);output(4,true);write(SPI+0x34,7);
  const mac=[2,0x11,0x22,0x33,0x44,0x55];ethWrite(0,9,mac);ethWrite(1,0,[4]);ethWrite(1,1,[1]);ethRead(1,3,1);spiByte(3,0x42,'MACRAW OPEN');ethWrite(0,0x18,[1]);
  const arp=[...Array(6).fill(255),...mac,8,6,0,1,8,0,6,4,0,1,...mac,10,0,2,15,...Array(6).fill(0),10,0,2,2];ethWrite(2,0,arp);ethWrite(1,0x24,[0,arp.length]);ethWrite(1,1,[0x20]);
  ethRead(1,0x26,2);spiByte(3,0,'ARP RX size high');spiByte(4,44,'ARP RX size low');read(4,GPIO+0x3c);masked(4,4,0,'W5500 G2 IRQ');
  ethRead(3,0,44);const reply=[0,44,...mac,2,0x53,0x49,0x4d,0,2,8,6,0,1,8,0,6,4,0,2,2,0x53,0x49,0x4d,0,2,10,0,2,2,...mac,10,0,2,15];reply.forEach((b,n)=>spiByte(n+3,b,'ARP packet byte '+n));
  ethWrite(1,0x28,[0,44]);ethWrite(1,1,[0x40]);ethWrite(1,2,[0x14]);ethRead(1,0x26,2);spiByte(3,0,'RECV size high');spiByte(4,0,'RECV size low');read(4,GPIO+0x3c);masked(4,4,4,'W5500 IRQ clear');ethWrite(1,1,[0x10]);uart('WIRED ARP PASS\n');
 }
 // Wrong physical SCCB pins must not reach the sensor. Correct pins then read PID.
 routeSccb(scl,sda);transaction([],0,true);uart('WRONG SCCB PASS\n');routeSccb(sda,scl);mode(reset,256);output(reset,true);
 if(ov2640){sensorWrite(0xff,1);transaction([0x0a]);transaction([],1);read(4,I2C+0x1c);masked(4,255,0x26,'OV2640 PID');}else{transaction([0x30,0x0a]);transaction([],2);read(4,I2C+0x1c);masked(4,255,0x36,'OV3660 PID high');read(4,I2C+0x1c);masked(4,255,0x60,'OV3660 PID low');}uart('SENSOR PID PASS\n');sensorConfig();
 write(0x3ff000c0,(1<<11)|(1<<4));write(0x3ff59140,3|(256<<5)|(1<<25));write(0x3ff59008,4<<4);write(0x3ff5900c,0xc0100400);write(0x3ff59000,4);mode(xclk,71);
 pins.forEach((pin,n)=>{mux(pin);write(GPIO+0x130+4*(n<8?140+n:[28,190,191][n-8]),0x80|pin);});write(GPIO+0x130+4*192,0xb8);write(I+0xa8,0x21);write(I+0x20,(1<<12)|(1<<16)|(1<<20));write(I+8,(1<<7)|(1<<5));
 write(GPIO+0x130+4*140,0x80|(pins[0]===32?17:32));arm();delay(48_000_000);read(4,I+0xc);masked(4,1<<9,0,'wrong D0 cannot EOF');pixels(0xa5a5a5a5,'wrong route untouched DMA');uart('WRONG DVP PASS\n');write(GPIO+0x130+4*140,0x80|pins[0]);
 arm();waitEof();pixels(0x00f80000,'red RGB565 DMA');uart('RED DMA PASS\n');
 output(reset,false);transaction([],0,true);arm();delay(48_000_000);read(4,I+0xc);masked(4,1<<9,0,'RESET cannot EOF');pixels(0xa5a5a5a5,'RESET untouched DMA');uart('RESET PASS\n');output(reset,true);sensorConfig();
 // Re-arm finite native descriptors until the explicit host image replacement
 // becomes a new physical frame; an already latched frame may finish first.
 label('green-frame');arm();waitEof();read(4,B);li(5,0x000700e0);bcc('green-seen',4,5);jump('green-frame');label('green-seen');pixels(0x000700e0,'green RGB565 DMA');uart('GREEN DMA PASS\nCAMERA PROBE PASS\n');label('park');jump('park');
 label('failure');uart('FAIL CAMERA ASSERT #');for(const shift of[4,0]){emit((3<<20)|(4<<16)|(4<<12)|(shift<<8)|(9<<4));li(5,10);const digit='digit-'+unique++,done='digit-done-'+unique++;bcc(digit,4,5,3);rri(12,4,4,55);jump(done);label(digit);rri(12,4,4,48);label(done);li(3,0x3ff40000);store(4,3);}uart('\n');jump('park');
 if(literals.size*4>POOL)throw Error('Literal pool overflow');
 for(const f of fixups){const pc=ENTRY+f.at*3;if(f.type==='literal'){const d=IRAM+literals.get(f.name)-((pc+3)&~3);if(d%4||d<-262144||d>-4)throw Error('L32R range');code[f.at]=1|(f.r<<4)|(((d>>2)&65535)<<8);continue;}if(!labels.has(f.name))throw Error('Missing label '+f.name);const d=labels.get(f.name)-(pc+4);if(f.type==='jump'){if(d<-131072||d>131071)throw Error('J range');code[f.at]=6|((d&0x3ffff)<<6);}else if(f.type==='condition'){if(d<-128||d>127)throw Error('BCC range '+f.name);code[f.at]=7|(f.t<<4)|(f.s<<8)|(f.condition<<12)|((d&255)<<16);}else{if(d<-2048||d>2047)throw Error('BZ range');code[f.at]=0x16|(Number(f.nonzero)<<6)|(f.r<<8)|((d&4095)<<12);}}
 const data=Buffer.alloc((POOL+code.length*3+3)&~3);for(const[v,p]of literals)data.writeUInt32LE(v,p);code.forEach((w,n)=>data.writeUIntLE(w&0xffffff,POOL+n*3,3));
 const descriptor=Buffer.alloc(256);descriptor.writeUInt32LE(0xabcd5432);descriptor.write('diagnostic-1',16);descriptor.write('m5-classic-camera-probe',48);descriptor.write('Nov 26 2026',96);descriptor.write('original LX6 SRAM diagnostic',112);createHash('sha256').update(data).digest().copy(descriptor,144);
 const end=24+8+256+8+data.length,checksumAt=Math.floor(end/16)*16+15,image=Buffer.alloc(checksumAt+33);image[0]=0xe9;image[1]=2;image[3]=profile.flashMiB===16?0x40:0x20;image.writeUInt32LE(ENTRY,4);image.writeUInt16LE(0,12);image[23]=1;image.writeUInt32LE(0x3ffb0000,24);image.writeUInt32LE(256,28);descriptor.copy(image,32);image.writeUInt32LE(IRAM,288);image.writeUInt32LE(data.length,292);data.copy(image,296);image[checksumAt]=data.reduce((s,b)=>s^b,descriptor.reduce((s,b)=>s^b,0xef));createHash('sha256').update(image.subarray(0,checksumAt+1)).digest().copy(image,checksumAt+1);if(image.length>0x7000)throw Error('Diagnostic bootloader overlaps partition table');Object.defineProperty(image,'assertions',{value:[...failures]});return image;
}
