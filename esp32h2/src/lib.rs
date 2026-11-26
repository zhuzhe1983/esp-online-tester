//! Independent ESP32-H2 SRAM app-direct subset, chip ID16. No C6 SoC alias.
//! HP320KiB/LP4KiB, RV32IMAC at nominal96MHz, 19 physical GPIOs (28 register positions), UART FIFO polling and
//! finite RMT TX at the board's32MHz crystal. Unknown MMIO and ROM execution fault.
pub mod board;
pub mod nanoh2;
pub mod gateway;
pub mod bus;
pub mod image;
pub mod soc;
mod rmt;
pub use bus::H2Bus;
pub use soc::{machine,Machine,H2};
pub(crate) fn core()->riscv_rv32::Cpu{let mut c=riscv_rv32::Cpu::new_rv32imac();c.pc=bus::ROM_BASE;c}
