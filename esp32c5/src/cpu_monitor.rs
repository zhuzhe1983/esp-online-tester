//! Typed C5 ROM execution samples. No shared-core hook or SRAM fast-path observer.
use std::{cell::RefCell, rc::Rc};
use crate::{C5, C5Bus, bus_monitor::{C5BusMonitor, CpuAccess, CpuBus}};
use esp_soc::{observe::{Ctx, Observer, Wants}, Stop};
use riscv_rv32::{Cpu, Trap,Fault};
use riscv_rv32::c5_memory_protection::{Access,GuardedMachineContext,PolicyDecision,SnapshotV1,UnsupportedReason};

pub(crate) struct CpuMonitor { pub regs:C5BusMonitor, context:Option<(u32,u32)>, machine_cpu:bool, mprv:bool,
    policy:Option<SnapshotV1>,policy_error:Option<UnsupportedReason>,memory_unsupported:Option<(Access,UnsupportedReason)> }
pub(crate) type Monitor = Rc<RefCell<CpuMonitor>>;
impl CpuMonitor {
    pub fn new()->Self{Self{regs:C5BusMonitor::new(),context:None,machine_cpu:false,mprv:false,policy:None,policy_error:None,memory_unsupported:None}}
    pub fn begin(&mut self,pc:u32,sp:u32){self.context=Some((pc,sp));self.machine_cpu=false;self.mprv=false;self.regs.observe_cpu(pc,Some(sp));}
    pub fn begin_core(&mut self,pc:u32,cpu:&Cpu){
        self.begin(pc,cpu.x[2]);self.machine_cpu=cpu.machine_only;self.mprv=cpu.mstatus&riscv_rv32::state::mstatus::MPRV!=0;
        self.policy_error=cpu.c5_memory_error();
        if let Some(p)=cpu.c5_memory_protection(){
            if self.policy.as_ref().is_none_or(|s|s.epoch()!=p.epoch()||s.generation()!=p.generation()){self.policy=Some(p.snapshot());}
        }else{self.policy=None;}
    }
    pub fn machine_context(&self)->Option<bool>{self.context.map(|_|self.machine_cpu&&!self.mprv)}
    pub fn policy_active(&self)->bool{self.policy.is_some()||self.policy_error.is_some()}
    pub fn memory_unsupported(&self)->Option<(Access,UnsupportedReason)>{self.memory_unsupported}
    pub fn check_memory(&mut self,access:Access)->Result<(),Fault>{
        // Host inspection is distinct from an observed CPU instruction.
        if self.context.is_none(){return Ok(());}
        let check=(||{
            if let Some(e)=self.policy_error{return Err(e);}
            let Some(p)=&self.policy else{return Ok(true);};let ctx=GuardedMachineContext::new(self.machine_cpu,self.mprv)?;
            for d in[p.check_pma(access,ctx),p.check_pmp(access,ctx)]{match d{
                PolicyDecision::Deny{..}=>return Ok(false),PolicyDecision::Unsupported(e)=>return Err(e),_=>{}
            }}Ok(true)
        })();
        match check{Ok(true)=>Ok(()),Ok(false)=>Err(Fault::Prohibited),Err(e)=>{self.memory_unsupported=Some((access,e));Err(Fault::Unsupported)}}
    }
    pub fn end(&mut self){self.context=None;self.machine_cpu=false;self.mprv=false;}
    pub fn trap(&mut self,pc:u32,trap:&Trap){
        if matches!(trap,Trap::Exception(_)|Trap::Ebreak(_)){self.regs.observe_exception(pc);}
        self.end();
    }
    pub fn access(&mut self,bus:CpuBus,address:u32,width:u8,write:bool){
        if let Some((pc,sp))=self.context{self.regs.observe_access(CpuAccess{bus,address,width,write,pc,sp:Some(sp)});}
    }
    pub fn cpu_reset(&mut self){self.end();self.policy=None;self.policy_error=None;self.memory_unsupported=None;self.regs.cpu_reset();}
    pub fn core_reset(&mut self){self.end();self.policy=None;self.policy_error=None;self.memory_unsupported=None;self.regs.core_reset();}
}
pub(crate) struct CpuObserver(pub Monitor);
impl Observer<C5> for CpuObserver {
    fn name(&self)->&'static str{"c5-hardware-monitor"}
    fn wants(&self)->Wants{Wants::INSN|Wants::TRAP}
    fn on_insn(&mut self,_cx:&Ctx,core:usize,cpu:&Cpu,bus:&mut C5Bus,pc:u32)->Option<Stop>{
        // An assisted API refused before this instruction. Stop before step()
        // can accept an already-pending IRQ or change guest trap CSRs.
        if bus.wifi_compat.as_ref().is_some_and(|w|w.limited()){return Some(Stop::ModelLimit{core,pc});}
        let mut m=self.0.borrow_mut();let before=m.regs.irq();m.begin_core(pc,cpu);bus.irq_dirty|=before!=m.regs.irq();None
    }
    fn on_trap(&mut self,_cx:&Ctx,_core:usize,_cpu:&Cpu,pc:u32,trap:&Trap){self.0.borrow_mut().trap(pc,trap);}
    fn after_insn(&mut self,_cx:&Ctx,_core:usize,_cpu:&Cpu,_bus:&mut C5Bus)->Option<Stop>{self.0.borrow_mut().end();None}
}

#[cfg(test)]
mod tests{
    use super::*;
    use emu_core::Bus;
    use esp_soc::SocBus;
    struct StopAfter;
    impl Observer<C5> for StopAfter{
        fn name(&self)->&'static str{"stop-after-fixture"}
        fn wants(&self)->Wants{Wants::INSN}
        fn after_insn(&mut self,_:&Ctx,_:usize,cpu:&Cpu,_:&mut C5Bus)->Option<Stop>{Some(Stop::Breakpoint(cpu.pc))}
    }
    #[test]
    fn earlier_after_observer_stop_also_drops_hardware_monitor_context(){
        let mut m=crate::machine([0;6],4<<20);let ram=crate::bus::SRAM_BASE;
        m.bus.load_ram(ram,&0x0000_0013u32.to_le_bytes(),4).unwrap();m.cores[0].pc=ram;m.cores[0].x[2]=ram+0x50000;
        m.add_observer(Box::new(StopAfter));let monitor=m.bus.cpu_monitor();m.add_observer(Box::new(CpuObserver(monitor)));
        for(a,v)in[(0x6009_6100,1),(0x600c_2044,3),(0x600c_2010,ram+0x100),(0x600c_2014,ram+0x103),(0x600c_2000,3),(0x600c_2008,3),(0x6001_006c,23)]{m.bus.write32(a,v).unwrap();}
        assert!(matches!(m.run(1),Stop::Breakpoint(_)));assert_eq!(m.insns(),1);
        m.bus.write32(ram+0x100,42).unwrap();assert!(m.bus.irq_sources_of(0,7).is_empty());
        assert_eq!(m.bus.read32(0x600c_2030).unwrap(),0);assert_eq!(m.bus.read32(0x600c_2034).unwrap(),0);
    }
}
