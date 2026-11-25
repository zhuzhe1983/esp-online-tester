//! Original LCD273 Sensor probe: caller inputs, real GPIO, no environment changes.
use esp_soc::{RunUntil,SocBus};
use std::{fs,path::PathBuf};
fn internal(bus:&esp32c5::C5Bus,address:u32,len:usize)->Option<&[u8]>{let i=address.checked_sub(0x4080_0000)? as usize;bus.sram.get(i..i.checked_add(len)?)}
struct SensorCalls{output:Option<(u32,u32)>}
impl esp_soc::observe::Observer<esp32c5::C5>for SensorCalls{
 fn name(&self)->&'static str{"sensor-call-observation"}
 fn wants(&self)->esp_soc::observe::Wants{esp_soc::observe::Wants::INSN}
 fn on_insn(&mut self,_:&esp_soc::observe::Ctx,_:usize,cpu:&riscv_rv32::Cpu,bus:&mut esp32c5::C5Bus,pc:u32)->Option<esp_soc::Stop>{
  match pc{
   0x4201_89e4=>{self.output=Some((cpu.x[12],cpu.x[13]));println!("sensor_guest cycle={} pc={pc:#010x} event=GET_TH_ENTRY sp={:#x} handle={:#x} command={:#x} temp_out={:#x} humidity_out={:#x}",bus.cycles,cpu.x[2],cpu.x[10],cpu.x[11],cpu.x[12],cpu.x[13]);},
   0x4201_88c4=>println!("sensor_guest cycle={} pc={pc:#010x} event=WAKE_TX_RETURN esp_err={:#x}",bus.cycles,cpu.x[10]),
   0x4201_8a1c|0x4201_8a2e|0x4201_8a38|0x4201_8a4a|0x4201_8b56=>println!("sensor_guest cycle={} pc={pc:#010x} event=CALL_RETURN a0={:#x} sp={:#x} buffer={:02x?}",bus.cycles,cpu.x[10],cpu.x[2],internal(bus,cpu.x[2].saturating_add(8),6)),
   0x4211_e748=>println!("sensor_guest cycle={} pc={pc:#010x} event=RECEIVE_ENTRY handle={:#x} buffer_ptr={:#x} len={} timeout={} initial={:02x?}",bus.cycles,cpu.x[10],cpu.x[11],cpu.x[12],cpu.x[13],internal(bus,cpu.x[11],cpu.x[12].min(32)as usize)),
   0x4200_dc46=>{if let Some((t,h))=self.output{let f=|a|internal(bus,a,4).map(|b|f32::from_le_bytes(b.try_into().unwrap()));println!("sensor_guest cycle={} pc={pc:#010x} event=GET_TH_CALLER_RETURN ret={:#x} temperature={:?} humidity={:?}",bus.cycles,cpu.x[10],f(t),f(h));}},
   _=>{},
  }None
 }
}
fn advance(m:&mut esp32c5::Machine,label:&str,cycles:u64)->bool{
 let target=m.bus.cycles().saturating_add(cycles);
 loop{match m.run_until_cycle(target){
  RunUntil::Reached=>{println!("stage={label} reached cycles={} insns={} pc={:#010x}",m.bus.cycles(),m.insns(),m.cores[0].pc);return true;},
  RunUntil::Yield=>continue,
  RunUntil::Stop(stop)=>{println!("stage={label} stop={stop:?} cycles={} insns={} pc={:#010x}",m.bus.cycles(),m.insns(),m.cores[0].pc);return false;},
 }}
}
fn pulse(m:&mut esp32c5::Machine,name:&str,pin:u8)->bool{
 m.bus.gpio_set_input(pin,false);if !advance(m,&format!("{name}-down"),31_200_000){return false;}
 m.bus.gpio_set_input(pin,true);advance(m,&format!("{name}-up"),50_400_000)
}
fn main()->Result<(),Box<dyn std::error::Error>>{
 let a=std::env::args().collect::<Vec<_>>();if a.len()!=5{return Err("usage: factory_sensor_boundary rom.elf original-factory.bin sd.img fresh-output-dir (explicit offline driver profile)".into());}
 let rom=fs::read(&a[1])?;let fw=fs::read(&a[2])?;let sd=fs::read(&a[3])?;let out=PathBuf::from(&a[4]);fs::create_dir(&out)?;
 let mut m=esp32c5::rom_machine([0;6],32<<20);m.bus.set_xtal_mhz(48)?;m.bus.configure_psram(8<<20)?;m.bus.attach_board(esp32c5::board::make_board("waveshare-c5-lcd273").ok_or("missing board")?);
 m.bus.attach_sd_card(Box::new(esp_periph::sdmmc::MemStore(sd)),Default::default())?;m.load_rom(&rom)?;m.load_input(esp_soc::LoadKind::Flash,&fw)?;
 esp32c5::enable_wifi_driver_compat(&mut m,"waveshare-lcd273-idf553-offline-v1")?;
 m.add_observer(Box::new(SensorCalls{output:None}));
 m.console.capture=true;m.console.mask=0;m.boot_rom();
 if advance(&mut m,"menu",500_000_000)&&pulse(&mut m,"next-sensor",3)&&pulse(&mut m,"boot",28){advance(&mut m,"sensor-read",156_000_000);}
 m.drain_console();let cpu=&m.cores[0];
 println!("first_fault={:?} mepc={:#010x} mcause={:#010x} mtval={:#010x} mstatus={:#010x} retired={} cpuSlots={} cycles={}",m.bus.first_fault,cpu.mepc,cpu.mcause,cpu.mtval,cpu.mstatus,cpu.retired_count,cpu.insn_count,cpu.cycle_count);
 for(i,value)in cpu.x.iter().enumerate(){println!("x{i}={value:#010x}");}
 if let Some(diag)=m.bus.board.hardware_diagnostics(){fs::write(out.join("hardware.json"),diag)?;}
 if let Some(state)=m.bus.wifi_driver_compat_snapshot(){fs::write(out.join("driver-compatibility.json"),state)?;}
 let mut uart=m.console.uart0.clone();uart.extend_from_slice(&m.bus.uart[0].tx_out);fs::write(out.join("uart0.txt"),&uart)?;
 if let Some((width,height,pixels,_))=m.bus.board.display(){let raw=pixels.iter().flat_map(|p|p.to_le_bytes()).collect::<Vec<_>>();fs::write(out.join("frame.rgb565"),raw)?;println!("frame width={width} height={height} nonzero={}",pixels.iter().filter(|&&p|p!=0).count());}
 println!("uartBytes={} exceptions={} interrupts={} board={}",uart.len(),m.exceptions,m.interrupts,m.bus.report());Ok(())
}
