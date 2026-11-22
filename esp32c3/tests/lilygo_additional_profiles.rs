#[path="../src/board/lilygo_additional.rs"]
mod lilygo_additional;
use emu_core::Bus;
use esp_soc::BoardModel;
use esp_periph::sdmmc::MemStore;
use lilygo_additional::LilygoAdditionalC3;

// ESP-IDF v5.5.4 C3 gpio_reg.h/io_mux_reg.h/spi_reg.h, independently
// exercised through the existing MMIO bus and GPIO matrix, not board setters.
fn drive(b:&mut esp32c3::bus::SocBus,pin:u8,high:bool) {
    b.write32(0x6000_9004+4*u32::from(pin),1<<12|1<<9).unwrap();b.write32(0x6000_4554+4*u32::from(pin),128).unwrap();
    b.write32(0x6000_4024,1<<pin).unwrap();b.write32(if high {0x6000_4008}else{0x6000_400c},1<<pin).unwrap();
}
fn machine(board:LilygoAdditionalC3)->esp32c3::Machine {let mut m=esp32c3::machine([0;6],4<<20);m.bus.board=Box::new(board);m.bus.attach_board_devices();m}

#[test]
fn c3_led_routes_observe_enable_mux_inversion_without_instruction_scheduling() {
    for (board,color) in [(LilygoAdditionalC3::t01_c3(),[0,255,0]),(LilygoAdditionalC3::t8_c3(),[0,255,0]),(LilygoAdditionalC3::t_zigbee(),[0,0,255])] {
        let mut m=machine(board);let b=&mut m.bus;assert!(!b.board.uses_gpio_waveform());assert!(b.board.uses_gpio_routes());assert!(b.board.display().is_none());
        drive(b,3,true);assert_eq!(b.board.leds().unwrap().0,[color]);
        b.write32(0x6000_4554+4*3,128|1<<8).unwrap();assert_eq!(b.board.leds().unwrap().0,[[0;3]]);
        drive(b,3,true);b.write32(0x6000_4028,1<<3).unwrap();assert_eq!(b.board.leds().unwrap().0,[[0;3]]);
        drive(b,3,true);b.write32(0x6000_9004+4*3,0).unwrap();assert_eq!(b.board.leds().unwrap().0,[[0;3]]);
        drive(b,3,true);b.write32(0x6000_4554+4*3,65).unwrap();assert_eq!(b.board.leds().unwrap().0,[[0;3]]);
        drive(b,3,false);b.board.rmt_frame(3,&[true;24]);assert_eq!(b.board.leds().unwrap().0,[[0;3]],"plain LED is not an RGB serial device");
        assert!(b.board.i2c_devices().is_empty());assert!(b.board.lora_snapshot().is_none());
    }
}

#[test]
fn cold_route_hook_keeps_legacy_waveform_opt_in() {
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    struct Legacy(Arc<AtomicUsize>);
    impl BoardModel for Legacy {
        fn name(&self)->&'static str {"test-legacy-waveform"}
        fn uses_gpio_waveform(&self)->bool {true}
        fn gpio_waveform_at(&mut self,_cycle:u64,_routes:esp_soc::pins::PinRoutes<'_>) {self.0.fetch_add(1,Ordering::Relaxed);}
    }
    let count=Arc::new(AtomicUsize::new(0));let mut m=esp32c3::machine([0;6],4<<20);m.bus.board=Box::new(Legacy(count.clone()));
    assert!(m.bus.board.uses_gpio_routes());m.bus.attach_board_devices();let before=count.load(Ordering::Relaxed);assert!(before>0);
    m.bus.write32(0x6000_9004,1<<12).unwrap();assert!(count.load(Ordering::Relaxed)>before);
}

#[test]
fn exact_buttons_do_not_invent_download_pad_or_tlsr_controls() {
    for (mut board,id,pin) in [(LilygoAdditionalC3::t8_c3(),"boot",9),(LilygoAdditionalC3::t_zigbee(),"user",2)] {
        assert_eq!(board.named_pin(id),Some(pin));assert!(board.button_at(10,id,true));assert!(board.button_at(11,id,true));
        assert_eq!(board.take_edges(),[esp_soc::board::BoardEdge{cycle:10,pin,level:false}]);assert!(board.button_at(20,id,false));
        assert_eq!(board.take_edges(),[esp_soc::board::BoardEdge{cycle:20,pin,level:true}]);assert!(!board.button_at(21,"reset",true));
    }
    let mut t01=LilygoAdditionalC3::t01_c3();assert!(t01.named_pin("boot").is_none());assert!(!t01.button_at(1,"boot",true));assert!(t01.input_levels().is_empty());
    let mut zigbee=LilygoAdditionalC3::t_zigbee();assert!(zigbee.named_pin("boot").is_none());assert!(!zigbee.button_at(1,"tlsr",true));
    for board in [&mut t01,&mut zigbee] {assert!(board.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),Default::default()).is_err());}
}

