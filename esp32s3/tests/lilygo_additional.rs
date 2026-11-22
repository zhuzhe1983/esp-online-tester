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

#[test]
fn tft_deck_sx1262_packets_require_routed_power_reset_spi_and_matching_peer() {
    use esp_soc::devices::sx1262::{LoRaConfig,LoRaInput};
    let packet=LoRaInput{config:LoRaConfig{frequency:0x36400000,sf:12,bandwidth:3,coding_rate:4,sync_word:0x3444,implicit_header:false,crc:true,invert_iq:false},payload:b"peer".to_vec(),rssi_dbm:-70,snr_quarter_db:24,crc_error:false};
    for id in TFT {
        let mut r=Rig::new(id);let pins=SpiPins{sclk:1<<40,mosi:1<<41,miso:Some(38),cs:1<<9};
        assert!(!r.board.lora_receive_at(0,&packet));r.drive(9,true);r.drive(10,true);r.drive(17,true);r.finish();
        let command=|r:&mut Rig,bytes:&[u8]| {r.drive(9,false);let rx=r.board.spi_transfer_pins(2,pins,bytes,bytes.len());r.drive(9,true);r.finish();rx};
        for bytes in [&[0x80,0][..],&[0x8a,1],&[0x86,0x36,0x40,0,0],&[0x8b,12,3,4,1],&[0x8c,0,10,0,5,1,0],&[0x0d,7,0x40,0x34,0x44],&[0x08,3,0xff,3,0xff,0,0,0,0]] {command(&mut r,bytes);}
        r.drive(9,false);assert_eq!(r.board.spi_transfer_pins(2,SpiPins{sclk:1<<39,..pins},&[0x82,0xff,0xff,0xff],4),[255;4]);r.drive(9,true);assert!(!r.board.lora_receive_at(r.cycle,&packet),"wrong clock cannot arm RX");
        r.drive(9,false);assert_eq!(r.board.spi_transfer_pins(2,SpiPins{cs:(1<<9)|(1<<12),..pins},&[0x82,0xff,0xff,0xff],4),[255;4]);r.drive(9,true);assert!(!r.board.lora_receive_at(r.cycle,&packet),"shared CS contention cannot arm RX");
        command(&mut r,&[0x82,0xff,0xff,0xff]);let wrong=LoRaInput{config:LoRaConfig{frequency:1,..packet.config.clone()},payload:packet.payload.clone(),rssi_dbm:-70,snr_quarter_db:24,crc_error:false};assert!(!r.board.lora_receive_at(r.cycle,&wrong));assert!(r.board.lora_receive_at(r.cycle,&packet));assert!(!r.board.input_levels().contains(&(45,true)));r.finish();assert!(r.board.input_levels().contains(&(45,true)));assert_eq!(&command(&mut r,&[0x13,0,0,0])[2..],[4,0]);assert_eq!(&command(&mut r,&[0x1e,0,0,0,0,0,0])[3..],b"peer");assert!(r.board.lora_snapshot().unwrap().contains("\"rxCount\":1"));assert!(r.board.lora_snapshot().unwrap().contains("\"physicalRf\":false"));
        command(&mut r,&[0x02,0xff,0xff]);assert!(r.board.input_levels().contains(&(45,false)));command(&mut r,&[0x0e,0,b'H',b'e',b'l',b'l',b'o']);command(&mut r,&[0x83,0,0,0]);let snapshot=r.board.lora_snapshot().unwrap();assert!(snapshot.contains("\"payloadHex\":\"48656c6c6f\""));assert!(snapshot.contains("\"txCount\":1"));
        r.drive(10,false);assert!(!r.board.lora_receive_at(r.cycle,&packet));assert!(r.board.lora_snapshot().unwrap().contains("\"config\":null"));r.drive(10,true);r.finish();assert!(!r.board.lora_receive_at(r.cycle,&packet),"power cycle loses RX configuration");r.drive(17,false);assert!(!r.board.lora_receive_at(r.cycle,&packet));
    }
}

