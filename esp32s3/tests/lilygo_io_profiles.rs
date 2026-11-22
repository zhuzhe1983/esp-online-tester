#[path="../src/board/lilygo_io.rs"]
mod lilygo_io;
use emu_core::Bus;
use esp_soc::{BoardModel,SocBus as _};
use esp_periph::sdmmc::MemStore;
use lilygo_io::LilygoIo;
use esp32s3::bus::SocBus;

// Existing S3 MMIO path; IDF v5.5.4 gpio_reg.h/io_mux_reg.h/spi_reg.h,
// gpio_sig_map.h and rmt_reg.h. No host writes into a device framebuffer.
fn machine(board:LilygoIo)->SocBus {let mut b=SocBus::new(16<<20,8<<20,[0;6]);b.board=Box::new(board);b.attach_board_devices();b}
fn route(b:&mut SocBus,pin:u8,signal:u32) {b.write32(0x6000_9004+4*u32::from(pin),1<<12|1<<9).unwrap();b.write32(0x6000_4554+4*u32::from(pin),signal).unwrap();}
fn level(b:&mut SocBus,pin:u8,high:bool) {b.write32(0x6000_4000+if pin<32 {if high {8}else{12}}else if high {20}else{24},1<<(pin%32)).unwrap();}
fn drive(b:&mut SocBus,pin:u8,high:bool) {route(b,pin,256);b.write32(if pin<32 {0x6000_4024}else{0x6000_4030},1<<(pin%32)).unwrap();level(b,pin,high);}
fn disable(b:&mut SocBus,pin:u8) {b.write32(if pin<32 {0x6000_4028}else{0x6000_4034},1<<(pin%32)).unwrap();}
fn spi(b:&mut SocBus,bytes:&[u8])->Vec<u8> {
    let mut out=Vec::new();for chunk in bytes.chunks(64) {
        for (i,word) in chunk.chunks(4).enumerate() {let mut data=[0;4];data[..word.len()].copy_from_slice(word);b.write32(0x6002_4098+4*i as u32,u32::from_le_bytes(data)).unwrap();}
        b.write32(0x6002_4010,1<<27|1<<28).unwrap();b.write32(0x6002_401c,(chunk.len()*8-1) as u32).unwrap();b.write32(0x6002_4000,1<<24).unwrap();
        out.extend((0..chunk.len()).map(|i|((b.read32(0x6002_4098+4*(i/4) as u32).unwrap()>>((i%4)*8))&255) as u8));
    }out
}

#[test]
fn all_ten_boards_have_exact_controls_and_no_invented_devices() {
    for mut board in [LilygoIo::t_connect(),LilygoIo::t_eth_elite(),LilygoIo::t_energy_s3(),LilygoIo::t_sim7670g_s3(),LilygoIo::t_rs_s3(),LilygoIo::t_eth_lite(),LilygoIo::t_relay_s3(),LilygoIo::t_sim7080_s3(),LilygoIo::t_camera_s3(),LilygoIo::t_simcam()] {
        assert!(!board.uses_gpio_waveform());assert_eq!(board.named_pin("boot"),Some(0));assert!(board.button_at(10,"boot",true));assert!(board.button_at(11,"boot",true));assert_eq!(board.take_edges(),[esp_soc::board::BoardEdge{cycle:10,pin:0,level:false}]);
        assert!(board.button_at(20,"boot",false));assert_eq!(board.take_edges(),[esp_soc::board::BoardEdge{cycle:20,pin:0,level:true}]);assert!(!board.button_at(30,"reset",true));assert!(!board.button_at(30,"power",true));assert!(board.camera_frame().is_none());assert!(board.lora_snapshot().is_none());
        if board.name()=="lilygo-t-rs-s3" {assert_eq!(board.named_pin("user"),Some(5));assert!(board.button_at(30,"user",true));assert_eq!(board.take_edges()[0].pin,5);}else{assert!(board.named_pin("user").is_none());}
        if board.name()!="lilygo-t-camera-s3" {assert!(board.display().is_none());}
        if !matches!(board.name(),"lilygo-t-camera-s3"|"lilygo-t-sim7080-s3") {assert!(board.i2c_devices().is_empty());}
    }
}

fn apa_frame()->Vec<u8> {let mut v=vec![0;4];v.extend([0xff,0,0,255,0xff,0,255,0,0xff,255,0,0,0xef,31,62,93]);v.extend([0xff,0,0,0]);v}
fn clock_bytes(b:&mut SocBus,bytes:&[u8]) {for &byte in bytes {for bit in (0..8).rev() {level(b,3,false);level(b,8,byte&(1<<bit)!=0);level(b,3,true);}}level(b,3,false);}

