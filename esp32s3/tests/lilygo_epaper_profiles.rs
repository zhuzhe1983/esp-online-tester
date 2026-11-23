//! Public board protocol contracts. The path import also permits independent
//! testing before the product registry adds these exact board constructors.
use esp32s3::periph;
#[path="../src/board/lilygo_epaper.rs"]
mod lilygo_epaper;
use lilygo_epaper::LilygoEpaper;
use esp_periph::{Gpio,RegRam,i2c::I2cDevice,sdmmc::MemStore};
use esp_soc::{board::{BoardModel,BoardEdge,SpiPins},pins::ChipPins};

struct Rig {board:LilygoEpaper,gpio:Gpio,mux:RegRam,cycle:u64,pins:SpiPins,dc:u8,busy:u8}
impl Rig {
    fn deck()->Self {Self{board:LilygoEpaper::t_deck_pro(),gpio:Gpio::new(),mux:RegRam::new(),cycle:0,pins:SpiPins{sclk:1<<36,mosi:1<<33,cs:1<<34,..Default::default()},dc:35,busy:37}}
    fn mini()->Self {Self{board:LilygoEpaper::mini_epaper_s3(),gpio:Gpio::new(),mux:RegRam::new(),cycle:0,pins:SpiPins{sclk:1<<14,mosi:1<<15,cs:1<<13,..Default::default()},dc:12,busy:10}}
    fn drive(&mut self,pin:u8,level:bool) {self.gpio.func_out_sel[usize::from(pin)]=256;self.mux.write(4+4*u32::from(pin),1<<12);self.gpio.enable|=1<<pin;if level {self.gpio.out|=1<<pin;}else{self.gpio.out&=!(1<<pin);}self.sample();}
    fn sample(&mut self) {self.board.gpio_waveform_at(self.cycle,ChipPins::S3.routes(&self.gpio,&self.mux));}
    fn cmd_on(&mut self,command:u8,data:&[u8],pins:SpiPins) {self.drive(self.dc,false);self.board.spi_transfer_pins(2,pins,&[command],0);self.drive(self.dc,true);self.board.spi_transfer_pins(3,pins,data,0);}
    fn cmd(&mut self,command:u8,data:&[u8]) {self.cmd_on(command,data,self.pins);}
    fn finish(&mut self) {while let Some(at)=self.board.next_deadline() {assert!(at>=self.cycle);self.cycle=at;self.board.advance_to(at);}}
    fn setup_deck(&mut self) {self.cmd(0,[0x1e,0x0d].as_slice());self.cmd(0,[0x1f,0x0d].as_slice());}
    fn power(&mut self) {self.cmd(4,&[]);self.finish();}
    fn refresh(&mut self) {self.cmd(0x12,&[]);self.finish();}
    fn pixels(&self)->Vec<u16> {self.board.display().unwrap().2}
}
fn write8(d:&mut dyn I2cDevice,reg:u8,value:u8) {assert!(d.start(false));assert!(d.write(reg));assert!(d.write(value));d.stop();}
fn read8(d:&mut dyn I2cDevice,reg:u8)->u8 {assert!(d.start(false));assert!(d.write(reg));assert!(d.start(true));let value=d.read();d.stop();value}
fn pointer(d:&mut dyn I2cDevice,reg:u16) {assert!(d.start(false));for byte in reg.to_be_bytes() {assert!(d.write(byte));}}

