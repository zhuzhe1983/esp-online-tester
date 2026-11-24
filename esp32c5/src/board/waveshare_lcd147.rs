//! ESP32-C5-LCD-1.47 @ acf81d83f8cc580fb8bc4f3f13e7332ab6b1332d.
//! BSP1.0.0 and schematic: ST7789 172x320, 4MiB flash/no PSRAM, BOOT28,
//! shared SPI SD socket and an explicitly RGB-ordered onboard LED on GPIO8.
use esp_soc::{BoardModel,board::SpiQspiPins,devices::DcsPanel,pins::PinRoutes};
use esp_periph::sdmmc::{SdCardConfig,SdStore,card::spi::SpiSdCard};

pub struct Lcd147 {
    panel:DcsPanel, dc:Option<bool>,reset_high:bool,backlight:u32,gpio_backlight:Option<bool>,
    generation:u64,cmd:u8,rejected:u64,leds:[[u8;3];1],led_generation:u64,sd:Option<SpiSdCard>,
}
impl Default for Lcd147{fn default()->Self{Self::new()}}
impl Lcd147{
    pub fn new()->Self{Self{panel:DcsPanel::st7789(),dc:None,reset_high:false,backlight:0,gpio_backlight:None,generation:1,cmd:0,rejected:0,leds:[[0;3];1],led_generation:1,sd:None}}
    fn changed(&mut self){self.generation=self.generation.wrapping_add(1);}
    fn brightness(&mut self,value:u32){if self.backlight!=value{self.backlight=value;self.changed();}}
    fn swap_rb(p:u16)->u16{(p&0x07e0)|((p&0xf800)>>11)|((p&31)<<11)}
    fn shade(p:u16,d:u32)->u16{let r=u32::from((p>>11)&31)*d/65535;let g=u32::from((p>>5)&63)*d/65535;let b=u32::from(p&31)*d/65535;((r<<11)|(g<<5)|b) as u16}
}
impl BoardModel for Lcd147{
    fn name(&self)->&'static str{"waveshare-c5-lcd147"}
    fn input_levels(&self)->Vec<(u8,bool)>{vec![(28,true)]}
    fn named_pin(&self,name:&str)->Option<u8>{matches!(name,"boot"|"button").then_some(28)}
    fn uses_gpio_routes(&self)->bool{true}
    fn gpio_waveform_at(&mut self,_cycle:u64,r:PinRoutes<'_>){
        self.dc=r.software_output(24);self.gpio_backlight=r.software_output(10);
        if let Some(on)=self.gpio_backlight{self.brightness(if on{65535}else{0});}
        let high=r.software_output(26)==Some(true);
        if self.reset_high!=high{self.reset_high=high;if !high{self.panel.reset();self.cmd=0;}self.changed();}
        let sd_gpio=r.software_output(4);
        let sd_hardware=[61,101,102,103,104,105].into_iter().any(|s|r.output_pins(s).any(|p|p==4));
        if sd_gpio==Some(true)||(sd_gpio.is_none()&&!sd_hardware){if let Some(sd)=&mut self.sd{sd.deselect();}}
    }
    fn advance_inputs_with_clock(&mut self,_cycle:u64,clock:&dyn Fn(u8)->Option<(f64,u32)>){
        let value=self.gpio_backlight.map_or_else(||clock(10).map_or(0,|(_,d)|d),|on|if on{65535}else{0});self.brightness(value);
    }
    fn uses_qspi_pins(&self)->bool{true}
    fn spi_transfer_qspi_pins(&mut self,host:u8,p:SpiQspiPins,tx:&[u8],rx_len:usize)->Vec<u8>{
        let floating=vec![0xff;rx_len];
        if host!=2||p.unsupported_mode||p.sio||p.clock_idle_high||p.clock_output_edge
            ||[p.command_width,p.address_width,p.data_width,p.read_width].iter().any(|&w|w>1)
            ||p.sclk&(1<<7)==0||p.data[0]&(1<<6)==0||!p.command_bits.is_multiple_of(8)||!p.address_bits.is_multiple_of(8)
            ||!p.data_bits.is_multiple_of(8)||!p.read_bits.is_multiple_of(8)||p.dummy_cycles!=0{
            self.rejected+=1;return floating;
        }
        let lcd=p.cs&(1<<23)!=0;let card=p.cs&(1<<4)!=0;
        if lcd&&card{self.rejected+=1;if let Some(sd)=&mut self.sd{sd.deselect();}return floating;}
        if let Some(sd)=&mut self.sd{
            // Unselected MOSI clocks perform the actual SD power-up clock requirement.
            let rx=if p.full_duplex||tx.is_empty()||rx_len==0{sd.transfer(card,tx,rx_len)}
                else{sd.transfer(card,tx,0);sd.transfer(card,&[],rx_len)};
            if card{
                if p.hardware_cs&(1<<4)!=0&&!p.keep_cs&&p.software_cs&(1<<4)==0{sd.deselect();}
                return if p.input[1]==Some(5){rx}else{floating};
            }
        }
        if lcd&&self.reset_high&&self.dc.is_some(){
            if rx_len!=0||p.full_duplex{self.rejected+=1;return floating;}
            self.panel.dc=self.dc==Some(true);
            if self.panel.dc&&self.cmd==0x2c&&self.panel.colmod!=0x55{self.rejected+=1;return floating;}
            for &b in tx{if !self.panel.dc{self.cmd=b;}self.panel.byte(b);}
            if !tx.is_empty(){self.changed();}
        }
        floating
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String>{
        let sd=SpiSdCard::new(image,config)?;let result=sd.describe();self.sd=Some(sd);Ok(result)
    }
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]){
        if pin!=8||bits.len()!=24{return;}let mut rgb=[0u8;3];
        for(n,&b)in bits.iter().enumerate(){rgb[n/8]=(rgb[n/8]<<1)|u8::from(b);}
        if self.leds[0]!=rgb{self.leds[0]=rgb;self.led_generation=self.led_generation.wrapping_add(1);}
    }
    fn leds(&self)->Option<(&[[u8;3]],u64)>{Some((&self.leds,self.led_generation))}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{
        let p=&self.panel;let visible=self.reset_high&&self.backlight!=0&&p.on&&!p.sleeping;
        let mut pixels=Vec::with_capacity(172*320);
        for y in 0..320{for x in 34..206{
            let mut v=if visible{p.gram[y*p.cols+x]}else{0};
            if visible{if p.madctl&8!=0{v=Self::swap_rb(v);}
                if !p.inverted{v= !v;}v=Self::shade(v,self.backlight);}
            pixels.push(v);
        }}Some((172,320,pixels,self.generation))
    }
    fn display_version(&self)->u64{self.generation}
    fn display_frames(&self)->u64{self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)>{Some((self.panel.gram.clone(),240,320))}
    fn report(&self)->String{format!("[waveshare-c5-lcd147] ST7789 172x320 pixels={} RAMWR={} duty={} rejected={}; BOOT28 RGB8={:?}; SD {}; source-wired SRAM diagnostic subset; factory/USB/RF unverified",self.panel.pixels_written,self.panel.frames,self.backlight,self.rejected,self.leds[0],self.sd.as_ref().map_or_else(||"absent".into(),SpiSdCard::report))}
}
