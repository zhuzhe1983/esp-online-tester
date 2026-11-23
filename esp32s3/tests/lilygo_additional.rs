//! Independent-pin contracts for additional exact sales/hardware variants.
//! Path imports permit verification before the root board registry is updated.
use esp32s3::{periph,board::tdeck};
#[allow(dead_code)] // The reused Mini constructor is outside this T-Deck test.
#[path="../src/board/lilygo_epaper.rs"]
mod lilygo_epaper;
#[allow(dead_code)] // Exact additional profiles reuse the private display transport.
#[path="../src/board/lilygo_display.rs"]
mod lilygo_display;
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
        let mut r=Rig::new(id);let pins=if id==PRO{SpiPins{sclk:1<<36,mosi:1<<33,miso:Some(47),cs:1<<48,keep_cs:false}}else{r.drive(10,true);SpiPins{sclk:1<<40,mosi:1<<41,miso:Some(38),cs:1<<39,keep_cs:false}};
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
        let mut r=Rig::new(id);let pins=SpiPins{sclk:1<<40,mosi:1<<41,miso:Some(38),cs:1<<9,keep_cs:false};
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
    let web=WebServer::queued();m.web=Some(web.clone());for &pin in pins {let input=GPIO+if pin<32{0x3c}else{0x40};let mask=1<<(pin%32);write(m,0x6000_9004+4*u32::from(pin),1<<12|1<<9);web.push_incoming(format!(r#"{{"t":"btn","pin":{pin},"v":1}}"#));assert_eq!(guest_read(m,input)&mask,0);web.push_incoming(format!(r#"{{"t":"btn","pin":{pin},"v":0}}"#));assert_ne!(guest_read(m,input)&mask,0);}
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
        let mut b=lilygo_additional::make_board(id).unwrap();let bpf=id=="lilygo-t-beam-bpf";let one=id=="lilygo-t-beam-1w";let (clk,mosi,miso,cs)=if supreme {(36,35,37,47)}else if bpf{(12,11,13,10)}else if one{(13,11,12,10)}else{(14,11,2,13)};let pins=SpiPins{sclk:1<<clk,mosi:1<<mosi,miso:Some(miso),cs:1<<cs,keep_cs:false};
        b.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),Default::default()).unwrap();if supreme||bpf {assert_eq!(sd_command(&mut b,pins,0,0),[255,255]);power_register(&mut b,0x90,0x13);}
        b.spi_transfer_pins(2,SpiPins{cs:0,..pins},&[255;10],0);assert_eq!(sd_command(&mut b,pins,0,0),[255,1]);sd_command(&mut b,pins,8,0x1aa);b.spi_transfer_pins(2,pins,&[],4);for _ in 0..2 {sd_command(&mut b,pins,55,0);sd_command(&mut b,pins,41,1<<30);}
        assert_eq!(sd_command(&mut b,pins,24,7),[255,0]);let mut bytes=vec![0xfe];bytes.extend([0x69;512]);bytes.extend([255,255]);b.spi_transfer_pins(2,pins,&bytes,0);assert_eq!(b.spi_transfer_pins(2,pins,&[],1),[5]);
        if supreme||bpf {power_register(&mut b,0x90,0);power_register(&mut b,0x90,0x13);assert_eq!(sd_command(&mut b,pins,17,7),[255,255],"power cycle requires startup clocks even if no SPI access happened while off");b.spi_transfer_pins(2,SpiPins{cs:0,..pins},&[255;10],0);assert_eq!(sd_command(&mut b,pins,0,0),[255,1]);sd_command(&mut b,pins,8,0x1aa);b.spi_transfer_pins(2,pins,&[],4);for _ in 0..2 {sd_command(&mut b,pins,55,0);sd_command(&mut b,pins,41,1<<30);}}
        assert_eq!(sd_command(&mut b,pins,17,7),[255,0]);assert_eq!(b.spi_transfer_pins(2,pins,&[],2),[255,0xfe]);assert_eq!(b.spi_transfer_pins(2,pins,&[],512),vec![0x69;512]);b.spi_transfer_pins(2,pins,&[],2);
        assert_eq!(sd_command(&mut b,SpiPins{miso:Some(0),..pins},58,0),[255,255]);if supreme||bpf||one {let other=if supreme {34}else if bpf{1}else{15};assert_eq!(sd_command(&mut b,SpiPins{cs:pins.cs|1<<other,..pins},0,0),[255,255]);}b.set_sd_card_present(false);assert_eq!(sd_command(&mut b,pins,0,0),[255,255]);
    }
}

