//! Run a caller-supplied SRAM ELF or official ROM and preserve the first unsupported access.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().ok_or("usage: bringup ram|rom ELF [instruction-budget]")?;
    let path = args.next().ok_or("missing ELF path")?;
    let budget = args.next().map(|n| n.parse::<u64>()).transpose()?.unwrap_or(1_000_000);
    if args.next().is_some() { return Err("unexpected argument".into()); }
    let mut m = esp32c5::Bringup::new();
    m.bus.attach_board(Box::new(esp32c5::board::StampC5::new(false)));
    let elf = std::fs::read(path)?;
    match mode.as_str() { "ram" => m.load_elf(&elf)?, "rom" => m.load_rom_elf(&elf)?, _ => return Err("mode must be ram or rom".into()) }
    let run = m.run(budget);
    print!("{}", String::from_utf8_lossy(&m.bus.uart[0].tx_out));
    eprintln!("steps={} stop={:?} pc={:#010x} first_fault={:?}", run.steps, run.stop, m.cpu.pc, m.bus.first_fault);
    Ok(())
}
