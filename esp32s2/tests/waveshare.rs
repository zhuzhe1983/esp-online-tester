//! Actual S2 LX7 integer instructions drive S2 MMIO and mapped PSRAM.
//! Tests cover only the explicit SRAM diagnostic / data-PSRAM subset.
use emu_core::{Bus,Core,Fault};
use esp_soc::SocBus;
use esp32s2::{S2Bus,S2Core,bus::{IRAM_BASE,GPIO_BASE as GPIO,IO_MUX_BASE as MUX},psram::{EXTMEM,MMU},waveshare::{PICO,LCD}};
struct Guest {bus:S2Bus,cpu:S2Core}
impl Guest {
    fn new(id:&str)->Self {let mut bus=S2Bus::new();bus.set_flash_size(4<<20);bus.set_psram_size(8<<20).unwrap();bus.attach_board(esp32s2::board::make_board(id).unwrap());let mut s=Self{bus,cpu:S2Core::new()};s.program();s}
    fn program(&mut self){self.bus.load_bytes(IRAM_BASE,&[0x32,0x64,0,0x52,0x24,0]).unwrap();self.cpu.ps=0;}
    fn put(&mut self,a:u32,v:u32) {self.cpu.pc=IRAM_BASE;self.cpu.set_ar(4,a);self.cpu.set_ar(3,v);assert!(self.cpu.step(&mut self.bus).trap().is_none(),"S2 guest store {a:#x}");}
    fn get(&mut self,a:u32)->u32 {self.cpu.pc=IRAM_BASE+3;self.cpu.set_ar(4,a);assert!(self.cpu.step(&mut self.bus).trap().is_none(),"S2 guest load {a:#x}");self.cpu.get_ar(5)}
    fn mux(&mut self,pin:u8,function:u32){self.put(MUX+4+4*u32::from(pin),(function<<12)|(1<<9));}
    fn level(&mut self,pin:u8,high:bool){let off=match(pin<32,high){(true,true)=>8,(true,false)=>12,(false,true)=>20,(false,false)=>24};self.put(GPIO+off,1<<(pin%32));}
    fn output(&mut self,pin:u8,high:bool){self.mux(pin,1);self.put(GPIO+0x554+4*u32::from(pin),256);self.put(GPIO+if pin<32{0x24}else{0x30},1<<(pin%32));self.level(pin,high);}
    fn spi(&mut self,dc:bool,bytes:&[u8]){self.level(9,false);self.level(18,dc);for &byte in bytes{for bit in(0..8).rev(){self.level(10,false);self.level(11,byte&(1<<bit)!=0);self.level(10,true);}}self.level(9,true);}
    fn cmd(&mut self,c:u8,data:&[u8]){self.spi(false,&[c]);if !data.is_empty(){self.spi(true,data);}}
    fn pixel(&mut self,c:u16){self.cmd(0x2a,&[0,1,0,1]);self.cmd(0x2b,&[0,26,0,26]);self.cmd(0x2c,&c.to_be_bytes());}
}
#[test]
fn s2_pico_actual_guest_mmu_last_page_bus_masks_capacity_reset_and_no_s3_aliases() {
    for id in[PICO,LCD] {
        let mut g=Guest::new(id);assert_eq!(g.get(EXTMEM),0);assert_eq!(g.get(EXTMEM+4),7);assert_eq!(g.get(MMU+383*4),1<<14);
        assert_eq!(g.bus.read32(0x3f500000),Err(Fault::Unmapped));g.put(EXTMEM,1);g.put(EXTMEM+4,0);
        for (vaddr,entry)in[(0x3f500000,336),(0x3f800000,256),(0x3fc00000,192)] {g.put(MMU+entry*4,(1<<16)|127);g.put(vaddr+0xfffc,0x89abcdef);assert_eq!(g.get(vaddr+0xfffc),0x89abcdef);}
        g.put(MMU+337*4,1<<16);assert_eq!(g.get(0x3f510000),0,"highest page must not wrap or alias physical page0");
        g.put(MMU+336*4,(1<<16)|128);assert_eq!(g.bus.read32(0x3f500000),Err(Fault::Unmapped));assert_eq!(g.bus.write32(0x3f500000,1),Err(Fault::Unmapped));
        for bad in[127,(1<<15)|127,(1<<16)|(1<<15)|127,(1<<16)|(1<<14)|127] {g.put(MMU+336*4,bad);assert_eq!(g.bus.read32(0x3f500000),Err(Fault::Unmapped));}
        g.put(MMU+336*4,(1<<16)|127);g.put(EXTMEM+4,4);assert_eq!(g.bus.read32(0x3f500000),Err(Fault::Unmapped));assert_eq!(g.get(0x3f80fffc),0x89abcdef);g.put(EXTMEM+4,0);
        assert_eq!(g.bus.fetch(0x3f500000),Err(Fault::Prohibited));assert_eq!(g.bus.read32(0x3ff80000),Err(Fault::Unmapped));assert_eq!(g.bus.write32(0x60004004,1),Err(Fault::Unmapped));
        g.bus.reboot([0;6]);g.program();assert_eq!(g.bus.psram.len(),8<<20);assert_eq!(g.get(EXTMEM),0);g.put(EXTMEM,1);g.put(EXTMEM+4,0);g.put(MMU+336*4,(1<<16)|127);assert_eq!(g.get(0x3f50fffc),0,"diagnostic restart clearsPSRAM; not a retention claim");
    }
    let mut b=S2Bus::new();for size in[1,2<<20,4<<20,16<<20] {assert!(b.set_psram_size(size).is_err());}b.set_psram_size(0).unwrap();
}
#[test]
fn s2_lcd_true_guest_software_spi_offsets_reset_light_and_wrong_mux() {
    let mut g=Guest::new(LCD);for(pin,v)in[(9,true),(10,false),(11,false),(18,false),(21,true),(45,true)]{g.output(pin,v);}
    for(c,data)in[(0x11,&[][..]),(0x29,&[][..]),(0x3a,&[5][..]),(0x36,&[0xa8][..])]{g.cmd(c,data);}
    g.pixel(0xf800);let(w,h,pixels,_)=g.bus.board.display().unwrap();assert_eq!((w,h),(160,80));assert_eq!(pixels[0],0xf800);assert_eq!(pixels.iter().filter(|&&v|v!=0).count(),1);
    g.mux(11,2);g.pixel(0x001f);assert_eq!(g.bus.board.display().unwrap().2,pixels);g.mux(11,1);
    g.put(GPIO+0x28,1<<10);g.pixel(0x001f);assert_eq!(g.bus.board.display().unwrap().2,pixels);g.put(GPIO+0x24,1<<10);
    g.level(45,false);assert!(g.bus.board.display().unwrap().2.iter().all(|&v|v==0));g.level(45,true);assert_eq!(g.bus.board.display().unwrap().2,pixels);
    g.cmd(0x3a,&[6]);g.pixel(0x001f);assert_eq!(g.bus.board.display().unwrap().2,pixels);g.cmd(0x3a,&[5]);g.pixel(0x07e0);assert_eq!(g.bus.board.display().unwrap().2[0],0x07e0);
    g.level(21,false);assert!(g.bus.board.display().unwrap().2.iter().all(|&v|v==0));g.level(21,true);assert!(g.bus.board.display().unwrap().2.iter().all(|&v|v==0));
    g.mux(0,1);assert_ne!(g.get(GPIO+0x3c)&1,0);g.bus.gpio_set_input(0,false);assert_eq!(g.get(GPIO+0x3c)&1,0);
    assert!(Guest::new(PICO).bus.board.display().is_none());
}


