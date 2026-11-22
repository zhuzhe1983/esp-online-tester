//! Guest MMIO coverage for bare EE adapters and the distinct attached panels.
//! ESP-IDF v4.4.7 S3 gpio_reg.h/spi_reg.h and gpio_sig_map.h: FSPI
//! CLK/D/Q/CS0=101/103/102/110; software GPIO=256. No direct panel writes.
//! The factory descriptor identifies v4.4.7-dirty; these fields are unchanged
//! from the v5.5.2 SPI transport fixtures. Header hashes are in EE evidence.
use emu_core::Bus;

const MS:u32=(esp32s3::periph::CPU_HZ/1000) as u32;
fn machine(id:&str)->esp32s3::Machine{let mut m=esp32s3::machine([0;6]);m.bus.board=esp32s3::board::make_board(id).unwrap();m.bus.attach_board_devices();m}
fn mux(bus:&mut esp32s3::bus::SocBus,pin:u8,signal:u32){bus.write32(0x6000_9004+4*u32::from(pin),1<<12|1<<9).unwrap();bus.write32(0x6000_4554+4*u32::from(pin),signal).unwrap();}
fn gpio(bus:&mut esp32s3::bus::SocBus,pin:u8,high:bool){let bank=u32::from(pin>=32)*12;bus.write32(0x6000_4024+bank,1<<(pin%32)).unwrap();bus.write32(0x6000_4008+bank+if high{0}else{4},1<<(pin%32)).unwrap();}
fn setup(bus:&mut esp32s3::bus::SocBus){for (pin,signal) in [(7,101),(9,103),(44,110),(10,256),(38,256),(43,256)]{mux(bus,pin,signal);}gpio(bus,38,true);gpio(bus,43,true);}
fn transfer(bus:&mut esp32s3::bus::SocBus,tx:&[u8],read:bool){assert!(!tx.is_empty()&&tx.len()<=64);for (i,word) in tx.chunks(4).enumerate(){let mut b=[0;4];b[..word.len()].copy_from_slice(word);bus.write32(0x6002_4098+4*i as u32,u32::from_le_bytes(b)).unwrap();}
    for (off,v) in [(0x10,1<<27|if read{1<<28}else{0}),(0x1c,(tx.len()*8-1) as u32),(0x20,0x3e),(0,1<<24)]{bus.write32(0x6002_4000+off,v).unwrap();}}
fn send(bus:&mut esp32s3::bus::SocBus,c:u8,data:&[u8]){gpio(bus,10,false);transfer(bus,&[c],false);gpio(bus,10,true);for chunk in data.chunks(64){transfer(bus,chunk,false);}}
fn pixels(bus:&esp32s3::bus::SocBus)->Vec<u16>{bus.board.display().unwrap().2}

#[test]
fn bare_adapters_do_not_invent_glass_busy_or_spi_responses(){
    for id in ["seeed-xiao-epaper-ee02","seeed-xiao-epaper-ee03","seeed-xiao-epaper-ee04","seeed-xiao-epaper-ee05"]{
        let mut m=machine(id);let b=&mut m.bus;setup(b);send(b,0x61,&[3,32,1,224]);send(b,0x04,&[]);send(b,0x13,&[0]);send(b,0x12,&[]);b.tick(MS*50);
        assert_eq!(b.board.name(),id);assert!(b.board.display().is_none());assert_eq!(b.board.display_frames(),0);assert_eq!(b.board.display_version(),0);
        assert!(!b.board.uses_spi_pins());assert!(!b.board.input_levels().iter().any(|(p,_)|*p==4));assert!(b.board.next_deadline().is_none());assert!(b.board.take_edges().is_empty());
        assert_eq!(b.board.spi_transfer(2,&[0x10,0],4),[255;4]);
    }
    assert!(esp32s3::board::make_board("seeed-xiao-epaper-ee03-p075-mono").is_none(),"unsupported adapter/glass combination must not silently alias");
}

#[test]
fn variant_buttons_reach_actual_gpio_in_and_ee05_does_not_use_gpio5(){
    for (id,third) in [("seeed-xiao-epaper-ee02",5),("seeed-xiao-epaper-ee03",5),("seeed-xiao-epaper-ee04",5),("seeed-xiao-epaper-ee05",8)]{
        let mut m=machine(id);let bus=&mut m.bus;
        for (name,pin) in [("boot",0),("key1",2),("key2",3),("key3",third)]{
            assert_eq!(bus.board.named_pin(name),Some(pin));mux(bus,pin,256);
            assert_ne!(bus.read32(0x6000_403c).unwrap()&(1<<pin),0);
            esp_soc::SocBus::gpio_set_input(bus,pin,false);assert_eq!(bus.read32(0x6000_403c).unwrap()&(1<<pin),0);
            esp_soc::SocBus::gpio_set_input(bus,pin,true);assert_ne!(bus.read32(0x6000_403c).unwrap()&(1<<pin),0);
        }
    }
}

