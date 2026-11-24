//! Manufacturer C3 routing through actual MMIO, not direct board byte injection.
use emu_core::Bus;
use esp32c3::bus::{SocBus, DRAM_LOW as RAM};
use esp_soc::{BoardModel,board::SpiQspiPins};
const SPI:u32=0x6002_4000;
fn mux(b:&mut SocBus,p:u8,s:u32){b.write32(0x6000_9004+4*u32::from(p),1<<12|1<<9).unwrap();b.write32(0x6000_4554+4*u32::from(p),s).unwrap();}
fn gpio(b:&mut SocBus,p:u8,v:bool){b.write32(0x6000_4024,1<<p).unwrap();b.write32(if v{0x6000_4008}else{0x6000_400c},1<<p).unwrap();}
fn exp(b:&mut SocBus,reg:u8,value:u8){let mut ds=b.board.i2c_devices();let d=&mut ds.iter_mut().find(|(_,a,_)|*a==0x24).unwrap().2;assert!(d.start_address_with_clock(0x24,0x24,false,None));assert!(d.write(reg));assert!(d.write(value));d.stop();b.board.advance_to(b.cycles);}
fn byte(b:&mut SocBus,dc:bool,v:u8){gpio(b,8,dc);for(o,v)in[(0x10,1<<27),(0x1c,7),(0x98,u32::from(v)),(0x20,0x3f),(0,1<<24)]{b.write32(SPI+o,v).unwrap();}}
fn cmd(b:&mut SocBus,c:u8,v:&[u8]){byte(b,false,c);for x in v{byte(b,true,*x);}}
#[test]
fn waveshare_c3_mmio_routes_expander_selected_lcd_and_rejects_wrong_mosi(){
 let mut b=SocBus::new(4<<20,[0;6]);b.board=esp32c3::board::make_board("waveshare-c3-lcd147").unwrap();b.attach_board_devices();for(p,s)in[(7,63),(5,65),(8,128)]{mux(&mut b,p,s);}
 exp(&mut b,3,6);exp(&mut b,5,255);cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x36,&[0x40]);cmd(&mut b,0x2a,&[0,34,0,34]);cmd(&mut b,0x2b,&[0,0,0,0]);cmd(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.board.display().unwrap().2[0],0xf800);
 mux(&mut b,5,103);cmd(&mut b,0x2c,&[0,31]);assert_eq!(b.board.display().unwrap().2[0],0xf800,"S3 MOSI signal cannot clock C3 glass");
 mux(&mut b,5,65);cmd(&mut b,0x36,&[0]);cmd(&mut b,0x2c,&[0,31]);assert_eq!(b.board.display().unwrap().2[171],31,"guest MX remains meaningful relative to sourced glass reference");exp(&mut b,3,7);cmd(&mut b,0x2c,&[0,31]);assert_eq!(b.board.display().unwrap().2[0],0xf800,"hardware CS disable bits cannot replace CH32 EX0 selection");
}
struct Receiver;
impl BoardModel for Receiver{
 fn name(&self)->&'static str{"phase-receiver"}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,h:u8,p:SpiQspiPins,tx:&[u8],n:usize)->Vec<u8>{assert_eq!(h,2);assert_eq!(p.input[1],Some(6));assert_eq!(p.sclk&(1<<7),1<<7);assert_eq!(p.read_bits,32);assert!(tx.is_empty());assert_eq!(n,4);vec![0x12,0x34,0x56,0x78]}
}
#[test]
fn waveshare_c3_phase_rx_reaches_gdma_before_done_and_short_chain_reports_error(){
 for size in[4,2]{let mut b=SocBus::new(4<<20,[0;6]);b.board=Box::new(Receiver);b.attach_board_devices();mux(&mut b,7,63);mux(&mut b,6,128);b.write32(0x6000_4154+64*4,6|1<<6).unwrap();
  for(a,v)in[(RAM,1<<31|size),(RAM+4,RAM+0x100),(RAM+8,0),(0x6003_f0a0,0),(0x6003_f080,1<<22|RAM&0xfffff),(SPI+0x30,1<<27),(SPI+0x10,1<<28),(SPI+0x1c,31),(SPI,1<<24)]{b.write32(a,v).unwrap();}
  assert_eq!(b.read8(RAM+0x100).unwrap(),0x12);assert_eq!(b.read8(RAM+0x101).unwrap(),0x34);assert_ne!(b.read32(SPI+0x3c).unwrap()&(1<<12),0);
  if size==4{assert_eq!(b.read32(RAM+0x100).unwrap(),0x78563412);assert_eq!(b.periph.gdma.gdma.inp[0].int_raw&3,3);}else{assert_eq!(b.read8(RAM+0x102).unwrap(),0);assert_ne!(b.periph.gdma.gdma.inp[0].int_raw&(1<<4),0);}
 }
}
