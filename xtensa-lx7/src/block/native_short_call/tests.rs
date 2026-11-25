use super::*;
use crate::{Fault,FlatRam};
use crate::state::ps;
const CODE:u32=0x4200_0000;
const LEAF:u32=CODE+0x100;
struct CodeBus {ram:FlatRam,versions:[u32;8],pc:u32,stop:u32,remap:bool,change_on_call:bool}
impl Bus for CodeBus {
    fn note_code_page(&mut self,_:u32) {}
    fn read8(&mut self,a:u32)->Result<u8,Fault> {self.ram.read8(a)}
    fn read16(&mut self,a:u32)->Result<u16,Fault> {self.ram.read16(a)}
    fn read32(&mut self,a:u32)->Result<u32,Fault> {self.ram.read32(a)}
    fn write8(&mut self,a:u32,v:u8)->Result<(),Fault> {self.ram.write8(a,v)}
    fn write16(&mut self,a:u32,v:u16)->Result<(),Fault> {self.ram.write16(a,v)}
    fn write32(&mut self,a:u32,v:u32)->Result<(),Fault> {self.ram.write32(a,v)}
    fn fetch(&mut self,a:u32)->Result<[u8;4],Fault> {self.ram.fetch(a)}
    fn page_versions(&self)->&[u32] {&self.versions}
    fn code_page(&mut self,a:u32)->u32 {let p=a.wrapping_sub(CODE)/128;if self.remap&&p==2 {3} else {p}}
    fn note_pc(&mut self,pc:u32) {
        self.pc=pc;
        if self.change_on_call&&pc==CODE {self.ram.mem[0x101]=0x81;self.versions[2]+=1;self.change_on_call=false;}
    }
    fn block_break(&self)->bool {self.stop!=0&&self.stop==self.pc}
}
fn bus()->CodeBus {
    bus_at(LEAF)
}
fn bus_at(leaf:u32)->CodeBus {
    let mut b=CodeBus{ram:FlatRam::new(CODE,1024),versions:[0;8],pc:0,stop:0,remap:false,change_on_call:false};
    // Xtensa CALL8's displacement is relative to (PC&!3)+4, in words.
    let call=0x25|((leaf-(CODE+4))/4)<<6;
    b.ram.mem[..3].copy_from_slice(&call.to_le_bytes()[..3]);
    b.ram.mem[3..6].copy_from_slice(&[0x06,0xff,0xff]); // j self after return
    let start=(leaf-CODE) as usize;b.ram.mem[start..start+5].copy_from_slice(&[0x36,0x41,0,0x1d,0xf0]);
    assert_eq!(decode(CODE,b.fetch(CODE).unwrap()).imm as u32,leaf);b
}
fn setup(c:&mut Cpu,wb:u32,status:u32,ws:u32,cpi:u32) {
    c.pc=CODE;c.ar=std::array::from_fn(|i|0xffff_0000|i as u32);c.ps=status;c.windowbase=wb;c.windowstart=ws;
    c.set_ar(1,16);c.epc=[0;8];c.depc=0;c.exccause=123;c.vecbase=CODE+0x1000;
    c.lbeg=CODE+0x40;c.lend=0;c.lcount=0;c.waiting=false;c.insn_count=0;c.ccount=0;
    c.ccompare=[u32::MAX;3];c.intenable=0;c.interrupt=0;c.approximate_cpi=cpi;c.jit_trap=None;
    c.boundary_bloom=0;c.blocks.observed=false;c.blocks.resume=(0,0,1);
}
#[derive(Debug,Eq,PartialEq)]
struct State {ar:[u32;64],pc:u32,ps:u32,wb:u32,ws:u32,epc:[u32;8],depc:u32,cause:u32,lcount:u32,insns:u64,ccount:u32,interrupt:u32}
fn state(c:&Cpu)->State {State{ar:c.ar,pc:c.pc,ps:c.ps,wb:c.windowbase,ws:c.windowstart,epc:c.epc,depc:c.depc,cause:c.exccause,lcount:c.lcount,insns:c.insn_count,ccount:c.ccount,interrupt:c.interrupt}}
fn warmed()->(Cpu,CodeBus) {
    warmed_at(LEAF)
}
fn warmed_at(leaf:u32)->(Cpu,CodeBus) {
    let mut c=Cpu::new(0);let mut b=bus_at(leaf);c.blocks.jit_enabled=true;
    assert_eq!(decode_block(&mut c,&mut b,leaf).unwrap().2,2);
    assert_eq!(decode_block(&mut c,&mut b,CODE).unwrap().2,1);
    setup(&mut c,0,ps::WOE,1,1);
    assert_eq!(run_block(&mut c,&mut b,3),(3,None),"the actual generated caller must retire CALL/ENTRY/RETW");
    assert_eq!(c.blocks.entries[BlockCache::index(CODE)].short,2);(c,b)
}
fn compare(c:&mut Cpu,b:&mut CodeBus,oracle:&mut Cpu,ob:&mut CodeBus,budget:u32)->(u32,Option<Trap>) {
    let before=c.blocks.jit_instructions;let r=run_block(c,b,budget);let mut expected=None;
    for _ in 0..r.0 {if let Err(t)=crate::exec::step(oracle,ob) {expected=Some(t);break;}}
    assert_eq!(r.1,expected);assert_eq!(state(c),state(oracle));
    assert_eq!(c.blocks.jit_instructions-before,c.insn_count,"every retired ISA instruction uses actual native code/helpers");r
}
#[test]
fn real_three_instruction_call_matches_single_steps_across_windows_and_budgets() {
    let (mut c,mut b)=warmed();let mut oracle=Cpu::new(0);let mut ob=bus();let mut checked=0;
    for wb in 0..16u32 {for status in [0,ps::EXCM,ps::WOE,ps::WOE|ps::EXCM,ps::WOE|ps::UM] {
        for inc in 0..4 {for ws in [0,1<<wb,1<<((wb+2)&15),0xffff] {for looped in [false,true] {
            for cpi in [1,16] {for budget in [1,2,3,4,64] {
                let status=status|(inc<<ps::CALLINC_SHIFT)|(5<<ps::OWB_SHIFT);
                setup(&mut c,wb,status,ws,cpi);setup(&mut oracle,wb,status,ws,cpi);
                if looped {c.lend=LEAF+3;c.lcount=2;oracle.lend=c.lend;oracle.lcount=c.lcount;}
                compare(&mut c,&mut b,&mut oracle,&mut ob,budget);checked+=1;
            }}
        }}}
    }}
    assert_eq!(checked,25_600);
}
#[test]
fn guards_refuse_stale_or_remapped_callee_and_observer_boundaries() {
    for mode in 0..6 {
        let (mut c,mut b)=warmed();let mut oracle=Cpu::new(0);let mut ob=bus();
        setup(&mut c,0,ps::WOE,1,1);setup(&mut oracle,0,ps::WOE,1,1);
        match mode {
            0=>{b.versions[2]+=1;b.ram.mem[0x101]=0x81;ob.versions[2]+=1;ob.ram.mem[0x101]=0x81;}
            1=>{b.remap=true;ob.remap=true;}
            2=>{c.boundary_bloom=pc_bit(LEAF);oracle.boundary_bloom=c.boundary_bloom;}
            3=>{c.boundary_bloom=pc_bit(LEAF+3);oracle.boundary_bloom=c.boundary_bloom;}
            4=>{c.blocks.observed=true;}
            _=>{b.change_on_call=true;ob.change_on_call=true;}
        }
        assert_eq!(compare(&mut c,&mut b,&mut oracle,&mut ob,64).0,1,"guard{mode} must execute only the real CALL");
    }
    for stop in [CODE,LEAF,LEAF+3] {
        let (mut c,mut b)=warmed();let mut oracle=Cpu::new(0);let mut ob=bus();
        setup(&mut c,0,ps::WOE,1,1);setup(&mut oracle,0,ps::WOE,1,1);b.stop=stop;ob.stop=stop;
        assert_eq!(compare(&mut c,&mut b,&mut oracle,&mut ob,64).0,if stop==CODE {1} else if stop==LEAF {2} else {3});
    }
}
#[test]
fn ccompare_matches_and_wrap_never_cross_an_unretired_boundary() {
    for cpi in [1,16] {for start in [0,u32::MAX-1] {for distance in [1,2,3,4] {for budget in [1,2,3,63,64,65,30_000] {
        let (mut c,mut b)=warmed();let mut oracle=Cpu::new(0);let mut ob=bus();
        setup(&mut c,0,ps::WOE|ps::UM,1,cpi);setup(&mut oracle,0,ps::WOE|ps::UM,1,cpi);
        c.ccount=start;oracle.ccount=start;c.ccompare[0]=start.wrapping_add(distance*cpi);oracle.ccompare=c.ccompare;
        c.intenable=1<<6;oracle.intenable=c.intenable;
        let r=compare(&mut c,&mut b,&mut oracle,&mut ob,budget);
        if c.interrupt&(1<<6)!=0 {
            let nr=run_block(&mut c,&mut b,1);let ir=crate::exec::step(&mut oracle,&mut ob);
            assert_eq!(nr.1,ir.err());assert_eq!(state(&c),state(&oracle));
        }
        assert!(r.0<=budget);
    }}}}
}
#[test]
fn both_callee_pages_and_a_suspended_plain_entry_remain_owned_and_validated() {
    let leaf=LEAF+0x7c; // RETW.N's second byte crosses the128-byte code-page boundary.
    for page in [2,3] {
        let (mut c,mut b)=warmed_at(leaf);let mut oracle=Cpu::new(0);let mut ob=bus_at(leaf);
        setup(&mut c,0,ps::WOE,1,1);setup(&mut oracle,0,ps::WOE,1,1);
        b.versions[page]+=1;ob.versions[page]+=1;
        assert_eq!(compare(&mut c,&mut b,&mut oracle,&mut ob,64).0,1,"either changed page must refuse the callee");
    }
    let mut c=Cpu::new(0);let mut b=bus();let mut oracle=Cpu::new(0);let mut ob=bus();
    for bus in [&mut b,&mut ob] {
        let call=bus.ram.mem[..3].to_vec();bus.ram.mem[..2].copy_from_slice(&[0x1b,0x66]);
        bus.ram.mem[2..5].copy_from_slice(&call);bus.ram.mem[5..8].copy_from_slice(&[0x06,0xff,0xff]);
    }
    setup(&mut c,0,ps::WOE,1,1);setup(&mut oracle,0,ps::WOE,1,1);
    assert_eq!(run_block(&mut c,&mut b,1),(1,None));assert!(crate::exec::step(&mut oracle,&mut ob).is_ok());
    let ei=BlockCache::index(CODE);assert_eq!(c.blocks.entries[ei].short,1);assert_eq!(c.blocks.resume.2,CODE+2);
    let old_code=c.blocks.entries[ei].code;let old_bytes=c.blocks.code_bytes();
    // A callee is decoded between calls; the already-suspended caller still names its old arena.
    decode_block(&mut c,&mut b,LEAF).unwrap();c.insn_count=0;oracle.insn_count=0;
    assert_eq!(compare(&mut c,&mut b,&mut oracle,&mut ob,3).0,3);
    assert_ne!(c.blocks.entries[ei].code,old_code);assert!(c.blocks.code_bytes()>old_bytes);
    assert_eq!(c.pc,CODE+5);assert_eq!(c.get_ar(6),0xffff_0007,"the prefix must not run twice");
    let cloned=c.blocks.clone();assert_eq!(cloned.code_bytes(),0);drop(cloned);
    c.blocks.flush();assert_eq!(c.blocks.code_bytes(),0);assert_eq!(c.blocks.resume.2,1);
    assert_eq!(c.blocks.entries[ei].short,0);
}
