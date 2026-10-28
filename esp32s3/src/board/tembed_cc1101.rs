//! LILYGO T-Embed CC1101 V1.0: ST7789, GPIO buttons/quadrature and RMT RGB.
//! Manufacturer c36826e0 utilities.h/Setup214 and schematic V1.0 24-07-29:
//! LCD VDD3V3 is always supplied; G15 gates VCC3V3 for the eight RGB LEDs.
//! LCD reset is the schematic's RC pull-up, TFT_RST=-1 in the shipping driver.
use esp_soc::board::{BoardModel,SpiPins,VirtualCycle};
use esp_soc::devices::{DcsPanel,Ws2812Chain};
use esp_soc::pins::PinRoutes;
use esp_periph::i2c::I2cDevice;
use std::sync::{Arc,Mutex};

pub struct TEmbedCc1101 {panel:DcsPanel,dc:Option<bool>,backlight:bool,rgb_power:bool,rgb:Ws2812Chain,charger:Arc<Mutex<ChargerRegisters>>,display_changes:u64,spi_transfers:u64}
impl TEmbedCc1101 {
    pub fn new()->Self {Self{panel:DcsPanel::st7789(),dc:None,backlight:false,rgb_power:false,rgb:Ws2812Chain::new(8),charger:Arc::new(Mutex::new(ChargerRegisters::new())),display_changes:0,spi_transfers:0}}
    fn drive(&mut self,dc:Option<bool>,backlight:bool,rgb_power:bool) {
        self.dc=dc;
        if self.backlight!=backlight {self.backlight=backlight;self.display_changes+=1;}
        if self.rgb_power && !rgb_power {self.rgb.leds.fill([0;3]);self.rgb.updates+=1;}
        self.rgb_power=rgb_power;
    }
}
impl Default for TEmbedCc1101 {fn default()->Self {Self::new()}}
impl BoardModel for TEmbedCc1101 {
    fn name(&self)->&'static str {"tembed-cc1101"}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,_cycle:VirtualCycle,routes:PinRoutes<'_>) {self.drive(routes.software_output(16),routes.software_output(21)==Some(true),routes.software_output(15)==Some(true));}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if pins.sclk&(1<<11)==0 || pins.mosi&(1<<9)==0 || pins.cs&(1<<41)==0 {return vec![0xff;rx_len];}
        let Some(dc)=self.dc else {return vec![0xff;rx_len];};
        self.panel.dc=dc;self.spi_transfers+=1;
        for &byte in tx {if !dc {self.display_changes+=1;}self.panel.byte(byte);}
        // No panel readback is claimed; the board's MISO also serves SD/radio.
        vec![0xff;rx_len]
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(6,true),(4,true),(5,true)]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"encoder-press"|"boot"|"sw"=>Some(0),"user"=>Some(6),"clk"=>Some(4),"dt"=>Some(5),_=>None}}
    fn encoder(&self)->Option<(u8,u8)> {Some((4,5))}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {(0..2).map(|bus|(bus,0x6b,Box::new(ChargerI2c{state:self.charger.clone(),register:0,address_byte:true}) as Box<dyn I2cDevice>)).collect()}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==14 && self.rgb_power {self.rgb.from_bits(bits);}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        // Fixed landscape glass. TFT_eSPI rotation 3 addresses column35+y,row319-x.
        let mut frame=vec![0;320*170];
        if self.backlight && self.panel.on && !self.panel.sleeping {for y in 0..170 {for x in 0..320 {frame[y*320+x]=self.panel.gram[(319-x)*240+35+y];}}}
        Some((320,170,frame,self.display_version()))
    }
    fn display_version(&self)->u64 {self.panel.pixels_written+self.panel.resets+self.display_changes}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),240,320))}
    fn report(&self)->String {format!("[tembed-cc1101] frames={} pixels={} spi={} madctl={:#x} backlight={} rgb_power={} rgb_updates={}; BQ25896=register-subset; battery/ADC/charger-timing/CC1101/PN532/SD/audio unmodeled",self.panel.frames,self.panel.pixels_written,self.spi_transfers,self.panel.madctl,self.backlight,self.rgb_power,self.rgb.updates)}
}

