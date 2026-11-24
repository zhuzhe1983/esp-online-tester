use emu_core::Bus;
use esp32c5::C5Bus;
const LP:u32=0x600b_2800;
fn get(b:&mut C5Bus,o:u32)->u32{b.read32(LP+o).unwrap()}
fn put(b:&mut C5Bus,o:u32,v:u32){b.write32(LP+o,v).unwrap();}

#[test]
fn virtual_rng_has_explicit_reset_gate_config_and_reproducible_samples(){
 let mut b=C5Bus::new();assert_eq!(get(&mut b,0),0x7f00_0000);assert_eq!(get(&mut b,4),0);assert_eq!(get(&mut b,0x24),0xffe);assert_eq!(get(&mut b,8),0);
 put(&mut b,0x24,0xfff);let a=get(&mut b,8);let next=get(&mut b,0x28);assert_ne!(a,0);assert_ne!(a,next);assert_eq!(get(&mut b,0x24),0x0200_0fff);
 put(&mut b,0,0x7e00_0000);b.advance(1_000_000);assert_eq!(get(&mut b,8),next);assert_eq!(get(&mut b,0x24)>>24,2,"no pretend physical sample rate");
 put(&mut b,0,0x7f00_0000);put(&mut b,0x24,0xffe);assert_eq!(get(&mut b,8),next);
 put(&mut b,4,1<<24);put(&mut b,0x24,0xfff);assert_eq!(get(&mut b,8),0);assert_eq!(get(&mut b,0x24),0xffe);
 put(&mut b,4,0);put(&mut b,0x24,0xfff);assert_eq!(get(&mut b,8),a,"documented virtual seed, not physical entropy");
 put(&mut b,8,0);assert_ne!(get(&mut b,8),0,"read-only DATA cannot inject a guest-selected RNG word");
}

#[test]
fn unmodeled_lp_engines_and_bus_cpu_reset_are_not_generic_register_ram(){
 let mut b=C5Bus::new();assert!(b.read32(LP+0x10).is_err());assert!(b.write32(LP+4,1<<23).is_err());assert!(b.write32(LP+4,1<<31).is_err());
 put(&mut b,0x3fc,0x87654321);assert_eq!(get(&mut b,0x3fc),0x87654321);put(&mut b,0,0x12345678);assert_eq!(get(&mut b,0),0x12000000);
 assert!(b.read32(0x600b_2c00).is_err(),"LPPERI is not the analog register bus");
}
