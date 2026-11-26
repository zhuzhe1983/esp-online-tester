//! U213 original RISC-V guest: exact P4 native RMII/matrix SMI/clock/reset,
//! real enhanced descriptors in L2, frame buffers in MMU-backed PSRAM.
use emu_core::Bus;
use esp_soc::SocBus as _;
const ENTRY:u32=0x4ff1_0000;
const RESULT:u32=0x4ff0_2000;
const TXDESC:u32=0x4ff0_3000;
const RXDESC:u32=TXDESC+0x40;
const TXBUF:u32=0x4801_0000;
const RXBUF:u32=0x4802_0000;
const MAC:u32=0x5009_8000;
const DMA:u32=MAC+0x1000;
fn li(c:&mut Vec<u32>,r:u32,v:u32){c.push((v.wrapping_add(0x800)&0xffff_f000)|(r<<7)|0x37);c.push(((v&0xfff)<<20)|(r<<15)|(r<<7)|0x13);}
fn write(c:&mut Vec<u32>,a:u32,v:u32){li(c,1,a);li(c,2,v);c.push(2<<20|1<<15|2<<12|0x23);}
fn copy(c:&mut Vec<u32>,a:u32,b:u32){li(c,1,a);c.push(1<<15|2<<12|2<<7|3);li(c,1,b);c.push(2<<20|1<<15|2<<12|0x23);}
fn words(c:&mut Vec<u32>,a:u32,w:&[u32]){for(i,&v)in w.iter().enumerate(){write(c,a+4*i as u32,v);}}
fn run(m:&mut esp32p4::Machine,mut c:Vec<u32>){c.push(0x0000_006f);m.bus.load_bytes(ENTRY,&c.into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>()).unwrap();m.cores[0].pc=ENTRY;m.run(100_000);assert_eq!(m.exceptions,0);}
fn setup(c:&mut Vec<u32>){
    write(c,0x500e_6018,1<<13);write(c,0x500e_6030,1<<27|1<<29);write(c,0x500e_6034,1|1<<9|1<<10);write(c,0x5011_1040,1<<15);write(c,0x500e_514c,4<<2);
    for pin in [49,34,35]{write(c,0x500e_1004+4*pin,3<<12);}
    for pin in [29,30,28,50]{write(c,0x500e_1004+4*pin,3<<12|1<<9);}
    for(pin,signal)in[(31,108),(52,109)]{write(c,0x500e_1004+4*pin,1<<12|1<<9);write(c,0x500e_0558+4*pin,signal);}write(c,0x500e_0158+4*107,0x80|52);
    write(c,0x500e_1004+4*51,1<<12|1<<9);write(c,0x500e_0558+4*51,256);write(c,0x500e_0030,1<<19);write(c,0x500e_0014,1<<19);
    for page in [1,2]{write(c,0x5008_e380,page);write(c,0x5008_e37c,0xc00|page);}
}
fn mdio(c:&mut Vec<u32>,reg:u32,phy:u32){write(c,MAC+0x10,1|reg<<6|phy<<11);li(c,1,MAC+0x10);c.extend([0x0000_a183,0x0011_f193,0xfe01_9ce3]);copy(c,MAC+0x14,RESULT);}
fn frame()->Vec<u8>{let mut f=vec![0;60];f[..6].fill(255);f[6..12].copy_from_slice(&[2,3,4,5,6,7]);f[12..14].copy_from_slice(&[8,0]);for(i,b)in f[14..].iter_mut().enumerate(){*b=(i as u8)^0x5a;}f}
fn transmit(c:&mut Vec<u32>,buffer:u32){words(c,TXDESC,&[1<<31|1<<28|1<<29|1<<30|1<<20,60,buffer,TXDESC,0,0,0,0]);write(c,DMA,1<<7);write(c,DMA+0x10,TXDESC);write(c,DMA+0x1c,1<<16|1<<15|1<<13|1|1<<6);write(c,MAC,12);write(c,DMA+0x18,1<<13|2);write(c,DMA+4,1);}
fn machine()->esp32p4::Machine{let mut m=esp32p4::machine([0;6],16<<20);m.bus.psram=vec![0;32<<20];m.bus.board=esp32p4::board::make_board("m5stack-poe-p4").unwrap();m.bus.attach_board_devices();m.boot_rom();m.bus.set_ethernet_relay(true).unwrap();m}
#[test]
fn p4_guest_mdio_enhanced_dma_and_host_frames_use_source_correct_u213_routes(){
    let mut m=machine();let mut c=Vec::new();setup(&mut c);mdio(&mut c,2,1);run(&mut m,c);assert_eq!(m.bus.read32(RESULT).unwrap(),0x0243);
    let f=frame();let mut c=Vec::new();words(&mut c,TXBUF,&f.as_chunks::<4>().0.iter().map(|b|u32::from_le_bytes(*b)).collect::<Vec<_>>());transmit(&mut c,TXBUF);run(&mut m,c);assert_eq!(m.bus.take_ethernet_frames(),vec![f.clone()]);assert_eq!(m.bus.read32(TXDESC).unwrap()>>31,0);assert_ne!(m.bus.read32(DMA+0x14).unwrap()&1,0);
    let mut c=Vec::new();words(&mut c,RXDESC,&[1<<31,1<<14|1536,RXBUF,RXDESC,0,0,0,0]);write(&mut c,DMA+0x0c,RXDESC);write(&mut c,DMA+0x14,u32::MAX);run(&mut m,c);m.bus.receive_ethernet_frame(&f).unwrap();run(&mut m,Vec::new());let received=(0..60).map(|i|m.bus.read8(RXBUF+i).unwrap()).collect::<Vec<_>>();assert_eq!(received,f);assert_eq!(m.bus.read32(RXDESC).unwrap(),64<<16|1<<9|1<<8);assert_ne!(m.bus.read32(DMA+0x14).unwrap()&1<<6,0);
    let mut c=Vec::new();write(&mut c,DMA+0x14,u32::MAX);run(&mut m,c);assert_eq!(m.bus.read32(DMA+0x14).unwrap()&(1<<16|1<<6),0);
    m.bus.reboot([0;6]);assert!(m.bus.take_ethernet_frames().is_empty());let mut c=Vec::new();setup(&mut c);transmit(&mut c,TXBUF);run(&mut m,c);assert_eq!(m.bus.take_ethernet_frames().len(),1,"MCU reboot preserves the selected host relay and discards its queues");
}
#[test]
fn p4_mdio_wrong_address_pin_phy_reset_and_dma_clock_memory_negatives(){
    let mut m=machine();let mut c=Vec::new();setup(&mut c);mdio(&mut c,2,2);run(&mut m,c);assert_eq!(m.bus.read32(RESULT).unwrap(),0xffff);
    let mut c=Vec::new();write(&mut c,0x500e_0158+4*107,0x80|53);mdio(&mut c,2,1);run(&mut m,c);assert_eq!(m.bus.read32(RESULT).unwrap(),0xffff,"MDI input107 must reach52, independently of output109");
    let mut c=Vec::new();write(&mut c,0x500e_0158+4*107,0x80|52);write(&mut c,0x500e_0018,1<<19);mdio(&mut c,2,1);run(&mut m,c);assert_eq!(m.bus.read32(RESULT).unwrap(),0xffff,"physical PHY_RST51 low suppresses MDIO");
    let mut c=Vec::new();write(&mut c,0x500e_0014,1<<19);write(&mut c,0x500e_1004+4*50,0);transmit(&mut c,TXBUF);run(&mut m,c);assert!(m.bus.take_ethernet_frames().is_empty());assert_eq!(m.bus.read32(TXDESC).unwrap()>>31,1,"unrouted reference clock cannot consume descriptors");
    let mut c=Vec::new();write(&mut c,0x500e_1004+4*50,3<<12|1<<9);write(&mut c,0x500e_6018,0);transmit(&mut c,TXBUF);run(&mut m,c);assert!(m.bus.take_ethernet_frames().is_empty());assert_eq!(m.bus.read32(TXDESC).unwrap()>>31,1);
    let mut c=Vec::new();transmit(&mut c,0x4803_0000);write(&mut c,0x500e_6018,1<<13);run(&mut m,c);assert!(m.bus.take_ethernet_frames().is_empty());assert_ne!(m.bus.read32(DMA+0x14).unwrap()&1<<13,0,"unmapped PSRAM source raises FBE, with no MMIO or host-byte fallback");
}
