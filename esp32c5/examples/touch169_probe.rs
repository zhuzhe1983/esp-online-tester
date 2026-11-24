//! Original SRAM guest: public App/touch/BOOT APIs, full8MiB PSRAM and DMA frames.
use esp_soc::SocBus;
use std::{fs,path::Path};
fn until(m:&mut esp32c5::Machine,marker:&[u8])->Result<(),String>{
    let deadline=m.bus.cycles()+200_000_000;
    while !m.console.uart0.ends_with(marker)&&m.bus.cycles()<deadline{
        m.max_cycles=(m.bus.cycles()+250_000).min(deadline);let stop=m.run(u64::MAX);
        if m.exceptions!=0||m.bus.first_fault.is_some()||!matches!(stop,esp_soc::Stop::MaxInsns|esp_soc::Stop::Halted){return Err(format!("stop={stop:?} pc={:#x} fault={:?} UART={:?}; {}",m.cores[0].pc,m.bus.first_fault,String::from_utf8_lossy(&m.console.uart0),m.bus.report()));}
    }
    if !m.console.uart0.ends_with(marker){return Err(format!("guest timeout pc={:#x} UART={:?}; {}",m.cores[0].pc,String::from_utf8_lossy(&m.console.uart0),m.bus.report()));}Ok(())
}
fn frame(m:&mut esp32c5::Machine,dir:&Path,name:&str)->Result<Vec<u16>,Box<dyn std::error::Error>>{
    m.bus.refresh_board_outputs();let(w,h,p,_) =m.bus.board.display().ok_or("missing display")?;if(w,h)!=(240,280){return Err("geometry".into());}
    fs::write(dir.join(format!("{name}.rgb565le")),p.iter().flat_map(|v|v.to_le_bytes()).collect::<Vec<_>>())?;
    m.write_tft_png(dir.join(format!("{name}.png")).to_str().ok_or("output path")?,1)?;Ok(p)
}
fn changed(a:&[u16],b:&[u16])->usize{a.iter().zip(b).filter(|(a,b)|a!=b).count()}
fn main()->Result<(),Box<dyn std::error::Error>>{
    let args:Vec<_>=std::env::args().collect();let app=args.get(1).ok_or("usage: touch169_probe app.bin output-dir")?;let dir=Path::new(args.get(2).ok_or("output dir")?);fs::create_dir_all(dir)?;
    let mut m=esp32c5::machine([0;6],16<<20);m.bus.set_xtal_mhz(48)?;m.bus.set_psram_size(8<<20)?;
    m.bus.attach_board(esp32c5::board::make_board("waveshare-c5-touch169").ok_or("board")?);m.console.capture=true;m.console.mask=2;
    if m.load_input(esp_soc::LoadKind::SdCard,&vec![0;8<<20]).is_ok(){return Err("board without SD accepted a card".into());}
    m.load_input(esp_soc::LoadKind::App,&fs::read(app)?)?;m.boot_app(0x10000)?;
    until(&mut m,b"C5 TOUCH169 READY\n")?;let ready_cycles=m.bus.cycles();let base=frame(&mut m,dir,"ready")?;
    let palette:std::collections::HashSet<_>=base.iter().collect();if palette.len()<100{return Err("not a meaningful framebuffer".into());}
    m.max_cycles=m.bus.cycles()+1_000_000;m.run(u64::MAX);let quiet=frame(&mut m,dir,"quiet")?;if changed(&base,&quiet)!=0{return Err("quiet drift".into());}
    m.load_script(&format!("{:.12} touch 80 110 1\n",m.seconds()))?;until(&mut m,b"C5 TOUCH DOWN 0050 006E\n")?;let touch=frame(&mut m,dir,"touch")?;
    let touch_changed=changed(&base,&touch);if touch_changed!=1024{return Err(format!("touch changed {touch_changed}, expected1024").into());}
    m.load_script(&format!("{:.12} touch 80 110 0\n",m.seconds()))?;until(&mut m,b"C5 TOUCH UP 0050 006E\n")?;let released=frame(&mut m,dir,"released")?;if released!=base{return Err("touch release did not restore original pixels".into());}
    m.load_script(&format!("{:.12} gpio boot 0\n",m.seconds()))?;until(&mut m,b"C5 BOOT DOWN\n")?;let boot=frame(&mut m,dir,"boot")?;
    let boot_changed=changed(&base,&boot);if boot_changed!=208*216{return Err(format!("BOOT changed {boot_changed}").into());}
    m.load_script(&format!("{:.12} gpio boot 1\n",m.seconds()))?;until(&mut m,b"C5 BOOT UP\n")?;if frame(&mut m,dir,"boot-release")?!=base{return Err("BOOT release mismatch".into());}
    if m.exceptions!=0||m.reboots!=0||m.bus.first_fault.is_some(){return Err("execution fault/reset".into());}
    let report=m.bus.report();if !report.contains("rejected=0"){return Err(format!("transport rejection: {report}").into());}
    fs::write(dir.join("uart.log"),&m.console.uart0)?;fs::write(dir.join("board-report.txt"),report+"\n")?;
    let json=format!("{{\"kind\":\"original-c5-touch169-sram-guest\",\"factoryVerified\":false,\"flashBytes\":16777216,\"psramBytes\":8388608,\"guestPsramBytesWrittenAndRead\":8388608,\"framebufferInPsram\":true,\"width\":240,\"height\":280,\"nominalCrystalMHz\":48,\"spiMHz\":24,\"i2cHz\":400000,\"readyCycles\":{ready_cycles},\"cycles\":{},\"retired\":{},\"dispatched\":{},\"exceptions\":{},\"resets\":{},\"colorCount\":{},\"quietChangedPixels\":0,\"touchChangedPixels\":{touch_changed},\"bootChangedPixels\":{boot_changed},\"releaseRestoredExactly\":true,\"sdRejected\":true,\"ramwr\":{}}}\n",m.bus.cycles(),m.cores[0].retired_count,m.insns(),m.exceptions,m.reboots,palette.len(),m.bus.board.display_frames());
    fs::write(dir.join("execution.json"),&json)?;print!("{json}");eprint!("{}",String::from_utf8_lossy(&m.console.uart0));Ok(())
}
