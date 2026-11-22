//! Independent-pin contracts for additional exact sales/hardware variants.
//! Path imports permit verification before the root board registry is updated.
use esp32s3::{periph,board::tdeck};
#[allow(dead_code)] // The reused Mini constructor is outside this T-Deck test.
#[path="../src/board/lilygo_epaper.rs"]
mod lilygo_epaper;
#[path="../src/board/lilygo_additional.rs"]
mod lilygo_additional;
use emu_core::Core;
use esp_periph::{Gpio,RegRam,i2c::I2cDevice,sdmmc::MemStore};
use esp_soc::{Stop,board::{Board,SpiPins},pins::ChipPins,web::WebServer};

const TFT:[&str;3]=["lilygo-t-deck-meshtastic","lilygo-t-deck-plus-meshtastic","lilygo-t-deck-plus-1"];
const PRO:&str="lilygo-t-deck-pro-meshtastic";
const IRAM:u32=0x4037_0000;
const GPIO:u32=0x6000_4000;
const SPI:u32=0x6002_4000;
fn machine(id:&str)->esp32s3::Machine {
    let mut m=esp32s3::machine([0;6]);m.bus.board=lilygo_additional::make_board(id).unwrap();m.bus.attach_board_devices();
    m.bus.load_bytes(IRAM,&[0x22,0x63,0,0x06,0xff,0xff,0,0,0x22,0x23,0,0x06,0xff,0xff]).unwrap();m.cores[0].ps=0;m.cores[0].set_jit(false);m
}
fn write(m:&mut esp32s3::Machine,address:u32,value:u32) {m.cores[0].pc=IRAM;m.cores[0].set_ar(2,value);m.cores[0].set_ar(3,address);assert!(matches!(m.run(64),Stop::MaxInsns));}
fn mode(m:&mut esp32s3::Machine,pin:u8,signal:u32) {write(m,0x6000_9004+4*u32::from(pin),1<<12|1<<9);write(m,GPIO+0x554+4*u32::from(pin),signal);if signal==256 {write(m,GPIO+if pin<32{0x24}else{0x30},1<<(pin%32));}}
fn output(m:&mut esp32s3::Machine,pin:u8,high:bool) {write(m,GPIO+if pin<32{if high{8}else{12}}else if high{20}else{24},1<<(pin%32));}
fn transfer(m:&mut esp32s3::Machine,bytes:&[u8]) {
    if bytes.is_empty(){return;}write(m,SPI+0x10,1<<27);write(m,SPI+0x1c,(bytes.len()*8-1) as u32);
    for (i,chunk) in bytes.chunks(4).enumerate() {let value=chunk.iter().enumerate().fold(0,|v,(j,b)|v|(u32::from(*b)<<(j*8)));write(m,SPI+0x98+4*i as u32,value);}write(m,SPI,1<<24);
}
fn command(m:&mut esp32s3::Machine,c:u8,data:&[u8]) {output(m,11,false);transfer(m,&[c]);output(m,11,true);transfer(m,data);}

#[test]
fn distinct_profiles_never_silently_cover_other_revisions_or_chipsets() {
    for id in TFT.into_iter().chain([PRO]) {let b=lilygo_additional::make_board(id).unwrap();assert_eq!(b.name(),id);assert!(b.report().contains("factory/Meshtastic-boot=unverified"));assert_eq!(b.lora_snapshot().is_some(),id!=PRO);assert!(b.audio().0.is_empty());assert_eq!(b.sd_card_slot(),None);assert!(!b.uses_uart_pins());}
    for id in ["lilygo-t-deck-v2","lilygo-t-deck-pro-v1.1","lilygo-t-deck-pro-max","lilygo-t-deck-without-lora","esp32s3"] {assert!(lilygo_additional::make_board(id).is_none());}
}

