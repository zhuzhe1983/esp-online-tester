use emu_core::Bus;
use esp32c5::C5Bus;
const BASE:u32=0x600b_1c00;
#[test]fn exact_defaults_protection_and_write_triggers_are_not_register_ram(){
 let mut b=C5Bus::new();assert_eq!(b.read32(BASE),Ok(0x13214));assert_eq!(b.read32(BASE+4),Ok(200_000));assert_eq!(b.read32(BASE+8),Ok(80_000));assert_eq!(b.read32(BASE+0xc),Ok(4095));assert_eq!(b.read32(BASE+0x10),Ok(4095));
 assert_eq!(b.read32(BASE+0x1c),Ok(300<<20));assert_eq!(b.read32(BASE+0x18),Ok(0));assert_eq!(b.read32(BASE+0x20),Ok(0));
 b.write32(BASE+4,1).unwrap();assert_eq!(b.read32(BASE+4),Ok(200_000));b.write32(BASE+0x18,0x50d83aa1).unwrap();b.write32(BASE+4,123).unwrap();assert_eq!(b.read32(BASE+4),Ok(123));
 b.write32(BASE+0x1c,u32::MAX).unwrap();assert_eq!(b.read32(BASE+0x1c),Ok(300<<20),"RWDT key does not unlock SWD");
 b.write32(BASE+0x20,0x50d83aa1).unwrap();b.write32(BASE+0x1c,u32::MAX).unwrap();assert_eq!(b.read32(BASE+0x1c),Ok(0x7ff4_0000),"WT and RO/reserved bits do not latch");
 b.write32(BASE+0x14,u32::MAX).unwrap();assert_eq!(b.read32(BASE+0x14),Ok(0));assert!(b.read32(BASE+0x100).is_err());
}
#[test]fn unmodeled_active_watchdogs_and_interrupts_are_rejected_atomically(){
 let mut b=C5Bus::new();b.write32(BASE+0x18,0x50d83aa1).unwrap();assert!(b.write32(BASE,1<<31).is_err());assert_eq!(b.read32(BASE),Ok(0x13214));
 b.write32(BASE+0x20,0x50d83aa1).unwrap();assert!(b.write32(BASE+0x1c,(300<<20)|(1<<18)).is_err());assert_eq!(b.read32(BASE+0x1c),Ok(300<<20));
 b.write32(BASE+0x1c,(300<<20)|(1<<30)).unwrap();assert!(b.write32(BASE+0x1c,300<<20).is_err());assert_eq!(b.read32(BASE+0x1c),Ok((300<<20)|(1<<30)));
 assert!(b.write32(BASE+0x2c,1<<31).is_err());assert!(b.read32(BASE+0x24).is_err());
}
