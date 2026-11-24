//! Physical reset, PWM and bus boundaries for the source-wired C5 LCD2.73.
use emu_core::Bus;
use esp32c5::C5Bus;
use esp_soc::SocBus;
const I:u32=0x6000_4000;const S:u32=0x6008_1000;
fn put(b:&mut C5Bus,a:u32,v:u32){b.write32(a,v).unwrap();}
fn get(b:&mut C5Bus,a:u32)->u32{b.read32(a).unwrap()}
fn pin(b:&mut C5Bus,n:u32,signal:u32){put(b,0x6009_0000+4*n,0x1a00);put(b,0x6009_1ad4+4*n,signal);put(b,0x6009_1038,1<<n);}
fn fixture()->C5Bus{
 let mut b=C5Bus::new();b.attach_board(esp32c5::board::make_board("waveshare-c5-lcd273").unwrap());
 put(&mut b,0x6009_6020,3);put(&mut b,0x6009_6020,1);put(&mut b,0x6009_6024,1<<22);
 pin(&mut b,27,47);pin(&mut b,26,46);put(&mut b,0x6009_12d4+4*47,0x11b);put(&mut b,0x6009_12d4+4*46,0x11a);
 put(&mut b,I,59);put(&mut b,I+0x38,60);for o in[0x40,0x44,0x48,0x4c]{put(&mut b,I+o,0);}b
}
fn transfer(b:&mut C5Bus,address:u8,bytes:&[u8],count:u32)->Result<Vec<u8>,()>{
 put(b,I+0x24,0x7ffff);put(b,I+0x18,0x408b|(3<<12));put(b,I+0x1c,u32::from(address)<<1);for &byte in bytes{put(b,I+0x1c,byte.into());}
 put(b,I+0x58,6<<11);put(b,I+0x5c,(1<<11)|(1<<8)|(bytes.len()as u32+1));
 if count==0{put(b,I+0x60,2<<11);}
 else{put(b,I+0x1c,(u32::from(address)<<1)|1);put(b,I+0x60,6<<11);put(b,I+0x64,(1<<11)|(1<<8)|1);
  let mut cmd=0x68;if count>1{put(b,I+cmd,(3<<11)|(count-1));cmd+=4;}put(b,I+cmd,(3<<11)|(1<<10)|1);put(b,I+cmd+4,2<<11);}
 put(b,I+4,(1<<4)|(1<<5));b.advance(100_000);if get(b,I+0x20)&(1<<10)!=0{return Err(());}Ok((0..count).map(|_|get(b,I+0x1c)as u8).collect())
}
fn lcd(b:&mut C5Bus,command:u8,data:&[u8]){
 for(dc,bytes)in[(false,&[command][..]),(true,data)]{if bytes.is_empty(){continue;}put(b,if dc{0x6009_1008}else{0x6009_100c},1<<4);put(b,S+0x10,1<<27);put(b,S+0x1c,bytes.len()as u32*8-1);let mut word=0;for(n,v)in bytes.iter().enumerate(){word|=u32::from(*v)<<(8*n);}put(b,S+0x98,word);put(b,S,1<<24);b.advance(1000);}
}
fn initialize(b:&mut C5Bus){
 for(p,s)in[(6,56),(7,58),(8,61),(4,256),(9,256)]{pin(b,p,s);}put(b,0x6009_1008,1<<9);put(b,S+0xc,1<<12);put(b,S+0x20,0x3e);
 for bytes in[&[2,1,0][..],&[3,1,0][..],&[5,255][..]]{transfer(b,0x24,bytes,0).unwrap();}
 for(c,p)in[(0x11,&[][..]),(0x3a,&[0x66][..]),(0x36,&[0x48][..]),(0x21,&[][..]),(0x29,&[][..]),(0x2a,&[0,0,0,0][..]),(0x2b,&[0,0,0,0][..]),(0x2c,&[0xfc,0,0][..])]{lcd(b,c,p);}
}
#[test]fn lcd273_rgb666_three_gpio_keys_and_real_v006_reset_pwm(){
 let mut b=fixture();initialize(&mut b);assert_eq!(b.board.display().unwrap().2[0],0xf800);assert_eq!(b.board.display().unwrap().0,320);
 for(name,pin)in[("prev",2u8),("next",3),("boot",28)]{
  assert_eq!(b.board.named_pin(name),Some(pin));put(&mut b,0x6009_0000+4*u32::from(pin),0x1b00);
  assert_ne!(get(&mut b,0x6009_1064)&(1<<pin),0);b.gpio_set_input(pin,false);assert_eq!(get(&mut b,0x6009_1064)&(1<<pin),0);b.gpio_set_input(pin,true);
 }
 assert!(b.board.touch_irq().is_none());assert!(b.board.named_pin("pwr").is_none());
 transfer(&mut b,0x24,&[5,0],0).unwrap();assert!(b.board.display().unwrap().2.iter().all(|v|*v==0));transfer(&mut b,0x24,&[5,255],0).unwrap();assert_eq!(b.board.display().unwrap().2[0],0xf800);
 lcd(&mut b,0x3a,&[0x55]);lcd(&mut b,0x2c,&[0,0x1f]);assert_eq!(b.board.display().unwrap().2[0],0xf800,"incorrect16-bit pixel encoding is rejected");
 transfer(&mut b,0x24,&[3,0,0],0).unwrap();assert!(b.board.display().unwrap().2.iter().all(|v|*v==0));
 transfer(&mut b,0x24,&[3,1,0],0).unwrap();lcd(&mut b,0x11,&[]);lcd(&mut b,0x29,&[]);lcd(&mut b,0x36,&[0x48]);lcd(&mut b,0x21,&[]);
 assert!(b.board.display().unwrap().2.iter().all(|v|*v==0),"undefined reset RAM is the explicit zero virtual placeholder");
}
#[test]fn lcd273_does_not_inherit_touch_audio_pmic_or_analog_ack(){
 let mut b=fixture();assert_eq!(transfer(&mut b,0x6b,&[0],1).unwrap(),[5]);assert!(transfer(&mut b,0x51,&[0],1).is_ok());
 for a in[0x6a,0x20,0x15,0x38,0x58,0x18,0x34,0x70]{assert!(transfer(&mut b,a,&[0],1).is_err());}
 for reg in[6,7,255]{assert!(transfer(&mut b,0x24,&[reg],1).is_err());}
 transfer(&mut b,0x24,&[2,1,0x80],0).unwrap();transfer(&mut b,0x24,&[3,1,0x80],0).unwrap();assert_eq!(transfer(&mut b,0x24,&[4],2).unwrap()[1]&0x80,0x80);
 put(&mut b,0x6009_6020,3);put(&mut b,0x6009_6020,1);assert_eq!(transfer(&mut b,0x24,&[4],2).unwrap()[1]&0x80,0x80,"C5I2C controller reset does not erase board MCU state");
 pin(&mut b,27,256);pin(&mut b,0,47);put(&mut b,0x6009_12d4+4*47,0x100);assert!(transfer(&mut b,0x24,&[4],2).is_err());
}
