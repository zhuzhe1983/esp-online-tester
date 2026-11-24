//! C5 physical addresses and hardware transactions, not host panel injection.
use emu_core::Bus;
use esp32c5::C5Bus;
use esp_soc::{BoardModel,SocBus,board::SpiQspiPins};
use std::{cell::RefCell,rc::Rc};
const SPI:u32=0x6008_1000;const DMA:u32=0x6008_0000;
fn put(b:&mut C5Bus,a:u32,v:u32){b.write32(a,v).unwrap();}
fn get(b:&mut C5Bus,a:u32)->u32{b.read32(a).unwrap()}
fn matrix(b:&mut C5Bus,p:u32,s:u32){put(b,0x6009_0000+4*p,0x1800);put(b,0x6009_1ad4+4*p,s);put(b,0x6009_1038,1<<p);}
type Calls=Rc<RefCell<Vec<(SpiQspiPins,Vec<u8>)>>>;
struct Capture(Calls);
impl BoardModel for Capture{
 fn name(&self)->&'static str{"c5-spi-contract-fixture"}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,tx:&[u8],n:usize)->Vec<u8>{self.0.borrow_mut().push((p,tx.to_vec()));(0..n).map(|i|0xa0+i as u8).collect()}
}
fn fixture()->(C5Bus,Calls){
 let calls=Rc::new(RefCell::new(Vec::new()));let mut b=C5Bus::new();b.attach_board(Box::new(Capture(calls.clone())));
 for(p,s)in[(7,56),(6,58),(23,61)]{matrix(&mut b,p,s);}
 put(&mut b,SPI+0x0c,1<<12);put(&mut b,SPI+0x10,1<<27);put(&mut b,SPI+0x98,0x04030201);put(&mut b,SPI+0x1c,31);(b,calls)
}
fn desc(b:&mut C5Bus,a:u32,buf:u32,n:u32,next:u32,eof:bool){for(o,v)in[(0,(1<<31)|u32::from(eof)<<30|(n<<12)|n),(4,buf),(8,next)]{put(b,a+o,v);}}
fn arm(b:&mut C5Bus,a:u32,peri:u32){put(b,DMA+0xd0,12);put(b,DMA+0xd4,1<<12);put(b,DMA+0x100,peri);put(b,DMA+0x3b8,a);put(b,DMA+0xe0,2);}

#[test]
fn c5_usr_snapshots_routes_and_finishes_after_real_selected_serial_clocks(){
 let(mut b,calls)=fixture();put(&mut b,SPI,1<<24);assert_ne!(get(&mut b,SPI)&(1<<24),0);
 matrix(&mut b,7,256);put(&mut b,SPI+0x10,1<<28);put(&mut b,SPI,1<<24); // busy USR does not replace accepted direction/routes
 b.advance(319);assert!(calls.borrow().is_empty());b.advance(1);
 let records=calls.borrow();assert_eq!(records.len(),1);let(p,tx)=&records[0];assert_eq!(tx,&[1,2,3,4]);assert_eq!((p.sclk,p.data[0],p.hardware_cs),(1<<7,1<<6,1<<23));assert_eq!((p.data_bits,p.read_bits),(32,0));
 assert_eq!(get(&mut b,SPI)&(1<<24),0);assert_ne!(get(&mut b,SPI+0x3c)&(1<<12),0);
 put(&mut b,SPI+0x3c,1<<12);assert_eq!(get(&mut b,SPI+0x3c),0,"C5 RAW W1C");
}

#[test]
fn unsupported_qpi_partial_byte_capacity_and_unknown_clock_are_not_silent_fallbacks(){
 let(mut b,calls)=fixture();
 for(user,bits)in[((1<<27)|8,31),(1<<27,30),(1<<27,519)]{
  put(&mut b,SPI+0x10,user);put(&mut b,SPI+0x1c,bits);assert!(b.write32(SPI,1<<24).is_err());
 }
 put(&mut b,SPI+0x10,1<<27);put(&mut b,SPI+0x1c,31);put(&mut b,0x6009_60c8,(1<<22)|(1<<20));
 assert!(b.write32(SPI,1<<24).is_err(),"uninitialized PLL160 cannot become XTAL40");
 put(&mut b,0x6009_60c8,1<<22);put(&mut b,SPI,1<<24);put(&mut b,0x6009_60c4,0);b.advance(1000);
 assert!(calls.borrow().is_empty(),"gate cancels pending byte-level work");assert!(b.read32(SPI).is_err());
}

