//! Caller-supplied Atom Voice ROM/full image diagnostic; no binary is redistributed.
use esp_periph::Device;
use xtensa_lx7::Bus;
fn main() {
    let a:Vec<_>=std::env::args().collect();assert_eq!(a.len(),3,"atom_voice_probe <firmware.bin> <rom.elf>");
    let mut m=esp32::machine([0x24,0x6f,0x28,0,0x11,0x22],4<<20);
    m.bus.board=esp32::board::make_board("atom-voice").unwrap();m.bus.attach_board_devices();
    m.web=Some(esp_soc::web::WebServer::queued());m.rt.enabled=false;m.console.capture=true;m.console.mask=2;
    if std::env::var_os("ATOM_PROBE_WASM_SCHED").is_some() {m.quantum=std::env::var("ATOM_PROBE_Q").ok().and_then(|x|x.parse().ok()).unwrap_or(256);m.vq_max=std::env::var("ATOM_PROBE_VQ").ok().and_then(|x|x.parse().ok()).unwrap_or(1024);m.bb_max=128;}
    m.load_rom(&std::fs::read(&a[2]).unwrap()).unwrap();m.write_flash(0,&std::fs::read(&a[1]).unwrap()).unwrap();m.boot_rom();
    m.add_observer(Box::new(esp_soc::observers::BlockProfile::new(12)));
    for i in 1..=20 {
        m.max_cycles=i*4_800_000;let stop=m.run(u64::MAX);
        if i%10==0||!matches!(stop,esp32::Stop::Halted) {println!("slice={i} stop={stop:?} pc={:x}/{:x} leds={:?} frc={:?} lines={:x}/{:x}",m.cores[0].pc,m.cores[1].pc,m.bus.board.leds(),[0x20,0x24,0x28,0x30].map(|o|m.bus.periph.frc2.read(o)),m.bus.periph.cpu_lines(0),m.bus.periph.cpu_lines(1));}
        if !matches!(stop,esp32::Stop::Halted) {break;}
    }
    println!("regs a2={} a0={} wb={}/{} literals={:?}",m.cores[0].get_ar(2),m.cores[1].get_ar(0),m.cores[0].windowbase,m.cores[1].windowbase,[0x400805a8,0x400805ac,0x400805b4].map(|a|m.bus.read32(a)));
    println!("{}",String::from_utf8_lossy(&m.console.all));println!("{}",m.reports());
}
