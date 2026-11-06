//! Architectural state of one LX7 core (ESP32-S3 configuration).

/// PS register bit fields (XEA2, windowed).
pub mod ps {
    pub const INTLEVEL_MASK: u32 = 0xf;
    pub const EXCM: u32 = 1 << 4;
    pub const UM: u32 = 1 << 5;
    pub const RING_SHIFT: u32 = 6;
    pub const RING_MASK: u32 = 3 << 6;
    pub const OWB_SHIFT: u32 = 8;
    pub const OWB_MASK: u32 = 0xf << 8;
    pub const CALLINC_SHIFT: u32 = 16;
    pub const CALLINC_MASK: u32 = 3 << 16;
    pub const WOE: u32 = 1 << 18;
}

/// Special register numbers (subset we implement; others are stored raw).
pub mod sr {
    pub const LBEG: u32 = 0; pub const LEND: u32 = 1; pub const LCOUNT: u32 = 2; pub const SAR: u32 = 3; pub const BR: u32 = 4;
    pub const SCOMPARE1: u32 = 12; pub const ACCLO: u32 = 16; pub const ACCHI: u32 = 17;
    pub const M0: u32 = 32; pub const WINDOWBASE: u32 = 72; pub const WINDOWSTART: u32 = 73;
    pub const IBREAKENABLE: u32 = 96; pub const MEMCTL: u32 = 97; pub const ATOMCTL: u32 = 99; pub const DDR: u32 = 104;
    pub const IBREAKA0: u32 = 128; pub const DBREAKA0: u32 = 144; pub const DBREAKC0: u32 = 160;
    pub const CONFIGID0: u32 = 176; pub const EPC1: u32 = 177; pub const DEPC: u32 = 192; pub const EPS2: u32 = 194;
    pub const CONFIGID1: u32 = 208; pub const EXCSAVE1: u32 = 209; pub const CPENABLE: u32 = 224;
    pub const INTERRUPT: u32 = 226; pub const INTSET: u32 = 226; pub const INTCLEAR: u32 = 227; pub const INTENABLE: u32 = 228;
    pub const PS: u32 = 230; pub const VECBASE: u32 = 231; pub const EXCCAUSE: u32 = 232; pub const DEBUGCAUSE: u32 = 233;
    pub const CCOUNT: u32 = 234; pub const PRID: u32 = 235; pub const ICOUNT: u32 = 236; pub const ICOUNTLEVEL: u32 = 237;
    pub const EXCVADDR: u32 = 238; pub const CCOMPARE0: u32 = 240; pub const MISC0: u32 = 244;
}

/// Exception causes (EXCCAUSE).
pub mod exc {
    pub const ILLEGAL: u32 = 0; pub const SYSCALL: u32 = 1; pub const IFETCH_ERROR: u32 = 2; pub const LOAD_STORE_ERROR: u32 = 3;
    pub const LEVEL1_INTERRUPT: u32 = 4; pub const ALLOCA: u32 = 5; pub const DIVIDE_BY_ZERO: u32 = 6; pub const PRIVILEGED: u32 = 8;
    pub const LOAD_STORE_ALIGNMENT: u32 = 9; pub const IFETCH_PIF_DATA_ERROR: u32 = 12; pub const LS_PIF_DATA_ERROR: u32 = 13;
    pub const IFETCH_PIF_ADDR_ERROR: u32 = 14; pub const LS_PIF_ADDR_ERROR: u32 = 15;
    pub const ITLB_MISS: u32 = 16; pub const IFETCH_PROHIBITED: u32 = 20; pub const DTLB_MISS: u32 = 24;
    pub const LOAD_PROHIBITED: u32 = 28; pub const STORE_PROHIBITED: u32 = 29; pub const COPROCESSOR0_DISABLED: u32 = 32;
}

/// Vector offsets from VECBASE (ESP32-S3 core-isa.h).
pub mod vec {
    pub const WINDOW_OF4: u32 = 0x000; pub const WINDOW_UF4: u32 = 0x040; pub const WINDOW_OF8: u32 = 0x080; pub const WINDOW_UF8: u32 = 0x0C0;
    pub const WINDOW_OF12: u32 = 0x100; pub const WINDOW_UF12: u32 = 0x140;
    pub const LEVEL2: u32 = 0x180; pub const LEVEL3: u32 = 0x1C0; pub const LEVEL4: u32 = 0x200; pub const LEVEL5: u32 = 0x240;
    pub const DEBUG: u32 = 0x280; pub const NMI: u32 = 0x2C0; pub const KERNEL: u32 = 0x300; pub const USER: u32 = 0x340; pub const DOUBLE: u32 = 0x3C0;
}

