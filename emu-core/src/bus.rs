//! Memory bus abstraction between a core and the SoC. One trait for every core: the six
//! accessors and `fetch` are what an interpreter needs; the rest are hooks a block cache or a
//! JIT uses and a simple bus leaves at their defaults.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// no memory / device at this address
    Unmapped,
    /// address exists but access of this kind is not allowed (e.g. exec from data-only)
    Prohibited,
    /// address is not aligned for this access width
    Misaligned,
}

/// Number of entries in a bus's software TLB and the write-version page size, fixed here
/// because generated code indexes both directly.
pub const TLB_ENTRIES: usize = 512;
pub const VPAGE_SHIFT: u32 = 8;
/// A four-byte instruction can overlap the next page by at most three bytes.
pub const PREV_PAGE_BYTES: u32 = 3;
/// Address hash shared by the bus and generated memory-access probes.
pub const TLB_INDEX_SHIFT: u32 = 16;
pub const TLB_XOR_SHIFT: u32 = 24;
#[inline(always)]
pub fn tlb_index(addr: u32) -> usize { (((addr >> TLB_INDEX_SHIFT) ^ (addr >> TLB_XOR_SHIFT)) as usize) & (TLB_ENTRIES - 1) }

/// One software-TLB entry: guest `[lo, hi)` is host memory starting at `base`; `vbase` is the
/// write-version index of `lo`. The JIT uses this C layout's field offsets and entry size. Copying or
/// sharing an entry never dereferences `base`; a JIT owner must separately keep its backing buffer
/// alive and unmoved, and serialize generated access to it.
///
/// EX110: `code` is zero only when no decoded consumer depends on any byte this mapping covers,
/// nor on the pages `bump` would move with them, so a write through it needs no version
/// bookkeeping at all. A bus that cannot prove that must publish a nonzero `code`, and a bus that
/// does must answer `note_code_page` by clearing entries it has already published.
/// Both backends keep 32-byte entries. Native retains its natural pointer alignment so
/// returning an optional entry does not acquire extra padding and stack alignment.
///
/// On wasm32, `span` is `hi - lo` for a live mapping and 0 for an empty slot (EX173). It lets generated code
/// decide a whole access with one unsigned compare against `addr - lo`, because an address below
/// `lo` wraps the subtraction above any possible span and an empty slot rejects every offset.
/// Always derive it with [`TlbEntry::with_span`] from the final endpoints.
/// x4 pack: `writable` and `code` are 16-bit flags sharing one word, so the entry stays 32 bytes
/// on wasm32 with both `code` (EX110) and `span` (EX173); generated code loads them as u16.
/// Native probes use `lo`/`hi`, so omit their unused span to keep native entries at 32 bytes too.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TlbEntry { pub lo: u32, pub hi: u32, pub base: *mut u8, pub vbase: u32, pub writable: u16, pub code: u16, pub off: u32, pub src: u32,
    #[cfg(target_arch = "wasm32")] pub span: u32,
}
impl TlbEntry {
    pub const EMPTY: TlbEntry = TlbEntry { lo: 1, hi: 0, base: std::ptr::null_mut(), vbase: 0, writable: 0, off: 0, src: 0, code: 1,
        #[cfg(target_arch = "wasm32")] span: 0,
    };
    /// Publish the endpoints to generated code. An entry that never passes through this keeps
    /// `span` 0, which no access can satisfy, so it simply takes the slow path.
    #[inline(always)]
    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_mut))]
    pub fn with_span(mut self) -> TlbEntry {
        debug_assert!(self.hi >= self.lo);
        debug_assert!(self.lo.is_multiple_of(16));
        #[cfg(target_arch = "wasm32")]
        { self.span = self.hi.wrapping_sub(self.lo); }
        self
    }
}
// SAFETY: Sending this Copy value transfers only address bits. TlbEntry has no safe operation that
// dereferences `base`; generated access must separately uphold the documented owner invariants.
unsafe impl Send for TlbEntry {}
// Both JIT backends must retain the compact entry footprint.
#[cfg(any(target_arch = "wasm32", target_pointer_width = "64"))]
const _: () = assert!(std::mem::size_of::<TlbEntry>() == 32);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::align_of::<TlbEntry>() == std::mem::align_of::<usize>());
#[cfg(target_arch = "wasm32")]
const _: () = assert!(std::mem::size_of::<Option<TlbEntry>>() == 36);
// SAFETY: Sharing this value exposes address bits but performs no dereference. Generated access
// through `base` must separately uphold the documented lifetime and synchronization invariants.
unsafe impl Sync for TlbEntry {}

/// What generated code needs to access memory without calling back: the TLB and the
/// write-version counters. Both pointers, and every backing buffer named by a TLB entry, must stay
/// valid and unmoved while generated code can access them.
#[derive(Clone, Copy)]
pub struct FastMem { pub tlb: *const TlbEntry, pub page_ver: *mut u32 }