#[test]
fn s2_original_app_images_run_to_uart_frame_input_echo_and_reset() {
    for id in[PICO,LCD] {
        let bytes=if id==LCD {include_bytes!("fixtures/waveshare-lcd096-sram-diagnostic.bin").as_slice()}else{include_bytes!("fixtures/s2-sram-console.bin").as_slice()};
        let mut m=esp32s2::machine([0;6],4<<20);m.console.capture=true;m.console.mask=2;m.bus.set_psram_size(8<<20).unwrap();m.bus.attach_board(esp32s2::board::make_board(id).unwrap());
        m.bus.write_flash(0x10000,bytes).unwrap();assert_eq!(m.boot_app(0x10000).unwrap(),0x40021400);
        let run=|m:&mut esp32s2::Machine,cycles:u64| {m.max_cycles=m.bus.cycles()+cycles;let stop=m.run(u64::MAX);assert!(matches!(stop,esp_soc::Stop::MaxInsns|esp_soc::Stop::Halted),"{stop:?}");assert_eq!(m.exceptions,0);assert!(m.bus.first_fault.is_none());};
        for(pressed,color)in[(false,0xf800),(true,0x07e0),(false,0xf800)] {
            m.bus.gpio_set_input(0,!pressed);run(&mut m,if id==LCD{3_000_000}else{3000});
            assert_eq!(m.bus.read32(0x3ffff000),Ok(u32::from(!pressed)));
            let message=if id==LCD {if pressed {"FRAME GREEN\n"}else{"FRAME RED\n"}}else if pressed {"BUTTON DOWN\n"}else{"BUTTON UP\n"};
            assert!(m.console.uart0.ends_with(message.as_bytes()),"{}",String::from_utf8_lossy(&m.console.uart0));
            if id==LCD {let(w,h,pixels,_)=m.bus.board.display().unwrap();assert_eq!((w,h),(160,80));assert!(pixels.iter().all(|&p|p==color));}else{assert!(m.bus.board.display().is_none());}
        }
        m.bus.uart_input(0,b"S2-ECHO\n");run(&mut m,3000);assert!(m.console.uart0.ends_with(b"S2-ECHO\n"));
        m.bus.request_reset(3);m.reboot();run(&mut m,if id==LCD{3_000_000}else{3000});assert_eq!(m.bus.read32(0x3ffff000),Ok(1));assert_eq!(m.bus.psram.len(),8<<20);assert_eq!(m.reboots,1);
    }
}
