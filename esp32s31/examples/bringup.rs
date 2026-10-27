//! Run a caller-supplied SRAM/ROM ELF and report the first unimplemented access.
use std::io::Write;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1).peekable(); let mut rom = false; let mut usb_host = false; let mut strap = None; let mut flash = None; let mut spi0_shadow = false; let mut nor = None; let mut bias = None;
    while let Some(arg) = args.peek() {
        match arg.as_str() {
            "--rom" => rom = true, "--usb-host" => usb_host = true,
            "--rom-spi0-shadow" => spi0_shadow = true,
            value if value.starts_with("--analog-bias=") => {
                let text = &value[14..]; if text.len() != 4 { return Err("analog BIAS requires exactly four hex digits (register0 then register1)".into()); }
                let word = u16::from_str_radix(text, 16)?; bias = Some([(word >> 8) as u8, word as u8]);
            }
            value if value.starts_with("--nor-jedec=") => {
                let text = &value[12..]; if text.len() != 6 { return Err("NOR JEDEC requires exactly six hex digits".into()); }
                let word = u32::from_str_radix(text, 16)?;
                nor = Some([(word >> 16) as u8, (word >> 8) as u8, word as u8]);
            }
            value if value.starts_with("--strap=0x") => strap = Some(u16::from_str_radix(&value[10..], 16)?),
            value if value.starts_with("--flash=") => flash = Some(value[8..].to_owned()),
            _ => break,
        }
        args.next();
    }
    let elf = args.next().ok_or("usage: bringup [--rom] [--usb-host] [--strap=0xNN] [--flash=path] [--rom-spi0-shadow] [--nor-jedec=hhhhhh] [--analog-bias=hhhh] <RISC-V SRAM/ROM ELF> [UART RX text] [step budget]")?;
    let input = args.next().unwrap_or_default();
    let budget = args.next().map(|s| s.parse::<u64>()).transpose()?.unwrap_or(100_000).min(10_000_000);
    if args.next().is_some() { return Err("too many bringup arguments".into()); }
    let mut m = esp32s31::Bringup::new(); let bytes = std::fs::read(elf)?;
    if rom { m.load_rom_elf(&bytes)?; } else { m.load_elf(&bytes)?; }
    if let Some(path) = flash {
        let bytes = std::fs::read(path)?; m.bus.cache.set_flash(&bytes)?;
        eprintln!("flash bytes={} (caller input; no inferred NOR identity or OTP image)", bytes.len());
    }
    if let Some(id) = nor {
        m.bus.mspi.attach_flash(id, 0).map_err(|e| format!("invalid NOR descriptor: {e:?}"))?;
        eprintln!("development NOR JEDEC={id:02x?} status=0 (caller input, not identified product hardware)");
    }
    if let Some(bytes) = bias { m.bus.analog.set_bias(bytes); eprintln!("development analog BIAS seed={bytes:02x?} (caller input, no public reset value or voltage calibration)"); }
    m.bus.usb_console.attach_host(usb_host);
    if spi0_shadow { m.bus.mspi.enable_legacy_rom_lengths(); eprintln!("development SPI0 legacy length shadows enabled (ROM/upstream inference, not verified silicon)"); }
    if let Some(value) = strap { m.bus.pads.strap = value; }
    m.bus.uart_input(input.as_bytes()).map_err(|e| format!("UART input unavailable: {e:?}"))?;
    let mut steps = 0; let mut stop = esp32s31::Stop::Budget; let mut usb = Vec::new(); let mut packets = 0;
    while steps < budget {
        // Explicit functional host polling, not a USB frame/time guarantee. Without this
        // opt-in, no peer drains packets or fabricates receive data/readiness.
        let run = m.run((budget - steps).min(1000)); steps += run.steps; stop = run.stop;
        if let Some(packet) = m.bus.usb_console.host_receive() { packets += 1; usb.extend(packet); }
        if stop != esp32s31::Stop::Budget { break; }
    }
    std::io::stdout().write_all(&m.bus.uart0.tx_out)?;
    eprintln!("stop={stop:?} steps={steps} pc={:#010x} mepc={:#010x} mcause={:#x} mtval={:#010x} fault={:?}",
        m.cpu.pc, m.cpu.mepc, m.cpu.mcause, m.cpu.mtval, m.bus.first_fault);
    eprintln!("mspi-commands={} cache-maintenance={} sha-blocks={}", m.bus.mspi.commands, m.bus.cache.maintenance_operations, m.bus.sha.blocks());
    eprintln!("analog-transactions={} bias={:?} mpll-shutdown={}", m.bus.analog.transactions, m.bus.analog.bias(), m.bus.power.mpll_disabled());
    if usb_host { eprintln!("usb-host packets={packets} bytes={} data={:?}", usb.len(), String::from_utf8_lossy(&usb)); }
    Ok(())
}
