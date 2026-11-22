//! Real classic ESP32 MMIO paths, not host-written framebuffer fixtures.
//! Register locations/selectors: ESP-IDF v5.5.4 ESP32 gpio_reg.h,
//! gpio_sig_map.h, io_mux_reg.h, i2c_reg.h and rmt_reg.h. The classic adapter
//! preserves the corresponding IDF4.4 registers; no S3 addresses are used.
use emu_core::Bus;
use esp32::bus::SocBus;
use esp_soc::board::BoardModel;
use esp_periph::i2c::INT_NACK;
#[path="../src/board/lilygo_additional.rs"] mod additional;

const GPIO:u32=0x3ff4_4000;
const MUX:u32=0x3ff4_9000;
const OFFSETS:[u32;40]=[0x44,0x88,0x40,0x84,0x48,0x6c,0x60,0x64,0x68,0x54,0x58,0x5c,0x34,0x38,0x30,0x3c,0x4c,0x50,0x70,0x74,0x78,0x7c,0x80,0x8c,0x90,0x24,0x28,0x2c,0,0,0,0,0x1c,0x20,0x14,0x18,4,8,0x0c,0x10];
fn put(b:&mut SocBus,a:u32,v:u32){b.write32(a,v).unwrap();}
fn board(id:&str)->SocBus {let mut b=SocBus::new(4<<20,[0;6]);b.board=additional::make_board(id).unwrap();b.attach_board_devices();b}
fn mux(b:&mut SocBus,pin:u8,function:u32){put(b,MUX+OFFSETS[pin as usize],(function<<12)|(1<<9)|(1<<8));}
fn output(b:&mut SocBus,pin:u8,on:bool){mux(b,pin,2);put(b,GPIO+0x530+4*u32::from(pin),256);put(b,GPIO+if pin<32{0x24}else{0x30},1<<(pin%32));level(b,pin,on);}
fn level(b:&mut SocBus,pin:u8,on:bool){put(b,GPIO+match(pin<32,on){(true,true)=>8,(true,false)=>12,(false,true)=>20,(false,false)=>24},1<<(pin%32));}
fn i2c_route(b:&mut SocBus,port:usize,sda:u8,scl:u8){let(scl_signal,sda_signal)=[(29,30),(95,96)][port];for(pin,signal)in[(sda,sda_signal),(scl,scl_signal)]{mux(b,pin,2);put(b,GPIO+0x130+4*signal,(1<<7)|u32::from(pin));put(b,GPIO+0x530+4*u32::from(pin),signal);}}
fn i2c(b:&mut SocBus,port:usize,address:u8,data:&[u8])->u32 {
    let base=[0x3ff5_3000,0x3ff6_7000][port];put(b,base+0x24,u32::MAX);put(b,base+0x18,3<<12);put(b,base+0x1c,u32::from(address)<<1);
    for &byte in data{put(b,base+0x1c,u32::from(byte));}
    for(i,command)in[0,(1<<11)|(1<<8)|(1+data.len()as u32),3<<11].into_iter().enumerate(){put(b,base+0x58+4*i as u32,command);}put(b,base+4,1<<5);b.read32(base+0x20).unwrap()
}
fn rmt(b:&mut SocBus,pin:u8,grb:&[u8]){
    const RMT:u32=0x3ff5_6000;put(b,0x3ff0_00c0,1<<9);mux(b,pin,2);put(b,GPIO+0x530+4*u32::from(pin),87);
    let bits:Vec<_>=grb.iter().flat_map(|byte|(0..8).rev().map(move|bit|byte&(1<<bit)!=0)).collect();
    for(i,&one)in bits.iter().enumerate(){let hi=if one{64}else{32};let lo=100-hi;put(b,RMT+0x800+4*i as u32,hi|0x8000|(lo<<16));}put(b,RMT+0x800+4*bits.len()as u32,0);
    put(b,RMT+0x20,(1<<24)|1);put(b,RMT+0x24,(1<<17)|(1<<19)|(1<<3)|1);b.tick(30_000);
}