#[test]
fn connect_apa102_gpio_and_spi_preserve_wire_order_brightness_and_frame_boundaries() {
    let mut b=machine(LilygoIo::t_connect());drive(&mut b,3,false);drive(&mut b,8,false);let frame=apa_frame();
    clock_bytes(&mut b,&frame[..19]);assert_eq!(b.board.leds().unwrap().0,[[0;3];4],"incomplete LED frame is not published");
    clock_bytes(&mut b,&frame[19..]);assert_eq!(b.board.leds().unwrap().0,[[255,0,0],[0,255,0],[0,0,255],[45,30,15]]);
    let version=b.board.leds().unwrap().1;disable(&mut b,8);clock_bytes(&mut b,&[0;32]);assert_eq!(b.board.leds().unwrap().1,version,"disconnected data cannot create pixels");
    drive(&mut b,8,false);route(&mut b,3,101);route(&mut b,8,103);b.write32(0x6002_4020,0x3f).unwrap();
    let mut off=vec![0;4];for _ in 0..4 {off.extend([0xff,0,0,0]);}off.push(0xff);
    b.write32(0x6000_9004+4*8,0).unwrap();spi(&mut b,&off);assert_eq!(b.board.leds().unwrap().1,version);
    route(&mut b,8,103);spi(&mut b,&off);assert_eq!(b.board.leds().unwrap().0,[[0;3];4]);
    let mut invalid=frame.clone();invalid[4]=0x7f;spi(&mut b,&invalid);assert_eq!(b.board.leds().unwrap().0,[[0;3];4],"a 0-leading LED header cannot update a chain");
    spi(&mut b,&frame);assert_eq!(b.board.leds().unwrap().0[0],[255,0,0]);
}

fn shift(b:&mut SocBus,byte:u8) {for bit in (0..8).rev() {level(b,5,false);level(b,7,byte&(1<<bit)!=0);level(b,5,true);}level(b,5,false);}
fn latch(b:&mut SocBus) {level(b,6,true);level(b,6,false);}
#[test]
fn relay_595_shift_latch_and_output_enable_are_independent_physical_signals() {
    let mut b=machine(LilygoIo::t_relay_s3());for pin in [4,5,6,7] {drive(&mut b,pin,pin==4);}
    shift(&mut b,0xc5);latch(&mut b);assert!(b.board.report().contains("relay-drive-mask=0x00"));assert_eq!(b.board.leds().unwrap().0,[[0;3];2]);
    level(&mut b,4,false);assert!(b.board.report().contains("relay-drive-mask=0x05"));assert_eq!(b.board.leds().unwrap().0,[[0,255,0],[255,0,0]]);
    shift(&mut b,0x12);assert!(b.board.report().contains("relay-drive-mask=0x05"),"shift clocks alone leave storage latch unchanged");latch(&mut b);assert!(b.board.report().contains("relay-drive-mask=0x12"));
    disable(&mut b,4);assert!(b.board.report().contains("relay-drive-mask=0x00"));drive(&mut b,4,false);assert!(b.board.report().contains("relay-drive-mask=0x12"));
    b.write32(0x6000_4554+4*4,256|1<<9).unwrap();assert!(b.board.report().contains("relay-drive-mask=0x00"),"physical /OE inversion disables coils");drive(&mut b,4,false);
    b.write32(0x6000_9004+4*5,0).unwrap();shift(&mut b,0xff);latch(&mut b);assert!(b.board.report().contains("relay-drive-mask=0x12"),"wrong clock mux cannot shift");
    assert!(b.board.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),Default::default()).is_err());
}

