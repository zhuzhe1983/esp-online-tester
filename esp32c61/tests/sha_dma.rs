use emu_core::Bus;
use esp32c61::bus::{SocBus,MMU_VALID};
fn start(b:&mut SocBus,descriptor:u32) {
    for (a,v) in [(0x60080100,7),(0x600800d0,4),(0x600800d4,1<<12),(0x600803b8,descriptor),(0x600800e0,2),(0x60089000,2),(0x6008900c,1),(0x6008901c,1)] {b.write32(a,v).unwrap();}
}
fn descriptor(b:&mut SocBus,address:u32,buffer:u32) {
    for (offset,v) in [(0,3<<30|64<<12|64),(4,buffer),(8,0)] {b.write32(address+offset,v).unwrap();}
}
#[test]
fn sha_dma_reads_psram_pages_and_writes_back_external_descriptor() {
    let mut b=SocBus::new(1<<20,[0;6]);b.psram.resize(3<<16,0);
    // Adjacent virtual pages deliberately map in reverse physical order.
    b.mmu[256]=MMU_VALID|1<<9|2;b.mmu[257]=MMU_VALID|1<<9;
    let (desc,buffer)=(0x43000000,0x4300ffe0);descriptor(&mut b,desc,buffer);
    let mut padded=[0u8;64];padded[..3].copy_from_slice(b"abc");padded[3]=0x80;padded[63]=24;
    for (i,v) in padded.iter().enumerate() {b.write8(buffer+i as u32,*v).unwrap();}
    start(&mut b,desc);
    assert_eq!(b.periph.sha.h[..8],[0xba7816bf,0x8f01cfea,0x414140de,0x5dae2223,0xb00361a3,0x96177a9c,0xb410ff61,0xf20015ad]);
    assert_eq!(b.periph.sha.blocks,1);assert_eq!(b.read32(desc).unwrap()>>31,0);assert_eq!(b.periph.gdma.gdma.out[0].int_raw&7,3);
}
#[test]
fn sha_dma_rejects_flash_invalid_pages_and_mmio_without_hashing() {
    for mode in 0..3 {
        let mut b=SocBus::new(1<<20,[0;6]);b.psram.resize(1<<16,0);b.mmu[256]=if mode==0 {MMU_VALID}else{1<<9};
        let buffer=if mode==2 {0x60091000}else{0x43000000};descriptor(&mut b,0x40801000,buffer);start(&mut b,0x40801000);
        assert_eq!(b.periph.sha.blocks,0);assert_ne!(b.periph.gdma.gdma.out[0].int_raw&4,0);assert!(!b.periph.gdma.gdma.out[0].running);
    }
}