#[test]
fn deck_requires_exact_spi_routes_exclusive_selection_and_routed_dc() {
    let mut r=Rig::deck();
    // A GPIO latch callback alone cannot substitute for an enabled GPIO route.
    r.board.gpio_output_at(0,&[(35,false)],1<<35,0);
    r.board.spi_transfer_pins(2,r.pins,&[0],0);r.board.gpio_output_at(0,&[(35,true)],1<<35,1<<35);r.board.spi_transfer_pins(2,r.pins,&[0x1f,0x0d],0);
    r.cmd(0x13,&[0]);r.power();r.refresh();assert_eq!(r.board.display_frames(),0);
    r.setup_deck();
    for pins in [SpiPins{sclk:1<<14,..r.pins},SpiPins{mosi:1<<15,..r.pins},SpiPins{cs:1<<48,..r.pins},SpiPins{cs:(1<<34)|(1<<3),..r.pins}] {r.cmd_on(0x13,&[0],pins);}
    r.power();r.refresh();assert!(r.pixels().iter().all(|&p|p==0xffff));
    r.cmd(0x13,&[0x7f]);r.refresh();assert_eq!(&r.pixels()[..9],&[0,0xffff,0xffff,0xffff,0xffff,0xffff,0xffff,0xffff,0xffff]);
    assert!(r.board.report().contains("factory-boot=unverified"));
}

#[test]
fn ram_is_not_a_frame_and_busy_completion_publishes_one_frozen_image() {
    let mut r=Rig::deck();r.setup_deck();r.cmd(0x13,&[0x80,0x00]);assert_eq!(r.board.display_version(),0);assert!(r.pixels().iter().all(|&p|p==0xffff));
    r.cmd(0x12,&[]);assert_eq!(r.board.next_deadline(),None,"display refresh requires panel power");
    r.cmd(4,&[]);assert!(r.board.input_levels().contains(&(r.busy,false)));assert_eq!(r.board.display_frames(),0);r.finish();assert_eq!(r.board.display_frames(),0,"power-on is not a refresh");
    r.board.take_edges();r.cmd(0x12,&[]);let deadline=r.board.next_deadline().unwrap();
    assert_eq!(deadline-r.cycle,periph::CPU_HZ*1100/1000);assert_eq!(r.board.take_edges(),[BoardEdge{cycle:r.cycle,pin:37,level:false}]);
    r.cmd(0x13,&[0xff,0xff]);r.board.advance_to(deadline-1);assert_eq!(r.board.display_frames(),0);
    r.cycle=deadline;r.board.advance_to(deadline);assert_eq!(r.board.display_frames(),1);assert_eq!(r.board.take_edges(),[BoardEdge{cycle:deadline,pin:37,level:true}]);
    let frame=r.board.display().unwrap();assert_eq!((frame.0,frame.1),(240,320));assert_eq!(frame.2[0],0xffff);assert!(frame.2[1..16].iter().all(|&p|p==0));
    r.board.advance_to(deadline+periph::CPU_HZ);assert_eq!(r.board.display_frames(),1,"no duplicate refresh publication");
}

#[test]
fn uc8253_partial_window_uses_sixteen_bit_rows_and_rejects_malformed_areas() {
    let mut r=Rig::deck();r.setup_deck();r.power();
    r.cmd(0x10,&[0;30]);r.refresh();assert!(r.pixels().iter().all(|&p|p==0xffff),"previous RAM cannot draw the current frame");
    r.cmd(0x91,&[]);r.cmd(0x90,&[8,15,1,44,1,45,1]);r.cmd(0x13,&[0x7f,0xfe,0]);r.refresh();
    let p=r.pixels();assert_eq!(p[300*240+8],0);assert_eq!(p[301*240+15],0);assert_eq!(p.iter().filter(|&&v|v==0).count(),2,"RAM stream is bounded by the window");
    for window in [[240,255,0,0,0,1,1],[16,8,0,0,0,1,1],[0,7,1,64,1,65,1],[0,7,0,2,0,1,1]] {r.cmd(0x90,&window);r.cmd(0x13,&[0;128]);r.refresh();}
    assert_eq!(r.pixels(),p);assert_eq!(r.board.display_frames(),2,"invalid windows never begin a refresh");
    r.cmd(0x90,&[0,7]);r.cmd(0x13,&[0]);r.refresh();assert_eq!(r.pixels(),p,"truncated window is not silently reused");
}

