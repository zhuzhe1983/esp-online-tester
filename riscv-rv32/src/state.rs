//! Machine-mode CPU state. Only M-mode exists on the C3, so there is no privilege stack beyond
//! `mstatus.MPP` and no address translation.

use std::collections::HashMap;

pub mod csr {
    /// The F extension's CSRs (RISC-V unprivileged ISA, F, "Floating-Point Control and Status Register").
    pub const FFLAGS: u32 = 0x001; pub const FRM: u32 = 0x002; pub const FCSR: u32 = 0x003;
    pub const MSTATUS: u32 = 0x300; pub const MISA: u32 = 0x301; pub const MIE: u32 = 0x304;
    pub const MTVEC: u32 = 0x305; pub const MSCRATCH: u32 = 0x340; pub const MEPC: u32 = 0x341;
    pub const MCAUSE: u32 = 0x342; pub const MTVAL: u32 = 0x343; pub const MIP: u32 = 0x344;
    pub const MCYCLE: u32 = 0xB00; pub const MINSTRET: u32 = 0xB02;
    pub const MCYCLEH: u32 = 0xB80; pub const MINSTRETH: u32 = 0xB82;
    pub const MVENDORID: u32 = 0xF11; pub const MARCHID: u32 = 0xF12; pub const MIMPID: u32 = 0xF13;
    pub const MHARTID: u32 = 0xF14;
    /// The unprivileged counters (RISC-V unprivileged ISA, "Zicntr"): read-only shadows of
    /// mcycle and minstret.
    pub const CYCLE: u32 = 0xC00; pub const INSTRET: u32 = 0xC02;
    pub const CYCLEH: u32 = 0xC80; pub const INSTRETH: u32 = 0xC82;
    /// Espressif performance counters. The machine-mode set is what `esp_cpu_get_cycle_count`
    /// uses; the user-mode alias at 0x802 is what the ROM's `ets_delay_us` busy-waits on.
    pub const MPCER: u32 = 0x7E0; pub const MPCMR: u32 = 0x7E1; pub const MPCCR: u32 = 0x7E2;
    pub const PCER_U: u32 = 0x800; pub const PCMR_U: u32 = 0x801; pub const PCCR_U: u32 = 0x802;
    /// CLIC (RISC-V "Core-Local Interrupt Controller" specification): the hardware-vectoring table
    /// base, and the ESP32-P4 rev < 3's `mintstatus`, which sits at 0x346 instead of 0xFB1
    /// (ESP-IDF v5.5.3 riscv/include/esp_private/interrupt_clic.h:46-58).
    pub const MTVT: u32 = 0x307; pub const MINTSTATUS_P4: u32 = 0x346;
    /// The specification's `mintstatus` and `mintthresh`, as on the ESP32-P4 rev >= 3
    /// (ESP-IDF v5.5.4 riscv/include/esp_private/interrupt_clic.h:60-66).
    /// S31 uses the same placement (IDF v6.1 soc/esp32s31/include/soc/interrupt_reg.h,
    /// INTTHRESH_STANDARD=1, and riscv/include/riscv/csr_clic.h).
    pub const MINTSTATUS: u32 = 0xFB1; pub const MINTTHRESH: u32 = 0x347;
    /// ESP32-P4 EXT_ILL: why the last illegal-instruction trap was taken
    /// (ESP-IDF v5.5.3 `components/riscv/vectors.S`, `EXT_ILL_CSR`).
    pub const EXT_ILL: u32 = 0x7F0;
}

/// EXT_ILL reason bits (ESP-IDF v5.5.3 `components/riscv/vectors.S`, `EXT_ILL_RSN_*`).
pub mod ext_ill {
    pub const FPU: u32 = 1; pub const HWLP: u32 = 2; pub const PIE: u32 = 4;
}

pub mod exc {
    pub const INSN_MISALIGNED: u32 = 0; pub const INSN_ACCESS_FAULT: u32 = 1;
    pub const ILLEGAL_INSN: u32 = 2; pub const BREAKPOINT: u32 = 3;
    pub const LOAD_MISALIGNED: u32 = 4; pub const LOAD_ACCESS_FAULT: u32 = 5;
    pub const STORE_MISALIGNED: u32 = 6; pub const STORE_ACCESS_FAULT: u32 = 7;
    pub const ECALL_M: u32 = 11;
}