pub const RESET_VECTOR: u32 = 0x4000_0400;
pub const NUM_AREGS: usize = 64;
pub const NUM_WINDOWS: u32 = (NUM_AREGS / 4) as u32;

/// Per-interrupt level (index = interrupt number 0..31), from core-isa.h.
pub const INT_LEVEL: [u8; 32] = [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 1, 1, 7, 3, 5, 1, 1, 2, 2, 2, 3, 3, 4, 4, 5, 3, 4, 3, 4, 5];
/// Interrupt type masks.
/// INT_ABOVE[l] = interrupts whose level is strictly greater than l (deliverable at PS.INTLEVEL = l).
pub const INT_ABOVE: [u32; 16] = {
    let mut t = [0u32; 16]; let mut l = 0;
    while l < 16 { let mut i = 0; while i < 32 { if INT_LEVEL[i] as usize > l { t[l] |= 1 << i; } i += 1; } l += 1; }
    t
};
pub const INTTYPE_SOFTWARE: u32 = 0x2000_0080;
pub const INTTYPE_EDGE: u32 = 0x5040_0400;
pub const INTTYPE_LEVEL: u32 = 0x8FBE_333F;
pub const INTTYPE_TIMER: u32 = 0x0001_8040;
pub const INTTYPE_NMI: u32 = 0x0000_4000;
pub const INTTYPE_PROFILING: u32 = 0x0000_0800;
pub const TIMER_INTERRUPT: [u32; 3] = [6, 15, 16];
pub const NMI_INTERRUPT: u32 = 14;
pub const EXCM_LEVEL: u32 = 3;

