//! Bounded original-image observation, not a compatibility assertion.
//! Usage: board rom.elf original.bin flashMiB psramMiB guestMs outputDir [sd.img]
use esp_soc::{LoadKind,SocBus};
use std::{collections::BTreeSet,path::PathBuf};
fn main(){
 let a=std::env::args().skip(1).collect::<Vec<_>>();assert!((7..=8).contains(&a.len()),"board rom.elf original.bin flashMiB psramMiB guestMs outputDir [sd.img]");
 let flash=a[3].parse::<usize>().expect("flashMiB");let psram=a[4].parse::<usize>().expect("psramMiB");let ms=a[5].parse::<u64>().expect("guestMs");assert!(ms>0&&ms<=30_000);let out=PathBuf::from(&a[6]);std::fs::create_dir_all(&out).unwrap();
 let mut m=esp32s3::machine([2,0,0,0,0,1]);m.bus.board=esp32s3::board::make_board(&a[0]).expect("registered exact board");m.bus.attach_board_devices();m.bus.set_flash_size(flash<<20);m.bus.set_psram_size(psram<<20).unwrap();m.console.capture=true;m.console.mask=2;
 m.load_input(LoadKind::Rom,&std::fs::read(&a[1]).unwrap()).unwrap();m.load_input(LoadKind::Flash,&std::fs::read(&a[2]).unwrap()).unwrap();if a.len()==8{m.load_input(LoadKind::SdCard,&std::fs::read(&a[7]).unwrap()).unwrap();}m.boot_rom();m.max_cycles=ms*240_000;let stop=m.run(u64::MAX);m.bus.refresh_board_outputs();
 std::fs::write(out.join("uart.txt"),&m.console.all).unwrap();std::fs::write(out.join("board.txt"),m.bus.board.report()).unwrap();let(w,h,colors,version)=if let Some((w,h,p,v))=m.bus.board.display(){let count=p.iter().copied().collect::<BTreeSet<_>>().len();std::fs::write(out.join("frame.rgb565"),p.iter().flat_map(|x|x.to_le_bytes()).collect::<Vec<_>>()).unwrap();m.write_tft_png(out.join("frame.png").to_str().unwrap(),1).unwrap();(w,h,count,v)}else{(0,0,0,0)};
 let sd=&m.bus.periph.sdmmc.stats;let receipt=format!("{{\"board\":\"{}\",\"guestCycles\":{},\"guestMilliseconds\":{},\"stop\":\"{stop:?}\",\"reboots\":{},\"width\":{w},\"height\":{h},\"distinctColors\":{colors},\"version\":{version},\"frames\":{},\"pc\":[{},{}],\"sdCommands\":{},\"sdTimeouts\":{},\"sdBytesRead\":{},\"sdBytesWritten\":{},\"physicalValidated\":false,\"fullApplicationValidated\":false}}\n",a[0],m.bus.cycles,m.bus.cycles/240_000,m.reboots,m.bus.board.display_frames(),m.cores[0].pc,m.cores[1].pc,sd.commands,sd.timeouts,sd.bytes_read,sd.bytes_written);std::fs::write(out.join("state.json"),&receipt).unwrap();println!("{receipt}");
}
