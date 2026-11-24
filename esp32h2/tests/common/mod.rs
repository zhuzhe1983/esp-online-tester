#![allow(dead_code)]
//! Source-visible, newly authored SRAM test program. Never a manufacturer image.
pub fn li(w:&mut Vec<u32>,r:u32,v:u32){let hi=v.wrapping_add(0x800)&0xfffff000;w.push(hi|(r<<7)|0x37);w.push(((v&0xfff)<<20)|(r<<15)|(r<<7)|0x13);}
pub fn store(w:&mut Vec<u32>,a:u32,v:u32){li(w,5,a);li(w,6,v);w.push((6<<20)|(5<<15)|(2<<12)|0x23);}
fn branch(rs1:u32,rs2:u32,delta:i32,funct:u32)->u32{let v=delta as u32;((v>>12&1)<<31)|((v>>5&63)<<25)|(rs2<<20)|(rs1<<15)|(funct<<12)|((v>>1&15)<<8)|((v>>11&1)<<7)|0x63}
pub fn program(pin:u32)->Vec<u8>{
 let mut w=vec![];
 // H2 PCR RMT SCLK uses32MHz XTAL, no C6 peripheral address or80MHz clock.
 store(&mut w,0x60096034,1);store(&mut w,0x60096038,1<<21);
 store(&mut w,0x60007068,0x8500001b);
 store(&mut w,0x60090004+pin*4,1<<12);store(&mut w,0x60091554+pin*4,71);
 for(i,bit)in[0x12u8,0x34,0x56].into_iter().flat_map(|v|(0..8).rev().map(move|i|v&(1<<i)!=0)).enumerate(){store(&mut w,0x60007400+i as u32*4,if bit{0x8000|26|(14<<16)}else{0x8000|13|(27<<16)});}
 store(&mut w,0x60007460,0);store(&mut w,0x60007010,(1<<16)|(1<<8)|1);
 li(&mut w,5,0x60007038);w.push((5<<15)|(2<<12)|(6<<7)|3);w.push((1<<20)|(6<<15)|(7<<12)|(6<<7)|0x13);w.push(branch(6,0,-8,0));
 for b in b"H2 SRAM chip16 RGB8\n"{store(&mut w,0x60000000,u32::from(*b));}
 li(&mut w,5,0x6009103c);w.push((5<<15)|(2<<12)|(10<<7)|3);
 li(&mut w,5,0x40804000);w.push((10<<20)|(5<<15)|(2<<12)|0x23);
 store(&mut w,0x50000000,0x48325352);
 w.push(0x0000006f);w.into_iter().flat_map(u32::to_le_bytes).collect()
}
pub fn image(entry:u32,segments:&[(u32,Vec<u8>)],hash:bool)->Vec<u8>{
 let mut b=vec![0;24];b[0]=0xe9;b[1]=segments.len()as u8;b[3]=0x20;b[4..8].copy_from_slice(&entry.to_le_bytes());b[8]=0xee;b[12]=16;b[17..19].copy_from_slice(&0xffffu16.to_le_bytes());b[23]=u8::from(hash);let mut checksum=0xef;
 for(a,d)in segments{b.extend_from_slice(&a.to_le_bytes());b.extend_from_slice(&(d.len()as u32).to_le_bytes());b.extend_from_slice(d);checksum=d.iter().fold(checksum,|v,x|v^x);}
 while b.len()%16!=15{b.push(0);}b.push(checksum);
 if hash{let mut padded=b.clone();padded.push(0x80);while padded.len()%64!=56{padded.push(0);}padded.extend_from_slice(&(b.len()as u64*8).to_be_bytes());let mut sha=esp_periph::Sha::new();for(i,blk)in padded.as_chunks::<64>().0.iter().enumerate(){sha.hash_block(blk,i==0);}b.extend(sha.h[..8].iter().flat_map(|v|v.to_be_bytes()));}b
}
pub fn fixture(pin:u32)->Vec<u8>{image(0x40800000,&[(0x40800000,program(pin)),(0x50000000,vec![0;4])],true)}
pub fn configure_rmt(b:&mut esp32h2::H2Bus,pin:u32,div:u32,symbols:&[u32]){
 use emu_core::Bus;
 b.write32(0x60096038,1<<21).unwrap();b.write32(0x60007068,0x8500001b).unwrap();b.write32(0x60090004+pin*4,1<<12).unwrap();b.write32(0x60091554+pin*4,71).unwrap();
 for(i,v)in symbols.iter().enumerate(){b.write32(0x60007400+i as u32*4,*v).unwrap();}
 b.write32(0x60007010,(1<<16)|(div<<8)|1).unwrap();
}
