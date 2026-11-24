//! Common-input inactive-memory path for before/after CPU-cost comparison.
//! This executes a guest SRAM loop; it is not a factory or PSRAM benchmark.
use esp32c5::{Bringup, Stop, bus::SRAM_BASE};
fn main() {
    let count = std::env::args().nth(1).map(|s| s.parse::<u64>().expect("dispatch count")).unwrap_or(100_000_000);
    let code = [0x0012_8293u32, 0xffdf_f06f].into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>(); // addi t0,t0,1; j -4
    let mut m = Bringup::new(); m.bus.load_ram(SRAM_BASE, &code, code.len()).unwrap(); m.cpu.pc = SRAM_BASE;
    let result = m.run(count); assert_eq!(result.stop, Stop::Budget); assert!(m.bus.first_fault.is_none());
    assert_eq!(m.cpu.x[5], count.div_ceil(2) as u32);
    println!("{{\"dispatched\":{},\"retired\":{},\"cycles\":{},\"t0\":{},\"uartBytes\":{}}}", result.steps, m.cpu.retired_count, m.cpu.cycle_count, m.cpu.x[5], m.bus.uart[0].tx_out.len());
}
