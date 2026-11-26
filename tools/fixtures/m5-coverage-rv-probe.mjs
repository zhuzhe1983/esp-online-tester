// Authored RV32I SRAM diagnostics, explicit app-direct. No factory image/ROM claim.
// Actual C6 GPIO/I2C/SPI -> PI4IOE5V6408 P0/SSD1306 and H2 PCR/RMT routes.
class RV {
 constructor(){this.words=[];this.labels=new Map();this.fixups=[];}
 emit(w){this.words.push(w>>>0);}
 li(r,v){v>>>=0;this.emit(((v+0x800)&0xfffff000)|(r<<7)|0x37);this.emit(((v&4095)<<20)|(r<<15)|(r<<7)|0x13);}
 addi(d,s,v){this.emit(((v&4095)<<20)|(s<<15)|(d<<7)|0x13);}
 and(d,a,b){this.emit((b<<20)|(a<<15)|(7<<12)|(d<<7)|0x33);}
 lw(d,a){this.li(5,a);this.emit((5<<15)|(2<<12)|(d<<7)|3);}
 sw(r,a){this.li(5,a);this.emit((r<<20)|(5<<15)|(2<<12)|0x23);}
 write(a,v){this.li(6,v);this.sw(6,a);}
 label(n){if(this.labels.has(n))throw Error('duplicate label');this.labels.set(n,this.words.length*4);}
 branch(n,a=0,b=0,fn=0){this.fixups.push({at:this.words.length,n,a,b,fn});this.emit(0);}
 uart(s){for(const b of Buffer.from(s))this.write(0x60000000,b);}
 wait(a,mask){const n='wait-'+this.words.length;this.label(n);this.lw(7,a);this.li(8,mask);this.and(7,7,8);this.branch(n,7,0,1);}
 image(chip,entry,flashCode){for(const f of this.fixups){const n=this.labels.get(f.n)-f.at*4;if(n%2||n< -4096||n>4094)throw Error('branch range '+f.n);this.words[f.at]=(((n>>12)&1)<<31)|(((n>>5)&63)<<25)|(f.b<<20)|(f.a<<15)|(f.fn<<12)|(((n>>1)&15)<<8)|(((n>>11)&1)<<7)|0x63;}const d=Buffer.alloc(this.words.length*4);this.words.forEach((w,i)=>d.writeUInt32LE(w>>>0,i*4));const descriptor=Buffer.alloc(256);descriptor.writeUInt32LE(0xabcd5432);descriptor.write('m5-coverage-rv',48);const check=Math.floor((296+d.length)/16)*16+15,b=Buffer.alloc(check+1);b[0]=0xe9;b[1]=2;b[3]=flashCode<<4;b.writeUInt32LE(entry,4);b.writeUInt16LE(chip,12);b.writeUInt16LE(65535,17);b.writeUInt32LE(0x40800000,24);b.writeUInt32LE(256,28);descriptor.copy(b,32);b.writeUInt32LE(entry,288);b.writeUInt32LE(d.length,292);d.copy(b,296);b[check]=d.reduce((a,v)=>a^v,descriptor.reduce((a,v)=>a^v,0xef));return b;}
}
export function m5CoverageRvProbe(board,{wrongRoute=false}={}){
 const p=new RV(),GPIO=0x60091000,mode=(pin,signal=128)=>{p.write(0x60090004+pin*4,1<<12|1<<9);p.write(GPIO+0x554+pin*4,signal);if(signal===128)p.write(GPIO+0x24,1<<pin);},output=(pin,high)=>p.write(GPIO+(high?8:12),1<<pin);
 if(board==='nanoh2'){
  mode(10);output(10,true);mode(4);output(4,false);p.write(0x60090028,1<<12|1<<9|1<<8);
  p.write(0x60096034,1);p.write(0x60096038,1<<21);p.write(0x60007068,0x8500001b);mode(wrongRoute?8:11,71);
  const rmt=green=>{let i=0;for(const b of green?[255,0,0]:[0,255,0])for(let bit=7;bit>=0;bit--)p.write(0x60007400+4*i++,(b&(1<<bit))?0x8000|26|(14<<16):0x8000|13|(27<<16));p.write(0x60007460,0);p.write(0x60007044,0xffffffff);p.write(0x60007010,(1<<16)|(1<<8)|1);const wait='rmt-'+p.words.length;p.label(wait);p.lw(7,0x60007038);p.li(8,1);p.and(7,7,8);p.branch(wait,7,0);};
  p.uart('COVERAGE nanoh2 READY\n');p.li(21,0xffffffff);p.label('poll');p.lw(20,GPIO+0x3c);p.li(8,1<<9);p.and(20,20,8);p.branch('poll',20,21);p.addi(21,20,0);p.branch('down',20,0);p.uart('KEY UP\n');rmt(false);p.uart('OUTPUT COMPLETE\n');p.branch('poll');p.label('down');p.uart('KEY DOWN\n');rmt(true);p.uart('OUTPUT COMPLETE\n');p.branch('poll');return p.image(16,0x40801000,2);
 }
 if(board!=='unit-c6l')throw Error('Unknown coverage RV board');
 for(const[pin,sig]of[[wrongRoute?11:10,46],[8,45]]){mode(pin,sig);p.write(GPIO+0x154+sig*4,0x80|pin);}
 for(const[pin,sig]of[[20,63],[21,65],[6,68]])mode(pin,sig);mode(18);mode(15);output(15,true);p.write(0x60081020,0x3e);
 const spi=bytes=>{for(let i=0;i<bytes.length;i+=4){let v=0;for(let j=0;j<4&&i+j<bytes.length;j++)v|=bytes[i+j]<<(8*j);p.write(0x60081098+i,v);}p.write(0x60081010,1<<27);p.write(0x6008101c,bytes.length*8-1);p.write(0x60081000,1<<24);p.wait(0x60081000,1<<24);};
 const cmd=bytes=>{output(18,false);spi(bytes);};cmd([0xaf,0x20,0,0x21,32,95,0x22,0,5]);
 p.uart('COVERAGE unit-c6l READY\n');p.li(21,0xffffffff);p.label('poll');
 const I2C=0x60004000;p.write(I2C+0x24,0xffffffff);p.write(I2C+0x18,3<<12);for(const b of[0x86,15,0x87])p.write(I2C+0x1c,b);[6<<11,(1<<11)|(1<<8)|2,6<<11,(1<<11)|(1<<8)|1,(3<<11)|1,2<<11].forEach((v,i)=>p.write(I2C+0x58+4*i,v));p.write(I2C+4,1<<5);
 const done='i2c-done';p.label(done);p.lw(7,I2C+0x20);p.li(8,(1<<7)|(1<<10));p.and(7,7,8);p.branch(done,7,0);p.lw(7,I2C+0x20);p.li(8,1<<10);p.and(7,7,8);p.branch('i2c-nack',7,0,1);p.lw(20,I2C+0x1c);p.li(8,1);p.and(20,20,8);p.branch('i2c-sampled');p.label('i2c-nack');p.li(20,1);p.li(22,0xffffffff);p.branch('i2c-sampled',21,22,1);p.uart('I2C NACK\n');p.label('i2c-sampled');p.branch('poll',20,21);p.addi(21,20,0);p.branch('down',20,0);
 p.uart('KEY UP\n');cmd([0x21,32,32,0x22,0,0]);output(18,true);spi([1]);p.uart('OUTPUT COMPLETE\n');p.branch('poll');p.label('down');p.uart('KEY DOWN\n');cmd([0x21,32,32,0x22,0,0]);output(18,true);spi([2]);p.uart('OUTPUT COMPLETE\n');p.branch('poll');return p.image(13,0x40801000,4);
}
