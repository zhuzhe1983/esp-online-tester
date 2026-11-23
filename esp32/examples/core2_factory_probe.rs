//! Local probe for an unchanged, explicitly supplied Core2 factory image and ROM ELF.
use esp_soc::{SocBus,Stop};

fn main() {
    let args:Vec<_>=std::env::args().collect();
    assert!((3..=4).contains(&args.len()),"core2_factory_probe <firmware.bin> <esp32_rev300_rom.elf> [board]");
    let board=args.get(3).map_or("core2",String::as_str);
    assert!(board.starts_with("core2"),"probe requires a Core2 board");
    let mut machine=esp32::machine([0;6],16<<20);
    machine.bus.board=esp32::board::make_board(board).expect("known Core2 board");
    machine.bus.attach_board_devices();
    machine.bus.set_flash_size(16<<20);
    machine.bus.set_psram_size(4<<20).unwrap();
    if let Some(path)=std::env::var_os("CORE2_PROBE_SD_IMAGE") {
        let bytes=std::fs::read(path).expect("prepared SD image");
        machine.bus.attach_sd_card(Box::new(esp_periph::sdmmc::MemStore(bytes)),Default::default()).unwrap();
    }
    machine.load_rom(&std::fs::read(&args[2]).expect("ROM ELF")).unwrap();
    machine.write_flash(0,&std::fs::read(&args[1]).expect("merged firmware image")).unwrap();
    machine.console.mask=0;
    machine.console.capture=true;
    machine.boot_rom();
    let image_dir=std::env::var_os("CORE2_PROBE_PPM_DIR").map(std::path::PathBuf::from);
    for step in 1..=18 {
        if step==12 {machine.bus.touch_input(160,120,true);}
        if step==13 {machine.bus.touch_input(160,120,false);}
        if step==16 {let cycle=machine.bus.cycles();machine.bus.board.button_at(cycle,"button-a",true);}
        if step==17 {let cycle=machine.bus.cycles();machine.bus.board.button_at(cycle,"button-a",false);}
        machine.max_cycles=step*<esp32::Esp32 as esp_soc::Soc>::CPU_HZ/2;
        let stop=machine.run(u64::MAX);
        let frame=machine.bus.board.display().map(|(w,h,pixels,version)|{
            if let Some(path)=&image_dir {
                let mut bytes=format!("P6\n{w} {h}\n255\n").into_bytes();
                for &pixel in &pixels {
                    let r=((pixel>>11)&31) as u8;
                    let g=((pixel>>5)&63) as u8;
                    let b=(pixel&31) as u8;
                    bytes.extend([(r<<3)|(r>>2),(g<<2)|(g>>4),(b<<3)|(b>>2)]);
                }
                std::fs::write(path.join(format!("core2-{step:02}.ppm")),bytes).unwrap();
            }
            let nonzero=pixels.iter().filter(|&&p|p!=0).count();
            let fingerprint=pixels.iter().fold(0xcbf29ce484222325u64,|hash,p|(hash^u64::from(*p)).wrapping_mul(0x100000001b3));
            (w,h,version,nonzero,fingerprint)
        });
        println!("sec={} stop={stop:?} insns={} frame={frame:?} {}",machine.seconds(),machine.insns(),machine.bus.board.report());
        if !matches!(stop,Stop::Halted) {break;}
    }
    println!("UART0:\n{}",String::from_utf8_lossy(&machine.console.uart0));
}
