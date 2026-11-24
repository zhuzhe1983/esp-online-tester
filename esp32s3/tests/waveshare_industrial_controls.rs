//! Real LX7 load/store guests touch actual S3 GPIO/I2C/RMT/SPI MMIO.
//! These are original diagnostics, not manufacturer firmware boot acceptance.
use emu_core::{Bus,Core};
use esp_soc::{SocBus,Stop};
use esp32s3::board::{self,waveshare_industrial_controls::{Profile,memory_mib}};
const PC:u32=0x4037_0000;const GPIO:u32=0x6000_4000;
struct Guest {m:esp32s3::Machine}
impl Guest {
    fn new(name:&str)->Self {let mut m=esp32s3::machine([0;6]);m.bus.board=board::make_board(name).unwrap();m.bus.attach_board_devices();let(f,p)=memory_mib(name).unwrap();m.bus.set_flash_size(f<<20);m.bus.set_psram_size(p<<20).unwrap();m.bus.load_bytes(PC,&[0x32,0x64,0,0x52,0x24,0]).unwrap();m.cores[0].ps=0;m.cores[0].set_jit(false);Self{m}}
    fn put(&mut self,address:u32,value:u32){self.m.cores[0].pc=PC;self.m.cores[0].set_ar(4,address);self.m.cores[0].set_ar(3,value);assert!(matches!(self.m.run(1),Stop::MaxInsns));}
    fn get(&mut self,address:u32)->u32 {self.m.cores[0].pc=PC+3;self.m.cores[0].set_ar(4,address);assert!(matches!(self.m.run(1),Stop::MaxInsns));self.m.cores[0].get_ar(5)}
    fn route(&mut self,pin:u8,signal:u32){self.put(0x6000_9004+4*u32::from(pin),1<<12|1<<9);self.put(GPIO+0x554+4*u32::from(pin),signal);if signal==256{self.put(GPIO+if pin<32{0x24}else{0x30},1<<(pin%32));}}
    fn high(&mut self,pin:u8,value:bool){self.put(GPIO+if pin<32{if value{8}else{12}}else if value{20}else{24},1<<(pin%32));}
    fn i2c_route(&mut self,b:u32,sda:u8,scl:u8){for(p,s)in[(sda,90+b*2),(scl,89+b*2)]{self.route(p,s);self.put(GPIO+0x154+4*s,0x80|u32::from(p));}}
    fn i2c(&mut self,b:u32,address:u8,reg:u8,data:&[u8],n:u8)->Option<Vec<u8>>{
        let base=[0x6001_3000,0x6002_7000][b as usize];self.put(base+0x24,u32::MAX);self.put(base+0x18,3<<12);
        for v in[address<<1,reg].into_iter().chain(data.iter().copied()){self.put(base+0x1c,u32::from(v));}
        let mut commands=vec![6<<11,1<<11|1<<8|(data.len()+2)as u32];if n>0{self.put(base+0x1c,u32::from(address<<1|1));commands.extend([6<<11,1<<11|1<<8|1,3<<11|u32::from(n)]);}commands.push(2<<11);
        for(i,c)in commands.into_iter().enumerate(){self.put(base+0x58+4*i as u32,c);}self.put(base+4,1<<5);self.m.bus.tick(100_000);
        if self.get(base+0x20)&(1<<10)!=0{return None;}assert_ne!(self.get(base+0x20)&(1<<7),0);Some((0..n).map(|_|self.get(base+0x1c)as u8).collect())
    }
    fn rmt(&mut self,bytes:&[u8]) {let bits:Vec<bool>=bytes.iter().flat_map(|b|(0..8).map(move|i|b&(0x80>>i)!=0)).collect();for(i,&b)in bits.iter().enumerate(){self.put(0x6001_6800+4*i as u32,0x8000|if b{64|32<<16}else{32|64<<16});}self.put(0x6001_6800+4*bits.len()as u32,0);self.put(0x6001_6020,4<<16|1<<8|1);self.m.bus.tick(200_000);}
    fn diag(&self)->String {self.m.bus.board.hardware_diagnostics().unwrap()}
}
#[test]
fn all_nine_profiles_keep_exact_memory_i2c_routes_rtc_and_headless_scope(){
    for p in Profile::ALL {let mut g=Guest::new(p.name());assert_eq!(g.m.bus.flash.len(),16<<20);assert_eq!(g.m.bus.psram.len(),8<<20);assert!(g.m.bus.board.display().is_none());assert!(g.m.bus.board.camera_frame().is_none());assert_eq!(g.m.bus.board.named_pin("boot"),Some(0));assert_eq!(g.m.bus.board.leds().map(|x|x.0.len()).unwrap_or(0),p.led_count());
        let(sda,scl)=p.i2c_pins();g.i2c_route(0,sda,scl);assert_eq!(g.i2c(0,0x51,4,&[],1),Some(vec![0x80]));g.i2c(0,0x51,4,&[0x59,0x59,0x23,0x31,6,0x12,0x24],0).unwrap();g.m.bus.tick(240_000_000);g.m.bus.board.advance_to(g.m.bus.cycles);assert_eq!(g.i2c(0,0x51,4,&[],7),Some(vec![0,0,0,1,0,1,0x25]));g.i2c(0,0x51,0,&[0x20],0).unwrap();g.m.bus.tick(480_000_000);g.m.bus.board.advance_to(g.m.bus.cycles);assert_eq!(g.i2c(0,0x51,4,&[],1),Some(vec![0]));
        g.i2c_route(0,scl,sda);assert_eq!(g.i2c(0,0x51,4,&[],1),None);g.i2c_route(1,sda,scl);assert_eq!(g.i2c(1,0x51,4,&[],1),Some(vec![0]));assert_eq!(g.i2c(1,0x50,4,&[],1),None);
    }
    assert!(board::make_board("waveshare-s3-audio-unknown").is_none());
}
#[test]
fn audio_exact_tca9555_pair_wrapping_keys_and_codec_control_ports(){
    for p in[Profile::Audio,Profile::AudioEn]{let mut g=Guest::new(p.name());g.i2c_route(0,11,10);assert_eq!(g.i2c(0,0x18,0xfd,&[],1),Some(vec![0x83]));assert_eq!(g.i2c(0,0x40,0x3d,&[],1),Some(vec![0x72]));assert_eq!(g.i2c(0,0x20,6,&[],4),Some(vec![255;4]));
        g.i2c(0,0x20,2,&[0x12,0x34,0x56],0).unwrap();assert_eq!(g.i2c(0,0x20,2,&[],4),Some(vec![0x56,0x34,0x56,0x34]));assert_eq!(g.i2c(0,0x20,0,&[],2),Some(vec![255;2]),"output latch alone does not drive an input");
        for(id,bit)in[("key-1",1),("key-2",2),("key-3",3)] {assert!(g.m.bus.board.button_at(g.m.bus.cycles,id,true));assert_eq!(g.i2c(0,0x20,1,&[],1),Some(vec![255^(1<<bit)]));assert!(g.m.bus.board.button_at(g.m.bus.cycles,id,false));assert_eq!(g.i2c(0,0x20,1,&[],1),Some(vec![255]));}
        assert!(!g.m.bus.board.button_at(g.m.bus.cycles,"missing-key",true));assert_eq!(g.i2c(0,0x21,0,&[],1),None);g.i2c_route(0,10,11);assert_eq!(g.i2c(0,0x20,0,&[],1),None);
    }
}
#[test]
fn native_rmt_rgb_wire_order_counts_and_wrong_pin_are_observed_by_guests(){
    for p in[Profile::Audio,Profile::AudioEn,Profile::PoeDiDo,Profile::EthDiRoC,Profile::PoeDiRoC] {let mut g=Guest::new(p.name());let data:Vec<u8>=(0..p.led_count()).flat_map(|i|[0x12+i as u8,0x34,0x56]).collect();g.route(37,81);g.rmt(&data);assert_eq!(g.m.bus.board.leds().unwrap().1,0);g.route(37,256);g.route(38,81);g.rmt(&data);let(leds,version)=g.m.bus.board.leds().unwrap();assert!(version>0);for(i,rgb)in leds.iter().enumerate(){assert_eq!(*rgb,[0x12+i as u8,0x34,0x56]);}}
}
#[test]
fn relay1_guest_controls_only_gpio47_with_direction_mux_and_inversion(){
    for p in[Profile::Relay1,Profile::Relay1U]{let mut g=Guest::new(p.name());g.route(46,256);g.high(46,true);assert!(g.diag().contains("\"relay1CoilDrive\":null"));g.route(47,256);g.high(47,true);assert!(g.diag().contains("\"relay1CoilDrive\":true"));g.high(47,false);assert!(g.diag().contains("\"relay1CoilDrive\":false"));g.put(GPIO+0x554+4*47,256|1<<9);assert!(g.diag().contains("\"relay1CoilDrive\":true"));g.put(GPIO+0x34,1<<(47-32));assert!(g.diag().contains("\"relay1CoilDrive\":null"));g.i2c_route(0,39,38);assert_eq!(g.i2c(0,0x20,0,&[],1),None);}
    let mut g=Guest::new(Profile::Rs485Can.name());g.route(47,256);g.high(47,true);assert!(g.diag().contains("\"relay1CoilDrive\":null"),"RS485-CAN does not inherit Relay1");
}
#[test]
fn eight_channel_expander_and_gpio_inputs_distinguish_relay_from_darlington_outputs(){
    for p in[Profile::PoeDiDo,Profile::EthDiRoC,Profile::PoeDiRoC]{let mut g=Guest::new(p.name());g.i2c_route(0,42,41);assert_eq!(g.i2c(0,0x20,3,&[],1),Some(vec![255]));g.i2c(0,0x20,1,&[0x55,0xaa],0).unwrap();assert!(g.diag().contains("\"channelDriveHigh\":[null,null,null,null,null,null,null,null]"));assert_eq!(g.i2c(0,0x20,1,&[],2),Some(vec![0xaa;2]));g.i2c(0,0x20,3,&[0],0).unwrap();assert!(g.diag().contains("\"channelDriveHigh\":[false,true,false,true,false,true,false,true]"));assert_eq!(g.i2c(0,0x20,0,&[],1),Some(vec![0xaa]));g.i2c(0,0x20,2,&[255],0).unwrap();assert_eq!(g.i2c(0,0x20,0,&[],1),Some(vec![0xaa]),"polarity does not invert output pads");
        assert!(g.diag().contains(if p==Profile::PoeDiDo{"darlington-sink"}else{"relay-coil"}));for i in 0..8 {let pin=4+i;assert_eq!(g.m.bus.board.named_pin(&format!("di{}",i+1)),Some(pin));g.route(pin,256);g.put(GPIO+0x28,1<<pin);g.m.bus.gpio_set_input(pin,true);assert_ne!(g.get(GPIO+0x3c)&(1<<pin),0);g.m.bus.gpio_set_input(pin,false);assert_eq!(g.get(GPIO+0x3c)&(1<<pin),0);}
        g.i2c_route(0,41,42);assert_eq!(g.i2c(0,0x20,1,&[],1),None);
    }
}
#[test]
fn five_sd_profiles_run_actual_guest_one_bit_dma_roundtrip_and_reject_wrong_routes(){
    use esp_periph::sdmmc::{MemStore,reg,des0,int,CMD_START,CMD_RESP,CMD_RESP_LONG,CMD_DATA,CMD_WRITE,CTRL_USE_INTERNAL_DMA,BMOD_DE};
    const SD:u32=0x6002_8000;const RAM:u32=0x3fca_0000;
    fn cmd(g:&mut Guest,n:u8,arg:u32,flags:u32)->u32{g.put(SD+reg::RINTSTS,u32::MAX);g.put(SD+reg::CMDARG,arg);g.put(SD+reg::CMD,CMD_START|1<<16|u32::from(n)|flags);g.get(SD+reg::RESP0)}
    for p in[Profile::Audio,Profile::AudioEn,Profile::PoeDiDo,Profile::EthDiRoC,Profile::PoeDiRoC]{let mut g=Guest::new(p.name());let [clk,c,d,..]=p.sd_pins().unwrap();g.m.bus.attach_sd_card(Box::new(MemStore(vec![0;1<<20])),Default::default()).unwrap();for(pin,sig)in[(clk,173),(c,179),(d,213)]{g.route(pin,sig);g.put(GPIO+0x154+4*sig,0x80|u32::from(pin));}g.put(0x600c_001c,1<<7);g.put(0x600c_0024,0);
        cmd(&mut g,0,0,0);assert_eq!(cmd(&mut g,8,0x1aa,CMD_RESP),0x1aa);for _ in 0..2{cmd(&mut g,55,0,CMD_RESP);cmd(&mut g,41,0x40ff8000,CMD_RESP);}cmd(&mut g,2,0,CMD_RESP|CMD_RESP_LONG);let rca=cmd(&mut g,3,0,CMD_RESP)&0xffff0000;cmd(&mut g,7,rca,CMD_RESP);g.put(SD+reg::CTYPE,0);
        for i in 0..128 {g.put(RAM+512+4*i,0x963a_0000+i);}
        for(n,buffer)in[(24,RAM+512),(17,RAM+1024)]{for(i,v)in[des0::OWN|des0::FS|des0::LD,512,buffer,0].into_iter().enumerate(){g.put(RAM+4*i as u32,v);}for(o,v)in[(reg::CTRL,CTRL_USE_INTERNAL_DMA),(reg::BMOD,BMOD_DE),(reg::DBADDR,RAM),(reg::BLKSIZ,512),(reg::BYTCNT,512)]{g.put(SD+o,v);}cmd(&mut g,n,7,CMD_RESP|CMD_DATA|if n==24{CMD_WRITE}else{0});assert_ne!(g.get(SD+reg::RINTSTS)&int::DATA_OVER,0);assert_eq!(g.get(RAM)&des0::OWN,0);}
        for i in 0..128{assert_eq!(g.get(RAM+1024+4*i),0x963a_0000+i);}g.route(clk,172);cmd(&mut g,8,0x1aa,CMD_RESP);assert_ne!(g.get(SD+reg::RINTSTS)&int::RTO,0);g.route(clk,173);g.m.bus.board.set_sd_card_present(false);cmd(&mut g,8,0x1aa,CMD_RESP);assert_ne!(g.get(SD+reg::RINTSTS)&int::RTO,0);
    }
}
