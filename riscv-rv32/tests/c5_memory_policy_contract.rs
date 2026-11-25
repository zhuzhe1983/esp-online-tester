//! Run the independent contract fixture against the actual workspace module.
extern crate self as c5_memory_protection_harness;
pub use riscv_rv32::c5_memory_protection as policy;
#[path="../../tools/c5-memory-protection-harness/tests/policy.rs"]
mod contract;
