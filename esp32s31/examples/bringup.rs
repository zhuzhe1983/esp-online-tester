//! Run a caller-supplied SRAM/ROM ELF and report the first unimplemented access.
use std::io::Write;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1).peekable(); let mut rom = false; let mut usb_host = false; let mut strap = None; let mut flash = None; let mut spi0_shadow = false; let mut nor = None; let mut bias = None; let mut perif = None; let mut dig = None; let mut sfdp = None; let mut bbpll = None; let mut rtc_timer_from_cpu = false; let mut watch_pc = None;
    while let Some(arg) = args.peek() {
        match arg.as_str() {
            "--rom" => rom = true, "--usb-host" => usb_host = true,
            value if value.starts_with("--watch-pc=0x") => watch_pc = Some(u32::from_str_radix(&value[13..], 16)?),
            "--rtc-timer-from-cpu" => rtc_timer_from_cpu = true,
            "--rom-spi0-shadow" => spi0_shadow = true,
            value if value.starts_with("--bbpll-hz=") => bbpll = Some(value["--bbpll-hz=".len()..].parse::<u64>()?),
            value if value.starts_with("--nor-sfdp=") => {
                let text = &value["--nor-sfdp=".len()..]; let (offset, hex) = text.split_once(':').ok_or("SFDP needs hex-offset:hex-bytes")?;
                if !hex.is_ascii() || hex.is_empty() || hex.len() > 131072 || !hex.len().is_multiple_of(2) { return Err("invalid SFDP byte interval".into()); }
                let bytes: Result<Vec<_>, _> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16)).collect();
                sfdp = Some((u32::from_str_radix(offset.trim_start_matches("0x"), 16)?, bytes?));
            }
            value if value.starts_with("--analog-dig=") => {
                let text = &value["--analog-dig=".len()..]; if text.len() != 8 { return Err("analog DIG registers5,7,13,14 require eight hex digits".into()); }
                dig = Some(u32::from_str_radix(text, 16)?.to_be_bytes());
            }
            value if value.starts_with("--analog-perif-glitch=") => {
                let text = &value["--analog-perif-glitch=".len()..]; if text.len() != 2 { return Err("analog PERIF register10 requires exactly two hex digits".into()); }
                perif = Some(u8::from_str_radix(text, 16)?);
            }
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
    let elf = args.next().ok_or("usage: bringup [--rom] [--usb-host] [--strap=0xNN] [--flash=path] [--rom-spi0-shadow] [--nor-jedec=hhhhhh] [--nor-sfdp=hex-offset:hex-bytes] [--bbpll-hz=480000000] [--rtc-timer-from-cpu] [--watch-pc=0xADDRESS] [--analog-bias=hhhh] [--analog-perif-glitch=hh] [--analog-dig=hhhhhhhh] <RISC-V SRAM/ROM ELF> [UART RX text] [step budget]")?;
    let input = args.next().unwrap_or_default();
    let budget = args.next().map(|s| s.parse::<u64>()).transpose()?.unwrap_or(100_000).min(100_000_000);
    if args.next().is_some() { return Err("too many bringup arguments".into()); }
    let mut m = esp32s31::Bringup::new(); let bytes = std::fs::read(elf)?;
    m.pc_watch = watch_pc.map(|address| esp32s31::PcWatch { address, ..Default::default() });
    m.set_rtc_timer_from_cpu(rtc_timer_from_cpu);
    if rtc_timer_from_cpu { eprintln!("development RTC_TIMER clock from interpreter cycles (one instruction/cycle, nominal RC_SLOW; watchdog timing still explicit)"); }
    if rom { m.load_rom_elf(&bytes)?; } else { m.load_elf(&bytes)?; }
    if let Some(path) = flash {
        let bytes = std::fs::read(path)?; m.bus.cache.set_flash(&bytes)?;
        eprintln!("flash bytes={} (caller input; no inferred NOR identity or OTP image)", bytes.len());
    }
    if let Some(id) = nor {
        m.bus.mspi.attach_flash(id, 0).map_err(|e| format!("invalid NOR descriptor: {e:?}"))?;
        eprintln!("development NOR JEDEC={id:02x?} status=0 (caller input, not identified product hardware)");
    }
    if let Some(hz) = bbpll {
        m.bus.mspi.set_bbpll_input(Some(hz)).map_err(|e| format!("unsupported BBPLL input: {e:?}"))?;
        eprintln!("development BBPLL stable input={hz}Hz (caller input, not a modeled analog PLL lock)");
    }
    if let Some((offset, bytes)) = sfdp {
        m.bus.mspi.attach_sfdp(offset, &bytes).map_err(|e| format!("invalid SFDP interval: {e:?}"))?;
        eprintln!("development SFDP interval offset={offset:#x} bytes={:02x?} (caller input, not identified product parameters)", bytes);
    }
    if let Some(bytes) = bias { m.bus.analog.set_bias(bytes); eprintln!("development analog BIAS seed={bytes:02x?} (caller input, no public reset value or voltage calibration)"); }
    if let Some(byte) = perif { m.bus.analog.set_perif_glitch(byte); eprintln!("development analog PERIF reg10 seed={byte:02x} (caller input, not a measured voltage/glitch profile)"); }
    if let Some(bytes) = dig { m.bus.analog.set_dig_reg(bytes); eprintln!("development analog DIG registers5,7,13,14={bytes:02x?} (caller input, no voltage profile)"); }
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
    if let Some(watch) = &m.pc_watch { eprintln!("pc-watch address={:#010x} retired={} first-cycle={:?}", watch.address, watch.retired, watch.first_cycle); }
    std::io::stdout().write_all(&m.bus.uart0.tx_out)?;
    eprintln!("stop={stop:?} steps={steps} pc={:#010x} mepc={:#010x} mcause={:#x} mtval={:#010x} fault={:?}",
        m.cpu.pc, m.cpu.mepc, m.cpu.mcause, m.cpu.mtval, m.bus.first_fault);
    eprintln!("mspi-commands={} cache-maintenance={} sha-blocks={}", m.bus.mspi.commands, m.bus.cache.maintenance_operations, m.bus.sha.blocks());
    eprintln!("mspi-address={:?} mosi-dlen={:?} miso-dlen={:?}", m.bus.mspi.read(1, 4), m.bus.mspi.read(1, 0x24), m.bus.mspi.read(1, 0x28));
    eprintln!("flash-core-hz={:?}", m.bus.mspi.core_hz());
    eprintln!("mspi-last-command={:?} user={:?} user1={:?} user2={:?} ctrl={:?} status={:?} w0={:?}", m.bus.mspi.last_command, m.bus.mspi.read(1, 0x18), m.bus.mspi.read(1, 0x1c), m.bus.mspi.read(1, 0x20), m.bus.mspi.read(1, 8), m.bus.mspi.read(1, 0x2c), m.bus.mspi.read(1, 0x58));
    eprintln!("analog-transactions={} bias={:?} mpll-shutdown={}", m.bus.analog.transactions, m.bus.analog.bias(), m.bus.power.mpll_disabled());
    eprintln!("analog-dig={:?}", m.bus.analog.dig_reg());
    eprintln!("clock-rates={:?} rtc-fast-hz={} rtc-slow-hz={} cpll-control={:?}", m.bus.clocks.rates(), m.bus.clocks.rtc_fast_hz(), m.bus.clocks.rtc_slow_hz(), m.bus.clocks.read(0x174));
    eprintln!("permission-writes={}", m.bus.permissions.writes);
    eprintln!("rtc-timer-counter={}", m.bus.rtc_timer.counter());
    eprintln!("reset-controls={:?}/{:?} pending={:?}", m.bus.reset.read(0x2070103c), m.bus.reset.read(0x20701040), m.bus.reset.request);
    if m.bus.first_fault.is_some() { eprintln!("fault-registers={:08x?}", m.cpu.x); }
    if m.bus.first_fault.as_ref().is_some_and(|f| (0x20583064..=0x2058306b).contains(&f.address)) {
        for pin in 0..62 {
            if let Ok(mux) = m.bus.pads.read(esp32s31::pads::IO_MUX_BASE + pin * 4) {
                if mux & 512 != 0 { eprintln!("pad-input pin={pin} mux={mux:#x} out-sel={:?} drive={:?}", m.bus.pads.read(esp32s31::pads::GPIO_BASE + 0xaf4 + pin * 4), m.bus.pads.drive(pin as usize)); }
            }
        }
    }
    eprintln!("analog-perif-glitch={:?}", m.bus.analog.perif_glitch());
    if usb_host { eprintln!("usb-host packets={packets} bytes={} data={:?}", usb.len(), String::from_utf8_lossy(&usb)); }
    Ok(())
}
