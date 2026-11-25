use emu_core::Bus;
use esp32c5::C5Bus;
use esp_soc::SocBus;
const CACHE:u32=0x600c_8000;const MAPPED:u32=0x4200_0000;
fn put(b:&mut C5Bus,a:u32,v:u32){b.write32(a,v).unwrap();}
fn get(b:&mut C5Bus,a:u32)->u32{b.read32(a).unwrap()}
fn flash_map()->C5Bus{
 let mut b=C5Bus::new();b.flash=vec![255;65536];b.flash[..4].copy_from_slice(&0x0250_0513u32.to_le_bytes());
 put(&mut b,0x6000_203c,0);put(&mut b,0x6000_2384,4);put(&mut b,0x6000_2380,0);put(&mut b,0x6000_237c,0x400);b
}
#[test]fn c5_cache_mask_controls_instruction_and_data_access_independently(){
 let mut b=flash_map();assert_eq!(get(&mut b,CACHE+4),0);assert_eq!(get(&mut b,MAPPED),0x0250_0513);assert_eq!(b.fetch(MAPPED).unwrap(),0x0250_0513u32.to_le_bytes());
 put(&mut b,CACHE+4,1);assert!(b.fetch(MAPPED).is_err());assert_eq!(get(&mut b,MAPPED),0x0250_0513);
 put(&mut b,CACHE+4,2);assert!(b.read32(MAPPED).is_err());assert_eq!(b.fetch(MAPPED).unwrap(),0x0250_0513u32.to_le_bytes());
 put(&mut b,CACHE+4,3);assert!(b.fetch(MAPPED).is_err());assert!(b.read8(MAPPED).is_err());
 put(&mut b,0x4080_0000,0x0000_0013);assert_eq!(b.fetch(0x4080_0000).unwrap(),[0x13,0,0,0]);assert_eq!(get(&mut b,0x4080_0000),0x13,"internal SRAM remains accessible");
 put(&mut b,CACHE+4,0);put(&mut b,0x6000_237c,0);assert!(b.read32(MAPPED).is_err(),"unmasking cannot create an MMU mapping");
 b.reboot([0;6]);assert_eq!(get(&mut b,CACHE+4),0);assert!(b.fetch(MAPPED).is_err(),"reset does not restore MMU setup");
}
#[test]fn c5_cache_geometry_is_read_only_and_absent_engines_do_not_fake_completion(){
 let mut b=C5Bus::new();for(off,value)in[(0x10,16),(0x14,8),(0x18,128),(0x1c,4)]{assert_eq!(get(&mut b,CACHE+off),value);put(&mut b,CACHE+off,u32::MAX);assert_eq!(get(&mut b,CACHE+off),value);}
 assert_eq!(get(&mut b,CACHE+0x3fc),36_773_968);put(&mut b,CACHE+0x3fc,u32::MAX);assert_eq!(get(&mut b,CACHE+0x3fc),0x0fff_ffff);
 for off in[0x84,0x234]{assert!(b.read32(CACHE+off).is_err());assert!(b.write32(CACHE+off,1).is_err());}
 assert_eq!(get(&mut b,CACHE+0x94),16,"POR invalidation applies to the actual empty coherent cache");
 assert_eq!(get(&mut b,CACHE+0xd4),2);assert_eq!(get(&mut b,CACHE+0x130),2);
 assert!(b.write32(CACHE+0xd4,1).is_err());assert!(b.write32(CACHE+0x130,1).is_err(),"unsupported fill engines cannot manufacture completion");
 assert!(b.write32(CACHE,4).is_err(),"reserved ICACHE shut fields are not invented bus aliases");
}
#[test]fn c5_cache_trace_configuration_has_reserved_boundaries_and_no_synthetic_events(){
 let mut b=C5Bus::new();for off in[0x3d0,0x168,0x170,0x174]{assert_eq!(get(&mut b,CACHE+off),0);}
 put(&mut b,CACHE+0x3d0,1);assert_eq!(get(&mut b,CACHE+0x3d0),1);assert!(b.write32(CACHE+0x3d0,2).is_err());assert_eq!(get(&mut b,CACHE+0x3d0),1);
 put(&mut b,CACHE+0x168,31);assert!(b.read32(CACHE+0x168).is_err(),"whole-word reserved ENA readback remains unknown");
 assert_eq!(get(&mut b,CACHE+0x170),0);assert_eq!(get(&mut b,CACHE+0x174),0);assert!(b.write32(CACHE+0x170,16).is_err(),"RAW cannot fabricate a software-set cache failure");
 put(&mut b,CACHE+0x16c,31);assert_eq!(get(&mut b,CACHE+0x170),0);put(&mut b,0x6009_6104,7);put(&mut b,0x6009_6104,5);
 assert_eq!(get(&mut b,CACHE+0x3d0),0);assert_eq!(get(&mut b,CACHE+0x168),0);
}
#[test]fn c5_mode0_freeze_preserves_actual_backing_reads_and_cpu_results_without_fills(){
 let mut m=esp32c5::Bringup::new();m.bus=flash_map();m.cpu.pc=MAPPED;
 assert_eq!(m.run(1).stop,esp32c5::Stop::Budget);assert_eq!(m.cpu.x[10],37);
 assert_eq!(get(&mut m.bus,CACHE+0x28),0);put(&mut m.bus,CACHE+0x28,1<<16);assert_eq!(get(&mut m.bus,CACHE+0x28),0x50000);
 m.cpu.pc=MAPPED;m.cpu.x[10]=99;assert_eq!(m.run(1).stop,esp32c5::Stop::Budget);assert_eq!(m.cpu.x[10],37);
 // Real SPI1 program after freeze: clear the immediate, preserving the
 // instruction. Coherent passthrough has no cache lines to retain stale bytes.
 let spi=0x6000_3000;put(&mut m.bus,spi+4,4<<24);put(&mut m.bus,spi+0x58,0x0000_0513);
 put(&mut m.bus,spi,1<<30);put(&mut m.bus,spi,1<<25);
 assert_eq!(&m.bus.flash[..4],&0x0000_0513u32.to_le_bytes());assert_eq!(get(&mut m.bus,MAPPED),0x0000_0513);
 m.cpu.pc=MAPPED;m.cpu.x[10]=99;assert_eq!(m.run(1).stop,esp32c5::Stop::Budget);assert_eq!(m.cpu.x[10],0);
 assert_eq!(get(&mut m.bus,CACHE+0x28),0x50000);assert_eq!(get(&mut m.bus,CACHE+0xd4),2);assert_eq!(get(&mut m.bus,CACHE+0x130),2);
 assert!(m.bus.write32(CACHE+0x28,0x30000).is_err());assert_eq!(get(&mut m.bus,CACHE+0x28),0x50000);
 put(&mut m.bus,CACHE+4,1);assert!(m.bus.fetch(MAPPED).is_err());assert_eq!(get(&mut m.bus,MAPPED),0x0000_0513);put(&mut m.bus,CACHE+4,0);
 put(&mut m.bus,CACHE+0x28,1<<18);assert_eq!(get(&mut m.bus,CACHE+0x28),0);
 put(&mut m.bus,CACHE+0x28,1<<16);m.bus.reboot([0;6]);assert_eq!(get(&mut m.bus,CACHE+0x28),0);assert_eq!(&m.bus.flash[..4],&0x0000_0513u32.to_le_bytes());
}