/// `mcause` in CLIC mode: the interrupt bit, MINHV (a vector-table fetch is in progress), MPP and
/// MPIE (aliases of mstatus's), MPIL (the interrupt level before the trap), a 12-bit cause
/// (ESP-IDF v5.5.3 riscv/include/esp_private/vectors_const.h; CLIC specification, "mcause").
pub mod mcause {
    pub const INTERRUPT: u32 = 1 << 31; pub const MINHV: u32 = 1 << 30;
    pub const MPP: u32 = 3 << 28; pub const MPIE: u32 = 1 << 27;
    pub const MPIL_SHIFT: u32 = 16; pub const MPIL: u32 = 0xff << 16;
    pub const CODE: u32 = 0xfff;
}

pub mod mstatus {
    pub const MIE: u32 = 1 << 3;
    pub const MPIE: u32 = 1 << 7;
    pub const MPP: u32 = 3 << 11;
    /// FP unit status, Off/Initial/Clean/Dirty, and SD = some state Dirty (RISC-V privileged ISA
    /// v1.13, "Extension Context Status in mstatus Register"). Modelled only on a core with F.
    pub const FS: u32 = 3 << 13;
    pub const FS_INITIAL: u32 = 1 << 13;
    pub const FS_CLEAN: u32 = 2 << 13;
    pub const FS_DIRTY: u32 = 3 << 13;
    pub const SD: u32 = 1 << 31;
}

/// Reset vector: the C3 and the C6 start executing in the mask ROM.
pub const RESET_VECTOR: u32 = 0x4000_0000;
/// `misa` for the two cores: RV32IMC (bits I|M|C, MXL=1) and RV32IMAC.
pub const MISA_RV32IMC: u32 = 0x4000_1104;
pub const MISA_RV32IMAC: u32 = 0x4000_1105;
/// `misa` bit F: the core decodes and executes RV32F and its compressed loads and stores.
pub const MISA_F: u32 = 1 << 5;
/// RV32IMAFC, the ESP32-P4's ISA without its custom extensions (ESP-IDF v5.5.3
/// `tools/cmake/toolchain-esp32p4.cmake`). Whether silicon also sets X is not known.
pub const MISA_RV32IMAFC: u32 = MISA_RV32IMAC | MISA_F;

pub struct Cpu {
    pub pc: u32,
    pub x: [u32; 32],
    pub mstatus: u32,
    pub mtvec: u32,
    pub mepc: u32,
    pub mcause: u32,
    pub mtval: u32,
    pub mie: u32,
    pub mscratch: u32,
    /// Host-facing scheduling work. This remains monotonic across reset and includes skipped idle
    /// cycles so existing run statistics keep their meaning.
    pub insn_count: u64,
    /// Architecturally retired instructions. Traps accepted between instructions and trapping
    /// instructions do not increment this counter.
    pub retired_count: u64,
    /// Architectural cycles, including timing-only advancement while no instruction retires.
    pub cycle_count: u64,
    /// Counter bases at the last reset. Guest counters restart at zero while the host-facing
    /// monotonic counters do not.
    pub cycle_base: u64,
    pub instret_base: u64,
    /// halted by WFI until the SoC raises a line
    pub waiting: bool,
    /// the line the SoC's interrupt controller wants taken, if any (`Core::set_irq`)
    pub irq: Option<u32>,
    /// which ISA `misa` reports: IMC on the C3, IMAC on the C6 (execution accepts A on both).
    /// F here is what enables the F extension, so a core without it decodes F encodings as illegal.
    pub misa: u32,
    /// the core has the ESP32-P4's EXT_ILL CSR (0x7F0); without it 0x7F0 is an unmodelled CSR
    pub has_ext_ill: bool,
    /// f0-f31 as binary32 bit patterns (FLEN = 32, so no NaN-boxing)
    pub f: [u32; 32],
    /// frm in bits 7:5, the accrued flags in bits 4:0; bits 31:8 read zero
    pub fcsr: u32,
    /// EXT_ILL's value, when `has_ext_ill`
    pub ext_ill: u32,
    /// EXT_ILL flags every FP instruction taken with FS = Off, FLW/FSW, their compressed forms and
    /// the FP CSRs included, as on the ESP32-P4 rev >= 3. Without it those are left unflagged
    /// (SOC_CPU_HAS_FPU_EXT_ILL_BUG, rev < 3) and IDF's `vectors.S` decodes `mtval` for them.
    pub ext_ill_all_fp: bool,
    /// `cycle`/`instret` (Zicntr) read the machine counters; without it they are unmodelled CSRs,
    /// as they always were on the C3 and C6, which read the cycle count from MPCCR
    pub user_counters: bool,
    /// CLIC mode exists (`mtvec` mode 3, `mtvt`, `mintstatus`, levels): the SoC presents the
    /// candidate as `line | level << 8 | SHV << 16` (`exec::CLIC_IRQ_*`). Without it the presented
    /// value is a line the SoC has already arbitrated, as on the C3 and C6.
    pub clic: bool,
    /// the CLIC is the specification's: the threshold is the `mintthresh` CSR and `mintstatus`
    /// sits at 0xFB1, as on the ESP32-P4 rev >= 3. Without it the SoC applies a memory-mapped
    /// threshold and `mintstatus` is at 0x346, as on rev < 3.
    pub clic_std: bool,
    /// mintthresh.th when `clic_std`: an interrupt is taken only above it (and above `mil`)
    pub mintthresh: u32,
    /// the hardware-vectoring table base (CSR 0x307), 64-byte aligned
    pub mtvt: u32,
    /// what mhartid reads: which core of the chip this is; a reset keeps it
    pub hartid: u32,
    /// mintstatus.mil: the level of the interrupt being handled, 0 outside any handler
    pub mil: u32,
    /// the address an `lr.w` reserved, until the next `sc.w`
    pub reservation: Option<u32>,
    /// pcs the machine intercepts (stubs, probes), as a bloom over `emu_core::pc_bit`: `run` stops there
    pub boundary_bloom: u64,
    /// CSRs we do not model: read back what was written, like the SoC's unknown registers
    pub csr_other: HashMap<u32, u32>,
}

