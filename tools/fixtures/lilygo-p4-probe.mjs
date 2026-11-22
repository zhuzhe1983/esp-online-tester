// Original RV32I app-direct diagnostic for the T-Display-P4 V1.0 profile.
// It executes real P4 I2C0/GPIO/UART instructions. No host device-register or
// framebuffer writes; XL9535 replies must come through the physical G7/G8 route.
import {createHash} from 'node:crypto';
export function lilygoP4Probe(){
  const code=[],labels=new Map(),fixups=[];
  const emit=value=>code.push(value>>>0);
  const ins=(op,rd,funct,rs,imm)=>emit(((imm&4095)<<20)|(rs<<15)|(funct<<12)|(rd<<7)|op);
  const li=(rd,value)=>{value>>>=0;emit(((value+0x800)&0xfffff000)|(rd<<7)|0x37);ins(0x13,rd,0,rd,value&4095);};
  const sw=(rs,base)=>emit((rs<<20)|(base<<15)|(2<<12)|0x23);
  const lw=(rd,base)=>ins(3,rd,2,base,0);
  const write=(address,value)=>{li(5,address);li(6,value);sw(6,5);};
  const uart=text=>{for(const c of text)write(0x500ca000,c.charCodeAt(0));};
  const label=name=>labels.set(name,code.length*4);
  const jump=name=>{fixups.push({at:code.length,name,kind:'jump'});emit(0);};
  const branch=(name,a,b,funct=0)=>{fixups.push({at:code.length,name,kind:'branch',a,b,funct});emit(0);};
  let sequence=0;
  const transaction=(bytes,reads=0)=>{
    write(0x500c4018,3<<12);write(0x500c4024,0xffffffff);
    for(const byte of bytes)write(0x500c401c,byte);
    const commands=[6<<11,(1<<11)|(1<<8)|bytes.length,...(reads?[(3<<11)|reads]:[]),2<<11];
    commands.forEach((value,index)=>write(0x500c4058+index*4,value));write(0x500c4004,1<<5);
    const poll='i2c'+sequence++;label(poll);li(5,0x500c4020);lw(7,5);
    ins(0x13,8,7,7,1<<10);branch('failure',8,0,1);
    ins(0x13,8,7,7,1<<7);branch(poll,8,0);
  };
  for(const [pin,signal] of [[7,69],[8,68]]){
    write(0x500e1004+pin*4,(1<<12)|(1<<9));write(0x500e0558+pin*4,signal);write(0x500e0158+signal*4,0x80|pin);
  }
  transaction([0x40,2,0x40,0x21]);transaction([0x40,2]);transaction([0x41],2);
  li(5,0x500c401c);lw(7,5);li(6,0x40);branch('failure',7,6,1);
  lw(7,5);li(6,0x21);branch('failure',7,6,1);uart('XL9535 OK\nLILYGO P4 SRAM READY\n');
  li(21,2);label('poll');li(5,0x500e0040);lw(7,5);ins(0x13,7,7,7,8);ins(0x13,7,3,7,1);
  branch('poll',7,21);ins(0x13,21,0,7,0);branch('released',7,0);
  uart('BOOT DOWN\n');jump('poll');label('released');uart('BOOT UP\n');jump('poll');
  label('failure');uart('P4 I2C FAILURE\n');label('halt');jump('halt');
  for(const f of fixups){
    const n=labels.get(f.name)-f.at*4;if(!Number.isInteger(n)||n%2)throw Error('Invalid RV32 label '+f.name);
    if(f.kind==='jump'){
      if(n<-(1<<20)||n>=(1<<20))throw Error('RV32 jump range');
      code[f.at]=(((n>>20)&1)<<31)|(((n>>1)&1023)<<21)|(((n>>11)&1)<<20)|(((n>>12)&255)<<12)|0x6f;
    }else{
      if(n< -4096||n>4094)throw Error('RV32 branch range');
      code[f.at]=(((n>>12)&1)<<31)|(((n>>5)&63)<<25)|(f.b<<20)|(f.a<<15)|(f.funct<<12)|(((n>>1)&15)<<8)|(((n>>11)&1)<<7)|0x63;
    }
  }
  const data=Buffer.alloc(code.length*4);code.forEach((word,index)=>data.writeUInt32LE(word>>>0,index*4));
  const descriptor=Buffer.alloc(256);descriptor.writeUInt32LE(0xabcd5432);descriptor.write('diagnostic-1',16);
  descriptor.write('lilygo-p4-sram-probe',48);descriptor.write('00:00:00',80);descriptor.write('Nov 22 2026',96);
  descriptor.write('original RV32I SRAM probe',112);createHash('sha256').update(data).digest().copy(descriptor,144);
  const checksumAt=Math.floor((296+data.length)/16)*16+15,image=Buffer.alloc(checksumAt+33);
  image[0]=0xe9;image[1]=2;image[3]=0x40;image.writeUInt32LE(0x4ff10000,4);image.writeUInt16LE(18,12);
  image.writeUInt16LE(100,15);image.writeUInt16LE(199,17);image[23]=1;
  image.writeUInt32LE(0x4ff00000,24);image.writeUInt32LE(descriptor.length,28);descriptor.copy(image,32);
  image.writeUInt32LE(0x4ff10000,288);image.writeUInt32LE(data.length,292);data.copy(image,296);
  image[checksumAt]=data.reduce((checksum,byte)=>checksum^byte,descriptor.reduce((checksum,byte)=>checksum^byte,0xef));
  createHash('sha256').update(image.subarray(0,checksumAt+1)).digest().copy(image,checksumAt+1);return image;
}
