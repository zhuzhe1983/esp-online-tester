//! A policy rejection must precede even the read half of an AMO.
use emu_core::{Bus,Fault};
use riscv_rv32::{Cpu,FlatRam,Trap,step};
const BASE:u32=0x4038_0000;
struct Probe{ram:FlatRam,deny:bool,calls:Vec<(u32,bool,bool)>,reads:u32,writes:u32}
impl Bus for Probe{
    fn note_code_page(&mut self,_a:u32){}
    fn check_atomic32(&mut self,a:u32,r:bool,w:bool)->Result<(),Fault>{self.calls.push((a,r,w));if self.deny{Err(Fault::Prohibited)}else{Ok(())}}
    fn fetch(&mut self,a:u32)->Result<[u8;4],Fault>{self.ram.fetch(a)}
    fn read8(&mut self,a:u32)->Result<u8,Fault>{self.ram.read8(a)}
    fn read16(&mut self,a:u32)->Result<u16,Fault>{self.ram.read16(a)}
    fn read32(&mut self,a:u32)->Result<u32,Fault>{self.reads+=1;self.ram.read32(a)}
    fn write8(&mut self,a:u32,v:u8)->Result<(),Fault>{self.ram.write8(a,v)}
    fn write16(&mut self,a:u32,v:u16)->Result<(),Fault>{self.ram.write16(a,v)}
    fn write32(&mut self,a:u32,v:u32)->Result<(),Fault>{self.writes+=1;self.ram.write32(a,v)}
}
fn fixture(word:u32,deny:bool)->(Cpu,Probe){
    let mut cpu=Cpu::new_rv32imac();cpu.pc=BASE;cpu.mtvec=BASE+0x100;cpu.x[5]=BASE+0x80;cpu.x[10]=0x55;cpu.x[11]=1;
    let mut ram=FlatRam::new(BASE,512);ram.mem[..4].copy_from_slice(&word.to_le_bytes());ram.mem[0x80..0x84].copy_from_slice(&17u32.to_le_bytes());
    (cpu,Probe{ram,deny,calls:Vec::new(),reads:0,writes:0})
}
#[test]fn lr_sc_and_rmw_reject_before_read_write_or_destination_commit(){
    for(word,r,w,cause)in[(0x1002_a52f,true,false,5),(0x18b2_a52f,false,true,7),(0x00b2_a52f,true,true,7)]{
        let(mut cpu,mut bus)=fixture(word,true);cpu.reservation=Some(cpu.x[5]);
        assert_eq!(step(&mut cpu,&mut bus),Err(Trap::Exception(cause)));assert_eq!(bus.calls,[(BASE+0x80,r,w)]);
        assert_eq!((bus.reads,bus.writes),(0,0));assert_eq!(cpu.x[10],0x55);assert_eq!(cpu.mtval,BASE+0x80);
        assert_eq!(&bus.ram.mem[0x80..0x84],&17u32.to_le_bytes());
    }
}
#[test]fn failed_sc_with_no_reservation_has_no_memory_operation_to_probe(){
    let(mut cpu,mut bus)=fixture(0x18b2_a52f,true);step(&mut cpu,&mut bus).unwrap();
    assert_eq!(cpu.x[10],1);assert!(bus.calls.is_empty());assert_eq!((bus.reads,bus.writes),(0,0));
}
#[test]fn successful_policy_probe_is_not_itself_an_access(){
    let(mut cpu,mut bus)=fixture(0x00b2_a52f,false);step(&mut cpu,&mut bus).unwrap();
    assert_eq!(bus.calls,[(BASE+0x80,true,true)]);assert_eq!((bus.reads,bus.writes),(1,1));assert_eq!(cpu.x[10],17);
    assert_eq!(&bus.ram.mem[0x80..0x84],&18u32.to_le_bytes());
}