// TI BQ25896 SLUSC76C (May 2018), §§9.3.1/9.4, register figures 25–45.
// This is its documented identity/configuration register subset, not a charger
// or battery simulation. ADC command completion, analog readings, watchdog and
// charge/OTG transitions are absent; ADC data stays at power-on values.
struct ChargerRegisters {registers:[u8;21]}
impl ChargerRegisters {
    // Table 6 explicitly defaults EN_ILIM to 1; Figure 25 contradicts it.
    // Use the field definition (0x48), without claiming silicon verification.
    fn new()->Self {Self{registers:[0x48,0x06,0x11,0x1a,0x20,0x13,0x5e,0x9d,0x03,0x44,0x73,0x00,0x80,0x12,0,0,0,0,0,0,0x02]}}
    fn write(&mut self,register:u8,value:u8) {
        match register {
            0x14 if value&0x80!=0=>*self=Self::new(),
            0x00..=0x0a|0x0d=>{
                // WD_RST resets the interface watchdog and self-clears. It does
                // not constitute an implemented charger watchdog deadline.
                let value=match register {
                    3=>value&!0x40,
                    // Table 19: only FORCE_VINDPM is writable in relative mode;
                    // an absolute threshold below 3.9 V is clamped to 3.9 V.
                    0x0d if value&0x80==0=>self.registers[0x0d]&0x7f,
                    0x0d=>0x80|(value&0x7f).max(0x0d),
                    _=>value,
                };
                self.registers[usize::from(register)]=value;self.registers[0x0c]&=!0x80;
            },
            _=>{}, // Status/ADC/device identity are read-only.
        }
    }
}
struct ChargerI2c {state:Arc<Mutex<ChargerRegisters>>,register:u8,address_byte:bool}
impl I2cDevice for ChargerI2c {
    fn pins(&self)->Option<(u8,u8)> {Some((8,18))}
    fn start(&mut self,read:bool)->bool {if !read {self.address_byte=true;}true}
    fn write(&mut self,value:u8)->bool {
        if self.address_byte {self.register=value;self.address_byte=false;}else{self.state.lock().expect("T-Embed charger mutex poisoned").write(self.register,value);self.register=self.register.wrapping_add(1);}true
    }
    fn read(&mut self)->u8 {let value=self.state.lock().expect("T-Embed charger mutex poisoned").registers.get(usize::from(self.register)).copied().unwrap_or(0xff);self.register=self.register.wrapping_add(1);value}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pins()->SpiPins {SpiPins{sclk:1<<11,mosi:1<<9,cs:1<<41,..Default::default()}}
    fn cmd(b:&mut TEmbedCc1101,c:u8,data:&[u8]) {b.drive(Some(false),true,false);b.spi_transfer_pins(3,pins(),&[c],0);b.drive(Some(true),true,false);b.spi_transfer_pins(3,pins(),data,0);}
    #[test]
    fn landscape_corners_and_backlight_preserve_real_gram() {
        let mut b=TEmbedCc1101::new();cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x36,&[0xa0]);
        for (x,y,color) in [(0u16,0u16,0xf800u16),(319,0,0x07e0),(0,169,0x001f),(319,169,0xffff)] {
            let x=x.to_be_bytes();let y=(35+y).to_be_bytes();cmd(&mut b,0x2a,&[x[0],x[1],x[0],x[1]]);cmd(&mut b,0x2b,&[y[0],y[1],y[0],y[1]]);cmd(&mut b,0x2c,&color.to_be_bytes());
        }
        let pixels=b.display().unwrap().2;assert_eq!([pixels[0],pixels[319],pixels[169*320],pixels[170*320-1]],[0xf800,0x07e0,0x001f,0xffff]);
        let version=b.display_version();b.drive(Some(true),false,false);assert!(b.display_version()>version);assert!(b.display().unwrap().2.iter().all(|p|*p==0));
        b.drive(Some(true),true,false);assert_eq!(b.display().unwrap().2,pixels,"RGB's G15 supply does not power the LCD");
        cmd(&mut b,0x28,&[]);assert!(b.display().unwrap().2.iter().all(|p|*p==0));cmd(&mut b,0x29,&[]);cmd(&mut b,0x10,&[]);assert!(b.display().unwrap().2.iter().all(|p|*p==0));
    }
    #[test]
    fn missing_dc_and_wrong_spi_routes_do_not_reach_display() {
        let mut b=TEmbedCc1101::new();b.spi_transfer_pins(3,pins(),&[0x29],0);assert_eq!(b.spi_transfers,0);
        b.drive(Some(false),true,false);
        for p in [SpiPins{sclk:1<<12,..pins()},SpiPins{mosi:1<<10,..pins()},SpiPins{cs:1<<12,..pins()}] {assert_eq!(b.spi_transfer_pins(3,p,&[0x29],2),[0xff;2]);}
        assert_eq!(b.spi_transfers,0);assert!(!b.panel.on);
        assert_eq!(b.named_pin("encoder-press"),Some(0));assert_eq!(b.named_pin("user"),Some(6));assert_eq!(b.encoder(),Some((4,5)));
    }
    #[test]
    fn rgb_requires_its_pin_and_supply_and_a_fresh_frame_after_power_cut() {
        let mut b=TEmbedCc1101::new();let bits=vec![true;8*24];
        b.rmt_frame(14,&bits);assert_eq!(b.rgb.updates,0);b.drive(None,false,true);b.rmt_frame(13,&bits);assert_eq!(b.rgb.updates,0);
        b.rmt_frame(14,&bits);assert!(b.rgb.leds.iter().all(|color|*color==[255;3]));
        b.drive(None,false,false);assert!(b.rgb.leds.iter().all(|color|*color==[0;3]));b.drive(None,false,true);assert!(b.rgb.leds.iter().all(|color|*color==[0;3]));
        b.rmt_frame(14,&bits);assert!(b.rgb.leds.iter().all(|color|*color==[255;3]));
    }
    #[test]
    fn charger_register_identity_readonly_reset_and_physical_route() {
        let mut b=TEmbedCc1101::new();let mut devices=b.i2c_devices();assert_eq!(devices.len(),2);assert!(devices.iter().all(|(_,a,d)|*a==0x6b && d.pins()==Some((8,18))));
        let dev=devices[0].2.as_mut();dev.start(false);dev.write(0x14);dev.start(true);assert_eq!(dev.read(),2);assert_eq!(dev.read(),0xff);
        dev.start(false);dev.write(4);dev.write(0x0a);dev.start(false);dev.write(4);dev.start(true);assert_eq!(dev.read(),0x0a);
        dev.start(false);dev.write(0x14);dev.write(0x3f);dev.start(false);dev.write(0x14);dev.start(true);assert_eq!(dev.read(),2);
        dev.start(false);dev.write(0x14);dev.write(0x80);dev.start(false);dev.write(4);dev.start(true);assert_eq!(dev.read(),0x20);
        dev.start(false);dev.write(0x0e);dev.write(0xff);dev.start(false);dev.write(0x0e);dev.start(true);assert_eq!(dev.read(),0,"read-only ADC power-on value is not a completed measurement");
    }
}
