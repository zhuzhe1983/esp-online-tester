//! Native ROM boundary probe. Raw image placement is explicit and does not
//! bypass or change the public SRAM-only loader's factory rejection policy.
use esp_soc::SocBus;
fn main()->Result<(),Box<dyn std::error::Error>>{
    let a:Vec<_>=std::env::args().collect();
    if !matches!(a.len(),7|8){return Err("usage: factory_boundary board flash-MiB psram-MiB rom.elf padded-factory.bin budget [sd.img]".into());}
    let flash=a[2].parse::<usize>()?.checked_mul(1<<20).ok_or("capacity overflow")?;let psram=a[3].parse::<usize>()?.checked_mul(1<<20).ok_or("capacity overflow")?;
    if !matches!(flash,0x400000|0x800000|0x1000000|0x2000000){return Err("unsupported flash capacity".into());}
    let bytes=std::fs::read(&a[5])?;if bytes.len()>flash{return Err("image exceeds physical flash".into());}
    let mut m=esp32c5::Bringup::new();m.bus.set_xtal_mhz(48)?;m.bus.configure_psram(psram)?;m.bus.attach_board(esp32c5::board::make_board(&a[1]).ok_or("board")?);
    // Test a memory copy: guest writes never modify the caller's source IMG.
    if let Some(p)=a.get(7){m.bus.attach_sd_card(Box::new(esp_periph::sdmmc::MemStore(std::fs::read(p)?)),Default::default())?;}
    m.bus.flash=vec![255;flash];m.bus.flash[..bytes.len()].copy_from_slice(&bytes);m.load_rom_elf(&std::fs::read(&a[4])?)?;
    let run=m.run(a[6].parse()?);let p=m.bus.board.display();let nonzero=p.as_ref().map_or(0,|(_,_,v,_)|v.iter().filter(|&&n|n!=0).count());
    println!("board={} flashBytes={} psramBytes={} imageBytes={} loadOffset=0 rawNativePlacement=true publicLoaderAcceptance=false",a[1],flash,psram,bytes.len());
    println!("steps={} stop={:?} pc={:#010x} firstFault={:?} uartBytes={} nonzeroPixels={}",run.steps,run.stop,m.cpu.pc,m.bus.first_fault,m.bus.uart[0].tx_out.len(),nonzero);
    println!("UART={:?}\n{}",String::from_utf8_lossy(&m.bus.uart[0].tx_out),m.bus.report());Ok(())
}
