// Original RV32 SRAM diagnostic. Every MAC register, PHY read, enhanced DMA
// descriptor, payload check and USER/RGB decision is executed by the P4 guest.
const ENTRY=0x4ff10000,TXDESC=0x4ff03000,RXDESC=TXDESC+0x40,TXBUF=0x48010000,RXBUF=0x48020000,MAC=0x50098000,DMA=MAC+0x1000;
class Asm{
  c=[];labels=new Map();fix=[];
  emit(...words){this.c.push(...words.map(v=>v>>>0));}
  li(r,v){this.emit(((v+0x800)&0xfffff000)|(r<<7)|0x37,((v&0xfff)<<20)|(r<<15)|(r<<7)|0x13);}
  write(address,value){this.li(1,address);this.li(2,value);this.emit(2<<20|1<<15|2<<12|0x23);}
  load(r,address){this.li(1,address);this.emit(1<<15|2<<12|r<<7|3);}
  label(s){this.labels.set(s,this.c.length);}
  branch(r1,r2,label,nz=false){this.fix.push([this.c.length,label,r1,r2,nz]);this.emit(0);}
  jump(label){this.fix.push([this.c.length,label]);this.emit(0);}
  uart(text){for(const b of new TextEncoder().encode(text))this.write(0x500ca000,b);}
  finish(){for(const [at,label,r1,r2,nz]of this.fix){if(!this.labels.has(label))throw Error('missing label '+label);const d=(this.labels.get(label)-at)*4;if(r1===undefined)this.c[at]=((d>>20&1)<<31)|((d>>1&1023)<<21)|((d>>11&1)<<20)|(d&0xff000)|0x6f;else{if(d< -4096||d>4094)throw Error('branch too far');this.c[at]=((d>>12&1)<<31)|((d>>5&63)<<25)|(r2<<20)|(r1<<15)|(nz?1<<12:0)|((d>>1&15)<<8)|((d>>11&1)<<7)|0x63;}}const out=new Uint8Array(this.c.length*4),v=new DataView(out.buffer);this.c.forEach((w,i)=>v.setUint32(4*i,w,true));return out;}
}
export function poeP4Frame(seed=0x5a){const f=new Uint8Array(60);f.fill(255,0,6);f.set([2,3,4,5,6,7,8,0],6);for(let i=14;i<f.length;i++)f[i]=(i-14)^seed;return f;}
function words(a,address,values){values.forEach((v,i)=>a.write(address+4*i,v));}
function frameWords(bytes){const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);return Array.from({length:bytes.length/4},(_,i)=>view.getUint32(4*i,true));}
function mdio(a,reg,phy,tag,expected){a.write(MAC+0x10,1|reg<<6|phy<<11);a.label(tag);a.load(3,MAC+0x10);a.emit(0x0011f193);a.branch(3,0,tag,true);a.load(3,MAC+0x14);a.li(4,expected);a.branch(3,4,'failure',true);}
export function poeP4ProbeImage(){
 const a=new Asm();
 for(const[address,value]of[[0x500e6018,1<<13],[0x500e6030,1<<27|1<<29],[0x500e6034,1|1<<9|1<<10],[0x50111040,1<<15],[0x500e514c,4<<2]])a.write(address,value);
 for(const pin of[49,34,35])a.write(0x500e1004+4*pin,3<<12);
 for(const pin of[29,30,28,50])a.write(0x500e1004+4*pin,3<<12|1<<9);
 for(const[pin,signal]of[[31,108],[52,109]]){a.write(0x500e1004+4*pin,1<<12|1<<9);a.write(0x500e0558+4*pin,signal);}a.write(0x500e0158+4*107,0x80|52);
 a.write(0x500e1004+4*51,1<<12|1<<9);a.write(0x500e0558+4*51,256);a.write(0x500e0030,1<<19);a.write(0x500e0014,1<<19);
 for(const page of[1,2]){a.write(0x5008e380,page);a.write(0x5008e37c,0xc00|page);}
 // IP101GRI addr1; wrong addr2 must float high before the correct ID is read.
 mdio(a,2,2,'wrong-phy',0xffff);mdio(a,2,1,'phy-id',0x0243);
 // Matrix MDI107 is independent of MDIO output109.
 a.write(0x500e0158+4*107,0x80|53);mdio(a,2,1,'wrong-mdi',0xffff);a.write(0x500e0158+4*107,0x80|52);
 words(a,TXBUF,frameWords(poeP4Frame()));
 words(a,TXDESC,[1<<31|1<<28|1<<29|1<<30|1<<20,60,TXBUF,TXDESC,0,0,0,0]);
 words(a,RXDESC,[1<<31,1<<14|1536,RXBUF,RXDESC,0,0,0,0]);
 // Platform reset starts the replacement worker immediately. Wait for a real
 // UART0 byte so the host can reselect its independent raw-frame transport
 // before this guest sends; no queued or synthetic Ethernet result is used.
 a.uart('U213_HOST_READY\n');a.label('host-ready');a.load(3,0x500ca01c);a.emit(0x3ff1f193);a.branch(3,0,'host-ready');a.load(3,0x500ca000);a.li(4,0x47);a.branch(3,4,'failure',true);
 a.write(DMA,1<<7);a.write(DMA+0x10,TXDESC);a.write(DMA+0x0c,RXDESC);a.write(DMA+0x1c,1<<16|1<<15|1<<13|1|1<<6);a.write(MAC,12);
 // REF_CLK pad disconnected: descriptor OWN cannot be consumed.
 a.write(0x500e1004+4*50,0);a.write(DMA+0x18,1<<13|2);a.write(DMA+4,1);a.load(3,TXDESC);a.li(4,1<<31|1<<28|1<<29|1<<30|1<<20);a.branch(3,4,'failure',true);
 a.write(0x500e1004+4*50,3<<12|1<<9);a.write(DMA+4,1);
 a.label('tx-wait');a.load(3,TXDESC);a.li(4,1<<31);a.emit(4<<20|3<<15|7<<12|3<<7|0x33);a.branch(3,0,'tx-wait',true);a.uart('U213_TX_DONE\n');
 a.label('rx-wait');a.load(3,RXDESC);a.li(4,1<<31);a.emit(4<<20|3<<15|7<<12|3<<7|0x33);a.branch(3,0,'rx-wait',true);
 a.load(3,RXDESC);a.li(4,64<<16|1<<9|1<<8);a.branch(3,4,'failure',true);
 for(const[i,value]of frameWords(poeP4Frame(0xa5)).entries()){a.load(3,RXBUF+4*i);a.li(4,value);a.branch(3,4,'failure',true);}a.uart('U213_RX_EXACT\n');
 // Source-correct common-anode red GPIO17; first prove unpressed USER45.
 a.load(3,0x500e0040);a.li(4,1<<13);a.emit(4<<20|3<<15|7<<12|3<<7|0x33);a.branch(3,0,'failure');
 a.write(0x500e1004+4*17,1<<12|1<<9);a.write(0x500e0558+4*17,256);a.write(0x500e0024,1<<17);a.write(0x500e0008,1<<17);a.uart('U213_USER_READY\n');
 a.label('press-wait');a.load(3,0x500e0040);a.li(4,1<<13);a.emit(4<<20|3<<15|7<<12|3<<7|0x33);a.branch(3,0,'press-wait',true);a.write(0x500e000c,1<<17);a.uart('U213_USER_DOWN\n');
 a.label('release-wait');a.load(3,0x500e0040);a.li(4,1<<13);a.emit(4<<20|3<<15|7<<12|3<<7|0x33);a.branch(3,0,'release-wait');a.write(0x500e0008,1<<17);a.uart('U213_USER_UP\n');a.label('done');a.jump('done');
 a.label('failure');a.uart('U213_FAIL\n');a.jump('failure');
 const code=a.finish(),head=new Uint8Array(24),hv=new DataView(head.buffer);head.set([0xe9,2,0,0x40]);hv.setUint32(4,ENTRY,true);hv.setUint16(12,18,true);hv.setUint16(15,100,true);hv.setUint16(17,199,true);
 const desc=new Uint8Array(256);new DataView(desc.buffer).setUint32(0,0xabcd5432,true);desc.set(new TextEncoder().encode('poep4-rv32-emac-probe'),16);
 const parts=[head];let checksum=0xef;for(const[address,bytes]of[[0x4ff60000,desc],[ENTRY,code]]){const sh=new Uint8Array(8),sv=new DataView(sh.buffer);sv.setUint32(0,address,true);sv.setUint32(4,bytes.length,true);parts.push(sh,bytes);for(const b of bytes)checksum^=b;}
 const length=parts.reduce((n,b)=>n+b.length,0),out=new Uint8Array(Math.ceil((length+1)/16)*16);let at=0;for(const p of parts){out.set(p,at);at+=p.length;}out[out.length-1]=checksum;return out;
}
