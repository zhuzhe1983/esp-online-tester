//! Pure C5 CPU policy, not an address decoder, DMA policy, or physical cache.
//! Sources: ESP-IDF v5.5.3 2c211b236707889e8400c4dc5644dd5c4ee071e0,
//! components/riscv/include/riscv/csr.h and esp_hw_support/port/esp32c5/cpu_region_protect.c;
//! both files are identical in v5.5.4 735507283d5b2f9fb363a1901172dbd9e847945d.
//! C5 TRM1.1 sections2.9.1/2.9.2, registers2.80/2.82/2.83; privileged ISA1.10 PMP.
//! The POR A=2/NA4 contradiction and unqualified address WARL remain explicit.

const PMA_MASK: u32 = 0xef00_001d;
const PMA_LOCK: u32 = 1 << 29;
const PMP_LOCK: u8 = 0x80;
const PHYSICAL_END: u64 = 1 << 32;
const GRAIN: u64 = 128;

/// Construct for each operation from the caller's reliable machine-only guard.
/// MPP is previous privilege and is deliberately not a constructor argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuardedMachineContext(());
impl GuardedMachineContext {
    pub fn new(machine_only: bool, mprv: bool) -> Result<Self, UnsupportedReason> {
        if machine_only && !mprv { Ok(Self(())) } else { Err(UnsupportedReason::Context) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessKind { Fetch, Load, Store, AtomicRead, AtomicWrite, AtomicRmw }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Access { pub address: u32, pub width: u8, pub kind: AccessKind }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry { Pma(u8), Pmp(u8) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum AccessFault { Instruction = 1, Load = 5, Store = 7 }
impl AccessKind {
    fn fault(self) -> AccessFault {
        match self { Self::Fetch => AccessFault::Instruction, Self::Load | Self::AtomicRead => AccessFault::Load, _ => AccessFault::Store }
    }
    fn pmp_bits(self) -> u8 {
        match self { Self::Fetch => 4, Self::Load | Self::AtomicRead => 1, Self::Store | Self::AtomicWrite => 2, Self::AtomicRmw => 3 }
    }
    fn pma_bits(self) -> u32 {
        match self { Self::Fetch => 4, Self::Load | Self::AtomicRead => 16, Self::Store | Self::AtomicWrite => 8, Self::AtomicRmw => 24 }
    }
}

/// Diagnostic values only. No physical line, latency, or cache-sideband claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheAttributes {
    pub noncacheable: bool, pub write_through: bool,
    pub write_miss_no_allocate: bool, pub read_miss_no_allocate: bool,
}
impl CacheAttributes {
    fn from_cfg(cfg: u32) -> Self {
        Self { noncacheable: cfg & (1 << 27) != 0, write_through: cfg & (1 << 26) != 0,
            write_miss_no_allocate: cfg & (1 << 25) != 0, read_miss_no_allocate: cfg & (1 << 24) != 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsupportedReason {
    Context, Csr(u32), ReservedBits(u32), Na4(Entry), PmpXl(u8), PmpWriteWithoutRead(u8),
    PhysicalAddress(u32), PhysicalRange(Entry), PorAddressWrite(u8), PorWindow(u8),
    AddressWindow(Entry), AddressReadback(Entry), PmaOverlap, PmaPartial(Entry),
    PmaTorPreviousLock(u8), Width(AccessKind, u8), MisalignedAtomic, AddressOverflow,
    GenerationExhausted, SnapshotVersion(u32),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyDecision {
    /// This layer grants no permission; continue through the other policy/Bus/APM gates.
    NoMatch,
    Allow { entry: Entry, attributes: Option<CacheAttributes> },
    Deny { entry: Entry, cause: AccessFault },
    Unsupported(UnsupportedReason),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteEffect { Changed, Unchanged, IgnoredLocked }

/// Bounded raw diagnostic image, not a claim of unknown WARL readback semantics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotV1 {
    version: u32, epoch: u64, generation: u64,
    pma_cfg: [u32; 16], pma_addr: [u32; 16], pma_por: u16,
    pmp_cfg: [u32; 4], pmp_addr: [u32; 16],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pma { cfg: u32, addr: u32, por: bool }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pmp { cfg: u8, addr: u32 }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct C5MemoryProtection { pma: [Pma; 16], pmp: [Pmp; 16], epoch: u64, generation: u64 }
#[derive(Clone, Copy)]
enum Csr { PmaCfg(usize), PmaAddr(usize), PmpCfg(usize), PmpAddr(usize) }
fn csr(number: u32) -> Result<Csr, UnsupportedReason> {
    match number {
        0xbc0..=0xbcf => Ok(Csr::PmaCfg((number - 0xbc0) as usize)),
        0xbd0..=0xbdf => Ok(Csr::PmaAddr((number - 0xbd0) as usize)),
        0x3a0..=0x3a3 => Ok(Csr::PmpCfg((number - 0x3a0) as usize)),
        0x3b0..=0x3bf => Ok(Csr::PmpAddr((number - 0x3b0) as usize)),
        _ => Err(UnsupportedReason::Csr(number)),
    }
}
pub fn accepts_csr(number: u32) -> bool { csr(number).is_ok() }

#[derive(Clone, Copy)]
struct Range { start: u64, end: u64 }
impl Range {
    fn intersects(self, other: Self) -> bool { self.start < self.end && other.start < other.end && self.start < other.end && other.start < self.end }
    fn contains(self, other: Self) -> bool { self.start <= other.start && other.end <= self.end }
}
#[derive(Clone, Copy)]
struct Region { known: Range, uncertain: [Range; 2] }
impl Region {
    fn off() -> Self { Self { known: Range { start: 0, end: 0 }, uncertain: [Range { start: 0, end: 0 }; 2] } }
    fn tor(lower: u32, upper: u32) -> Self {
        if lower >= upper { return Self::off(); }
        let lo = u64::from(lower) << 2; let hi = u64::from(upper) << 2;
        let floor = |n: u64| n & !(GRAIN - 1);
        let ceil = |n: u64| ((n + GRAIN - 1) & !(GRAIN - 1)).min(PHYSICAL_END);
        Self { known: Range { start: ceil(lo), end: floor(hi) },
            uncertain: [Range { start: floor(lo), end: ceil(lo) }, Range { start: floor(hi), end: ceil(hi) }] }
    }
    fn napot(addr: u32, entry: Entry) -> Result<Self, UnsupportedReason> {
        let size = 1u64 << (addr.trailing_ones() + 3);
        let start = (u64::from(addr) << 2) & !(size - 1); let end = start + size;
        if end > PHYSICAL_END { return Err(UnsupportedReason::PhysicalRange(entry)); }
        if size < GRAIN {
            let start = start & !(GRAIN - 1);
            return Ok(Self { known: Range { start: 0, end: 0 },
                uncertain: [Range { start, end: start + GRAIN }, Range { start: 0, end: 0 }] });
        }
        Ok(Self { known: Range { start, end }, uncertain: [Range { start: 0, end: 0 }; 2] })
    }
    fn uncertain_at(self, access: Range) -> bool { self.uncertain.iter().any(|r| r.intersects(access)) }
}

impl Default for C5MemoryProtection { fn default() -> Self { Self::new() } }
impl C5MemoryProtection {
    pub fn new() -> Self {
        // TRM1.1 Reg2.82 says A reset2 while section2.9.2.2 excludes NA4.
        // Preserve that raw POR and mark its128B window unknown. Address staging
        // moves that reset-only A2 window; an address does not select A3/NAPOT.
        Self { pma: [Pma { cfg: 0x8000_0000, addr: 0, por: true }; 16],
            pmp: [Pmp { cfg: 0, addr: 0 }; 16], epoch: 0, generation: 0 }
    }
    pub fn epoch(&self) -> u64 { self.epoch }
    pub fn generation(&self) -> u64 { self.generation }
    pub fn snapshot(&self) -> SnapshotV1 {
        SnapshotV1 { version: 1, epoch: self.epoch, generation: self.generation,
            pma_cfg: self.pma.map(|e| e.cfg), pma_addr: self.pma.map(|e| e.addr),
            pma_por: self.pma.iter().enumerate().fold(0, |m, (i, e)| m | if e.por { 1 << i } else { 0 }),
            pmp_cfg: std::array::from_fn(|i| (0..4).fold(0, |v, b| v | u32::from(self.pmp[4 * i + b].cfg) << (8 * b))),
            pmp_addr: self.pmp.map(|e| e.addr) }
    }
    /// Only an actual core reset calls this. ROM bytes/profile replacement is not reset.
    pub fn reset_core(&mut self) -> Result<(), UnsupportedReason> {
        let epoch = self.epoch.checked_add(1).ok_or(UnsupportedReason::GenerationExhausted)?;
        let generation = self.generation.checked_add(1).ok_or(UnsupportedReason::GenerationExhausted)?;
        *self = Self { epoch, generation, ..Self::new() }; Ok(())
    }
    pub fn read_csr(&self, number: u32, _ctx: GuardedMachineContext) -> Result<u32, UnsupportedReason> {
        match csr(number)? {
            Csr::PmaCfg(i) => Ok(self.pma[i].cfg),
            Csr::PmaAddr(i) => {
                let e = self.pma[i];
                if e.por && e.addr != 0 { return Err(UnsupportedReason::AddressReadback(Entry::Pma(i as u8))); }
                if !e.por { Self::readback(e.cfg >> 30, e.addr, Entry::Pma(i as u8))?; }
                Ok(e.addr)
            }
            Csr::PmpCfg(i) => Ok((0..4).fold(0, |v, b| v | u32::from(self.pmp[4 * i + b].cfg) << (8 * b))),
            Csr::PmpAddr(i) => { let e = self.pmp[i]; Self::readback(u32::from(e.cfg >> 3 & 3), e.addr, Entry::Pmp(i as u8))?; Ok(e.addr) }
        }
    }
    fn readback(mode: u32, addr: u32, entry: Entry) -> Result<(), UnsupportedReason> {
        if (mode < 2 && addr & 31 != 0) || (mode == 3 && addr.trailing_ones() < 4) {
            Err(UnsupportedReason::AddressReadback(entry))
        } else { Ok(()) }
    }
    pub fn write_csr(&mut self, number: u32, value: u32, _ctx: GuardedMachineContext) -> Result<WriteEffect, UnsupportedReason> {
        let mut next = self.clone(); let mut ignored = false;
        match csr(number)? {
            Csr::PmaCfg(i) => {
                if self.pma[i].cfg & PMA_LOCK != 0 { return Ok(WriteEffect::IgnoredLocked); }
                if value & !PMA_MASK != 0 { return Err(UnsupportedReason::ReservedBits(number)); }
                if value >> 30 == 2 { return Err(UnsupportedReason::Na4(Entry::Pma(i as u8))); }
                next.pma[i].cfg = value; next.pma[i].por = false; next.pma_region(i)?;
            }
            Csr::PmaAddr(i) => {
                if self.pma[i].cfg & PMA_LOCK != 0 { return Ok(WriteEffect::IgnoredLocked); }
                if i < 15 && self.pma[i + 1].cfg & PMA_LOCK != 0 && self.pma[i + 1].cfg >> 30 == 1 {
                    return Err(UnsupportedReason::PmaTorPreviousLock(i as u8));
                }
                // Rev100 ROM writes the RW address before its explicit cfg.
                // Retain POR A2/tag and unqualified readback until that cfg write.
                Self::address(value)?; next.pma[i].addr = value; next.pma_region(i)?;
            }
            Csr::PmpCfg(word) => {
                for b in 0..4 {
                    let i = 4 * word + b;
                    if self.pmp[i].cfg & PMP_LOCK != 0 { ignored = true; continue; }
                    let cfg = (value >> (8 * b)) as u8;
                    if cfg & 0x40 != 0 { return Err(UnsupportedReason::PmpXl(i as u8)); }
                    if cfg & 0x20 != 0 { return Err(UnsupportedReason::ReservedBits(number)); }
                    if cfg >> 3 & 3 == 2 { return Err(UnsupportedReason::Na4(Entry::Pmp(i as u8))); }
                    if cfg & 3 == 2 { return Err(UnsupportedReason::PmpWriteWithoutRead(i as u8)); }
                    next.pmp[i].cfg = cfg; next.pmp_region(i)?;
                }
            }
            Csr::PmpAddr(i) => {
                if self.pmp[i].cfg & PMP_LOCK != 0 || i < 15 && self.pmp[i + 1].cfg & 0x98 == 0x88 {
                    return Ok(WriteEffect::IgnoredLocked);
                }
                Self::address(value)?; next.pmp[i].addr = value; next.pmp_region(i)?;
                // A TOR successor also changes if its unlocked lower boundary changes.
                if i < 15 { next.pmp_region(i + 1)?; }
            }
        }
        if next.pma == self.pma && next.pmp == self.pmp {
            return Ok(if ignored { WriteEffect::IgnoredLocked } else { WriteEffect::Unchanged });
        }
        next.generation = self.generation.checked_add(1).ok_or(UnsupportedReason::GenerationExhausted)?;
        *self = next; Ok(WriteEffect::Changed)
    }
    fn address(value: u32) -> Result<(), UnsupportedReason> {
        if value <= 0x3fff_ffff { Ok(()) } else { Err(UnsupportedReason::PhysicalAddress(value)) }
    }
    pub fn check_pma(&self, access: Access, ctx: GuardedMachineContext) -> PolicyDecision { PolicyView::check_pma(self, access, ctx) }
    pub fn check_pmp(&self, access: Access, ctx: GuardedMachineContext) -> PolicyDecision { PolicyView::check_pmp(self, access, ctx) }

}

// Both the live CSR owner and the observer's immutable cached snapshot use this
// one policy implementation. No full snapshot copy/allocation on an access.
trait PolicyView: Sized {
    fn pma_entry(&self, i: usize) -> Pma;
    fn pmp_entry(&self, i: usize) -> Pmp;
    fn pma_region(&self, i: usize) -> Result<Region, UnsupportedReason> {
        let e = self.pma_entry(i);
        if e.por { return Ok(Region::off()); }
        match e.cfg >> 30 {
            0 => Ok(Region::off()),
            1 => Ok(Region::tor(if i == 0 { 0 } else { self.pma_entry(i - 1).addr }, e.addr)),
            3 => Region::napot(e.addr, Entry::Pma(i as u8)),
            _ => Err(UnsupportedReason::Na4(Entry::Pma(i as u8))),
        }
    }
    fn pmp_region(&self, i: usize) -> Result<Region, UnsupportedReason> {
        let e = self.pmp_entry(i);
        match e.cfg >> 3 & 3 {
            0 => Ok(Region::off()),
            1 => Ok(Region::tor(if i == 0 { 0 } else { self.pmp_entry(i - 1).addr }, e.addr)),
            3 => Region::napot(e.addr, Entry::Pmp(i as u8)),
            _ => Err(UnsupportedReason::Na4(Entry::Pmp(i as u8))),
        }
    }
    fn access(access: Access) -> Result<Range, UnsupportedReason> {
        let valid = match access.kind {
            AccessKind::Fetch => matches!(access.width, 2 | 4),
            AccessKind::Load | AccessKind::Store => matches!(access.width, 1 | 2 | 4),
            _ => access.width == 4,
        };
        if !valid { return Err(UnsupportedReason::Width(access.kind, access.width)); }
        if matches!(access.kind, AccessKind::AtomicRead | AccessKind::AtomicWrite | AccessKind::AtomicRmw) && access.address & 3 != 0 {
            return Err(UnsupportedReason::MisalignedAtomic);
        }
        let start = u64::from(access.address); let end = start + u64::from(access.width);
        if end > PHYSICAL_END { return Err(UnsupportedReason::AddressOverflow); }
        Ok(Range { start, end })
    }
    fn check_pma(&self, access: Access, _ctx: GuardedMachineContext) -> PolicyDecision {
        let span = match Self::access(access) { Ok(s) => s, Err(e) => return PolicyDecision::Unsupported(e) };
        let mut matched = None;
        for i in 0..16 {
            let e = self.pma_entry(i);
            if e.por {
                let start = (u64::from(e.addr) << 2) & !(GRAIN - 1);
                if span.intersects(Range { start, end: start + GRAIN }) {
                    return PolicyDecision::Unsupported(UnsupportedReason::PorWindow(i as u8));
                }
            }
            let region = match self.pma_region(i) { Ok(r) => r, Err(e) => return PolicyDecision::Unsupported(e) };
            if region.uncertain_at(span) { return PolicyDecision::Unsupported(UnsupportedReason::AddressWindow(Entry::Pma(i as u8))); }
            if !region.known.intersects(span) { continue; }
            if !region.known.contains(span) { return PolicyDecision::Unsupported(UnsupportedReason::PmaPartial(Entry::Pma(i as u8))); }
            if matched.is_some() { return PolicyDecision::Unsupported(UnsupportedReason::PmaOverlap); }
            matched = Some(i);
        }
        let Some(i) = matched else { return PolicyDecision::NoMatch; };
        let e = self.pma_entry(i); let bits = access.kind.pma_bits(); let entry = Entry::Pma(i as u8);
        if e.cfg & 1 == 0 || e.cfg & bits != bits { PolicyDecision::Deny { entry, cause: access.kind.fault() } }
        else { PolicyDecision::Allow { entry, attributes: Some(CacheAttributes::from_cfg(e.cfg)) } }
    }
    fn check_pmp(&self, access: Access, _ctx: GuardedMachineContext) -> PolicyDecision {
        let span = match Self::access(access) { Ok(s) => s, Err(e) => return PolicyDecision::Unsupported(e) };
        for i in 0..16 {
            let entry = Entry::Pmp(i as u8);
            let region = match self.pmp_region(i) { Ok(r) => r, Err(e) => return PolicyDecision::Unsupported(e) };
            if region.uncertain_at(span) { return PolicyDecision::Unsupported(UnsupportedReason::AddressWindow(entry)); }
            if !region.known.intersects(span) { continue; }
            let bits = access.kind.pmp_bits(); let cfg = self.pmp_entry(i).cfg;
            if !region.known.contains(span) || cfg & PMP_LOCK != 0 && cfg & bits != bits {
                return PolicyDecision::Deny { entry, cause: access.kind.fault() };
            }
            return PolicyDecision::Allow { entry, attributes: None };
        }
        PolicyDecision::NoMatch
    }
}
impl PolicyView for C5MemoryProtection {
    fn pma_entry(&self, i: usize) -> Pma { self.pma[i] }
    fn pmp_entry(&self, i: usize) -> Pmp { self.pmp[i] }
}
impl PolicyView for SnapshotV1 {
    fn pma_entry(&self, i: usize) -> Pma { Pma { cfg: self.pma_cfg[i], addr: self.pma_addr[i], por: self.pma_por & (1 << i) != 0 } }
    fn pmp_entry(&self, i: usize) -> Pmp { Pmp { cfg: (self.pmp_cfg[i / 4] >> (8 * (i % 4))) as u8, addr: self.pmp_addr[i] } }
}
impl SnapshotV1 {
    pub fn version(&self) -> u32 { self.version }
    pub fn epoch(&self) -> u64 { self.epoch }
    pub fn generation(&self) -> u64 { self.generation }
    pub fn pma_cfg(&self) -> &[u32; 16] { &self.pma_cfg }
    pub fn pma_addr(&self) -> &[u32; 16] { &self.pma_addr }
    pub fn pma_por_mask(&self) -> u16 { self.pma_por }
    pub fn pmp_cfg(&self) -> &[u32; 4] { &self.pmp_cfg }
    pub fn pmp_addr(&self) -> &[u32; 16] { &self.pmp_addr }
    /// Only consume snapshots produced by the live owner, not user-provided metadata.
    pub fn check_pma(&self, access: Access, ctx: GuardedMachineContext) -> PolicyDecision {
        if self.version != 1 { return PolicyDecision::Unsupported(UnsupportedReason::SnapshotVersion(self.version)); }
        PolicyView::check_pma(self, access, ctx)
    }
    pub fn check_pmp(&self, access: Access, ctx: GuardedMachineContext) -> PolicyDecision {
        if self.version != 1 { return PolicyDecision::Unsupported(UnsupportedReason::SnapshotVersion(self.version)); }
        PolicyView::check_pmp(self, access, ctx)
    }
}

#[cfg(test)]
mod generation_tests {
    use super::*;
    #[test]
    fn exhausted_generation_never_commits_or_panics() {
        let ctx = GuardedMachineContext::new(true, false).unwrap(); let mut p = C5MemoryProtection::new();
        p.generation = u64::MAX; let before = p.clone();
        assert_eq!(p.write_csr(0xbc0, 0, ctx), Err(UnsupportedReason::GenerationExhausted)); assert_eq!(p, before);
        assert_eq!(p.reset_core(), Err(UnsupportedReason::GenerationExhausted)); assert_eq!(p, before);
        p.generation = 0; p.epoch = u64::MAX; let before = p.clone();
        assert_eq!(p.reset_core(), Err(UnsupportedReason::GenerationExhausted)); assert_eq!(p, before);
    }
    #[test]
    fn snapshot_version_guard_does_not_fall_back_to_permissive_policy() {
        let ctx = GuardedMachineContext::new(true, false).unwrap(); let mut s = C5MemoryProtection::new().snapshot();
        s.version = 2; let a = Access { address: 0x4080_0000, width: 4, kind: AccessKind::Load };
        assert_eq!(s.check_pma(a, ctx), PolicyDecision::Unsupported(UnsupportedReason::SnapshotVersion(2)));
        assert_eq!(s.check_pmp(a, ctx), PolicyDecision::Unsupported(UnsupportedReason::SnapshotVersion(2)));
    }
}
