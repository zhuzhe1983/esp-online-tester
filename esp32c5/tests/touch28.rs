//! Physical reset, PWM and bus boundaries for the source-wired C5 Touch2.8.
use emu_core::Bus;
use esp32c5::C5Bus;
use esp_soc::SocBus;
const I:u32=0x6000_4000;const S:u32=0x6008_1000;
fn put(b:&mut C5Bus,a:u32,v:u32){b.write32(a,v).unwrap();}
fn get(b:&mut C5Bus,a:u32)->u32{b.read32(a).unwrap()}
fn pin(b:&mut C5Bus,n:u32,signal:u32){put(b,0x6009_0000+4*n,0x1a00);put(b,0x6009_1ad4+4*n,signal);put(b,0x6009_1038,1<<n);}
fn fixture()->C5Bus{
 let mut b=C5Bus::new();b.attach_board(esp32c5::board::make_board("waveshare-c5-touch28").unwrap());
 put(&mut b,0x6009_6020,3);put(&mut b,0x6009_6020,1);put(&mut b,0x6009_6024,1<<22);
 pin(&mut b,0,47);pin(&mut b,1,46);put(&mut b,0x6009_12d4+4*47,0x100);put(&mut b,0x6009_12d4+4*46,0x101);
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
 for(dc,bytes)in[(false,&[command][..]),(true,data)]{if bytes.is_empty(){continue;}put(b,if dc{0x6009_1008}else{0x6009_100c},1<<9);put(b,S+0x10,1<<27);put(b,S+0x1c,bytes.len()as u32*8-1);let mut word=0;for(n,v)in bytes.iter().enumerate(){word|=u32::from(*v)<<(8*n);}put(b,S+0x98,word);put(b,S,1<<24);b.advance(1000);}
}
#[test]fn touch28_expander_resets_touch_and_panel_independently_and_pwm_gates_light(){
 let mut b=fixture();for(p,s)in[(6,56),(7,58),(10,61),(9,256),(23,256)]{pin(&mut b,p,s);}put(&mut b,0x6009_1008,1<<23);put(&mut b,S+0xc,1<<12);put(&mut b,S+0x20,0x3e);
 assert!(transfer(&mut b,0x58,&[0xd0,7,0,0],9).is_err(),"touch held in reset");
 for bytes in[[2,3],[3,3],[5,255]]{transfer(&mut b,0x24,&bytes,0).unwrap();}
 for(c,p)in[(0x11,&[][..]),(0x3a,&[0x55][..]),(0x21,&[][..]),(0x29,&[][..]),(0x2a,&[0,0,0,0][..]),(0x2b,&[0,0,0,0][..]),(0x2c,&[0xf8,0][..])]{lcd(&mut b,c,p);}
 assert_eq!(b.board.display().unwrap().2[0],0xf800);
 transfer(&mut b,0x24,&[3,2],0).unwrap();b.touch_input(80,110,true);assert_ne!(b.gpio_input()&(1<<5),0);assert!(transfer(&mut b,0x58,&[0xd0,7,0,0],9).is_err());
 assert_eq!(b.board.display().unwrap().2[0],0xf800,"EX0 touch reset must not reset LCD on EX1");
 transfer(&mut b,0x24,&[3,3],0).unwrap();b.touch_input(200,300,true);assert_eq!(b.gpio_input()&(1<<5),0);
 let r=transfer(&mut b,0x58,&[0xd0,7,0,0],9).unwrap();assert_eq!((r[3],r[4],r[5],r[7],r[8]),(1,200,44,0x10,0x20));
 assert_eq!(b.gpio_input()&(1<<5),0,"CST report requires the real end-read command");transfer(&mut b,0x58,&[0xd0,0,2,0xab],0).unwrap();assert_ne!(b.gpio_input()&(1<<5),0);
 transfer(&mut b,0x24,&[5,0],0).unwrap();assert!(b.board.display().unwrap().2.iter().all(|p|*p==0));transfer(&mut b,0x24,&[5,255],0).unwrap();assert_eq!(b.board.display().unwrap().2[0],0xf800);
 transfer(&mut b,0x24,&[3,1],0).unwrap();assert!(b.board.display().unwrap().2.iter().all(|p|*p==0),"EX1 reset gates LCD");
 b.touch_input(80,110,true);assert!(transfer(&mut b,0x58,&[0xd0,7,0,0],9).is_ok(),"LCD reset does not reset touch");
 assert_eq!(b.board.named_pin("boot"),Some(28));assert_eq!(b.board.named_pin("pwr"),None);
}
#[test]fn touch28_does_not_alias_other_board_addresses_or_invent_adc_or_touch_identity(){
 let mut b=fixture();assert_eq!(transfer(&mut b,0x6b,&[0],1).unwrap(),[5]);
 for address in[0x6a,0x20,0x15,0x38,0x70]{assert!(transfer(&mut b,address,&[0],1).is_err());}
 for reg in[6,7,0xff]{assert!(transfer(&mut b,0x24,&[reg],1).is_err());}
 transfer(&mut b,0x24,&[2,3],0).unwrap();transfer(&mut b,0x24,&[3,3],0).unwrap();assert!(transfer(&mut b,0x58,&[0xd0,3,0,0],4).is_err());
 put(&mut b,0x6009_6020,3);put(&mut b,0x6009_6020,1);assert_eq!(transfer(&mut b,0x24,&[4],1).unwrap()[0]&3,3,"ESP I2C reset leaves board-side CH32 outputs");
 pin(&mut b,0,256);pin(&mut b,8,47);put(&mut b,0x6009_12d4+4*47,0x108);assert!(transfer(&mut b,0x24,&[4],1).is_err(),"the Touch169 SDA8 route is wrong for Touch2.8");
}
