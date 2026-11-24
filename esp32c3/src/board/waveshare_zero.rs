//! Waveshare C3-Zero: exact C3FH4/4MiB, RGB GPIO10 and BOOT GPIO9.
//! Source schematic and BlinkRGB example are retained with fixed byte hashes.
use esp_soc::{BoardModel,devices::Ws2812Chain};

pub struct Zero{rgb:Ws2812Chain}
impl Zero{pub fn new()->Self{Self{rgb:Ws2812Chain::new(1)}}}
impl Default for Zero{fn default()->Self{Self::new()}}
impl BoardModel for Zero{
    fn name(&self)->&'static str{"waveshare-c3-zero"}
    fn named_pin(&self,name:&str)->Option<u8>{matches!(name,"boot"|"button").then_some(9)}
    fn input_levels(&self)->Vec<(u8,bool)>{vec![(9,true)]}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]){if pin==10{self.rgb.from_bits(bits);}}
    fn leds(&self)->Option<(&[[u8;3]],u64)>{Some((&self.rgb.leds,self.rgb.updates))}
    fn report(&self)->String{format!("[waveshare-c3-zero] ESP32-C3FH4 integrated4MiB BOOT9 RGB10 rgb={:?} updates={}; sourceUART0 TX21/RX20; USB/analog/RF unverified; no display/touch/TF/I2C device",self.rgb.leds,self.rgb.updates)}
}
#[cfg(test)]mod tests{
    use super::*;use riscv_rv32::bus::Bus;
    #[test]fn waveshare_c3_zero_real_sram_bus_routes_rgb10_and_rejects_c6_rgb8(){
        for(pin,expected)in[(10,1),(8,0)]{
            let mut b=crate::bus::SocBus::new(4<<20,[0;6]);b.board=crate::board::make_board("waveshare-c3-zero").unwrap();b.attach_board_devices();
            assert_eq!(b.board.named_pin("boot"),Some(9));assert!(b.board.display().is_none());assert!(b.board.i2c_devices().is_empty());assert!(b.board.touch_irq().is_none());
            let mut words=vec![];
            fn li(w:&mut Vec<u32>,r:u32,v:u32){let hi=v.wrapping_add(0x800)&0xffff_f000;w.push(hi|(r<<7)|0x37);w.push(((v&0xfff)<<20)|(r<<15)|(r<<7)|0x13);}
            fn store(w:&mut Vec<u32>,a:u32,v:u32){li(w,5,a);li(w,6,v);w.push((6<<20)|(5<<15)|(2<<12)|0x23);}
            store(&mut words,0x6000_9004+pin*4,1<<12);store(&mut words,0x6000_4554+pin*4,51);
            for(i,bit)in[0x12u8,0x34,0x56].into_iter().flat_map(|v|(0..8).rev().map(move|i|v&(1<<i)!=0)).enumerate(){store(&mut words,0x6001_6400+i as u32*4,if bit{0x8000|64|(32<<16)}else{0x8000|32|(64<<16)});}
            store(&mut words,0x6001_6460,0);store(&mut words,0x6001_6010,(1<<16)|(8<<8)|1);store(&mut words,0x6000_0000,u32::from(b'C'));
            li(&mut words,5,0x6000_403c);words.push((5<<15)|(2<<12)|(10<<7)|3);
            let mut cpu=riscv_rv32::Cpu::new();cpu.pc=crate::bus::IRAM_LOW+0x4000;
            for(i,w)in words.iter().enumerate(){b.write32(cpu.pc+i as u32*4,*w).unwrap();}
            for _ in 0..words.len(){riscv_rv32::step(&mut cpu,&mut b).unwrap();b.tick(1);}
            b.tick(100_000);assert_eq!(b.periph.uart[0].tx_out,b"C");assert_ne!(cpu.x[10]&(1<<9),0);assert_eq!(b.board.leds().unwrap().1,expected);
            if expected==1{assert_eq!(b.board.leds().unwrap().0,[[0x34,0x12,0x56]]);}
            esp_soc::SocBus::gpio_set_input(&mut b,9,false);assert_eq!(b.read32(0x6000_403c).unwrap()&(1<<9),0);
        }
    }
    #[test]fn waveshare_c3_zero_has_no_card_or_fabricated_reset_gpio(){
        use esp_periph::sdmmc::{card::MemStore,SdCardConfig};let mut b=Zero::new();assert_eq!(b.named_pin("reset"),None);assert!(b.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),SdCardConfig::default()).is_err());assert!(crate::board::make_board("waveshare-c6-zero").is_none());
    }
}
