//! Named LPCON request only: no COEX/RF clock or readiness is inferred.
use crate::{C5Bus,Machine};
use emu_core::{Bus,Fault};
use esp_soc::SocBus;
const GATE:u32=0x600a_f018;const FORCE:u32=GATE+4;const SELECT:u32=0x600a_9c04;const MASTER:u32=0x600a_f800;
const RAM:u32=crate::bus::SRAM_BASE;
fn selected()->C5Bus{let mut b=C5Bus::new();b.select_validated_rom_profile().unwrap();b.rom_boot=true;b}
struct UnitCost;
impl emu_core::CostModel for UnitCost{
    fn lifecycle(&mut self,_:&emu_core::LifecycleFacts)->Result<(),String>{Ok(())}
    fn cycles(&mut self,_:&emu_core::ExecutionFacts<'_>)->Result<u32,String>{Ok(1)}
}
fn guest(words:&[u32])->Machine{
    let mut m=crate::rom_machine([0;6],4<<20);m.quantum=1;m.bus.select_validated_rom_profile().unwrap();m.bus.rom_boot=true;
    let bytes=words.iter().flat_map(|w|w.to_le_bytes()).collect::<Vec<_>>();m.bus.load_ram(RAM,&bytes,bytes.len()).unwrap();
    m.cores[0].pc=RAM;m.cores[0].x[2]=RAM+0x50000;m.cores[0].x[5]=GATE;m
}
#[test]fn guest_coex_field_rmw_keeps_named_requests_and_does_not_select_a_source(){
    for priced in[false,true]{for base in[0,1,4,5]{
        // Actual field operation: load, clear bit1, OR the enable bit, store.
        let mut m=guest(&[0x0002_a583,0xffd5_f593,0x0025_e593,0x00b2_a023,0x0002_a503]);m.bus.write32(GATE,base).unwrap();
        let source=m.bus.read32(SELECT).unwrap();if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}for _ in 0..5{m.run(1);}
        assert_eq!(m.cores[0].pc,RAM+20);assert_eq!(m.cores[0].x[10],base|2);assert_eq!(m.bus.read32(GATE),Ok(base|2));assert_eq!(m.bus.read32(SELECT),Ok(source));
        let a=m.bus.boot_analog.as_ref().unwrap();assert!(a.coex_clock_requested());assert!(!a.pll_ready());assert!(!a.sar_reserved_path_ready());assert_eq!(a.bod_threshold(),None);assert_eq!(a.deadline(),None);
        assert!(m.bus.modem_lp_clock.is_none());assert_eq!(m.exceptions,0);assert!(m.bus.first_fault.is_none());
    }}
}
#[test]fn coex_and_wifipwr_requests_do_not_replace_i2c_enable_force_or_apb_permission(){
    let mut b=selected();b.write32(GATE,3).unwrap();assert!(b.read32(MASTER).is_err());assert_eq!(b.read32(SELECT),Ok(0x0020_0002));
    b.write32(GATE,7).unwrap();assert_eq!(b.read32(MASTER),Ok(0));b.write32(GATE,3).unwrap();assert!(b.read32(MASTER).is_err());
    b.write32(FORCE,4).unwrap();assert_eq!(b.read32(MASTER),Ok(0));
    // Apply ACTIVE while leaving MODEM_APB permission closed. FORCE2 cannot
    // replace APB accessibility even though the COEX request stays recorded.
    for(a,v)in[(0x600b_000c,0x8000_0000),(0x600a_9c0c,0),(0x600a_f020,0x4000_0000),(0x600b_00dc,0x8000_0000),(0x600b_00d0,0x1000_0000)]{b.write32(a,v).unwrap();}
    assert!(b.read32(MASTER).is_err());assert!(b.boot_analog.as_ref().unwrap().coex_clock_requested());
}
#[test]fn unsupported_gate_bits_and_partial_stores_preserve_real_guest_trap_state(){
    for priced in[false,true]{for(value,word)in[(8,0x00b2_a023),(u32::MAX,0x00b2_a023),(3,0x00b2_8023),(3,0x00b2_9023)]{
        let mut m=guest(&[word]);m.bus.write32(GATE,1).unwrap();m.cores[0].x[11]=value;m.cores[0].x[10]=0x55;
        m.cores[0].mepc=0x1234;m.cores[0].mcause=99;m.cores[0].mtval=88;let status=m.cores[0].mstatus;let before=m.bus.boot_analog.clone();
        m.dbg.stop_on_unimplemented=false;if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        assert!(matches!(m.run(1),esp_soc::Stop::ModelLimit{core:0,pc}if pc==RAM));assert_eq!(m.cores[0].pc,RAM);
        assert_eq!((m.cores[0].mepc,m.cores[0].mcause,m.cores[0].mtval,m.cores[0].mstatus),(0x1234,99,88,status));assert_eq!(m.cores[0].x[10],0x55);assert_eq!(m.exceptions,0);assert_eq!(m.bus.boot_analog,before);
    }}
    assert_eq!(C5Bus::new().write32(GATE,8),Err(Fault::Unmapped));assert_eq!(selected().read32(GATE+12),Err(Fault::Unmapped));
}
#[test]fn gate_retention_and_watchdog_precedence_include_partial_writes(){
    let mut b=selected();b.write32(GATE,3).unwrap();b.request_reset(0x0c);assert_eq!(b.write32(GATE,1),Err(Fault::Unsupported));
    b.reboot([0;6]);assert_eq!(b.read32(GATE),Ok(3));b.request_reset(3);b.reboot([0;6]);assert_eq!(b.read32(GATE),Ok(0));
    for width in[1,2,4]{let mut b=selected();b.write32(GATE,1).unwrap();let mut expected=b.boot_analog.clone().unwrap();b.cycles=130_000_000;
        let result=match width{1=>b.write8(GATE,3),2=>b.write16(GATE,3),_=>b.write32(GATE,3)};assert_eq!(result,Err(Fault::Unsupported));
        // Old autonomous analog work legitimately settles strictly before
        // reset_at; compare with that elapsed state, not its earlier clock.
        expected.pump(129_999_999);assert!(b.sw_reset());assert_eq!(b.reset_cause(),7);assert_eq!(b.watchdog_reset_at,Some(130_000_000));assert_eq!(b.boot_analog,Some(expected));
    }
}
