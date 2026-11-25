//! EX164: bounded native CALL8/ENTRY/RETW fusion; the default feature set is unchanged.
use super::*;

/// Upgrade only between executions, before retaining any entry/arena reference. The callee
/// must already have been decoded normally; no speculative fetch, compilation during JIT
/// execution, cache reset or release of the previous entry occurs here.
#[cold]
pub(super) fn try_fuse<B:Bus>(cpu:&mut Cpu,bus:&mut B,ei:usize) {
    if cpu.price_control || cpu.blocks.observed || !cpu.blocks.jit_active() {return;}
    let caller=cpu.blocks.entries[ei];
    if caller.code==crate::jit::NONE || caller.n==0 {return;}
    let call=cpu.blocks.arena[(caller.start+u32::from(caller.n)-1) as usize];
    let target=call.insn.imm as u32;
    let callee=cpu.blocks.entries[BlockCache::index(target)];
    if callee.pc!=target || !BlockCache::valid(&callee,bus.page_versions()) {return;}
    let shape=if callee.n==2 {
        let entry=cpu.blocks.arena[callee.start as usize];let ret=cpu.blocks.arena[callee.start as usize+1];
        (entry.insn.op==Op::Entry && entry.insn.s==1 && entry.insn.imm==32 && entry.insn.len==3
            && ret.insn.op==Op::RetwN && ret.insn.len==2).then_some((entry,ret))
    } else {None};
    let Some((entry,ret))=shape else {cpu.blocks.entries[ei].short=0;return;};
    let pc=cpu.blocks.arena[caller.start as usize..(caller.start+u32::from(caller.n)-1) as usize]
        .iter().fold(caller.pc,|p,i|p.wrapping_add(u32::from(i.insn.len)));
    let ret_pc=target.wrapping_add(3);let last=ret_pc.wrapping_add(1);
    if [bus.code_page(target),bus.code_page(last)]!=callee.vidx {return;}
    let pv=bus.page_versions();
    if callee.vidx.iter().zip(callee.ver).any(|(&i,v)|pv.get(i as usize)!=Some(&v)) {return;}
    let data=crate::jit::ShortCall{call,entry,ret,pcs:[pc,target,ret_pc],last,vidx:callee.vidx,ver:callee.ver};
    let fast=bus.fast_mem().is_some();
    let b=&mut cpu.blocks;let ops=&mut b.arena[caller.start as usize..(caller.start+u32::from(caller.n)) as usize];
    let old:Vec<_>=ops.iter().map(|i|i.off).collect();
    if let Some(code)=b.code.as_mut().unwrap().compile_short_call(ops,caller.pc,fast,&data) {
        b.entries[ei].code=code;b.entries[ei].short=2;b.compiled+=1;
    } else {
        // A cache close to its normal flush threshold keeps its old entry and offsets.
        for (i,off) in ops.iter_mut().zip(old) {i.off=off;}
        b.entries[ei].short=0;
    }
}

#[cfg(test)]
#[path="native_short_call/tests.rs"]
mod tests;