impl Default for Cpu {
    fn default() -> Self { Self::new() }
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            pc: RESET_VECTOR, x: [0; 32],
            mstatus: 0x1800,          // MPP = machine
            mtvec: 0, mepc: 0, mcause: 0, mtval: 0, mie: 0, mscratch: 0,
            insn_count: 0, retired_count: 0, cycle_count: 0, cycle_base: 0, instret_base: 0, waiting: false, irq: None, misa: MISA_RV32IMC,
            has_ext_ill: false, f: [0; 32], fcsr: 0, ext_ill: 0, ext_ill_all_fp: false, user_counters: false, clic: false, clic_std: false, mintthresh: 0, mtvt: 0, hartid: 0, mil: 0, reservation: None, boundary_bloom: 0, csr_other: HashMap::new(),
        }
    }
    pub fn new_rv32imac() -> Self { let mut c = Cpu::new(); c.misa = MISA_RV32IMAC; c }
    /// An ESP32-P4 HP core: RV32IMAFC with EXT_ILL and the Zicntr counters, which IDF reads the
    /// cycle count from (`rdcycle`, ESP-IDF v5.5.3 riscv/include/riscv/rv_utils.h:106-108). Its
    /// custom extensions (Xesppie, hardware loops) are not modelled and decode as illegal. FS is
    /// Off at reset, as on any core with F.
    pub fn new_esp32p4() -> Self { let mut c = Cpu::new(); c.misa = MISA_RV32IMAFC; c.has_ext_ill = true; c.user_counters = true; c.clic = true; c }
    /// An ESP32-P4 rev >= 3 HP core: the same, with the specification's CLIC CSRs and EXT_ILL
    /// flagging every FP instruction ("The FPU_EXT_ILL bug was fixed on revision 3.0 and higher",
    /// ESP-IDF v5.5.4 riscv/vectors.S:279).
    pub fn new_esp32p4_rev3() -> Self { let mut c = Cpu::new_esp32p4(); c.clic_std = true; c.ext_ill_all_fp = true; c }
    /// S31's RV32IMAFC subset and standard CLIC CSRs (IDF v6.1 soc_caps.h, interrupt_reg.h).
    /// Custom PIE/HWLOOP instructions remain unimplemented. The SoC supplies its own reset PC.
    pub fn new_esp32s31() -> Self { let mut c = Cpu::new(); c.misa = MISA_RV32IMAFC; c.has_ext_ill = true; c.user_counters = true; c.clic = true; c.clic_std = true; c }

    pub fn reset(&mut self) {
        // `insn_count` is the emulator's own monotonic counter — it drives the run statistics and
        // the trace, so a chip reset must not rewind it (a reset that did made the browser UI
        // report a negative instruction rate). The architectural cycle counter is separately
        // monotonic and firmware reads its reset-relative value.
        let (insns, retired, cycles, misa, ext_ill, counters, clic, clic_std, hart, bloom) = (self.insn_count, self.retired_count, self.cycle_count, self.misa, self.has_ext_ill, self.user_counters, self.clic, self.clic_std, self.hartid, self.boundary_bloom);
        let all_fp = self.ext_ill_all_fp;
        *self = Cpu::new();
        self.insn_count = insns; self.retired_count = retired; self.cycle_count = cycles;
        self.cycle_base = cycles; self.instret_base = retired;
        self.misa = misa; self.has_ext_ill = ext_ill; self.user_counters = counters; self.clic = clic; self.clic_std = clic_std; self.hartid = hart; self.boundary_bloom = bloom; self.ext_ill_all_fp = all_fp;
    }

    #[inline(always)]
    pub fn has_f(&self) -> bool { self.misa & MISA_F != 0 }
    /// FS = Off: every F instruction and FP CSR access is illegal (privileged ISA v1.13,
    /// "Extension Context Status in mstatus Register").
    #[inline(always)]
    pub fn fs_off(&self) -> bool { self.mstatus & mstatus::FS == 0 }
    /// Any write to an f register or to fcsr makes the FP state Dirty, and SD follows.
    #[inline(always)]
    pub fn set_fs_dirty(&mut self) { self.mstatus |= mstatus::FS_DIRTY | mstatus::SD; }
    /// OR exception flags into fflags; any raised flag is a write to fcsr.
    #[inline(always)]
    pub fn accrue_fflags(&mut self, flags: u32) { if flags != 0 { self.fcsr |= flags; self.set_fs_dirty(); } }

    #[inline(always)]
    pub fn get(&self, r: u8) -> u32 { self.x[r as usize] }
    #[inline(always)]
    pub fn set(&mut self, r: u8, v: u32) { if r != 0 { self.x[r as usize] = v; } }

    #[inline(always)]
    pub fn mie_enabled(&self) -> bool { self.mstatus & mstatus::MIE != 0 }

    pub fn read_csr(&mut self, n: u32) -> u32 {
        use csr::*;
        match n {
            MSTATUS => self.mstatus,
            MISA => self.misa,
            MIE => self.mie,
            MTVEC => self.mtvec,
            MSCRATCH => self.mscratch,
            MEPC => self.mepc,
            MCAUSE if self.clic_mode() => self.clic_mcause(),
            MCAUSE => self.mcause,
            MTVAL => self.mtval,
            MTVT if self.clic => self.mtvt,
            MINTSTATUS_P4 if self.clic && !self.clic_std => self.mil << 24,
            MINTSTATUS if self.clic_std => self.mil << 24,
            MINTTHRESH if self.clic_std => self.mintthresh,
            MIP => 0,                               // the SoC's INTC holds pending state, not `mip`
            MCYCLE | MPCCR | PCCR_U => self.cycle_count.wrapping_sub(self.cycle_base) as u32,
            MCYCLEH => (self.cycle_count.wrapping_sub(self.cycle_base) >> 32) as u32,
            MINSTRET => self.retired_count.wrapping_sub(self.instret_base) as u32,
            MINSTRETH => (self.retired_count.wrapping_sub(self.instret_base) >> 32) as u32,
            CYCLE if self.user_counters => self.cycle_count.wrapping_sub(self.cycle_base) as u32,
            CYCLEH if self.user_counters => (self.cycle_count.wrapping_sub(self.cycle_base) >> 32) as u32,
            INSTRET if self.user_counters => self.retired_count.wrapping_sub(self.instret_base) as u32,
            INSTRETH if self.user_counters => (self.retired_count.wrapping_sub(self.instret_base) >> 32) as u32,
            MVENDORID => 0, MARCHID => 0, MIMPID => 0, MHARTID => self.hartid,
            FFLAGS if self.has_f() => self.fcsr & 0x1f,
            FRM if self.has_f() => (self.fcsr >> 5) & 7,
            FCSR if self.has_f() => self.fcsr,
            EXT_ILL if self.has_ext_ill => self.ext_ill,
            _ => self.csr_other.get(&n).copied().unwrap_or(0),
        }
    }

    pub fn write_csr(&mut self, n: u32, v: u32) {
        use csr::*;
        match n {
            // FS is writable and SD read-only on a core with F; without F, mstatus keeps whatever
            // was written, as it always has on the C3 and C6.
            MSTATUS if self.has_f() => {
                let sd = if v & mstatus::FS == mstatus::FS_DIRTY { mstatus::SD } else { 0 };
                self.mstatus = (v & !mstatus::SD) | sd;
            }
            MSTATUS => self.mstatus = v,
            FFLAGS if self.has_f() => { self.fcsr = (self.fcsr & !0x1f) | (v & 0x1f); self.set_fs_dirty(); }
            FRM if self.has_f() => { self.fcsr = (self.fcsr & 0x1f) | ((v & 7) << 5); self.set_fs_dirty(); }
            FCSR if self.has_f() => { self.fcsr = v & 0xff; self.set_fs_dirty(); }
            EXT_ILL if self.has_ext_ill => self.ext_ill = v,
            MIE => self.mie = v,
            MTVEC => self.mtvec = v,
            MSCRATCH => self.mscratch = v,
            MEPC => self.mepc = v & !1,
            // MPP and MPIE are mstatus's own bits seen through mcause (CLIC specification, "mcause")
            MCAUSE if self.clic_mode() => {
                self.mcause = v & (mcause::INTERRUPT | mcause::MINHV | mcause::MPIL | mcause::CODE);
                self.mstatus = (self.mstatus & !(mstatus::MPP | mstatus::MPIE)) | ((v & mcause::MPP) >> 17) | ((v & mcause::MPIE) >> 20);
            }
            MCAUSE => self.mcause = v,
            MTVAL => self.mtval = v,
            MTVT if self.clic => self.mtvt = v & !0x3f,
            MINTSTATUS_P4 if self.clic && !self.clic_std => {}
            MINTSTATUS if self.clic_std => {}
            MINTTHRESH if self.clic_std => self.mintthresh = v & 0xff,
            // a write moves the guest-visible counter; the emulator's own count stays monotonic
            MCYCLE | MPCCR | PCCR_U => { let c = self.cycle_count.wrapping_sub(self.cycle_base) & !0xffff_ffff | v as u64; self.cycle_base = self.cycle_count.wrapping_sub(c); }
            MCYCLEH => { let c = self.cycle_count.wrapping_sub(self.cycle_base) & 0xffff_ffff | ((v as u64) << 32); self.cycle_base = self.cycle_count.wrapping_sub(c); }
            MINSTRET => { let c = self.retired_count.wrapping_sub(self.instret_base) & !0xffff_ffff | v as u64; self.instret_base = self.retired_count.wrapping_sub(c); }
            MINSTRETH => { let c = self.retired_count.wrapping_sub(self.instret_base) & 0xffff_ffff | ((v as u64) << 32); self.instret_base = self.retired_count.wrapping_sub(c); }
            MISA | MIP | MVENDORID | MARCHID | MIMPID | MHARTID => {}   // read-only
            CYCLE | CYCLEH | INSTRET | INSTRETH if self.user_counters => {}
            _ => { self.csr_other.insert(n, v); }
        }
    }

    /// mcause as CLIC mode shows it: MPP and MPIE read through from mstatus.
    fn clic_mcause(&self) -> u32 { self.mcause | ((self.mstatus & mstatus::MPP) << 17) | ((self.mstatus & mstatus::MPIE) << 20) }

    /// `mtvec` mode 3: CLIC mode.
    #[inline(always)]
    pub fn clic_mode(&self) -> bool { self.clic && self.mtvec & 3 == 3 }

    /// Enter the trap handler. `cause` has bit 31 set for interrupts; `tval` goes to `mtval`.
    /// `mtvec` mode 1 vectors interrupts to `base + 4*line` — how the C3 dispatches its 31 lines —
    /// while exceptions always enter at the base. In CLIC mode an exception enters at the
    /// 64-byte-aligned base and records the current level in MPIL; the level does not change.
    pub fn trap(&mut self, cause: u32, tval: u32, epc: u32) {
        // An SC after the handler returns must not succeed over the handler's own stores; the
        // P4's IDF vectors issue no SC to break the reservation, so the trap does.
        if self.clic { self.reservation = None; }
        self.mepc = epc;
        self.mcause = if self.clic_mode() { (cause & (mcause::INTERRUPT | mcause::CODE)) | self.mil << mcause::MPIL_SHIFT } else { cause };
        self.mtval = tval;
        let mpie = if self.mstatus & mstatus::MIE != 0 { mstatus::MPIE } else { 0 };
        self.mstatus = (self.mstatus & !(mstatus::MIE | mstatus::MPIE)) | mpie | mstatus::MPP;
        let base = if self.clic_mode() { self.mtvec & !0x3f } else { self.mtvec & !3 };
        self.pc = if self.mtvec & 3 == 1 && cause & 0x8000_0000 != 0 {
            base.wrapping_add(4 * (cause & 0x1f))
        } else {
            base
        };
        self.waiting = false;
    }

    /// `mret`: restore the interrupt-enable bit and jump back; with a CLIC the level comes back
    /// from mcause.MPIL.
    pub fn mret(&mut self) {
        if self.clic_mode() { self.mil = (self.mcause & mcause::MPIL) >> mcause::MPIL_SHIFT; }
        let mpie = self.mstatus & mstatus::MPIE != 0;
        self.mstatus = (self.mstatus & !mstatus::MIE) | if mpie { mstatus::MIE } else { 0 } | mstatus::MPIE;
        self.pc = self.mepc;
    }
}
