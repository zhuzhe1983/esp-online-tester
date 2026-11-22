//! Small authored Xtensa instruction probes, not vendor factory firmware.
//! All display writes go through CPU stores, IO_MUX/GPIO matrix and GP-SPI.
use emu_core::Core;
use esp_soc::{Stop,web::WebServer};

const IRAM:u32=0x4037_0000;
const GPIO:u32=0x6000_4000;
const SPI:u32=0x6002_4000;

fn machine(board:&str)->esp32s3::Machine {
    let mut m=esp32s3::machine([0;6]);
    m.bus.board=esp32s3::board::make_board(board).unwrap();
    m.bus.attach_board_devices();
    // s32i a2,a3,0; j .  The host supplies operands, not peripheral results.
    m.bus.load_bytes(IRAM,&[0x22,0x63,0,0x06,0xff,0xff]).unwrap();
    m.cores[0].ps=0;m.cores[0].set_jit(false);m
}
fn write(m:&mut esp32s3::Machine,address:u32,value:u32) {
    m.cores[0].pc=IRAM;m.cores[0].set_ar(2,value);m.cores[0].set_ar(3,address);
    assert!(matches!(m.run(64),Stop::MaxInsns));
}
fn output(m:&mut esp32s3::Machine,pin:u8,value:bool) {
    write(m,GPIO+if pin<32 {if value {8}else{12}}else if value {20}else{24},1<<(pin%32));
}
fn mode(m:&mut esp32s3::Machine,pin:u8,signal:u32) {
    write(m,0x6000_9004+4*u32::from(pin),1<<12|1<<9);
    write(m,GPIO+0x554+4*u32::from(pin),signal);
    if signal==256 {write(m,GPIO+if pin<32 {0x24}else{0x30},1<<(pin%32));}
}
fn transfer(m:&mut esp32s3::Machine,bytes:&[u8]) {
    if bytes.is_empty() {return;}
    write(m,SPI+0x10,1<<27);
    write(m,SPI+0x1c,(8*bytes.len()-1) as u32);
    for (i,chunk) in bytes.chunks(4).enumerate() {
        let value=chunk.iter().enumerate().fold(0,|v,(j,b)|v|(u32::from(*b)<<(j*8)));
        write(m,SPI+0x98+4*i as u32,value);
    }
    write(m,SPI,1<<24);
}
fn command(m:&mut esp32s3::Machine,dc:Option<u8>,c:u8,data:&[u8]) {
    if let Some(dc)=dc {
        output(m,dc,false);transfer(m,&[c]);output(m,dc,true);transfer(m,data);
    }else{
        // Real GP-SPI command/address registers generate [02,00,CMD,00].
        write(m,SPI+0x10,(1<<31)|(1<<30)|if data.is_empty(){0}else{1<<27});
        write(m,SPI+0x18,(7<<28)|2);write(m,SPI+0x14,23<<27);write(m,SPI+4,u32::from(c)<<16);
        if !data.is_empty() {
            write(m,SPI+0x1c,(data.len()*8-1) as u32);
            let value=data.iter().enumerate().fold(0,|v,(j,b)|v|(u32::from(*b)<<(j*8)));
            write(m,SPI+0x98,value);
        }
        write(m,SPI,1<<24);
    }
}

#[test]
fn guest_stores_drive_exact_pro_bar_and_amoled_pins_and_visible_windows() {
    // Manufacturer source values independent of the profile table: newest
    // External Pro, narrow Bar and AMOLED1.64 (including its separate enable).
    for (id,pins,dc,reset,light,enable,size,origin) in [
        ("lilygo-t-display-s3-pro-external",[18,17,39],Some(9),47,Some((48,true)),None,(222,480),(49u16,0u16)),
        ("lilygo-t-display-bar",[7,6,8],Some(5),40,Some((15,false)),None,(76,284),(82,18)),
        ("lilygo-t-display-s3-amoled-1-64",[12,11,10],None,17,None,Some(16),(280,456),(20,0)),
    ] {
        let mut m=machine(id);
        for (pin,signal) in [(pins[0],101),(pins[1],103),(pins[2],110)] {mode(&mut m,pin,signal);}
        write(&mut m,SPI+0x20,0x3e);
        for p in [Some(reset),dc,enable,light.map(|p|p.0)].into_iter().flatten() {mode(&mut m,p,256);output(&mut m,p,true);}
        if let Some((pin,active))=light {output(&mut m,pin,active);}
        for (c,data) in [(0x11,vec![]),(0x29,vec![]),(0x3a,vec![0x55])] {command(&mut m,dc,c,&data);}
        let [x0,x1]=origin.0.to_be_bytes();let [y0,y1]=origin.1.to_be_bytes();
        command(&mut m,dc,0x2a,&[x0,x1,x0,x1]);command(&mut m,dc,0x2b,&[y0,y1,y0,y1]);command(&mut m,dc,0x2c,&[0xf8,0]);
        m.bus.flush_ticks();let frame=m.bus.board.display().unwrap();
        assert_eq!((frame.0,frame.1),size,"{id}");assert_eq!(frame.2[0],0xf800,"{id}: {}",m.bus.board.report());
        mode(&mut m,pins[1],104);command(&mut m,dc,0x2c,&[0,0x1f]);
        assert_eq!(m.bus.board.display().unwrap().2[0],0xf800,"wrong MOSI route must not write {id}");
        mode(&mut m,pins[1],103);output(&mut m,reset,false);
        assert!(m.bus.board.display().unwrap().2.iter().all(|p|*p==0));
        output(&mut m,reset,true);
        assert!(m.bus.board.display().unwrap().2.iter().all(|p|*p==0),"reset needs a fresh DCS initialization");
    }
}

#[test]
fn web_pro_buttons_reach_guest_gpio_and_release_while_paused() {
    let mut m=machine("lilygo-t-display-s3-pro-external");
    let web=WebServer::queued();m.web=Some(web.clone());
    for pin in [0,12,16] {write(&mut m,0x6000_9004+4*pin,1<<12|1<<9);}
    m.bus.load_bytes(IRAM+16,&[0x22,0x23,0,0x06,0xff,0xff]).unwrap(); // l32i a2,a3,0; j .
    let read=|m:&mut esp32s3::Machine| {m.cores[0].pc=IRAM+16;m.cores[0].set_ar(3,GPIO+0x3c);assert!(matches!(m.run(64),Stop::MaxInsns));m.cores[0].get_ar(2)&0x1_1001};
    assert_eq!(read(&mut m),0x1_1001);
    for pin in [0,12,16] {
        web.push_incoming(format!(r#"{{"t":"btn","pin":{pin},"v":1}}"#));assert_eq!(read(&mut m),0x1_1001&!(1<<pin));
        web.push_incoming(format!(r#"{{"t":"btn","pin":{pin},"v":0}}"#));m.run(0);assert_eq!(read(&mut m),0x1_1001);
    }
}