#[derive(Clone)]
pub struct Cpu {
    pub pc: u32,
    /// physical address registers; AR[n] = ar[(windowbase*4 + n) % 64]
    pub ar: [u32; NUM_AREGS],
    pub windowbase: u32,
    pub windowstart: u32,
    pub ps: u32,
    pub sar: u32,
    pub lbeg: u32,
    pub lend: u32,
    pub lcount: u32,
    pub br: u32,
    pub scompare1: u32,
    pub acclo: u32,
    pub acchi: u32,
    pub m: [u32; 4],
    pub epc: [u32; 8],      // epc[1..=7]
    pub eps: [u32; 8],      // eps[2..=7]
    pub excsave: [u32; 8],  // excsave[1..=7]
    pub depc: u32,
    pub vecbase: u32,
    pub exccause: u32,
    pub excvaddr: u32,
    pub debugcause: u32,
    pub interrupt: u32,
    pub intenable: u32,
    pub ccount: u32,
    /// Provisional uniform fast-path cost, not a calibrated silicon price.
    pub approximate_cpi: u32,
    /// Provisional PIE issue experiments: 1=staging memory +1, 2=SRC.Q.LD +2.
    pub approximate_pie_mode: u32,
    pub approximate_pie_events: u64,
    pub approximate_pie_cycles: u64,
    pub ccompare: [u32; 3],
    /// event-s1: CCOUNT one cycle before the nearest CCOMPARE match. `advance_ccount` and `write_sr`
    /// keep it; code that assigns `ccount` or `ccompare` directly must call `refresh_event`.
    #[cfg(target_arch = "wasm32")]
    pub event_at: u32,
    pub cpenable: u32,
    pub prid: u32,
    pub threadptr: u32,
    /// LX6 DFP accelerator context registers (F64R_LO, F64R_HI, F64S).
    /// ESP-IDF v4.4.8 and v5.5.4: components/xtensa/esp32/include/xtensa/config/tie.h, UR 234-236.
    pub f64: [u32; 3],
    /// ESP-IDF v4.4.8 and v5.5.4: components/xtensa/{esp32,esp32s3}/include/xtensa/config/core-isa.h
    /// sets XCHAL_HAVE_DFP_ACCEL to 1 on ESP32 and 0 on S3. Defaults to S3.
    pub lx6: bool,
    pub misc: [u32; 4],
    pub icount: u32,
    pub icountlevel: u32,
    pub ibreakenable: u32,
    pub ibreaka: [u32; 2],
    pub dbreaka: [u32; 2],
    pub dbreakc: [u32; 2],
    pub memctl: u32,
    pub atomctl: u32,
    pub ddr: u32,
    pub configid: [u32; 2],
    // FPU
    pub fr: [u32; 16],
    pub fcr: u32,
    pub fsr: u32,
    // PIE (ESP32-S3 SIMD) register file — stored so lazy context save/restore works
    pub qr: [u128; 8],
    pub accx: [u32; 2],
    pub qacc_h: [u32; 5],
    pub qacc_l: [u32; 5],
    pub sar_byte: u32,
    pub fft_bit_width: u32,
    pub ua_state: [u32; 4],
    pub gpio_out: u32,
    /// halted by WAITI until an interrupt arrives
    pub waiting: bool,
    /// Previous external interrupt input levels, used to detect rising edges.
    pub ext_irq_lines: u32,
    pub insn_count: u64,
    /// decoded-instruction cache: direct-mapped on pc, validated by the page write-version
    pub icache: Vec<crate::decode::CacheEntry>,
    /// basic-block cache used by `block::run_block` (the fast path)
    pub blocks: crate::block::BlockCache,
    /// pcs that must start a block (the machine's stubs and probes), as a bloom over `block::pc_bit`
    pub boundary_bloom: u64,
    /// trap raised inside native code, handed back to `block::run_block`
    pub jit_trap: Option<crate::exec::Trap>,
    /// A helper ran in this CPU's current WASM wrapper call, requiring redispatch.
    #[cfg(target_arch = "wasm32")]
    pub(crate) jit_helped: bool,
    /// EX138: cycles beyond one per instruction that priced control flow has accrued since the
    /// machine last collected them; charged only while `price_control` is set.
    pub timing_extra: u32,
    pub price_control: bool,
    /// EX147: instruction-fetch cache for flash-mapped code, 64 sets x 8 ways x 32-byte lines,
    /// one for both cores; `icache_fill` cycles per missing line (0 = off). Tags hold line + 1.
    pub icache_fill: u32, pub icache_misses: u64,
    pub fetch_cache: SharedFetchCache,
    /// Executed instruction byte ranges in a bounded compiled call (written by
    /// generated code, drained by `jit::run`).
    #[cfg(target_arch = "wasm32")]
    pub fetch_ring: [[u32; 2]; 64],
    #[cfg(target_arch = "wasm32")]
    pub fetch_n: u32,
}

impl Default for Cpu {
    fn default() -> Self { Self::new(0) }
}

/// Fetch tags shared only by the cores and bus of one machine.
#[derive(Clone)]
pub struct SharedFetchCache(std::sync::Arc<std::sync::Mutex<[[u32; 8]; 64]>>);

impl Default for SharedFetchCache {
    fn default() -> Self { Self(std::sync::Arc::new(std::sync::Mutex::new([[0; 8]; 64]))) }
}

impl SharedFetchCache {
    /// Invalidate this machine's tags after a remap or chip reset.
    pub fn reset(&self) { *self.0.lock().unwrap() = [[0; 8]; 64]; }
}

impl Cpu {
    /// EX147: fetch the 32-byte lines covering `lo..=hi` of flash-mapped code; misses are charged.
    #[inline]
    pub fn touch_fetch_lines(&mut self, lo: u32, hi: u32) {
        if self.icache_fill == 0 || !(0x4200_0000..0x4400_0000).contains(&lo) { return; }
        // One cache for both cores, as on the chip (the emulator is single-threaded).
        let mut shared = self.fetch_cache.0.lock().unwrap();
        for line in lo >> 5..=hi >> 5 {
            let set = &mut shared[(line & 63) as usize];
            if set[0] == line + 1 { continue; }   // already the most recent line of its set
            let way = match set.iter().position(|&t| t == line + 1) { Some(w) => w, None => { self.timing_extra += self.icache_fill; self.icache_misses += 1; 7 } };
            set.copy_within(0..way, 1);
            set[0] = line + 1;
        }
    }

