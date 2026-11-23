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

#[test]
fn eight_more_exact_boards_expose_only_present_controls_and_source_proven_gpio() {
    for mut board in [LilygoIo::t_keyboard_s3_pro(),LilygoIo::t_micro32_s3(),LilygoIo::t_panel_s3(),LilygoIo::t_camera_plus_s3(),LilygoIo::t_vending(),LilygoIo::t_track(),LilygoIo::t_fpga(),LilygoIo::t7_s3()] {
        assert!(!board.uses_gpio_waveform());assert!(board.camera_frame().is_none());assert!(board.lora_snapshot().is_none());
        if board.name()=="lilygo-t-micro32-s3" {assert!(board.input_levels().is_empty());assert_eq!(board.named_pin("boot"),None);assert!(!board.button_at(10,"boot",true));assert!(board.i2c_devices().is_empty());assert!(board.display().is_none());assert!(board.leds().is_none());}
        else {assert_eq!(board.named_pin("boot"),Some(0));assert!(board.button_at(10,"boot",true));assert_eq!(board.take_edges()[0].pin,0);assert!(board.button_at(11,"boot",false));assert!(board.take_edges()[0].level);}
        assert!(!board.button_at(12,"reset",true));assert!(!board.button_at(12,"power",true));
    }
    for (board,pin,color) in [(LilygoIo::t7_s3(),17,[0,255,0]),(LilygoIo::t_fpga(),46,[0,0,255])] {
        let mut b=machine(board);drive(&mut b,pin,true);assert_eq!(b.board.leds().unwrap().0,[color]);disable(&mut b,pin);assert_eq!(b.board.leds().unwrap().0,[[0;3]]);
        drive(&mut b,pin,true);route(&mut b,pin,256|1<<9);assert_eq!(b.board.leds().unwrap().0,[[0;3]]);drive(&mut b,pin,true);b.write32(0x6000_9004+4*u32::from(pin),0).unwrap();assert_eq!(b.board.leds().unwrap().0,[[0;3]]);
    }
    let mut b=machine(LilygoIo::t_fpga());i2c_route(&mut b,(38,39));assert!(i2c_write(&mut b,0x34,&[3]));assert_eq!(i2c_read(&mut b,0x34),Some(0x4a));assert!(!i2c_write(&mut b,0x34,&[0]));assert!(b.board.report().contains("FPGA/Cortex-M3"));
    let mut b=machine(LilygoIo::t_vending());assert!(b.board.report().contains("capacity unknown"));drive(&mut b,16,true);drive(&mut b,4,true);assert!(b.board.report().contains("pulse drive=Some(true), GPIO4 peripheral supply=true"));disable(&mut b,16);assert!(b.board.report().contains("pulse drive=None"));assert!(b.board.leds().is_none());
}

