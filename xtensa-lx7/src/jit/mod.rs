//! Code generation for scheduled blocks (AArch64 and WASM). A compiled block is the same straight-line
//! run `block.rs` interprets, with the same exit rules — cut by budget, timer bound handled by the
//! caller, `lend` checked after every fall-through instruction, exceptions and interrupt-line
//! changes leaving immediately — so the interpreter is the oracle and the two must produce
//! bit-identical machine state.
//!
//! The following register plan describes the AArch64 backend (AAPCS64 callee-saved, so helpers may be called freely):
//!   x19 = &Cpu, x20 = &Bus, x21 = &cpu.ar[0], w22 = windowbase*4, w23 = instructions left,
//!   x24 = TLB entries, x25 = &Helpers, w26 = cpu.lend, x27 = write-version counters,
//!   w28 = lend − block start, so the loop-end test is a compare with an immediate. The initial
//!   budget is kept in the frame at [sp, #96].
//! Guest register `n` lives at `ar[(w22 + n) & 63]`. Anything the fast path does not implement
//! is executed by calling back into `exec_insn` through `Helpers::exec`.
#![allow(clippy::too_many_arguments)]

#[cfg(all(target_arch = "aarch64", any(target_os = "macos", target_os = "linux")))]
pub use emu_core::jit_a64 as a64;

#[cfg(all(target_arch = "aarch64", any(target_os = "macos", target_os = "linux")))]
mod native {
    use super::a64::{Asm, Cond, Label, Reg, SP, ZR};
    use crate::block::BlockInsn;
    use crate::bus::{Bus, FastMem, TlbEntry, TLB_ENTRIES, TLB_INDEX_SHIFT, TLB_XOR_SHIFT, VPAGE_SHIFT};
    use crate::decode::Op;
    use crate::exec::{exec_insn, Trap};
    use crate::state::{exc, Cpu};
    use std::ffi::{c_int, c_void};

    pub const AVAILABLE: bool = true;
    pub const NONE: u32 = u32::MAX;

    // ------------------------------------------------------------------ executable memory
    extern "C" {
        fn mmap(addr: *mut c_void, len: usize, prot: c_int, flags: c_int, fd: c_int, off: i64) -> *mut c_void;
        #[cfg(target_os = "macos")] fn pthread_jit_write_protect_np(enabled: c_int);
        #[cfg(target_os = "macos")] fn sys_icache_invalidate(start: *mut c_void, len: usize);
        #[cfg(target_os = "linux")] fn __clear_cache(start: *mut c_void, end: *mut c_void);
    }

    pub struct CodeCache { base: *mut u8, size: usize, used: usize }
    // SAFETY: Moving the cache transfers ownership of its process-local mapping. Access to the
    // mapping remains synchronized through the cache.
    unsafe impl Send for CodeCache {}
    // SAFETY: Shared access cannot mutate the mapping, and generated code is published only while
    // the cache is exclusively borrowed.
    unsafe impl Sync for CodeCache {}

    impl CodeCache {
        pub fn new(size: usize) -> Option<CodeCache> {
            #[cfg(target_os = "macos")] let flags = 0x0002 | 0x1000 | 0x0800;   // PRIVATE | ANON | JIT
            #[cfg(target_os = "linux")] let flags = 0x0002 | 0x0020;            // PRIVATE | ANONYMOUS
            // SAFETY: A null address lets the operating system select the mapping. The length and
            // integer flags borrow no memory, and failure is checked before the pointer is stored.
            let p = unsafe { mmap(std::ptr::null_mut(), size, 7, flags, -1, 0) };
            if p as isize == -1 || p.is_null() { return None; }
            Some(CodeCache { base: p as *mut u8, size, used: 0 })
        }
        pub fn remaining(&self) -> usize { self.size - self.used }
        pub fn used(&self) -> usize { self.used }
        pub fn reset(&mut self) { self.used = 0; }
        /// Append machine code; returns its byte offset in the cache.
        fn write(&mut self, words: &[u32]) -> Option<u32> {
            let bytes = words.len() * 4;
            if self.used + bytes > self.size { return None; }
            let off = self.used;
            // SAFETY: The bounds check keeps the destination range inside the owned mapping. The
            // source slice is distinct, and the exclusive cache borrow prevents concurrent writes.
            // The cache-maintenance calls receive exactly the initialized code range.
            unsafe {
                #[cfg(target_os = "macos")] pthread_jit_write_protect_np(0);
                std::ptr::copy_nonoverlapping(words.as_ptr() as *const u8, self.base.add(off), bytes);
                #[cfg(target_os = "macos")] pthread_jit_write_protect_np(1);
                #[cfg(target_os = "macos")] sys_icache_invalidate(self.base.add(off) as *mut c_void, bytes);
                #[cfg(target_os = "linux")] __clear_cache(self.base.add(off) as *mut c_void, self.base.add(off + bytes) as *mut c_void);
            }
            self.used += bytes;
            Some(off as u32)
        }
        /// Copy the executable address without retaining a reference to this cache.
        /// Calling it requires the published-code and lifetime guarantees of `run`.
        pub fn entry_point(&self, off: u32) -> *const u8 {
            self.base.wrapping_add(off as usize)
        }
    }

    // ------------------------------------------------------------------ helpers called from generated code
    /// Function pointers the generated code calls; field order is the offset table below.
    #[repr(C)]
    pub struct Helpers { read8: *const (), read16: *const (), read32: *const (), write8: *const (), write16: *const (), write32: *const (), exec: *const (), raise_mem: *const (), overflow: *const (), pie: *const () }
    const H_READ8: u32 = 0; const H_READ16: u32 = 8; const H_READ32: u32 = 16;
    const H_WRITE8: u32 = 24; const H_WRITE16: u32 = 32; const H_WRITE32: u32 = 40;
    const H_EXEC: u32 = 48; const H_RAISE_MEM: u32 = 56; const H_OVERFLOW: u32 = 64; const H_PIE: u32 = 72;