// id, clock, MOSI, CS, D/C, reset, enable, light pin/polarity, x offset, width, height
type ScreenProbe=(&'static str,u8,u8,u8,Option<u8>,Option<u8>,Option<u8>,Option<(u8,bool)>,u16,u16,u16);
const NEW_SCREENS:&[ScreenProbe]=&[
    ("lilygo-t-connect-pro",12,11,21,Some(41),None,None,Some((46,true)),49,222,480),
    ("lilygo-t-circle-s3",15,17,13,Some(16),None,None,Some((18,false)),0,160,160),
    ("lilygo-t-encoder-pro",12,11,10,None,Some(4),Some(3),None,0,390,390),
];
fn screen_command(m:&mut esp32s3::Machine,dc:Option<u8>,c:u8,data:&[u8]) {if let Some(pin)=dc {output(m,pin,false);transfer(m,&[c]);output(m,pin,true);transfer(m,data);}else{let mut bytes=vec![2,0,c,0];bytes.extend(data);transfer(m,&bytes);}}
#[test]
fn three_exact_screen_variants_execute_xtensa_spi_with_power_and_route_gates() {
    for &(id,clock,mosi,cs,dc,reset,enable,light,x,w,h) in NEW_SCREENS {
        let mut m=machine(id);assert_eq!(m.bus.board.name(),id);for (pin,signal) in [(clock,101),(mosi,103),(cs,110)] {mode(&mut m,pin,signal);}write(&mut m,SPI+0x20,0x3e);
        for pin in dc.into_iter().chain(reset).chain(enable) {mode(&mut m,pin,256);output(&mut m,pin,true);}if let Some((p,on))=light {mode(&mut m,p,256);output(&mut m,p,on);}
        for (c,data) in [(0x11,vec![]),(0x29,vec![]),(0x3a,vec![0x55]),(0x2a,[x.to_be_bytes(),x.to_be_bytes()].concat()),(0x2b,vec![0;4]),(0x2c,vec![0xf8,0])] {screen_command(&mut m,dc,c,&data);}
        let last_x=(x+w-1).to_be_bytes();let last_y=(h-1).to_be_bytes();screen_command(&mut m,dc,0x2a,&[last_x[0],last_x[1],last_x[0],last_x[1]]);screen_command(&mut m,dc,0x2b,&[last_y[0],last_y[1],last_y[0],last_y[1]]);screen_command(&mut m,dc,0x2c,&[7,0xe0]);
        let frame=m.bus.board.display().unwrap();assert_eq!((frame.0,frame.1),(u32::from(w),u32::from(h)));assert_eq!(frame.2[0],0xf800,"{id}: {}",m.bus.board.report());assert_eq!(frame.2[frame.2.len()-1],0x07e0);
        mode(&mut m,clock,102);screen_command(&mut m,dc,0x2c,&[255,255]);assert_eq!(m.bus.board.display().unwrap().2,frame.2);mode(&mut m,clock,101);
        if id=="lilygo-t-connect-pro" {mode(&mut m,14,111);write(&mut m,SPI+0x20,0x3c);screen_command(&mut m,dc,0x2c,&[255,255]);assert_eq!(m.bus.board.display().unwrap().2,frame.2,"shared LoRa CS collision must not write display");write(&mut m,SPI+0x20,0x3e);}
        screen_command(&mut m,dc,0x28,&[]);assert!(m.bus.board.display().unwrap().2.iter().all(|&p|p==0));screen_command(&mut m,dc,0x29,&[]);assert_eq!(m.bus.board.display().unwrap().2,frame.2);
        if let Some((pin,_))=light {write(&mut m,0x6000_9004+4*u32::from(pin),0);assert!(m.bus.board.display().unwrap().2.iter().all(|&p|p==0),"GPIO latch without routed backlight cannot light {id}");}else if let Some(pin)=enable {output(&mut m,pin,false);assert!(m.bus.board.display().unwrap().2.iter().all(|&p|p==0));}
        boot_probe(&mut m,&[0]);assert!(m.bus.board.lora_snapshot().is_none());assert!(m.bus.board.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),Default::default()).is_err());
        if id=="lilygo-t-encoder-pro" {assert_eq!(m.bus.board.encoder(),Some((1,2)));assert_eq!(m.bus.board.named_pin("encoder-press"),Some(0));assert_eq!(m.bus.board.named_pin("boot"),Some(0));}
    }
}
fn read8(d:&mut dyn I2cDevice,address:u8,n:usize)->Vec<u8> {assert!(d.start(false));assert!(d.write(address));assert!(d.start(true));(0..n).map(|_|d.read()).collect()}
#[test]
fn cst226se_and_cst816d_keep_distinct_registers_ids_sleep_reset_and_coordinates() {
    for (id,sda,scl,addr,irq,reset,x,y) in [("lilygo-t-connect-pro",39,40,0x5a,3,Some(47),221,479),("lilygo-t-circle-s3",11,14,0x15,12,None,159,159),("lilygo-t-encoder-pro",5,6,0x15,9,Some(8),389,389)] {
        let mut r=Rig::new(id);let mut devices=r.board.i2c_devices();assert!(devices.iter().all(|(_,_,d)|d.pins()==Some((sda,scl))));assert_eq!(devices.len(),2);assert!(devices.iter().all(|(_,a,_)|*a==addr));
        if let Some(pin)=reset {assert!(devices.iter_mut().all(|(_,_,d)|!d.start(false)));r.drive(pin,true);}
        r.board.touch(x,y,true);assert_eq!(r.board.touch_irq(),Some((irq,false)));let d=&mut devices.iter_mut().find(|(bus,_,_)|*bus==0).unwrap().2;
        if id=="lilygo-t-connect-pro" {assert_eq!(read8(d.as_mut(),0,7),[6,13,29,0xdf,0,1,0xab]);assert_eq!(read8(d.as_mut(),6,1),[0xab]);assert!(d.start(false));assert!(d.write(0xd1));assert!(d.write(5));}
        else {assert_eq!(read8(d.as_mut(),2,5),[1,(x>>8) as u8,x as u8,(y>>8) as u8,y as u8]);assert_eq!(read8(d.as_mut(),0xa7,1),[0xb6]);assert!(d.start(false));assert!(d.write(0xe5));assert!(d.write(3));}
        assert!(!d.start(false));assert_eq!(r.board.touch_irq(),Some((irq,true)));r.board.touch(x,y,true);assert_eq!(r.board.touch_irq(),Some((irq,true)));
        if let Some(pin)=reset {r.drive(pin,false);r.drive(pin,true);assert!(d.start(false));r.board.touch(u16::MAX,u16::MAX,true);let count=if id=="lilygo-t-connect-pro" {5}else{2};assert_eq!(read8(d.as_mut(),count,1),[1]);r.board.touch(0,0,false);assert_eq!(read8(d.as_mut(),count,1),[0]);}
        let mut m=machine(id);if let Some(pin)=reset {mode(&mut m,pin,256);output(&mut m,pin,true);}i2c_route(&mut m,sda,scl,0);assert!(i2c_write(&mut m,0,addr,&[0]));i2c_route(&mut m,sda,48,0);assert!(!i2c_write(&mut m,0,addr,&[0]));
    }
}

