//! Exact C3-LCD-0.71: original Demo ZIP TFT_eSPI User_Setup/GC9D01 sources
//! and 2024-11-13 schematic. GC9D01 DCS RGB565 subset, not ST7789 identity.
//! LEDA is tied to 3V3 and LEDK to ground through R1: no software backlight.
use esp_soc::{BoardModel,board::SpiQspiPins,pins::PinRoutes,devices::DcsPanel};

pub struct Lcd071{panel:DcsPanel,dc:Option<bool>,reset_high:bool,command:u8,generation:u64,rejected:u64}
impl Default for Lcd071{fn default()->Self{Self::new()}}
impl Lcd071{pub fn new()->Self{Self{panel:DcsPanel::new(160,160),dc:None,reset_high:true,command:0,generation:1,rejected:0}}}
impl BoardModel for Lcd071{
    fn name(&self)->&'static str{"waveshare-c3-lcd071"}
    fn named_pin(&self,name:&str)->Option<u8>{matches!(name,"boot"|"button").then_some(9)}
    fn input_levels(&self)->Vec<(u8,bool)>{vec![(9,true)]}
    fn uses_gpio_routes(&self)->bool{true}
    fn gpio_waveform_at(&mut self,_:u64,r:PinRoutes<'_>){
        self.dc=r.software_output(4);let high=r.software_output(8).unwrap_or(true);
        if self.reset_high!=high{if !high{self.panel.reset();self.command=0;}self.reset_high=high;self.generation=self.generation.wrapping_add(1);}
    }
    fn uses_qspi_pins(&self)->bool{true}
    fn spi_transfer_qspi_pins(&mut self,host:u8,p:SpiQspiPins,tx:&[u8],rx_len:usize)->Vec<u8>{
        let floating=vec![255;rx_len];
        if host!=2||p.unsupported_mode||p.sio||p.clock_idle_high||p.clock_output_edge||p.dummy_cycles!=0
            ||p.sclk&(1<<6)==0||p.data[0]&(1<<7)==0||p.cs&(1<<5)==0
            ||[p.command_width,p.address_width,p.data_width,p.read_width].iter().any(|&w|w>1)
            ||!p.command_bits.is_multiple_of(8)||!p.address_bits.is_multiple_of(8)||!p.data_bits.is_multiple_of(8)||!p.read_bits.is_multiple_of(8)
            ||(rx_len!=0&&!p.full_duplex){self.rejected+=1;return floating;}
        if !self.reset_high||self.dc.is_none(){return floating;}
        self.panel.dc=self.dc==Some(true);
        if self.panel.dc&&self.command==0x2c&&self.panel.colmod!=5{self.rejected+=1;return floating;}
        for &b in tx{if !self.panel.dc{self.command=b;}self.panel.byte(b);}
        if !tx.is_empty(){self.generation=self.generation.wrapping_add(1);}floating
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{
        // Manufacturer GC9D01_Defines.h deliberately defines MAD_BGR=0 and
        // MAD_RGB=8. The normal portrait source uses MADCTL=0, no crop.
        let p=&self.panel;let on=self.reset_high&&p.on&&!p.sleeping;
        let pixels=p.gram.iter().map(|&v|if !on{0}else if p.madctl&8!=0{(v&0x7e0)|((v&0xf800)>>11)|((v&31)<<11)}else{v}).collect();
        Some((160,160,pixels,self.generation))
    }
    fn display_version(&self)->u64{self.generation}
    fn display_frames(&self)->u64{self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)>{Some((self.panel.gram.clone(),160,160))}
    fn report(&self)->String{format!("[waveshare-c3-lcd071] GC9D01 DCS565 160x160 RAMWR={} pixels={} reset={} rejected={}; BOOT9; fixed backlight; no touch/TF/IMU/RGB; analog battery/charge/private gamma/RF unverified",self.panel.frames,self.panel.pixels_written,self.reset_high,self.rejected)}
}

#[cfg(test)]mod tests{
    use super::*;use esp_periph::{Gpio,RegRam};use esp_soc::pins::ChipPins;
    fn signals(b:&mut Lcd071,dc:bool,reset:bool){let mut g=Gpio::new();let mut m=RegRam::new();for(pin,high)in[(4,dc),(8,reset)]{g.func_out_sel[pin]=128;g.enable|=1<<pin;if high{g.out|=1<<pin;}m.write(4+pin as u32*4,1<<12|1<<9);}b.gpio_waveform_at(0,ChipPins::C3.routes(&g,&m));}
    fn pins(n:usize)->SpiQspiPins{SpiQspiPins{sclk:1<<6,data:[1<<7,0,0,0],cs:1<<5,data_width:1,data_bits:n as u32*8,..Default::default()}}
    fn send(b:&mut Lcd071,c:u8,data:&[u8]){signals(b,false,true);b.spi_transfer_qspi_pins(2,pins(1),&[c],0);signals(b,true,true);b.spi_transfer_qspi_pins(2,pins(data.len()),data,0);}
    #[test]fn waveshare_c3_gc9d01_routes_reset_format_and_visible_coordinates(){
        let mut b=Lcd071::new();send(&mut b,0x3a,&[5]);send(&mut b,0x11,&[]);send(&mut b,0x29,&[]);send(&mut b,0x2a,&[0,1,0,1]);send(&mut b,0x2b,&[0,2,0,2]);send(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[321],0xf800);
        let before=b.panel.pixels_written;for p in[SpiQspiPins{sclk:1<<7,..pins(2)},SpiQspiPins{cs:1<<4,..pins(2)},SpiQspiPins{data_width:4,..pins(2)}]{b.spi_transfer_qspi_pins(2,p,&[0,31],0);}assert_eq!(b.panel.pixels_written,before);
        send(&mut b,0x36,&[0x60]);send(&mut b,0x2c,&[0x07,0xe0]);assert_eq!(b.display().unwrap().2[160+157],0x07e0,"source MV/MX bits transpose and mirror the guest address");
        send(&mut b,0x3a,&[0x66]);send(&mut b,0x2c,&[255,255]);assert_eq!(b.panel.pixels_written,before+1);
        signals(&mut b,true,false);assert!(b.display().unwrap().2.iter().all(|v|*v==0));assert!(b.panel.sleeping);assert_eq!(b.panel.gram[321],0xf800,"reset keeps GRAM but removes visible output");
    }
    #[test]fn waveshare_c3_gc9d01_has_no_invented_touch_card_or_i2c_devices(){
        use esp_periph::sdmmc::{SdCardConfig,card::MemStore};
        let mut b=Lcd071::new();assert_eq!(b.named_pin("boot"),Some(9));assert!(b.i2c_devices().is_empty());b.touch(80,80,true);assert_eq!(b.touch_irq(),None);assert!(b.leds().is_none());assert!(b.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),SdCardConfig::default()).is_err());
    }
}
