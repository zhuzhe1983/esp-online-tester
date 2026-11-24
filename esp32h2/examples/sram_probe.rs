//! Auditable source fixture and original-image rejection probe. Not factory firmware.
#[path = "../tests/common/mod.rs"] mod common;
use emu_core::Bus;
use esp_soc::SocBus;
fn quote(s:&str)->String{format!("\"{}\"",esp_soc::web::json_escape(s))}
fn run(data:&[u8],name:&str)->Result<(),Box<dyn std::error::Error>>{
 let mut m=esp32h2::machine([0;6],4<<20);m.bus.attach_board(esp32h2::board::make_board(name).ok_or("unknown H2 board")?);m.bus.write_flash(0,data)?;let entry=m.boot_app(0)?;
 let stop=m.run_until_cycle(10_000);if !matches!(stop,esp_soc::RunUntil::Reached){return Err(format!("unexpected stop:{stop:?};{:?}",m.bus.first_fault).into());}
 let uart=m.bus.console_take();let uart=String::from_utf8(uart[1].clone())?;let (led,generation)=m.bus.board.leds().ok_or("missing RGB")?;let rgb=led[0];
 if uart!="H2 SRAM chip16 RGB8\n"||rgb!=[0x34,0x12,0x56]||generation!=1||m.bus.first_fault.is_some(){return Err("fixture UART/RGB/no-fault expectation failed".into());}
 let boot=m.bus.read32(0x40804000).map_err(|e|format!("BOOT read:{e:?}"))?&(1<<9)!=0;let lp=m.bus.read32(0x50000000).map_err(|e|format!("LP read:{e:?}"))?;
 println!("{{\"board\":{},\"chip\":\"esp32h2\",\"mode\":\"sram-app-direct\",\"entry\":{},\"cycles\":{},\"instructions\":{},\"exceptions\":{},\"uart0\":{},\"rgb\":[{},{},{}],\"rgbFrames\":{},\"bootReleased\":{},\"lpMarker\":{},\"originalManufacturerFirmware\":false,\"result\":\"pass\"}}",quote(name),entry,m.bus.cycles,m.insns(),m.exceptions,quote(&uart),rgb[0],rgb[1],rgb[2],generation,boot,lp);
 Ok(())
}
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args:Vec<_>=std::env::args().skip(1).collect();match args.first().map(String::as_str){
 Some("--fixture") if args.len()==2=>{let data=common::fixture(8);std::fs::write(&args[1],&data)?;for n in["waveshare-h2-zero","waveshare-h2-dev-kit-n4"]{run(&data,n)?;}},
 Some("--run") if args.len()==3=>run(&std::fs::read(&args[1])?,&args[2])?,
 Some("--check-image") if args.len()==3=>{let data=std::fs::read(&args[1])?;let offset:usize=args[2].parse()?;let selected=data.get(offset..).ok_or("image offset exceeds file")?;match esp32h2::image::parse(selected){Ok(i)=>println!("{{\"accepted\":true,\"offset\":{offset},\"entry\":{},\"segments\":{}}}",i.entry,i.segments.len()),Err(e)=>println!("{{\"accepted\":false,\"offset\":{offset},\"reason\":{}}}",quote(&e))}},
 _=>return Err("usage:sram_probe --fixture output.bin | --run app.bin board | --check-image original.bin decimal-offset".into())
 }Ok(())
}