const OLED:&[(&str,u8,u8,u8,bool)]=&[
    ("lilygo-t-beam-bpf",8,9,2,false),("lilygo-t3-s3-v1-3",18,17,0,false),
    ("lilygo-t-beam-supreme-meshtastic",17,18,2,true),("lilygo-t-beam-1w",8,9,2,false),
    ("lilygo-t3-s3-lr1121",18,17,0,false),("lilygo-t-beam-supreme",17,18,2,true),
    ("lilygo-t3s3-v1-0",18,17,0,false),("lilygo-t3-s3-meshtastic",18,17,0,false),
];
fn guest_read(m:&mut esp32s3::Machine,address:u32)->u32 {m.cores[0].pc=IRAM+8;m.cores[0].set_ar(3,address);assert!(matches!(m.run(64),Stop::MaxInsns));m.cores[0].get_ar(2)}
fn i2c_route(m:&mut esp32s3::Machine,sda:u8,scl:u8,controller:u32) {for (pin,signal) in [(sda,90+controller*2),(scl,89+controller*2)] {mode(m,pin,signal);write(m,GPIO+0x154+4*signal,0x80|u32::from(pin));}}
fn i2c_write(m:&mut esp32s3::Machine,controller:u32,address:u8,payload:&[u8])->bool {
    assert!(payload.len()<31);let base=if controller==0 {0x6001_3000}else{0x6002_7000};
    for (off,value) in [(0x24,u32::MAX),(0x18,3<<12),(0x1c,u32::from(address)<<1)] {write(m,base+off,value);}for &b in payload {write(m,base+0x1c,u32::from(b));}
    for (off,value) in [(0x58,6<<11),(0x5c,1<<11|1<<8|(payload.len()+1) as u32),(0x60,2<<11),(0x04,1<<5)] {write(m,base+off,value);}
    assert!(matches!(m.run(100_000),Stop::MaxInsns));guest_read(m,base+0x20)&(1<<10)==0
}
fn boot_probe(m:&mut esp32s3::Machine,pins:&[u8]) {
    let web=WebServer::queued();m.web=Some(web.clone());for &pin in pins {write(m,0x6000_9004+4*u32::from(pin),1<<12|1<<9);web.push_incoming(format!(r#"{{"t":"btn","pin":{pin},"v":1}}"#));assert_eq!(guest_read(m,GPIO+0x3c)&(1<<pin),0);web.push_incoming(format!(r#"{{"t":"btn","pin":{pin},"v":0}}"#));assert_ne!(guest_read(m,GPIO+0x3c)&(1<<pin),0);}
}
#[test]
fn oled_boards_execute_real_xtensa_i2c_and_gpio_with_wrong_route_and_power_rejection() {
    for &(id,sda,scl,offset,pmu_display) in OLED {
        let mut m=machine(id);i2c_route(&mut m,sda,scl,0);
        if pmu_display {assert!(!i2c_write(&mut m,0,0x3c,&[0,0xaf]));i2c_route(&mut m,42,41,1);assert!(i2c_write(&mut m,1,0x34,&[0x90,0x11]));}
        assert!(i2c_write(&mut m,0,0x3c,&[0,0xaf,0xb0,offset,0x10]),"{id}");assert!(i2c_write(&mut m,0,0x3c,&[0x40,1]));let frame=m.bus.board.display().unwrap();assert_eq!((frame.0,frame.1),(128,64));assert_eq!(frame.2.iter().filter(|&&p|p!=0).count(),1);assert_eq!(frame.2[0],65535,"{id}");
        i2c_route(&mut m,sda,48,0);assert!(!i2c_write(&mut m,0,0x3c,&[0,0xa5]));assert_eq!(m.bus.board.display().unwrap().2,frame.2);i2c_route(&mut m,sda,scl,0);assert!(!i2c_write(&mut m,0,0x3d,&[0,0xa5]));
        if id=="lilygo-t-beam-bpf" {boot_probe(&mut m,&[0,3]);}else if id=="lilygo-t-beam-1w" {boot_probe(&mut m,&[0,17]);}else {boot_probe(&mut m,&[0]);}
        if let Some((pin,_))=m.bus.board.leds().map(|_|(if id=="lilygo-t-beam-1w"{18}else{37},true)) {mode(&mut m,pin,256);output(&mut m,pin,true);assert_eq!(m.bus.board.leds().unwrap().0[0],[255;3]);write(&mut m,0x6000_9004+4*u32::from(pin),0);assert_eq!(m.bus.board.leds().unwrap().0[0],[0;3]);}
        if pmu_display {assert!(i2c_write(&mut m,1,0x34,&[0x90,0x10]));assert!(!i2c_write(&mut m,0,0x3c,&[0,0xaf]));assert!(m.bus.board.display().unwrap().2.iter().all(|&p|p==0));}
        if id=="lilygo-t-beam-bpf" {assert!(i2c_write(&mut m,0,0x34,&[0x80,0]));assert!(!i2c_write(&mut m,0,0x3c,&[0,0xaf]));assert!(m.bus.board.display().unwrap().2.iter().all(|&p|p==0));assert!(i2c_write(&mut m,0,0x34,&[0x80,1]));assert!(i2c_write(&mut m,0,0x3c,&[0,0xaf]));assert!(m.bus.board.display().unwrap().2.iter().all(|&p|p==0),"power cycle clears undefined display RAM deterministically");}
        assert!(m.bus.board.lora_snapshot().is_none());assert!(!m.bus.board.uses_uart_pins());
    }
}
#[test]
fn exact_can_board_exposes_only_documented_boot_input_and_no_fake_can_display_or_sd() {
    let mut m=machine("lilygo-t-2can");assert_eq!(m.bus.board.name(),"lilygo-t-2can");assert!(m.bus.board.display().is_none());assert!(m.bus.board.i2c_devices().is_empty());assert!(m.bus.board.leds().is_none());assert!(m.bus.board.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),Default::default()).is_err());boot_probe(&mut m,&[0]);assert!(m.bus.board.report().contains("MCP2515/TWAI/CAN PHY absent"));
    assert!(lilygo_additional::make_board("lilygo-t-radar").is_none());assert!(lilygo_additional::make_board("lilygo-t3-tcxo").is_none());
}
fn power_register(board:&mut Board,register:u8,value:u8) {let mut devices=board.i2c_devices();let d=&mut devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x34).unwrap().2;assert!(d.start(false));assert!(d.write(register));assert!(d.write(value));d.stop();}
#[test]
fn oled_board_sd_roundtrips_and_pmu_rails_reset_protocol_without_erasing_storage() {
    for &(id,_,_,_,supreme) in OLED {
        let mut b=lilygo_additional::make_board(id).unwrap();let bpf=id=="lilygo-t-beam-bpf";let one=id=="lilygo-t-beam-1w";let (clk,mosi,miso,cs)=if supreme {(36,35,37,47)}else if bpf{(12,11,13,10)}else if one{(13,11,12,10)}else{(14,11,2,13)};let pins=SpiPins{sclk:1<<clk,mosi:1<<mosi,miso:Some(miso),cs:1<<cs};
        b.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),Default::default()).unwrap();if supreme||bpf {assert_eq!(sd_command(&mut b,pins,0,0),[255,255]);power_register(&mut b,0x90,0x13);}
        b.spi_transfer_pins(2,SpiPins{cs:0,..pins},&[255;10],0);assert_eq!(sd_command(&mut b,pins,0,0),[255,1]);sd_command(&mut b,pins,8,0x1aa);b.spi_transfer_pins(2,pins,&[],4);for _ in 0..2 {sd_command(&mut b,pins,55,0);sd_command(&mut b,pins,41,1<<30);}
        assert_eq!(sd_command(&mut b,pins,24,7),[255,0]);let mut bytes=vec![0xfe];bytes.extend([0x69;512]);bytes.extend([255,255]);b.spi_transfer_pins(2,pins,&bytes,0);assert_eq!(b.spi_transfer_pins(2,pins,&[],1),[5]);
        if supreme||bpf {power_register(&mut b,0x90,0);power_register(&mut b,0x90,0x13);assert_eq!(sd_command(&mut b,pins,17,7),[255,255],"power cycle requires startup clocks even if no SPI access happened while off");b.spi_transfer_pins(2,SpiPins{cs:0,..pins},&[255;10],0);assert_eq!(sd_command(&mut b,pins,0,0),[255,1]);sd_command(&mut b,pins,8,0x1aa);b.spi_transfer_pins(2,pins,&[],4);for _ in 0..2 {sd_command(&mut b,pins,55,0);sd_command(&mut b,pins,41,1<<30);}}
        assert_eq!(sd_command(&mut b,pins,17,7),[255,0]);assert_eq!(b.spi_transfer_pins(2,pins,&[],2),[255,0xfe]);assert_eq!(b.spi_transfer_pins(2,pins,&[],512),vec![0x69;512]);b.spi_transfer_pins(2,pins,&[],2);
        assert_eq!(sd_command(&mut b,SpiPins{miso:Some(0),..pins},58,0),[255,255]);if supreme||bpf||one {let other=if supreme {34}else if bpf{1}else{15};assert_eq!(sd_command(&mut b,SpiPins{cs:pins.cs|1<<other,..pins},0,0),[255,255]);}b.set_sd_card_present(false);assert_eq!(sd_command(&mut b,pins,0,0),[255,255]);
    }
}