#[test]
fn mini_uses_its_own_window_format_supply_reset_and_retained_optical_image() {
    let mut r=Rig::mini();r.cmd(0,&[0x5f]);r.cmd(0x13,&[0]);r.power();r.refresh();assert_eq!(r.board.display_frames(),0,"EN42 must be driven high");
    r.drive(42,true);r.drive(11,true);r.cmd(0,&[0x5f]);r.power();r.cmd(0x91,&[]);r.cmd(0x90,&[72,79,127,127,1]);r.cmd(0x13,&[0xfe]);r.refresh();
    let frame=r.board.display().unwrap();assert_eq!((frame.0,frame.1),(80,128));assert_eq!(*frame.2.last().unwrap(),0);assert_eq!(frame.2.iter().filter(|&&p|p==0).count(),1);
    r.cmd(2,&[]);r.finish();r.cmd(7,&[0xa5]);r.cmd(0,&[0x5f]);r.power();r.cmd(0x13,&[0]);r.refresh();assert_eq!(r.board.display_frames(),1,"deep sleep requires a hardware reset");
    r.drive(11,false);r.drive(11,true);r.cmd(0,&[0x5f]);r.power();r.cmd(0x13,&[0]);r.cmd(0x12,&[]);assert!(r.board.next_deadline().is_some());
    r.drive(42,false);assert_eq!(r.board.next_deadline(),None);assert_eq!(r.pixels(),frame.2,"power loss cancels pending updates but retains the optical image");
    assert!(r.board.input_levels().contains(&(10,true)));
}

#[test]
fn keyboard_is_the_official_thirty_five_switch_matrix_with_scan_and_irq() {
    let mut r=Rig::deck();let mut devices=r.board.i2c_devices();assert_eq!(devices.len(),4);assert!(devices.iter().all(|(_,_,d)|d.pins()==Some((13,14))));
    let d=devices.iter_mut().find(|(bus,addr,_)|*bus==0 && *addr==0x34).unwrap().2.as_mut();
    for (reg,value) in [(0x1d,15),(0x1e,255),(0x1f,3),(1,1)] {write8(d,reg,value);}
    assert!(!r.board.matrix_key_at(0,3,0,true));assert!(!r.board.matrix_key_at(0,4,0,true));assert!(!r.board.matrix_key_at(0,0,10,true));
    assert!(r.board.matrix_key_at(0,0,0,true));r.board.advance_to(periph::CPU_HZ/40);assert_eq!(read8(d,3),0);
    r.cycle=periph::CPU_HZ/20;r.board.advance_to(r.cycle);assert_eq!(read8(d,4),0x8a,"Q is physical row0,column0, TCA row0,column9");
    assert!(r.board.take_edges().contains(&BoardEdge{cycle:r.cycle,pin:15,level:false}));write8(d,2,1);assert!(r.board.take_edges().contains(&BoardEdge{cycle:r.cycle,pin:15,level:true}));
    r.board.matrix_key_at(r.cycle,0,0,false);r.cycle=periph::CPU_HZ/10;r.board.advance_to(r.cycle);assert_eq!(read8(d,4),10);
    let mut count=0;for row in 0..4 {for col in 0..10 {count+=usize::from(r.board.matrix_key_at(r.cycle,row,col,false));}}assert_eq!(count,35);
}

#[test]
fn cst328_has_a_sixteen_bit_report_ack_and_reset_with_no_fabricated_identity() {
    let mut r=Rig::deck();let mut devices=r.board.i2c_devices();let d=devices.iter_mut().find(|(bus,addr,_)|*bus==0 && *addr==0x1a).unwrap().2.as_mut();
    pointer(d,0xd1f4);assert!(!d.start(true),"unmodeled firmware identity is not acknowledged");
    r.board.touch_at(12,239,319,true);pointer(d,0xd000);assert!(d.start(true));let packet:Vec<u8>=(0..7).map(|_|d.read()).collect();assert_eq!(packet,[6,14,19,255,1,1,0xab]);
    assert_eq!(r.board.touch_irq(),Some((12,false)),"reading is not the report acknowledge");pointer(d,0xd000);assert!(d.write(0xab));d.stop();assert_eq!(r.board.touch_irq(),Some((12,true)));pointer(d,0xd000);assert!(d.start(true));assert_eq!(d.read(),0xab,"acknowledged reports are consumed");
    r.board.touch_at(13,500,500,false);pointer(d,0xd000);assert!(d.start(true));let released:Vec<u8>=(0..7).map(|_|d.read()).collect();assert_eq!(released,[0,14,19,255,0,0,0xab]);
    r.cycle=14;r.drive(45,false);assert!(!d.start(false));r.board.touch(10,10,true);assert_eq!(r.board.touch_irq(),Some((12,true)));r.drive(45,true);assert!(d.start(false));
}

