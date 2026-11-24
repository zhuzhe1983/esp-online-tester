//! ESP32-S31-Zero: four storage assemblies, each available with/without headers.
//! Exact BOOT61 / WS2812B-1010 GPIO60: waveshareteam/ESP32-S31-Zero
//! 8feb078e9a7c958635cc77db3a61f8bb527f0156, example02 and schematic.
//! The factory board-test LCD/SD fixture is not an onboard peripheral.
use esp_soc::{Board,BoardModel,board::BoardEdge,devices::Ws2812Chain};
pub const PROFILES:[(&str,usize,usize);4]=[
    ("waveshare-s31-zero-n16r16",16,16),("waveshare-s31-zero-n16r32",16,32),
    ("waveshare-s31-zero-n32r16",32,16),("waveshare-s31-zero-n32r32",32,32),
];
pub fn inputs(name:&str)->Option<crate::soc::BoardInputs>{
    // A host-selected Winbond-compatible NOR scenario, not identification of a
    // physical board's manufacturer. The documented capacity must also reach
    // guest RDID (18=16MiB, 19=32MiB), not just the host backing allocation.
    // The inherited partial SFDP header supplies no density/BFPT measurement.
    PROFILES.iter().find(|p|p.0==name).map(|p|crate::soc::BoardInputs{flash_id:[0xef,0x40,if p.1==32{0x19}else{0x18}],psram_bytes:p.2<<20,inputs:Vec::new(),..crate::soc::BoardInputs::devkit_w25q128()})
}
pub fn make_board(name:&str)->Option<Board>{PROFILES.iter().find(|p|p.0==name).map(|p|Box::new(Zero{name:p.0,boot:true,cycle:0,edges:Vec::new(),rgb:Ws2812Chain::new(1).with_gpio_clock(1_000_000_000)})as Board)}
struct Zero{name:&'static str,boot:bool,cycle:u64,edges:Vec<BoardEdge>,rgb:Ws2812Chain}
impl BoardModel for Zero{
    fn name(&self)->&'static str{self.name}
    fn named_pin(&self,name:&str)->Option<u8>{(name=="boot").then_some(61)}
    fn input_levels(&self)->Vec<(u8,bool)>{vec![(61,self.boot)]}
    fn button_at(&mut self,_cycle:u64,id:&str,pressed:bool)->bool{if id!="boot"{return false;}let high=!pressed;if high!=self.boot{self.boot=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:61,level:high});}true}
    fn gpio_output_at(&mut self,ns:u64,_changes:&[(u8,bool)],enabled:u64,output:u64){self.cycle=ns;self.rgb.gpio_drive(ns,enabled&(1<<60)!=0,output&(1<<60)!=0);}
    fn advance_to(&mut self,ns:u64){self.cycle=ns;self.rgb.advance_gpio(ns);}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]){if pin==60&&bits.len()==24{self.rgb.from_bits(bits);}}
    fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
    fn leds(&self)->Option<(&[[u8;3]],u64)>{Some((&self.rgb.leds,self.rgb.updates))}
    fn report(&self)->String{format!("[{}] BOOT61 WS2812-GRB60 updates={}; no external board-test display/touch/SD fixture; RMT decoding is a ratio-only digital subset, not waveform/analog validation; native USB/radios and factory boot unverified",self.name,self.rgb.updates)}
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn s31_guest_rdid_and_flash_mmu_boundaries_follow_each_zero_capacity(){
        use crate::{bus::SRAM_BASE,cache::{FLASH_MMU,FLASH_VADDR},mspi};
        use emu_core::Bus;
        fn li(code:&mut Vec<u32>,reg:u32,value:u32){code.push((value.wrapping_add(0x800)&0xfffff000)|(reg<<7)|0x37);code.push(((value&0xfff)<<20)|(reg<<15)|(reg<<7)|0x13);}
        fn write(code:&mut Vec<u32>,addr:u32,value:u32){li(code,1,addr);li(code,2,value);code.push((2<<20)|(1<<15)|(2<<12)|0x23);}
        fn read_to(code:&mut Vec<u32>,addr:u32,dst:u32){li(code,1,addr);code.push((1<<15)|(2<<12)|(2<<7)|3);li(code,1,dst);code.push((2<<20)|(1<<15)|(2<<12)|0x23);}
        fn mapped_read(code:&mut Vec<u32>,offset:u32,dst:u32){
            write(code,FLASH_MMU+0x380,0);write(code,FLASH_MMU+0x37c,0x1000|(offset/32768));
            read_to(code,FLASH_VADDR+offset%32768,dst);
        }
        for(name,flash_mb,_)in PROFILES{
            let size=flash_mb<<20;let mut m=crate::soc::machine([0;6],size);
            m.bus.set_board_inputs(inputs(name).unwrap()).unwrap();m.bus.board=make_board(name).unwrap();m.bus.attach_board_devices();
            m.bus.write_flash(0,&[0x11,0x22,0x33,0x44]).unwrap();
            m.bus.write_flash((16<<20)-4,&[0x55,0x66,0x77,0x88]).unwrap();
            if flash_mb==32{m.bus.write_flash(16<<20,&[0x99,0xaa,0xbb,0xcc]).unwrap();m.bus.write_flash(size-4,&[0xdd,0xee,0x12,0x34]).unwrap();}
            let output=SRAM_BASE+0x10000;let mut code=Vec::new();
            // SPI1 controller RDID; the CPU copies the real receive word.
            write(&mut code,mspi::BASE+0x1000,1<<28);read_to(&mut code,mspi::BASE+0x1058,output);
            write(&mut code,FLASH_MMU+0x3c,0x40000001);write(&mut code,FLASH_MMU+0x384,3<<3);
            mapped_read(&mut code,0,output+4);mapped_read(&mut code,(16<<20)-4,output+8);
            if flash_mb==32{mapped_read(&mut code,16<<20,output+12);mapped_read(&mut code,size as u32-4,output+16);}
            code.push(0x6f);m.bus.load_bytes(SRAM_BASE,&code.iter().flat_map(|v|v.to_le_bytes()).collect::<Vec<_>>()).unwrap();m.cores[0].pc=SRAM_BASE;
            assert!(matches!(m.run(2000),esp_soc::Stop::MaxInsns));
            assert_eq!(m.bus.read32(output).unwrap()&0x00ffffff,if flash_mb==32{0x1940ef}else{0x1840ef});
            assert_eq!(m.bus.read32(output+4).unwrap(),0x44332211);assert_eq!(m.bus.read32(output+8).unwrap(),0x88776655);
            if flash_mb==32{assert_eq!(m.bus.read32(output+12).unwrap(),0xccbbaa99);assert_eq!(m.bus.read32(output+16).unwrap(),0x3412eedd);}
            assert!(m.bus.inner.unmodelled.is_empty()&&m.bus.inner.refused.is_empty());
            // Mapping the first byte after the configured backing must fault,
            // including 16MiB on N16: it must never wrap to address0.
            let mut negative=Vec::new();mapped_read(&mut negative,size as u32,output+20);negative.push(0x6f);
            m.bus.load_bytes(SRAM_BASE+0x2000,&negative.iter().flat_map(|v|v.to_le_bytes()).collect::<Vec<_>>()).unwrap();m.cores[0].pc=SRAM_BASE+0x2000;m.dbg.stop_after_exceptions=1;
            assert!(matches!(m.run(2000),esp_soc::Stop::Exceptions(1)));assert_eq!(m.cores[0].mcause&0x7ff,5);assert_eq!(m.cores[0].mtval,FLASH_VADDR);
        }
    }
    #[test]
    fn storage_variants_do_not_attach_factory_test_fixture(){
        for(name,_,psram)in PROFILES{let mut b=make_board(name).unwrap();assert_eq!(inputs(name).unwrap().psram_bytes,psram<<20);assert!(inputs(name).unwrap().efuse.is_empty());assert!(b.display().is_none()&&b.i2c_devices().is_empty()&&b.sd_card_slot().is_none());assert!(b.button_at(0,"boot",true));assert_eq!(b.input_levels(),[(61,false)]);let bits=(0..24).map(|i|0x123456&(1<<(23-i))!=0).collect::<Vec<_>>();b.rmt_frame(59,&bits);assert_eq!(b.leds().unwrap().1,0);b.rmt_frame(60,&bits);assert_eq!(b.leds().unwrap().0,[[0x34,0x12,0x56]]);}
    }
    // Instructions run on the actual S31 CPU, configure IO_MUX and RMT, then read BOOT.
    // No ROM/factory image, peripheral result injection or host-generated LED frame.
    #[test]
    fn s31_guest_rmt_and_gpio_route_reach_only_zero_led60(){
        use crate::{bus::SRAM_BASE,pads::{GPIO_BASE,IO_MUX_BASE},rmt};
        fn li(code:&mut Vec<u32>,reg:u32,value:u32){code.push((value.wrapping_add(0x800)&0xfffff000)|(reg<<7)|0x37);code.push(((value&0xfff)<<20)|(reg<<15)|(reg<<7)|0x13);}
        fn write(code:&mut Vec<u32>,addr:u32,value:u32){li(code,1,addr);li(code,2,value);code.push((2<<20)|(1<<15)|(2<<12)|0x23);}
        for pin in [59,60]{
            let mut m=crate::soc::machine([0;6],16<<20);m.bus.board=make_board("waveshare-s31-zero-n16r16").unwrap();m.bus.attach_board_devices();let mut code=Vec::new();
            write(&mut code,IO_MUX_BASE+pin*4,1<<12);write(&mut code,GPIO_BASE+0xaf4+pin*4,rmt::SIGNAL0);
            write(&mut code,IO_MUX_BASE+61*4,(1<<12)|(1<<9));write(&mut code,rmt::CLOCK,0x21);
            for i in 0..24{let one=0x123456&(1<<(23-i))!=0;write(&mut code,rmt::BASE+0x800+i*4,0x8000|if one{8|(4<<16)}else{4|(8<<16)});}
            write(&mut code,rmt::BASE+0x800+24*4,0);write(&mut code,rmt::BASE+0x20,(4<<8)|(1<<16)|1);
            li(&mut code,1,GPIO_BASE+0x68);code.push((1<<15)|(2<<12)|(2<<7)|3);li(&mut code,1,SRAM_BASE+0x10000);code.push((2<<20)|(1<<15)|(2<<12)|0x23);code.push(0x6f);
            m.bus.load_bytes(SRAM_BASE,&code.iter().flat_map(|i|i.to_le_bytes()).collect::<Vec<_>>()).unwrap();m.cores[0].pc=SRAM_BASE;
            assert!(matches!(m.run(100_000),esp_soc::Stop::MaxInsns));
            assert_eq!(m.bus.board.leds().unwrap().0,if pin==60{[[0x34,0x12,0x56]]}else{[[0,0,0]]});
            use emu_core::Bus;assert_ne!(m.bus.read32(SRAM_BASE+0x10000).unwrap()&(1<<29),0);
            assert!(m.bus.board.button_at(0,"boot",true));m.cores[0].pc=SRAM_BASE;assert!(matches!(m.run(100_000),esp_soc::Stop::MaxInsns));assert_eq!(m.bus.read32(SRAM_BASE+0x10000).unwrap()&(1<<29),0);
            assert!(m.bus.inner.unmodelled.is_empty()&&m.bus.inner.refused.is_empty());
        }
    }
}