#[test]
fn gdma_c5_uses_trigger_one_full_address_partial_descriptors_owner_and_bounds(){
 let(mut b,calls)=fixture();let descriptor=0x4080_1000;let buffer=0x4080_2000;
 put(&mut b,buffer,0x04030201);put(&mut b,buffer+4,0x08070605);desc(&mut b,descriptor,buffer,8,0,true);
 arm(&mut b,descriptor,0);put(&mut b,SPI+0x30,1<<28);put(&mut b,SPI,1<<24);b.advance(1000);
 assert!(calls.borrow().is_empty(),"C6 peripheral0 is not C5 SPI2");put(&mut b,DMA+0x100,1);b.advance(320);
 assert_eq!(calls.borrow()[0].1,[1,2,3,4]);assert_ne!(get(&mut b,descriptor)&(1<<31),0,"partial descriptor remains owned");
 put(&mut b,SPI,1<<24);b.advance(320);assert_eq!(calls.borrow()[1].1,[5,6,7,8]);assert_eq!(get(&mut b,descriptor)&(1<<31),0);assert_eq!(get(&mut b,DMA+0x414),descriptor);
 assert_eq!(get(&mut b,DMA+0x30)&0xf,0xb);put(&mut b,DMA+0x30,0xf);assert_eq!(get(&mut b,DMA+0x30),0);
 desc(&mut b,descriptor,buffer,8,0,true);put(&mut b,descriptor,8|(8<<12));arm(&mut b,descriptor,1);put(&mut b,SPI,1<<24);b.advance(320);
 assert_ne!(get(&mut b,DMA+0x30)&4,0);assert_eq!(calls.borrow().len(),2,"bad owner never creates a false board frame");
 desc(&mut b,descriptor,buffer,8,0,true);put(&mut b,DMA+0x3c4,descriptor);put(&mut b,DMA+0x3c8,descriptor+11);arm(&mut b,descriptor,1);put(&mut b,SPI,1<<24);b.advance(320);assert_eq!(calls.borrow().len(),2);
 assert!(b.first_fault.is_none(),"DMA faults do not invent a CPU load/store exception");
 assert!(b.write32(DMA+0x2f0,1).is_err(),"no fourth C6-style channel alias");
}

#[test]
fn gdma_receive_uses_c5_full_link_address_and_writes_actual_miso(){
 let(mut b,calls)=fixture();let d=0x4083_0100;let buf=0x4083_1000;desc(&mut b,d,buf,16,0,false);
 put(&mut b,DMA+0x70,0);put(&mut b,DMA+0x74,1<<12);put(&mut b,DMA+0xa0,1);put(&mut b,DMA+0x3ac,d);put(&mut b,DMA+0x80,5);
 put(&mut b,SPI+0x30,1<<27);put(&mut b,SPI+0x10,1<<28);put(&mut b,SPI,1<<24);b.advance(320);
 assert_eq!(get(&mut b,buf),0xa3a2a1a0);let control=get(&mut b,d);assert_eq!(control>>30,1);assert_eq!((control>>12)&0xfff,4);
 assert_eq!(get(&mut b,DMA)&3,3);assert_eq!(get(&mut b,DMA+0x410),d);assert_eq!(calls.borrow().len(),1);
}

#[test]
fn c5_ledc_48mhz_wrap_controls_real_backlight_and_reset_gates_it(){
 let mut b=C5Bus::new();b.attach_board(esp32c5::board::make_board("waveshare-c5-lcd147").unwrap());matrix(&mut b,10,0);
 for(a,v)in[(0x6009_6048,1),(0x6009_6050,2),(0x6009_604c,1<<22),(0x6000_70a0,10|(256<<5)|(1<<26)),(0x6000_7008,512<<4),(0x6000_700c,1<<31),(0x6000_7000,20),(0x6000_70c8,1<<4)]{put(&mut b,a,v);}
 b.advance(5120);assert_eq!(get(&mut b,0x6000_7010),0);b.advance(1);assert_eq!(get(&mut b,0x6000_7010),512<<4);assert!(b.board.report().contains("duty=32768"));
 assert_ne!(get(&mut b,0x6001_0154)&(1<<(49-32)),0);
 put(&mut b,0x6009_604c,(1<<22)|(1<<20));assert!(b.board.report().contains("duty=0"),"unknown RC_FAST does not inherit40MHz");
 put(&mut b,0x6009_6048,3);assert!(b.read32(0x6000_7010).is_err());put(&mut b,0x6009_6048,1);assert_eq!(get(&mut b,0x6000_7010),0);
}

#[test]
fn public_sd_loading_delegates_only_to_the_real_board_socket(){
 let image=vec![0u8;8<<20];let mut m=esp32c5::machine([0;6],4<<20);
 assert!(m.load_input(esp_soc::LoadKind::SdCard,&image).is_err());m.bus.attach_board(esp32c5::board::make_board("waveshare-c5-lcd147").unwrap());
 m.load_input(esp_soc::LoadKind::SdCard,&image).unwrap();assert!(m.bus.board.report().contains("SD commands=0"));
 assert_eq!(m.bus.board.named_pin("boot"),Some(28));assert_ne!(m.bus.gpio_input()&(1<<28),0);
}

