//! Explicit user-supplied firmware and ROM; temporary native bring-up probe.
use esp_soc::SocBus;
fn main() {
 let args:Vec<_>=std::env::args().collect();assert_eq!(args.len(),3,"nanoc6_probe <firmware.bin> <c6-rom.elf>");
 let mut m=esp32c6::machine([0;6],4<<20);m.bus.board=esp32c6::board::make_board("nanoc6").unwrap();m.bus.attach_board_devices();m.console.capture=true;m.console.mask=0;
 m.load_rom(&std::fs::read(&args[2]).unwrap()).unwrap();m.write_flash(0,&std::fs::read(&args[1]).unwrap()).unwrap();m.boot_rom();
 for sec in 1..=30 {if sec>=5 {m.bus.gpio_set_input(9,sec%2==0);} m.max_cycles=sec*160_000_000;let stop=m.run(u64::MAX);println!("sec={sec} stop={stop:?} pc={:#x} leds={:?} report={}",m.cores[0].pc,m.bus.board.leds(),m.bus.board.report());}
 println!("console={}",String::from_utf8_lossy(&m.console.all));
}
