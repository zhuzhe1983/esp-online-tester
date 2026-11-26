// Newly authored RV32I guest diagnostics. No ROM patches or manufacturer binary.
export class Rv32 {
 constructor(){this.words=[];this.labels=new Map();this.fixups=[];}
 emit(w){this.words.push(w>>>0);}
 li(r,v){v>>>=0;this.emit(((v+0x800)&0xfffff000)|(r<<7)|0x37);this.emit(((v&4095)<<20)|(r<<15)|(r<<7)|0x13);}
 addi(rd,rs,v){this.emit(((v&4095)<<20)|(rs<<15)|(rd<<7)|0x13);}
 and(rd,a,b){this.emit((b<<20)|(a<<15)|(7<<12)|(rd<<7)|0x33);}
 lw(rd,address){this.li(5,address);this.emit((5<<15)|(2<<12)|(rd<<7)|3);}
 sw(rs,address){this.li(5,address);this.emit((rs<<20)|(5<<15)|(2<<12)|0x23);}
 write(address,value){this.li(6,value);this.sw(6,address);}
 label(n){if(this.labels.has(n))throw Error('duplicate label '+n);this.labels.set(n,this.words.length*4);}
 branch(n,a=0,b=0,fn=0){this.fixups.push({at:this.words.length,n,a,b,fn});this.emit(0);}
 uart(s){for(const b of new TextEncoder().encode(s))this.write(0x60000000,b);}
 bytes(){for(const f of this.fixups){const n=this.labels.get(f.n)-f.at*4;if(!Number.isInteger(n)||n%2||n< -4096||n>4094)throw Error('branch range '+f.n);this.words[f.at]=(((n>>12)&1)<<31)|(((n>>5)&63)<<25)|(f.b<<20)|(f.a<<15)|(f.fn<<12)|(((n>>1)&15)<<8)|(((n>>11)&1)<<7)|0x63;}const b=Buffer.alloc(this.words.length*4);this.words.forEach((w,i)=>b.writeUInt32LE(w>>>0,i*4));return b;}
 image(chip,entry=0x40800000,flashCode=2){const data=this.bytes(),end=32+data.length,check=Math.floor(end/16)*16+15,b=Buffer.alloc(check+1);b[0]=0xe9;b[1]=1;b[3]=flashCode<<4;b.writeUInt32LE(entry>>>0,4);b.writeUInt16LE(chip,12);b.writeUInt16LE(65535,17);b.writeUInt32LE(entry>>>0,24);b.writeUInt32LE(data.length,28);data.copy(b,32);b[check]=data.reduce((a,v)=>a^v,0xef);return b;}
}
export function m5SsrProbe({pin=4,mux=true,enabled=true}={}){
 const p=new Rv32();
 p.write(0x60009004+4*pin,mux?1<<12:0);p.write(0x60004554+4*pin,128);p.write(enabled?0x60004024:0x60004028,1<<pin);p.write(0x6000400c,1<<pin);
 p.write(0x60009010,1<<12|1<<9|1<<8);p.uart('SSR READY\n');p.li(21,2);
 p.label('poll');p.lw(7,0x6000403c);p.li(8,1<<3);p.and(7,7,8);p.emit((7<<20)|(3<<12)|(7<<7)|0x33); // sltu x7,x0,x7
 p.branch('poll',7,21);p.addi(21,7,0);p.branch('down',7,0);
 p.write(0x6000400c,1<<pin);p.uart('SSR KEY UP\n');p.branch('poll');
 p.label('down');p.write(0x60004008,1<<pin);p.uart('SSR KEY DOWN\n');p.branch('poll');
 return p.image(5,0x40380000,2);
}

Rv32.prototype.andi=function(rd,rs,v){this.emit(((v&4095)<<20)|(rs<<15)|(7<<12)|(rd<<7)|0x13);};
Rv32.prototype.shift=function(rd,rs,bits,arithmetic=false,left=false){this.emit(((bits|(arithmetic?0x400:0))<<20)|(rs<<15)|((left?1:5)<<12)|(rd<<7)|0x13);};
Rv32.prototype.hex=function(register){
 for(let shift=28;shift>=0;shift-=4){const tag='hex'+this.words.length;this.shift(11,register,shift);this.andi(11,11,15);this.addi(12,11,-10);this.branch(tag+'digit',12,0,4);this.addi(11,11,87);this.branch(tag+'emit');this.label(tag+'digit');this.addi(11,11,48);this.label(tag+'emit');this.sw(11,0x60000000);}
};
Rv32.prototype.wait=function(address,mask,set=false){const tag='wait'+this.words.length;this.li(5,address);this.li(7,mask);this.label(tag);this.emit((5<<15)|(2<<12)|(6<<7)|3);this.and(6,6,7);this.branch(tag,6,0,set?0:1);};
Rv32.prototype.bswap10=function(){this.li(12,0);for(let i=0;i<4;i++){this.shift(11,10,i*8);this.andi(11,11,255);this.shift(11,11,24-i*8,false,true);this.emit((11<<20)|(12<<15)|(6<<12)|(12<<7)|0x33);}this.addi(10,12,0);};

