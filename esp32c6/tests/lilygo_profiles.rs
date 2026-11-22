use emu_core::Bus;
use esp_soc::{BoardModel,SocBus as _};
use esp32c6::board::{make_board,lilygo::LilygoC6};

// ESP-IDF v5.5.4 C6 gpio_reg.h/io_mux_reg.h: GPIO selector128,
// MCU_SEL1, OUT_INV bit8, OUT/ENABLE W1TS/W1TC. Same fields as v5.1/v5.3.
fn drive(b:&mut esp32c6::bus::SocBus,pin:u8,high:bool) {
    b.write32(0x6009_0004+4*u32::from(pin),1<<12).unwrap();
    b.write32(0x6009_1554+4*u32::from(pin),128).unwrap();
    b.write32(0x6009_1024,1<<pin).unwrap();
    b.write32(if high {0x6009_1008}else{0x6009_100c},1<<pin).unwrap();
}
#[test]
fn c6_exact_board_ids_boot_and_no_invented_screen_or_encoder() {
    for name in ["lilygo-t7-c6","lilygo-t-knob"] {
        let mut m=esp32c6::machine([0;6],4<<20);let b=&mut m.bus;b.board=make_board(name).unwrap();b.attach_board_devices();
        assert_eq!(b.board.name(),name);assert_eq!(b.gpio_input()&((1<<8)|(1<<9)),(1<<8)|(1<<9));
        assert_eq!(b.board.named_pin("boot"),Some(9));assert!(b.board.display().is_none());assert!(b.board.encoder().is_none());
        assert!(b.board.i2c_devices().is_empty());assert!(b.board.sd_card_slot().is_none());
        assert!(b.board.button_at(100,"boot",true));assert_eq!(b.board.take_edges(),[esp_soc::board::BoardEdge{cycle:100,pin:9,level:false}]);
        assert!(!b.board.input_levels().into_iter().find(|x|x.0==9).unwrap().1);
        assert!(b.board.button_at(101,"boot",true));assert!(b.board.take_edges().is_empty());
        assert!(b.board.button_at(102,"boot",false));assert!(b.board.take_edges()[0].level);
        assert!(!b.board.button_at(103,"encoder",true));
    }
    let mut bare=LilygoC6::t7_c6();bare.rmt_frame(8,&[true;24]);assert!(bare.leds().is_none());
    assert!(make_board("lilygo-t-knob-quadrature").is_none());
}
#[test]
fn knob_four_physical_green_leds_follow_low_active_gpio_routes_and_reset() {
    let mut m=esp32c6::machine([0;6],4<<20);let b=&mut m.bus;b.board=make_board("lilygo-t-knob").unwrap();b.attach_board_devices();
    for (index,pin) in [23,22,21,20].into_iter().enumerate() {
        drive(b,pin,false);assert_eq!(b.board.leds().unwrap().0[index],[0,255,0]);
        b.write32(0x6009_1554+4*u32::from(pin),128|1<<8).unwrap();assert_eq!(b.board.leds().unwrap().0[index],[0;3]);
        drive(b,pin,false);b.write32(0x6009_1028,1<<pin).unwrap();assert_eq!(b.board.leds().unwrap().0[index],[0;3]);
        drive(b,pin,false);b.write32(0x6009_0004+4*u32::from(pin),0).unwrap();assert_eq!(b.board.leds().unwrap().0[index],[0;3]);
        drive(b,pin,true);assert_eq!(b.board.leds().unwrap().0[index],[0;3]);
        drive(b,pin,false);
    }
    assert_eq!(b.board.leds().unwrap().0,[[0,255,0];4]);
    b.board.rmt_frame(20,&[false;24]);assert_eq!(b.board.leds().unwrap().0,[[0,255,0];4]);
    b.reboot([0;6]);assert_eq!(b.board.leds().unwrap().0,[[0;3];4]);
}
