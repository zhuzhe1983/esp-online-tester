//! Native, explicitly assisted original LCD273 Photo repro. Inputs are caller
//! supplied and used read-only; GPIO transitions are consumed by real firmware.
use esp_soc::{RunUntil,SocBus};
use emu_core::Bus;
use std::{fs,path::PathBuf};
struct FirstPhotoLoad{header:bool,load:bool}
fn header_input(cpu:&riscv_rv32::Cpu,bus:&esp32c5::C5Bus,io:u32){
 // Fixed esp_new_jpeg1.0.2 jpeg_dec_io_t, RV32: five32-bit words. Inspect
 // already-written internal SRAM directly; this is not another CPU load.
 let span=io.checked_sub(0x4080_0000).and_then(|p|bus.sram.get(p as usize..(p as usize).checked_add(20)?));
 if let Some(span)=span{let base=u32::from_le_bytes(span[..4].try_into().unwrap());let len=u32::from_le_bytes(span[4..8].try_into().unwrap());
  println!("jpeg_input pc={:#010x} io={io:#010x} base={base:#010x} len={len} backing={:?}",cpu.pc,bus.mapped_backing_debug(base,len as usize));
 }
}
impl esp_soc::observe::Observer<esp32c5::C5>for FirstPhotoLoad{
 fn name(&self)->&'static str{"photo-first-header-load"}
 fn wants(&self)->esp_soc::observe::Wants{esp_soc::observe::Wants::INSN}
 fn on_insn(&mut self,_:&esp_soc::observe::Ctx,_:usize,cpu:&riscv_rv32::Cpu,bus:&mut esp32c5::C5Bus,pc:u32)->Option<esp_soc::Stop>{
  if !self.header&&pc==0x4201_eb40{self.header=true;header_input(cpu,bus,cpu.x[11]);}
  if !self.load&&pc==0x4201_ed26&&cpu.x[14]&1!=0{self.load=true;let address=cpu.x[14].wrapping_add(2);println!("first_photo_odd_lhu pc={pc:#010x} a4={:#010x} address={address:#010x} width=2 {} neighbor={:?}",cpu.x[14],bus.mapped_memory_debug(address),address.checked_sub(8).and_then(|a|bus.mapped_backing_debug(a,16)));header_input(cpu,bus,cpu.x[20]);}
  None
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
 let a=std::env::args().collect::<Vec<_>>();if a.len()!=5{return Err("usage: factory_photo_boundary rom.elf original-factory.bin sd.img fresh-output-dir (explicit offline driver profile)".into());}
 let rom=fs::read(&a[1])?;let fw=fs::read(&a[2])?;let sd=fs::read(&a[3])?;let out=PathBuf::from(&a[4]);fs::create_dir(&out)?;
 let mut m=esp32c5::rom_machine([0;6],32<<20);m.bus.set_xtal_mhz(48)?;m.bus.configure_psram(8<<20)?;m.bus.attach_board(esp32c5::board::make_board("waveshare-c5-lcd273").ok_or("missing board")?);
 m.bus.attach_sd_card(Box::new(esp_periph::sdmmc::MemStore(sd)),Default::default())?;m.load_rom(&rom)?;m.load_input(esp_soc::LoadKind::Flash,&fw)?;
 esp32c5::enable_wifi_driver_compat(&mut m,"waveshare-lcd273-idf553-offline-v1")?;
 m.add_observer(Box::new(FirstPhotoLoad{header:false,load:false}));
 m.console.capture=true;m.console.mask=0;m.boot_rom();
 if advance(&mut m,"menu",500_000_000)&&pulse(&mut m,"next-one",3)&&pulse(&mut m,"next-two",3)&&pulse(&mut m,"boot",28){advance(&mut m,"photo-decode",240_000_000);}
 m.drain_console();let cpu=&m.cores[0];
 println!("first_fault={:?} mepc={:#010x} mcause={:#010x} mtval={:#010x} mstatus={:#010x} retired={} cpuSlots={} cycles={}",m.bus.first_fault,cpu.mepc,cpu.mcause,cpu.mtval,cpu.mstatus,cpu.retired_count,cpu.insn_count,cpu.cycle_count);
 for(i,value)in cpu.x.iter().enumerate(){println!("x{i}={value:#010x}");}
 if let Some(fault)=m.bus.first_fault.clone(){println!("fault_opcode={:?}",m.bus.fetch(fault.pc));println!("fault_mapping={}",m.bus.mapped_memory_debug(fault.address));}
 println!("photo_buffer_mapping={}",m.bus.mapped_memory_debug(0x4231_810b));println!("psram={}",m.bus.psram_command_debug());
 println!("spi={}",m.bus.spi_transport_debug());println!("board={}",m.bus.report());
 if let Some(diag)=m.bus.board.hardware_diagnostics(){fs::write(out.join("hardware.json"),diag)?;}
 if let Some(state)=m.bus.wifi_driver_compat_snapshot(){fs::write(out.join("driver-compatibility.json"),state)?;}
 let mut uart=m.console.uart0.clone();uart.extend_from_slice(&m.bus.uart[0].tx_out);fs::write(out.join("uart0.txt"),&uart)?;
 if let Some((width,height,pixels,_))=m.bus.board.display(){let raw=pixels.iter().flat_map(|p|p.to_le_bytes()).collect::<Vec<_>>();fs::write(out.join("frame.rgb565"),raw)?;println!("frame width={width} height={height} nonzero={}",pixels.iter().filter(|&&p|p!=0).count());}
 println!("uartBytes={} exceptions={} interrupts={}",uart.len(),m.exceptions,m.interrupts);Ok(())
}
