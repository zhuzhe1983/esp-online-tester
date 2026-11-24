//! Exact Waveshare C6 basic boards. Zero's current overview is FH8/8MiB,
//! while the retained older schematic is FH4/4MiB: distinct source profiles,
//! not invented sale revisions. Only Zero-B has GPIO7 RF-switch control.
use esp_soc::{BoardModel,devices::Ws2812Chain,pins::PinRoutes};
use esp_soc::devices::tca9554::{Tca9554,Tca9554State};
use esp_periph::i2c::I2cDevice;
use std::sync::{Arc,Mutex};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum DevVariant{ZeroB,Zero,ZeroFh4,Pico,DevN8,DevN16}
impl DevVariant{
    pub fn from_name(name:&str)->Option<Self>{match name{"waveshare-c6-zero-b"=>Some(Self::ZeroB),"waveshare-c6-zero"=>Some(Self::Zero),"waveshare-c6-zero-fh4"=>Some(Self::ZeroFh4),"waveshare-c6-pico"=>Some(Self::Pico),"waveshare-c6-dev-kit-n8"=>Some(Self::DevN8),"waveshare-c6-dev-kit-n16"=>Some(Self::DevN16),_=>None}}
    pub fn name(self)->&'static str{match self{Self::ZeroB=>"waveshare-c6-zero-b",Self::Zero=>"waveshare-c6-zero",Self::ZeroFh4=>"waveshare-c6-zero-fh4",Self::Pico=>"waveshare-c6-pico",Self::DevN8=>"waveshare-c6-dev-kit-n8",Self::DevN16=>"waveshare-c6-dev-kit-n16"}}
    pub fn flash_mb(self)->usize{match self{Self::ZeroFh4|Self::Pico=>4,Self::DevN16=>16,_=>8}}
    pub fn integrated_flash_mb(self)->Option<usize>{(!matches!(self,Self::DevN8|Self::DevN16)).then_some(self.flash_mb())}
    fn module(self)->&'static str{match self{Self::Pico=>"ESP32-C6-MINI-1 FH4",Self::DevN8|Self::DevN16=>"ESP32-C6-WROOM-1 external flash",Self::ZeroFh4=>"ESP32-C6FH4",_=>"ESP32-C6FH8"}}
}
struct PicoExpander(Tca9554);
impl I2cDevice for PicoExpander{
    fn pins(&self)->Option<(u8,u8)>{Some((22,23))}
    fn start(&mut self,read:bool)->bool{self.0.start(read)}
    fn write(&mut self,value:u8)->bool{self.0.write(value)}
    fn read(&mut self)->u8{self.0.read()}
    fn stop(&mut self){self.0.stop();}
}
pub struct WaveshareDev{variant:DevVariant,rgb:Ws2812Chain,antenna_control:Option<bool>,expander:Option<Arc<Mutex<Tca9554State>>>}
impl WaveshareDev{
    pub fn new(variant:DevVariant)->Self{Self{variant,rgb:Ws2812Chain::new(1),antenna_control:None,expander:(variant==DevVariant::Pico).then(||Arc::new(Mutex::new(Tca9554State::default())))}}
    pub fn zero_b()->Self{Self::new(DevVariant::ZeroB)}
}
impl BoardModel for WaveshareDev{
    fn name(&self)->&'static str{self.variant.name()}
    fn named_pin(&self,name:&str)->Option<u8>{matches!(name,"boot"|"button").then_some(9)}
    fn input_levels(&self)->Vec<(u8,bool)>{if self.variant==DevVariant::Pico{vec![(9,true)]}else{vec![(8,true),(9,true)]}}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{self.expander.as_ref().map_or_else(Vec::new,|s|vec![(0,0x20,Box::new(PicoExpander(Tca9554::new(s.clone())))as Box<dyn I2cDevice>)])}
    fn uses_gpio_routes(&self)->bool{true}
    fn gpio_waveform_at(&mut self,_:u64,r:PinRoutes<'_>){if self.variant==DevVariant::ZeroB{self.antenna_control=r.software_output(7);}}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]){if pin==8{self.rgb.from_bits(bits);}}
    fn leds(&self)->Option<(&[[u8;3]],u64)>{Some((&self.rgb.leds,self.rgb.updates))}
    fn report(&self)->String{format!("[{}] {} {}MiB BOOT9 WS2812 GPIO8 rgb={:?} updates={} RF-control GPIO7={:?} TCA9554@20 SDA22/SCL23 levels={:?}; RF/USB/analog power unverified; no screen/touch/SD",self.name(),self.variant.module(),self.variant.flash_mb(),self.rgb.leds,self.rgb.updates,self.antenna_control,self.expander.as_ref().map(|s|s.lock().expect("TCA9554 state").levels()))}
}

#[cfg(test)]mod tests{
    use super::*;use esp_periph::{Gpio,RegRam};use esp_soc::pins::ChipPins;
    #[test]fn waveshare_zero_b_exact_led_boot_and_rf_control_without_fake_peripherals(){
        let mut b=WaveshareDev::zero_b();let bits=[0x12u8,0x34,0x56].into_iter().flat_map(|byte|(0..8).rev().map(move|i|byte&(1<<i)!=0)).collect::<Vec<_>>();
        b.rmt_frame(7,&bits);assert_eq!(b.leds().unwrap().1,0);b.rmt_frame(8,&bits);assert_eq!(b.leds().unwrap(),(&[[0x34,0x12,0x56]][..],1));
        assert_eq!(b.named_pin("boot"),Some(9));assert!(b.display().is_none());assert!(b.i2c_devices().is_empty());assert_eq!(b.touch_irq(),None);
        let mut g=Gpio::new();let mut m=RegRam::new();g.func_out_sel[7]=128;g.enable=1<<7;g.out=1<<7;m.write(4+7*4,1<<12|1<<9);b.gpio_waveform_at(0,ChipPins::C6.routes(&g,&m));assert_eq!(b.antenna_control,Some(true));m.write(4+7*4,0);b.gpio_waveform_at(0,ChipPins::C6.routes(&g,&m));assert_eq!(b.antenna_control,None);
        assert_eq!(WaveshareDev::zero_b().leds().unwrap().0,[[0;3]]);
    }
    #[test]fn waveshare_zero_source_capacities_select_true_fh_identity_and_have_no_rf_switch(){
        use riscv_rv32::bus::Bus;
        use esp_periph::sdmmc::{card::MemStore,SdCardConfig};
        for(name,mb,cap)in[("waveshare-c6-zero",8,2),("waveshare-c6-zero-fh4",4,1)]{
            let mut bus=crate::bus::SocBus::new(mb<<20,[0;6]);bus.board=crate::board::make_board(name).unwrap();bus.attach_board_devices();
            assert_eq!(bus.flash.len(),mb<<20);assert_eq!((bus.periph.efuse.read(0x50)>>24)&7,1);assert_eq!(bus.periph.efuse.read(0x54)&7,cap);
            assert_eq!(bus.board.named_pin("boot"),Some(9));assert_ne!(bus.read32(0x6009_103c).unwrap()&(1<<9),0);
            esp_soc::SocBus::gpio_set_input(&mut bus,9,false);assert_eq!(bus.read32(0x6009_103c).unwrap()&(1<<9),0);
            assert!(bus.board.display().is_none());assert!(bus.board.i2c_devices().is_empty());assert_eq!(bus.board.touch_irq(),None);
            assert!(bus.board.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),SdCardConfig::default()).is_err());
            let mut b=WaveshareDev::new(DevVariant::from_name(name).unwrap());let mut g=Gpio::new();let mut m=RegRam::new();g.func_out_sel[7]=128;g.enable=1<<7;g.out=1<<7;m.write(4+7*4,1<<12|1<<9);b.gpio_waveform_at(0,ChipPins::C6.routes(&g,&m));assert_eq!(b.antenna_control,None);
            let bits=[0x12u8,0x34,0x56].into_iter().flat_map(|byte|(0..8).rev().map(move|i|byte&(1<<i)!=0)).collect::<Vec<_>>();b.rmt_frame(10,&bits);assert_eq!(b.leds().unwrap().1,0);b.rmt_frame(8,&bits);assert_eq!(b.leds().unwrap(),(&[[0x34,0x12,0x56]][..],1));
        }
        assert!(DevVariant::from_name("waveshare-h2-zero").is_none());
    }
    #[test]fn waveshare_zero_sram_guest_routes_uart_boot_and_rmt_to_the_real_pad(){
        use riscv_rv32::bus::Bus;
        // A transparent source-map regression fixture, not vendor firmware.
        // It executes RV32 stores through the actual C6 bus and GPIO matrix.
        for(name,mb,pin,expected)in[("waveshare-c6-zero",8,8,1),("waveshare-c6-zero-fh4",4,8,1),("waveshare-c6-pico",4,8,1),("waveshare-c6-dev-kit-n8",8,8,1),("waveshare-c6-dev-kit-n16",16,8,1),("waveshare-c6-zero",8,10,0)]{
            let mut b=crate::bus::SocBus::new(mb<<20,[0;6]);b.board=crate::board::make_board(name).unwrap();b.attach_board_devices();
            let mut words=Vec::<u32>::new();
            fn li(w:&mut Vec<u32>,r:u32,v:u32){let hi=v.wrapping_add(0x800)&0xffff_f000;w.push(hi|(r<<7)|0x37);w.push(((v&0xfff)<<20)|(r<<15)|(r<<7)|0x13);}
            fn store(w:&mut Vec<u32>,a:u32,v:u32){li(w,5,a);li(w,6,v);w.push((6<<20)|(5<<15)|(2<<12)|0x23);}
            store(&mut words,0x6009_0004+pin*4,1<<12);
            store(&mut words,0x6009_1554+pin*4,71);
            for(i,bit)in[0x12u8,0x34,0x56].into_iter().flat_map(|byte|(0..8).rev().map(move|i|byte&(1<<i)!=0)).enumerate(){
                store(&mut words,0x6000_6400+i as u32*4,if bit{0x8000|64|(32<<16)}else{0x8000|32|(64<<16)});
            }
            store(&mut words,0x6000_6460,0);store(&mut words,0x6000_6010,(1<<16)|(8<<8)|1);store(&mut words,0x6000_0000,u32::from(b'Z'));
            li(&mut words,5,0x6009_103c);words.push((5<<15)|(2<<12)|(10<<7)|3);words.push(0x0010_0073);
            let mut cpu=riscv_rv32::Cpu::new_rv32imac();cpu.pc=crate::bus::SRAM_LOW;
            for(i,w)in words.iter().enumerate(){b.write32(cpu.pc+i as u32*4,*w).unwrap();}
            for _ in 0..words.len()-1{riscv_rv32::step(&mut cpu,&mut b).unwrap();b.tick(1);}
            assert_ne!(cpu.x[10]&(1<<9),0);assert!(matches!(riscv_rv32::step(&mut cpu,&mut b),Err(riscv_rv32::Trap::Ebreak(_))));
            b.tick(100_000);assert_eq!(b.periph.uart[0].tx_out,b"Z");assert_eq!(b.board.leds().unwrap().1,expected);
            if expected==1{assert_eq!(b.board.leds().unwrap().0,[[0x34,0x12,0x56]]);}else{assert_eq!(b.board.leds().unwrap().0,[[0,0,0]]);}
        }
    }
    #[test]fn waveshare_pico_expander_uses_exact_pins_address_and_external_port_registers(){
        let mut b=WaveshareDev::new(DevVariant::Pico);let mut devices=b.i2c_devices();assert_eq!(devices.len(),1);let(bus,address,d)=devices.first_mut().unwrap();assert_eq!((*bus,*address,d.pins()),(0,0x20,Some((22,23))));
        assert!(d.start(false));assert!(d.write(3));assert!(d.write(0xfe));d.stop();
        assert!(d.start(false));assert!(d.write(1));assert!(d.write(0));d.stop();
        assert_eq!(b.expander.as_ref().unwrap().lock().unwrap().driven(0),Some(false));assert_eq!(b.expander.as_ref().unwrap().lock().unwrap().driven(1),None);
        assert!(d.start(false));assert!(d.write(0));assert!(d.start(true));assert_eq!(d.read(),0xfe);d.stop();
        assert!(d.start(false));assert!(!d.write(4));assert_eq!(b.touch_irq(),None);assert!(b.take_edges().is_empty(),"INT is not wired to an MCU GPIO");
        assert!(b.named_pin("exio0").is_none(),"expander port must not masquerade as MCU GPIO0");
        assert_eq!(DevVariant::Pico.integrated_flash_mb(),Some(4));
    }
    #[test]fn waveshare_external_devkit_capacity_does_not_select_fh8_identity(){
        for(name,mb)in[("waveshare-c6-dev-kit-n8",8),("waveshare-c6-dev-kit-n16",16)]{
            let mut bus=crate::bus::SocBus::new(mb<<20,[0;6]);bus.board=crate::board::make_board(name).unwrap();bus.attach_board_devices();
            assert_eq!(bus.flash.len(),mb<<20);assert_eq!((bus.periph.efuse.read(0x50)>>24)&7,0);assert_eq!(bus.periph.efuse.read(0x54)&0xe7,0);
            assert!(bus.board.i2c_devices().is_empty());assert!(bus.board.display().is_none());assert!(bus.board.touch_irq().is_none());assert_eq!(bus.board.named_pin("boot"),Some(9));
        }
    }
}