#[test]
fn mvsr_kit_and_three_twr_revisions_execute_their_own_oled_encoder_and_gpio_maps() {
    for (id,sda,scl,offset) in [("lilygo-t3-s3-mvsr",18,17,0),("lilygo-t-twr-rev2-1",8,9,2),("lilygo-t-twr-plus",8,9,2),("lilygo-t-twr",13,14,0)] {
        let mut m=machine(id);i2c_route(&mut m,sda,scl,0);assert!(i2c_write(&mut m,0,0x3c,&[0,0xaf,0xb0,offset,0x10]));assert!(i2c_write(&mut m,0,0x3c,&[0x40,1]));
        if id=="lilygo-t-twr" {assert!(m.bus.board.display().unwrap().2.iter().all(|&p|p==0),"OLED VBAT off still accepts I2C/RAM");mode(&mut m,21,256);output(&mut m,21,true);}
        assert_eq!(m.bus.board.display().unwrap().2[0],65535,"{id}");
        if id=="lilygo-t3-s3-mvsr" {assert_eq!(m.bus.board.encoder(),None);boot_probe(&mut m,&[0]);mode(&mut m,37,256);output(&mut m,37,true);assert_eq!(m.bus.board.leds().unwrap().0[0],[255;3]);}
        else if id=="lilygo-t-twr" {assert_eq!(m.bus.board.encoder(),Some((9,5)));boot_probe(&mut m,&[0,3,38,7]);output(&mut m,21,false);assert!(m.bus.board.display().unwrap().2.iter().all(|&p|p==0));output(&mut m,21,true);assert_eq!(m.bus.board.display().unwrap().2[0],65535,"emission gate preserves digital RAM");assert!(m.bus.board.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),Default::default()).is_err());}
        else {assert_eq!(m.bus.board.encoder(),Some((47,46)));boot_probe(&mut m,&[0,3,21]);assert!(!i2c_write(&mut m,0,0x3d,&[0,0xaf]),"UHF address is not silently enabled");assert!(i2c_write(&mut m,0,0x34,&[0x80,0]));assert!(!i2c_write(&mut m,0,0x3c,&[0,0xaf]));assert!(m.bus.board.display().unwrap().2.iter().all(|&p|p==0));}
        if let Some((a,b))=m.bus.board.encoder() {assert_eq!(m.bus.board.named_pin("clk"),Some(a));assert_eq!(m.bus.board.named_pin("dt"),Some(b));assert!(m.bus.board.input_levels().contains(&(a,true)));assert!(m.bus.board.input_levels().contains(&(b,true)));}
        assert!(m.bus.board.lora_snapshot().is_none());assert!(!m.bus.board.uses_uart_pins());
    }
}
#[test]
fn mvsr_and_twr_plus_card_writes_obey_their_separate_spi_and_aldo2_supply() {
    for id in ["lilygo-t3-s3-mvsr","lilygo-t-twr-plus","lilygo-t-twr-rev2-1"] {
        let mut b=lilygo_additional::make_board(id).unwrap();let twr=id!="lilygo-t3-s3-mvsr";let pins=if twr{SpiPins{sclk:1<<12,mosi:1<<11,miso:Some(13),cs:1<<10,keep_cs:false}}else{SpiPins{sclk:1<<14,mosi:1<<11,miso:Some(2),cs:1<<13,keep_cs:false}};
        b.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),Default::default()).unwrap();if twr {assert_eq!(sd_command(&mut b,pins,0,0),[255,255]);power_register(&mut b,0x90,2);}
        b.spi_transfer_pins(2,SpiPins{cs:0,..pins},&[255;10],0);assert_eq!(sd_command(&mut b,pins,0,0),[255,1]);sd_command(&mut b,pins,8,0x1aa);b.spi_transfer_pins(2,pins,&[],4);for _ in 0..2 {sd_command(&mut b,pins,55,0);sd_command(&mut b,pins,41,1<<30);}
        assert_eq!(sd_command(&mut b,pins,24,8),[255,0]);let mut bytes=vec![0xfe];bytes.extend([0x36;512]);bytes.extend([255,255]);b.spi_transfer_pins(2,pins,&bytes,0);assert_eq!(b.spi_transfer_pins(2,pins,&[],1),[5]);assert_eq!(sd_command(&mut b,pins,17,8),[255,0]);assert_eq!(b.spi_transfer_pins(2,pins,&[],2),[255,0xfe]);assert_eq!(b.spi_transfer_pins(2,pins,&[],512),vec![0x36;512]);b.spi_transfer_pins(2,pins,&[],2);
        if twr {assert_eq!(sd_command(&mut b,SpiPins{cs:pins.cs|1<<14,..pins},0,0),[255,255]);power_register(&mut b,0x90,0);assert_eq!(sd_command(&mut b,pins,0,0),[255,255]);}
        b.set_sd_card_present(false);assert_eq!(sd_command(&mut b,pins,0,0),[255,255]);
    }
}