#[test]
fn simple_led_polarities_and_rs_rmt_use_actual_output_routes() {
    for (board,pin,on,color) in [(LilygoIo::t_eth_elite(),38,true,[0,255,0]),(LilygoIo::t_sim7670g_s3(),12,false,[0,0,255])] {
        let mut b=machine(board);drive(&mut b,pin,on);assert_eq!(b.board.leds().unwrap().0,[color]);disable(&mut b,pin);assert_eq!(b.board.leds().unwrap().0,[[0;3]]);drive(&mut b,pin,on);
        b.write32(0x6000_4554+4*u32::from(pin),256|1<<9).unwrap();assert_eq!(b.board.leds().unwrap().0,[[0;3]]);drive(&mut b,pin,on);b.write32(0x6000_9004+4*u32::from(pin),0).unwrap();assert_eq!(b.board.leds().unwrap().0,[[0;3]]);
    }
    let mut b=machine(LilygoIo::t_rs_s3());route(&mut b,2,81);rmt(&mut b,[0x12,0x34,0x56]);assert_eq!(b.board.leds().unwrap().0,[[0;3]]);
    route(&mut b,1,81);rmt(&mut b,[0x12,0x34,0x56]);assert_eq!(b.board.leds().unwrap().0,[[0x12,0x34,0x56]]);
    route(&mut b,1,81|1<<9);rmt(&mut b,[255;3]);assert_eq!(b.board.leds().unwrap().0,[[0x12,0x34,0x56]],"inverted RMT is not a valid routed frame");
}
fn rmt(b:&mut SocBus,rgb:[u8;3]) {
    let mut index=0;for byte in [rgb[1],rgb[0],rgb[2]] {for bit in (0..8).rev() {let (h,l)=if byte&(1<<bit)!=0 {(64,32)}else{(32,64)};b.write32(0x6001_6800+4*index,0x8000|h|l<<16).unwrap();index+=1;}}
    b.write32(0x6001_6800+4*index,0).unwrap();b.write32(0x6001_6020,1<<16|1<<8|1).unwrap();b.tick(100_000);
}

fn crc7(bytes:&[u8])->u8 {let mut crc=0u8;for &value in bytes {let mut byte=value;for _ in 0..8 {crc<<=1;if (byte^crc)&0x80!=0 {crc^=9;}byte<<=1;}}crc&0x7f}
fn command(b:&mut SocBus,cmd:u8,arg:u32)->Vec<u8> {let mut bytes=vec![0x40|cmd];bytes.extend(arg.to_be_bytes());bytes.push(crc7(&bytes)<<1|1);spi(b,&bytes);spi(b,&[255;2])}
fn init_card(b:&mut SocBus,cs:u8) {drive(b,cs,true);spi(b,&[255;10]);level(b,cs,false);assert_eq!(command(b,0,0),[255,1]);assert_eq!(command(b,8,0x1aa),[255,1]);assert_eq!(spi(b,&[255;4]),[0,0,1,0xaa]);for _ in 0..2 {command(b,55,0);command(b,41,1<<30);}}
fn spi_card(b:&mut SocBus,pins:[u8;4]) {let [sclk,mosi,miso,_]=pins;route(b,sclk,101);route(b,mosi,103);b.write32(0x6000_9004+4*u32::from(miso),1<<12|1<<9).unwrap();b.write32(0x6000_42ec,0x80|u32::from(miso)).unwrap();b.write32(0x6002_4020,0x3f).unwrap();b.attach_sd_card(Box::new(MemStore(vec![0;1<<20])),Default::default()).unwrap();}
fn read_block(b:&mut SocBus)->Vec<u8> {assert_eq!(command(b,17,3),[255,0]);assert_eq!(spi(b,&[255;2]),[255,0xfe]);let bytes=spi(b,&[255;512]);spi(b,&[255;2]);bytes}

#[test]
fn all_four_spi_sd_pinouts_roundtrip_real_blocks_and_reject_other_board_routes() {
    for (board,pins) in [(LilygoIo::t_eth_elite(),[10,11,9,12]),(LilygoIo::t_sim7670g_s3(),[21,14,47,13]),(LilygoIo::t_eth_lite(),[7,6,5,42]),(LilygoIo::t_simcam(),[39,38,40,47])] {
        let mut b=machine(board);spi_card(&mut b,pins);if b.board.name()=="lilygo-t-simcam" {drive(&mut b,1,true);}init_card(&mut b,pins[3]);
        assert_eq!(command(&mut b,24,3),[255,0]);let mut data=vec![0xfe];data.extend((0..512).map(|i|(i%251) as u8));data.extend([255;2]);spi(&mut b,&data);assert_eq!(spi(&mut b,&[255]),[5]);assert_eq!(read_block(&mut b),data[1..513]);
        b.write32(0x6000_9004+4*u32::from(pins[1]),0).unwrap();assert_eq!(command(&mut b,58,0),[255;2]);route(&mut b,pins[1],103);
        b.write32(0x6000_42ec,0x80|2).unwrap();assert_eq!(command(&mut b,58,0),[255;2]);b.write32(0x6000_42ec,0x80|u32::from(pins[2])).unwrap();level(&mut b,pins[3],true);level(&mut b,pins[3],false);
        assert_eq!(command(&mut b,24,3),[255,0]);spi(&mut b,&[0xfe,9,9,9]);level(&mut b,pins[3],true);level(&mut b,pins[3],false);assert_eq!(read_block(&mut b),data[1..513],"CS rising aborts a partial block");
        b.board.set_sd_card_present(false);assert_eq!(command(&mut b,0,0),[255;2]);b.board.set_sd_card_present(true);assert_eq!(command(&mut b,0,0),[255;2]);init_card(&mut b,pins[3]);assert_eq!(read_block(&mut b),data[1..513]);
    }
}