/// Experimental generated-code view of a 32 KiB, 64-byte, four-way cache.
/// The owner keeps both pointers live and unmoved during a generated call.
/// Only zero-cost hits are admitted; misses return through ordinary bus helpers.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FastCache { pub lines: *mut FastCacheLine, pub hits: *mut u64 }

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FastCacheLine { pub tag: u32, pub dirty: u32, pub valid: u32 }
impl Default for FastCacheLine {
    fn default() -> Self { Self { tag: u32::MAX, dirty: 0, valid: 0 } }
}

pub trait Bus {
    fn read8(&mut self, addr: u32) -> Result<u8, Fault>;
    fn read16(&mut self, addr: u32) -> Result<u16, Fault>;
    fn read32(&mut self, addr: u32) -> Result<u32, Fault>;
    fn write8(&mut self, addr: u32, v: u8) -> Result<(), Fault>;
    fn write16(&mut self, addr: u32, v: u16) -> Result<(), Fault>;
    fn write32(&mut self, addr: u32, v: u32) -> Result<(), Fault>;
    /// DMA and host accesses retain functional effects but bypass CPU cache timing.
    /// Buses without CPU-specific accounting can use the ordinary accessors.
    fn read8_unpriced(&mut self, addr: u32) -> Result<u8, Fault> { self.read8(addr) }
    fn read16_unpriced(&mut self, addr: u32) -> Result<u16, Fault> { self.read16(addr) }
    fn read32_unpriced(&mut self, addr: u32) -> Result<u32, Fault> { self.read32(addr) }
    fn write8_unpriced(&mut self, addr: u32, v: u8) -> Result<(), Fault> { self.write8(addr, v) }
    fn write16_unpriced(&mut self, addr: u32, v: u16) -> Result<(), Fault> { self.write16(addr, v) }
    fn write32_unpriced(&mut self, addr: u32, v: u32) -> Result<(), Fault> { self.write32(addr, v) }
    /// Fetch up to 4 instruction bytes at `pc` (any alignment). Bytes past the
    /// end of a mapped region may be zero.
    fn fetch(&mut self, pc: u32) -> Result<[u8; 4], Fault>;
    /// Write-version counters, one per page of guest memory: `page_versions()[code_page(pc)]`
    /// changes whenever a write could have altered an instruction at `pc`. A decode-cache entry
    /// remembers its page index and the version it saw, so the common path is one indexed load
    /// instead of re-fetching and comparing the bytes; `code_page` is only called on a miss.
    /// Pages must be at least 128 bytes: a block (`block::MAX_LEN` instructions) records the
    /// versions of the pages holding its first and last byte and assumes there is no third.
    fn page_versions(&self) -> &[u32] { &[] }
    fn code_page(&mut self, pc: u32) -> u32 { let _ = pc; 0 }
    /// shell-s2 (EX168 t5): `(lo, hi, epoch)`: every change to a page version with index in `lo..hi`
    /// also moves `epoch`, so one epoch compare vouches for all of them. Empty by default.
    #[inline(always)]
    fn stable_pages(&self) -> (u32, u32, u64) { (0, 0, 0) }
    /// EX110: decoded code is about to record the version of page `vidx`, so every later write
    /// that could change a byte of it must move a version this consumer will compare. Call this
    /// before reading the version to remember: the bytes and the version are then read after the
    /// page is watched, so writes before the call are already visible in the bytes and writes
    /// after it move the version. A bus that skips version bookkeeping for unwatched memory must
    /// implement this. Wrappers must forward it; buses that always bump explicitly do nothing.
    /// Requiring the method prevents wrappers from silently disabling invalidation.
    fn note_code_page(&mut self, vidx: u32);
    /// The pc of the instruction about to execute, for buses that attribute accesses to code.
    #[inline(always)]
    fn note_pc(&mut self, pc: u32) { let _ = pc; }
    /// A load-reserved by the running core at `addr`, for a bus shared by several harts: their
    /// stores to the reservation set must break it (RISC-V unprivileged ISA, "Zalrsc"). Wrappers
    /// must forward it.
    #[inline(always)]
    fn reserve(&mut self, addr: u32) { let _ = addr; }
    /// Whether the running core's reservation at `addr` still holds for its store-conditional;
    /// the reservation is used up either way. A bus with one hart leaves this to the core.
    #[inline(always)]
    fn reservation_held(&mut self, addr: u32) -> bool { let _ = addr; true }
    /// True when the last instruction may have changed an interrupt line, so a block must end
    /// and let the machine re-derive the CPU's interrupt inputs before the next instruction.
    #[inline(always)]
    fn block_break(&self) -> bool { false }
    /// Virtual quanta (EX133): true while the machine runs one core across several scheduling
    /// quanta. Device registers must then be reached at exact device time, so the executor asks
    /// `defer_access` before a word access and stops in front of the instruction when it says yes.
    #[inline(always)]
    fn defer_armed(&self) -> bool { false }
    /// True, and remembered for the machine, when `addr` is a device register and a
    /// multi-quantum run is active. The instruction must not execute in this dispatch.
    #[inline(always)]
    fn defer_access(&mut self, addr: u32) -> bool { let _ = addr; false }
    /// True when this dispatch stopped in front of a deferred access (not cleared by reading).
    #[inline(always)]
    fn deferred(&self) -> bool { false }
    /// Direct memory access for generated code, if the bus has a `TlbEntry` table.
    fn fast_mem(&mut self) -> Option<FastMem> { None }
    /// Copy guest memory at `addr` into `out` in one piece when the whole range is plain
    /// memory with no device behind it. `false` changes nothing and means the caller must use
    /// the per-access reads, which also report where a fault is. A fast path for vector loads.
    fn read_bulk(&mut self, addr: u32, out: &mut [u8]) -> bool { let _ = (addr, out); false }
    fn fast_cache(&mut self) -> Option<FastCache> { None }
    /// Drain provisional synchronous data-access penalties for a fast-path timing experiment.
    /// The scheduler decides when to settle this batch; this does not imply access-level ordering.
    fn take_timing_penalty(&mut self) -> u32 { 0 }
    /// Start a compiled batch's provisional memory-service cursor in shared CPU cycles.
    /// Access effects are still immediate; this is not instruction-level interleaving.
    fn begin_timing_batch(&mut self, _core: usize, _now: u64) {}
    /// Add an opt-in instruction penalty to the same provisional batch drain.
    fn add_timing_penalty(&mut self, _cycles: u32) {}
    /// Called by the machine after each scheduling round with the cycles it ran; lets the SoC
    /// advance timers and DMA (the S3 batches this device time). Non-zero means interrupt lines
    /// may have moved and must be re-derived.
    fn tick(&mut self, cycles: u32) -> u32 { let _ = cycles; 0 }
}