#[test]
fn relay_coils_and_indicators_follow_mux_and_enable_not_only_latches(){
    for(id,pins)in[("lilygo-t-relay",&[21u8,19,18,5][..]),("lilygo-t-relay-5v-8-channel-relay",&[33u8,32,13,12,21,19,18,5][..])]{
        let mut b=board(id);assert!(!b.board.uses_gpio_waveform());
        for(i,&pin)in pins.iter().enumerate(){output(&mut b,pin,true);let leds=b.board.leds().unwrap().0;assert_eq!(leds[..pins.len()].iter().filter(|c|**c==[0,0,255]).count(),i+1);}
        let pin=pins[0];mux(&mut b,pin,3);assert_eq!(b.board.leds().unwrap().0[0],[0;3],"wrong mux disconnects an asserted coil");mux(&mut b,pin,2);assert_eq!(b.board.leds().unwrap().0[0],[0,0,255]);
        put(&mut b,GPIO+if pin<32{0x28}else{0x34},1<<(pin%32));assert_eq!(b.board.leds().unwrap().0[0],[0;3],"disabled output cannot energize relay");
        output(&mut b,25,true);assert_eq!(*b.board.leds().unwrap().0.last().unwrap(),[0,0,255]);
        assert!(b.board.hardware_diagnostics().unwrap().contains("\"physicalContactDynamics\":false"));
        for &pin in pins{output(&mut b,pin,false);}assert!(b.board.leds().unwrap().0[..pins.len()].iter().all(|c|*c==[0;3]));
    }
}

#[test]
fn real_rmt_symbols_reach_only_each_selected_rgb_pin(){
    for(id,pin,count)in[("lilygo-t-lora-dual",5,1),("lilygo-t-can485",4,1),("lilygo-t-encoder-esp32",25,2)]{
        let mut b=board(id);let grb=[0x22,0x44,0x66].repeat(count);rmt(&mut b,22,&grb);assert_eq!(b.board.leds().unwrap().1,0);
        // Disconnect the old matrix output before routing the same channel again.
        put(&mut b,GPIO+0x530+4*22,256);rmt(&mut b,pin,&grb);assert_eq!(b.board.leds().unwrap().0,vec![[0x44,0x22,0x66];count]);
        assert_eq!(b.board.leds().unwrap().1,1);assert_eq!(b.periph.rmt.tx_count,2);
    }
}

#[test]
fn oled_i2c_windows_go_through_both_classic_controllers_and_real_routes(){
    for id in["lilygo-t-beam-meshtastic","lilygo-t-beam-softrf","lilygo-helium-t-beam","lilygo-lora-v1-3","lilygo-lora3","lilygo-t-lite-w5500","lilygo-t3-tcxo"]{
        let mut b=board(id);if id=="lilygo-t3-tcxo"{output(&mut b,4,true);}
        i2c_route(&mut b,0,21,22);assert_eq!(i2c(&mut b,0,0x3c,&[0,0xaf,0xa0,0xc0,0x20,0,0x21,1,1,0x22,0,0])&INT_NACK,0);
        assert_eq!(i2c(&mut b,0,0x3c,&[0x40,4])&INT_NACK,0);let(w,h,pixels,version)=b.board.display().unwrap();assert_eq!((w,h),(128,64));assert_eq!(pixels[2*128+1],0xffff);assert_eq!(pixels.iter().filter(|p|**p!=0).count(),1);
        i2c_route(&mut b,1,21,22);assert_eq!(i2c(&mut b,1,0x3c,&[0,0xae])&INT_NACK,0);assert!(b.board.display().unwrap().2.iter().all(|p|*p==0));assert!(b.board.display_version()>version);
        assert_eq!(i2c(&mut b,1,0x3c,&[0,0xaf])&INT_NACK,0);assert_eq!(b.board.display().unwrap().2[257],0xffff);
        i2c_route(&mut b,1,22,21);assert_ne!(i2c(&mut b,1,0x3c,&[0,0xae])&INT_NACK,0,"crossed wires cannot reach OLED");
        i2c_route(&mut b,1,21,22);assert_ne!(i2c(&mut b,1,0x3d,&[0,0xae])&INT_NACK,0,"wrong address must NACK");
        if id=="lilygo-t3-tcxo"{level(&mut b,4,false);assert_ne!(i2c(&mut b,1,0x3c,&[0,0xaf])&INT_NACK,0);assert!(b.board.display().unwrap().2.iter().all(|p|*p==0));}
    }
}

