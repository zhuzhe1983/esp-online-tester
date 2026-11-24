//! Board-level regressions pinned to the individually retained 2026-11-23 BSPs.
use super::make_board;
use esp_periph::{Gpio,RegRam,i2c::I2cDevice};
use esp_soc::{BoardModel,board::SpiQspiPins,pins::ChipPins};

fn gpio(g:&mut Gpio,m:&mut RegRam,pin:u8,high:bool){g.func_out_sel[usize::from(pin)]=128;g.enable|=1<<pin;if high{g.out|=1<<pin;}else{g.out&=!(1<<pin);}m.write(4+u32::from(pin)*4,1<<12|1<<9);}
fn write(d:&mut dyn I2cDevice,address:u8,bytes:&[u8]){assert!(d.start_address_with_clock(address,address,false,Some(100000)));for &v in bytes{assert!(d.write(v));}d.stop();}
fn command(b:&mut dyn BoardModel,g:&mut Gpio,m:&mut RegRam,pins:(u8,u8,u8,u8),cmd:u8,args:&[u8]){
    let(clk,mosi,cs,dc)=pins;gpio(g,m,dc,false);b.gpio_waveform_at(0,ChipPins::C6.routes(g,m));
    let mut p=SpiQspiPins{sclk:1<<clk,data:[1<<mosi,0,0,0],cs:1<<cs,data_width:1,data_bits:8,..Default::default()};
    b.spi_transfer_qspi_pins(2,p,&[cmd],0);
    if !args.is_empty(){gpio(g,m,dc,true);b.gpio_waveform_at(0,ChipPins::C6.routes(g,m));p.data_bits=args.len()as u32*8;b.spi_transfer_qspi_pins(2,p,args,0);}
}
#[test]
fn round_and_square_variants_use_exact_buses_reset_and_contact_net(){
    for(name,pins,reset,bl,touch_reset,irq,addresses)in[
        ("waveshare-c6-lcd128",(11,21,10,19),20,7,None,None,vec![0x6b,0x51]),
        ("waveshare-c6-touch-lcd128",(11,21,10,19),20,7,Some(23),Some(4),vec![0x15,0x6b,0x51]),
        ("waveshare-c6-lcd154",(1,2,11,3),4,6,None,None,vec![0x6b,0x18,0x40]),
        ("waveshare-c6-touch-lcd154",(1,2,11,3),4,6,Some(4),Some(5),vec![0x15,0x6b,0x18,0x40]),
    ]{
        let mut b=make_board(name).unwrap();let mut g=Gpio::new();let mut m=RegRam::new();
        let mut devices=b.i2c_devices();assert_eq!(devices.iter().map(|(_,a,_)|*a).collect::<Vec<_>>(),addresses);
        let i2c=if name.ends_with("128"){(18,8)}else{(8,7)};assert!(devices.iter().all(|(_,_,d)|d.pins()==Some(i2c)));
        assert_eq!(b.named_pin("boot"),Some(9));assert_eq!(b.named_pin("user"),name.ends_with("154").then_some(18));
        assert!(b.display().unwrap().2.iter().all(|&x|x==0));
        gpio(&mut g,&mut m,reset,true);gpio(&mut g,&mut m,bl,true);if let Some(p)=touch_reset{gpio(&mut g,&mut m,p,true);}
        b.gpio_waveform_at(0,ChipPins::C6.routes(&g,&m));
        command(&mut *b,&mut g,&mut m,pins,0x11,&[]);command(&mut *b,&mut g,&mut m,pins,0x29,&[]);
        command(&mut *b,&mut g,&mut m,pins,0x3a,&[if name.ends_with("128"){5}else{0x55}]);
        command(&mut *b,&mut g,&mut m,pins,0x2a,&[0,0,0,0]);command(&mut *b,&mut g,&mut m,pins,0x2b,&[0,0,0,0]);command(&mut *b,&mut g,&mut m,pins,0x2c,&[0xf8,0]);
        assert_eq!((b.display().unwrap().0,b.display().unwrap().1),(240,240));assert_eq!(b.display().unwrap().2[0],0xf800);
        let wrong=SpiQspiPins{sclk:1<<pins.1,data:[1<<pins.0,0,0,0],cs:1<<pins.2,data_width:1,data_bits:16,..Default::default()};
        b.spi_transfer_qspi_pins(2,wrong,&[0,0x1f],0);assert_eq!(b.display().unwrap().2[0],0xf800,"swapped SPI wires must not reach the panel");
        b.touch(999,999,true);assert_eq!(b.touch_irq(),irq.map(|p|(p,false)));
        if let Some((_,_,d))=devices.iter_mut().find(|(_,a,_)|*a==0x15){
            write(&mut **d,0x15,&[2]);assert!(d.start_address_with_clock(0x15,0x15,true,Some(100000)));assert_eq!((0..5).map(|_|d.read()).collect::<Vec<_>>(),[1,0,239,0,239]);d.stop();
            assert_eq!(b.touch_irq(),irq.map(|p|(p,true)));b.touch(20,30,true);
            gpio(&mut g,&mut m,touch_reset.unwrap(),false);b.gpio_waveform_at(0,ChipPins::C6.routes(&g,&m));assert_eq!(b.touch_irq(),irq.map(|p|(p,true)));assert!(!d.start_address_with_clock(0x15,0x15,false,Some(100000)));
        }
        gpio(&mut g,&mut m,reset,false);b.gpio_waveform_at(0,ChipPins::C6.routes(&g,&m));assert!(b.display().unwrap().2.iter().all(|&x|x==0));
    }
}
#[test]
fn ch32_protocol_width_is_board_specific_and_expander_controls_pixels(){
    for(name,pins,size,wide,addresses)in[
        ("waveshare-c6-touch28",(0,1,11,10),(240,320),false,vec![0x24,0x58,0x6b,0x51,0x18]),
        ("waveshare-c6-lcd273",(0,1,8,19),(320,320),true,vec![0x24,0x6b,0x51]),
    ]{
        let mut b=make_board(name).unwrap();let mut devices=b.i2c_devices();assert_eq!(devices.iter().map(|(_,a,_)|*a).collect::<Vec<_>>(),addresses);
        assert!(devices.iter().all(|(_,_,d)|d.pins()==Some(if wide{(20,2)}else{(6,7)})));assert_eq!(b.named_pin("boot"),Some(9));
        let exp=&mut *devices[0].2;
        if wide{write(exp,0x24,&[2,0xf7,0xff]);write(exp,0x24,&[3,0xff,0xff]);}else{write(exp,0x24,&[2,0x0b]);write(exp,0x24,&[3,0x03]);assert!(exp.start_address_with_clock(0x24,0x24,false,Some(100000)));assert!(exp.write(2));assert!(exp.write(0x0b));assert!(!exp.write(0xff),"V003 must reject the V006 high byte");exp.stop();}
        write(exp,0x24,&[5,255]);let mut g=Gpio::new();let mut m=RegRam::new();b.advance_to(0);
        command(&mut *b,&mut g,&mut m,pins,0x11,&[]);command(&mut *b,&mut g,&mut m,pins,0x21,&[]);command(&mut *b,&mut g,&mut m,pins,0x29,&[]);
        command(&mut *b,&mut g,&mut m,pins,0x36,&[if wide{0x48}else{0}]);command(&mut *b,&mut g,&mut m,pins,0x3a,&[if wide{0x66}else{0x55}]);
        command(&mut *b,&mut g,&mut m,pins,0x2a,&[0,0,0,0]);command(&mut *b,&mut g,&mut m,pins,0x2b,&[0,0,0,0]);command(&mut *b,&mut g,&mut m,pins,0x2c,if wide{&[0xfc,0,0]}else{&[0xf8,0]});
        assert_eq!((b.display().unwrap().0,b.display().unwrap().1),size);assert_eq!(b.display().unwrap().2[0],0xf800);
        write(exp,0x24,&[5,0]);b.advance_to(1);assert!(b.display().unwrap().2.iter().all(|&x|x==0));
        write(exp,0x24,&[5,255]);b.advance_to(2);assert_eq!(b.display().unwrap().2[0],0xf800);
        if wide{assert_eq!(b.named_pin("prev"),Some(4));assert_eq!(b.named_pin("next"),Some(5));write(exp,0x24,&[3,0xfe,0xff]);}
        else{b.touch(10,20,true);assert_eq!(b.touch_irq(),Some((18,false)));write(exp,0x24,&[3,0]);assert_eq!(b.touch_irq(),Some((18,true)));}
        b.advance_to(3);assert!(b.display().unwrap().2.iter().all(|&x|x==0),"expander reset must blank the display");
    }
}