fn spi(b:&mut esp32c3::bus::SocBus,bytes:&[u8])->Vec<u8> {
    let mut out=Vec::new();for chunk in bytes.chunks(64) {
        for (index,word) in chunk.chunks(4).enumerate() {let mut w=[0;4];w[..word.len()].copy_from_slice(word);b.write32(0x6002_4098+4*index as u32,u32::from_le_bytes(w)).unwrap();}
        b.write32(0x6002_4010,1<<27|1<<28).unwrap();b.write32(0x6002_401c,(chunk.len()*8-1) as u32).unwrap();b.write32(0x6002_4000,1<<24).unwrap();
        out.extend((0..chunk.len()).map(|i|((b.read32(0x6002_4098+4*(i/4) as u32).unwrap()>>((i%4)*8))&255) as u8));
    }out
}
fn crc7(bytes:&[u8])->u8 {let mut crc=0u8;for &value in bytes {let mut byte=value;for _ in 0..8 {crc<<=1;if (byte^crc)&0x80!=0 {crc^=9;}byte<<=1;}}crc&0x7f}
fn command(b:&mut esp32c3::bus::SocBus,cmd:u8,arg:u32)->Vec<u8> {let mut bytes=vec![0x40|cmd];bytes.extend(arg.to_be_bytes());bytes.push(crc7(&bytes)<<1|1);spi(b,&bytes);spi(b,&[255;2])}
fn init_card(b:&mut esp32c3::bus::SocBus) {
    drive(b,6,true);spi(b,&[255;10]);drive(b,6,false);assert_eq!(command(b,0,0),[255,1]);assert_eq!(command(b,8,0x1aa),[255,1]);assert_eq!(spi(b,&[255;4]),[0,0,1,0xaa]);
    for _ in 0..2 {command(b,55,0);command(b,41,1<<30);}
}
fn card()->esp32c3::Machine {
    let mut m=machine(LilygoAdditionalC3::t8_c3());let b=&mut m.bus;
    for (pin,signal) in [(4,63),(7,65)] {b.write32(0x6000_9004+4*pin,1<<12|1<<9).unwrap();b.write32(0x6000_4554+4*pin,signal).unwrap();}
    b.write32(0x6000_9018,1<<12|1<<9).unwrap();b.write32(0x6000_4254,0x40|5).unwrap();b.write32(0x6002_4020,0x3f).unwrap();
    b.board.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),Default::default()).unwrap();init_card(b);m
}
fn read_block(b:&mut esp32c3::bus::SocBus)->Vec<u8> {assert_eq!(command(b,17,3),[255,0]);assert_eq!(spi(b,&[255;2]),[255,0xfe]);let data=spi(b,&[255;512]);spi(b,&[255;2]);data}

