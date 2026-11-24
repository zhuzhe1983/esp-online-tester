//! Real GP-SPI MISO must reach C6 AHB GDMA IN memory, not remain in W0.
//! Regression for unmodified IDF SDSPI CMD0 receive buffers.
use emu_core::Bus;
use esp32c6::bus::{SocBus,SRAM_LOW as RAM};
use esp_soc::board::{BoardModel,SpiQspiPins};
const SPI:u32=0x6008_1000;
struct CardReply;
impl BoardModel for CardReply {
    fn name(&self)->&'static str{"c6-spi-rx-fixture"}
    fn uses_qspi_pins(&self)->bool{true}
    fn spi_transfer_qspi_pins(&mut self,_:u8,_:SpiQspiPins,_:&[u8],n:usize)->Vec<u8>{(0..n).map(|i|if i==7{1}else{0xff}).collect()}
}
fn bus()->SocBus {let mut b=SocBus::new(8<<20,[0;6]);b.board=Box::new(CardReply);b.attach_board_devices();b}
fn descriptor(b:&mut SocBus,addr:u32,size:u32,dest:u32,next:u32,owner:bool){for(off,v)in[(0,size|if owner{1<<31}else{0}),(4,dest),(8,next)]{b.write32(addr+off,v).unwrap();}}
fn start(b:&mut SocBus,desc:u32,n:u32){
    let c=&mut b.periph.gdma.gdma.inp[0];c.running=true;c.desc=desc;c.peri_sel=0;c.conf1=1<<12;
    for(off,v)in[(0x30,1<<27),(0x10,1<<28),(0x1c,n*8-1),(0x98,0x12345678),(0,1<<24)]{b.write32(SPI+off,v).unwrap();}
}
#[test]
fn receive_scattered_bytes_and_owner_eof_match_the_card_response(){
    let mut b=bus();descriptor(&mut b,RAM+0x100,5,RAM+0x201,RAM+0x110,true);descriptor(&mut b,RAM+0x110,10,RAM+0x301,0,true);
    start(&mut b,RAM+0x100,15);let mut bytes:Vec<u8>=(0..5).map(|i|b.read8(RAM+0x201+i).unwrap()).collect();bytes.extend((0..10).map(|i|b.read8(RAM+0x301+i).unwrap()));
    assert_eq!(bytes,(0..15).map(|i|if i==7{1}else{255}).collect::<Vec<_>>());assert_eq!(b.read32(SPI+0x98).unwrap(),0x12345678);
    assert_eq!(b.read32(RAM+0x100).unwrap(),5|(5<<12));assert_eq!(b.read32(RAM+0x110).unwrap(),10|(10<<12)|(1<<30));
    let c=&b.periph.gdma.gdma.inp[0];assert_eq!(c.eof_desc,RAM+0x110);assert_eq!(c.int_raw&0x1b,3);assert!(!c.running);
}
#[test]
fn invalid_owner_mmio_destination_and_cyclic_chain_never_report_success(){
    for case in 0..3{
        let mut b=bus();let dest=if case==1{0x6009_1004}else{RAM+0x200};let next=if case==2{RAM+0x100}else{0};descriptor(&mut b,RAM+0x100,4,dest,next,case!=0);
        start(&mut b,RAM+0x100,15);let c=&b.periph.gdma.gdma.inp[0];assert_ne!(c.int_raw&8,0);assert_eq!(c.int_raw&2,0);assert!(!c.running);assert_eq!(b.periph.gpio.out,0);
    }
}
#[test]
fn legacy_boards_keep_fifo_and_explicit_cpu_fifo_mode_bypasses_dma(){
    let mut b=bus();b.periph.spi2.dma_rx_divert=false;start(&mut b,RAM+0x100,15);assert_eq!(b.read32(SPI+0x9c).unwrap(),0x01ffffff);
    let mut b=bus();for(off,v)in[(0x30,0),(0x10,1<<28),(0x1c,119),(0,1<<24)]{b.write32(SPI+off,v).unwrap();}assert_eq!(b.read32(SPI+0x9c).unwrap(),0x01ffffff);
}
