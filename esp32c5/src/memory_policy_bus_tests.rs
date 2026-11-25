//! Actual guest CSR instructions and memory effects through the observer bridge.
use crate::Machine;
use emu_core::Bus;
const RAM:u32=crate::bus::SRAM_BASE;
fn csrw(n:u32,rs:u32)->u32{(n<<20)|(rs<<15)|(1<<12)|0x73}
fn fixture(words:&[u32])->Machine{
    let mut m=crate::rom_machine([0;6],4<<20);m.bus.select_validated_rom_profile().unwrap();m.bus.rom_boot=true;
    m.cores[0].enable_c5_memory_protection().unwrap();m.cores[0].pc=RAM;m.cores[0].x[2]=RAM+0x50000;
    let mut code=words.iter().flat_map(|w|w.to_le_bytes()).collect::<Vec<_>>();code.extend_from_slice(&0x0000_006fu32.to_le_bytes());
    m.bus.load_ram(RAM,&code,code.len()).unwrap();m
}
struct UnitCost;
impl emu_core::CostModel for UnitCost{
    fn lifecycle(&mut self,_:&emu_core::LifecycleFacts)->Result<(),String>{Ok(())}
    fn cycles(&mut self,_:&emu_core::ExecutionFacts<'_>)->Result<u32,String>{Ok(1)}
}
#[test]fn a_new_policy_generation_applies_before_the_next_load_or_store(){
    for (cfg,cause)in[(0xc000_0015,7),(0xc000_0014,5)]{
        // sw a1,0(t0); cfgOFF; addr; cfg; lw a3,0(t0); sw a2,0(t0).
        let mut m=fixture(&[0x00b2_a023,csrw(0xbc0,14),csrw(0xbd0,15),csrw(0xbc0,16),0x0002_a683,0x00c2_a023]);
        let data=RAM+0x1000;m.cores[0].x[5]=data;m.cores[0].x[11]=17;m.cores[0].x[12]=18;
        m.cores[0].x[14]=0;m.cores[0].x[15]=(data>>2)|0x1ff;m.cores[0].x[16]=cfg;
        assert!(matches!(m.run(20),esp_soc::Stop::Exceptions(1)));assert_eq!(m.cores[0].mcause,cause);assert_eq!(m.bus.read32(data),Ok(17));
        assert_eq!(m.bus.first_fault.as_ref().unwrap().address,data);
        if cause==7{assert_eq!(m.cores[0].x[13],17,"TYPE1 permits read but not write");}
    }
}
#[test]fn decoded_four_byte_fetch_checks_the_whole_pmp_match(){
    let mut m=fixture(&[csrw(0x3b0,11),csrw(0x3a0,12),0x0003_0067]); // jalr zero,0(t1)
    m.cores[0].x[11]=((RAM+128)>>2)|15;m.cores[0].x[12]=0x9d;m.cores[0].x[6]=RAM+254;m.cores[0].x[10]=0x55;
    m.bus.load_ram(RAM+254,&0x0015_0513u32.to_le_bytes(),4).unwrap();
    assert!(matches!(m.run(20),esp_soc::Stop::Exceptions(1)));assert_eq!(m.cores[0].mcause,1);assert_eq!(m.cores[0].x[10],0x55);
    let f=m.bus.first_fault.as_ref().unwrap();assert_eq!((f.address,f.width),(RAM+254,4));
}
#[test]fn an_unqualified_policy_stops_plain_and_priced_execution_without_guest_exception_state(){
    for priced in[false,true]{for operation in[0x0002_a503,0x00b2_a023,0x00b2_a52f,0x0002_8067]{
        let data=RAM+0x1000;let mut m=fixture(&[csrw(0xbd0,15),operation]);
        m.cores[0].x[5]=data;m.cores[0].x[15]=data>>2;m.cores[0].x[10]=0x55;m.cores[0].x[11]=18;
        m.cores[0].mepc=0x1234;m.cores[0].mcause=99;m.cores[0].mtval=88;let status=m.cores[0].mstatus;
        m.bus.load_ram(data,&0x0015_0513u32.to_le_bytes(),4).unwrap();m.dbg.stop_on_unimplemented=false;
        if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        let expected=if operation==0x0002_8067{data}else{RAM+4};
        assert!(matches!(m.run(20),esp_soc::Stop::ModelLimit{core:0,pc}if pc==expected));
        assert_eq!((m.cores[0].pc,m.cores[0].mepc,m.cores[0].mcause,m.cores[0].mtval,m.cores[0].mstatus),(expected,0x1234,99,88,status));
        assert_eq!(m.cores[0].x[10],0x55);assert_eq!(m.exceptions,0);assert_eq!(m.bus.read32(data),Ok(0x0015_0513));
        assert_eq!(m.bus.memory_policy_unsupported().unwrap().0.address,data);
    }}
}
struct EarlyStop(bool);
impl esp_soc::observe::Observer<crate::C5>for EarlyStop{
    fn name(&self)->&'static str{"policy-context-stop"}
    fn wants(&self)->esp_soc::observe::Wants{esp_soc::observe::Wants::INSN}
    fn on_insn(&mut self,_:&esp_soc::observe::Ctx,_:usize,_:&riscv_rv32::Cpu,_:&mut crate::C5Bus,pc:u32)->Option<esp_soc::Stop>{(!self.0).then_some(esp_soc::Stop::Breakpoint(pc))}
    fn after_insn(&mut self,_:&esp_soc::observe::Ctx,_:usize,cpu:&riscv_rv32::Cpu,_:&mut crate::C5Bus)->Option<esp_soc::Stop>{self.0.then_some(esp_soc::Stop::Breakpoint(cpu.pc))}
}
#[test]fn observer_order_cannot_leave_a_cpu_context_on_host_inspection(){
    for priced in[false,true]{for after in[false,true]{
        let mut m=crate::machine([0;6],4<<20);let monitor=m.bus.cpu_monitor();
        if !after{m.add_observer(Box::new(crate::cpu_monitor::CpuObserver(monitor.clone())));}
        m.add_observer(Box::new(EarlyStop(after)));
        if after{m.add_observer(Box::new(crate::cpu_monitor::CpuObserver(monitor)));}
        m.bus.select_validated_rom_profile().unwrap();m.bus.rom_boot=true;m.cores[0].enable_c5_memory_protection().unwrap();
        let data=RAM+0x1000;m.cores[0].try_write_csr(0xbc0,0).unwrap();m.cores[0].try_write_csr(0xbd0,(data>>2)|15).unwrap();m.cores[0].try_write_csr(0xbc0,0xe000_001c).unwrap();
        m.bus.load_ram(RAM,&0x0000_0013u32.to_le_bytes(),4).unwrap();m.cores[0].pc=RAM;
        if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        assert!(matches!(m.run(20),esp_soc::Stop::Breakpoint(_)));
        assert_eq!(m.bus.write32(data,0xfeed),Ok(()));assert_eq!(m.bus.read32(data),Ok(0xfeed));assert!(m.bus.first_fault.is_none());
    }}
}