#[test]
fn schematics_battery_gate_uses_gpio39_on_ee05_and_preserves_host_overrides(){
    for (id,enable) in [("seeed-xiao-epaper-ee02",6),("seeed-xiao-epaper-ee03",6),("seeed-xiao-epaper-ee04",6),("seeed-xiao-epaper-ee05",39)]{
        let mut m=machine(id);let bus=&mut m.bus;assert_eq!(bus.periph.rtc.analog.volts(1,0),0.0);
        if enable==39{mux(bus,6,256);gpio(bus,6,true);assert_eq!(bus.periph.rtc.analog.volts(1,0),0.0,"SenseCraft's common GPIO6 definition cannot power EE05's physical divider");}
        gpio(bus,enable,true);assert_eq!(bus.periph.rtc.analog.volts(1,0),0.0,"unmuxed output is not ADC_EN");
        mux(bus,enable,256);assert_eq!(bus.periph.rtc.analog.volts(1,0),2.0);gpio(bus,enable,false);assert_eq!(bus.periph.rtc.analog.volts(1,0),0.0);
        bus.periph.rtc.analog.set(1,esp_periph::AnalogSource::Const(1.7));gpio(bus,enable,true);bus.attach_board_devices();assert_eq!(bus.periph.rtc.analog.volts(1,0),1.7);
        mux(bus,21,256);gpio(bus,21,false);assert_eq!(bus.board.leds().unwrap().0,&[[0,255,0]]);gpio(bus,21,true);assert_eq!(bus.board.leds().unwrap().0,&[[0,0,0]]);
    }
}

#[test]
fn uc8179_requires_real_clock_mosi_cs_power_reset_and_busy_completion(){
    let mut m=machine("seeed-xiao-epaper-ee04-p075-mono");let bus=&mut m.bus;setup(bus);
    send(bus,0x61,&[3,32,1,224]);send(bus,0x04,&[]);bus.tick(MS*2);
    for (pin,signal) in [(7,101),(9,103),(44,110)]{mux(bus,pin,256);if pin==44{gpio(bus,pin,true);}send(bus,0x13,&[0]);mux(bus,pin,signal);}
    send(bus,0x13,&[0x7f]);send(bus,0x12,&[]);assert_eq!(bus.board.display_frames(),0);assert_eq!(pixels(bus)[0],0xffff);assert!(bus.board.input_levels().contains(&(4,false)));
    bus.tick(MS*21);assert_eq!(bus.board.display_frames(),1);assert_eq!(&pixels(bus)[..3],&[0,0xffff,0xffff]);
    gpio(bus,38,false);send(bus,0x13,&[0]);send(bus,0x12,&[]);bus.tick(MS*21);assert_eq!(bus.board.display_frames(),1);assert_eq!(pixels(bus)[0],0);
    gpio(bus,38,true);gpio(bus,43,false);send(bus,0x61,&[3,32,1,224]);send(bus,0x04,&[]);send(bus,0x12,&[]);bus.tick(MS*21);assert_eq!(bus.board.display_frames(),1);
    gpio(bus,43,true);send(bus,0x61,&[3,32,1,224]);send(bus,0x04,&[]);bus.tick(MS*2);send(bus,0x12,&[]);bus.tick(MS*21);assert!(pixels(bus).iter().all(|p|*p==0xffff),"power loss clears volatile controller RAM, retaining ink until refresh");
}

#[test]
fn t133a01_secondary_chip_select_controls_the_other_half_of_the_glass(){
    let mut m=machine("seeed-xiao-epaper-ee02-p133-sp6");let bus=&mut m.bus;setup(bus);mux(bus,41,110);
    send(bus,0x61,&[4,0xb0,3,0x20]);mux(bus,41,256);gpio(bus,41,true);send(bus,0x10,&[0x23]);
    mux(bus,44,256);gpio(bus,44,true);mux(bus,41,110);send(bus,0x10,&[0x56]);mux(bus,44,110);
    send(bus,0x04,&[]);bus.tick(MS*2);send(bus,0x12,&[1]);assert_eq!(bus.board.display_frames(),0);bus.tick(MS*21);
    let (w,h,frame,_)=bus.board.display().unwrap();assert_eq!((w,h),(1200,1600));assert_eq!(&frame[..2],&[0xffe0,0xf800]);assert_eq!(&frame[600..602],&[0x001f,0x07e0]);assert_eq!(frame[1200],0xffff);
}

