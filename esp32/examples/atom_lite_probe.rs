//! Explicit local firmware probe; no fixture body or private device data is embedded.
use esp_soc::SocBus;
use esp_periph::Device;
fn main() {
    let args:Vec<_>=std::env::args().collect();assert_eq!(args.len(),3,"atom_lite_probe <firmware.bin> <classic-rom.elf>");
    let mut m=esp32::machine([0;6],4<<20);m.bus.board=esp32::board::make_board("atom-lite").unwrap();m.bus.attach_board_devices();
    m.load_rom(&std::fs::read(&args[2]).unwrap()).unwrap();m.write_flash(0,&std::fs::read(&args[1]).unwrap()).unwrap();m.console.capture=true;m.console.mask=0;m.boot_rom();m.bus.gpio_events=Some(Vec::new());
    for sec in 1..=10 {
        if sec>=3 {m.bus.gpio_set_input(39,sec%2==0);}
        m.max_cycles=sec*240_000_000;let stop=m.run(u64::MAX);
        let events=m.bus.gpio_events.as_mut().unwrap();let g27:Vec<_>=events.iter().copied().filter(|x|x.1==27).collect();let n=g27.len();let first:Vec<_>=g27.into_iter().take(12).collect();events.clear();
        let regs=[0x20,0x24,0xa0,0xa4,0xa8,0xd0].map(|x|m.bus.periph.rmt.read(x));
        println!("sec={sec} stop={stop:?} pc0={:#x} pc1={:#x} g39={} edges27={n} first={first:?} leds={:?} rmt={} regs={:?}",m.cores[0].pc,m.cores[1].pc,m.bus.gpio_input()>>39&1,m.bus.board.leds(),m.bus.periph.rmt.tx_count,regs);
    }
    println!("console bytes={} tail={}",m.console.all.len(),String::from_utf8_lossy(&m.console.all[m.console.all.len().saturating_sub(240)..]));
}