#[test]
fn t8_spi_sd_uses_guest_registers_persists_blocks_and_rejects_wrong_routes() {
    let mut m=card();let b=&mut m.bus;assert_eq!(command(b,24,3),[255,0]);let mut data=vec![0xfe];data.extend([0x5a;512]);data.extend([255;2]);spi(b,&data);assert_eq!(spi(b,&[255]),[5]);
    assert_eq!(read_block(b),vec![0x5a;512]);
    b.write32(0x6000_9020,0).unwrap();assert_eq!(command(b,58,0),[255;2]);b.write32(0x6000_9020,1<<12|1<<9).unwrap();
    b.write32(0x6000_4254,0x40|2).unwrap();assert_eq!(command(b,58,0),[255;2]);b.write32(0x6000_4254,0x40|5).unwrap();drive(b,6,true);drive(b,6,false);
    drive(b,6,true);assert_eq!(command(b,0,0),[255;2]);drive(b,6,false);
    // Raising the physical CS pin aborts a partial write even without further clocks.
    assert_eq!(command(b,24,3),[255,0]);spi(b,&[0xfe,9,9,9]);drive(b,6,true);drive(b,6,false);assert_eq!(read_block(b),vec![0x5a;512]);
    b.board.set_sd_card_present(false);assert_eq!(command(b,0,0),[255;2]);b.board.set_sd_card_present(true);assert_eq!(command(b,0,0),[255;2],"reinsertion requires startup clocks");
    init_card(b);assert_eq!(read_block(b),vec![0x5a;512]);assert!(b.board.report().contains("read-blocks="));
}

#[test]
fn watch_keyboard_mcu_runs_original_rv32i_sram_uart_without_invented_keyboard_wiring() {
    let mut m=machine(LilygoAdditionalC3::watch_keyboard_c3());assert_eq!(m.cores.len(),1);assert_eq!(m.bus.flash.len(),4<<20);assert_eq!(m.bus.board.name(),"lilygo-watch-keyboard-c3-v1-0");
    assert!(!m.bus.board.uses_gpio_routes());assert!(!m.bus.board.uses_gpio_waveform());assert!(!m.bus.board.uses_spi_pins());assert!(m.bus.board.i2c_devices().is_empty());assert!(m.bus.board.input_levels().is_empty());
    for id in ["boot","button","user","a","key1","reset"] {assert_eq!(m.bus.board.named_pin(id),None);assert!(!m.bus.board.button_at(10,id,true));}assert!(m.bus.board.take_edges().is_empty());
    for pin in [3,8,9] {drive(&mut m.bus,pin,true);assert!(m.bus.board.leds().is_none());assert!(m.bus.board.display().is_none());}
    assert!(m.bus.board.attach_spi_sd_card(Box::new(MemStore(vec![0;1024])),Default::default()).is_err());assert!(m.bus.board.report().contains("physical Flash capacity unknown"));
    // Original RV32I guest, not a ROM replacement or vendor firmware. UART0
    // FIFO=60000000h follows ESP-IDF v5.5.4 esp32c3/register/soc/uart_reg.h.
    // The virtual console is not a claim that an unknown physical connector is routed.
    let message=b"WATCH-C3:MCU-DIAG\n";let mut words=vec![0x6000_02b7u32]; // lui t0,60000h
    for &byte in message {words.extend([(u32::from(byte)<<20)|(6<<7)|0x13,(6<<20)|(5<<15)|(2<<12)|0x23]);}
    words.extend([(37<<20)|(7<<7)|0x13,(5<<20)|(7<<15)|(7<<7)|0x13,0x3fc8_1437,(7<<20)|(8<<15)|(2<<12)|0x23,0x0000_006f]);
    let bytes:Vec<u8>=words.into_iter().flat_map(u32::to_le_bytes).collect();m.bus.load_bytes(esp32c3::bus::IRAM_LOW,&bytes).unwrap();m.cores[0].pc=esp32c3::bus::IRAM_LOW;m.console.capture=true;m.run(128);
    assert_eq!(m.console.uart0,message);assert_eq!(m.bus.read32(0x3fc8_1000).unwrap(),42);assert_eq!(m.exceptions,0);assert_eq!(m.stub_hits,0);assert!(m.stubs.is_empty());
}
