//! Real instruction fixtures for the constrained startup context, not silicon.
use crate::{C5Bus,Machine};
use emu_core::Bus;
use esp_soc::SocBus;
const STORES:[u32;20]=[0x6009_90c4,0x6009_98c4,0x600b_38c4,0x6009_a0c4,
    0x6009_8000,0x6009_8004,0x600b_3400,0x6009_8008,0x6009_800c,0x6009_8010,
    0x6009_8014,0x6009_8018,0x6009_8020,0x6009_8044,0x6009_8048,0x6009_804c,
    0x6009_8058,0x6009_805c,0x6009_8060,0x6009_8064];
fn fixture(observer:bool)->Machine{
    let mut m=if observer{crate::rom_machine([0;6],4<<20)}else{crate::machine([0;6],4<<20)};
    m.bus.select_validated_rom_profile().unwrap();m.bus.rom_boot=true;let mut code=Vec::new();
    for address in STORES{
        for word in[(address.wrapping_add(0x800)&0xffff_f000)|0x2b7,((address&4095)<<20)|0x0002_8293,0x0002_a023]{code.extend_from_slice(&word.to_le_bytes());}
    }
    code.extend_from_slice(&0x0000_006fu32.to_le_bytes());m.bus.load_ram(crate::bus::SRAM_BASE,&code,code.len()).unwrap();
    m.cores[0].pc=crate::bus::SRAM_BASE;m.cores[0].x[2]=crate::bus::SRAM_BASE+0x50000;m
}
struct UnitCost(std::sync::Arc<std::sync::atomic::AtomicUsize>);
impl emu_core::CostModel for UnitCost{
    fn lifecycle(&mut self,_:&emu_core::LifecycleFacts)->Result<(),String>{Ok(())}
    fn cycles(&mut self,f:&emu_core::ExecutionFacts<'_>)->Result<u32,String>{
        self.0.fetch_add(f.accesses.iter().filter(|a|a.kind!=emu_core::MemoryAccessKind::Fetch).count(),std::sync::atomic::Ordering::Relaxed);Ok(1)
    }
}
#[test]fn atomic_policy_cannot_consume_uart_data_in_plain_or_priced_execution(){
    for priced in[false,true]{for ram in[false,true]{
        let mut m=fixture(true);let code=[0x00b2_a52fu32,0x0000_006f].into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>();
        m.bus.load_ram(crate::bus::SRAM_BASE,&code,code.len()).unwrap();
        let data=crate::bus::SRAM_BASE+0x1000;m.bus.load_ram(data,&17u32.to_le_bytes(),4).unwrap();
        m.cores[0].x[5]=if ram{data}else{0x6000_0000};m.cores[0].x[11]=1;m.cores[0].x[10]=0x55;
        m.bus.uart_input(0,b"D");let accesses=std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        if priced{m.set_cost_model(Box::new(UnitCost(accesses.clone()))).unwrap();}
        let stop=m.run(1);assert!(m.console.uart0.is_empty());assert!(m.bus.uart[0].tx_out.is_empty());assert_eq!(m.bus.uart[0].rx_pending(),1);
        if ram{assert!(matches!(stop,esp_soc::Stop::MaxInsns),"{stop:?} priced={priced} fault={:?}",m.bus.first_fault);assert_eq!(m.cores[0].x[10],17);assert_eq!(m.bus.read32(data),Ok(18));}
        else{assert!(matches!(stop,esp_soc::Stop::Exceptions(1)));assert_eq!(m.cores[0].mcause,7);assert_eq!(m.cores[0].x[10],0x55);}
        if priced{assert_eq!(accesses.load(std::sync::atomic::Ordering::Relaxed),if ram{2}else{0});}
    }}
}
#[test]fn observed_machine_instructions_supply_all_twenty_real_zero_writes(){
    let mut m=fixture(true);assert!(!m.bus.dma_apm_supported());
    m.cores[0].mstatus=0; // Saved MPP0 is not the current privilege.
    assert!(matches!(m.run(60),esp_soc::Stop::MaxInsns));assert!(m.bus.first_fault.is_none());
    assert!(m.bus.dma_apm_supported());assert!(m.bus.boot_apm.as_ref().unwrap().report().contains("writes=4/15/1"));
    let state=m.bus.boot_apm.clone();assert!(m.bus.write32(STORES[0],0).is_err(),"host inspection is not an instruction context");assert_eq!(m.bus.boot_apm,state);
}
#[test]fn missing_observer_and_unconstrained_cpu_do_not_acquire_machine_permission(){
    let mut m=fixture(false);m.run(60);assert_eq!(m.bus.first_fault.as_ref().unwrap().address,STORES[0]);assert!(!m.bus.dma_apm_supported());
    let mut m=fixture(true);m.cores[0].machine_only=false;m.cores[0].mstatus=riscv_rv32::state::mstatus::MPP;
    m.run(60);assert_eq!(m.bus.first_fault.as_ref().unwrap().address,crate::bus::SRAM_BASE);assert!(!m.bus.dma_apm_supported());
    assert!(m.bus.boot_apm.as_ref().unwrap().report().contains("writes=0/0/0"));
}
#[test]fn owner_reset_domains_and_raw_deselection_restore_the_declared_boundary(){
    let mut m=fixture(true);m.run(60);let state=m.bus.boot_apm.clone();
    m.bus.request_reset(0x0c);assert_eq!(m.bus.reboot([0;6]),0x0c);assert_eq!(m.bus.boot_apm,state);
    m.bus.request_reset(3);assert_eq!(m.bus.reboot([0;6]),3);assert!(!m.bus.dma_apm_supported());
    assert!(m.bus.boot_apm.as_ref().unwrap().report().contains("func=[0x1f,0x1,0x0,0x3]"));
    m.bus.request_reset(0x10);assert_eq!(m.bus.reboot([0;6]),0x10);assert!(m.bus.boot_apm.as_ref().unwrap().report().contains("func=[0x1f,0x1,0x3,0x3]"));
    m.bus.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();assert!(m.bus.boot_apm.is_none());assert!(m.bus.dma_apm_supported());
    let mut b=C5Bus::new();assert!(b.write32(STORES[0],0).is_err());assert!(b.boot_apm.is_none());
}