    /// Loads return the value in the low word, bit 32 = fault, bit 33 = the bus wants the block to end.
    macro_rules! read_helper {
        ($name:ident, $f:ident) => {
            extern "C" fn $name<B: Bus>(bus: *mut B, addr: u32, pc: u32) -> u64 {
                // SAFETY: Compiled blocks pass the exclusive bus pointer supplied to `run`, and
                // the helper returns before generated code resumes.
                let bus = unsafe { &mut *bus };
                bus.note_pc(pc);
                match bus.$f(addr) { Ok(v) => v as u64 | (bus.block_break() as u64) << 33, Err(_) => 1 << 32 }
            }
        };
    }
    read_helper!(h_read8, read8);
    read_helper!(h_read16, read16);
    read_helper!(h_read32, read32);
    /// Stores return bit 0 = fault, bit 1 = block must end.
    macro_rules! write_helper {
        ($name:ident, $f:ident, $t:ty) => {
            extern "C" fn $name<B: Bus>(bus: *mut B, addr: u32, v: u32, pc: u32) -> u32 {
                // SAFETY: Compiled blocks pass the exclusive bus pointer supplied to `run`, and
                // the helper returns before generated code resumes.
                let bus = unsafe { &mut *bus };
                bus.note_pc(pc);
                match bus.$f(addr, v as $t) { Ok(()) => (bus.block_break() as u32) << 1, Err(_) => 1 }
            }
        };
    }
    write_helper!(h_write8, write8, u8);
    write_helper!(h_write16, write16, u16);
    write_helper!(h_write32, write32, u32);
    /// Run one instruction through the interpreter. Bit 0 = trap (stored in `cpu.jit_trap`), bit 1 = block must end.
    extern "C" fn h_exec<B: Bus>(cpu: *mut Cpu, bus: *mut B, insn: *const BlockInsn, pc: u32) -> u32 {
        // SAFETY: The CPU and bus pointers are the exclusive pointers supplied to `run`, and the
        // instruction points into the stable block arena for the duration of that call.
        // Copy before borrowing Cpu: its owned arena must not remain shared through i.
        let i = unsafe { *insn };
        let (cpu, bus) = unsafe { (&mut *cpu, &mut *bus) };
        cpu.pc = pc;
        bus.note_pc(pc);
        match exec_insn(cpu, bus, &i.insn) { Ok(()) => (bus.block_break() as u32) << 1, Err(t) => { cpu.jit_trap = Some(t); 1 } }
    }
    /// A PIE instruction straight into `pie::exec`, skipping `exec_insn`'s dispatch. PIE never
    /// transfers control, so its tail is `exec_insn`'s fall-through tail: the zero-overhead loop
    /// back-edge, then the PC. Same calling convention and result bits as `h_exec`.
    extern "C" fn h_pie<B: Bus>(cpu: *mut Cpu, bus: *mut B, insn: *const BlockInsn, pc: u32) -> u32 {
        // SAFETY: as for h_exec: exclusive CPU and bus pointers from `run`, and an instruction in
        // the stable block arena, copied before the CPU is borrowed.
        let i = unsafe { *insn };
        let (cpu, bus) = unsafe { (&mut *cpu, &mut *bus) };
        cpu.pc = pc;
        bus.note_pc(pc);
        match crate::pie::exec(cpu, bus, &i.insn) {
            Ok(()) => {
                let mut next = pc.wrapping_add(i.insn.len as u32);
                if next == cpu.lend && cpu.lcount != 0 {
                    cpu.lcount = cpu.lcount.wrapping_sub(1);
                    next = cpu.lbeg;
                }
                cpu.pc = next;
                (bus.block_break() as u32) << 1
            }
            Err(t) => { cpu.jit_trap = Some(t); 1 }
        }
    }
    extern "C" fn h_raise_mem(cpu: *mut Cpu, cause: u32, addr: u32, pc: u32) {
        // SAFETY: Compiled blocks pass the exclusive CPU pointer supplied to `run`, and the helper
        // returns before generated code resumes.
        let cpu = unsafe { &mut *cpu };
        cpu.pc = pc;
        let t = cpu.raise_mem(cause, addr);
        cpu.jit_trap = Some(t);
    }
    extern "C" fn h_overflow(cpu: *mut Cpu, max_ar: u32, pc: u32) -> u32 {
        // SAFETY: Compiled blocks pass the exclusive CPU pointer supplied to `run`, and the helper
        // returns before generated code resumes.
        let cpu = unsafe { &mut *cpu };
        cpu.pc = pc;
        match cpu.check_overflow(max_ar as u8) { Some(t) => { cpu.jit_trap = Some(t); 1 } None => 0 }
    }

    impl Helpers {
        pub const fn new<B: Bus>() -> Helpers {
            Helpers {
                read8: h_read8::<B> as *const (), read16: h_read16::<B> as *const (), read32: h_read32::<B> as *const (),
                write8: h_write8::<B> as *const (), write16: h_write16::<B> as *const (), write32: h_write32::<B> as *const (),
                exec: h_exec::<B> as *const (), raise_mem: h_raise_mem as *const (), overflow: h_overflow as *const (),
                pie: h_pie::<B> as *const (),
            }
        }