#[test]
fn glass_v2_and_long_execute_native_spi_without_vendor_page_side_effects() {
    for (id,clock,mosi,cs,dc,rst,light,x,y,w,h) in [
        ("lilygo-t-glass",15,16,17,Some(21),18,None,0u16,168u16,126u16,126u16),
        ("lilygo-t-display-s3-long",17,13,12,None,16,Some(1),0,0,180,640),
    ] {
        let mut m=machine(id);for (pin,signal) in [(clock,101),(mosi,103),(cs,110)] {mode(&mut m,pin,signal);}write(&mut m,SPI+0x20,0x3e);mode(&mut m,rst,256);output(&mut m,rst,true);
        if let Some(pin)=dc {mode(&mut m,pin,256);}
        if let Some(pin)=light{mode(&mut m,pin,256);output(&mut m,pin,false);}
        for (c,d) in [(0x11,vec![]),(0x29,vec![]),(0x3a,vec![0x55])] {screen_command(&mut m,dc,c,&d);}
        let put=|m:&mut esp32s3::Machine,x:u16,y:u16,color:u16| {screen_command(m,dc,0x2a,&[x.to_be_bytes(),x.to_be_bytes()].concat());screen_command(m,dc,0x2b,&[y.to_be_bytes(),y.to_be_bytes()].concat());screen_command(m,dc,0x2c,&color.to_be_bytes());};
        put(&mut m,x,y,0xf800);put(&mut m,x+w-1,y+h-1,0x07e0);let frame=m.bus.board.display().unwrap();assert_eq!((frame.0,frame.1),(u32::from(w),u32::from(h)));assert_eq!(frame.2[0],0xf800);assert_eq!(frame.2[frame.2.len()-1],0x07e0);
        if dc.is_some() {screen_command(&mut m,dc,0xfe,&[7]);for c in [0x28,0x10,0x2c,0x01] {screen_command(&mut m,dc,c,&[0xff;4]);}assert_eq!(m.bus.board.display().unwrap().2,frame.2,"JD9613 analog page is not DCS");screen_command(&mut m,dc,0xfe,&[0]);put(&mut m,0,0,0xffff);assert_eq!(m.bus.board.display().unwrap().2,frame.2,"V2 prism hides the top of the panel");}
        mode(&mut m,clock,102);put(&mut m,x,y,0xffff);assert_eq!(m.bus.board.display().unwrap().2,frame.2);mode(&mut m,clock,101);output(&mut m,rst,false);assert!(m.bus.board.display().unwrap().2.iter().all(|&v|v==0));boot_probe(&mut m,&[0]);
    }
}
#[test]
fn long_axs_touch_requires_its_eleven_byte_query_and_shared_reset() {
    let mut r=Rig::new("lilygo-t-display-s3-long");let mut devices=r.board.i2c_devices();assert_eq!(devices.len(),2);assert!(devices.iter().all(|(_,a,d)|*a==0x3b&&d.pins()==Some((15,10))));let d=devices[0].2.as_mut();assert!(!d.start(false));r.drive(16,true);r.board.touch(179,639,true);
    let read=|d:&mut dyn I2cDevice,query:&[u8]|{assert!(d.start(false));for &b in query{assert!(d.write(b));}assert!(d.start(true));(0..8).map(|_|d.read()).collect::<Vec<_>>()};
    assert_eq!(read(d,&[0]),[255;8]);let query=[0xb5,0xab,0xa5,0x5a,0,0,0,8,0,0,0];assert_eq!(read(d,&query),[0,1,0x80,1,0,179,0,0]);assert_eq!(r.board.touch_irq(),Some((11,false)));r.board.touch(0,0,true);assert_eq!(read(d,&query),[0,1,0x82,0x80,0,0,0,0]);r.board.touch(0,0,false);assert_eq!(read(d,&query)[1],0);r.drive(16,false);assert!(!d.start(false));r.drive(16,true);assert_eq!(read(d,&query)[1],0);
    let mut m=machine("lilygo-t-display-s3-long");mode(&mut m,16,256);output(&mut m,16,true);i2c_route(&mut m,15,10,0);assert!(i2c_write(&mut m,0,0x3b,&query));i2c_route(&mut m,15,9,0);assert!(!i2c_write(&mut m,0,0x3b,&query));
}
#[test]
fn ts_ssd1680_native_spi_refreshes_real_crop_only_at_busy_deadline() {
    let mut m=machine("lilygo-ts-s3-epaper");for (pin,signal) in [(14,101),(11,103),(15,110),(16,256),(47,256)] {mode(&mut m,pin,signal);}write(&mut m,SPI+0x20,0x3e);output(&mut m,47,true);
    for (c,d) in [(0x11,vec![3]),(0x44,vec![15,15]),(0x45,vec![249,0,249,0]),(0x4e,vec![15]),(0x4f,vec![249,0]),(0x24,vec![0x3f])] {screen_command(&mut m,Some(16),c,&d);}
    assert!(m.bus.board.display().unwrap().2.iter().all(|&v|v==65535));screen_command(&mut m,Some(16),0x22,&[0xf7]);screen_command(&mut m,Some(16),0x20,&[]);let due=m.bus.board.next_deadline().unwrap();assert!(m.bus.board.input_levels().contains(&(48,true)));m.bus.board.advance_to(due-1);assert_eq!(m.bus.board.display_frames(),0);m.bus.board.advance_to(due);let frame=m.bus.board.display().unwrap();assert_eq!((frame.0,frame.1),(122,250));assert_eq!(&frame.2[frame.2.len()-2..],&[0,0]);assert_eq!(frame.2.iter().filter(|&&p|p==0).count(),2);assert!(m.bus.board.input_levels().contains(&(48,false)));
    output(&mut m,47,false);assert_eq!(m.bus.board.display().unwrap().2,frame.2,"reset retains optical pixels");output(&mut m,47,true);mode(&mut m,14,102);screen_command(&mut m,Some(16),0x20,&[]);assert_eq!(m.bus.board.next_deadline(),None);boot_probe(&mut m,&[0]);
    let mut r=Rig::new("lilygo-ts-s3-epaper");r.drive(47,true);let p=SpiPins{sclk:1<<14,mosi:1<<11,cs:1<<15,..Default::default()};r.drive(16,false);r.board.spi_transfer_pins(2,SpiPins{cs:p.cs|1<<13,..p},&[0x20],0);assert_eq!(r.board.next_deadline(),None,"SD and display cannot be selected together");r.board.spi_transfer_pins(2,p,&[0x10],0);r.drive(16,true);r.board.spi_transfer_pins(2,p,&[1],0);assert!(r.board.input_levels().contains(&(48,true)));assert_eq!(r.board.next_deadline(),None,"deep sleep BUSY is not a polling deadline");
}
#[test]
fn long_and_ts_sd_use_separate_exact_buses_and_persist_across_removal() {
    for (id,clk,mosi,miso,cs,conflict) in [("lilygo-t-display-s3-long",40,39,41,38,None),("lilygo-ts-s3-epaper",14,11,2,13,Some(15))] {
        let mut b=lilygo_additional::make_board(id).unwrap();let p=SpiPins{sclk:1<<clk,mosi:1<<mosi,miso:Some(miso),cs:1<<cs,keep_cs:false};b.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),Default::default()).unwrap();b.spi_transfer_pins(2,SpiPins{cs:0,..p},&[255;10],0);assert_eq!(sd_command(&mut b,p,0,0),[255,1]);sd_command(&mut b,p,8,0x1aa);b.spi_transfer_pins(2,p,&[],4);for _ in 0..2 {sd_command(&mut b,p,55,0);sd_command(&mut b,p,41,1<<30);}
        assert_eq!(sd_command(&mut b,p,24,4),[255,0]);let mut payload=vec![0xfe];payload.extend([0x96;512]);payload.extend([255;2]);b.spi_transfer_pins(2,p,&payload,0);assert_eq!(b.spi_transfer_pins(2,p,&[],1),[5]);assert_eq!(sd_command(&mut b,p,17,4),[255,0]);assert_eq!(b.spi_transfer_pins(2,p,&[],2),[255,0xfe]);assert_eq!(b.spi_transfer_pins(2,p,&[],512),[0x96;512]);b.spi_transfer_pins(2,p,&[],2);assert_eq!(sd_command(&mut b,SpiPins{miso:Some(0),..p},58,0),[255;2]);if let Some(pin)=conflict {assert_eq!(sd_command(&mut b,SpiPins{cs:p.cs|1<<pin,..p},0,0),[255;2]);}b.set_sd_card_present(false);assert_eq!(sd_command(&mut b,p,0,0),[255;2]);
    }
}
#[test]
fn touchbar_native_spi_broadcast_and_independent_panels_keep_real_pixels() {
    let mut m=machine("lilygo-t-touch-bar");for(pin,signal)in[(17,101),(18,103),(16,256),(15,256),(13,256),(14,256)]{mode(&mut m,pin,signal);}write(&mut m,SPI+0x20,0x3f);output(&mut m,15,true);output(&mut m,13,false);output(&mut m,14,false);
    for(c,d)in[(0x11,vec![]),(0x29,vec![]),(0x3a,vec![0x55])]{screen_command(&mut m,Some(16),c,&d);}
    // Software rotation of native (125,0) places the first composite pixel.
    for (cs,color) in [(13,0xf800u16),(14,0x07e0)] {output(&mut m,13,cs!=13);output(&mut m,14,cs!=14);screen_command(&mut m,Some(16),0x2a,&[0,125,0,125]);screen_command(&mut m,Some(16),0x2b,&[0;4]);screen_command(&mut m,Some(16),0x2c,&color.to_be_bytes());}
    let frame=m.bus.board.display().unwrap();assert_eq!((frame.0,frame.1),(588,126));assert_eq!((frame.2[0],frame.2[294]),(0xf800,0x07e0));assert_eq!(frame.2.iter().filter(|&&p|p!=0).count(),2);
    output(&mut m,13,true);output(&mut m,14,true);screen_command(&mut m,Some(16),0x2c,&[255;2]);assert_eq!(m.bus.board.display().unwrap().2,frame.2);output(&mut m,13,false);output(&mut m,14,false);screen_command(&mut m,Some(16),0x28,&[]);assert!(m.bus.board.display().unwrap().2.iter().all(|&p|p==0));screen_command(&mut m,Some(16),0x29,&[]);assert_eq!(m.bus.board.display().unwrap().2,frame.2);output(&mut m,15,false);assert!(m.bus.board.display().unwrap().2.iter().all(|&p|p==0));assert!(m.bus.board.named_pin("boot").is_none());
    let mut r=Rig::new("lilygo-t-touch-bar");let mut ds=r.board.i2c_devices();assert!(ds.iter().all(|(_,a,d)|*a==0x15&&d.pins()==Some((10,11))));assert!(!ds[0].2.start(false));r.drive(9,true);r.board.touch(300,100,true);let d=ds[0].2.as_mut();assert!(d.start(false));d.write(2);assert!(d.start(true));assert_eq!((0..5).map(|_|d.read()).collect::<Vec<_>>(),[1,0,26,1,76]);assert_eq!(r.board.touch_irq(),Some((12,false)));r.board.touch(0,0,false);assert_eq!(r.board.touch_irq(),Some((12,true)));r.drive(9,false);assert!(!d.start(true));
}
fn hmi_gpio_byte(m:&mut esp32s3::Machine,dc:bool,byte:u8) {output(m,8,false);output(m,7,dc);for(i,p)in[48,47,39,40,41,42,45,46].into_iter().enumerate(){output(m,p,byte&(1<<i)!=0);}output(m,8,true);}
#[test]
fn hmi_native_gpio_i80_and_cold_dma_use_exact_hmi_data_permutation() {
    let mut m=machine("lilygo-t-hmi");for pin in [10,38,6,7,8,48,47,39,40,41,42,45,46]{mode(&mut m,pin,256);}output(&mut m,10,true);output(&mut m,38,true);output(&mut m,6,false);
    for(c,d)in[(0x11,vec![]),(0x29,vec![]),(0x3a,vec![0x55]),(0x2a,vec![0,0,0,0]),(0x2b,vec![0,0,0,0]),(0x2c,vec![0xf8,0])]{hmi_gpio_byte(&mut m,false,c);for b in d{hmi_gpio_byte(&mut m,true,b);}}
    let f=m.bus.board.display().unwrap();assert_eq!((f.0,f.1),(240,320));assert_eq!(f.2[0],0xf800);mode(&mut m,48,133);hmi_gpio_byte(&mut m,false,0x28);assert_eq!(m.bus.board.display().unwrap().2,f.2,"an i80 data bit without GPIO routing cannot latch");boot_probe(&mut m,&[0,21]);
    use esp_soc::board::LcdI80Pins;let p=LcdI80Pins{data:std::array::from_fn(|i|if i<8{1<<[48,47,39,40,41,42,45,46][i]}else{0}),clock:1<<8,select:1<<6,width:8,dc:1<<7,dc_high:false};m.bus.board.lcd_i80(LcdI80Pins{data:[0;16],..p},false,&[0x28]);assert_eq!(m.bus.board.display().unwrap().2,f.2);m.bus.board.lcd_i80(p,false,&[0x28]);assert!(m.bus.board.display().unwrap().2.iter().all(|&p|p==0));m.bus.board.lcd_i80(p,false,&[0x29]);output(&mut m,10,false);assert!(m.bus.board.display().unwrap().2.iter().all(|&p|p==0));assert!(!m.bus.board.sd_card_slot().unwrap().1);assert!(m.bus.board.touch_irq().is_none());
}
#[test]
fn hmi_native_sdmmc_runs_one_bit_card_and_dma_roundtrip_with_route_rejection() {
    use esp_soc::SocBus as _;use esp_periph::sdmmc::{reg,des0,int,CMD_START,CMD_RESP,CMD_RESP_LONG,CMD_DATA,CMD_WRITE,CTRL_USE_INTERNAL_DMA,BMOD_DE};
    for (id,clock,command,data0) in [("lilygo-t-hmi",12,11,13),("lilygo-t-rgb",39,40,38)] {
    let mut m=machine(id);m.bus.attach_sd_card(Box::new(MemStore(vec![0;1<<20])),Default::default()).unwrap();if id=="lilygo-t-hmi"{mode(&mut m,10,256);output(&mut m,10,true);}else{i2c_route(&mut m,8,48,0);assert!(i2c_write(&mut m,0,0x21,&[6,0x7f,255]));}
    let base=0x6002_8000;for(p,s)in[(clock,173),(command,179),(data0,213)]{mode(&mut m,p,s);write(&mut m,GPIO+0x154+4*s,0x80|u32::from(p));}write(&mut m,0x600c_001c,1<<7);write(&mut m,0x600c_0024,0);
    let cmd=|m:&mut esp32s3::Machine,n:u8,arg:u32,flags:u32|{write(m,base+reg::RINTSTS,u32::MAX);write(m,base+reg::CMDARG,arg);write(m,base+reg::CMD,CMD_START|1<<16|u32::from(n)|flags);guest_read(m,base+reg::RESP0)};
    cmd(&mut m,0,0,0);assert_eq!(cmd(&mut m,8,0x1aa,CMD_RESP),0x1aa);for _ in 0..2{cmd(&mut m,55,0,CMD_RESP);cmd(&mut m,41,0x40ff8000,CMD_RESP);}cmd(&mut m,2,0,CMD_RESP|CMD_RESP_LONG);let rca=cmd(&mut m,3,0,CMD_RESP)&0xffff0000;cmd(&mut m,7,rca,CMD_RESP);write(&mut m,base+reg::CTYPE,0);
    let desc=0x3fc8_8100;let data=0x3fc8_8800;for i in 0..128{write(&mut m,data+i*4,0x96a5_1234);}
    for (n,buffer) in [(24,data),(17,data+512)]{for(i,v)in[des0::OWN|des0::FS|des0::LD,512,buffer,0].into_iter().enumerate(){write(&mut m,desc+4*i as u32,v);}for(o,v)in[(reg::CTRL,CTRL_USE_INTERNAL_DMA),(reg::BMOD,BMOD_DE),(reg::DBADDR,desc),(reg::BLKSIZ,512),(reg::BYTCNT,512)]{write(&mut m,base+o,v);}cmd(&mut m,n,6,CMD_RESP|CMD_DATA|if n==24{CMD_WRITE}else{0});assert_ne!(guest_read(&mut m,base+reg::RINTSTS)&int::DATA_OVER,0,"n={n} rint={:x} idsts={:x} resp={:x} stats={:?}",guest_read(&mut m,base+reg::RINTSTS),guest_read(&mut m,base+reg::IDSTS),guest_read(&mut m,base+reg::RESP0),m.bus.periph.sdmmc.stats);}
    for i in 0..128{assert_eq!(guest_read(&mut m,data+512+i*4),0x96a5_1234);}mode(&mut m,clock,172);cmd(&mut m,8,0x1aa,CMD_RESP);assert_ne!(guest_read(&mut m,base+reg::RINTSTS)&int::RTO,0);assert_eq!(m.bus.periph.sdmmc.stats.bytes_written,512);
    }
}
fn raw_rgb_routes(r:&mut Rig) {for(pin,signal)in[(4,154),(42,151),(41,150),(18,141),(8,142),(16,143),(17,144),(7,145),(15,146),(5,147),(6,148)]{r.gpio.func_out_sel[pin as usize]=signal;r.mux.write(4+4*pin,1<<12);}r.board.gpio_waveform_at(r.cycle,ChipPins::S3.routes(&r.gpio,&r.mux));}
fn expander_write(board:&mut Board,pins:(u8,u8),address:u8,register:u8,bytes:&[u8]) {let mut devices=board.i2c_devices();let d=devices.iter_mut().find(|(bus,a,d)|*bus==0&&*a==address&&d.pins()==Some(pins)).unwrap().2.as_mut();assert!(d.start(false));assert!(d.write(register));for &b in bytes{assert!(d.write(b));}d.stop();}
#[test]
fn t5_pro_raw_drive_uses_pca9535_routes_gate_clock_padding_and_retained_pixels() {
    let mut m=machine("lilygo-t5-e-paper-s3-pro");i2c_route(&mut m,39,40,0);assert!(i2c_write(&mut m,0,0x20,&[6,255,0xc4]));assert!(i2c_write(&mut m,0,0x20,&[3,0x39]));assert!(!i2c_write(&mut m,0,0x68,&[1,0x3f]),"TPS analog regulator is not fabricated");boot_probe(&mut m,&[0]);
    let mut r=Rig::new("lilygo-t5-e-paper-s3-pro");raw_rgb_routes(&mut r);expander_write(&mut r.board,(39,40),0x20,6,&[255,0xc4]);expander_write(&mut r.board,(39,40),0x20,3,&[0x39]);r.drive(45,true);r.drive(45,false);r.drive(45,true);
    let mut bytes=vec![0u8;241*544];for row in bytes.as_chunks_mut::<241>().0.iter_mut(){row[0]=0x55;}bytes[1]=1;bytes[539*241+240]=0x40;let start=|r:&mut Rig,pulses|r.board.lcd_rgb_start(r.cycle,241,544,1,&|p|if p==48{Some((5280,pulses))}else{None});
    start(&mut r,543);r.board.lcd_frame(241,544,&bytes);assert_eq!(r.board.display_frames(),0);start(&mut r,544);r.board.lcd_frame(241,544,&bytes);let f=r.board.display().unwrap();assert_eq!((f.0,f.1),(960,540));assert_eq!((f.2[0],f.2[f.2.len()-1]),(0,0));assert_eq!(f.2.iter().filter(|&&p|p==0).count(),2,"dummy prefix and four padded rows cannot paint pixels");assert_eq!(r.board.display_frames(),1);
    bytes.fill(0xaa);start(&mut r,544);r.board.lcd_frame(241,544,&bytes);assert_eq!(r.board.display().unwrap().2,f.2,"new source transfer needs a fresh STV scan");r.drive(45,false);r.drive(45,true);r.gpio.func_out_sel[18]^=1<<9;r.board.gpio_waveform_at(0,ChipPins::S3.routes(&r.gpio,&r.mux));start(&mut r,544);r.board.lcd_frame(241,544,&bytes);assert_eq!(r.board.display().unwrap().2,f.2);raw_rgb_routes(&mut r);expander_write(&mut r.board,(39,40),0x20,3,&[0]);start(&mut r,544);r.board.lcd_frame(241,544,&bytes);assert_eq!(r.board.display().unwrap().2,f.2);
    assert!(r.board.button_at(10,"function",true));let mut ds=r.board.i2c_devices();let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x20).unwrap().2.as_mut();assert!(d.start(false));d.write(1);assert!(d.start(true));let input=d.read();assert_eq!(input&(1<<2),0);assert_eq!(input&(1<<6),0,"unmodeled TPS PG is never asserted");
}
fn shift_config(r:&mut Rig,byte:u8) {r.drive(0,false);for bit in(0..8).rev(){r.drive(12,false);r.drive(13,byte&(1<<bit)!=0);r.drive(12,true);}r.drive(0,true);}
#[test]
fn epd47_s3_raw_i80_latches_real_symbols_and_rejects_wrong_data_or_disabled_rails() {
    use esp_soc::board::LcdI80Pins;let mut r=Rig::new("lilygo-t5-4-7-inch-e-paper-v2-3");shift_config(&mut r,0x3c);shift_config(&mut r,0x6c);shift_config(&mut r,0xfc);let p=LcdI80Pins{data:std::array::from_fn(|i|if i<8{1<<[6,7,4,5,2,3,8,1][i]}else{0}),clock:1<<41,select:0,width:8,dc:1<<40,dc_high:false};let mut line=vec![0u16;248];line[0]=1;line[239]=0x40;line[240..].fill(0x55);
    r.board.lcd_i80(LcdI80Pins{data:[0;16],..p},true,&line);shift_config(&mut r,0xfd);shift_config(&mut r,0xfc);assert!(r.board.display().unwrap().2.iter().all(|&v|v==65535));r.board.lcd_i80(p,true,&line);assert!(r.board.display().unwrap().2.iter().all(|&v|v==65535));shift_config(&mut r,0xfd);shift_config(&mut r,0xfc);let f=r.board.display().unwrap();assert_eq!((f.0,f.1),(960,540));assert_eq!((f.2[0],f.2[959]),(0,0));assert_eq!(f.2.iter().filter(|&&p|p==0).count(),2);shift_config(&mut r,0x32);r.board.lcd_i80(p,true,&[0xaa;248]);shift_config(&mut r,0x33);assert_eq!(r.board.display().unwrap().2,f.2,"power-off retains glass and cannot latch new data");
    let mut m=machine("lilygo-t5-4-7-inch-e-paper-v2-3");for pin in[0,12,13]{mode(&mut m,pin,256);}for cfg in[0x3c,0x6c,0xfc]{output(&mut m,0,false);for bit in(0..8).rev(){output(&mut m,12,false);output(&mut m,13,cfg&(1<<bit)!=0);output(&mut m,12,true);}output(&mut m,0,true);}assert!(m.bus.board.report().contains("powered=true"));mode(&mut m,0,133);output(&mut m,0,false);assert!(m.bus.board.report().contains("powered=true"),"unrouted strobe cannot apply a config");boot_probe(&mut m,&[21]);
}
#[test]
fn both_raw_epaper_s3_cards_are_real_spi_sockets_with_distinct_wiring() {
    for (id,clk,mosi,miso,cs,other) in [("lilygo-t5-e-paper-s3-pro",14,13,21,12,Some(46)),("lilygo-t5-4-7-inch-e-paper-v2-3",11,15,16,42,None)] {let mut b=lilygo_additional::make_board(id).unwrap();let p=SpiPins{sclk:1<<clk,mosi:1<<mosi,miso:Some(miso),cs:1<<cs,keep_cs:false};b.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),Default::default()).unwrap();b.spi_transfer_pins(2,SpiPins{cs:0,..p},&[255;10],0);assert_eq!(sd_command(&mut b,p,0,0),[255,1]);sd_command(&mut b,p,8,0x1aa);assert_eq!(b.spi_transfer_pins(2,p,&[],4),[0,0,1,0xaa]);if let Some(pin)=other {assert_eq!(sd_command(&mut b,SpiPins{cs:p.cs|1<<pin,..p},0,0),[255;2]);}assert_eq!(sd_command(&mut b,SpiPins{miso:Some(0),..p},0,0),[255;2]);}
}
fn rgb_routes(r:&mut Rig) {for(pin,signal)in[(42,154),(47,151),(41,152),(45,150)].into_iter().chain([7,6,5,3,2,14,13,12,11,10,9,21,18,17,16,15].into_iter().enumerate().map(|(i,p)|(p,133+i as u32))){r.gpio.func_out_sel[pin as usize]=signal;r.mux.write(4+4*pin,1<<12);}r.drive(46,true);}
fn rgb_serial(board:&mut Board,word:u16) {expander_write(board,(8,48),0x21,2,&[0x4e]);for bit in(0..9).rev(){let byte=0x46|if word&(1<<bit)!=0{16}else{0};expander_write(board,(8,48),0x21,2,&[byte]);expander_write(board,(8,48),0x21,2,&[byte|32]);}expander_write(board,(8,48),0x21,2,&[0x4e]);}
#[test]
fn rgb_full_circle_exact_expander_serial_rgb_wires_and_cst820_contracts() {
    let mut r=Rig::new("lilygo-t-rgb");rgb_routes(&mut r);let mut frame=vec![0u8;480*480*2];frame[..4].copy_from_slice(&[0,0xf8,0xe0,7]);r.board.lcd_rgb_start(0,480,480,2,&|_|None);r.board.lcd_frame(480,480,&frame);assert_eq!(r.board.display_frames(),0,"floating reset and missing ST7701 initialization cannot render");
    expander_write(&mut r.board,(8,48),0x21,6,&[0x81,255]);for word in[0x11,0x29,0x3a,0x166,0x36,0x108]{rgb_serial(&mut r.board,word);}r.board.lcd_rgb_start(0,480,480,2,&|_|None);r.board.lcd_frame(480,480,&frame);let f=r.board.display().unwrap();assert_eq!((f.0,f.1),(480,480));assert_eq!(&f.2[..2],&[0xf800,0x07e0]);assert_eq!(r.board.display_frames(),1);
    // Command2 pages reuse native opcodes; their 28h/10h are not display off/sleep.
    for word in[0xff,0x177,0x101,0x100,0x100,0x111,0x28,0x10]{rgb_serial(&mut r.board,word);}assert_eq!(r.board.display().unwrap().2,f.2);for word in[0xff,0x177,0x101,0x100,0x100,0x100]{rgb_serial(&mut r.board,word);}r.gpio.func_out_sel[7]^=1<<9;r.board.gpio_waveform_at(0,ChipPins::S3.routes(&r.gpio,&r.mux));r.board.lcd_rgb_start(0,480,480,2,&|_|None);r.board.lcd_frame(480,480,&vec![255;480*480*2]);assert_eq!(r.board.display().unwrap().2,f.2);rgb_routes(&mut r);r.board.lcd_rgb_start(0,480,480,1,&|_|None);r.board.lcd_frame(480,480,&frame);assert_eq!(r.board.display_frames(),1,"wrong RGB data width cannot publish");
    let mut ds=r.board.i2c_devices();let touch=ds.iter_mut().find(|(b,a,_)|*b==0&&*a==0x15).unwrap().2.as_mut();r.board.touch(479,300,true);assert_eq!(r.board.touch_irq(),Some((1,false)));assert!(touch.start(false));touch.write(2);assert!(touch.start(true));assert_eq!((0..5).map(|_|touch.read()).collect::<Vec<_>>(),[1,1,223,1,44]);expander_write(&mut r.board,(8,48),0x21,2,&[0x4c]);assert!(!touch.start(false));assert_eq!(r.board.touch_irq(),Some((1,true)));expander_write(&mut r.board,(8,48),0x21,2,&[0x0e]);assert!(r.board.display().unwrap().2.iter().all(|&p|p==0));assert!(!r.board.uses_gpio_pwm(),"AW9364 pulse dimming is not average PWM");
}
#[test]
fn rgb_expander_nine_bit_commands_execute_real_xtensa_i2c_and_reject_wrong_address() {
    let mut m=machine("lilygo-t-rgb");i2c_route(&mut m,8,48,0);assert!(!i2c_write(&mut m,0,0x20,&[6,0x81,255]));assert!(i2c_write(&mut m,0,0x21,&[6,0x81,255]));
    for word in[0x11u16,0x29,0x3a,0x166,0x36,0x108] {assert!(i2c_write(&mut m,0,0x21,&[2,0x4e]));for bit in(0..9).rev(){let byte=0x46|if word&(1<<bit)!=0{16}else{0};assert!(i2c_write(&mut m,0,0x21,&[2,byte]));assert!(i2c_write(&mut m,0,0x21,&[2,byte|32]));}assert!(i2c_write(&mut m,0,0x21,&[2,0x4e]));}
    assert!(m.bus.board.report().contains("serial words=6"));i2c_route(&mut m,8,47,0);assert!(!i2c_write(&mut m,0,0x21,&[2,0]));boot_probe(&mut m,&[0]);
}
