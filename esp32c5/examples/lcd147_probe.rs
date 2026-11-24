//! Reproducible original SRAM guest through Machine App/SD/input APIs.
use esp_soc::SocBus;
use std::{path::Path,fs,sync::{Arc,Mutex}};
struct ObservedCard(Arc<Mutex<Vec<u8>>>);
impl esp_periph::sdmmc::SdStore for ObservedCard {
    fn len(&self)->u64{self.0.lock().unwrap().len() as u64}
    fn read(&mut self,lba:u64,buf:&mut [u8;512])->std::io::Result<()>{let data=self.0.lock().unwrap();let at=lba as usize*512;buf.copy_from_slice(data.get(at..at+512).ok_or(std::io::ErrorKind::UnexpectedEof)?);Ok(())}
    fn write(&mut self,lba:u64,buf:&[u8;512])->std::io::Result<()>{let mut data=self.0.lock().unwrap();let at=lba as usize*512;data.get_mut(at..at+512).ok_or(std::io::ErrorKind::UnexpectedEof)?.copy_from_slice(buf);Ok(())}
}
fn until(m:&mut esp32c5::Machine,marker:&[u8])->Result<(),String>{
    let deadline=m.bus.cycles()+40_000_000;
    while !m.console.uart0.ends_with(marker)&&m.bus.cycles()<deadline{
        m.max_cycles=(m.bus.cycles()+500_000).min(deadline);let stop=m.run(u64::MAX);
        if m.exceptions!=0||m.bus.first_fault.is_some()||!matches!(stop,esp_soc::Stop::MaxInsns|esp_soc::Stop::Halted){return Err(format!("stop={stop:?} pc={:#x} fault={:?} UART={:?}",m.cores[0].pc,m.bus.first_fault,String::from_utf8_lossy(&m.console.uart0)));}
    }
    if !m.console.uart0.ends_with(marker){return Err(format!("guest timeout pc={:#x} UART={:?}; {}",m.cores[0].pc,String::from_utf8_lossy(&m.console.uart0),m.bus.report()));}Ok(())
}
fn frame(m:&mut esp32c5::Machine,dir:&Path,name:&str)->Result<Vec<u16>,Box<dyn std::error::Error>>{
    m.bus.refresh_board_outputs();let(w,h,p,_) =m.bus.board.display().ok_or("missing display")?;
    if(w,h)!=(172,320){return Err("unexpected geometry".into());}
    fs::write(dir.join(format!("{name}.rgb565le")),p.iter().flat_map(|v|v.to_le_bytes()).collect::<Vec<_>>())?;
    m.write_tft_png(dir.join(format!("{name}.png")).to_str().ok_or("output path")?,1)?;Ok(p)
}
fn main()->Result<(),Box<dyn std::error::Error>>{
    let args:Vec<_>=std::env::args().collect();let app=args.get(1).ok_or("usage: lcd147_probe app.bin output-dir")?;let dir=Path::new(args.get(2).ok_or("output dir")?);fs::create_dir_all(dir)?;
    let mut m=esp32c5::machine([0;6],4<<20);m.bus.set_xtal_mhz(48)?;m.bus.attach_board(esp32c5::board::make_board("waveshare-c5-lcd147").ok_or("board")?);
    let original:Vec<u8>=(0..8<<20).map(|i|((i*31)^(i>>9)^0x37) as u8).collect();
    let card=Arc::new(Mutex::new(original.clone()));
    m.console.capture=true;m.console.mask=2;m.bus.attach_sd_card(Box::new(ObservedCard(card.clone())),Default::default())?;
    m.load_input(esp_soc::LoadKind::App,&fs::read(app)?)?;m.boot_app(0x10000)?;
    until(&mut m,b"C5 LCD147 READY SD PASS\n")?;let ready_cycles=m.bus.cycles();let base=frame(&mut m,dir,"ready")?;
    let mut palette=std::collections::HashSet::new();for &p in &base{palette.insert(p);}if palette.len()<100{return Err("diagnostic framebuffer is not a meaningful color pattern".into());}
    m.max_cycles=m.bus.cycles()+1_000_000;m.run(u64::MAX);let quiet=frame(&mut m,dir,"quiet")?;
    let quiet_changed=base.iter().zip(&quiet).filter(|(a,b)|a!=b).count();if quiet_changed!=0{return Err("quiet framebuffer drift".into());}
    m.load_script(&format!("{:.12} gpio boot 0\n",m.seconds()))?;until(&mut m,b"C5 BOOT DOWN\n")?;let down=frame(&mut m,dir,"pressed")?;
    let changed=base.iter().zip(&down).filter(|(a,b)|a!=b).count();if changed!=140*256{return Err(format!("BOOT change={changed}, expected140x256").into());}
    m.load_script(&format!("{:.12} gpio boot 1\n",m.seconds()))?;until(&mut m,b"C5 BOOT UP\n")?;let released=frame(&mut m,dir,"released")?;
    if released!=base||m.exceptions!=0||m.reboots!=0{return Err("release or execution mismatch".into());}
    if !m.bus.board.report().contains("read-blocks=3 written-blocks=2") || !m.console.uart0.windows(30).any(|w|w==b"C5 SD ORIGINAL BLOCK RESTORED\n") {return Err("actual SD card did not save, test and restore its original block".into());}
    if *card.lock().unwrap()!=original{return Err("diagnostic changed original SD image bytes".into());}
    fs::write(dir.join("uart.log"),&m.console.uart0)?;fs::write(dir.join("board-report.txt"),m.bus.report()+"\n")?;
    let json=format!("{{\"kind\":\"original-c5-lcd147-sram-guest\",\"factoryVerified\":false,\"flashBytes\":4194304,\"psramBytes\":0,\"nominalCrystalMHz\":48,\"spiMHz\":24,\"width\":172,\"height\":320,\"readyCycles\":{ready_cycles},\"cycles\":{},\"retired\":{},\"dispatched\":{},\"exceptions\":{},\"resets\":{},\"colorCount\":{},\"quietChangedPixels\":{quiet_changed},\"bootChangedPixels\":{changed},\"releaseRestoredExactly\":true,\"sdGuestLba\":32,\"sdOriginalImageRestoredExactly\":true,\"sdReadBlocks\":3,\"sdWrittenBlocks\":2,\"sdBytesWrittenAndRead\":512,\"sdFilesystemMounted\":false,\"ramwr\":{}}}\n",m.bus.cycles(),m.cores[0].retired_count,m.insns(),m.exceptions,m.reboots,palette.len(),m.bus.board.display_frames());
    fs::write(dir.join("execution.json"),&json)?;print!("{json}");eprint!("{}",String::from_utf8_lossy(&m.console.uart0));Ok(())
}