        /// Promote the immutable table for this concrete Bus type outside any Cpu.
        /// Pointer-valued fields allow constant evaluation without exposing addresses.
        pub fn shared<B: Bus>() -> &'static Helpers {
            &const { Helpers::new::<B>() }
        }
    }

    // The generated AArch64 loads use these byte offsets and pointer width directly.
    const _: () = {
        assert!(std::mem::size_of::<Helpers>() == 80);
        assert!(std::mem::align_of::<Helpers>() == 8);
        assert!(std::mem::offset_of!(Helpers, read8) == H_READ8 as usize);
        assert!(std::mem::offset_of!(Helpers, read16) == H_READ16 as usize);
        assert!(std::mem::offset_of!(Helpers, read32) == H_READ32 as usize);
        assert!(std::mem::offset_of!(Helpers, write8) == H_WRITE8 as usize);
        assert!(std::mem::offset_of!(Helpers, write16) == H_WRITE16 as usize);
        assert!(std::mem::offset_of!(Helpers, write32) == H_WRITE32 as usize);
        assert!(std::mem::offset_of!(Helpers, exec) == H_EXEC as usize);
        assert!(std::mem::offset_of!(Helpers, raise_mem) == H_RAISE_MEM as usize);
        assert!(std::mem::offset_of!(Helpers, overflow) == H_OVERFLOW as usize);
        assert!(std::mem::offset_of!(Helpers, pie) == H_PIE as usize);
    };

    // ------------------------------------------------------------------ the compiler
    const OFF_PC: u32 = std::mem::offset_of!(Cpu, pc) as u32;
    const OFF_AR: u32 = std::mem::offset_of!(Cpu, ar) as u32;
    const OFF_WB: u32 = std::mem::offset_of!(Cpu, windowbase) as u32;
    const OFF_WS: u32 = std::mem::offset_of!(Cpu, windowstart) as u32;
    const OFF_SAR: u32 = std::mem::offset_of!(Cpu, sar) as u32;
    const OFF_LBEG: u32 = std::mem::offset_of!(Cpu, lbeg) as u32;
    const OFF_LEND: u32 = std::mem::offset_of!(Cpu, lend) as u32;
    const OFF_LCOUNT: u32 = std::mem::offset_of!(Cpu, lcount) as u32;
    const OFF_BR: u32 = std::mem::offset_of!(Cpu, br) as u32;

    const TLB_INDEX_BITS: u32 = TLB_ENTRIES.ilog2();
    /// Scale an index by the entry size with shifted adds: size == TLB_ENTRY_ODD << TLB_ENTRY_SHIFT.
    const TLB_ENTRY_SHIFT: u32 = std::mem::size_of::<TlbEntry>().trailing_zeros();
    const TLB_ENTRY_ODD: u32 = (std::mem::size_of::<TlbEntry>() >> TLB_ENTRY_SHIFT) as u32;
    const TLB_LO: u32 = std::mem::offset_of!(TlbEntry, lo) as u32;
    const TLB_HI: u32 = std::mem::offset_of!(TlbEntry, hi) as u32;
    const TLB_BASE: u32 = std::mem::offset_of!(TlbEntry, base) as u32;
    const TLB_VBASE: u32 = std::mem::offset_of!(TlbEntry, vbase) as u32;
    const TLB_WRITABLE: u32 = std::mem::offset_of!(TlbEntry, writable) as u32;
    // Generated instructions require power-of-two indexing and encodable shifts/offsets.
    const _: () = {
        assert!(TLB_ENTRIES.is_power_of_two() && TLB_INDEX_BITS > 0 && TLB_INDEX_BITS < 32);
        assert!(matches!(TLB_ENTRY_ODD, 1 | 5) && TLB_ENTRY_SHIFT < 64);   // 1 << k, or 5 << k with one add
        assert!(TLB_INDEX_SHIFT < 32 && TLB_XOR_SHIFT < 32);
        assert!(VPAGE_SHIFT >= 3 && VPAGE_SHIFT <= 12); // page edge test uses a 12-bit immediate
        assert!(TLB_LO.is_multiple_of(4) && TLB_LO < 16384 && TLB_HI.is_multiple_of(4) && TLB_HI < 16384);
        assert!(TLB_BASE.is_multiple_of(8) && TLB_BASE < 32768);
        assert!(TLB_VBASE.is_multiple_of(4) && TLB_VBASE < 16384);
        assert!(TLB_WRITABLE.is_multiple_of(2) && TLB_WRITABLE < 8192);
    };

    const CPU: Reg = 19; const BUS: Reg = 20; const AR: Reg = 21; const WB4: Reg = 22; const LEFT: Reg = 23;
    const TLB: Reg = 24; const HELP: Reg = 25; const LEND: Reg = 26; const PVER: Reg = 27; const LOFF: Reg = 28;
    const FRAME: i32 = 112; const BUDGET_SLOT: u32 = 96;
    const IDX: Reg = 17;
    const EXIT_END: u32 = 0; const EXIT_LEFT: u32 = 1; const EXIT_TRAP: u32 = 2; const EXIT_CUT: u32 = 3; const EXIT_TRAP_PRE: u32 = 4;
    /// Most code one block can need, with slack; the cache is flushed when less than this is free.
    pub const MAX_BLOCK_CODE: usize = 24 * 1024;

    type Stub<'a> = Box<dyn FnOnce(&mut Asm) + 'a>;

    struct Gen<'a> {
        a: Asm,
        exit_trap: Label, exit_trap_pre: Label, exit_left: Label,
        /// shared tails: `w9` holds the pc to store, then leave with EXIT_CUT / EXIT_LEFT
        cut_w9: Label, left_w9: Label, loop_shared: Label,
        pc0: u32,
        stubs: Vec<Stub<'a>>,
    }

    impl<'a> Gen<'a> {
        fn ld_ar(&mut self, dst: Reg, n: u8) { self.a.add_imm(IDX, WB4, n as u32); self.a.and_mask(IDX, IDX, 6, 0); self.a.ldr_idx(dst, AR, IDX); }
        fn st_ar(&mut self, n: u8, src: Reg) { self.a.add_imm(IDX, WB4, n as u32); self.a.and_mask(IDX, IDX, 6, 0); self.a.str_idx(src, AR, IDX); }
        fn set_pc(&mut self, pc: u32) { self.a.mov32(9, pc); self.a.str(9, CPU, OFF_PC); }
        fn call(&mut self, h: u32) { self.a.ldr_x(9, HELP, h); self.a.blr(9); }
        /// Out-of-line stub that leaves the block at `pc` with EXIT_LEFT (taken branch, bus break).
        fn left_stub(&mut self, pc: u32) -> Label {
            let l = self.a.label();
            let tail = self.left_w9;
            self.stubs.push(Box::new(move |a: &mut Asm| { a.bind(l); a.mov32(9, pc); a.b(tail); }));
            l
        }
        fn cut_stub(&mut self, pc: u32) -> Label {
            let l = self.a.label();
            let tail = self.cut_w9;
            self.stubs.push(Box::new(move |a: &mut Asm| { a.bind(l); a.mov32(9, pc); a.b(tail); }));
            l
        }
        fn load_loff(&mut self) { self.a.mov32(9, self.pc0); self.a.sub(LOFF, LEND, 9); }
        /// Probe the software TLB for `size` bytes at `w_addr`. On a hit: x9 = &entry,
        /// x12 = host base of the entry, w10 = offset of the access within it. Otherwise jumps to `slow`.
        fn tlb_probe(&mut self, addr: Reg, size: u32, slow: Label) {
            self.a.lsr_imm(9, addr, TLB_INDEX_SHIFT); self.a.eor_lsr(9, 9, addr, TLB_XOR_SHIFT); self.a.and_mask(9, 9, TLB_INDEX_BITS, 0);
            // The masked index is below 2^32 / size, so the 32-bit scaling add cannot overflow.
            if TLB_ENTRY_ODD == 5 { self.a.add_lsl(9, 9, 9, 2); }
            self.a.add_x_lsl(9, TLB, 9, TLB_ENTRY_SHIFT);
            self.a.ldr(10, 9, TLB_LO); self.a.ldr(11, 9, TLB_HI);
            self.a.cmp(addr, 10); self.a.b_cond(Cond::Lo, slow);
            self.a.cmp(addr, 11); self.a.b_cond(Cond::Hs, slow);
            // Subtract only after ordering the endpoints: addr + size could wrap at 4 GiB.
            self.a.sub(13, 11, addr); self.a.cmp_imm(13, size); self.a.b_cond(Cond::Lo, slow);
            self.a.sub(10, addr, 10);                                             // offset
            self.a.ldr_x(12, 9, TLB_BASE);
        }
        fn reload_after_helper(&mut self) { self.a.ldr(WB4, CPU, OFF_WB); self.a.lsl_imm(WB4, WB4, 2); self.a.ldr(LEND, CPU, OFF_LEND); self.load_loff(); }
    }

    /// Compile one block. Fills `insns[i].off` with each instruction's byte offset from the body
    /// start (the entry-point argument of `run`) and returns the code's offset in the cache.
    pub fn compile(cc: &mut CodeCache, insns: &mut [BlockInsn], pc0: u32, fast: bool) -> Option<u32> {
        if cc.remaining() < MAX_BLOCK_CODE { return None; }
        let mut a = Asm::new();
        let (exit, exit_trap, exit_trap_pre, exit_left) = (a.label(), a.label(), a.label(), a.label());
        let body = a.label();
        // prologue
        a.stp_pre(29, 30, SP, -FRAME);
        a.stp(19, 20, SP, 16); a.stp(21, 22, SP, 32); a.stp(23, 24, SP, 48); a.stp(25, 26, SP, 64); a.stp(27, 28, SP, 80);
        a.mov_x(CPU, 0); a.mov_x(BUS, 1); a.mov_x(HELP, 2); a.mov(LEFT, 3); a.str(3, SP, BUDGET_SLOT);
        a.mov_x(TLB, 5); a.mov_x(PVER, 6);
        a.add_imm_x(AR, CPU, OFF_AR);
        a.ldr(WB4, CPU, OFF_WB); a.lsl_imm(WB4, WB4, 2);
        a.ldr(LEND, CPU, OFF_LEND);
        a.mov32(9, pc0); a.sub(LOFF, LEND, 9);
        a.mov(4, 4);                                   // zero-extend the entry offset
        a.adr(9, body); a.add_x(9, 9, 4); a.br(9);
        a.bind(body);
        let body_at = a.here();
        let (cut_w9, left_w9, loop_shared) = (a.label(), a.label(), a.label());
        let mut g = Gen { a, exit_trap, exit_trap_pre, exit_left, cut_w9, left_w9, loop_shared, pc0, stubs: Vec::new() };
        // windows already verified free within this block (reset when a helper may have rotated them)
        let mut checked_w: u32 = 0;

        let mut pc = pc0;
        for block_insn in insns.iter_mut() {
            let i = block_insn.insn;
            let max_ar = block_insn.max_ar;
            let next = pc.wrapping_add(i.len as u32);
            block_insn.off = ((g.a.here() - body_at) * 4) as u32;
            // budget: cut before this instruction if none left
            let cut = g.cut_stub(pc);
            g.a.cbz(LEFT, cut);
            // window overflow pre-check (exact decision in the helper); windowstart cannot change
            // between two instructions of a block unless a helper ran, so each frame count is
            // verified once
            if max_ar >= 4 && (max_ar / 4) as u32 > checked_w {
                let w = (max_ar / 4) as u32;
                checked_w = w;
                let ovf = g.a.label(); let back = g.a.label();
                g.a.ldr(9, CPU, OFF_WS); g.a.orr_lsl(9, 9, 9, 16);
                g.a.add_imm(10, WB4, 4); g.a.lsr_imm(10, 10, 2);
                g.a.lsrv(9, 9, 10); g.a.tst_mask(9, w, 0); g.a.b_cond(Cond::Ne, ovf);
                g.a.bind(back);
                let (tp, mar) = (g.exit_trap_pre, max_ar as u32);
                g.stubs.push(Box::new(move |a: &mut Asm| {
                    a.bind(ovf); a.mov_x(0, CPU); a.movz(1, mar, 0); a.mov32(2, pc);
                    a.ldr_x(9, HELP, H_OVERFLOW); a.blr(9); a.cbnz(0, tp); a.b(back);
                }));
            }
            g.a.sub_imm(LEFT, LEFT, 1);

            // the instruction; `flag` = register holding "bus wants the block to end" (bit 0), if any
            let (r, s, t, imm, imm2) = (i.r, i.s, i.t, i.imm, i.imm2);
            let immu = imm as u32;
            let mut flag: Option<Reg> = None;
            let mut fell_through = true;          // false for unconditional transfers and fallback (they handle pc themselves)
            use Op::*;
            match i.op {
                Nop | NopN | Isync | Rsync | Esync | Dsync | Excw | Memw | Extw
                | Dpfr | Dpfw | Dpfro | Dpfwo | Dhwb | Dhwbi | Dhi | Dii | Ipf | Ihi | Iii | Ipfl | Ihu | Iiu | Dpfl | Dhu | Diu => {}
                Movi => { g.a.mov32(11, immu); g.st_ar(t, 11); }
                MoviN => { g.a.mov32(11, immu); g.st_ar(s, 11); }
                Mov | MovN => { g.ld_ar(11, s); g.st_ar(t, 11); }
                Add | AddN => { g.ld_ar(9, s); g.ld_ar(10, t); g.a.add(11, 9, 10); g.st_ar(r, 11); }
                Sub => { g.ld_ar(9, s); g.ld_ar(10, t); g.a.sub(11, 9, 10); g.st_ar(r, 11); }
                Addi | Addmi => { g.ld_ar(9, s); g.a.add_imm32(11, 9, immu, 10); g.st_ar(t, 11); }
                AddiN => { g.ld_ar(9, s); g.a.add_imm32(11, 9, immu, 10); g.st_ar(r, 11); }
                Addx2 | Addx4 | Addx8 => { let sh = match i.op { Addx2 => 1, Addx4 => 2, _ => 3 }; g.ld_ar(9, s); g.ld_ar(10, t); g.a.add_lsl(11, 10, 9, sh); g.st_ar(r, 11); }
                Subx2 | Subx4 | Subx8 => { let sh = match i.op { Subx2 => 1, Subx4 => 2, _ => 3 }; g.ld_ar(9, s); g.ld_ar(10, t); g.a.lsl_imm(9, 9, sh); g.a.sub(11, 9, 10); g.st_ar(r, 11); }
                And => { g.ld_ar(9, s); g.ld_ar(10, t); g.a.and(11, 9, 10); g.st_ar(r, 11); }
                Or => { g.ld_ar(9, s); g.ld_ar(10, t); g.a.orr(11, 9, 10); g.st_ar(r, 11); }
                Xor => { g.ld_ar(9, s); g.ld_ar(10, t); g.a.eor(11, 9, 10); g.st_ar(r, 11); }
                Neg => { g.ld_ar(10, t); g.a.neg(11, 10); g.st_ar(r, 11); }
                Abs => { g.ld_ar(10, t); g.a.cmp_imm(10, 0); g.a.cneg(11, 10, Cond::Mi); g.st_ar(r, 11); }
                Extui => {
                    g.ld_ar(10, t);
                    let (sh, w) = (immu & 31, imm2 as u32);
                    if w >= 32 { g.a.lsr_imm(11, 10, sh); } else if sh + w <= 32 { g.a.ubfx(11, 10, sh, w); } else { g.a.lsr_imm(11, 10, sh); }
                    g.st_ar(r, 11);
                }
                Sext => { g.ld_ar(9, s); g.a.sbfx(11, 9, 0, immu + 1); g.st_ar(r, 11); }
                Slli => { g.ld_ar(9, s); g.a.lsl_imm(11, 9, immu & 31); g.st_ar(r, 11); }
                Srai => { g.ld_ar(10, t); g.a.asr_imm(11, 10, immu & 31); g.st_ar(r, 11); }
                Srli => { g.ld_ar(10, t); g.a.lsr_imm(11, 10, immu & 31); g.st_ar(r, 11); }
                Ssr => { g.ld_ar(9, s); g.a.and_mask(9, 9, 5, 0); g.a.str(9, CPU, OFF_SAR); }
                Ssl => { g.ld_ar(9, s); g.a.and_mask(9, 9, 5, 0); g.a.movz(10, 32, 0); g.a.sub(9, 10, 9); g.a.str(9, CPU, OFF_SAR); }
                Ssa8l => { g.ld_ar(9, s); g.a.and_mask(9, 9, 2, 0); g.a.lsl_imm(9, 9, 3); g.a.str(9, CPU, OFF_SAR); }
                Ssa8b => { g.ld_ar(9, s); g.a.and_mask(9, 9, 2, 0); g.a.lsl_imm(9, 9, 3); g.a.movz(10, 32, 0); g.a.sub(9, 10, 9); g.a.str(9, CPU, OFF_SAR); }
                Ssai => { g.a.movz(9, immu & 31, 0); g.a.str(9, CPU, OFF_SAR); }
                Sll => { g.ld_ar(10, s); g.a.ldr(9, CPU, OFF_SAR); g.a.movz(12, 32, 0); g.a.sub(9, 12, 9); g.a.and_mask(9, 9, 6, 0); g.a.lslv(11, 10, 9); g.a.cmp_imm(9, 32); g.a.csel(11, ZR, 11, Cond::Hs); g.st_ar(r, 11); }
                Srl => { g.ld_ar(10, t); g.a.ldr(9, CPU, OFF_SAR); g.a.lsrv(11, 10, 9); g.a.cmp_imm(9, 32); g.a.csel(11, ZR, 11, Cond::Hs); g.st_ar(r, 11); }
                Sra => { g.ld_ar(10, t); g.a.ldr(9, CPU, OFF_SAR); g.a.asrv(11, 10, 9); g.a.asr_imm(12, 10, 31); g.a.cmp_imm(9, 32); g.a.csel(11, 12, 11, Cond::Hs); g.st_ar(r, 11); }
                Src => { g.ld_ar(9, s); g.ld_ar(10, t); g.a.ldr(12, CPU, OFF_SAR); g.a.and_mask(12, 12, 6, 0); g.a.lsl_imm_x(11, 9, 32); g.a.orr_x(11, 11, 10); g.a.lsrv_x(11, 11, 12); g.st_ar(r, 11); }
                Nsau => { g.ld_ar(9, s); g.a.clz(11, 9); g.st_ar(t, 11); }
                Min | Max | Minu | Maxu => {
                    let c = match i.op { Min => Cond::Lt, Max => Cond::Gt, Minu => Cond::Lo, _ => Cond::Hi };
                    g.ld_ar(9, s); g.ld_ar(10, t); g.a.cmp(9, 10); g.a.csel(11, 9, 10, c); g.st_ar(r, 11);
                }
                Moveqz | Movnez | Movltz | Movgez => {
                    let skip = g.a.label();
                    g.ld_ar(10, t);
                    match i.op { Moveqz => g.a.cbnz(10, skip), Movnez => g.a.cbz(10, skip), Movltz => g.a.tbz(10, 31, skip), _ => g.a.tbnz(10, 31, skip) }
                    g.ld_ar(9, s); g.st_ar(r, 9); g.a.bind(skip);
                }
                Movf | Movt => {
                    let skip = g.a.label();
                    g.a.ldr(9, CPU, OFF_BR);
                    if i.op == Movf { g.a.tbnz(9, t as u32, skip); } else { g.a.tbz(9, t as u32, skip); }
                    g.ld_ar(9, s); g.st_ar(r, 9); g.a.bind(skip);
                }
                Mull => { g.ld_ar(9, s); g.ld_ar(10, t); g.a.mul(11, 9, 10); g.st_ar(r, 11); }
                Muluh => { g.ld_ar(9, s); g.ld_ar(10, t); g.a.umull(11, 9, 10); g.a.lsr_imm_x(11, 11, 32); g.st_ar(r, 11); }
                Mulsh => { g.ld_ar(9, s); g.ld_ar(10, t); g.a.smull(11, 9, 10); g.a.asr_imm_x(11, 11, 32); g.st_ar(r, 11); }
                Mul16u => { g.ld_ar(9, s); g.ld_ar(10, t); g.a.uxth(9, 9); g.a.uxth(10, 10); g.a.mul(11, 9, 10); g.st_ar(r, 11); }
                Mul16s => { g.ld_ar(9, s); g.ld_ar(10, t); g.a.sxth(9, 9); g.a.sxth(10, 10); g.a.mul(11, 9, 10); g.st_ar(r, 11); }
                // A zero divisor takes the interpreter, which raises IntegerDivideByZero as the
                // fallback below would. sdiv gives i32::MIN / -1 = i32::MIN and msub then 0,
                // what wrapping_div / wrapping_rem give.
                Quou | Quos | Remu | Rems => {
                    let slow = g.a.label();
                    g.ld_ar(9, s); g.ld_ar(10, t);
                    g.a.cbz(10, slow);
                    if matches!(i.op, Quou | Remu) { g.a.udiv(11, 9, 10); } else { g.a.sdiv(11, 9, 10); }
                    if matches!(i.op, Remu | Rems) { g.a.msub(11, 11, 10, 9); }
                    g.st_ar(r, 11);
                    let (tr, el, insn) = (g.exit_trap, g.exit_left, (&raw const *block_insn) as u64);
                    g.stubs.push(Box::new(move |a: &mut Asm| {
                        a.bind(slow);
                        a.mov_x(0, CPU); a.mov_x(1, BUS);
                        a.mov64(2, insn);
                        a.mov32(3, pc);
                        a.ldr_x(9, HELP, H_EXEC); a.blr(9);
                        a.tbnz(0, 0, tr);
                        a.b(el);                                // cpu.pc is the interpreter's
                    }));
                }
                Salt | Saltu => { g.ld_ar(9, s); g.ld_ar(10, t); g.a.cmp(9, 10); g.a.cset(11, if i.op == Salt { Cond::Lt } else { Cond::Lo }); g.st_ar(r, 11); }

                // ---- loads and stores through the bus helpers
                L8ui | L16ui | L16si | L32i | L32iN | L32ai | L32r => {
                    let (h, size) = match i.op { L8ui => (H_READ8, 1), L16ui | L16si => (H_READ16, 2), _ => (H_READ32, 4) };
                    // address in w1 (kept intact for the slow path)
                    if i.op == L32r { g.a.mov32(1, immu); } else { g.ld_ar(1, s); g.a.add_imm32(1, 1, immu, 9); }
                    let (slow, done) = (g.a.label(), g.a.label());
                    if fast {
                        g.tlb_probe(1, size, slow);                               // x12 = entry base, w10 = offset
                        match i.op { L8ui => g.a.ldrb_u(0, 12, 10), L16ui => g.a.ldrh_u(0, 12, 10), L16si => g.a.ldrsh_u(0, 12, 10), _ => g.a.ldr_u(0, 12, 10) }
                        g.st_ar(t, 0);
                        g.a.movz(12, 0, 0);
                        g.a.b(done);
                    } else { g.a.b(slow); }
                    flag = Some(12);
                    let (tr, op) = (g.exit_trap, i.op);
                    g.stubs.push(Box::new(move |a: &mut Asm| {
                        a.bind(slow);
                        a.mov_x(0, BUS); a.mov32(2, pc);
                        a.ldr_x(9, HELP, h); a.blr(9);
                        let fault = a.label();
                        a.tbnz(0, 32, fault);
                        a.lsr_imm_x(12, 0, 33);
                        if op == Op::L16si { a.sxth(0, 0); }
                        // st_ar(t, w0)
                        a.add_imm(IDX, WB4, t as u32); a.and_mask(IDX, IDX, 6, 0); a.str_idx(0, AR, IDX);
                        a.b(done);
                        a.bind(fault);                                          // recompute the address: nothing changed
                        a.mov_x(0, CPU); a.mov32(1, exc::LOAD_PROHIBITED);
                        if op == Op::L32r { a.mov32(2, immu); } else { a.add_imm(IDX, WB4, s as u32); a.and_mask(IDX, IDX, 6, 0); a.ldr_idx(2, AR, IDX); a.add_imm32(2, 2, immu, 9); }
                        a.mov32(3, pc);
                        a.ldr_x(9, HELP, H_RAISE_MEM); a.blr(9); a.b(tr);
                    }));
                    g.a.bind(done);
                }
                S8i | S16i | S32i | S32iN | S32ri => {
                    let (h, size) = match i.op { S8i => (H_WRITE8, 1), S16i => (H_WRITE16, 2), _ => (H_WRITE32, 4) };
                    g.ld_ar(1, s); g.a.add_imm32(1, 1, immu, 9);              // w1 = address, w2 = value
                    g.ld_ar(2, t);
                    let (slow, done) = (g.a.label(), g.a.label());
                    if fast {
                        g.tlb_probe(1, size, slow);                               // x12 = entry base, w10 = offset, x9 = entry
                        g.a.ldrh(11, 9, TLB_WRITABLE); g.a.cbz(11, slow);
                        // stay on the fast path only when the write-version bump touches one page
                        // and not its first three bytes (an instruction may straddle into it)
                        g.a.and_mask(13, 10, VPAGE_SHIFT, 0); g.a.sub_imm(13, 13, emu_core::bus::PREV_PAGE_BYTES); g.a.cmp_imm(13, (1 << VPAGE_SHIFT) - emu_core::bus::PREV_PAGE_BYTES - size); g.a.b_cond(Cond::Hi, slow);
                        match i.op { S8i => g.a.strb_u(2, 12, 10), S16i => g.a.strh_u(2, 12, 10), _ => g.a.str_u(2, 12, 10) }
                        // Retain unconditional native version bumps; watched-code gating is
                        // used by the WASM backend.
                        g.a.ldr(11, 9, TLB_VBASE); g.a.add_lsr(11, 11, 10, VPAGE_SHIFT);
                        g.a.ldr_idx(13, PVER, 11); g.a.add_imm(13, 13, 1); g.a.str_idx(13, PVER, 11);
                        g.a.movz(12, 0, 0);
                        g.a.b(done);
                    } else { g.a.b(slow); }
                    flag = Some(12);
                    let tr = g.exit_trap;
                    g.stubs.push(Box::new(move |a: &mut Asm| {
                        a.bind(slow);
                        a.mov_x(0, BUS); a.mov32(3, pc);
                        a.ldr_x(9, HELP, h); a.blr(9);
                        let fault = a.label();
                        a.tbnz(0, 0, fault);
                        a.lsr_imm(12, 0, 1);
                        a.b(done);
                        a.bind(fault);
                        a.mov_x(0, CPU); a.mov32(1, exc::STORE_PROHIBITED);
                        a.add_imm(IDX, WB4, s as u32); a.and_mask(IDX, IDX, 6, 0); a.ldr_idx(2, AR, IDX); a.add_imm32(2, 2, immu, 9);
                        a.mov32(3, pc);
                        a.ldr_x(9, HELP, H_RAISE_MEM); a.blr(9); a.b(tr);
                    }));
                    g.a.bind(done);
                }

                // ---- control transfers
                J => { let tk = g.left_stub(immu); g.a.b(tk); fell_through = false; }
                Beqz | BeqzN | Bnez | BnezN | Bltz | Bgez | Beqi | Bnei | Blti | Bgei | Bltui | Bgeui
                | Bnone | Bany | Ball | Bnall | Beq | Bne | Blt | Bge | Bltu | Bgeu | Bbc | Bbs | Bbci | Bbsi | Bf | Bt => {
                    let tk = g.left_stub(immu);
                    match i.op {
                        Beqz | BeqzN => { g.ld_ar(9, s); g.a.cbz(9, tk); }
                        Bnez | BnezN => { g.ld_ar(9, s); g.a.cbnz(9, tk); }
                        Bltz => { g.ld_ar(9, s); g.a.tbnz(9, 31, tk); }
                        Bgez => { g.ld_ar(9, s); g.a.tbz(9, 31, tk); }
                        Beqi | Bnei | Blti | Bgei | Bltui | Bgeui => {
                            let c = match i.op { Beqi => Cond::Eq, Bnei => Cond::Ne, Blti => Cond::Lt, Bgei => Cond::Ge, Bltui => Cond::Lo, _ => Cond::Hs };
                            g.ld_ar(9, s); g.a.mov32(10, imm2 as u32); g.a.cmp(9, 10); g.a.b_cond(c, tk);
                        }
                        Bnone | Bany => { g.ld_ar(9, s); g.ld_ar(10, t); g.a.tst(9, 10); g.a.b_cond(if i.op == Bnone { Cond::Eq } else { Cond::Ne }, tk); }
                        Ball | Bnall => { g.ld_ar(9, s); g.ld_ar(10, t); g.a.bic(9, 10, 9); if i.op == Ball { g.a.cbz(9, tk); } else { g.a.cbnz(9, tk); } }
                        Beq | Bne | Blt | Bge | Bltu | Bgeu => {
                            let c = match i.op { Beq => Cond::Eq, Bne => Cond::Ne, Blt => Cond::Lt, Bge => Cond::Ge, Bltu => Cond::Lo, _ => Cond::Hs };
                            g.ld_ar(9, s); g.ld_ar(10, t); g.a.cmp(9, 10); g.a.b_cond(c, tk);
                        }
                        Bbc | Bbs => { g.ld_ar(9, s); g.ld_ar(10, t); g.a.and_mask(10, 10, 5, 0); g.a.lsrv(9, 9, 10); if i.op == Bbc { g.a.tbz(9, 0, tk); } else { g.a.tbnz(9, 0, tk); } }
                        Bbci | Bbsi => { g.ld_ar(9, s); if i.op == Bbci { g.a.tbz(9, imm2 as u32, tk); } else { g.a.tbnz(9, imm2 as u32, tk); } }
                        _ => { g.a.ldr(9, CPU, OFF_BR); if i.op == Bf { g.a.tbz(9, s as u32, tk); } else { g.a.tbnz(9, s as u32, tk); } }
                    }
                }

                // ---- everything else: the interpreter
                _ => {
                    g.a.mov_x(0, CPU); g.a.mov_x(1, BUS);
                    g.a.mov64(2, (&raw const *block_insn) as u64);
                    g.a.mov32(3, pc);
                    g.call(if i.op == crate::decode::Op::Pie { H_PIE } else { H_EXEC });
                    let tr = g.exit_trap;
                    g.a.tbnz(0, 0, tr);
                    g.a.lsr_imm(12, 0, 1);
                    g.reload_after_helper();
                    g.a.ldr(9, CPU, OFF_PC); g.a.mov32(10, next); g.a.cmp(9, 10);
                    let el = g.exit_left;
                    g.a.b_cond(Cond::Ne, el);
                    g.a.cbnz(12, el);
                    fell_through = false;
                    checked_w = 0;
                }
            }

            // zero-overhead loop back-edge on fall-through, then the bus's request to stop
            if fell_through {
                let cont = g.a.label();
                g.a.cmp_imm(LOFF, next.wrapping_sub(pc0)); g.a.b_cond(Cond::Ne, cont);
                g.a.ldr(9, CPU, OFF_LCOUNT); g.a.cbz(9, cont);
                let ls = g.loop_shared; g.a.b(ls);
                g.a.bind(cont);
                if let Some(f) = flag { let nx = g.left_stub(next); g.a.cbnz(f, nx); }
            }
            pc = next;
        }
        // fell off the end of the block
        g.set_pc(pc); g.a.movz(0, EXIT_END, 0); g.a.b(exit);

        // shared exits
        g.a.bind(exit_trap); g.a.movz(0, EXIT_TRAP, 0); g.a.b(exit);
        g.a.bind(exit_trap_pre); g.a.movz(0, EXIT_TRAP_PRE, 0); g.a.b(exit);
        g.a.bind(exit_left); g.a.movz(0, EXIT_LEFT, 0); g.a.b(exit);
        g.a.bind(loop_shared);                                             // lcount != 0 and next == lend
        g.a.ldr(9, CPU, OFF_LCOUNT); g.a.sub_imm(9, 9, 1); g.a.str(9, CPU, OFF_LCOUNT);
        g.a.ldr(9, CPU, OFF_LBEG);
        g.a.bind(left_w9); g.a.str(9, CPU, OFF_PC); g.a.movz(0, EXIT_LEFT, 0); g.a.b(exit);
        g.a.bind(cut_w9); g.a.str(9, CPU, OFF_PC); g.a.movz(0, EXIT_CUT, 0); g.a.b(exit);
        g.a.bind(exit);
        g.a.ldr(9, SP, BUDGET_SLOT); g.a.sub(9, 9, LEFT); g.a.orr_lsl(0, 9, 0, 16);
        g.a.ldp(27, 28, SP, 80); g.a.ldp(25, 26, SP, 64); g.a.ldp(23, 24, SP, 48); g.a.ldp(21, 22, SP, 32); g.a.ldp(19, 20, SP, 16);
        g.a.ldp_post(29, 30, SP, FRAME);
        g.a.ret();
        for s in std::mem::take(&mut g.stubs) { s(&mut g.a); }
        let words = g.a.finish();
        cc.write(&words)
    }

    /// Run compiled code. Returns `done | code << 16`; see the EXIT_* codes.
    ///
    /// # Safety
    /// `code` must be an address returned by `CodeCache::entry_point` for live code produced
    /// by `compile`, and `entry` must be an instruction offset recorded by that compilation.
    /// The owning cache must remain alive and cannot reset or overwrite the code until return. `h` must have been created by
    /// `Helpers::new::<B>()`. The backing `BlockInsn` slice passed to `compile` must remain alive
    /// and unmoved until `code` can no longer run because fallback paths embed pointers into it.
    /// Code compiled with `fast = true` requires `Some(fm)`; it must describe `bus`, including
    /// valid unmoved backing buffers for its TLB entries, and remain valid for this call.
    pub unsafe fn run<B: Bus>(code: *const u8, cpu: &mut Cpu, bus: &mut B, h: &Helpers, budget: u32, entry: u32, fm: Option<FastMem>) -> u32 {
        // SAFETY: The caller guarantees that `code` names a live compiled entry point in this
        // cache with the declared ABI and concrete bus type.
        let f: extern "C" fn(*mut Cpu, *mut B, *const Helpers, u32, u32, *const crate::bus::TlbEntry, *mut u32) -> u32 = unsafe { std::mem::transmute(code) };
        let (tlb, pv) = match fm { Some(m) => (m.tlb, m.page_ver), None => (std::ptr::null(), std::ptr::null_mut()) };
        f(cpu, bus, h, budget, entry, tlb, pv)
    }
    pub const CODE_END: u32 = EXIT_END; pub const CODE_LEFT: u32 = EXIT_LEFT; pub const CODE_TRAP: u32 = EXIT_TRAP;
    pub const CODE_CUT: u32 = EXIT_CUT; pub const CODE_TRAP_PRE: u32 = EXIT_TRAP_PRE;
    #[allow(dead_code)] fn _uses(_: Label, _: Trap) {}
}

