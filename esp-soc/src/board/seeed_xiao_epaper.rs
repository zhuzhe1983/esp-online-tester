//! Seeed 7.5-inch UC8179 boards with different MCU wiring, not board aliases.
//! Official ogdiy_kit_works_with_{arduino,esphome} and
//! xiao_075inch_epaper_panel_{arduino,esphome}: docs/platform/seeed-75-runtime.md.
use super::{BoardModel,BoardEdge,SpiPins,VirtualCycle};
use crate::{devices::uc8179::Uc8179,pins::PinRoutes};

#[derive(Clone,Copy,PartialEq)]
pub enum Variant {TrmnlS3,XiaoC3}
pub struct SeeedXiaoEpaper {variant:Variant,panel:Uc8179,powered:bool,active:bool,dc:Option<bool>,events:u64,cycle:u64,battery:esp_periph::analog::AnalogStream,battery_enabled:bool}
impl SeeedXiaoEpaper {
    pub fn new(variant:Variant,cycles_per_ms:u64)->Self {Self{variant,panel:Uc8179::new(4,cycles_per_ms),powered:variant==Variant::XiaoC3,active:variant==Variant::XiaoC3,dc:None,events:0,cycle:0,battery:esp_periph::analog::AnalogStream::new(1,0.0,0).expect("constant battery source"),battery_enabled:false}}
    fn pins(&self)->[u8;5] {match self.variant {Variant::TrmnlS3=>[7,9,44,10,38],Variant::XiaoC3=>[8,10,3,5,2]}}
}
impl BoardModel for SeeedXiaoEpaper {
    fn name(&self)->&'static str {match self.variant {Variant::TrmnlS3=>"seeed-trmnl-75-diy",Variant::XiaoC3=>"seeed-xiao-epaper-75"}}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_output_at(&mut self,_cycle:VirtualCycle,changes:&[(u8,bool)],_enabled:u64,_output:u64) {self.events+=changes.len() as u64;}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        self.cycle=cycle;self.panel.advance_to(cycle);let pins=self.pins();
        // The kit's PWR_EN is U0TXD43 (sheet5), with a100k pulldown (sheet4).
        // IDFv4.4.7 esp32s3 gpio_sig_map.h/io_mux_reg.h: UART0 TX signal12,
        // native GPIO43 function0. Model its idle high; UART-bit supply ripple
        // and inverted/break signalling are outside this steady-state supply model.
        let supply=self.variant==Variant::XiaoC3||routes.software_output(43)==Some(true)||routes.function(43,0)||routes.matrix_output(43,12);
        let active=routes.software_output(pins[4])!=Some(false)&&supply;
        if self.powered&&!supply {self.panel.power_loss();}else if self.active&&!active {self.panel.reset();}self.powered=supply;self.active=active;self.dc=routes.software_output(pins[3]);
        if self.variant==Variant::TrmnlS3 {let on=routes.software_output(6)==Some(true);if on!=self.battery_enabled {self.battery.replace_current(if on{2.0}else{0.0}).expect("constant divider voltage");self.battery_enabled=on;}}
    }
    fn gpio_events(&self)->u64 {self.events}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        let [sck,mosi,cs,_,_]=self.pins();
        if self.active&&pins.sclk&(1u64<<sck)!=0&&pins.mosi&(1u64<<mosi)!=0&&pins.cs&(1u64<<cs)!=0 {if let Some(dc)=self.dc {self.panel.write(dc,tx);}}
        // The panel's SPI connection is write-only; unsupported reads float high.
        vec![0xff;rx_len]
    }
    fn input_levels(&self)->Vec<(u8,bool)> {let mut pins=vec![(4,self.panel.busy_high())];if self.variant==Variant::TrmnlS3 {pins.extend([(0,true),(2,true),(3,true),(5,true)]);}else{pins.push((9,true));}pins}
    fn named_pin(&self,name:&str)->Option<u8> {match (self.variant,name) {(Variant::TrmnlS3,"key1"|"button-d1")=>Some(2),(Variant::TrmnlS3,"key2"|"button-d2")=>Some(3),(Variant::TrmnlS3,"key3"|"button-d4")=>Some(5),(Variant::TrmnlS3,"boot")=>Some(0),(Variant::XiaoC3,"boot")=>Some(9),_=>None}}
    fn default_analog_inputs(&self)->Vec<(u8,esp_periph::AnalogSource)> {
        // Seeed XIAO_ePaper_driver_board_sch.pdf rev1.0, sheets4/5: U17 gated by
        // ADC_EN6 supplies R28=R29=10k, midpoint BAT_ADC1. Nominal4.0V battery;
        // host voltage/raw overrides stay authoritative. No charger/discharge model.
        if self.variant==Variant::TrmnlS3 {vec![(1,esp_periph::AnalogSource::Stream(self.battery.clone()))]}else{vec![]}
    }
    fn advance_to(&mut self,cycle:VirtualCycle) {self.cycle=cycle;self.panel.advance_to(cycle);}
    fn next_deadline(&self)->Option<VirtualCycle> {self.panel.next_deadline()}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.panel.take_edges()}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {Some((800,480,self.panel.frame.clone(),self.panel.version))}
    fn display_version(&self)->u64 {self.panel.version}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn display_quiet_push(&self)->bool {true}
    fn report(&self)->String {format!("[{}] frames={} RAM_bytes={} panel_enabled={} partial; optical-waveforms/battery/sleep-wake unverified",self.name(),self.panel.frames,self.panel.ram_bytes_written,self.active)}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn route(b:&mut SeeedXiaoEpaper,dc:bool,enable:bool,reset:bool) {
        let mut gpio=esp_periph::Gpio::new();let mut mux=esp_periph::RegRam::new();let [_,_,_,d,r]=b.pins();
        gpio.enable=(1u64<<d)|(1u64<<r);gpio.out=if dc {1u64<<d}else{0}|if reset{1u64<<r}else{0};
        if b.variant==Variant::XiaoC3 {gpio.func_out_sel[d as usize]=128;gpio.func_out_sel[r as usize]=128;}
        if b.variant==Variant::TrmnlS3 {gpio.enable|=1u64<<43;if enable {gpio.out|=1u64<<43;}mux.write(4+4*43,1<<12);}
        mux.write(4+4*u32::from(d),1<<12);mux.write(4+4*u32::from(r),1<<12);
        let chip=if b.variant==Variant::XiaoC3 {crate::pins::ChipPins::C3}else{crate::pins::ChipPins::S3};b.gpio_waveform_at(b.cycle,chip.routes(&gpio,&mux));
    }
    fn send(b:&mut SeeedXiaoEpaper,c:u8,bytes:&[u8]) {let [sck,mosi,cs,_,_]=b.pins();let p=SpiPins{sclk:1u64<<sck,mosi:1u64<<mosi,cs:1u64<<cs,..Default::default()};route(b,false,true,true);b.spi_transfer_pins(2,p,&[c],0);route(b,true,true,true);b.spi_transfer_pins(2,p,bytes,0);}
    #[test]
    fn two_physical_routes_publish_only_after_busy_and_keep_ink_after_reset() {
        for variant in [Variant::TrmnlS3,Variant::XiaoC3] {
            let mut b=SeeedXiaoEpaper::new(variant,1);send(&mut b,0x61,&[3,32,1,224]);send(&mut b,0x04,&[]);b.advance_to(1);send(&mut b,0x13,&[0x7f]);assert_eq!(b.panel.frame[0],0xffff);
            send(&mut b,0x12,&[]);assert!(b.input_levels().contains(&(4,false)));b.advance_to(20);assert_eq!(b.panel.frame[0],0xffff);b.advance_to(21);assert_eq!(&b.panel.frame[..2],&[0,0xffff]);assert_eq!(b.display_frames(),1);
            route(&mut b,true,true,false);assert_eq!(b.panel.frame[0],0,"bistable image survives reset");let before=b.panel.ram_bytes_written;let [sck,mosi,cs,_,_]=b.pins();b.spi_transfer_pins(2,SpiPins{sclk:1<<sck,mosi:1<<mosi,cs:1<<cs,..Default::default()},&[0],0);assert_eq!(b.panel.ram_bytes_written,before);
        }
    }
    #[test]
    fn kit_requires_power_enable_and_unknown_routes_or_inputs_are_not_accepted() {
        let mut b=SeeedXiaoEpaper::new(Variant::TrmnlS3,1);route(&mut b,false,false,true);let pins=SpiPins{sclk:1<<7,mosi:1<<9,cs:1<<44,..Default::default()};b.spi_transfer_pins(2,pins,&[0x13],0);route(&mut b,true,false,true);b.spi_transfer_pins(2,pins,&[0],0);assert_eq!(b.panel.ram_bytes_written,0);
        send(&mut b,0x13,&[]);for wrong in [SpiPins{sclk:1<<8,..pins},SpiPins{mosi:1<<10,..pins},SpiPins{cs:1<<3,..pins}] {b.spi_transfer_pins(2,wrong,&[0],0);}assert_eq!(b.panel.ram_bytes_written,0);
        b.spi_transfer_pins(2,pins,&[0],0);assert_eq!(b.panel.ram_bytes_written,1);assert_eq!(b.named_pin("key3"),Some(5));assert_eq!(b.named_pin("touch"),None);
        let c3=SeeedXiaoEpaper::new(Variant::XiaoC3,1);assert_eq!(c3.named_pin("boot"),Some(9));assert_eq!(c3.named_pin("key3"),None);
    }
    #[test]
    fn battery_divider_obeys_real_gpio_route_and_preserves_explicit_host_voltage() {
        let mut b=SeeedXiaoEpaper::new(Variant::TrmnlS3,1);let mut pads=esp_periph::AnalogInputs::new(1000);for (pin,source) in b.default_analog_inputs(){pads.set_default(pin,source);}
        let mut gpio=esp_periph::Gpio::new();let mut mux=esp_periph::RegRam::new();gpio.enable=1<<6;gpio.out=1<<6;assert_eq!(pads.volts(1,0),0.0);
        b.gpio_waveform_at(0,crate::pins::ChipPins::S3.routes(&gpio,&mux));assert_eq!(pads.volts(1,0),0.0,"unmuxed ADC_EN is not a driven high");
        mux.write(4+4*6,1<<12);b.gpio_waveform_at(1,crate::pins::ChipPins::S3.routes(&gpio,&mux));assert_eq!(pads.volts(1,1),2.0);
        gpio.out=0;b.gpio_waveform_at(2,crate::pins::ChipPins::S3.routes(&gpio,&mux));assert_eq!(pads.volts(1,2),0.0);
        pads.set(1,esp_periph::AnalogSource::Const(1.6));gpio.out=1<<6;b.gpio_waveform_at(3,crate::pins::ChipPins::S3.routes(&gpio,&mux));for (pin,source) in b.default_analog_inputs(){pads.set_default(pin,source);}assert_eq!(pads.volts(1,3),1.6);
        assert!(SeeedXiaoEpaper::new(Variant::XiaoC3,1).default_analog_inputs().is_empty());
    }
    #[test]
    fn native_uart_idle_supplies_the_kits_uart_connected_power_switch() {
        let mut b=SeeedXiaoEpaper::new(Variant::TrmnlS3,1);let mut gpio=esp_periph::Gpio::new();let mut mux=esp_periph::RegRam::new();
        b.gpio_waveform_at(0,crate::pins::ChipPins::S3.routes(&gpio,&mux));assert!(b.powered,"GPIO43 native UART0 TX idles high");
        mux.write(4+4*43,1<<12);b.gpio_waveform_at(1,crate::pins::ChipPins::S3.routes(&gpio,&mux));assert!(!b.powered,"R26 pulldown wins when GPIO is undriven");
        gpio.func_out_sel[43]=12;b.gpio_waveform_at(2,crate::pins::ChipPins::S3.routes(&gpio,&mux));assert!(b.powered,"routed UART0 TX has the same idle level");
        gpio.func_out_sel[43]=256;gpio.enable=1<<43;b.gpio_waveform_at(3,crate::pins::ChipPins::S3.routes(&gpio,&mux));assert!(!b.powered,"explicit GPIO low cuts panel supply");
    }
}