fn screen_spi(b:&mut SocBus,clk:u8,mosi:u8,dc:u8,cs:Option<u8>) {route(b,clk,101);route(b,mosi,103);drive(b,dc,false);if let Some(cs)=cs {drive(b,cs,false);}b.write32(0x6002_4020,0x3f).unwrap();}
fn dcs(b:&mut SocBus,dc:u8,cmd:u8,data:&[u8]) {level(b,dc,false);spi(b,&[cmd]);if !data.is_empty() {level(b,dc,true);spi(b,data);}}
fn pixel(b:&mut SocBus,dc:u8,x:u16,y:u16,color:u16) {let x=x.to_be_bytes();let y=y.to_be_bytes();dcs(b,dc,0x2a,&[x[0],x[1],x[0],x[1]]);dcs(b,dc,0x2b,&[y[0],y[1],y[0],y[1]]);dcs(b,dc,0x2c,&color.to_be_bytes());}
fn keyboard_init(b:&mut SocBus) {dcs(b,39,0x11,&[]);dcs(b,39,0x3a,&[5]);dcs(b,39,0x36,&[0xc8]);dcs(b,39,0x21,&[]);dcs(b,39,0x29,&[]);}
#[test]
fn keyboard_original_stm32_delays_cs_and_key_snapshots_and_keeps_four_panels_independent() {
    let mut b=machine(LilygoIo::t_keyboard_s3_pro());screen_spi(&mut b,41,40,39,None);drive(&mut b,38,true);drive(&mut b,1,false);
    i2c_route(&mut b,(6,7));assert!(!i2c_write(&mut b,1,&[1,31]));i2c_route(&mut b,(42,2));assert!(i2c_write(&mut b,1,&[1,31]));keyboard_init(&mut b);pixel(&mut b,39,2,1,0xf800);assert_eq!(b.board.display_frames(),0,"first CS write has not yet reached the original STM32 address callback");
    assert!(i2c_write(&mut b,1,&[1,31]));keyboard_init(&mut b);pixel(&mut b,39,2,1,0xf800);let (w,h,p,v)=b.board.display().unwrap();assert_eq!((w,h),(512,128));assert_eq!([p[0],p[128],p[256],p[384]],[0xf800;4]);assert_eq!(v,b.board.display_version());
    assert!(i2c_write(&mut b,1,&[1,8]));pixel(&mut b,39,2,1,0x07e0);let p=b.board.display().unwrap().2;assert_eq!([p[0],p[128],p[256],p[384]],[0x07e0;4],"new selection is still pending");
    assert!(i2c_write(&mut b,1,&[1,8]));pixel(&mut b,39,2,1,0x001f);let p=b.board.display().unwrap().2;assert_eq!([p[0],p[128],p[256],p[384]],[0x07e0,0x001f,0x07e0,0x07e0]);
    let version=b.board.display_version();route(&mut b,40,103|1<<9);pixel(&mut b,39,2,1,65535);assert_eq!(b.board.display_version(),version,"inverted MOSI route is not a valid frame");route(&mut b,40,103);
    assert!(i2c_write(&mut b,1,&[1,1]));assert!(i2c_write(&mut b,1,&[1,1]));let frames=b.board.display_frames();pixel(&mut b,39,2,1,65535);assert_eq!(b.board.display_frames(),frames,"5th bit is the unattached expansion slot, not a fabricated fifth screen");
    assert!(b.board.button_at(40,"key1",true));assert!(b.board.button_at(41,"knob",true));assert_eq!(b.board.take_edges(),[esp_soc::board::BoardEdge{cycle:41,pin:0,level:false}]);assert_eq!(b.board.encoder(),Some((4,5)));
    assert!(i2c_write(&mut b,1,&[2]));assert!(i2c_write(&mut b,1,&[2]));assert_eq!(i2c_read(&mut b,1),Some(0x11));assert!(b.board.button_at(42,"key1",false));assert_eq!(i2c_read(&mut b,1),Some(0x11),"read returns retained key snapshot");assert!(i2c_write(&mut b,1,&[2]));assert_eq!(i2c_read(&mut b,1),Some(1));
    assert!(i2c_write(&mut b,1,&[0x10]));assert!(i2c_write(&mut b,1,&[0x10]));assert_eq!(i2c_read(&mut b,1),Some(2));assert!(!i2c_write(&mut b,1,&[3,0]),"LED mode registers are explicitly unsupported");
    level(&mut b,1,true);assert!(b.board.display().unwrap().2.iter().all(|&p|p==0));level(&mut b,1,false);assert_eq!(b.board.display().unwrap().2[128],0x001f);level(&mut b,38,false);assert!(b.board.display().unwrap().2.iter().all(|&p|p==0));
}