fn spi(b:&mut SocBus,dc:bool,data:&[u8]){
    spi_at(b,16,5,dc,data);
}
fn spi_at(b:&mut SocBus,dc_pin:u8,cs_pin:u8,dc:bool,data:&[u8]){
    const SPI:u32=0x3ff6_5000;level(b,dc_pin,dc);level(b,cs_pin,false);
    for(i,chunk)in data.chunks(4).enumerate(){let mut bytes=[0;4];bytes[..chunk.len()].copy_from_slice(chunk);put(b,SPI+0x80+4*i as u32,u32::from_le_bytes(bytes));}
    put(b,SPI+0x1c,1<<27);put(b,SPI+0x28,data.len()as u32*8-1);put(b,SPI,1<<18);level(b,cs_pin,true);
}
fn cmd(b:&mut SocBus,c:u8,data:&[u8]){spi(b,false,&[c]);if!data.is_empty(){spi(b,true,data);}}
#[test]
fn keyboard_edition_base_tdisplay_has_native_glass_and_reset_gates(){
    let mut b=board("lilygo-t-display-keyboard");for(pin,on)in[(5,true),(16,false),(23,true),(4,true)]{output(&mut b,pin,on);}
    for(pin,signal)in[(18,63),(19,65)]{mux(&mut b,pin,2);put(&mut b,GPIO+0x530+4*pin as u32,signal);}
    cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x36,&[8]);cmd(&mut b,0x2a,&[0,52,0,52]);cmd(&mut b,0x2b,&[0,40,0,40]);cmd(&mut b,0x2c,&[0xf8,0]);
    let(w,h,pixels,v)=b.board.display().unwrap();assert_eq!((w,h),(135,240));assert_eq!(pixels[0],0xf800);
    level(&mut b,4,false);assert!(b.board.display().unwrap().2.iter().all(|p|*p==0));assert!(b.board.display_version()>v);level(&mut b,4,true);
    mux(&mut b,19,3);cmd(&mut b,0x2c,&[0,0x1f]);assert_eq!(b.board.display().unwrap().2[0],0xf800,"wrong MOSI mux cannot write pixels");
    level(&mut b,23,false);assert!(b.board.display().unwrap().2.iter().all(|p|*p==0));
}

#[test]
fn named_inputs_are_actual_classic_gpio_levels_and_encoder_channels(){
    for(id,name,pin)in[("lilygo-t-beam-meshtastic","user",38),("lilygo-t-lora-dual","boot",0),("lilygo-t-encoder-esp32","encoder-press",38),("lilygo-t-display-keyboard","user",35)]{
        let mut b=board(id);assert_eq!(b.board.named_pin(name),Some(pin));mux(&mut b,pin,2);let reg=GPIO+if pin<32{0x3c}else{0x40};assert_ne!(b.read32(reg).unwrap()&(1<<(pin%32)),0);
        esp_soc::SocBus::gpio_set_input(&mut b,pin,false);assert_eq!(b.read32(reg).unwrap()&(1<<(pin%32)),0);esp_soc::SocBus::gpio_set_input(&mut b,pin,true);assert_ne!(b.read32(reg).unwrap()&(1<<(pin%32)),0);
    }
    let b=board("lilygo-t-encoder-esp32");assert_eq!(b.board.encoder(),Some((36,37)));assert!(b.board.display().is_none());assert!(b.board.named_pin("boot").is_none());
    assert!(additional::make_board("lilygo-t-lora-c6").is_none());assert!(additional::make_board("lilygo-t-encoder-pro").is_none());
}

#[test]
fn cold_route_opt_in_preserves_legacy_waveform_callbacks(){
    use std::sync::{Arc,Mutex};use esp_soc::pins::PinRoutes;
    struct Legacy(Arc<Mutex<u32>>);impl BoardModel for Legacy{fn name(&self)->&'static str{"test-legacy"}fn uses_gpio_waveform(&self)->bool{true}fn gpio_waveform_at(&mut self,_:u64,_:PinRoutes<'_>){*self.0.lock().unwrap()+=1;}}
    struct Cold(Arc<Mutex<u32>>);impl BoardModel for Cold{fn name(&self)->&'static str{"test-cold"}fn uses_gpio_routes(&self)->bool{true}fn gpio_waveform_at(&mut self,_:u64,_:PinRoutes<'_>){*self.0.lock().unwrap()+=1;}}
    let a=Arc::new(Mutex::new(0));let c=Arc::new(Mutex::new(0));let models:Vec<Box<dyn BoardModel>>=vec![Box::new(Legacy(a.clone())),Box::new(Cold(c.clone()))];
    for model in models{let mut b=SocBus::new(4<<20,[0;6]);b.board=model;b.attach_board_devices();put(&mut b,MUX+0x44,2<<12);put(&mut b,GPIO+8,1);b.tick(100);}
    assert_eq!(*a.lock().unwrap(),3);assert_eq!(*c.lock().unwrap(),3);
}