#[test]
fn simcam_sd_supply_requires_gpio1_and_power_loss_resets_card_without_erasing_media() {
    let mut b=machine(LilygoIo::t_simcam());spi_card(&mut b,[39,38,40,47]);drive(&mut b,47,true);spi(&mut b,&[255;10]);level(&mut b,47,false);assert_eq!(command(&mut b,0,0),[255;2]);
    drive(&mut b,1,true);init_card(&mut b,47);disable(&mut b,1);assert_eq!(command(&mut b,0,0),[255;2]);drive(&mut b,1,true);assert_eq!(command(&mut b,0,0),[255;2],"supply recovery needs startup clocks");init_card(&mut b,47);
}

fn i2c_route(b:&mut SocBus,pins:(u8,u8)) {for (pin,sig) in [(pins.0,90),(pins.1,89)] {route(b,pin,sig);b.write32(0x6000_4154+4*sig,0x80|u32::from(pin)).unwrap();}}
fn i2c_write(b:&mut SocBus,address:u8,bytes:&[u8])->bool {
    assert!(bytes.len()<31);let base=0x6001_3000;b.write32(base+0x24,u32::MAX).unwrap();b.write32(base+0x18,3<<12).unwrap();
    for byte in std::iter::once(address<<1).chain(bytes.iter().copied()) {b.write32(base+0x1c,u32::from(byte)).unwrap();}
    for (off,value) in [(0x58,6<<11),(0x5c,1<<11|1<<8|(bytes.len()+1) as u32),(0x60,2<<11),(4,1<<5)] {b.write32(base+off,value).unwrap();}b.tick(100_000);b.read32(base+0x20).unwrap()&(1<<10)==0
}
fn i2c_read(b:&mut SocBus,address:u8)->Option<u8> {
    let base=0x6001_3000;for (off,value) in [(0x24,u32::MAX),(0x18,3<<12),(0x1c,u32::from(address)<<1|1),(0x58,6<<11),(0x5c,1<<11|1<<8|1),(0x60,3<<11|1),(0x64,2<<11),(4,1<<5)] {b.write32(base+off,value).unwrap();}b.tick(100_000);
    (b.read32(base+0x20).unwrap()&(1<<10)==0).then(||b.read32(base+0x1c).unwrap() as u8)
}
#[test]
fn camera_oled_and_axp_id_use_real_i2c_and_reject_sccb_pins_unknown_registers() {
    let mut b=machine(LilygoIo::t_camera_s3());i2c_route(&mut b,(5,4));assert!(!i2c_write(&mut b,0x3c,&[0,0xaf]));assert!(!i2c_write(&mut b,0x34,&[3]));
    i2c_route(&mut b,(7,6));assert!(i2c_write(&mut b,0x34,&[3]));assert_eq!(i2c_read(&mut b,0x34),Some(0x4a));assert!(!i2c_write(&mut b,0x34,&[0]),"no fabricated USB/battery status");assert!(!i2c_write(&mut b,0x34,&[3,0]));
    assert!(i2c_write(&mut b,0x34,&[0x30]));assert_eq!(i2c_read(&mut b,0x34),Some(3));assert!(i2c_write(&mut b,0x34,&[0x30,1]));assert!(i2c_write(&mut b,0x34,&[0x30]));assert_eq!(i2c_read(&mut b,0x34),Some(1));
    assert!(b.board.display().unwrap().2.iter().all(|&p|p==0));assert!(i2c_write(&mut b,0x3c,&[0,0xaf,0x20,0,0x21,0,127,0x22,0,7]));assert!(i2c_write(&mut b,0x3c,&[0x40,1]));let (w,h,p,version)=b.board.display().unwrap();assert_eq!((w,h,b.board.display_frames()),(128,64,1));assert_eq!(version,b.board.display_version());assert_eq!(p[0],65535);assert_eq!(p.iter().filter(|&&p|p!=0).count(),1);
    assert!(i2c_write(&mut b,0x3c,&[0,0xa1,0xc8]));assert_eq!(b.board.display().unwrap().2[8191],65535);assert!(b.board.display().unwrap().3>version,"command-only changes publish a new display version");assert_eq!(b.board.display_frames(),1);
    b.write32(0x6000_9004+4*7,1<<12).unwrap();assert!(!i2c_write(&mut b,0x3c,&[0,0xae]));assert_eq!(b.board.display().unwrap().2[8191],65535);
}

