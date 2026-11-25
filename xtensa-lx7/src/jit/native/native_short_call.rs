//! EX164 native experiment: execute a cached two-instruction callee, never skip its ISA work.
use super::*;

#[derive(Clone,Copy)]
pub(crate) struct ShortCall {
    pub call:BlockInsn,pub entry:BlockInsn,pub ret:BlockInsn,pub pcs:[u32;3],
    pub last:u32,pub vidx:[u32;2],pub ver:[u32;2],
}
impl CodeCache {
    /// Immutable metadata has the same owner/lifetime as its generated entry. Copy the typed
    /// object rather than constructing a byte slice over possibly uninitialized Rust padding.
    fn write_short_call(&mut self,data:&ShortCall)->Option<*const ShortCall> {
        let align=std::mem::align_of::<ShortCall>();
        let off=self.used.checked_add(align-1)? & !(align-1);
        let end=off.checked_add(std::mem::size_of::<ShortCall>())?;
        if end>self.size {return None;}
        // SAFETY: aligned, checked space in the exclusively owned live mapping. The stack
        // source is distinct. No generated code can run or reset this cache during compilation.
        let ptr=unsafe {
            let dst=self.base.add(off).cast::<ShortCall>();
            #[cfg(target_os="macos")] pthread_jit_write_protect_np(0);
            std::ptr::copy_nonoverlapping(data,dst,1);
            #[cfg(target_os="macos")] pthread_jit_write_protect_np(1);
            dst.cast_const()
        };
        self.used=end;Some(ptr)
    }
    pub(crate) fn compile_short_call(&mut self,insns:&mut [BlockInsn],pc:u32,fast:bool,data:&ShortCall)->Option<u32> {
        if self.remaining()<MAX_BLOCK_CODE+std::mem::size_of::<ShortCall>()+std::mem::align_of::<ShortCall>() {return None;}
        let ptr=self.write_short_call(data)?;
        compile_inner(self,insns,pc,fast,Some(ptr))
    }
}

/// Bits0/1 retain the ordinary trap/bus-stop protocol; bit2 is a pre-instruction trap.
/// Bits8..9 count the extra retired instructions, in addition to the already-charged CALL.
pub(super) extern "C" fn h_short_call<B:Bus>(cpu:*mut Cpu,bus:*mut B,data:*const ShortCall,pc:u32,room:u32)->u32 {
    // SAFETY: a record in the live owning cache, copied before borrowing that cache's Cpu.
    // The generated call passes the same exclusive CPU/bus pointers as h_window.
    let data=unsafe {*data};
    let flags=h_window::<B,0>(cpu,bus,&data.call,pc);
    if flags!=0 {return flags;}
    {
        // SAFETY: the preceding helper returned; no reference into Cpu or Bus remains live.
        let (cpu,bus)=unsafe {(&mut *cpu,&mut *bus)};
        if room<2 || cpu.pc!=data.pcs[1] || cpu.price_control || cpu.blocks.observed
            || cpu.boundary_bloom & (crate::block::pc_bit(data.pcs[1])|crate::block::pc_bit(data.pcs[2]))!=0 {return 0;}
        // A remapped page is refused even if it happens to have the same write version.
        if [bus.code_page(data.pcs[1]),bus.code_page(data.last)]!=data.vidx {return 0;}
        let pv=bus.page_versions();
        if data.vidx.iter().zip(data.ver).any(|(&i,v)|pv.get(i as usize)!=Some(&v)) {return 0;}
        // The approximate machine settles pending data/control prices after this original
        // CALL block. Crossing it would move CCOMPARE before/after ENTRY. Unknown buses
        // decline; observing this debt never consumes or requeues it.
        if cpu.timing_extra!=0 || bus.timing_penalty_pending()!=Some(false) || bus.block_break() {return 0;}
    }
    if data.entry.max_ar>=4 && h_overflow(cpu,u32::from(data.entry.max_ar),data.pcs[1])!=0 {return 4;}
    let flags=h_window::<B,2>(cpu,bus,&data.entry,data.pcs[1]);
    // SAFETY: h_window returned, so reading the exclusive Cpu pointer is disjoint from it.
    if flags!=0 || unsafe {(*cpu).pc}!=data.pcs[2] {return 1<<8|flags;}
    if data.ret.max_ar>=4 && h_overflow(cpu,u32::from(data.ret.max_ar),data.pcs[2])!=0 {return 1<<8|4;}
    2<<8|h_window::<B,3>(cpu,bus,&data.ret,data.pcs[2])
}