#[test]
fn independent_classic_revisions_keep_their_button_and_led_polarities(){
    for(id,pin,active,color)in[("lilygo-helium-t-beam",4,false,[255,0,0]),("lilygo-a-t-pcie",12,false,[0,0,255]),("lilygo-t-sim7000g",12,false,[0,0,255]),("lilygo-t-sim7070g",12,false,[0,0,255]),("lilygo-t-sim7600",12,true,[0,0,255]),("lilygo-t-sim7600e-l1",12,true,[0,0,255]),("lilygo-t-call-v1-4",13,true,[0,0,255]),("lilygo-t7-v1-3-mini-32-esp32",22,false,[0,255,0])]{
        let mut b=board(id);assert_eq!(b.board.leds().unwrap().0,[[0;3]]);output(&mut b,pin,active);assert_eq!(b.board.leds().unwrap().0,[color]);
        mux(&mut b,pin,3);assert_eq!(b.board.leds().unwrap().0,[[0;3]]);output(&mut b,pin,!active);assert_eq!(b.board.leds().unwrap().0,[[0;3]]);
    }
    for(id,name,pin)in[("lilygo-t-poe-pro","boot",0),("lilygo-t-a7608e-h","boot",0),("lilygo-t-sim-a7670e","boot",0),("lilygo-t-weigh","boot",0),("lilygo-t7-v1-3-mini-32-esp32","boot",0),("lilygo-t-watch-2021","button-2",35),("lilygo-t-watch-2021","button-3",34),("lilygo-t-higrow","user",35)]{
        let mut b=board(id);assert_eq!(b.board.named_pin(name),Some(pin));mux(&mut b,pin,2);let reg=GPIO+if pin<32{0x3c}else{0x40};let mask=1<<(pin%32);assert_ne!(b.read32(reg).unwrap()&mask,0);
        esp_soc::SocBus::gpio_set_input(&mut b,pin,false);assert_eq!(b.read32(reg).unwrap()&mask,0);esp_soc::SocBus::gpio_set_input(&mut b,pin,true);assert_ne!(b.read32(reg).unwrap()&mask,0);
    }
    // These are PMIC/modem power keys, not sourced ESP GPIO switches.
    for id in["lilygo-a-t-pcie","lilygo-t-sim7000g","lilygo-t-sim7070g","lilygo-t-sim7600","lilygo-t-call-v1-4"]{assert!(board(id).board.named_pin("boot").is_none());}
    assert!(board("lilygo-t-bao").board.display().is_none(),"K210 LCD is not an ESP32 display");
}

#[test]
fn watch2021_v12_uses_its_gc9a01_rail_and_three_real_inputs(){
    let mut b=board("lilygo-t-watch-2021");for(pin,on)in[(15,true),(19,false),(27,true),(21,true),(5,false)]{output(&mut b,pin,on);}
    for(pin,signal)in[(14,63),(13,65)]{mux(&mut b,pin,2);put(&mut b,GPIO+0x530+4*pin as u32,signal);}
    let command=|b:&mut SocBus,c:u8,d:&[u8]|{spi_at(b,19,15,false,&[c]);if !d.is_empty(){spi_at(b,19,15,true,d);}};
    command(&mut b,0x11,&[]);command(&mut b,0x29,&[]);assert!(b.board.display().unwrap().2.iter().all(|v|*v==0),"rail is off");
    level(&mut b,5,true);command(&mut b,0x11,&[]);command(&mut b,0x29,&[]);command(&mut b,0x3a,&[0x55]);command(&mut b,0x36,&[8]);command(&mut b,0x2a,&[0,120,0,120]);command(&mut b,0x2b,&[0,120,0,120]);command(&mut b,0x2c,&[0xf8,0]);
    let(w,h,p,v)=b.board.display().unwrap();assert_eq!((w,h),(240,240));assert_eq!(p[120*240+120],0xf800);
    level(&mut b,21,false);assert!(b.board.display().unwrap().2.iter().all(|v|*v==0));assert!(b.board.display_version()>v);level(&mut b,21,true);assert_eq!(b.board.display().unwrap().2[120*240+120],0xf800);
    level(&mut b,5,false);command(&mut b,0x2c,&[0,0x1f]);level(&mut b,5,true);assert_eq!(b.board.display().unwrap().2[120*240+120],0xf800,"unpowered write is ignored");
    level(&mut b,27,false);assert!(b.board.display().unwrap().2.iter().all(|v|*v==0));
}