#[cfg(not(any(target_arch = "wasm32", all(target_arch = "aarch64", any(target_os = "macos", target_os = "linux")))))]
mod native {
    use crate::block::BlockInsn;
    use crate::bus::Bus;
    use crate::state::Cpu;
    pub const AVAILABLE: bool = false;
    pub const NONE: u32 = u32::MAX;
    pub const MAX_BLOCK_CODE: usize = 0;
    pub struct CodeCache;
    impl CodeCache { pub fn entry_point(&self, _: u32) -> *const u8 { std::ptr::null() } pub fn new(_: usize) -> Option<CodeCache> { None } pub fn remaining(&self) -> usize { 0 } pub fn used(&self) -> usize { 0 } pub fn reset(&mut self) {} }
    pub struct Helpers;
    impl Helpers { pub fn new<B: Bus>() -> Helpers { Helpers } pub fn shared<B: Bus>() -> &'static Helpers { &Helpers } }
    pub fn compile(_: &mut CodeCache, _: &mut [BlockInsn], _: u32, _: bool) -> Option<u32> { None }
    /// The non-native implementation does not execute code.
    ///
    /// # Safety
    /// This signature matches the native implementation; no additional requirements apply here.
    pub unsafe fn run<B: Bus>(_: *const u8, _: &mut Cpu, _: &mut B, _: &Helpers, _: u32, _: u32, _: Option<crate::bus::FastMem>) -> u32 { 0 }
    pub const CODE_END: u32 = 0; pub const CODE_LEFT: u32 = 1; pub const CODE_TRAP: u32 = 2; pub const CODE_CUT: u32 = 3; pub const CODE_TRAP_PRE: u32 = 4;
}

#[cfg(target_arch = "wasm32")]
#[path = "wasm.rs"]
mod native;

pub use native::*;

#[cfg(not(target_arch = "wasm32"))]
pub fn ready(_: &CodeCache, _: u32, _: u32) -> bool { true }