/// Simple flat RAM for unit tests.
pub struct FlatRam {
    pub base: u32,
    pub mem: Vec<u8>,
    /// bumped on every write: coarse, but this RAM only backs unit tests
    pub ver: u32,
}

impl FlatRam {
    pub fn new(base: u32, size: usize) -> Self { FlatRam { base, mem: vec![0; size], ver: 0 } }
    fn off(&self, addr: u32, n: usize) -> Result<usize, Fault> {
        let o = addr.wrapping_sub(self.base) as usize;
        if o.checked_add(n).is_some_and(|end| end <= self.mem.len()) { Ok(o) } else { Err(Fault::Unmapped) }
    }
}

impl Bus for FlatRam {
    fn note_code_page(&mut self, _vidx: u32) {} // All writes already update versions, or this bus has no decode cache.
    fn read8(&mut self, a: u32) -> Result<u8, Fault> { let o = self.off(a, 1)?; Ok(self.mem[o]) }
    fn read16(&mut self, a: u32) -> Result<u16, Fault> { let o = self.off(a, 2)?; Ok(u16::from_le_bytes([self.mem[o], self.mem[o + 1]])) }
    fn read32(&mut self, a: u32) -> Result<u32, Fault> { let o = self.off(a, 4)?; Ok(u32::from_le_bytes(self.mem[o..o + 4].try_into().unwrap())) }
    fn read_bulk(&mut self, a: u32, out: &mut [u8]) -> bool {
        match self.off(a, out.len()) { Ok(o) => { out.copy_from_slice(&self.mem[o..o + out.len()]); true } Err(_) => false }
    }
    fn write8(&mut self, a: u32, v: u8) -> Result<(), Fault> { let o = self.off(a, 1)?; self.mem[o] = v; self.ver += 1; Ok(()) }
    fn write16(&mut self, a: u32, v: u16) -> Result<(), Fault> { let o = self.off(a, 2)?; self.mem[o..o + 2].copy_from_slice(&v.to_le_bytes()); self.ver += 1; Ok(()) }
    fn write32(&mut self, a: u32, v: u32) -> Result<(), Fault> { let o = self.off(a, 4)?; self.mem[o..o + 4].copy_from_slice(&v.to_le_bytes()); self.ver += 1; Ok(()) }
    fn page_versions(&self) -> &[u32] { std::slice::from_ref(&self.ver) }
    fn fetch(&mut self, pc: u32) -> Result<[u8; 4], Fault> {
        let o = self.off(pc, 1)?;
        let mut b = [0u8; 4];
        for (i, byte) in b.iter_mut().enumerate() { if o + i < self.mem.len() { *byte = self.mem[o + i]; } }
        Ok(b)
    }
}

#[cfg(test)]
mod bounds_tests {
    use super::*;

    #[test]
    fn access_length_cannot_wrap_into_the_valid_range() {
        let ram = FlatRam::new(0x1000, 16);
        assert_eq!(ram.off(0x1001, usize::MAX), Err(Fault::Unmapped));
        assert_eq!(ram.off(0x0ffc, 4), Err(Fault::Unmapped));
        assert_eq!(ram.off(0x100c, 4), Ok(12));
        assert_eq!(ram.off(0x1010, 0), Ok(16));
        assert_eq!(ram.off(0x1010, 1), Err(Fault::Unmapped));
    }
}
