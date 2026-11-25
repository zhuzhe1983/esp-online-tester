//! LP_AON GPIO mux disabled-LP subset, not an LP GPIO/pad engine.
use crate::C5Bus;
use emu_core::{Bus,Fault};
use esp_soc::SocBus;
const MUX:u32=0x600b_1028;const RAM:u32=crate::bus::SRAM_BASE;
fn selected()->C5Bus{let mut b=C5Bus::new();b.select_validated_rom_profile().unwrap();b.rom_boot=true;b}
#[test]fn all_hp_mux_por_is_readable_and_repeated_full_word_clears_are_inert(){
 let mut b=selected();assert_eq!(b.read32(MUX),Ok(0));
 for pin in 0..8{let v=b.read32(MUX).unwrap()&!(1<<pin);b.write32(MUX,v).unwrap();assert_eq!(b.read32(MUX),Ok(0));}
 for bit in 0..32{assert_eq!(b.write32(MUX,1<<bit),Err(Fault::Unsupported));assert_eq!(b.read32(MUX),Ok(0));}
 assert_eq!(b.write8(MUX,0),Err(Fault::Unsupported));assert_eq!(b.write16(MUX,0),Err(Fault::Unsupported));assert_eq!(b.read32(MUX),Ok(0));
 assert_eq!(C5Bus::new().read32(MUX),Err(Fault::Unmapped));assert_eq!(C5Bus::new().write32(MUX,0),Err(Fault::Unmapped));
}
struct UnitCost;
impl emu_core::CostModel for UnitCost{
 fn lifecycle(&mut self,_:&emu_core::LifecycleFacts)->Result<(),String>{Ok(())}
 fn cycles(&mut self,_:&emu_core::ExecutionFacts<'_>)->Result<u32,String>{Ok(1)}
}
#[test]fn actual_digital_select_rmw_runs_and_nonzero_lp_requests_keep_guest_exception_state(){
 for priced in[false,true]{
  // lw a1,0(t0); andi a1,a1,-5 (RTC index2); sw a1,0(t0); sw a2,0(t0).
  let mut m=crate::rom_machine([0;6],4<<20);m.quantum=1;m.bus.select_validated_rom_profile().unwrap();m.bus.rom_boot=true;
  m.cores[0].enable_c5_memory_protection().unwrap();let words=[0x0002_a583u32,0xffb5_f593,0x00b2_a023,0x00c2_a023];
  let bytes=words.iter().flat_map(|w|w.to_le_bytes()).collect::<Vec<_>>();m.bus.load_ram(RAM,&bytes,bytes.len()).unwrap();
  m.cores[0].pc=RAM;m.cores[0].x[5]=MUX;m.cores[0].x[12]=4;
  if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
  for _ in 0..3{assert!(matches!(m.run(1),esp_soc::Stop::MaxInsns));}
  assert_eq!(m.cores[0].x[11],0);assert_eq!(m.bus.read32(MUX),Ok(0));assert_eq!(m.exceptions,0);
  m.cores[0].mepc=17;m.cores[0].mcause=19;m.cores[0].mtval=23;
  assert!(matches!(m.run(1),esp_soc::Stop::ModelLimit{core:0,pc}if pc==RAM+12));
  assert_eq!((m.cores[0].mepc,m.cores[0].mcause,m.cores[0].mtval),(17,19,23));assert_eq!(m.exceptions,0);
 }
}
#[test]fn lp_mux_access_never_crosses_an_older_unapplied_watchdog_reset(){
 for write in[false,true]{let mut b=selected();b.cycles=130_000_000;
  let result=if write{b.write32(MUX,0)}else{b.read32(MUX).map(|_|())};assert_eq!(result,Err(Fault::Unsupported));
  assert!(b.sw_reset());assert_eq!(b.reset_cause(),7);assert_eq!(b.watchdog_reset_at,Some(130_000_000));
 }
}
