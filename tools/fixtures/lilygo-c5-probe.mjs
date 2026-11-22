// Original RV32I SRAM diagnostic, assembled explicitly so CI needs no cross
// compiler. It runs actual GPIO instructions and feeds the ST7735 over its
// physical software-SPI pins; no host framebuffer writes or ROM patches.
// C5 register source: ESP-IDF v5.5.4 gpio_reg.h/io_mux_reg.h/uart_reg.h.
// LilyGO pins and active-low backlight: pinned vendor sources in the C5 report.
import {createHash} from 'node:crypto';
export function lilygoC5Probe() {
  const code=[],labels=new Map(),fixups=[];
  const emit=x=>code.push(x>>>0);
  const i=(op,rd,f,rs,imm)=>emit(((imm&4095)<<20)|(rs<<15)|(f<<12)|(rd<<7)|op);
  const li=(rd,n)=>{const value=n>>>0;emit(((value+0x800)&0xfffff000)|(rd<<7)|0x37);i(0x13,rd,0,rd,value&4095);};
  const addi=(rd,rs,n)=>i(0x13,rd,0,rs,n);
  const sw=(value,base,offset=0)=>emit(((offset>>5)<<25)|(value<<20)|(base<<15)|(2<<12)|((offset&31)<<7)|0x23);
  const lw=(rd,base)=>i(0x03,rd,2,base,0);
  const and=(rd,a,b)=>emit((b<<20)|(a<<15)|(7<<12)|(rd<<7)|0x33);
  const label=name=>labels.set(name,code.length*4);
  const jump=(name,rd=0)=>{fixups.push({at:code.length,name,kind:'jal',rd});emit(0);};
  const branch=(name,a,b,fn=0)=>{fixups.push({at:code.length,name,kind:'branch',a,b,fn});emit(0);};
  const write=(addr,value)=>{li(5,addr);li(6,value);sw(6,5);};
  const uart=text=>{for(const char of text)write(0x60000000,char.charCodeAt(0));};
  const byte=value=>{li(10,value);jump('spi',1);};
  const command=(value,data=[])=>{li(6,8);sw(6,19);byte(value);li(6,8);sw(6,18);for(const b of data)byte(b);};
  // s2/s3 keep GPIO OUT_W1TS/OUT_W1TC; no C3/C6 register aliases.
  li(18,0x60091008);li(19,0x6009100c);
  for(const pin of [0,1,2,3,6,10]){write(0x60090004+pin*4,1<<12);write(0x60091ad4+pin*4,256);}
  write(0x60091004,1<<1);write(0x60091038,(1<<0)|(1<<1)|(1<<2)|(1<<3)|(1<<6)|(1<<10));
  write(0x60090074,(1<<12)|(1<<9));
  command(0x11);command(0x29);command(0x3a,[0x05]);command(0x36,[0x08]);
  uart('LILYGO C5 SRAM READY\n');
  li(21,2); // impossible initial pressed state forces the first framebuffer.
  label('poll');li(5,0x60091064);lw(7,5);li(6,1<<28);and(7,7,6);i(0x13,7,3,7,1); // pressed = (G28 == 0)
  branch('poll',7,21);addi(21,7,0);
  branch('released',7,0);
  uart('BOOT DOWN\n');li(22,0x07);li(23,0xe0);jump('draw');
  label('released');uart('BOOT UP\n');li(22,0xf8);li(23,0x00);
  label('draw');command(0x2a,[0,26,0,105]);command(0x2b,[0,1,0,160]);command(0x2c);
  li(8,80*160);
  label('pixel');addi(10,22,0);jump('spi',1);addi(10,23,0);jump('spi',1);addi(8,8,-1);branch('pixel',8,0,1);
  uart('FRAME COMPLETE\n');jump('poll');
  label('spi');li(7,128);li(29,64);li(30,4);
  label('bit');sw(29,19);and(28,10,7);branch('zero',28,0);sw(30,18);jump('clock');
  label('zero');sw(30,19);
  label('clock');sw(29,18);i(0x13,7,5,7,1);branch('bit',7,0,1);sw(29,19);i(0x67,0,0,1,0);
  for(const f of fixups){
    if(!labels.has(f.name))throw Error('missing label '+f.name);
    const n=labels.get(f.name)-f.at*4;
    if(f.kind==='jal'){if(n%2||Math.abs(n)>=1<<20)throw Error('jump range');code[f.at]=(((n>>20)&1)<<31)|(((n>>1)&1023)<<21)|(((n>>11)&1)<<20)|(((n>>12)&255)<<12)|(f.rd<<7)|0x6f;}
    else {if(n%2||n< -4096||n>4094)throw Error('branch range');code[f.at]=(((n>>12)&1)<<31)|(((n>>5)&63)<<25)|(f.b<<20)|(f.a<<15)|(f.fn<<12)|(((n>>1)&15)<<8)|(((n>>11)&1)<<7)|0x63;}
  }
  const data=Buffer.alloc(code.length*4);code.forEach((word,index)=>data.writeUInt32LE(word>>>0,index*4));
  const end=32+data.length,checksumAt=Math.floor(end/16)*16+15,image=Buffer.alloc(checksumAt+1+32);
  image[0]=0xe9;image[1]=1;image[3]=0x40;image.writeUInt32LE(0x40800000,4);image.writeUInt16LE(23,12);image.writeUInt16LE(100,15);image.writeUInt16LE(199,17);image[23]=1;
  image.writeUInt32LE(0x40800000,24);image.writeUInt32LE(data.length,28);data.copy(image,32);
  image[checksumAt]=data.reduce((sum,b)=>sum^b,0xef);createHash('sha256').update(image.subarray(0,checksumAt+1)).digest().copy(image,checksumAt+1);
  return image;
}