#[test]
fn real_guest_spi_and_gpio_use_exact_tft_routes_power_and_controls() {
    for id in TFT {
        let mut m=machine(id);
        for (pin,signal) in [(40,101),(41,103),(12,110)] {mode(&mut m,pin,signal);}write(&mut m,SPI+0x20,0x3e);
        for pin in [10,42] {mode(&mut m,pin,256);output(&mut m,pin,true);}
        for (c,data) in [(0x11,vec![]),(0x29,vec![]),(0x3a,vec![0x55])] {command(&mut m,c,&data);}
        assert_eq!(m.bus.board.display_frames(),0,"unrouted DC cannot initialize {id}");
        mode(&mut m,11,256);
        for (c,data) in [(0x11,vec![]),(0x29,vec![]),(0x3a,vec![0x55]),(0x36,vec![0x60]),(0x2a,vec![0,0,1,63]),(0x2b,vec![0,0,0,239]),(0x2c,vec![0xf8,0,7,0xe0])] {command(&mut m,c,&data);}
        m.bus.flush_ticks();let frame=m.bus.board.display().unwrap();assert_eq!((frame.0,frame.1),(320,240));assert_eq!(&frame.2[..2],&[0xf800,0x07e0],"{id}: {}",m.bus.board.report());
        mode(&mut m,40,102);command(&mut m,0x2c,&[255,255]);assert_eq!(m.bus.board.display().unwrap().2,frame.2,"wrong SCLK must not write");
        write(&mut m,0x6000_9004+4*10,0);assert!(m.bus.board.display().unwrap().2.iter().all(|p|*p==0),"latched-high G10 without GPIO routing cannot power the display");
        let web=WebServer::queued();m.web=Some(web.clone());
        for pin in [0,1,2,3,15] {write(&mut m,0x6000_9004+4*pin,1<<12|1<<9);web.push_incoming(format!(r#"{{"t":"btn","pin":{pin},"v":1}}"#));m.cores[0].pc=IRAM+8;m.cores[0].set_ar(3,GPIO+0x3c);assert!(matches!(m.run(64),Stop::MaxInsns));assert_eq!(m.cores[0].get_ar(2)&(1<<pin),0);web.push_incoming(format!(r#"{{"t":"btn","pin":{pin},"v":0}}"#));m.run(0);m.cores[0].pc=IRAM+8;assert!(matches!(m.run(64),Stop::MaxInsns));assert_ne!(m.cores[0].get_ar(2)&(1<<pin),0);}
    }
}

struct Rig {board:Board,gpio:Gpio,mux:RegRam,cycle:u64}
impl Rig {
    fn new(id:&str)->Self {Self{board:lilygo_additional::make_board(id).unwrap(),gpio:Gpio::new(),mux:RegRam::new(),cycle:0}}
    fn drive(&mut self,pin:u8,high:bool) {self.gpio.func_out_sel[usize::from(pin)]=256;self.mux.write(4+4*u32::from(pin),1<<12);self.gpio.enable|=1<<pin;if high{self.gpio.out|=1<<pin;}else{self.gpio.out&=!(1<<pin);}self.board.gpio_waveform_at(self.cycle,ChipPins::S3.routes(&self.gpio,&self.mux));}
    fn cmd(&mut self,c:u8,data:&[u8]) {let pins=SpiPins{sclk:1<<36,mosi:1<<33,cs:1<<34,..Default::default()};self.drive(35,false);self.board.spi_transfer_pins(2,pins,&[c],0);self.drive(35,true);self.board.spi_transfer_pins(2,pins,data,0);}
    fn finish(&mut self) {while let Some(at)=self.board.next_deadline() {self.cycle=at;self.board.advance_to(at);}}
}
fn pointer(d:&mut dyn I2cDevice,address:u16) {assert!(d.start(false));for byte in address.to_be_bytes(){assert!(d.write(byte));}}

#[test]
fn tft_keyboard_scans_and_gt911_touch_keep_their_real_i2c_protocols() {
    for id in TFT {
        let mut r=Rig::new(id);let mut devices=r.board.i2c_devices();assert!(devices.iter_mut().all(|(_,_,d)|!d.start(true)));r.drive(10,true);assert!(devices.iter().all(|(_,_,d)|d.pins()==Some((18,8))));
        let k=devices.iter_mut().find(|(bus,addr,_)|*bus==0&&*addr==0x55).unwrap().2.as_mut();
        assert!(r.board.matrix_key_at(0,0,0,true));assert!(!r.board.matrix_key_at(0,3,0,true));r.board.advance_to(periph::CPU_HZ*35/1000-1);assert!(k.start(true));assert_eq!(k.read(),0);r.cycle=periph::CPU_HZ*35/1000;r.board.advance_to(r.cycle);assert!(k.start(true));assert_eq!(k.read(),b'q');assert!(k.start(true));assert_eq!(k.read(),0);
        let t=devices.iter_mut().find(|(bus,addr,_)|*bus==0&&*addr==0x5d).unwrap().2.as_mut();pointer(t,0x814e);assert!(t.write(0));r.board.touch_at(r.cycle,20,30,true);r.cycle+=periph::CPU_HZ/100;r.board.advance_to(r.cycle);pointer(t,0x814e);assert!(t.start(true));assert_eq!(t.read(),0x81);pointer(t,0x8150);assert!(t.start(true));assert_eq!((0..4).map(|_|t.read()).collect::<Vec<_>>(),[209,0,20,0]);
        pointer(t,0x814e);assert!(t.write(0));r.board.touch_at(r.cycle,20,30,false);r.cycle+=periph::CPU_HZ/100;r.board.advance_to(r.cycle);pointer(t,0x814e);assert!(t.start(true));assert_eq!(t.read(),0x80);r.drive(10,false);assert!(devices.iter_mut().all(|(_,_,d)|!d.start(true)));
    }
}

#[test]
fn pro_meshtastic_v1_keeps_epaper_busy_and_cst328_ack_semantics() {
    let mut r=Rig::new(PRO);r.cmd(0,&[0x1e,0x0d]);r.cmd(0,&[0x1f,0x0d]);r.cmd(0x13,&[0x7f]);assert_eq!(r.board.display_version(),0);
    r.cmd(4,&[]);assert!(r.board.input_levels().contains(&(37,false)));r.finish();r.cmd(0x12,&[]);let end=r.board.next_deadline().unwrap();assert_eq!(end-r.cycle,periph::CPU_HZ*1100/1000);assert!(r.board.display().unwrap().2.iter().all(|p|*p==0xffff));r.finish();let frame=r.board.display().unwrap();assert_eq!((frame.0,frame.1),(240,320));assert_eq!(&frame.2[..2],&[0,0xffff]);assert_eq!(r.board.display_push_hz(),8);
    let mut devices=r.board.i2c_devices();assert!(devices.iter().all(|(_,_,d)|d.pins()==Some((13,14))));let t=devices.iter_mut().find(|(bus,addr,_)|*bus==0&&*addr==0x1a).unwrap().2.as_mut();r.board.touch_at(r.cycle,239,319,true);pointer(t,0xd000);assert!(t.start(true));assert_eq!((0..7).map(|_|t.read()).collect::<Vec<_>>(),[6,14,19,255,1,1,0xab]);assert_eq!(r.board.touch_irq(),Some((12,false)));pointer(t,0xd000);assert!(t.write(0xab));assert_eq!(r.board.touch_irq(),Some((12,true)));assert!(r.board.button_at(r.cycle,"boot",true));assert!(r.board.input_levels().contains(&(0,false)));
}

fn crc7(bytes:&[u8])->u8 {let mut crc=0u8;for &value in bytes {let mut byte=value;for _ in 0..8 {crc<<=1;if (byte^crc)&0x80!=0 {crc^=9;}byte<<=1;}}crc&0x7f}
fn sd_command(board:&mut Board,pins:SpiPins,cmd:u8,arg:u32)->Vec<u8> {let mut packet=vec![0x40|cmd];packet.extend(arg.to_be_bytes());packet.push(crc7(&packet)<<1|1);board.spi_transfer_pins(2,pins,&packet,0);board.spi_transfer_pins(2,pins,&[],2)}
#[test]
fn all_four_sd_sockets_read_written_blocks_with_exact_routes_and_selection() {
    for id in TFT.into_iter().chain([PRO]) {
        let mut r=Rig::new(id);let pins=if id==PRO{SpiPins{sclk:1<<36,mosi:1<<33,miso:Some(47),cs:1<<48}}else{r.drive(10,true);SpiPins{sclk:1<<40,mosi:1<<41,miso:Some(38),cs:1<<39}};
        r.board.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),Default::default()).unwrap();r.board.spi_transfer_pins(2,SpiPins{cs:0,..pins},&[255;10],0);assert_eq!(sd_command(&mut r.board,pins,0,0),[255,1]);assert_eq!(sd_command(&mut r.board,pins,8,0x1aa),[255,1]);assert_eq!(r.board.spi_transfer_pins(2,pins,&[],4),[0,0,1,0xaa]);
        for _ in 0..2 {sd_command(&mut r.board,pins,55,0);sd_command(&mut r.board,pins,41,1<<30);}
        assert_eq!(sd_command(&mut r.board,pins,24,3),[255,0]);let mut block=vec![0xfe];block.extend([0x5a;512]);block.extend([255,255]);r.board.spi_transfer_pins(2,pins,&block,0);assert_eq!(r.board.spi_transfer_pins(2,pins,&[],1),[5]);assert_eq!(sd_command(&mut r.board,pins,17,3),[255,0]);assert_eq!(r.board.spi_transfer_pins(2,pins,&[],2),[255,0xfe]);assert_eq!(r.board.spi_transfer_pins(2,pins,&[],512),vec![0x5a;512]);r.board.spi_transfer_pins(2,pins,&[],2);
        assert_eq!(sd_command(&mut r.board,SpiPins{miso:Some(1),..pins},58,0),[255,255]);let other_cs=if id==PRO{34}else{12};assert_eq!(sd_command(&mut r.board,SpiPins{cs:pins.cs|1<<other_cs,..pins},0,0),[255,255]);r.board.set_sd_card_present(false);assert_eq!(sd_command(&mut r.board,pins,0,0),[255,255]);
    }
}
