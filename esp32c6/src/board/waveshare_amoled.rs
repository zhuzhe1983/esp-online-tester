//! Exact C6 CO5300 assemblies; sources/identity conflicts are retained per SKU.
//! 2.16: main.cpp calls DisplayPort(i2c,480,480), therefore constructor defaults
//! CS15/IRQ5 apply. The unused user_config.h CS5/IRQ15 are not another route.
//! 1.64: overview says FT6146; linked data sheet and BSP say FT3168. Only that
//! explicit compatible report subset is exposed, without a fabricated die ID.
use esp_periph::{i2c::I2cDevice,sdmmc::{SdCardConfig,SdStore,card::spi::SpiSdCard}};
use esp_soc::{BoardModel,board::{BoardEdge,SpiQspiPins},pins::PinRoutes,devices::{
    qspi_dcs::QspiDcsPanel,ft3168::{Ft3168,Ft3168State},cst92xx::{Cst92xx,Cst92State,Cst92Kind},
    axp2101::{Axp2101,Axp2101Power,PowerRail},qmi8658::Qmi8658,imu_motion::ImuMotion,
    pcf85063::{Pcf85063,Pcf85063State},es8311::Es8311,es7210::Es7210}};
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex,atomic::Ordering}};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum AmoledVariant{Touch216,Touch164}
impl AmoledVariant{
    fn name(self)->&'static str{match self{Self::Touch216=>"waveshare-c6-touch-amoled216",Self::Touch164=>"waveshare-c6-touch-amoled164"}}
    fn size(self)->(u16,u16){match self{Self::Touch216=>(480,480),Self::Touch164=>(280,456)}}
    fn pins(self)->(u8,[u8;4],u8,u8,(u8,u8),u8){match self{Self::Touch216=>(0,[1,2,3,4],15,6,(8,7),5),Self::Touch164=>(11,[4,5,7,19],10,15,(18,8),1)}}
}