fn ttv_soft_spi(b:&mut SocBus,dc:bool,bytes:&[u8]) {
    level(b,18,false);level(b,19,dc);level(b,5,false);
    for &byte in bytes {for bit in(0..8).rev(){level(b,23,byte&(1<<bit)!=0);level(b,18,true);level(b,18,false);}}
    level(b,5,true);
}
#[test]
fn transparent_tv_ssd1309_receives_software_and_hardware_spi_with_real_power_gates(){
    let mut b=board("lilygo-t-tv");for(pin,on)in[(18,false),(23,false),(5,true),(19,false),(4,true),(2,true),(33,false)]{output(&mut b,pin,on);}
    ttv_soft_spi(&mut b,false,&[0xfd,0x12,0xaf,0x20,0,0x21,2,2,0x22,0,0]);ttv_soft_spi(&mut b,true,&[4]);assert!(b.board.display().unwrap().2.iter().all(|v|*v==0));
    level(&mut b,33,true);ttv_soft_spi(&mut b,false,&[0xfd,0x12,0xaf,0xa0,0xc0,0x20,0,0x21,2,2,0x22,0,0]);ttv_soft_spi(&mut b,true,&[4]);assert_eq!(b.board.display().unwrap().2[258],0xffff);assert_eq!(b.board.display_frames(),1);
    // MOSI on a non-GPIO mux must not clock the software endpoint.
    mux(&mut b,23,3);ttv_soft_spi(&mut b,false,&[0xae]);assert_eq!(b.board.display().unwrap().2[258],0xffff);mux(&mut b,23,2);
    // The same electrical endpoint accepts properly routed peripheral SPI too.
    for(pin,signal)in[(18,63),(23,65)]{put(&mut b,GPIO+0x530+4*pin as u32,signal);}
    spi_at(&mut b,19,5,false,&[0xae]);assert!(b.board.display().unwrap().2.iter().all(|v|*v==0));spi_at(&mut b,19,5,false,&[0xaf]);assert_eq!(b.board.display().unwrap().2[258],0xffff);
    level(&mut b,2,false);spi_at(&mut b,19,5,false,&[0xae]);assert!(b.board.display().unwrap().2.iter().all(|v|*v==0));level(&mut b,2,true);assert!(b.board.display().unwrap().2.iter().all(|v|*v==0),"restored power must not invent the old SRAM image");spi_at(&mut b,19,5,false,&[0xaf,0x20,0,0x21,2,2,0x22,0,0]);spi_at(&mut b,19,5,true,&[4]);assert_eq!(b.board.display().unwrap().2[258],0xffff);
    level(&mut b,4,false);assert!(b.board.display().unwrap().2.iter().all(|v|*v==0));
}

#[test]
fn bao_coprocessor_executes_lx6_sram_and_uart_without_claiming_the_k210_display(){
    use emu_core::Core;
    const IRAM:u32=0x4008_0000;
    let mut m=esp32::machine([0;6],16<<20);m.bus.board=additional::make_board("lilygo-t-bao").unwrap();m.bus.attach_board_devices();m.console.capture=true;
    // Independent authored S32I / L32I instructions followed by a local loop.
    m.bus.load_bytes(IRAM,&[0x22,0x63,0,0x06,0xff,0xff,0,0,0x22,0x23,0,0x06,0xff,0xff]).unwrap();m.cores[0].ps=0;m.cores[0].set_jit(false);
    for(address,value)in[(0x3ffb_1000,0x5372_614d),(0x3ff4_0000,u32::from(b'K'))]{m.cores[0].pc=IRAM;m.cores[0].set_ar(2,value);m.cores[0].set_ar(3,address);assert!(matches!(m.run(64),esp32::Stop::MaxInsns));}
    m.cores[0].pc=IRAM+8;m.cores[0].set_ar(3,0x3ffb_1000);assert!(matches!(m.run(64),esp32::Stop::MaxInsns));assert_eq!(m.cores[0].get_ar(2),0x5372_614d);m.drain_console();assert_eq!(m.console.all,b"K");
    assert!(m.bus.board.display().is_none());assert!(m.bus.board.i2c_devices().is_empty());assert!(m.bus.board.leds().is_none());
}
