//! Run a caller-supplied SRAM/ROM ELF and report the first unimplemented access.
use std::io::Write;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let first = args.next().ok_or("usage: bringup [--rom] <RISC-V SRAM/ROM ELF> [UART RX text] [step budget]")?;
    let rom = first == "--rom";
    let elf = if rom { args.next().ok_or("--rom needs the S31 ROM ELF path")? } else { first };
    let input = args.next().unwrap_or_default();
    let budget = args.next().map(|s| s.parse::<u64>()).transpose()?.unwrap_or(100_000).min(10_000_000);
    let mut m = esp32s31::Bringup::new(); let bytes = std::fs::read(elf)?;
    if rom { m.load_rom_elf(&bytes)?; } else { m.load_elf(&bytes)?; }
    m.bus.uart0.host_input(input.as_bytes()); let run = m.run(budget);
    std::io::stdout().write_all(&m.bus.uart0.tx_out)?;
    eprintln!("stop={:?} steps={} pc={:#010x} mepc={:#010x} mcause={:#x} mtval={:#010x} fault={:?}",
        run.stop, run.steps, m.cpu.pc, m.cpu.mepc, m.cpu.mcause, m.cpu.mtval, m.bus.first_fault);
    Ok(())
}
