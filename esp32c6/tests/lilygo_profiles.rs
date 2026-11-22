use emu_core::Bus;
use esp_soc::{BoardModel,SocBus as _};
use esp32c6::board::{make_board,lilygo::LilygoC6};

// ESP-IDF v5.5.4 C6 gpio_reg.h/io_mux_reg.h: GPIO selector128,
// MCU_SEL1, OUT_INV bit8, OUT/ENABLE W1TS/W1TC. No factory image is executed here.
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

fn tqt()->esp32c6::Machine {
    let mut m=esp32c6::machine([0;6],4<<20);m.bus.board=make_board("lilygo-t-qt-c6").unwrap();m.bus.attach_board_devices();
    for (pin,signal) in [(18,63),(15,65),(14,68)] {
        m.bus.write32(0x6009_0004+4*pin,1<<12|1<<9).unwrap();m.bus.write32(0x6009_1554+4*pin,signal).unwrap();
    }
    m.bus.write32(0x6008_1020,0x3e).unwrap();drive(&mut m.bus,20,true);drive(&mut m.bus,2,false);m
}
fn spi(b:&mut esp32c6::bus::SocBus,data:bool,bytes:&[u8]) {
    drive(b,19,data);
    for chunk in bytes.chunks(64) {
        for (index,word) in chunk.chunks(4).enumerate() {let mut bytes=[0;4];bytes[..word.len()].copy_from_slice(word);b.write32(0x6008_1098+4*index as u32,u32::from_le_bytes(bytes)).unwrap();}
        b.write32(0x6008_1010,1<<27).unwrap();b.write32(0x6008_101c,(chunk.len()*8-1) as u32).unwrap();b.write32(0x6008_1000,1<<24).unwrap();
    }
}
fn command(b:&mut esp32c6::bus::SocBus,command:u8,data:&[u8]) {spi(b,false,&[command]);if !data.is_empty() {spi(b,true,data);}}
fn init_lcd(b:&mut esp32c6::bus::SocBus,madctl:u8) {for c in [0x11,0x29,0x21] {command(b,c,&[]);}command(b,0x3a,&[5]);command(b,0x36,&[madctl]);}
fn pixel(b:&mut esp32c6::bus::SocBus,x:u16,y:u16,color:u16) {
    let [xh,xl]=x.to_be_bytes();let [yh,yl]=y.to_be_bytes();command(b,0x2a,&[xh,xl,xh,xl]);command(b,0x2b,&[yh,yl,yh,yl]);command(b,0x2c,&color.to_be_bytes());
}
#[test]
fn gc9107_official_rotations_offsets_and_rgb565_use_actual_c6_spi() {
    // Independent pinned Arduino_GC9107/Arduino_TFT + pin_config combinations:
    // r0=C8 offsets2,1; r1=A8 offsets1,0; r2=08 offsets0,0; r3=68 offsets0,2.
    // An upright source pixel walks clockwise around the glass as rotation changes.
    for (madctl,ox,oy,corner) in [(0xc8,2,1,0),(0xa8,1,0,127),(8,0,0,128*128-1),(0x68,0,2,127*128)] {
        let mut m=tqt();let b=&mut m.bus;init_lcd(b,madctl);pixel(b,ox,oy,0xf800);
        let (w,h,pixels,_)=b.board.display().unwrap();assert_eq!((w,h),(128,128));assert_eq!(pixels[corner],0xf800);
        assert_eq!(pixels.iter().filter(|&&p|p!=0).count(),1);
        pixel(b,127+ox,127+oy,0x001f);assert_eq!(b.board.display().unwrap().2[128*128-1-corner],0x001f);
    }
    let mut m=tqt();let b=&mut m.bus;init_lcd(b,0xc8);pixel(b,2,1,0x07e0);assert_eq!(b.board.display().unwrap().2[0],0x07e0);
    command(b,0x20,&[]);assert_eq!(b.board.display().unwrap().2[0],!0x07e0u16);
    command(b,0x21,&[]);command(b,0x36,&[0xc0]);pixel(b,2,1,0xf800);assert_eq!(b.board.display().unwrap().2[0],0x001f);
}
#[test]
fn gc9107_pin_reset_sleep_and_bad_window_gates_do_not_invent_pixels() {
    let mut m=tqt();let b=&mut m.bus;init_lcd(b,0xc8);pixel(b,2,1,0xf800);let first=b.board.display_version();
    b.write32(0x6009_0004+4*15,0).unwrap();pixel(b,2,1,0xffff);assert_eq!(b.board.display_version(),first,"wrong MOSI mux cannot reach panel");
    b.write32(0x6009_0004+4*15,1<<12).unwrap();b.write32(0x6008_1020,0x3f).unwrap();pixel(b,2,1,0xffff);assert_eq!(b.board.display().unwrap().2[0],0xf800,"CS must be asserted");
    b.write32(0x6008_1020,0x3e).unwrap();
    for (x,y) in [(1,1),(130,1),(2,0),(2,129),(65535,65535)] {pixel(b,x,y,0xffff);assert_eq!(b.board.display().unwrap().2[0],0xf800);}
    command(b,0x2a,&[0,2,0]);command(b,0x2c,&[0xff,0xff]);assert_eq!(b.board.display().unwrap().2[0],0xf800,"incomplete window cannot reuse previous bounds");
    command(b,0x3a,&[6]);pixel(b,2,1,0xffff);assert_eq!(b.board.display().unwrap().2[0],0xf800,"unsupported RGB666 is not RGB565");
    command(b,0x10,&[]);assert_eq!(b.board.display().unwrap().2[0],0);command(b,0x11,&[]);assert_eq!(b.board.display().unwrap().2[0],0xf800);
    let before_reset=b.board.display_version();drive(b,20,false);assert_ne!(b.board.display_version(),before_reset,"reset must publish darkness even when the RAM counter restarts");
    pixel(b,2,1,0xffff);assert_eq!(b.board.display().unwrap().2[0],0);drive(b,20,true);assert_eq!(b.board.display().unwrap().2[0],0);
    init_lcd(b,0xc8);assert_eq!(b.board.display().unwrap().2[0],0xf800,"controller reset preserves RAM, without inventing pixels");
    drive(b,2,true);assert_eq!(b.board.display().unwrap().2[0],0);drive(b,2,false);assert_eq!(b.board.display().unwrap().2[0],0xf800);
    command(b,0x28,&[]);assert_eq!(b.board.display().unwrap().2[0],0);command(b,0x29,&[]);assert_eq!(b.board.display().unwrap().2[0],0xf800);
}
fn ledc(b:&mut esp32c6::bus::SocBus,pin:u8,duty:u32) {
    b.write32(0x6009_6034,1).unwrap();b.write32(0x6009_6038,1<<22|3<<20).unwrap();
    b.write32(0x6009_0004+4*u32::from(pin),1<<12).unwrap();b.write32(0x6009_1554+4*u32::from(pin),0).unwrap();b.write32(0x6009_1024,1<<pin).unwrap();
    b.write32(0x6000_70d0,3).unwrap();b.write32(0x6000_70a0,8|256<<5|1<<26).unwrap();b.write32(0x6000_7008,duty<<4).unwrap();
    b.write32(0x6000_700c,1<<31).unwrap();b.write32(0x6000_7000,4|1<<4).unwrap();b.tick(10_000);
}
#[test]
fn tqt_low_active_backlight_observes_latched_pwm_clock_mux_inversion_and_no_ticks() {
    let mut m=tqt();let b=&mut m.bus;init_lcd(b,0xc8);pixel(b,2,1,0xffff);
    ledc(b,2,0);let cycles=b.cycles();b.refresh_board_outputs();assert_eq!(b.cycles(),cycles);assert_eq!(b.board.display().unwrap().2[0],0xffff);
    let version=b.board.display_version();b.refresh_board_outputs();assert_eq!(b.board.display_version(),version,"observation does not create new output");
    ledc(b,2,128);b.refresh_board_outputs();assert_eq!(b.board.display().unwrap().2[0],0x7bef,"half low-active duty dims actual GRAM");
    ledc(b,2,256);b.refresh_board_outputs();assert_eq!(b.board.display().unwrap().2[0],0);
    b.write32(0x6009_1554+4*2,1<<8).unwrap();b.refresh_board_outputs();assert_eq!(b.board.display().unwrap().2[0],0xffff);
    for (address,value) in [(0x6009_6034,0),(0x6009_0004+4*2,0),(0x6009_1554+4*2,1<<8|1<<10),(0x6009_1028,1<<2)] {
        ledc(b,2,0);b.write32(address,value).unwrap();b.refresh_board_outputs();assert_eq!(b.board.display().unwrap().2[0],0);
    }
    drive(b,9,false);assert_eq!(b.board.leds().unwrap().0,[[255;3]]);b.board.rmt_frame(9,&[false;24]);assert_eq!(b.board.leds().unwrap().0,[[255;3]],"not a WS2812");
    ledc(b,9,256);b.refresh_board_outputs();assert_eq!(b.board.leds().unwrap().0,[[0;3]]);
    b.reboot([0;6]);b.refresh_board_outputs();assert_eq!(b.board.display().unwrap().2[0],0);
}
fn i2c_route(b:&mut esp32c6::bus::SocBus,sda:u8,scl:u8) {
    for (pin,signal) in [(sda,46),(scl,45)] {b.write32(0x6009_0004+4*u32::from(pin),1<<12|1<<9).unwrap();b.write32(0x6009_1554+4*u32::from(pin),signal).unwrap();b.write32(0x6009_1154+4*signal,0x80|u32::from(pin)).unwrap();}
}
fn i2c(b:&mut esp32c6::bus::SocBus,address:u8,write:&[u8],read:usize)->(bool,Vec<u8>) {
    let base=0x6000_4000;b.write32(base+0x24,u32::MAX).unwrap();b.write32(base+0x18,3<<12).unwrap();
    for byte in std::iter::once(address<<1).chain(write.iter().copied()).chain((read>0).then_some(address<<1|1)) {b.write32(base+0x1c,u32::from(byte)).unwrap();}
    let mut commands=vec![6<<11,1<<11|1<<8|(write.len()+1) as u32];
    if read>0 {commands.extend([6<<11,1<<11|1<<8|1,3<<11|read as u32]);}commands.push(2<<11);
    for (index,command) in commands.into_iter().enumerate() {b.write32(base+0x58+4*index as u32,command).unwrap();}b.write32(base+4,1<<5).unwrap();b.tick(100_000);
    let acknowledged=b.read32(base+0x20).unwrap()&(1<<10)==0;let data=(0..read).map(|_|b.read32(base+0x1c).unwrap() as u8).collect();(acknowledged,data)
}
#[test]
fn cst816t_real_i2c_route_contact_sleep_reset_and_unknown_ids() {
    let mut m=tqt();let b=&mut m.bus;i2c_route(b,21,22);b.board.touch(73,126,true);
    assert_eq!(i2c(b,0x15,&[2],5),(true,vec![1,0,73,0,126]));
    i2c_route(b,21,18);assert!(!i2c(b,0x15,&[2],5).0);i2c_route(b,21,22);
    assert!(!i2c(b,0x1f,&[0],1).0,"PMIC is not fabricated");assert!(!i2c(b,0x6a,&[0x0f],1).0,"IMU is not fabricated");
    assert!(!i2c(b,0x15,&[0xa7],1).0,"unknown firmware identity NACKs");
    assert!(!i2c(b,0x15,&[0xa5,3],0).0,"CST816S sleep address does not apply to CST816T");
    assert_eq!(i2c(b,0x15,&[2],1),(true,vec![1]));assert!(i2c(b,0x15,&[0xe5,3],0).0);b.board.touch(1,1,true);assert!(!i2c(b,0x15,&[2],1).0);
    drive(b,23,false);assert!(!i2c(b,0x15,&[2],1).0);drive(b,23,true);assert_eq!(i2c(b,0x15,&[2],1),(true,vec![0]));
    b.board.touch(65535,65535,true);assert_eq!(i2c(b,0x15,&[2],5),(true,vec![1,0,127,0,127]));b.board.touch(127,127,false);assert_eq!(i2c(b,0x15,&[2],1),(true,vec![0]));
}
#[test]
fn cst816t_change_irq_has_documented_pulse_width_and_gesture_mode_is_not_faked() {
    let mut b=esp32c6::board::lilygo_tqt::TQtC6::new();let mut devices=b.i2c_devices();assert_eq!(devices.len(),1);assert_eq!((devices[0].0,devices[0].1,devices[0].2.pins()),(0,0x15,Some((21,22))));
    let write=|device:&mut dyn esp_periph::i2c::I2cDevice,register,value| {assert!(device.start(false));assert!(device.write(register));assert!(device.write(value));};
    write(devices[0].2.as_mut(),0xfa,0x10);b.touch_at(1,10,20,true);assert_eq!(b.touch_irq(),Some((7,true)),"gesture recognition is not implemented");
    write(devices[0].2.as_mut(),0xfa,0x20);write(devices[0].2.as_mut(),0xed,2);b.touch_at(100,11,20,true);assert_eq!(b.touch_irq(),Some((7,false)));
    let end=100+esp32c6::periph::CPU_HZ/10_000*2;assert_eq!(b.next_deadline(),Some(end));b.advance_to(end-1);assert_eq!(b.touch_irq(),Some((7,false)));
    b.advance_to(end);assert_eq!(b.touch_irq(),Some((7,true)));assert_eq!(b.take_edges().iter().map(|e|(e.cycle,e.pin,e.level)).collect::<Vec<_>>(),[(100,7,false),(end,7,true)]);
    write(devices[0].2.as_mut(),0xe5,3);b.touch_at(end+1,12,20,true);assert_eq!(b.next_deadline(),None);assert_eq!(b.touch_irq(),Some((7,true)));
}
