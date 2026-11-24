//! Real ROM boot of unchanged manufacturer firmware with a normal SD image.
//! No application-direct boot, guest memory patches, PC skips or pixel injection.
#[path="../../../../../esp32s3/src/board/waveshare_epaper133.rs"]
mod waveshare_epaper133;
use esp_soc::SocBus;
use emu_core::Bus;

fn main()->Result<(),Box<dyn std::error::Error>>{
    let a:Vec<_>=std::env::args().collect();
    if a.len()!=6{return Err("usage: probe <ROM.elf> <factory.bin> <SD.img> <guest-seconds> <output-prefix>".into());}
    let seconds:u64=a[4].parse()?;if !(1..=120).contains(&seconds){return Err("guest seconds must be 1..120".into());}
    let mut m=esp32s3::Machine::new([0;6],esp32s3::bus::SocBus::new(32<<20,16<<20,[0;6]));
    m.bus.set_flash_size(32<<20);m.bus.set_psram_size(16<<20)?;
    m.bus.board=waveshare_epaper133::make_board(waveshare_epaper133::NAME).ok_or("missing board")?;
    if std::env::var("EPAPER133_SPI_MODE").ok().as_deref()==Some("dc"){m.bus.board=Box::new(waveshare_epaper133::WaveshareEpaper133::dc_mode());}
    m.bus.attach_board_devices();m.bus.gpio_events=Some(Vec::new());m.console.capture=true;m.console.mask=0;
    m.load_rom(&std::fs::read(&a[1])?)?;m.write_flash(0,&std::fs::read(&a[2])?)?;
    println!("{}",m.bus.attach_sd_card(Box::new(esp_periph::sdmmc::MemStore(std::fs::read(&a[3])?)),Default::default())?);
    m.boot_rom();if let Ok(path)=std::env::var("WAVESHARE_INPUT_SCRIPT"){m.load_script(&std::fs::read_to_string(path)?)?;}
    let mut report=String::new();let mut version=0;
    for second in 1..=seconds{
        m.max_cycles=second*240_000_000;
        let stop=loop{let stop=m.run(u64::MAX);if matches!(stop,esp32s3::Stop::SwReset)&&m.reboots<3{m.reboot();}else{break stop;}};
        m.bus.refresh_board_outputs();
        let inputs=m.bus.read32(0x6000_403c).unwrap_or(0);
        let line=format!("second={second} cycles={} instructions={} reboots={} stop={stop:?} pc={:#x} bootInput={} busyInput={} {}\n",m.bus.cycles(),m.insns(),m.reboots,m.cores[0].pc,inputs&1,(inputs>>12)&1,m.bus.board.report());
        print!("{line}");report.push_str(&line);
        let current=m.bus.board.display_version();
        if current!=version{version=current;m.write_tft_png(&format!("{}.frame-{version:03}-{second:03}s.png",a[5]),1)?;}
        // Retain useful progress even when the caller enforces a wall timeout.
        std::fs::write(format!("{}.console.txt",a[5]),&m.console.all)?;
        std::fs::write(format!("{}.uart0.txt",a[5]),&m.console.uart0)?;
        std::fs::write(format!("{}.usb.txt",a[5]),&m.console.usb)?;
        std::fs::write(format!("{}.report.txt",a[5]),&report)?;
        if matches!(stop,esp32s3::Stop::SwReset){break;}
    }
    m.write_tft_png(&format!("{}.final.png",a[5]),1)?;
    let mut edges=String::from("cycle,pin,high\n");
    for &(cycle,pin,high) in m.bus.gpio_events.as_ref().ok_or("missing GPIO capture")?{if [0,1,2,3,10,11,12].contains(&pin){edges.push_str(&format!("{cycle},{pin},{}\n",u8::from(high)));}}
    std::fs::write(format!("{}.gpio.csv",a[5]),edges)?;
    std::fs::write(format!("{}.soc.txt",a[5]),m.bus.report())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_busy_edges_reach_gpio_input_without_a_host_refresh(){
        let mut bus=esp32s3::bus::SocBus::new(32<<20,16<<20,[0;6]);
        bus.board=waveshare_epaper133::make_board(waveshare_epaper133::NAME).unwrap();bus.attach_board_devices();
        assert_eq!(bus.read32(0x6000_403c).unwrap()&(1<<12),0);
        for pin in [1,2]{bus.write32(0x6000_9004+pin*4,1<<12).unwrap();bus.write32(0x6000_4554+pin*4,0x100).unwrap();}
        bus.write32(0x6000_4004,(1<<1)|(1<<2)).unwrap();bus.write32(0x6000_4020,(1<<1)|(1<<2)).unwrap();
        assert_ne!(bus.read32(0x6000_403c).unwrap()&(1<<12),0,"powered, released panel must return HIGH idle on physical BUSY GPIO12");
        bus.write32(0x6000_400c,1<<2).unwrap();
        assert_eq!(bus.read32(0x6000_403c).unwrap()&(1<<12),0,"reset immediately updates board-driven GPIO input");
    }
}
