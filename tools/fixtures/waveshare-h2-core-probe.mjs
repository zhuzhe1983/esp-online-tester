// Newly authored RV32I SRAM diagnostic, not a manufacturer image.
// H2 has independent PCR/GPIO addresses, RMT at 0x60007000 and 32 MHz XTAL.
import {createHash} from 'node:crypto';
export function waveshareH2CoreProbe() {
  const code=[],labels=new Map(),fixups=[];
  const emit=value=>code.push(value>>>0);
  const insn=(op,rd,f,rs,imm)=>emit(((imm&4095)<<20)|(rs<<15)|(f<<12)|(rd<<7)|op);
  const li=(rd,n)=>{n>>>=0;emit(((n+0x800)&0xfffff000)|(rd<<7)|0x37);insn(0x13,rd,0,rd,n&4095);};
  const write=(address,value)=>{li(5,address);li(6,value);emit((6<<20)|(5<<15)|(2<<12)|0x23);};
  const label=name=>labels.set(name,code.length*4);
  const branch=(name,a,b,fn=0)=>{fixups.push({at:code.length,name,a,b,fn});emit(0);};
  const uart=text=>{for(const char of text)write(0x60000000,char.charCodeAt(0));};
  write(0x60096034,1);write(0x60096038,1<<21);write(0x60007068,0x8500001b);
  write(0x60090004+8*4,1<<12);write(0x60091554+8*4,71);
  let at=0;
  for(const value of [0x12,0x34,0x56])for(let bit=7;bit>=0;bit--)
    write(0x60007400+at++*4,value&(1<<bit)?(0x8000|26|(14<<16)):(0x8000|13|(27<<16)));
  write(0x60007460,0);write(0x60007010,(1<<16)|(1<<8)|1);
  li(5,0x60007038);label('tx');insn(3,6,2,5,0);insn(0x13,6,7,6,1);branch('tx',6,0);
  write(0x60090004+9*4,(1<<12)|(1<<9));uart('CORE H2 SRAM READY\n');li(21,2);
  label('poll');li(5,0x6009103c);insn(3,7,2,5,0);li(6,1<<9);
  emit((6<<20)|(7<<15)|(7<<12)|(7<<7)|0x33);insn(0x13,7,3,7,1);
  branch('poll',7,21);insn(0x13,21,0,7,0);branch('released',7,0);
  uart('BOOT DOWN\n');branch('poll',0,0);
  label('released');uart('BOOT UP\n');branch('poll',0,0);
  for(const f of fixups){
    const n=labels.get(f.name)-f.at*4;
    if(n%2||n< -4096||n>4094)throw Error('branch range');
    code[f.at]=(((n>>12)&1)<<31)|(((n>>5)&63)<<25)|(f.b<<20)|(f.a<<15)|(f.fn<<12)|(((n>>1)&15)<<8)|(((n>>11)&1)<<7)|0x63;
  }
  const data=Buffer.alloc(code.length*4);code.forEach((word,i)=>data.writeUInt32LE(word>>>0,i*4));
  const end=32+data.length,checksumAt=Math.floor(end/16)*16+15,image=Buffer.alloc(checksumAt+33);
  image[0]=0xe9;image[1]=1;image[3]=0x20;image.writeUInt32LE(0x40800000,4);
  image.writeUInt16LE(16,12);image.writeUInt16LE(0,15);image.writeUInt16LE(65535,17);image[23]=1;
  image.writeUInt32LE(0x40800000,24);image.writeUInt32LE(data.length,28);data.copy(image,32);
  image[checksumAt]=data.reduce((sum,b)=>sum^b,0xef);
  createHash('sha256').update(image.subarray(0,checksumAt+1)).digest().copy(image,checksumAt+1);
  return image;
}