export function m5GatewayH2Probe({tx=24,rx=23,enabled=true,div=0xc00115}={}){
 const p=new Rv32();p.write(0x60090064,1<<12);p.write(0x600915b4,128);p.write(0x60000014,div);
 for(const pin of [tx,rx])p.write(0x60090004+4*pin,1<<12|1<<9);
 p.write(0x60091554+4*tx,6);p.write(0x6009116c,0x80|rx);p.write(0x60091024,enabled?1<<tx:0);p.uart('H2 HOST READY\n');
 p.label('poll');p.lw(7,0x6000001c);p.andi(7,7,255);p.branch('poll',7,0);p.lw(10,0x60000000);p.uart('H2 RX ');p.hex(10);p.uart('\n');p.branch('poll');
 return p.image(16,0x40800000,1);
}
export const CARDKB2_COLUMNS=[4,5,6,7,8,29,10,11,22,23,24];
export function m5CardKb2Probe({badMux=false,enabled=true}={}){
 const p=new Rv32();for(let row=0;row<4;row++){p.write(0x60090000+4*row,badMux?0:1<<12);p.write(0x60091ad4+4*row,256);p.li(24+row,0xffff_ffff);}
 for(const col of CARDKB2_COLUMNS)p.write(0x60090000+4*col,1<<12|1<<9|1<<7);p.write(0x60091038,enabled?15:0);p.uart('CARDKB2 READY\n');
 p.label('scan');for(let row=0;row<4;row++){p.write(0x60091004,1<<row);p.lw(10,0x60091064);p.li(8,CARDKB2_COLUMNS.reduce((a,b)=>a|1<<b,0));p.and(10,10,8);p.branch('next'+row,10,24+row);p.addi(24+row,10,0);p.uart('KB ROW'+row+' ');p.hex(10);p.uart('\n');p.label('next'+row);}p.write(0x60091004,0);p.branch('scan');return p.image(20);
}
export function m5KmeterProbe({miso=5,clock=6}={}){
 const p=new Rv32();p.write(0x6000456c,128);
 for(const [pin,sig]of[[clock,63],[7,68]]){p.write(0x60009004+4*pin,1<<12|1<<9);p.write(0x60004554+4*pin,sig);}
 p.write(0x60009004+4*miso,1<<12|1<<9);p.write(0x60004254,0x40|miso);p.write(0x60024020,0x3e);p.write(0x60024010,1<<28);p.write(0x6002401c,31);p.li(21,0xffff_fffe);p.uart('KMETER READY\n');
 p.label('sample');p.write(0x60024000,1<<24);p.wait(0x60024000,1<<24);p.lw(10,0x60024098);p.bswap10();p.branch('sample',10,21);p.addi(21,10,0);p.uart('KMETER FRAME ');p.hex(10);p.uart(' HOTQ ');p.shift(10,10,18,true);p.hex(10);p.uart('\n');p.branch('sample');return p.image(5,0x40380000);
}
export function m5SwitchC6Probe({onPin=15,mux=true,enabled=true}={}){
 const p=new Rv32();for(const pin of[14,onPin]){p.write(0x60090004+4*pin,pin===onPin&&!mux?0:1<<12);p.write(0x60091554+4*pin,128);if(pin!==onPin||enabled)p.write(0x60091024,1<<pin);}
 p.write(0x60090020,1<<12|1<<9|1<<8);p.li(21,2);p.uart('SWITCHC6 READY\n');p.label('poll');p.lw(7,0x6009103c);p.li(8,1<<7);p.and(7,7,8);p.emit((7<<20)|(3<<12)|(7<<7)|0x33);p.branch('poll',7,21);p.addi(21,7,0);p.branch('on',7,0);
 p.write(0x60091008,1<<14);p.write(0x6009100c,1<<14);p.uart('SWITCH OFF\n');p.branch('poll');p.label('on');p.write(0x60091008,1<<onPin);p.write(0x6009100c,1<<onPin);p.uart('SWITCH ON\n');p.branch('poll');return p.image(13);
}
export function m5A172Probe({badPin=null}={}){
 const p=new Rv32();for(let pin=18;pin<=23;pin++)p.write(0x60090004+4*pin,(pin===badPin?1<<12:0)|1<<9);
 p.write(0x60016004,0x232013);p.write(0x60018078,0x41313732);p.li(21,0xffff_ffff);p.label('poll');p.lw(10,0x60018078);p.branch('poll',10,21);p.addi(21,10,0);p.uart('A172 MAILBOX ');p.hex(10);p.uart('\n');p.branch('poll');return p.image(13);
}
function c5Pin(p,pin,sig){p.write(0x60090000+4*pin,0x1a00);p.write(0x60091ad4+4*pin,sig);p.write(0x60091038,1<<pin);}
function c5I2c(p,addr,reg,value=null,count=0){
 const i=0x60004000;p.write(i+0x24,0x7ffff);p.write(i+0x18,0x408b|3<<12);p.write(i+0x1c,addr<<1);p.write(i+0x1c,reg);p.write(i+0x58,6<<11);
 if(value!==null){p.write(i+0x1c,value);p.write(i+0x5c,1<<11|1<<8|3);p.write(i+0x60,2<<11);}
 else{p.write(i+0x1c,addr<<1|1);for(const[o,v]of[[0x5c,1<<11|1<<8|2],[0x60,6<<11],[0x64,1<<11|1<<8|1],[0x68,3<<11|count-1],[0x6c,3<<11|1<<10|1],[0x70,2<<11]])p.write(i+o,v);}
 p.write(i+4,1<<4|1<<5);p.wait(i+0x20,1<<7|1<<10,true);
}
function c5Lcd(p,cmd,data=[]){
 for(const[dc,bytes]of[[false,[cmd]],[true,data]]){if(!bytes.length)continue;p.write(dc?0x60091008:0x6009100c,1<<26);p.write(0x60081010,1<<27);p.write(0x6008101c,bytes.length*8-1);p.write(0x60081098,bytes.reduce((v,b,i)=>v|b<<(i*8),0));p.write(0x60081000,1<<24);p.wait(0x60081000,1<<24);}
}
export function m5ToughC5Probe({wrongClock=false}={}){
 const p=new Rv32();p.write(0x60096020,3);p.write(0x60096020,1);p.write(0x60096024,1<<22);
 c5Pin(p,2,47);c5Pin(p,3,46);p.write(0x60091390,0x102);p.write(0x6009138c,0x103);p.write(0x60004000,239);p.write(0x60004038,240);for(const o of[0x40,0x44,0x48,0x4c])p.write(0x60004000+o,0);
 for(const[a,r,v]of[[0x4f,0x13,0xe7],[0x4f,3,0x18],[0x4f,5,0x18],[0x4f,0x14,0x3d],[0x4f,4,2],[0x4f,0x25,0xe8],[0x4f,0x26,3],[0x4f,0x21,255],[0x4f,0x22,0x8f],[0x6e,0x10,4],[0x6e,0x11,4],[0x6e,0x13,0]])c5I2c(p,a,r,v);
 for(const[pin,sig]of[[9,wrongClock?256:56],[7,58],[25,61],[26,256]])c5Pin(p,pin,sig);p.write(0x6008100c,1<<12);p.write(0x60081020,0x3e);
 for(const[c,d]of[[0x11,[]],[0x21,[]],[0x29,[]],[0x3a,[0x55]],[0x2a,[0,0,0,0]],[0x2b,[0,0,0,0]]])c5Lcd(p,c,d);
 p.li(21,2);p.uart('TOUGH C5 READY\n');p.label('poll');c5I2c(p,0x2e,2,null,5);p.lw(10,0x6000401c);p.lw(11,0x6000401c);p.lw(22,0x6000401c);p.lw(11,0x6000401c);p.lw(23,0x6000401c);p.branch('poll',10,21);p.addi(21,10,0);p.uart('TOUGH TOUCH ');p.hex(10);p.uart(' X ');p.hex(22);p.uart(' Y ');p.hex(23);p.uart('\n');p.branch('up',21,0);c5Lcd(p,0x2c,[7,0xe0]);p.branch('poll');p.label('up');c5Lcd(p,0x2c,[0xf8,0]);p.branch('poll');return p.image(23,0x40800000,4);
}
