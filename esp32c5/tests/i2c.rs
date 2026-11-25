use emu_core::Bus;
use esp32c5::C5Bus;
use esp_periph::i2c::I2cDevice;
use esp_soc::{BoardModel,SocBus};
use std::sync::{Arc,Mutex};
const I:u32=0x6000_4000;
#[derive(Default)]struct Seen{bytes:Vec<u8>,clocks:Vec<Option<u32>>,stops:usize,reject:Option<u8>}
struct Device(Arc<Mutex<Seen>>);
impl I2cDevice for Device{
 fn pins(&self)->Option<(u8,u8)>{Some((8,9))}
 fn start_address_with_clock(&mut self,c:u8,a:u8,_r:bool,hz:Option<u32>)->bool{if c!=a{return false;}self.0.lock().unwrap().clocks.push(hz);true}
 fn write(&mut self,v:u8)->bool{let mut seen=self.0.lock().unwrap();seen.bytes.push(v);seen.reject!=Some(v)}
 fn read(&mut self)->u8{0xa5}
 fn stop(&mut self){self.0.lock().unwrap().stops+=1;}
}
struct Probe(Arc<Mutex<Seen>>);
impl BoardModel for Probe{fn name(&self)->&'static str{"i2c-probe"}fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{vec![(0,0x20,Box::new(Device(self.0.clone())))]}}
fn put(b:&mut C5Bus,a:u32,v:u32){b.write32(a,v).unwrap();}
fn get(b:&mut C5Bus,a:u32)->u32{b.read32(a).unwrap()}
fn route(b:&mut C5Bus,sda:u32,scl:u32){
 for(p,s)in [(8,sda),(9,scl)]{put(b,0x6009_0000+4*p,0x1200);put(b,0x6009_1ad4+4*p,s);put(b,0x6009_12d4+4*s,0x100|p);}
}
fn configured()->(C5Bus,Arc<Mutex<Seen>>){
 let seen=Arc::new(Mutex::new(Seen::default()));let mut b=C5Bus::new();b.attach_board(Box::new(Probe(seen.clone())));
 put(&mut b,0x6009_6020,3);put(&mut b,0x6009_6020,1);put(&mut b,0x6009_6024,1<<22);route(&mut b,47,46);
 put(&mut b,I,59);put(&mut b,I+0x38,60);for o in [0x40,0x44,0x48,0x4c]{put(&mut b,I+o,0);}(b,seen)
}
fn send(b:&mut C5Bus,a:u8,read:bool){
 put(b,I+0x24,u32::MAX);put(b,I+0x18,0x408b|(3<<12));put(b,I+0x1c,u32::from((a<<1)|u8::from(read)));
 if !read{put(b,I+0x1c,0x95);}
 put(b,I+0x58,6<<11);put(b,I+0x5c,(1<<11)|(1<<8)|if read{1}else{2});
 if read{put(b,I+0x60,(3<<11)|(1<<10)|1);put(b,I+0x64,2<<11);}else{put(b,I+0x60,2<<11);}
 put(b,I+4,(1<<4)|(1<<5));
}
#[test]fn c5_pin_routes_byte_clock_ack_irq_and_stop_are_actual(){
 let(mut b,s)=configured();put(&mut b,I+0x28,(1<<7)|(1<<10));put(&mut b,0x6001_0000+56*4,17);
 send(&mut b,0x20,false);b.advance(12+5399);assert!(s.lock().unwrap().clocks.is_empty());b.advance(1);assert_eq!(s.lock().unwrap().clocks,[Some(400_000)]);
 assert_eq!(get(&mut b,I+8)&1,0,"C5 header ACK=0");b.advance(5400+12);assert_eq!(s.lock().unwrap().bytes,[0x95]);assert_eq!(s.lock().unwrap().stops,1);assert_eq!(b.irq_sources_of(0,1),[56]);
 put(&mut b,I+0x24,u32::MAX);assert!(b.irq_sources_of(0,1).is_empty());send(&mut b,0x20,true);b.advance(20_000);assert_eq!(get(&mut b,I+0x1c),0xa5);assert_eq!(s.lock().unwrap().stops,2);
 route(&mut b,46,45);send(&mut b,0x20,false);b.advance(20_000);assert_ne!(get(&mut b,I+0x20)&(1<<10),0,"C6 matrix numbering is not an alias");assert_eq!(get(&mut b,I+8)&1,1);
 assert_eq!(s.lock().unwrap().bytes,[0x95]);
}
#[test]fn c5_reset_gates_unknown_source_fifo_capacity_and_busy_routes(){
 let(mut b,s)=configured();assert_eq!(get(&mut b,I+0xf8),0x02401040);assert!(b.write32(I+0x100,0).is_err());assert!(b.write32(I+0x28,1).is_err());
 put(&mut b,0x6009_6024,(1<<22)|(1<<20));put(&mut b,I+0x1c,0x40);put(&mut b,I+0x58,6<<11);put(&mut b,I+0x5c,(1<<11)|1);put(&mut b,I+0x60,2<<11);assert!(b.write32(I+4,(1<<4)|(1<<5)).is_err());assert!(s.lock().unwrap().clocks.is_empty());
 put(&mut b,0x6009_6024,1<<22);send(&mut b,0x20,false);route(&mut b,46,45);put(&mut b,I+4,(1<<4)|(1<<5));b.advance(20_000);assert_eq!(s.lock().unwrap().bytes,[0x95],"busy START cannot replace accepted routes");
 put(&mut b,0x6009_6020,3);assert!(b.read32(I).is_err());put(&mut b,0x6009_6020,1);assert_eq!(get(&mut b,I+4),0x208);assert_eq!(get(&mut b,I+8),0xc000);
 for _ in 0..32{put(&mut b,I+0x1c,0x40);}assert!(b.write32(I+0x1c,0x41).is_err());
 put(&mut b,0x6009_6020,0);assert!(b.read32(I+0xf8).is_err());assert!(b.read32(0x600b_1800).is_err(),"LP I2C remains separate and unsupported");
}
// Fixed C5 IDF v5.5.3/v5.5.4 i2c_ll.h:74 and i2c_master.c:714/791-808.
// These exercise MMIO and device callbacks; they do not run the factory driver.
const MASTER_EVENTS:u32=0x5a8;
const UNRAISED_ERRORS:u32=(1<<5)|(1<<8);
fn write_program(b:&mut C5Bus,a:u8,bytes:&[u8],terminal:u32){
 put(b,I+0x24,u32::MAX);put(b,I+0x18,0x408b|(3<<12));put(b,I+0x1c,u32::from(a<<1));
 for &byte in bytes{put(b,I+0x1c,u32::from(byte));}
 put(b,I+0x58,6<<11);put(b,I+0x5c,(1<<11)|(1<<8)|(bytes.len()as u32+1));put(b,I+0x60,terminal<<11);
 put(b,I+4,(1<<4)|(1<<5));
}
#[test]fn c5_idf_master_event_enables_preserve_raw_status_and_selective_clear(){
 let(mut b,s)=configured();put(&mut b,0x6001_0000+56*4,17);put(&mut b,I+0x28,MASTER_EVENTS);
 assert_eq!(get(&mut b,I+0x28),MASTER_EVENTS);assert_eq!(get(&mut b,I+0x2c),0);
 for bit in [0,1,13,14,16,18]{assert!(b.write32(I+0x28,MASTER_EVENTS|(1<<bit)).is_err());assert_eq!(get(&mut b,I+0x28),MASTER_EVENTS);}
 put(&mut b,I+0xc,(1<<5)|17);b.advance(1_000_000);assert_eq!(get(&mut b,I+0x20)&MASTER_EVENTS,0,"enables and elapsed idle time do not create events");
 write_program(&mut b,0x20,&[0x00,0x01],2);b.advance(25_000);
 assert_eq!(s.lock().unwrap().bytes,[0x00,0x01]);assert_eq!(s.lock().unwrap().stops,1);
 assert_eq!(get(&mut b,I+0x20)&MASTER_EVENTS,1<<7);assert_eq!(get(&mut b,I+0x2c),1<<7);assert_eq!(b.irq_sources_of(0,1),[56]);
 put(&mut b,I+0x28,UNRAISED_ERRORS);assert_eq!(get(&mut b,I+0x2c),0);assert!(b.irq_sources_of(0,1).is_empty());
 put(&mut b,I+0x24,UNRAISED_ERRORS|(1<<10));assert_eq!(get(&mut b,I+0x20)&MASTER_EVENTS,1<<7,"clearing other causes retains completion");
 put(&mut b,I+0x28,MASTER_EVENTS);assert_eq!(b.irq_sources_of(0,1),[56]);assert_eq!(get(&mut b,I+0x24),0);
 put(&mut b,I+0x20,1<<7);assert_eq!(get(&mut b,I+0x20)&MASTER_EVENTS,0,"RAW is WTC in the fixed C5 header");assert_eq!(get(&mut b,I+0x2c),0);
 assert!(b.irq_sources_of(0,1).is_empty());put(&mut b,0x6009_6020,3);put(&mut b,0x6009_6020,1);assert_eq!(get(&mut b,I+0x28),0);
}
#[test]fn c5_idf_end_and_stop_are_distinct_device_driven_events(){
 let(mut b,s)=configured();put(&mut b,I+0x28,MASTER_EVENTS);write_program(&mut b,0x20,&[0x00,0x01],4);b.advance(25_000);
 assert_eq!(get(&mut b,I+0x20)&MASTER_EVENTS,1<<3);assert_eq!(get(&mut b,I+0x2c),1<<3);assert_eq!(s.lock().unwrap().bytes,[0x00,0x01]);assert_eq!(s.lock().unwrap().stops,0);
 put(&mut b,I+0x24,1<<3);put(&mut b,I+0x58,2<<11);put(&mut b,I+4,(1<<4)|(1<<5));b.advance(12);
 assert_eq!(get(&mut b,I+0x20)&MASTER_EVENTS,1<<7);assert_eq!(s.lock().unwrap().stops,1);put(&mut b,I+0x24,1<<7);assert_eq!(get(&mut b,I+0x2c),0);
}
#[test]fn c5_idf_address_and_data_nacks_never_synthesize_success_or_timeout(){
 let(mut b,s)=configured();put(&mut b,I+0x28,MASTER_EVENTS);
 write_program(&mut b,0x21,&[0x95,0x96],2);b.advance(25_000);
 assert_eq!(get(&mut b,I+0x20)&MASTER_EVENTS,1<<10);assert_eq!(get(&mut b,I+0x2c),1<<10);assert_eq!(get(&mut b,I+8)&17,1);
 assert!(s.lock().unwrap().clocks.is_empty());assert!(s.lock().unwrap().bytes.is_empty());assert_eq!(s.lock().unwrap().stops,0);
 assert_eq!(get(&mut b,I+0x60)&(1<<31),0,"NACK does not execute the programmed STOP");
 put(&mut b,I+0x24,1<<7);assert_eq!(get(&mut b,I+0x2c),1<<10);put(&mut b,I+0x24,1<<10);assert_eq!(get(&mut b,I+0x2c),0);
 s.lock().unwrap().reject=Some(0x95);write_program(&mut b,0x20,&[0x95,0x96],2);b.advance(25_000);
 assert_eq!(s.lock().unwrap().clocks,[Some(400_000)]);assert_eq!(s.lock().unwrap().bytes,[0x95]);assert_eq!(s.lock().unwrap().stops,0);
 assert_eq!(get(&mut b,I+0x20)&MASTER_EVENTS,1<<10);assert_eq!(get(&mut b,I+8)&17,1);assert_eq!(get(&mut b,I+0x60)&(1<<31),0);
 b.advance(1_000_000);assert_eq!(get(&mut b,I+0x20)&MASTER_EVENTS,1<<10,"an unmodeled stretch/collision cannot manufacture an error");
 put(&mut b,I+4,(1<<4)|(1<<10));s.lock().unwrap().reject=None;write_program(&mut b,0x20,&[0x96],2);b.advance(20_000);
 assert_eq!(get(&mut b,I+0x20)&MASTER_EVENTS,1<<7);assert_eq!(get(&mut b,I+8)&17,0);assert_eq!(s.lock().unwrap().bytes,[0x95,0x96]);assert_eq!(s.lock().unwrap().stops,1);
}