#[test]
fn six_boards_route_sd_card_startup_and_response_on_exact_miso(){
    use esp_periph::sdmmc::{card::MemStore,SdCardConfig};
    for(name,clk,mosi,miso,cs,lcd_cs)in[
        ("waveshare-c6-lcd128",11,21,22,15,10),("waveshare-c6-touch-lcd128",11,21,22,15,10),
        ("waveshare-c6-lcd154",1,2,16,17,11),("waveshare-c6-touch-lcd154",1,2,16,17,11),
        ("waveshare-c6-touch28",0,1,8,23,11),("waveshare-c6-lcd273",0,1,18,10,8),
    ]{
        let mut b=make_board(name).unwrap();b.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),SdCardConfig::default()).unwrap();
        let mut p=SpiQspiPins{sclk:1<<clk,data:[1<<mosi,0,0,0],data_width:1,read_width:1,data_bits:80,..Default::default()};
        b.spi_transfer_qspi_pins(2,p,&[255;10],0);p.cs=1<<cs;p.software_cs=1<<cs;p.data_bits=48;p.input[1]=Some(miso);
        b.spi_transfer_qspi_pins(2,p,&[0x40,0,0,0,0,0x95],0);p.data_bits=0;p.read_bits=16;
        assert_eq!(b.spi_transfer_qspi_pins(2,p,&[],2),[255,1],"CMD0 requires the board's actual socket and receive pad");
        p.data_bits=48;p.read_bits=0;b.spi_transfer_qspi_pins(2,p,&[0x48,0,0,1,0xaa,0x87],0);p.data_bits=0;p.read_bits=48;
        assert_eq!(b.spi_transfer_qspi_pins(2,p,&[],6),[255,1,0,0,1,0xaa]);
        p.data_bits=48;p.read_bits=0;b.spi_transfer_qspi_pins(2,p,&[0x40,0,0,0,0,0x95],0);p.data_bits=0;p.read_bits=16;p.input[1]=Some(miso+1);
        assert_eq!(b.spi_transfer_qspi_pins(2,p,&[],2),[255,255],"floating or wrong MISO cannot expose the card response");
        p.input[1]=Some(miso);p.cs|=1<<lcd_cs;p.data_bits=48;p.read_bits=0;
        b.spi_transfer_qspi_pins(2,p,&[0x40,0,0,0,0,0x95],0);p.cs=1<<cs;p.data_bits=0;p.read_bits=16;
        assert_eq!(b.spi_transfer_qspi_pins(2,p,&[],2),[255,255],"simultaneously selected LCD/card is rejected and clears card reply");
    }
}
#[test]
fn third_lcd_boards_do_not_invent_sensors_and_honor_source_windows(){
 for(name,pins,rst,bl,mad,gap,addresses,size)in[
  ("waveshare-c6-geek",(1,2,5,3),4,6,0,(52,40),vec![],(135,240)),
  ("waveshare-c6-geek-v1",(1,2,5,3),4,6,0,(52,40),vec![],(135,240)),
  ("waveshare-c6-lcd13",(7,6,14,15),21,22,0x40,(0,0),vec![],(240,240)),
  ("waveshare-c6-touch-lcd183",(1,2,5,3),4,6,0,(0,0),vec![0x15,0x6b,0x51,0x18,0x40,0x34],(240,284)),
 ]{
  let mut b=make_board(name).unwrap();let ds=b.i2c_devices();assert_eq!(ds.iter().map(|(_,a,_)|*a).collect::<Vec<_>>(),addresses);let mut g=Gpio::new();let mut m=RegRam::new();gpio(&mut g,&mut m,rst,true);gpio(&mut g,&mut m,bl,true);b.gpio_waveform_at(0,ChipPins::C6.routes(&g,&m));
  command(&mut *b,&mut g,&mut m,pins,0x36,&[mad]);command(&mut *b,&mut g,&mut m,pins,0x11,&[]);command(&mut *b,&mut g,&mut m,pins,0x29,&[]);command(&mut *b,&mut g,&mut m,pins,0x3a,&[0x55]);command(&mut *b,&mut g,&mut m,pins,0x2a,&[0,gap.0,0,gap.0]);command(&mut *b,&mut g,&mut m,pins,0x2b,&[0,gap.1,0,gap.1]);command(&mut *b,&mut g,&mut m,pins,0x2c,&[0xf8,0]);assert_eq!((b.display().unwrap().0,b.display().unwrap().1),size);assert_eq!(b.display().unwrap().2[0],0xf800);
  assert_eq!(b.set_imu_motion(1),name.ends_with("183"));assert_eq!(b.leds().is_some(),name.ends_with("13"));
  if name.ends_with("183"){assert!(ds.iter().all(|(_,_,d)|d.pins()==Some((7,8))));b.touch(999,999,true);assert_eq!(b.touch_irq(),Some((11,false)));gpio(&mut g,&mut m,rst,false);b.gpio_waveform_at(0,ChipPins::C6.routes(&g,&m));assert_eq!(b.touch_irq(),Some((11,true)));}
 }
}
#[test]
fn geek_revision_sd_wires_are_distinct_and_lcd13_uses_its_real_socket(){use esp_periph::sdmmc::{card::MemStore,SdCardConfig};
 for(name,clk,mosi,miso,cs)in[("waveshare-c6-geek",1,2,20,23),("waveshare-c6-geek-v1",19,18,20,23),("waveshare-c6-lcd13",7,6,5,4),("waveshare-c6-touch-lcd183",1,2,16,17)]{
  let mut b=make_board(name).unwrap();b.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),SdCardConfig::default()).unwrap();let mut p=SpiQspiPins{sclk:1<<clk,data:[1<<mosi,0,0,0],data_width:1,read_width:1,data_bits:80,..Default::default()};b.spi_transfer_qspi_pins(2,p,&[255;10],0);p.cs=1<<cs;p.software_cs=p.cs;p.input[1]=Some(miso);p.data_bits=48;b.spi_transfer_qspi_pins(2,p,&[0x40,0,0,0,0,0x95],0);p.data_bits=0;p.read_bits=16;assert_eq!(b.spi_transfer_qspi_pins(2,p,&[],2),[255,1]);p.input[1]=Some(24);assert_eq!(b.spi_transfer_qspi_pins(2,p,&[],2),[255,255]);
 }
}

#[test]
fn geek_arduino_full_duplex_write_is_visible_but_miso_stays_floating(){
 let mut b=make_board("waveshare-c6-geek").unwrap();let mut g=Gpio::new();let mut m=RegRam::new();for pin in[4,6]{gpio(&mut g,&mut m,pin,true);}b.gpio_waveform_at(0,ChipPins::C6.routes(&g,&m));
 for(c,a)in[(0x11,vec![]),(0x29,vec![]),(0x3a,vec![5]),(0x2a,vec![0,52,0,52]),(0x2b,vec![0,40,0,40])]{command(&mut *b,&mut g,&mut m,(1,2,5,3),c,&a);}
 command(&mut *b,&mut g,&mut m,(1,2,5,3),0x2c,&[]);gpio(&mut g,&mut m,3,true);b.gpio_waveform_at(0,ChipPins::C6.routes(&g,&m));let p=SpiQspiPins{sclk:2,data:[4,0,0,0],cs:32,data_width:1,data_bits:16,read_width:1,read_bits:16,full_duplex:true,clock_idle_high:true,..Default::default()};assert_eq!(b.spi_transfer_qspi_pins(2,p,&[0xf8,0],2),[255,255]);assert_eq!(b.display().unwrap().2[0],0xf800);
}