#[test]
fn ssd1681_and_ssd1677_use_their_distinct_address_formats_and_busy_polarity(){
    for (id,width,height) in [("seeed-xiao-epaper-ee04-p0154-mono",200,200),("seeed-xiao-epaper-ee05-p0426-mono",800,480)]{
        let mut m=machine(id);let bus=&mut m.bus;setup(bus);assert!(bus.board.input_levels().contains(&(4,false)));
        send(bus,0x11,&[3]);if width==200{send(bus,0x44,&[0,24]);send(bus,0x4e,&[0]);}else{send(bus,0x44,&[0,0,0x1f,3]);send(bus,0x4e,&[0,0]);}
        send(bus,0x45,&[0,0,(height-1) as u8,((height-1)>>8) as u8]);send(bus,0x4f,&[0,0]);send(bus,0x24,&[0x7f]);send(bus,0x22,&[0xf7]);send(bus,0x20,&[]);
        assert!(bus.board.input_levels().contains(&(4,true)));assert_eq!(bus.board.display_frames(),0);bus.tick(MS*21);
        let (w,h,frame,_)=bus.board.display().unwrap();assert_eq!((w,h),(width,height));assert_eq!(frame.iter().filter(|p|**p==0).count(),1);assert_eq!(bus.board.display_frames(),1);assert!(bus.board.input_levels().contains(&(4,false)));
    }
}

#[test]
fn ed2208_730_is_color_and_it8951_1030_requires_continuous_cs_and_real_miso(){
    let mut m=machine("seeed-xiao-epaper-ee04-p073-sp6");let bus=&mut m.bus;setup(bus);send(bus,0x61,&[3,32,1,224]);send(bus,0x10,&[0x01,0x23,0x56]);send(bus,0x04,&[]);bus.tick(MS*2);send(bus,0x12,&[0]);bus.tick(MS*21);
    assert_eq!(&pixels(bus)[..6],&[0,0xffff,0xff87,0xb800,0x621f,0x4443]);assert_eq!(bus.board.display_frames(),1);
    let mut m=machine("seeed-xiao-epaper-ee03-p103-mono");let bus=&mut m.bus;setup(bus);mux(bus,44,256);gpio(bus,44,true);mux(bus,8,256);bus.write32(0x6000_4154+4*102,0x80|8).unwrap();
    // Software CS stays low across separate SPI.transfer16 calls, as in the
    // official TCON driver. Hardware CS0 is not wired when GPIO44 is unmapped.
    gpio(bus,44,false);transfer(bus,&[0x60,0],true);transfer(bus,&[3,2],true);gpio(bus,44,true);
    gpio(bus,44,false);transfer(bus,&[0x10,0],true);transfer(bus,&[0,0],true);assert_eq!(bus.read32(0x6002_4098).unwrap()&0xffff,0);
    transfer(bus,&[0,0],true);assert_eq!((bus.read32(0x6002_4098).unwrap()&0xffff).swap_bytes()>>16,1872);
    transfer(bus,&[0,0],true);assert_eq!((bus.read32(0x6002_4098).unwrap()&0xffff).swap_bytes()>>16,1404);gpio(bus,44,true);
    gpio(bus,44,false);transfer(bus,&[0x10,0],true);gpio(bus,44,true);gpio(bus,44,false);transfer(bus,&[0,0],true);assert_eq!(bus.read32(0x6002_4098).unwrap()&0xffff,0xffff,"deasserting CS ends the read preamble");gpio(bus,44,true);
    bus.write32(0x6000_4154+4*102,0x80|6).unwrap();gpio(bus,44,false);transfer(bus,&[0x10,0,0,0,0,0],true);assert_eq!(bus.read32(0x6002_4098).unwrap(),0xffff_ffff,"unwired MISO cannot return TCON info");
    let (w,h,_,_)=bus.board.display().unwrap();assert_eq!((w,h),(1872,1404));
}

#[test]
fn every_official_kit_selects_its_own_bounded_visible_geometry(){
    let common=[("p0154-mono",200,200),("p0213-mono",122,250),("p0213-quad",122,250),("p0213-flex",104,212),("p029-mono",128,296),("p029-quad",128,296),("p029-flex",128,296),("p042-mono",400,300),("p0426-mono",800,480),("p0583-mono",648,480),("p075-mono",800,480)];
    let mut cases=vec![("seeed-xiao-epaper-ee02-p133-sp6".to_string(),1200,1600),("seeed-xiao-epaper-ee03-p103-mono".to_string(),1872,1404),("seeed-xiao-epaper-ee04-p073-sp6".to_string(),800,480)];
    for adapter in ["ee04","ee05"]{for (glass,w,h) in common{cases.push((format!("seeed-xiao-epaper-{adapter}-{glass}"),w,h));}}
    assert_eq!(cases.len(),25);
    for (id,w,h) in cases{let m=machine(&id);let (width,height,frame,_)=m.bus.board.display().unwrap();assert_eq!((width,height),(w,h),"{id}");assert_eq!(frame.len(),(w*h) as usize);assert_eq!(m.bus.board.name(),id);assert_eq!(m.bus.board.display_frames(),0);}
}

