//! Model uncertainty stops execution without manufacturing a hardware exception.
use emu_core::{Bus,Fault,StepKind};
use riscv_rv32::{Cpu,FlatRam,Trap,step,state::mstatus};
const BASE:u32=0x4038_0000;
struct LimitBus{ram:FlatRam,fetch:bool,reads:u32,writes:u32,atomic:u32}
impl Bus for LimitBus{
    fn note_code_page(&mut self,_:u32){}
    fn fetch(&mut self,a:u32)->Result<[u8;4],Fault>{if self.fetch{Err(Fault::Unsupported)}else{self.ram.fetch(a)}}
    fn read8(&mut self,_:u32)->Result<u8,Fault>{Err(Fault::Unsupported)}
    fn read16(&mut self,_:u32)->Result<u16,Fault>{Err(Fault::Unsupported)}
    fn read32(&mut self,_:u32)->Result<u32,Fault>{self.reads+=1;Err(Fault::Unsupported)}
    fn write8(&mut self,_:u32,_:u8)->Result<(),Fault>{Err(Fault::Unsupported)}
    fn write16(&mut self,_:u32,_:u16)->Result<(),Fault>{Err(Fault::Unsupported)}
    fn write32(&mut self,_:u32,_:u32)->Result<(),Fault>{self.writes+=1;Err(Fault::Unsupported)}
    fn check_atomic32(&mut self,_:u32,_:bool,_:bool)->Result<(),Fault>{self.atomic+=1;Err(Fault::Unsupported)}
}
fn fixture(word:u32)->(Cpu,LimitBus){
    let mut cpu=Cpu::new_esp32p4();cpu.pc=BASE;cpu.mtvec=BASE+0x100;cpu.mstatus|=mstatus::FS_INITIAL;
    cpu.mepc=0x1234;cpu.mcause=99;cpu.mtval=88;cpu.x[5]=BASE+0x80;cpu.x[10]=0x55;cpu.f[10]=0x66;cpu.reservation=Some(BASE+0x80);
    let mut ram=FlatRam::new(BASE,512);ram.mem[..4].copy_from_slice(&word.to_le_bytes());
    (cpu,LimitBus{ram,fetch:false,reads:0,writes:0,atomic:0})
}
#[test]fn fetch_does_not_claim_instruction_bytes_or_take_a_guest_trap(){
    let(mut cpu,mut bus)=fixture(0x0000_0013);bus.fetch=true;let outcome=riscv_rv32::exec::step_outcome(&mut cpu,&mut bus);
    assert_eq!((outcome.kind,outcome.bytes,outcome.length),(StepKind::TrapBefore(Trap::ModelLimit(BASE)),None,0));
    assert_eq!((cpu.pc,cpu.mepc,cpu.mcause,cpu.mtval),(BASE,0x1234,99,88));assert_eq!(cpu.retired_count,0);
}
#[test]fn integer_float_and_atomic_limits_leave_destinations_and_exception_state_unchanged(){
    for(word,reads,writes,atomic)in[(0x0002_a503,1,0,0),(0x00b2_a023,0,1,0),(0x0002_a507,1,0,0),(0x00b2_a027,0,1,0),
        (0x1002_a52f,0,0,1),(0x18b2_a52f,0,0,1),(0x00b2_a52f,0,0,1)]{
        let(mut cpu,mut bus)=fixture(word);let status=cpu.mstatus;let reservation=cpu.reservation;
        assert_eq!(step(&mut cpu,&mut bus),Err(Trap::ModelLimit(BASE)),"{word:#x}");
        assert_eq!((cpu.pc,cpu.mepc,cpu.mcause,cpu.mtval,cpu.mstatus),(BASE,0x1234,99,88,status));
        assert_eq!((cpu.x[10],cpu.f[10],cpu.reservation,cpu.retired_count),(0x55,0x66,reservation,0));
        assert_eq!((bus.reads,bus.writes,bus.atomic),(reads,writes,atomic));
    }
}
#[test]fn shv_vector_uncertainty_precedes_all_guest_clic_entry_state(){
    for waiting in[false,true]{
        let(mut cpu,mut bus)=fixture(0x0000_0013);cpu.mtvec=(BASE+0x100)|3;cpu.mtvt=BASE+0x180;cpu.mil=3;cpu.waiting=waiting;
        cpu.mstatus|=mstatus::MIE;cpu.irq=Some(riscv_rv32::exec::CLIC_IRQ_SHV|7<<riscv_rv32::exec::CLIC_IRQ_LEVEL_SHIFT|5);
        let status=cpu.mstatus;let reservation=cpu.reservation;
        assert_eq!(step(&mut cpu,&mut bus),Err(Trap::ModelLimit(BASE)));
        assert_eq!((cpu.pc,cpu.mepc,cpu.mcause,cpu.mtval,cpu.mstatus,cpu.mil,cpu.reservation,cpu.waiting),(BASE,0x1234,99,88,status,3,reservation,waiting));
        assert_eq!((cpu.insn_count,cpu.cycle_count,cpu.retired_count,bus.reads),(0,0,0,1));
    }
}
