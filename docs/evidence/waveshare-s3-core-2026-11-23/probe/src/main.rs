//! Actual S3 ROM boot of original manufacturer-linked firmware segments.
#[path="../../../../../esp32s3/src/board/waveshare_core.rs"]
mod waveshare_core;
use esp_soc::SocBus;
use emu_core::Bus;

fn main()->Result<(),Box<dyn std::error::Error>> {
    let a:Vec<_>=std::env::args().collect();if a.len()<6 {return Err("usage: probe <board> <ROM> <seconds> <output-prefix> <hex-offset:file>...".into());}
    let v=waveshare_core::variant(&a[1]).ok_or("unknown exact board")?;let seconds:u64=a[3].parse()?;if !(1..=30).contains(&seconds){return Err("guest seconds must be 1..30".into());}
    let (flash,psram)=v.memory_mib();let mut m=esp32s3::Machine::new([0;6],esp32s3::bus::SocBus::new(flash<<20,psram<<20,[0;6]));
    m.bus.set_flash_size(flash<<20);m.bus.set_psram_size(psram<<20)?;m.bus.board=waveshare_core::make_board(&a[1]).ok_or("missing board")?;m.bus.attach_board_devices();m.bus.gpio_events=Some(Vec::new());m.console.capture=true;m.console.mask=0;
    m.load_rom(&std::fs::read(&a[2])?)?;
    for segment in &a[5..] {let (offset,path)=segment.split_once(':').ok_or("segment requires offset:path")?;let offset=usize::from_str_radix(offset.trim_start_matches("0x"),16)?;let bytes=std::fs::read(path)?;
        if offset==0&&(bytes.len()<24||bytes[0]!=0xe9||u16::from_le_bytes([bytes[12],bytes[13]])!=9){return Err("boot segment is not an ESP32-S3 image (chip ID 9)".into());}m.write_flash(offset,&bytes)?;}
    m.boot_rom();if let Ok(path)=std::env::var("WAVESHARE_INPUT_SCRIPT"){
        m.load_script(&std::fs::read_to_string(path)?)?;
        // MicroPython's interactive terminal submits on CR. Keep the source
        // script readable; deliver a real UART Enter, not a firmware patch.
        for (_,action) in &mut m.script.events {if let esp_soc::machine::ScriptAction::Uart(0,text)=action {if text.ends_with('\n'){text.pop();text.push('\r');}}}
    }
    let mut reports=String::new();
    for sample in 1..=seconds*10 {m.max_cycles=sample*24_000_000;let stop=m.run(u64::MAX);m.bus.refresh_board_outputs();let gpio=m.bus.read32(0x6000_403c).unwrap_or(0);
        let line=format!("sample={sample} cycles={} instructions={} reboots={} stop={stop:?} bootLevel={} {}\n",m.bus.cycles(),m.insns(),m.reboots,gpio&1,m.bus.board.report());reports.push_str(&line);
        if sample%10==0 {print!("{line}");std::fs::write(format!("{}.console.txt",a[4]),&m.console.all)?;std::fs::write(format!("{}.report.txt",a[4]),&reports)?;}
        if matches!(stop,esp32s3::Stop::SwReset){break;}
    }
    std::fs::write(format!("{}.console.txt",a[4]),&m.console.all)?;std::fs::write(format!("{}.uart0.txt",a[4]),&m.console.uart0)?;std::fs::write(format!("{}.usb.txt",a[4]),&m.console.usb)?;std::fs::write(format!("{}.report.txt",a[4]),reports)?;
    std::fs::write(format!("{}.soc.txt",a[4]),m.bus.report())?;Ok(())
}