    pub fn new(prid: u32) -> Self {
        let mut c = Cpu {
            pc: RESET_VECTOR,
            ar: [0; NUM_AREGS], windowbase: 0, windowstart: 1,
            ps: 0x1f, sar: 0, lbeg: 0, lend: 0, lcount: 0, br: 0, scompare1: 0, acclo: 0, acchi: 0, m: [0; 4],
            epc: [0; 8], eps: [0; 8], excsave: [0; 8], depc: 0, vecbase: 0x4000_0000, exccause: 0, excvaddr: 0, debugcause: 0,
            interrupt: 0, intenable: 0, ccount: 0, approximate_cpi: 1, approximate_pie_mode: 0, approximate_pie_events: 0, approximate_pie_cycles: 0, ccompare: [0; 3],
            #[cfg(target_arch = "wasm32")]
            event_at: u32::MAX,
            cpenable: 0, prid, threadptr: 0, f64: [0; 3], lx6: false, misc: [0; 4],
            icount: 0, icountlevel: 0, ibreakenable: 0, ibreaka: [0; 2], dbreaka: [0; 2], dbreakc: [0; 2], memctl: 0, atomctl: 0, ddr: 0,
            configid: [0xC2ECFAFE, 0x22F86EDF],   // reported by real S3 (informational)
            fr: [0; 16], fcr: 0, fsr: 0,
            qr: [0; 8], accx: [0; 2], qacc_h: [0; 5], qacc_l: [0; 5], sar_byte: 0, fft_bit_width: 0, ua_state: [0; 4], gpio_out: 0,
            waiting: false, ext_irq_lines: 0, insn_count: 0,
            icache: vec![crate::decode::CacheEntry::EMPTY; crate::decode::ICACHE_SIZE],
            #[cfg(target_arch = "wasm32")]
            jit_helped: false,
            blocks: crate::block::BlockCache::new(), boundary_bloom: 0, jit_trap: None, timing_extra: 0, price_control: false, icache_fill: 0, icache_misses: 0, fetch_cache: SharedFetchCache::default(),
            #[cfg(target_arch = "wasm32")]
            fetch_ring: [[0; 2]; 64],
            #[cfg(target_arch = "wasm32")]
            fetch_n: 0,
        };
        c.reset();
        c
    }

    /// Processor reset state (XEA2): PS.INTLEVEL=15, EXCM=1, WOE=0, UM=0, RING=0.
    pub fn reset(&mut self) {
        self.pc = RESET_VECTOR;
        self.ps = 0x1f;   // INTLEVEL=15 | EXCM
        self.windowbase = 0;
        self.windowstart = 1;
        self.vecbase = 0x4000_0000;
        self.intenable = 0;
        self.interrupt = 0;
        self.ext_irq_lines = 0;
        self.lcount = 0;
        self.cpenable = 0;
        self.icountlevel = 0;
        self.memctl = 1;      // observed reset value on ESP32-S3 silicon (rsr.memctl in the ROM reset path)
        self.waiting = false;
        // The ISA leaves CCOUNT undefined after a processor reset (Xtensa ISA Reference Manual,
        // 2010, 4.4.6.2); 0 follows silicon: on an ESP32-S3 the bootloader's first early-log
        // timestamp after esp_restart() (RTC_SW_CPU_RST) is the cold boot's (EX259).
        self.ccount = 0;
        #[cfg(target_arch = "wasm32")]
        self.refresh_event();
    }

    #[inline(always)]
    pub fn phys(&self, n: u8) -> usize { ((self.windowbase as usize) * 4 + n as usize) & (NUM_AREGS - 1) }
    #[inline(always)]
    pub fn get_ar(&self, n: u8) -> u32 { self.ar[self.phys(n)] }
    #[inline(always)]
    pub fn set_ar(&mut self, n: u8, v: u32) { let p = self.phys(n); self.ar[p] = v; }

    #[inline(always)]
    pub fn intlevel(&self) -> u32 { self.ps & ps::INTLEVEL_MASK }
    #[inline(always)]
    pub fn excm(&self) -> bool { self.ps & ps::EXCM != 0 }
    #[inline(always)]
    pub fn woe(&self) -> bool { self.ps & ps::WOE != 0 }
    #[inline(always)]
    pub fn callinc(&self) -> u32 { (self.ps & ps::CALLINC_MASK) >> ps::CALLINC_SHIFT }
    /// Register offset of the frame a pending `callN` set up: 4·CALLINC, the rotation its `entry` will apply.
    pub fn call_window(&self) -> u8 { (self.callinc() * 4) as u8 }
}