#[test]
fn gdma_reaches_guest_mapped_quad_psram_without_an_sram_alias(){
 let(mut b,calls)=fixture();b.set_psram_size(8<<20).unwrap();
 for p in[15,17,18,20,21,22]{put(&mut b,0x6009_0000+4*p,0x800);}
 for(a,v)in[(0x6000_3034,1),(0x6000_3008,0),(0x6000_3018,1<<31),(0x6000_3020,(7<<28)|0x35),(0x6000_3000,1<<18),
  (0x6000_2040,(1<<20)|(23<<14)|(5<<6)|0x34),(0x6000_2048,(7<<28)|0xeb),(0x6000_204c,(7<<28)|0x38),
  (0x6000_203c,0),(0x6000_2384,4),(0x6000_2380,0),(0x6000_237c,0x600)]{put(&mut b,a,v);}
 let d=0x4200_0100;let buffer=0x4200_1000;put(&mut b,buffer,0x44332211);put(&mut b,buffer+4,0x88776655);desc(&mut b,d,buffer,8,0,true);
 put(&mut b,0x600c_8004,3); // CPU instruction/data bus masks do not gate the independent GDMA path.
 arm(&mut b,d,1);put(&mut b,SPI+0x1c,63);put(&mut b,SPI+0x30,1<<28);put(&mut b,SPI,1<<24);b.advance(640);
 put(&mut b,0x600c_8004,0);
 assert_eq!(calls.borrow()[0].1,[0x11,0x22,0x33,0x44,0x55,0x66,0x77,0x88]);assert_eq!(get(&mut b,d)&(1<<31),0);assert_eq!(b.psram[0x100+3]&0x80,0);
 desc(&mut b,d,buffer,8,0,false);for(a,v)in[(DMA+0x70,0),(DMA+0x74,1<<12),(DMA+0xa0,1),(DMA+0x3ac,d),(DMA+0x80,5),(SPI+0x30,1<<27),(SPI+0x10,1<<28),(SPI,1<<24)]{put(&mut b,a,v);}b.advance(640);
 assert_eq!(&b.psram[0x1000..0x1008],&[0xa0,0xa1,0xa2,0xa3,0xa4,0xa5,0xa6,0xa7]);assert_eq!(get(&mut b,DMA)&3,3);
 // The same high address must stop working when its MMU entry is invalid.
 put(&mut b,0x6000_237c,0);arm(&mut b,d,1);put(&mut b,SPI+0x30,1<<28);put(&mut b,SPI+0x10,1<<27);put(&mut b,SPI,1<<24);b.advance(640);
 assert_eq!(calls.borrow().len(),2);assert_ne!(get(&mut b,DMA+0x30)&4,0);assert!(b.first_fault.is_none());
}

#[test]
fn lcd_requires_its_actual_clock_reset_dc_and_backlight_nets(){
 fn bytes(b:&mut C5Bus,dc:bool,data:&[u8]){put(b,0x6009_1000+if dc{8}else{12},1<<24);put(b,SPI+0x10,1<<27);put(b,SPI+0x1c,data.len() as u32*8-1);let mut w=[0;4];w[..data.len()].copy_from_slice(data);put(b,SPI+0x98,u32::from_le_bytes(w));put(b,SPI,1<<24);b.advance(1000);}
 fn command(b:&mut C5Bus,c:u8,args:&[u8]){bytes(b,false,&[c]);if !args.is_empty(){bytes(b,true,args);}}
 let mut b=C5Bus::new();b.attach_board(esp32c5::board::make_board("waveshare-c5-lcd147").unwrap());
 for(p,s)in[(7,56),(6,58),(23,61),(24,256),(26,256),(10,256)]{matrix(&mut b,p,s);}put(&mut b,0x6009_1008,(1<<26)|(1<<10));
 command(&mut b,0x11,&[]);command(&mut b,0x3a,&[0x55]);command(&mut b,0x21,&[]);command(&mut b,0x29,&[]);
 command(&mut b,0x2a,&[0,34,0,34]);command(&mut b,0x2b,&[0,0,0,0]);command(&mut b,0x2c,&[0xf8,0]);
 assert_eq!(b.board.display().unwrap().2[0],0xf800);
 matrix(&mut b,7,256);command(&mut b,0x2c,&[0x07,0xe0]);assert_eq!(b.board.display().unwrap().2[0],0xf800,"GPIO latch is not SPI clock");
 matrix(&mut b,7,56);put(&mut b,0x6009_100c,1<<10);assert_eq!(b.board.display().unwrap().2[0],0,"physical backlight off");
 put(&mut b,0x6009_1008,1<<10);put(&mut b,0x6009_100c,1<<26);command(&mut b,0x2c,&[0x07,0xe0]);assert_eq!(b.board.display().unwrap().2[0],0);
 put(&mut b,0x6009_1008,1<<26);assert_eq!(b.board.display().unwrap().2[0],0,"reset cleared sleep/on/format state");
}
