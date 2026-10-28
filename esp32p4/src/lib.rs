//! ESP32-P4: two RISC-V HP cores (RV32IMAFC), 768 KB of L2MEM (code and data in one space, an uncached alias
//! at +0x4000_0000), 8 KB of TCM, 32 KB of LP RAM, and separate flash and PSRAM windows, each
//! behind its own 1024-entry MMU. No radio.
//!
//! The silicon revision is a type parameter (`Revision`), because rev < 3 and rev >= 3 differ in
//! ISA, interrupt-threshold location and ROM. `P4` is rev < 3 (v0.x, v1.x), `P4Rev3` rev >= 3.
//!
//! Shared IP (UART, USB-Serial/JTAG, systimer, timer groups, SPI flash controller, efuse) comes
//! from `esp-periph`. Written here: the memory map, the interrupt matrices and CLIC register
//! blocks, the cache controller, HP_SYSTEM, HP_SYS_CLKRST, the LP system blocks and the analog
//! I2C master. Each core's CLIC candidate is presented to the core, whose CLIC mode takes it
//! (`periph::Clic`, `riscv_rv32::exec::CLIC_IRQ_*`).
pub mod board;
pub mod bus;
pub mod coproc;
pub mod periph;
pub mod soc;

pub use esp_soc::Stop;
pub use soc::{built_for_rev3, machine, machine_rev, Esp32p4, Machine, P4Rev3, RevGe3, RevLt3, Revision, P4};
