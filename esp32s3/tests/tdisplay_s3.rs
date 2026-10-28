//! T-Display-S3 buttons through the web input queue and actual guest GPIO reads.
//! This tiny instruction probe is not the factory image or a factory-handler claim.
use emu_core::{Bus,Core};
use esp_soc::{Stop,web::WebServer};

#[test]
fn web_buttons_reach_guest_gpio0_and_gpio14_independently_and_together() {
    let mut m=esp32s3::machine([0;6]);m.bus.board=esp32s3::board::make_board("tdisplay-s3").unwrap();m.bus.attach_board_devices();
    let web=WebServer::queued();m.web=Some(web.clone());m.console.capture=true;
    for pin in [0,14] {
        // IDF v4.4.4 soc/esp32s3/include/soc/io_mux_reg.h: FUN_IE bit9,
        // MCU_SEL bits12..14; gpio_reg.h GPIO_IN at +0x3c.
        m.bus.write32(0x60009004+4*pin,1<<12|1<<9).unwrap();
    }
    const IRAM:u32=0x40370000;
    m.bus.load_bytes(IRAM,&[0x22,0x23,0x00,0x06,0xff,0xff]).unwrap(); // l32i a2,a3,0; j .
    m.cores[0].ps=0;m.cores[0].set_ar(3,0x6000403c);m.cores[0].set_jit(false);
    let read=|m:&mut esp32s3::Machine| {m.cores[0].pc=IRAM;assert!(matches!(m.run(64),Stop::MaxInsns));m.cores[0].get_ar(2)&((1<<14)|1)};
    assert_eq!(read(&mut m),(1<<14)|1);
    for (pin,remaining) in [(0,1<<14),(14,1)] {
        web.push_incoming(format!(r#"{{"t":"btn","pin":{pin},"v":1}}"#));assert_eq!(read(&mut m),remaining);
        web.push_incoming(format!(r#"{{"t":"btn","pin":{pin},"v":0}}"#));assert_eq!(read(&mut m),(1<<14)|1);
    }
    for pin in [0,14] {web.push_incoming(format!(r#"{{"t":"btn","pin":{pin},"v":1}}"#));}
    assert_eq!(read(&mut m),0);
    for pin in [0,14] {web.push_incoming(format!(r#"{{"t":"btn","pin":{pin},"v":0}}"#));}
    m.run(0);assert_eq!(read(&mut m),(1<<14)|1,"release while execution is paused must reach the next guest read");
    m.load_script("1.0 press boot 50\n2.0 press user 50\n").unwrap();
    assert!(matches!(m.script.events[0].1,esp_soc::ScriptAction::Gpio(0,false)));
    assert!(matches!(m.script.events[2].1,esp_soc::ScriptAction::Gpio(14,false)));
}

#[test]
fn pwm_backlight_uses_latched_duty_routes_clock_and_reset_without_advancing_time() {
    use esp_soc::{SocBus,board::LcdI80Pins};
    let mut m=esp32s3::machine([0;6]);let b=&mut m.bus;
    struct NoPwm;
    impl esp_soc::BoardModel for NoPwm {
        fn name(&self)->&'static str {"no-pwm-probe"}
        fn gpio_pwm_at(&mut self,_:u64,_:&dyn Fn(u8)->Option<(f64,u32)>) {panic!("non-opt-in board must not query PWM");}
    }
    b.board=Box::new(NoPwm);b.attach_board_devices();b.refresh_board_outputs();
    b.board=esp32s3::board::make_board("tdisplay-s3").unwrap();b.attach_board_devices();
    for pin in [5,9,38] {b.write32(0x60009004+4*pin,1<<12).unwrap();}
    b.write32(0x60004024,(1<<5)|(1<<9)).unwrap();b.write32(0x60004030,1<<6).unwrap();b.write32(0x60004008,(1<<5)|(1<<9)).unwrap();
    let pins=LcdI80Pins{data:std::array::from_fn(|i|if i<8 {1u64<<[39,40,41,42,45,46,47,48][i]} else {0}),clock:1<<8,select:1<<6,dc:1<<7,width:8,..Default::default()};
    for (cmd,data) in [(0x11,vec![]),(0x29,vec![]),(0x2a,vec![0,35,0,35]),(0x2b,vec![0,0,0,0]),(0x2c,vec![255,255])] {
        b.board.lcd_i80(pins,false,&[cmd]);b.board.lcd_i80(LcdI80Pins{dc_high:true,..pins},true,&data);
    }
    let pixel=|b:&mut esp32s3::bus::SocBus| {let before=b.cycles;b.refresh_board_outputs();assert_eq!(b.cycles,before);b.board.display().unwrap().2[0]};
    assert_eq!(pixel(b),0);
    // IDF v4.4.4 ledc_reg.h: timer +a0 PARA_UP bit25; channel CONF0 bit4
    // PARA_UP, bit2 SIG_OUT_EN; CONF1 bit31 DUTY_START; duty is Q4.
    for (address,value) in [(0x600c0018,1<<11),(0x600190d0,3),(0x600045ec,73),(0x600190a0,8|(384<<4)|(1<<25)),(0x60019008,64<<4),(0x6001900c,1<<31),(0x60019000,4|(1<<4))] {b.write32(address,value).unwrap();}
    assert_eq!(pixel(b),0,"pending duty is not active before timer wrap");
    b.tick(3000);b.flush_ticks();assert_eq!(pixel(b),0x39e7);let version=b.board.display_version();assert_eq!(pixel(b),0x39e7);assert_eq!(b.board.display_version(),version);
    b.write32(0x600045ec,74).unwrap();assert_eq!(pixel(b),0,"wrong LEDC channel is not connected");
    b.write32(0x600045ec,73).unwrap();b.write32(0x6000909c,0).unwrap();assert_eq!(pixel(b),0,"wrong IO_MUX is not connected");
    b.write32(0x6000909c,1<<12).unwrap();assert_eq!(pixel(b),0x39e7);
    b.write32(0x600c0018,0).unwrap();assert_eq!(pixel(b),0,"clock-gated PWM is unavailable");
    b.write32(0x600c0018,1<<11).unwrap();assert_eq!(pixel(b),0x39e7);
    for (a,v) in [(0x60019008,0),(0x6001900c,1<<31),(0x60019000,4|(1<<4))] {b.write32(a,v).unwrap();}
    b.tick(3000);b.flush_ticks();assert_eq!(pixel(b),0,"zero duty turns the backlight off");
    SocBus::reboot(b,[0;6]);assert_eq!(pixel(b),0,"reboot does not keep cached PWM");
}
