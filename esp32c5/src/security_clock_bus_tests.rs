//! Real CPU/MMIO ownership checks, separate from original-factory execution.
use crate::{C5Bus,Machine};
use emu_core::{Bus,Fault};
use esp_soc::SocBus;
const SEC:u32=0x6009_613c;const RAM:u32=crate::bus::SRAM_BASE;
fn selected()->C5Bus{let mut b=C5Bus::new();b.select_validated_rom_profile().unwrap();b.rom_boot=true;b}
fn fixture(words:&[u32])->Machine{
    let mut m=crate::rom_machine([0;6],4<<20);m.quantum=1;m.bus.select_validated_rom_profile().unwrap();m.bus.rom_boot=true;
    let bytes=words.iter().flat_map(|w|w.to_le_bytes()).collect::<Vec<_>>();m.bus.load_ram(RAM,&bytes,bytes.len()).unwrap();
    m.cores[0].pc=RAM;m.cores[0].x[5]=SEC;m.cores[0].x[2]=RAM+0x50000;m
}
struct UnitCost;
impl emu_core::CostModel for UnitCost{
    fn lifecycle(&mut self,_:&emu_core::LifecycleFacts)->Result<(),String>{Ok(())}
    fn cycles(&mut self,_:&emu_core::ExecutionFacts<'_>)->Result<u32,String>{Ok(1)}
}
#[test]fn guest_sec_rmw_selects_a_source_without_granting_pll_or_crypto_readiness(){
    for priced in[false,true]{
        // lw a1,0(t0); andi a1,a1,-4; ori a1,a1,2; sw a1,0(t0).
        let mut m=fixture(&[0x0002_a583,0xffc5_f593,0x0025_e593,0x00b2_a023]);
        if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        let rates=(m.bus.boot_clock.cpu_hz(),m.bus.boot_clock.ahb_hz(),m.bus.boot_clock.apb_hz());
        for _ in 0..4{assert!(matches!(m.run(1),esp_soc::Stop::MaxInsns));}
        assert_eq!(m.cores[0].pc,RAM+16);assert_eq!(m.bus.read32(SEC),Ok(2));assert_eq!(m.bus.boot_clock.sec_clock_source(),Some(2));
        assert_eq!(m.bus.boot_clock.sec_source_hz(),None,"mux selection cannot power or calibrate the PLL");
        assert_eq!((m.bus.boot_clock.cpu_hz(),m.bus.boot_clock.ahb_hz(),m.bus.boot_clock.apb_hz()),rates);
        assert_eq!(m.exceptions,0);assert!(m.bus.first_fault.is_none());
    }
}
#[test]fn unqualified_sec_controls_stop_as_model_limits_without_corrupting_trap_state(){
    for priced in[false,true]{for value in[1,3,4,8,u32::MAX]{
        let mut m=fixture(&[0x00b2_a023]);m.cores[0].x[11]=value;m.cores[0].x[10]=0x55;
        m.cores[0].mepc=0x1234;m.cores[0].mcause=99;m.cores[0].mtval=88;let status=m.cores[0].mstatus;let before=m.bus.boot_clock.report();
        m.dbg.stop_on_unimplemented=false;if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        assert!(matches!(m.run(1),esp_soc::Stop::ModelLimit{core:0,pc}if pc==RAM));
        assert_eq!((m.cores[0].mepc,m.cores[0].mcause,m.cores[0].mtval,m.cores[0].mstatus),(0x1234,99,88,status));
        assert_eq!(m.cores[0].x[10],0x55);assert_eq!(m.exceptions,0);assert_eq!(m.bus.boot_clock.report(),before);assert_eq!(m.bus.boot_clock.sec_source_hz(),Some(48_000_000));
    }}
    let mut b=selected();assert_eq!(b.read32(SEC-4),Err(Fault::Unmapped));
    assert_eq!(C5Bus::new().read32(SEC),Err(Fault::Unmapped),"off-profile classification unchanged");
}
#[test]fn sec_owner_honors_reset_retention_raw_exit_and_an_earlier_watchdog_barrier(){
    let mut b=selected();b.write32(SEC,2).unwrap();b.request_reset(0x0c);assert_eq!(b.write32(SEC,0),Err(Fault::Unsupported));
    assert_eq!(b.boot_clock.sec_clock_source(),Some(2));b.reboot([0;6]);assert_eq!(b.read32(SEC),Ok(2));
    b.request_reset(3);b.reboot([0;6]);assert_eq!(b.read32(SEC),Ok(0));
    b.write32(SEC,2).unwrap();b.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();assert_eq!(b.boot_clock.sec_clock_source(),None);assert_eq!(b.boot_clock.sec_source_hz(),None);
    assert_eq!(b.read32(SEC),Err(Fault::Unmapped));
    let mut b=selected();b.cycles=130_000_000;assert_eq!(b.write32(SEC,2),Err(Fault::Unsupported));
    assert!(b.sw_reset());assert_eq!(b.reset_cause(),7);assert_eq!(b.watchdog_reset_at,Some(130_000_000));assert_eq!(b.boot_clock.sec_clock_source(),Some(0));
}