#[test]
fn camera_plus_v12_spi_window_touch_and_user_button_do_not_alias_v10_or_capture_camera() {
    let mut b=machine(LilygoIo::t_camera_plus_s3());screen_spi(&mut b,35,34,45,Some(36));drive(&mut b,21,true);drive(&mut b,46,true);
    dcs(&mut b,45,0x11,&[]);dcs(&mut b,45,0x3a,&[0x55]);dcs(&mut b,45,0x36,&[0]);dcs(&mut b,45,0x21,&[]);dcs(&mut b,45,0x29,&[]);pixel(&mut b,45,0,0,0xf800);assert_eq!(b.board.display().unwrap().2[0],0xf800);
    let frames=b.board.display_frames();pixel(&mut b,45,240,0,65535);assert_eq!(b.board.display_frames(),frames,"outside documented native glass is not folded onto valid pixels");route(&mut b,34,256);route(&mut b,35,103);route(&mut b,36,101);pixel(&mut b,45,0,0,65535);assert_eq!(b.board.display_frames(),frames,"old revision SPI pins do not write V1.2");screen_spi(&mut b,35,34,45,Some(36));
    i2c_route(&mut b,(1,2));assert!(!i2c_write(&mut b,0x15,&[2]));i2c_route(&mut b,(33,37));b.board.touch(700,12,true);for (reg,value) in [(2,1),(3,0),(4,239),(5,0),(6,12)] {assert!(i2c_write(&mut b,0x15,&[reg]));assert_eq!(i2c_read(&mut b,0x15),Some(value));}
    b.board.touch(20,30,false);assert!(i2c_write(&mut b,0x15,&[2]));assert_eq!(i2c_read(&mut b,0x15),Some(0));assert!(!i2c_write(&mut b,0x15,&[0xa7]));assert!(!i2c_write(&mut b,0x6a,&[0]));assert_eq!(b.board.input_levels().iter().find(|p|p.0==47),Some(&(47,true)));assert!(b.board.camera_frame().is_none());assert!(b.board.button_at(50,"user",true));assert_eq!(b.board.take_edges()[0].pin,17);
}

#[test]
fn camera_plus_backlight_uses_real_ledc_duty_clock_mux_and_output_enable() {
    let mut b=machine(LilygoIo::t_camera_plus_s3());screen_spi(&mut b,35,34,45,Some(36));drive(&mut b,46,true);dcs(&mut b,45,0x11,&[]);dcs(&mut b,45,0x3a,&[5]);dcs(&mut b,45,0x21,&[]);dcs(&mut b,45,0x29,&[]);pixel(&mut b,45,0,0,65535);assert_eq!(b.board.display().unwrap().2[0],65535);
    for (addr,value) in [(0x600c0018,1<<11),(0x600190d0,3),(0x600190a0,8|(256<<4)|(1<<25)),(0x60019008,64<<4),(0x6001900c,1<<31),(0x60019000,4|(1<<4))] {b.write32(addr,value).unwrap();}b.tick(10000);route(&mut b,46,73);b.refresh_board_outputs();assert_eq!(b.board.display().unwrap().2[0],(7<<11)|(15<<5)|7);
    route(&mut b,46,73|1<<9);b.refresh_board_outputs();assert_eq!(b.board.display().unwrap().2[0],(23<<11)|(47<<5)|23);disable(&mut b,46);assert_eq!(b.board.display().unwrap().2[0],0);drive(&mut b,46,true);route(&mut b,46,73);b.write32(0x600c0018,0).unwrap();b.refresh_board_outputs();assert_eq!(b.board.display().unwrap().2[0],0);
}

#[test]
fn keyboard_active_low_pwm_and_hardware_reset_update_the_visible_version() {
    let mut b=machine(LilygoIo::t_keyboard_s3_pro());screen_spi(&mut b,41,40,39,None);drive(&mut b,38,true);drive(&mut b,1,false);i2c_route(&mut b,(42,2));for _ in 0..2 {assert!(i2c_write(&mut b,1,&[1,16]));}keyboard_init(&mut b);for i in 0..20 {pixel(&mut b,39,2+i,1,65535);}let version=b.board.display_version();
    for (addr,value) in [(0x600c0018,1<<11),(0x600190d0,3),(0x600190a0,8|(256<<4)|(1<<25)),(0x60019008,64<<4),(0x6001900c,1<<31),(0x60019000,4|(1<<4))] {b.write32(addr,value).unwrap();}b.tick(10000);route(&mut b,1,73);b.refresh_board_outputs();assert_eq!(b.board.display().unwrap().2[0],(23<<11)|(47<<5)|23,"25% high duty means 75% brightness on the low-active transistor");
    route(&mut b,1,73|1<<9);b.refresh_board_outputs();assert_eq!(b.board.display().unwrap().2[0],(7<<11)|(15<<5)|7);disable(&mut b,1);assert_eq!(b.board.display().unwrap().2[0],0);drive(&mut b,1,false);level(&mut b,38,false);assert!(b.board.display_version()>version,"reset must not reuse an old version after clearing a pixel counter");assert!(b.board.display().unwrap().2.iter().all(|&p|p==0));
}

