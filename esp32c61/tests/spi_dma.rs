use emu_core::Bus;
use esp_soc::SocBus;
struct Echo;
impl esp_soc::BoardModel for Echo {
    fn name(&self)->&'static str {"c61-spi-dma-echo"}
    fn spi_transfer(&mut self,_host:u8,tx:&[u8],rx_len:usize)->Vec<u8> {(0..rx_len).map(|i|tx.get(i).copied().unwrap_or(255)^0xa5).collect()}
}
fn word(b:&mut esp32c61::bus::SocBus,a:u32,v:u32) {b.write32(a,v).unwrap();}
#[test]
fn c61_gdma_v2_full_address_control_and_spi_duplex_receive() {
    let mut b=esp32c61::bus::SocBus::new(4<<20,[0;6]);b.board=Box::new(Echo);b.attach_board_devices();
    let (out_desc,in_desc,tx,rx)=(0x40801000,0x40801100,0x40802000,0x40803000);
    for (a,v) in [(out_desc,3<<30|96<<12|96),(out_desc+4,tx),(out_desc+8,0),(in_desc,1<<31|128),(in_desc+4,rx),(in_desc+8,0)] {word(&mut b,a,v);}
    for i in 0..96 {b.write8(tx+i,i as u8).unwrap();}
    // ESP-IDF v5.5.4 C61 ahb_dma_reg.h: selector1, separate full
    // LINK_ADDR registers, OUT startbit1 / IN startbit2 (not C6 bits21/22).
    for (off,v) in [(0x100,1),(0xa0,1),(0xd0,4),(0xd4,1<<12),(0x74,1<<12),(0x3b8,out_desc),(0x3ac,in_desc),(0xe0,2),(0x80,5),(0x38,15),(0x08,31)] {word(&mut b,0x60080000+off,v);}
    assert_eq!(b.periph.gdma.gdma.out[0].desc,out_desc);assert_eq!(b.periph.gdma.gdma.inp[0].desc,in_desc);
    assert_eq!(b.read32(0x600800e0).unwrap(),0);assert_eq!(b.read32(0x60080080).unwrap(),1);
    for (off,v) in [(0x10,(1<<27)|(1<<28)|1),(0x1c,96*8-1),(0x30,(1<<27)|(1<<28)),(0x34,1<<12),(0,1<<24)] {word(&mut b,0x60081000+off,v);}
    assert_eq!(b.periph.spi2.transfers,1);for i in 0..96 {assert_eq!(b.read8(rx+i).unwrap(),i as u8^0xa5);}
    assert_eq!(b.read32(out_desc).unwrap()>>31,0);assert_eq!(b.read32(in_desc).unwrap(),1<<30|96<<12|128);
    assert_eq!(b.read32(0x600800e0).unwrap(),8);assert_eq!(b.read32(0x60080080).unwrap(),17);
    assert_eq!(b.read32(0x60080110).unwrap(),out_desc);assert_eq!(b.read32(0x600800b0).unwrap(),in_desc);
    b.refresh_irq();assert_eq!(b.periph.intmtx.status[1]&((1<<26)|(1<<28)|(1<<30)),(1<<26)|(1<<28)|(1<<30));
}
#[test]
fn c61_dma_start_stop_preserves_full_pointer_and_only_two_channels() {
    let mut b=esp32c61::bus::SocBus::new(4<<20,[0;6]);
    word(&mut b,0x600803bc,0x42340000);word(&mut b,0x600801a0,2);assert_eq!(b.periph.gdma.gdma.out[1].desc,0x42340000);assert!(b.periph.gdma.gdma.out[1].running);
    word(&mut b,0x600801a0,1);assert!(!b.periph.gdma.gdma.out[1].running);word(&mut b,0x600801a0,4);assert!(b.periph.gdma.gdma.out[1].running);
    word(&mut b,0x60080260,u32::MAX);assert!(!b.periph.gdma.gdma.out[2].running);
}