#[test]
fn mini_gpio_buttons_do_not_invent_touch_keyboard_or_sensor_devices() {
    let mut b=LilygoEpaper::mini_epaper_s3();assert!(b.i2c_devices().is_empty());assert_eq!(b.touch_irq(),None);assert!(!b.matrix_key_at(0,0,0,true));
    assert_eq!(b.named_pin("right"),Some(3));assert_eq!(b.named_pin("left"),Some(4));assert!(b.button_at(10,"left",true));assert_eq!(b.take_edges(),[BoardEdge{cycle:10,pin:4,level:false}]);assert!(!b.button_at(10,"power",true));
    assert!(b.button_at(20,"left",false));assert_eq!(b.take_edges(),[BoardEdge{cycle:20,pin:4,level:true}]);
}

fn crc7(bytes:&[u8])->u8 {let mut crc=0u8;for &value in bytes {let mut byte=value;for _ in 0..8 {crc<<=1;if (byte^crc)&0x80!=0 {crc^=9;}byte<<=1;}}crc&0x7f}
fn sd_command(b:&mut LilygoEpaper,pins:SpiPins,cmd:u8,arg:u32)->Vec<u8> {let mut packet=vec![0x40|cmd];packet.extend(arg.to_be_bytes());packet.push(crc7(&packet)<<1|1);b.spi_transfer_pins(2,pins,&packet,0);b.spi_transfer_pins(2,pins,&[],2)}
#[test]
fn both_spi_sockets_route_real_card_storage_and_reject_wrong_read_pins() {
    for (mut b,pins) in [(LilygoEpaper::t_deck_pro(),SpiPins{sclk:1<<36,mosi:1<<33,miso:Some(47),cs:1<<48,keep_cs:false}),(LilygoEpaper::mini_epaper_s3(),SpiPins{sclk:1<<41,mosi:1<<39,miso:Some(38),cs:1<<40,keep_cs:false})] {
        b.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),Default::default()).unwrap();b.spi_transfer_pins(2,SpiPins{cs:0,..pins},&[255;10],0);
        assert_eq!(sd_command(&mut b,pins,0,0),[255,1]);assert_eq!(sd_command(&mut b,pins,8,0x1aa),[255,1]);assert_eq!(b.spi_transfer_pins(2,pins,&[],4),[0,0,1,0xaa]);
        for _ in 0..2 {sd_command(&mut b,pins,55,0);sd_command(&mut b,pins,41,1<<30);}
        assert_eq!(sd_command(&mut b,pins,24,3),[255,0]);let mut block=vec![0xfe];block.extend([0x5a;512]);block.extend([255,255]);b.spi_transfer_pins(2,pins,&block,0);assert_eq!(b.spi_transfer_pins(2,pins,&[],1),[5]);
        assert_eq!(sd_command(&mut b,pins,17,3),[255,0]);assert_eq!(b.spi_transfer_pins(2,pins,&[],2),[255,0xfe]);assert_eq!(b.spi_transfer_pins(2,pins,&[],512),vec![0x5a;512]);b.spi_transfer_pins(2,pins,&[],2);
        assert_eq!(sd_command(&mut b,SpiPins{miso:Some(1),..pins},58,0),[255,255]);b.set_sd_card_present(false);assert_eq!(sd_command(&mut b,pins,0,0),[255,255]);
    }
}
