//! Board-level regressions derived from the retained 1.43 and 2.06 schematics.
use super::*;
use esp_periph::{Gpio, RegRam, sdmmc::card::MemStore};
use esp_soc::pins::ChipPins;

fn gpio(g: &mut Gpio, m: &mut RegRam, pin: u8, high: bool) {
    g.func_out_sel[usize::from(pin)] = 128;
    g.enable |= 1 << pin;
    if high { g.out |= 1 << pin; } else { g.out &= !(1 << pin); }
    m.write(4 + u32::from(pin) * 4, 1 << 12 | 1 << 9);
}
fn write(ds: &mut [(u8,u8,Box<dyn I2cDevice>)], a: u8, bytes: &[u8]) {
    let d = &mut ds.iter_mut().find(|(_,addr,_)|*addr==a).unwrap().2;
    assert!(d.start_address_with_clock(a,a,false,None));
    for &byte in bytes { assert!(d.write(byte)); }
    d.stop();
}
fn read(ds: &mut [(u8,u8,Box<dyn I2cDevice>)], a: u8, register: u8, n: usize) -> Vec<u8> {
    write(ds,a,&[register]);
    let d = &mut ds.iter_mut().find(|(_,addr,_)|*addr==a).unwrap().2;
    assert!(d.start_address_with_clock(a,a,true,None));
    let out = (0..n).map(|_|d.read()).collect();
    d.stop(); out
}
fn start(ds: &mut [(u8,u8,Box<dyn I2cDevice>)], a: u8) -> bool {
    let d = &mut ds.iter_mut().find(|(_,addr,_)|*addr==a).unwrap().2;
    d.start_address_with_clock(a,a,false,None)
}
fn transfer(b: &mut CompactAmoled, bytes: &[u8], quad: bool, keep: bool) {
    let (clock,data,cs) = b.variant.bus();
    b.spi_transfer_qspi_pins(2,SpiQspiPins { sclk:1<<clock,data:data.map(|p|1<<p),
        cs:1<<cs,hardware_cs:1<<cs,data_width:if quad{4}else{1},data_bits:bytes.len()as u32*8,
        keep_cs:keep,..Default::default() },bytes,0);
}
fn command(b: &mut CompactAmoled, c: u8, p: &[u8]) {
    transfer(b,&[2,0,c,0],false,!p.is_empty());
    if !p.is_empty() { transfer(b,p,false,false); }
}
fn red_pixel(b: &mut CompactAmoled, x: u8) {
    for (c,p) in [(0x3a,&[0x55][..]),(0x11,&[][..]),(0x29,&[][..]),(0x53,&[0x20][..]),(0x51,&[0xff][..])] { command(b,c,p); }
    command(b,0x2a,&[0,x,0,x]); command(b,0x2b,&[0,0,0,0]);
    transfer(b,&[0x32,0,0x2c,0],false,true); transfer(b,&[0xf8,0],true,false);
    assert_eq!(b.display().unwrap().2[0],0xf800);
}

#[test]
fn waveshare_amoled206_separates_gpio_resets_from_pmic_audio_supply() {
    let mut b = CompactAmoled::new(Variant::Touch206);
    let mut ds = b.i2c_devices();
    assert_eq!(ds.iter().map(|(_,a,_)|*a).collect::<Vec<_>>(),[0x18,0x34,0x38,0x6b,0x40,0x51]);
    assert!(ds.iter().all(|(_,_,d)|d.pins()==Some((8,7))));
    assert!(b.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),SdCardConfig::default()).is_err());
    let mut g=Gpio::new();let mut m=RegRam::new();
    gpio(&mut g,&mut m,11,true);gpio(&mut g,&mut m,10,true);
    b.gpio_waveform_at(0,ChipPins::C6.routes(&g,&m));
    assert!(b.active,"DCDC1 supplies LCD logic; ALDO2 being off is not LCD reset");
    red_pixel(&mut b,22);
    assert!(start(&mut ds,0x40),"ES7210 control supply permits the PMIC 1.8 V default");
    write(&mut ds,0x34,&[0x92,10]);
    assert!(!start(&mut ds,0x40),"1.5 V falls below the documented control-supply range");
    write(&mut ds,0x34,&[0x92,28]);
    assert!(start(&mut ds,0x40));
    b.advance_to(crate::periph::CPU_HZ/10);b.touch(409,501,true);
    assert_eq!(read(&mut ds,0x38,2,5),[1,1,153,1,245]);
    gpio(&mut g,&mut m,10,false);b.gpio_waveform_at(crate::periph::CPU_HZ/10+1,ChipPins::C6.routes(&g,&m));
    assert!(!start(&mut ds,0x38));assert_eq!(b.display().unwrap().2[0],0xf800);
    gpio(&mut g,&mut m,11,false);b.gpio_waveform_at(crate::periph::CPU_HZ/10+2,ChipPins::C6.routes(&g,&m));
    assert!(b.display().unwrap().2.iter().all(|&p|p==0));
}

