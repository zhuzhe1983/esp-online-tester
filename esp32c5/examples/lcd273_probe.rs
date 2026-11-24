//! Original SRAM guest: public App/GPIO/SD APIs, full8MiB PSRAM and RGB666 DMA frames.
use esp_soc::SocBus;
use std::{fs,path::Path,sync::{Arc,Mutex}};
struct ObservedCard(Arc<Mutex<Vec<u8>>>);
impl esp_periph::sdmmc::SdStore for ObservedCard {
    fn len(&self)->u64{self.0.lock().unwrap().len() as u64}
    fn read(&mut self,lba:u64,buf:&mut [u8;512])->std::io::Result<()>{let data=self.0.lock().unwrap();let at=lba as usize*512;buf.copy_from_slice(data.get(at..at+512).ok_or(std::io::ErrorKind::UnexpectedEof)?);Ok(())}
    fn write(&mut self,lba:u64,buf:&[u8;512])->std::io::Result<()>{let mut data=self.0.lock().unwrap();let at=lba as usize*512;data.get_mut(at..at+512).ok_or(std::io::ErrorKind::UnexpectedEof)?.copy_from_slice(buf);Ok(())}
}

fn until(m:&mut esp32c5::Machine,marker:&[u8])->Result<(),String>{
    let deadline=m.bus.cycles()+200_000_000;
    while !m.console.uart0.ends_with(marker)&&m.bus.cycles()<deadline{
        m.max_cycles=(m.bus.cycles()+250_000).min(deadline);let stop=m.run(u64::MAX);
        if m.exceptions!=0||m.bus.first_fault.is_some()||!matches!(stop,esp_soc::Stop::MaxInsns|esp_soc::Stop::Halted){return Err(format!("stop={stop:?} pc={:#x} fault={:?} UART={:?}; {}",m.cores[0].pc,m.bus.first_fault,String::from_utf8_lossy(&m.console.uart0),m.bus.report()));}
    }
    if !m.console.uart0.ends_with(marker){return Err(format!("guest timeout pc={:#x} UART={:?}; {}",m.cores[0].pc,String::from_utf8_lossy(&m.console.uart0),m.bus.report()));}Ok(())
}
fn frame(m:&mut esp32c5::Machine,dir:&Path,name:&str)->Result<Vec<u16>,Box<dyn std::error::Error>>{
    m.bus.refresh_board_outputs();let(w,h,p,_) =m.bus.board.display().ok_or("missing display")?;if(w,h)!=(320,320){return Err("geometry".into());}
    fs::write(dir.join(format!("{name}.rgb565le")),p.iter().flat_map(|v|v.to_le_bytes()).collect::<Vec<_>>())?;
    m.write_tft_png(dir.join(format!("{name}.png")).to_str().ok_or("output path")?,1)?;Ok(p)
}
fn changed(a:&[u16],b:&[u16])->usize{a.iter().zip(b).filter(|(a,b)|a!=b).count()}
fn main()->Result<(),Box<dyn std::error::Error>>{
    let args:Vec<_>=std::env::args().collect();let app=args.get(1).ok_or("usage: lcd273_probe app.bin output-dir")?;let dir=Path::new(args.get(2).ok_or("output dir")?);fs::create_dir_all(dir)?;
    let mut m=esp32c5::machine([0;6],32<<20);m.bus.set_xtal_mhz(48)?;m.bus.set_psram_size(8<<20)?;
    m.bus.attach_board(esp32c5::board::make_board("waveshare-c5-lcd273").ok_or("board")?);m.console.capture=true;m.console.mask=2;
    let original:Vec<u8>=(0..8<<20).map(|i|((i*31)^(i>>9)^0x37)as u8).collect();
    let card=Arc::new(Mutex::new(original.clone()));m.bus.attach_sd_card(Box::new(ObservedCard(card.clone())),Default::default())?;
    m.load_input(esp_soc::LoadKind::App,&fs::read(app)?)?;m.boot_app(0x10000)?;
    until(&mut m,b"C5 LCD273 READY\n")?;let ready_cycles=m.bus.cycles();let base=frame(&mut m,dir,"ready")?;
    let palette:std::collections::HashSet<_>=base.iter().collect();if palette.len()<100{return Err("not a meaningful framebuffer".into());}
    m.max_cycles=m.bus.cycles()+1_000_000;m.run(u64::MAX);let quiet=frame(&mut m,dir,"quiet")?;if changed(&base,&quiet)!=0{return Err("quiet drift".into());}
    let mut deltas=Vec::new();
    for(name,label,expected)in[("prev","PREV",88*304),("next","NEXT",88*304),("boot","BOOT",288*256)]{
        m.load_script(&format!("{:.12} gpio {name} 0\n",m.seconds()))?;until(&mut m,format!("C5 {label} DOWN\n").as_bytes())?;
        let pixels=frame(&mut m,dir,name)?;let delta=changed(&base,&pixels);if delta!=expected{return Err(format!("{name} changed{delta}, expected{expected}").into());}deltas.push(delta);
        m.load_script(&format!("{:.12} gpio {name} 1\n",m.seconds()))?;until(&mut m,format!("C5 {label} UP\n").as_bytes())?;
        if frame(&mut m,dir,&format!("{name}-release"))?!=base{return Err(format!("{name} release mismatch").into());}
    }
    if m.exceptions!=0||m.reboots!=0||m.bus.first_fault.is_some(){return Err("execution fault/reset".into());}
    if *card.lock().unwrap()!=original{return Err("guest did not preserve original SD image".into());}
    if !m.bus.report().contains("read-blocks=3 written-blocks=2"){return Err("missing actual SD save/test/restore commands".into());}
    let report=m.bus.report();if !report.contains("rejected=0"){return Err(format!("transport rejection: {report}").into());}
    fs::write(dir.join("uart.log"),&m.console.uart0)?;fs::write(dir.join("board-report.txt"),report+"\n")?;
    let json=format!("{{\"kind\":\"original-c5-lcd273-sram-guest\",\"factoryVerified\":false,\"flashBytes\":33554432,\"psramBytes\":8388608,\"guestPsramBytesWrittenAndRead\":8388608,\"framebufferInPsram\":true,\"wirePixelFormat\":\"RGB666-three-bytes\",\"hostPixelFormat\":\"RGB565\",\"framebufferBytes\":307200,\"width\":320,\"height\":320,\"nominalCrystalMHz\":48,\"spiMHz\":24,\"i2cHz\":400000,\"readyCycles\":{ready_cycles},\"cycles\":{},\"retired\":{},\"dispatched\":{},\"exceptions\":{},\"resets\":{},\"colorCount\":{},\"quietChangedPixels\":0,\"prevChangedPixels\":{},\"nextChangedPixels\":{},\"bootChangedPixels\":{},\"releaseRestoredExactly\":true,\"sdOriginalImageRestoredExactly\":true,\"sdGuestLba\":32,\"sdBytesWrittenAndRead\":512,\"sdReadBlocks\":3,\"sdWrittenBlocks\":2,\"sdFilesystemMounted\":false,\"ramwr\":{}}}\n",m.bus.cycles(),m.cores[0].retired_count,m.insns(),m.exceptions,m.reboots,palette.len(),deltas[0],deltas[1],deltas[2],m.bus.board.display_frames());
    fs::write(dir.join("execution.json"),&json)?;print!("{json}");eprint!("{}",String::from_utf8_lossy(&m.console.uart0));Ok(())
}