#[test]
fn track_jd9613_pages_mount_orientation_and_shared_sensor_power_follow_factory_pins() {
    let mut b=machine(LilygoIo::t_track());screen_spi(&mut b,5,6,7,Some(9));drive(&mut b,8,true);assert!(!b.board.button_at(1,"track-right",true));dcs(&mut b,7,0x11,&[]);pixel(&mut b,7,125,0,0xf800);assert_eq!(b.board.display_frames(),0);
    drive(&mut b,4,true);dcs(&mut b,7,0xfe,&[1]);dcs(&mut b,7,0x11,&[]);dcs(&mut b,7,0x2c,&[0xf8,0]);assert_eq!(b.board.display_frames(),0,"page1 voltage/gamma data is not DCS RAMWR");dcs(&mut b,7,0xfe,&[0]);dcs(&mut b,7,0x11,&[]);dcs(&mut b,7,0x3a,&[0x55]);dcs(&mut b,7,0x36,&[0]);dcs(&mut b,7,0x51,&[255]);dcs(&mut b,7,0x29,&[]);pixel(&mut b,7,125,0,0xf800);assert_eq!(b.board.display().unwrap().2[0],0,"BCTRL must enable the brightness block");dcs(&mut b,7,0x53,&[0x28]);let (w,h,p,_)=b.board.display().unwrap();assert_eq!((w,h),(294,126));assert_eq!(p[0],0xf800);pixel(&mut b,7,0,0,0x001f);assert_eq!(b.board.display().unwrap().2[125*294],0x001f);
    let frames=b.board.display_frames();pixel(&mut b,7,126,0,65535);assert_eq!(b.board.display_frames(),frames);dcs(&mut b,7,0x3a,&[6]);pixel(&mut b,7,125,0,65535);assert_eq!(b.board.display_frames(),frames,"18-bit mode is not mislabeled as supported RGB565");
    assert!(b.board.button_at(2,"track-right",true));assert_eq!(b.board.take_edges()[0].pin,1);assert!(b.board.button_at(3,"track-down",true));assert_eq!(b.board.take_edges()[0].pin,10);assert_eq!(b.board.display().unwrap().2[0],0xf800,"GPIO10 belongs to sensor, not invalid TFT_BL macro");
    disable(&mut b,4);assert!(b.board.display().unwrap().2.iter().all(|&p|p==0));let edges=b.board.take_edges();assert_eq!(edges.len(),2);assert!(edges.iter().all(|e|e.level));assert!(b.board.leds().is_none(),"conflicting source LED colors are not invented");
}

#[test]
fn camera_plus_and_panel_spi_sd_slots_roundtrip_blocks_on_distinct_exact_buses() {
    for (board,pins) in [(LilygoIo::t_camera_plus_s3(),[35,34,48,21]),(LilygoIo::t_panel_s3(),[36,35,37,38])] {
        let mut b=machine(board);if b.board.name()=="lilygo-t-camera-plus-s3" {drive(&mut b,36,true);}spi_card(&mut b,pins);init_card(&mut b,pins[3]);assert_eq!(command(&mut b,24,3),[255,0]);let mut bytes=vec![0xfe];bytes.extend((0..512).map(|i|(i%251) as u8));bytes.extend([255;2]);spi(&mut b,&bytes);assert_eq!(spi(&mut b,&[255]),[5]);assert_eq!(read_block(&mut b),bytes[1..513]);
        b.write32(0x6000_9004+4*u32::from(pins[1]),0).unwrap();assert_eq!(command(&mut b,58,0),[255;2]);route(&mut b,pins[1],103);level(&mut b,pins[3],true);assert_eq!(command(&mut b,58,0),[255;2]);
    }
}