#[test]
fn sim7080_native_sd_dma_requires_exact_routes_and_explicit_axp_aldo3_supply() {
    use esp_periph::sdmmc::{reg,des0,int,CMD_START,CMD_RESP,CMD_DATA,CMD_WRITE,CMD_RESP_LONG,CTRL_USE_INTERNAL_DMA,CTRL_INT_ENABLE,BMOD_DE};
    fn cmd(b:&mut SocBus,n:u8,arg:u32,flags:u32)->u32 {b.write32(0x6002_8000+reg::RINTSTS,u32::MAX).unwrap();b.write32(0x6002_8000+reg::CMDARG,arg).unwrap();b.write32(0x6002_8000+reg::CMD,CMD_START|1<<16|u32::from(n)|flags).unwrap();b.read32(0x6002_8000+reg::RESP0).unwrap()}
    let mut b=machine(LilygoIo::t_sim7080_s3());b.attach_sd_card(Box::new(MemStore(vec![0;1<<20])),Default::default()).unwrap();i2c_route(&mut b,(15,7));
    for (pin,signal) in [(38,173),(39,179),(40,213)] {route(&mut b,pin,signal);b.write32(0x6000_4154+4*signal,0x80|u32::from(pin)).unwrap();}
    cmd(&mut b,8,0x1aa,CMD_RESP);assert_ne!(b.read32(0x6002_8000+reg::RINTSTS).unwrap()&int::RTO,0);
    assert!(i2c_write(&mut b,0x34,&[0x90,4]));assert!(!b.board.sd_card_slot().unwrap().1);assert!(i2c_write(&mut b,0x34,&[0x94,27]));assert!(!b.board.sd_card_slot().unwrap().1);assert!(i2c_write(&mut b,0x34,&[0x94,28]));assert!(b.board.sd_card_slot().unwrap().1);
    cmd(&mut b,0,0,0);assert_eq!(cmd(&mut b,8,0x1aa,CMD_RESP),0x1aa);for _ in 0..2 {cmd(&mut b,55,0,CMD_RESP);cmd(&mut b,41,0x40ff8000,CMD_RESP);}cmd(&mut b,2,0,CMD_RESP|CMD_RESP_LONG);let rca=cmd(&mut b,3,0,CMD_RESP)&0xffff0000;cmd(&mut b,7,rca,CMD_RESP);
    for write in [true,false] {let at=if write {0x3fc8_8400}else{0x3fc8_8800};for i in 0..512 {b.write8(at+i,if write {(i%251) as u8}else{0}).unwrap();}
        for (i,value) in [des0::OWN|des0::FS|des0::LD,512,at,0].into_iter().enumerate() {b.write32(0x3fc8_8100+4*i as u32,value).unwrap();}
        for (reg,value) in [(reg::CTRL,CTRL_USE_INTERNAL_DMA|CTRL_INT_ENABLE),(reg::BMOD,BMOD_DE),(reg::DBADDR,0x3fc8_8100),(reg::BLKSIZ,512),(reg::BYTCNT,512)] {b.write32(0x6002_8000+reg,value).unwrap();}
        cmd(&mut b,if write {24}else{17},8,CMD_RESP|CMD_DATA|if write {CMD_WRITE}else{0});assert_ne!(b.read32(0x6002_8000+reg::RINTSTS).unwrap()&int::DATA_OVER,0);if !write {for i in 0..512 {assert_eq!(b.read8(at+i).unwrap(),(i%251) as u8);}}
    }
    assert!(i2c_write(&mut b,0x34,&[0x90,2]));cmd(&mut b,8,0x1aa,CMD_RESP);assert_ne!(b.read32(0x6002_8000+reg::RINTSTS).unwrap()&int::RTO,0);assert!(i2c_write(&mut b,0x34,&[0x90,4]));
    b.write32(0x6000_4154+4*179,0x80|6).unwrap();cmd(&mut b,8,0x1aa,CMD_RESP);assert_ne!(b.read32(0x6002_8000+reg::RINTSTS).unwrap()&int::RTO,0);
    assert_eq!((b.periph.sdmmc.stats.bytes_written,b.periph.sdmmc.stats.bytes_read),(512,512));
}