type PersistentDevice=Rc<RefCell<Box<dyn I2cDevice>>>;
struct RoutedDevice{device:PersistentDevice,pins:(u8,u8),power:Option<(Arc<Mutex<Axp2101Power>>,PowerRail)>}
impl RoutedDevice{fn active(&self)->bool{self.power.as_ref().is_none_or(|(s,r)|s.lock().expect("AXP2101 state").output_mv(*r)==Some(3300))}}
impl I2cDevice for RoutedDevice{
    fn pins(&self)->Option<(u8,u8)>{Some(self.pins)}
    fn start_address_with_clock(&mut self,c:u8,a:u8,r:bool,h:Option<u32>)->bool{self.active()&&self.device.borrow_mut().start_address_with_clock(c,a,r,h)}
    fn write(&mut self,b:u8)->bool{self.active()&&self.device.borrow_mut().write(b)}
    fn read(&mut self)->u8{if self.active(){self.device.borrow_mut().read()}else{255}}
    fn stop(&mut self){self.device.borrow_mut().stop();}
}
pub struct WaveshareAmoled{
    variant:AmoledVariant,panel:QspiDcsPanel,devices:Vec<(u8,PersistentDevice)>,power:Option<Arc<Mutex<Axp2101Power>>>,
    ft:Option<Arc<Mutex<Ft3168State>>>,cst:Option<Arc<Mutex<Cst92State>>>,motion:Arc<ImuMotion>,rtc:Option<Arc<Mutex<Pcf85063State>>>,
    reset_pin:bool,touch_reset:bool,active:bool,software_cs:bool,cycle:u64,irq_high:bool,rejected:u64,edges:Vec<BoardEdge>,sd:Option<SpiSdCard>,
}
impl WaveshareAmoled{
    pub fn new(variant:AmoledVariant)->Self{
        let(w,h)=variant.size();let power=(variant==AmoledVariant::Touch216).then(||Arc::new(Mutex::new(Axp2101Power::default())));
        let ft=(variant==AmoledVariant::Touch164).then(||{let mut s=Ft3168State::new(crate::periph::CPU_HZ);s.hardware_at(0,true,true);Arc::new(Mutex::new(s))});
        let cst=(variant==AmoledVariant::Touch216).then(||Arc::new(Mutex::new(Cst92State::new(Cst92Kind::Cst9220,w,h))));
        let motion=Arc::new(ImuMotion::default());let rtc=(variant==AmoledVariant::Touch216).then(||Arc::new(Mutex::new(Pcf85063State::new(crate::periph::CPU_HZ))));
        let mut parts:Vec<(u8,Box<dyn I2cDevice>)>=vec![(0x6b,Box::new(Qmi8658::new_c(motion.clone(),crate::periph::CPU_HZ)))];
        if let Some(s)=&power{parts.push((0x34,Box::new(Axp2101::new(s.clone()))));parts.push((0x18,Box::new(Es8311::new())));parts.push((0x40,Box::new(Es7210::new())));}
        if let Some(s)=&ft{parts.push((0x38,Box::new(Ft3168::new(s.clone()))));}
        if let Some(s)=&cst{parts.push((0x5a,Box::new(Cst92xx::new(s.clone()))));}
        if let Some(s)=&rtc{parts.push((0x51,Box::new(Pcf85063::new(s.clone()))));}
        Self{variant,panel:QspiDcsPanel::co5300(w as usize,h as usize,if variant==AmoledVariant::Touch164{20}else{0},if variant==AmoledVariant::Touch216{0x30}else{0}),
            devices:parts.into_iter().map(|(a,d)|(a,Rc::new(RefCell::new(d)))).collect(),power,ft,cst,motion,rtc,reset_pin:false,touch_reset:false,active:false,software_cs:false,cycle:0,irq_high:true,rejected:0,edges:vec![],sd:None}
    }
    fn sync_power(&mut self){
        let powered=self.power.as_ref().is_none_or(|p|p.lock().expect("AXP2101 state").output_mv(PowerRail::Dcdc1)==Some(3300));
        // J4 glass supply/DSI_PWR_EN use VCC3V3 (DCDC1). R16 connects
        // ALDO3 to LCD_RESET. ALDO2 is the amplifier control, not glass power.
        let reset=self.power.as_ref().map_or(self.reset_pin,|p|p.lock().expect("AXP2101 state").output_mv(PowerRail::Aldo3)==Some(3300));
        let active=powered&&reset;if self.active!=active{if !active{self.panel.reset();}self.active=active;self.panel.changes+=1;}
        if let Some(s)=&self.ft{s.lock().expect("FT-compatible state").hardware_at(self.cycle,powered,true);}
        if let Some(s)=&self.cst{s.lock().expect("CST9220 state").hardware(powered,self.touch_reset);}
        self.sync_irq();
    }
    fn touch_high(&self)->bool{if let Some(s)=&self.ft{s.lock().expect("FT-compatible state").irq_high()}else{self.cst.as_ref().is_none_or(|s|s.lock().expect("CST9220 state").irq_high())}}
    fn sync_irq(&mut self){let high=self.touch_high();if high!=self.irq_high{self.irq_high=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:self.variant.pins().5,level:high});}}
}
impl BoardModel for WaveshareAmoled{
    fn name(&self)->&'static str{self.variant.name()}
    fn named_pin(&self,name:&str)->Option<u8>{matches!(name,"boot"|"button").then_some(9)}
    fn input_levels(&self)->Vec<(u8,bool)>{vec![(9,true),(self.variant.pins().5,self.touch_high())]}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{let pins=self.variant.pins().4;self.devices.iter().map(|(a,d)|{
        let power=self.power.as_ref().filter(|_|matches!(*a,0x6b|0x18|0x40)).map(|s|(s.clone(),if *a==0x40{PowerRail::Aldo1}else{PowerRail::Dcdc1}));
        (0,*a,Box::new(RoutedDevice{device:d.clone(),pins,power})as Box<dyn I2cDevice>)}).collect()}
    fn advance_to(&mut self,cycle:u64){self.cycle=cycle;self.motion.cycle.store(cycle,Ordering::Relaxed);if let Some(s)=&self.rtc{s.lock().expect("PCF85063 state").advance_to(cycle);}self.sync_power();}
    fn next_deadline(&self)->Option<u64>{self.ft.as_ref().and_then(|s|s.lock().expect("FT-compatible state").next_deadline())}
    fn take_edges(&mut self)->Vec<BoardEdge>{self.sync_irq();std::mem::take(&mut self.edges)}
    fn touch_irq(&self)->Option<(u8,bool)>{Some((self.variant.pins().5,self.touch_high()))}
    fn touch(&mut self,x:u16,y:u16,down:bool){let(w,h)=self.variant.size();let(x,y)=(x.min(w-1),y.min(h-1));
        if let Some(s)=&self.ft{s.lock().expect("FT-compatible state").touch_at(self.cycle,x,y,down);}
        // CST raw coordinates are exposed as raw input; the example applies
        // an additional software rotation. Physical touch/glass registration
        // is not established, so no calibrated screen-coordinate claim is made.
        if let Some(s)=&self.cst{s.lock().expect("CST9220 state").update(x,y,down);}self.sync_irq();}
    fn touch_at(&mut self,cycle:u64,x:u16,y:u16,down:bool){self.advance_to(cycle);self.touch(x,y,down);}
    fn set_imu_motion(&mut self,mode:u32)->bool{self.motion.mode.store(mode,Ordering::Relaxed);true}
    fn imu_motion(&self)->u32{self.motion.mode.load(Ordering::Relaxed)}
    fn uses_gpio_routes(&self)->bool{true}
    fn gpio_waveform_at(&mut self,cycle:u64,r:PinRoutes<'_>){self.advance_to(cycle);if self.variant==AmoledVariant::Touch164{self.reset_pin=r.software_output(20)==Some(true);}else{self.touch_reset=r.software_output(11)==Some(true);}self.sync_power();
        let(_,_,cs,sd_cs,_,_)=self.variant.pins();let selected=r.software_output(cs)==Some(false);if self.software_cs&&!selected{self.panel.deselect();}self.software_cs=selected;
        let gpio=r.software_output(sd_cs);let hardware=[68,101,102,103,104,105].into_iter().any(|s|r.output_pins(s).any(|p|p==sd_cs));if gpio==Some(true)||(gpio.is_none()&&!hardware){if let Some(sd)=&mut self.sd{sd.deselect();}}}
    fn uses_qspi_pins(&self)->bool{true}
    fn spi_transfer_qspi_pins(&mut self,host:u8,p:SpiQspiPins,tx:&[u8],rx_len:usize)->Vec<u8>{self.sync_power();let(clk,data,cs,sd_cs,_,_)=self.variant.pins();let floating=vec![255;rx_len];let lcd=p.cs&(1<<cs)!=0;let card=p.cs&(1<<sd_cs)!=0;
        if lcd&&card{self.rejected+=1;self.panel.reject_until_deselect();if let Some(sd)=&mut self.sd{sd.deselect();}return floating;}
        let routed=host==2&&p.sclk&(1<<clk)!=0&&p.data[0]&(1<<data[0])!=0;
        if card||!lcd{
            if !routed||p.unsupported_mode||p.sio||p.clock_idle_high||p.clock_output_edge||p.dummy_cycles!=0
                ||[p.command_width,p.address_width,p.data_width,p.read_width].iter().any(|&w|w>1)
                ||!p.command_bits.is_multiple_of(8)||!p.address_bits.is_multiple_of(8)||!p.data_bits.is_multiple_of(8)||!p.read_bits.is_multiple_of(8){self.rejected+=1;return floating;}
            if let Some(sd)=&mut self.sd{let rx=if p.full_duplex||tx.is_empty()||rx_len==0{sd.transfer(card,tx,rx_len)}else{sd.transfer(card,tx,0);sd.transfer(card,&[],rx_len)};
                if card{if p.hardware_cs&(1<<sd_cs)!=0&&!p.keep_cs&&p.software_cs&(1<<sd_cs)==0{sd.deselect();}return if p.input[1]==Some(data[1]){rx}else{floating};}}
            return floating;
        }
        let quad=[p.command_width,p.address_width,p.data_width].contains(&4);let routed=routed&&(!quad||data[1..].iter().enumerate().all(|(i,pin)|p.data[i+1]&(1<<pin)!=0));
        if !routed||!self.active||rx_len!=0{self.panel.reject_until_deselect();self.rejected+=1;}else{self.panel.transfer(p,tx);}
        if !p.keep_cs&&p.software_cs&(1<<cs)==0{self.panel.deselect();}floating
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String>{let sd=SpiSdCard::new(image,config)?;let description=sd.describe();self.sd=Some(sd);Ok(description)}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{let(w,h)=self.variant.size();Some((w as u32,h as u32,if self.active{self.panel.frame()}else{vec![0;w as usize*h as usize]},self.panel.version()))}
    fn display_version(&self)->u64{self.panel.version()}
    fn display_frames(&self)->u64{self.panel.frames()}
    fn report(&self)->String{format!("[{}] CO5300 {}x{} RAMWR={} pixels={} active={} MADCTL={:02x} commands={} rejected={}/{}; touch={} IRQ{}; BOOT9; SD {}; digital presentation only; touch rotation/FT6146-vs-FT3168 identity/analog power/audio/TE/private gamma/RF unverified",self.name(),self.panel.width(),self.panel.height(),self.panel.frames(),self.panel.pixels_written(),self.active,self.panel.madctl,self.panel.commands,self.rejected,self.panel.rejected,if self.ft.is_some(){"official FT3168-compatible BSP report"}else{"CST9220 raw-report"},self.variant.pins().5,self.sd.as_ref().map_or_else(||"absent".into(),SpiSdCard::report))}
}

#[cfg(test)]mod tests{
    use super::*;use esp_periph::{Gpio,RegRam};use esp_soc::pins::ChipPins;
    fn route(b:&mut WaveshareAmoled,reset:bool){let pin=if b.variant==AmoledVariant::Touch164{20}else{11};let mut g=Gpio::new();let mut m=RegRam::new();g.func_out_sel[pin]=128;g.enable=1<<pin;if reset{g.out=1<<pin;}m.write(4+pin as u32*4,1<<12|1<<9);b.gpio_waveform_at(b.cycle,ChipPins::C6.routes(&g,&m));}
    fn pins(b:&WaveshareAmoled,n:usize,width:u8,keep:bool)->SpiQspiPins{let(clk,data,cs,_,_,_)=b.variant.pins();SpiQspiPins{sclk:1<<clk,data:data.map(|p|1<<p),cs:1<<cs,hardware_cs:1<<cs,data_width:width,data_bits:n as u32*8,keep_cs:keep,..Default::default()}}
    fn command(b:&mut WaveshareAmoled,c:u8,data:&[u8]){let p=pins(b,4,1,!data.is_empty());b.spi_transfer_qspi_pins(2,p,&[2,0,c,0],0);if !data.is_empty(){let p=pins(b,data.len(),1,false);b.spi_transfer_qspi_pins(2,p,data,0);}}
    fn init(b:&mut WaveshareAmoled){route(b,true);command(b,0x36,&[if b.variant==AmoledVariant::Touch216{0x30}else{0}]);command(b,0x3a,&[0x55]);command(b,0x11,&[]);command(b,0x29,&[]);command(b,0x53,&[0x20]);command(b,0x51,&[255]);let gap=if b.variant==AmoledVariant::Touch164{20}else{0};command(b,0x2a,&[0,gap,0,gap]);command(b,0x2b,&[0,0,0,0]);}
    fn pixel(b:&mut WaveshareAmoled){let p=pins(b,4,1,true);b.spi_transfer_qspi_pins(2,p,&[0x32,0,0x2c,0],0);let p=pins(b,2,4,false);b.spi_transfer_qspi_pins(2,p,&[0xf8,0],0);}
    fn write(d:&mut dyn I2cDevice,a:u8,bytes:&[u8]){assert!(d.start_address_with_clock(a,a,false,None));for v in bytes{assert!(d.write(*v));}d.stop();}
    #[test]fn waveshare_amoled_exact_buses_and_power_reset_gate_real_quad_pixels(){for variant in [AmoledVariant::Touch216,AmoledVariant::Touch164]{let mut b=WaveshareAmoled::new(variant);let mut ds=b.i2c_devices();assert_eq!(b.named_pin("boot"),Some(9));assert_eq!(b.named_pin("user"),None);assert!(ds.iter().all(|(_,_,d)|d.pins()==Some(variant.pins().4)));
        let addresses=ds.iter().map(|(_,a,_)|*a).collect::<Vec<_>>();assert_eq!(addresses,if variant==AmoledVariant::Touch216{vec![0x6b,0x34,0x18,0x40,0x5a,0x51]}else{vec![0x6b,0x38]});
        init(&mut b);pixel(&mut b);assert_eq!(b.display().unwrap().2[0],0xf800);assert_eq!(b.panel.pixels_written(),1);
        let p=pins(&b,4,1,true);b.spi_transfer_qspi_pins(2,p,&[0x32,0,0x2c,0],0);let mut p=pins(&b,2,4,false);p.data[2]=0;b.spi_transfer_qspi_pins(2,p,&[0,31],0);assert_eq!(b.panel.pixels_written(),1,"missing physical QSPI lane must not reach GRAM");
        if variant==AmoledVariant::Touch216{let d=&mut ds.iter_mut().find(|(_,a,_)|*a==0x34).unwrap().2;write(&mut **d,0x34,&[0x90,0x41]);b.advance_to(1);assert!(b.display().unwrap().2.iter().all(|v|*v==0),"ALDO3 drives LCD reset");write(&mut **d,0x34,&[0x90,0x45]);b.advance_to(2);assert!(!b.panel.visible(),"released reset still requires guest initialization");}
        else{route(&mut b,false);assert!(b.display().unwrap().2.iter().all(|v|*v==0));}
    }}
    #[test]fn waveshare_amoled_touch_reports_keep_distinct_identity_reset_and_raw_bounds(){for variant in [AmoledVariant::Touch216,AmoledVariant::Touch164]{let mut b=WaveshareAmoled::new(variant);route(&mut b,true);b.touch_at(crate::periph::CPU_HZ/10,999,999,true);let address=if variant==AmoledVariant::Touch216{0x5a}else{0x38};let mut ds=b.i2c_devices();let d=&mut ds.iter_mut().find(|(_,a,_)|*a==address).unwrap().2;
        assert_eq!(b.touch_irq(),Some((variant.pins().5,false)));
        if variant==AmoledVariant::Touch216{write(&mut **d,address,&[0xd0,0]);assert!(d.start_address_with_clock(address,address,true,None));let v=(0..15).map(|_|d.read()).collect::<Vec<_>>();assert_eq!(v[5],1);assert_eq!(((v[1]as u16)<<4)|(v[3]as u16>>4),479);assert_eq!(((v[2]as u16)<<4)|(v[3]as u16&15),479);d.stop();write(&mut **d,address,&[0xd0,0,0xab]);assert_eq!(b.touch_irq(),Some((5,true)));route(&mut b,false);assert!(!d.start_address_with_clock(address,address,false,None));}
        else{write(&mut **d,address,&[2]);assert!(d.start_address_with_clock(address,address,true,None));let v=(0..7).map(|_|d.read()).collect::<Vec<_>>();assert_eq!(v,[1,1,23,1,199,0,0]);d.stop();assert!(d.start_address_with_clock(address,address,false,None));assert!(!d.write(0xa3),"no FT6146/FT3168 die identity is invented");}
    }}
    #[test]fn waveshare_amoled_sd_uses_shared_clock_and_exact_receive_pad(){use esp_periph::sdmmc::card::MemStore;for variant in[AmoledVariant::Touch216,AmoledVariant::Touch164]{let mut b=WaveshareAmoled::new(variant);b.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),SdCardConfig::default()).unwrap();let(clk,data,cs,sd_cs,_,_)=variant.pins();let mut p=SpiQspiPins{sclk:1<<clk,data:[1<<data[0],0,0,0],data_width:1,read_width:1,data_bits:80,..Default::default()};b.spi_transfer_qspi_pins(2,p,&[255;10],0);p.cs=1<<sd_cs;p.software_cs=p.cs;p.input[1]=Some(data[1]);p.data_bits=48;b.spi_transfer_qspi_pins(2,p,&[0x40,0,0,0,0,0x95],0);p.data_bits=0;p.read_bits=16;assert_eq!(b.spi_transfer_qspi_pins(2,p,&[],2),[255,1]);p.cs|=1<<cs;p.data_bits=48;p.read_bits=0;b.spi_transfer_qspi_pins(2,p,&[0x40,0,0,0,0,0x95],0);p.cs=1<<sd_cs;p.data_bits=0;p.read_bits=16;assert_eq!(b.spi_transfer_qspi_pins(2,p,&[],2),[255,255]);}}
}