fn panel_serial(b:&mut SocBus,data:bool,value:u8) {let word=u16::from(value)|u16::from(data)<<8;for bit in (0..9).rev() {let data=if word&(1<<bit)!=0 {64}else{0};assert!(i2c_write(b,0x20,&[3,data]));assert!(i2c_write(b,0x20,&[3,data|32]));}assert!(i2c_write(b,0x20,&[3,128]));}
fn panel_routes(b:&mut SocBus) {for (pin,signal) in [(41,154),(39,151),(40,152)] {route(b,pin,signal);}for (i,pin) in [1,2,3,4,5,6,7,8,9,10,11,12,13,42,46,45].into_iter().enumerate() {route(b,pin,133+i as u32);}drive(b,14,true);}
fn panel_dma(b:&mut SocBus,wide:bool) {
    const DESC:u32=0x3fc88000;const DATA:u32=0x3fc8a000;let total=480*480*2;for i in 0..total/2 {let value=if i==0 {0xf800u16}else if i==1 {0x07e0}else if i==480*479 {0x001f}else{0};b.write16(DATA+2*i,value).unwrap();}
    let mut pos=0;let mut index=0;while pos<total {let len=(total-pos).min(4092);let last=pos+len==total;for (i,value) in [1<<31|u32::from(last)<<30|len<<12|len,DATA+pos,if last {0}else{DESC+12*(index+1)}].into_iter().enumerate() {b.write32(DESC+12*index+4*i as u32,value).unwrap();}pos+=len;index+=1;}
    // IDF v4.4.7 lcd_ll.h: RGB output needs LCD_CLK_EN (bit31) as well as CLK_SEL.
    for (address,value) in [(0x60041014,0),(0x600c001c,1<<8),(0x6003f0a8,5),(0x6003f080,(DESC&0xfffff)|(1<<21)),(0x60041000,(1<<31)|(3<<29)|(20<<9)|(1<<6)),(0x60041020,(479<<8)|(501<<20)),(0x6004101c,(1<<31)|(479<<11)|(518<<21)),(0x60041014,(1<<27)|(1<<24)|if wide {1<<23}else{0})] {b.write32(address,value).unwrap();}
    b.tick(8_000_000);
}
#[test]
fn panel_xl9535_serial_init_and_rgb_dma_require_direction_reset_routes_and_native_geometry() {
    let mut b=machine(LilygoIo::t_panel_s3());i2c_route(&mut b,(17,18));assert!(!i2c_write(&mut b,0x5a,&[0]),"CST3240 touch is not fabricated");assert!(i2c_write(&mut b,0x20,&[6]));assert_eq!(i2c_read(&mut b,0x20),Some(255));assert!(i2c_write(&mut b,0x20,&[2,0x20,0x80]));panel_serial(&mut b,false,0x11);assert!(b.board.report().contains("commands=0"),"latch writes cannot drive input-configured pins");
    assert!(i2c_write(&mut b,0x20,&[6,0xdf,0x1f]));assert!(i2c_write(&mut b,0x20,&[2,0,0x80]));assert!(i2c_write(&mut b,0x20,&[2,0x20,0x80]));panel_serial(&mut b,false,0x11);panel_serial(&mut b,false,0x3a);panel_serial(&mut b,true,0x55);panel_serial(&mut b,false,0x29);panel_routes(&mut b);assert_eq!(b.board.display_frames(),0,"initialization does not create display pixels");panel_dma(&mut b,true);assert_eq!(b.board.display_frames(),1);let p=b.board.display().unwrap().2;assert_eq!([p[0],p[1],p[479*480]],[0xf800,0x07e0,0x001f]);
    let version=b.board.display_version();panel_serial(&mut b,false,0x36);panel_serial(&mut b,true,0x18);let p=b.board.display().unwrap().2;assert_eq!([p[0],p[479*480]],[0xf800,0x001f]);assert!(b.board.display_version()>version,"BGR and ML alter displayed scan, with no fake RAMWR");
    route(&mut b,42,146|1<<9);panel_dma(&mut b,true);assert_eq!(b.board.display_frames(),1,"inverted physical RGB data route rejects a completed DMA frame");route(&mut b,42,146);panel_dma(&mut b,false);assert_eq!(b.board.display_frames(),1,"8-bit DMA is not accepted as RGB565");
    assert!(i2c_write(&mut b,0x20,&[2,0]));panel_dma(&mut b,true);assert_eq!(b.board.display_frames(),1);assert!(b.board.display().unwrap().2.iter().all(|&p|p==0));assert!(b.periph.lcd_cam.lcd_frames>=3,"negative board cases still traverse real LCD DMA");
}
