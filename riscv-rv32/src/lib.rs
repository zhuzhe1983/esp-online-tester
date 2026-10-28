//! RV32IMC / RV32IMAC: the core in the ESP32-C3 and, with the A extension, the ESP32-C6. With F
//! in `misa` (`Cpu::new_esp32p4`) it is RV32IMAFC, with `mstatus.FS` and the P4's EXT_ILL CSR,
//! the Zicntr counters, CLIC mode and a hart ID. `Cpu::new_esp32s31` selects the
//! same base ISA subset with the standard CLIC CSR placement used by S31 IDF v6.1.
//!
//! Machine mode only, no MMU, no D extension. The C3 and C6 have no F, and decode its encodings
//! as illegal.
//! Interrupts are *not* the standard `mie`/`mip` external-interrupt model: the C3 routes 62
//! peripheral sources through its own interrupt matrix onto 31 CPU lines and vectors each one
//! separately (`mtvec` in vectored mode), so the SoC hands us the pending line and we take it.
//! On the P4 the SoC hands over its CLIC's candidate instead, and the core applies its own
//! interrupt level (`exec::CLIC_IRQ_*`).
pub mod bus { pub use emu_core::bus::*; }
pub mod core;
pub mod decode;
pub mod disasm;
pub mod exec;
mod fpu;
pub mod softfloat;
pub mod state;

pub use emu_core::{Bus, Core, Fault, FlatRam};
pub use decode::{decode, decode_for, Insn, Op};
pub use exec::{step, Trap};
pub use state::Cpu;
