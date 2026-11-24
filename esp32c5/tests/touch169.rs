//! Product-specific electrical boundaries; the separate SRAM guest verifies UI feedback.
use emu_core::Bus;
use esp32c5::C5Bus;
use esp_soc::SocBus;
const I:u32=0x6000_4000;const S:u32=0x6008_1000;
fn put(b:&mut C5Bus,a:u32,v:u32){b.write32(a,v).unwrap();}
fn get(b:&mut C5Bus,a:u32)->u32{b.read32(a).unwrap()}
fn pin(b:&mut C5Bus,n:u32,signal:u32){put(b,0x6009_0000+4*n,0x1a00);put(b,0x6009_1ad4+4*n,signal);put(b,0x6009_1038,1<<n);}
fn fixture()->C5Bus{
 let mut b=C5Bus::new();b.attach_board(esp32c5::board::make_board("waveshare-c5-touch169").unwrap());
 put(&mut b,0x6009_6020,3);put(&mut b,0x6009_6020,1);put(&mut b,0x6009_6024,1<<22);
 pin(&mut b,8,47);pin(&mut b,9,46);put(&mut b,0x6009_12d4+4*47,0x108);put(&mut b,0x6009_12d4+4*46,0x109);
 put(&mut b,I,59);put(&mut b,I+0x38,60);for o in[0x40,0x44,0x48,0x4c]{put(&mut b,I+o,0);}b
}
fn transfer(b:&mut C5Bus,address:u8,register:u8,write:Option<u8>,count:u32)->Result<Vec<u8>,()>{
 put(b,I+0x24,0x7ffff);put(b,I+0x18,0x408b|(3<<12));put(b,I+0x1c,u32::from(address)<<1);put(b,I+0x1c,register.into());
 put(b,I+0x58,6<<11);
 if let Some(value)=write{put(b,I+0x1c,value.into());put(b,I+0x5c,(1<<11)|(1<<8)|3);put(b,I+0x60,2<<11);}
 else{put(b,I+0x1c,(u32::from(address)<<1)|1);put(b,I+0x5c,(1<<11)|(1<<8)|2);put(b,I+0x60,6<<11);put(b,I+0x64,(1<<11)|(1<<8)|1);
  let mut cmd=0x68;if count>1{put(b,I+cmd,(3<<11)|(count-1));cmd+=4;}put(b,I+cmd,(3<<11)|(1<<10)|1);put(b,I+cmd+4,2<<11);}
 put(b,I+4,(1<<4)|(1<<5));b.advance(100_000);if get(b,I+0x20)&(1<<10)!=0{return Err(());}Ok((0..count).map(|_|get(b,I+0x1c)as u8).collect())
}
fn lcd(b:&mut C5Bus,command:u8,data:&[u8]){
 for(dc,bytes)in[(false,&[command][..]),(true,data)]{if bytes.is_empty(){continue;}put(b,if dc{0x6009_1008}else{0x6009_100c},1<<24);put(b,S+0x10,1<<27);put(b,S+0x1c,bytes.len()as u32*8-1);let mut word=0;for(n,v)in bytes.iter().enumerate(){word|=u32::from(*v)<<(8*n);}put(b,S+0x98,word);put(b,S,1<<24);b.advance(1000);}
}
#[test]fn touch169_uses_the_real_expander_reset_and_cst_irq_without_resetting_lcd(){
 let mut b=fixture();for(p,s)in[(23,56),(7,58),(27,61),(24,256),(26,256)]{pin(&mut b,p,s);}put(&mut b,0x6009_1008,1<<26);put(&mut b,S+0xc,1<<12);put(&mut b,S+0x20,0x3e);
 for(c,p)in[(0x11,&[][..]),(0x3a,&[0x55][..]),(0x21,&[][..]),(0x29,&[][..]),(0x2a,&[0,0,0,0][..]),(0x2b,&[0,20,0,20][..]),(0x2c,&[0xf8,0][..])]{lcd(&mut b,c,p);}
 assert_eq!(b.board.display().unwrap().2[0],0xf800);
 transfer(&mut b,0x20,3,Some(0xfc),0).unwrap();transfer(&mut b,0x20,1,Some(0xfd),0).unwrap();
 b.touch_input(80,110,true);assert_ne!(b.gpio_input()&(1<<5),0,"reset blocks the contact IRQ");assert!(transfer(&mut b,0x15,2,None,5).is_err());
 assert_eq!(b.board.display().unwrap().2[0],0xf800,"EX1 is touch reset, not LCD reset");
 transfer(&mut b,0x20,1,Some(0xff),0).unwrap();b.touch_input(80,110,true);assert_eq!(b.gpio_input()&(1<<5),0);
 assert_eq!(transfer(&mut b,0x15,2,None,5).unwrap(),[1,0,80,0,110]);assert_ne!(b.gpio_input()&(1<<5),0,"reading report releases pending digital IRQ");
 b.touch_input(239,279,false);assert_eq!(transfer(&mut b,0x15,2,None,5).unwrap(),[0,0,239,1,23]);
 assert!(transfer(&mut b,0x15,0xee,Some(1),0).is_err(),"unknown touch config does not ACK");
 pin(&mut b,23,256);lcd(&mut b,0x2c,&[0,31]);assert_eq!(b.board.display().unwrap().2[0],0xf800,"wrong LCD clock route leaves pixels unchanged");
 put(&mut b,0x6009_100c,1<<26);assert!(b.board.display().unwrap().2.iter().all(|p|*p==0),"real backlight net gates visible output");
 assert_eq!(b.board.named_pin("boot"),Some(28));assert_eq!(b.board.named_pin("pwr"),None);
}
#[test]fn touch169_fixed_bus_addresses_wrong_routes_and_peripheral_state_survive_controller_reset(){
 let mut b=fixture();assert_eq!(transfer(&mut b,0x6b,0,None,1).unwrap(),[5]);
 for address in[0x6a,0x55,0x14,0x38]{assert!(transfer(&mut b,address,0,None,1).is_err());}
 transfer(&mut b,0x20,3,Some(0xfc),0).unwrap();put(&mut b,0x6009_6020,3);put(&mut b,0x6009_6020,1);
 assert_eq!(transfer(&mut b,0x20,3,None,1).unwrap(),[0xfc],"controller reset does not power-cycle TCA9554");
 pin(&mut b,9,45);put(&mut b,0x6009_12d4+4*45,0x109);assert!(transfer(&mut b,0x20,3,None,1).is_err(),"C6 SCL signal is not accepted on C5");
}