#[test]
fn ssd1680_and_ssd1683_guest_byte_windows_preserve_native_rows(){
    for (id,width,height,stride) in [("seeed-xiao-epaper-ee04-p0213-mono",122,250,16),("seeed-xiao-epaper-ee05-p029-mono",128,296,16),("seeed-xiao-epaper-ee04-p042-mono",400,300,50)]{
        let mut m=machine(id);let bus=&mut m.bus;setup(bus);send(bus,0x44,&[0,stride-1]);send(bus,0x45,&[0,0,(height-1) as u8,((height-1)>>8) as u8]);send(bus,0x4e,&[0]);send(bus,0x4f,&[0,0]);
        let mut row=vec![255;stride as usize];if width==122{row[15]=0xc0;}send(bus,0x24,&row);transfer(bus,&[0x7f],false);send(bus,0x22,&[0xf7]);send(bus,0x20,&[]);assert_eq!(bus.board.display_frames(),0);bus.tick(MS*21);
        let frame=pixels(bus);assert_eq!(frame.iter().filter(|p|**p==0).count(),1);assert_eq!(frame[width],0);assert_eq!(frame[width+1],0xffff);assert_eq!(frame[width-1],0xffff);
    }
}

#[test]
fn jd796xx_guest_wire_colors_and_hidden_columns_do_not_become_grayscale(){
    for (id,width,height) in [("seeed-xiao-epaper-ee04-p0213-quad",122,250),("seeed-xiao-epaper-ee05-p029-quad",128,296)]{
        let mut m=machine(id);let bus=&mut m.bus;setup(bus);send(bus,0x00,&[0x1f]);send(bus,0x61,&[0,128,(height>>8) as u8,height as u8]);send(bus,0x04,&[]);bus.tick(MS*2);
        let mut data=vec![0;33];data[0]=0x1b;data[32]=0x40;if width==122{data[30]=0x0f;data[31]=255;}send(bus,0x10,&data);send(bus,0x12,&[0]);assert!(bus.board.input_levels().contains(&(4,false)));bus.tick(MS*21);
        let frame=pixels(bus);assert_eq!(&frame[..4],&[0xffff,0,0xffe0,0xf800]);assert_eq!(frame[width],0);assert_eq!(frame[width-1],0xffff);assert_eq!(frame.len(),width*height);
    }
}

#[test]
fn flexible_uc8151_uses_three_byte_resolution_and_seven_byte_partial_window(){
    for (id,width,height) in [("seeed-xiao-epaper-ee04-p0213-flex",104,212),("seeed-xiao-epaper-ee05-p029-flex",128,296)]{
        let mut m=machine(id);let bus=&mut m.bus;setup(bus);send(bus,0x04,&[]);bus.tick(MS*2);send(bus,0x00,&[0x1f]);send(bus,0x61,&[width as u8,(height>>8) as u8,height as u8]);
        send(bus,0x91,&[]);send(bus,0x90,&[8,15,0,2,0,2,1]);send(bus,0x13,&[0]);send(bus,0x12,&[]);assert_eq!(bus.board.display_frames(),0);bus.tick(MS*21);
        let frame=pixels(bus);assert_eq!(&frame[2*width+8..2*width+16],&[0;8]);assert_eq!(frame.iter().filter(|p|**p==0).count(),8);
        send(bus,0x90,&[0,8,0,15,0,2,0,2,1]);send(bus,0x13,&[0;20]);send(bus,0x12,&[]);bus.tick(MS*21);assert_eq!(bus.board.display_frames(),1,"a UC8179-shaped window is not silently repaired for UC8151");
    }
}

#[test]
fn uc8179_583_uses_648_storage_pixels_and_rejects_a_750_resolution(){
    let mut m=machine("seeed-xiao-epaper-ee05-p0583-mono");let bus=&mut m.bus;setup(bus);send(bus,0x61,&[2,0x88,1,0xe0]);send(bus,0x04,&[]);bus.tick(MS*2);
    let mut data=vec![255;82];data[80]=0x7f;data[81]=0x7f;send(bus,0x13,&data);send(bus,0x12,&[]);bus.tick(MS*21);let frame=pixels(bus);assert_eq!(frame.len(),648*480);assert_eq!(frame[640],0);assert_eq!(frame[648],0);assert_eq!(frame[649],0xffff);
    send(bus,0x61,&[3,32,1,224]);send(bus,0x12,&[]);bus.tick(MS*21);assert_eq!(bus.board.display_frames(),1);assert_eq!(pixels(bus),frame);
}