#[test]
fn waveshare_amoled143_touch_interrupt_passes_through_expander_not_rtc_reset() {
    let mut b=CompactAmoled::new(Variant::Touch143);let mut ds=b.i2c_devices();
    assert_eq!(ds.iter().map(|(_,a,_)|*a).collect::<Vec<_>>(),[0x18,0x20,0x38,0x6b,0x40,0x51]);
    assert!(ds.iter().all(|(_,_,d)|d.pins()==Some((18,8))));
    assert_eq!(b.named_pin("boot"),Some(9));assert_eq!(b.named_pin("power"),Some(2));
    let mut g=Gpio::new();let mut m=RegRam::new();gpio(&mut g,&mut m,3,true);
    b.gpio_waveform_at(0,ChipPins::C6.routes(&g,&m));red_pixel(&mut b,0);
    // EX1 belongs to RTC INT. R22 is NC, so driving it must not reset touch.
    write(&mut ds,0x20,&[3,0xfd]);write(&mut ds,0x20,&[1,0xfd]);
    b.advance_to(crate::periph::CPU_HZ/10);b.touch(999,999,true);
    assert_eq!(b.touch_irq(),Some((1,false)));
    assert_eq!(read(&mut ds,0x20,0,1)[0]&1,0);
    assert_eq!(b.touch_irq(),Some((1,true)),"reading TCA Input Port acknowledges CPU INT independently of the contact report");
    assert_eq!(read(&mut ds,0x38,2,5),[1,1,209,1,209]);
    let d=&mut ds.iter_mut().find(|(_,a,_)|*a==0x38).unwrap().2;
    assert!(d.start_address_with_clock(0x38,0x38,false,None));assert!(!d.write(0xa3),"do not invent a FT6146 silicon ID");
    gpio(&mut g,&mut m,3,false);b.gpio_waveform_at(crate::periph::CPU_HZ/10+1,ChipPins::C6.routes(&g,&m));
    assert!(b.display().unwrap().2.iter().all(|&p|p==0));assert!(start(&mut ds,0x38));
}

#[test]
fn waveshare_amoled143_shared_sd_bus_routes_real_miso_and_rejects_dual_selection() {
    let mut b=CompactAmoled::new(Variant::Touch143);
    b.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),SdCardConfig::default()).unwrap();
    let mut p=SpiQspiPins{sclk:1<<11,data:[1<<4,0,0,0],data_width:1,read_width:1,data_bits:80,..Default::default()};
    b.spi_transfer_qspi_pins(2,p,&[255;10],0);
    p.cs=1<<15;p.software_cs=p.cs;p.input[1]=Some(5);p.data_bits=48;
    b.spi_transfer_qspi_pins(2,p,&[0x40,0,0,0,0,0x95],0);
    p.data_bits=0;p.read_bits=16;assert_eq!(b.spi_transfer_qspi_pins(2,p,&[],2),[255,1]);
    p.input[1]=Some(18);assert_eq!(b.spi_transfer_qspi_pins(2,p,&[],2),[255,255]);
    p.cs|=1<<10;p.input[1]=Some(5);let before=b.rejected;
    assert_eq!(b.spi_transfer_qspi_pins(2,p,&[],2),[255,255]);assert_eq!(b.rejected,before+1);
}
