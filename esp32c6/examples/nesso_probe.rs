//! Explicit user-supplied firmware and ROM; temporary native bring-up probe.
use esp_soc::SocBus;
fn main() {
 let args:Vec<_>=std::env::args().collect();assert_eq!(args.len(),3,"nesso_probe <firmware.bin> <c6-rom.elf>");
 let mut m=esp32c6::machine([0;6],16<<20);m.bus.board=esp32c6::board::make_board("nesso-n1").unwrap();m.bus.attach_board_devices();m.bus.set_flash_size(16<<20);m.console.capture=true;m.console.mask=0;
 m.load_rom(&std::fs::read(&args[2]).unwrap()).unwrap();m.write_flash(0,&std::fs::read(&args[1]).unwrap()).unwrap();m.boot_rom();
 for step in 1..=90 {
  match step {30=>{m.bus.board.button_at(m.bus.cycles(),"btnB",true);},33=>{m.bus.board.button_at(m.bus.cycles(),"btnB",false);},60=>{m.bus.board.button_at(m.bus.cycles(),"btnA",true);},63=>{m.bus.board.button_at(m.bus.cycles(),"btnA",false);},_=>{}}
  m.max_cycles=step*16_000_000;let stop=m.run(u64::MAX);
  if step%10==0 {println!("step={step} stop={stop:?} pc={:#x} report={}",m.cores[0].pc,m.bus.board.report());}
  if [20,50,80].contains(&step) {if let Some((w,h,p,_))=m.bus.board.display() {let mut out=format!("P6\n{w} {h}\n255\n").into_bytes();for v in p {out.extend_from_slice(&[((v>>11)*255/31) as u8,(((v>>5)&63)*255/63) as u8,((v&31)*255/31) as u8]);}std::fs::write(format!("output/nesso-frame-{step}.ppm"),out).unwrap();}}
 }
 if let Some((w,h,p,_))=m.bus.board.display() {let mut out=format!("P6\n{w} {h}\n255\n").into_bytes();for v in p {out.extend_from_slice(&[((v>>11)*255/31) as u8,(((v>>5)&63)*255/63) as u8,((v&31)*255/31) as u8]);}std::fs::write("output/nesso-frame.ppm",out).unwrap();}println!("console={}",String::from_utf8_lossy(&m.console.all));
}
