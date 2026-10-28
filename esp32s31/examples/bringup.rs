//! Run a caller-supplied SRAM/ROM ELF and report the first unimplemented access.
use std::io::Write;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1).peekable(); let mut rom = false; let mut usb_host = false; let mut strap = None; let mut flash = None; let mut spi0_shadow = false; let mut nor = None; let mut bias = None; let mut perif = None; let mut dig = None; let mut bod = None; let mut sfdp = None; let mut bbpll = None; let mut systimer_from_cpu = false; let mut rtc_timer_from_cpu = false; let mut rtc_calibration_from_cpu = false; let mut watch_pc = None; let mut watch_pc1 = None; let mut peripheral_pads = Vec::new(); let mut psram_mode = None; let mut psram_bytes = None; let mut guest_exceptions = 0u32;
    while let Some(arg) = args.peek() {
        match arg.as_str() {
            value if value.starts_with("--guest-exceptions=") => { guest_exceptions = value["--guest-exceptions=".len()..].parse()?; if guest_exceptions > 1024 { return Err("guest-exceptions limit must be at most1024".into()); } }
            "--rom" => rom = true, "--usb-host" => usb_host = true,
            value if value.starts_with("--watch-pc1=0x") => watch_pc1 = Some(u32::from_str_radix(&value[14..], 16)?),
            value if value.starts_with("--watch-pc=0x") => watch_pc = Some(u32::from_str_radix(&value[13..], 16)?),
            value if value.starts_with("--peripheral-pad=") => {
                let (pin, level) = value["--peripheral-pad=".len()..].split_once(':').ok_or("peripheral-pad requires pin:0 or pin:1")?;
                let pin = pin.parse::<usize>()?; let level = match level { "0" => false, "1" => true, _ => return Err("peripheral-pad level must be 0 or 1".into()) };
                if peripheral_pads.iter().any(|&(n, _)| n == pin) { return Err("duplicate peripheral-pad input".into()); }
                peripheral_pads.push((pin, level));
            }
            value if value.starts_with("--psram-mr=") => {
                let text = &value["--psram-mr=".len()..]; if !text.is_ascii() || text.len() != 12 { return Err("PSRAM MR0,1,2,3,4,8 requires twelve hex digits".into()); }
                let mut mode = [0u8; 6]; for (i, byte) in mode.iter_mut().enumerate() { *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16)?; }
                psram_mode = Some(mode);
            }
            value if value.starts_with("--psram-bytes=") => psram_bytes = Some(value["--psram-bytes=".len()..].parse::<usize>()?),
            "--rtc-calibration-from-cpu" => rtc_calibration_from_cpu = true,
            "--systimer-from-cpu" => systimer_from_cpu = true,
            "--rtc-timer-from-cpu" => rtc_timer_from_cpu = true,
            "--rom-spi0-shadow" => spi0_shadow = true,
            value if value.starts_with("--bbpll-hz=") => bbpll = Some(value["--bbpll-hz=".len()..].parse::<u64>()?),
            value if value.starts_with("--nor-sfdp=") => {
                let text = &value["--nor-sfdp=".len()..]; let (offset, hex) = text.split_once(':').ok_or("SFDP needs hex-offset:hex-bytes")?;
                if !hex.is_ascii() || hex.is_empty() || hex.len() > 131072 || !hex.len().is_multiple_of(2) { return Err("invalid SFDP byte interval".into()); }
                let bytes: Result<Vec<_>, _> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16)).collect();
                sfdp = Some((u32::from_str_radix(offset.trim_start_matches("0x"), 16)?, bytes?));
            }
            value if value.starts_with("--analog-bod=") => {
                let text = &value["--analog-bod=".len()..]; if text.len() != 2 { return Err("analog BOD register5 requires two hex digits".into()); }
                bod = Some(u8::from_str_radix(text, 16)?);
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
    let elf = args.next().ok_or("usage: bringup [--rom] [--guest-exceptions=N] [--usb-host] [--strap=0xNN] [--flash=path] [--rom-spi0-shadow] [--nor-jedec=hhhhhh] [--nor-sfdp=hex-offset:hex-bytes] [--bbpll-hz=480000000] [--systimer-from-cpu] [--rtc-timer-from-cpu] [--rtc-calibration-from-cpu] [--watch-pc=0xADDRESS] [--watch-pc1=0xADDRESS] [--peripheral-pad=pin:0|1] [--analog-bias=hhhh] [--analog-perif-glitch=hh] [--analog-dig=hhhhhhhh] [--analog-bod=hh] [--psram-mr=12hex --psram-bytes=N] <RISC-V SRAM/ROM ELF> [UART RX text] [step budget]")?;
    let input = args.next().unwrap_or_default();
    let budget = args.next().map(|s| s.parse::<u64>()).transpose()?.unwrap_or(100_000).min(100_000_000);
    if args.next().is_some() { return Err("too many bringup arguments".into()); }
    let mut m = esp32s31::Bringup::new(); let bytes = std::fs::read(elf)?;
    for (pin, level) in peripheral_pads {
        m.bus.pads.set_unresolved_sample(pin, Some(level)).map_err(|_| "invalid peripheral-pad pin")?;
        eprintln!("development unresolved peripheral pad sample pin={pin} level={}", u8::from(level));
    }
    m.pc_watch1 = watch_pc1.map(|address| esp32s31::PcWatch { address, ..Default::default() });
    m.pc_watch = watch_pc.map(|address| esp32s31::PcWatch { address, ..Default::default() });
    m.set_systimer_from_cpu(systimer_from_cpu);
    if systimer_from_cpu { eprintln!("development SYSTIMER nominal CPU/source clock /2.5; no measured timing or idle-clock scheduler"); }
    m.set_rtc_timer_from_cpu(rtc_timer_from_cpu);
    m.set_rtc_calibration_from_cpu(rtc_calibration_from_cpu);
    if rtc_calibration_from_cpu { eprintln!("development TIMG0 calibration from nominal CPU/XTAL/target clocks,not measured oscillator or instruction timing"); }
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
    match (psram_mode, psram_bytes) {
        (Some(mode), Some(size)) if size > 0 => {
            m.bus.psram_controller.attach_mode(mode).map_err(|e| format!("invalid PSRAM MR configuration: {e:?}"))?;
            m.bus.cache.set_psram_size(size)?;
            eprintln!("development AP-compatible PSRAM MR0,1,2,3,4,8={mode:02x?} bytes={size} (caller input, not identified product die)");
        }
        (None, None) => (),
        _ => return Err("PSRAM requires both explicit MR responses and nonzero backing bytes".into()),
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
    if let Some(byte) = bod { m.bus.analog.set_bod_register(byte); eprintln!("development analog BOD register5 seed={byte:02x} (caller input,no measured threshold or voltage)"); }
    m.bus.usb_console.attach_host(usb_host);
    if spi0_shadow { m.bus.mspi.enable_legacy_rom_lengths(); eprintln!("development SPI0 legacy length shadows enabled (ROM/upstream inference, not verified silicon)"); }
    if let Some(value) = strap { m.bus.pads.strap = value; }
    m.bus.uart_input(input.as_bytes()).map_err(|e| format!("UART input unavailable: {e:?}"))?;
    let mut steps = 0; let mut stop = esp32s31::Stop::Budget; let mut usb = Vec::new(); let mut packets = 0; let mut guest_exceptions_run = 0;
    while steps < budget {
        // Explicit functional host polling, not a USB frame/time guarantee. Without this
        // opt-in, no peer drains packets or fabricates receive data/readiness.
        let run = m.run((budget - steps).min(1000)); steps += run.steps; stop = run.stop;
        if let Some(packet) = m.bus.usb_console.host_receive() { packets += 1; usb.extend(packet); }
        // The architecture already transferred control to mtvec. An explicit
        // budget lets the guest execute its own handler (e.g. lazy FPU enable),
        // never patches PC/registers or skips the trapping instruction. Unknown
        // bus operations still stop at their first fault; default remains zero.
        if matches!(stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(_))) && m.bus.first_fault.is_none() && guest_exceptions_run < guest_exceptions {
            let cpu = if m.last_core == 0 { &m.cpu } else { &m.cpu1 };
            eprintln!("guest-exception-resume hart={} pc={:#010x} mepc={:#010x} cause={:#x} mtval={:#010x} mstatus={:#x}", m.last_core, cpu.pc, cpu.mepc, cpu.mcause, cpu.mtval, cpu.mstatus);
            guest_exceptions_run += 1; continue;
        }
        if stop != esp32s31::Stop::Budget { break; }
    }
    eprintln!("guest-exception-resumes={guest_exceptions_run} limit={guest_exceptions}");
    if let Some(watch) = &m.pc_watch { eprintln!("pc-watch address={:#010x} retired={} first-cycle={:?}", watch.address, watch.retired, watch.first_cycle); }
    std::io::stdout().write_all(&m.bus.uart0.tx_out)?;
    eprintln!("stop={stop:?} steps={steps} pc={:#010x} mepc={:#010x} mcause={:#x} mtval={:#010x} fault={:?}",
        m.cpu.pc, m.cpu.mepc, m.cpu.mcause, m.cpu.mtval, m.bus.first_fault);
    eprintln!("last-hart={} quanta={} core-instructions={:?} core-retired={:?} core1-pc={:#010x} core1-mepc={:#010x} core1-mcause={:#x} core1-mtval={:#010x}", m.last_core, m.quanta, [m.cpu.insn_count, m.cpu1.insn_count], [m.cpu.retired_count, m.cpu1.retired_count], m.cpu1.pc, m.cpu1.mepc, m.cpu1.mcause, m.cpu1.mtval);
    eprintln!("core-controls={:?}/{:?} stall={:?} boot1={:?} last-nonzero-boot1={:x?} resets={:?} pc-watch1={:?}", m.bus.cores.read(0x2058701c), m.bus.cores.read(0x20587020), m.bus.cores.read(0x207041ec), m.bus.cores.read(0x20700164), m.bus.cores.last_nonzero_boot1, m.bus.cores.resets, m.pc_watch1);
    eprintln!("systimer-ticks={} counters={:?} events={:?}", m.bus.systimer.ticks, m.bus.systimer.counters(), m.bus.systimer.events);
    eprintln!("mspi-commands={} cache-maintenance={} sha-blocks={}", m.bus.mspi.commands, m.bus.cache.maintenance_operations, m.bus.sha.blocks());
    eprintln!("mspi-address={:?} mosi-dlen={:?} miso-dlen={:?}", m.bus.mspi.read(1, 4), m.bus.mspi.read(1, 0x24), m.bus.mspi.read(1, 0x28));
    eprintln!("core-fpu mstatus={:#x}/{:#x} ext-ill={:#x}/{:#x} mwdt-clocks={:#x}/{:#x}",m.cpu.mstatus,m.cpu1.mstatus,m.cpu.ext_ill,m.cpu1.ext_ill,m.bus.mwdt[0].clock_read(),m.bus.mwdt[1].clock_read());
    eprintln!("mspi-waiti-polls={} sample={:?} control={:?} suspend={:?}", m.bus.mspi.waiti_polls, m.bus.mspi.last_waiti_status, m.bus.mspi.read(1, 0x98), m.bus.mspi.read(1, 0x9c));
    eprintln!("flash-core-hz={:?}", m.bus.mspi.core_hz());
    eprintln!("mspi-last-command={:?} user={:?} user1={:?} user2={:?} ctrl={:?} status={:?} w0={:?}", m.bus.mspi.last_command, m.bus.mspi.read(1, 0x18), m.bus.mspi.read(1, 0x1c), m.bus.mspi.read(1, 0x20), m.bus.mspi.read(1, 8), m.bus.mspi.read(1, 0x2c), m.bus.mspi.read(1, 0x58));
    eprintln!("analog-transactions={} bias={:?} mpll-shutdown={}", m.bus.analog.transactions, m.bus.analog.bias(), m.bus.power.mpll_disabled());
    eprintln!("analog-dig={:?}", m.bus.analog.dig_reg());
    eprintln!("clock-rates={:?} rtc-fast-hz={} rtc-slow-hz={} cpll-control={:?}", m.bus.clocks.rates(), m.bus.clocks.rtc_fast_hz(), m.bus.clocks.rtc_slow_hz(), m.bus.clocks.read(0x174));
    eprintln!("mpll-divider={:#x} mpll-hz={:?} psram-core-hz={:?}", m.bus.clocks.mpll.divider(), m.bus.clocks.mpll.output_hz(), m.bus.clocks.psram_core_hz());
    eprintln!("psram-command-counts-mr-read-write-array-read-write={:?}", m.bus.psram_controller.command_counts);
    eprintln!("psram-commands={} mode={:02x?} ddr={:?} mosi={:?} miso={:?}", m.bus.psram_controller.commands, m.bus.psram_controller.mode(), m.bus.psram_controller.read(1, 0xd4), m.bus.psram_controller.read(1, 0x24), m.bus.psram_controller.read(1, 0x28));
    eprintln!("psram-last-command={:?} ctrl={:?} user={:?} user1={:?} user2={:?} address={:?} misc={:?}", m.bus.psram_controller.last_command, m.bus.psram_controller.read(1, 8), m.bus.psram_controller.read(1, 0x18), m.bus.psram_controller.read(1, 0x1c), m.bus.psram_controller.read(1, 0x20), m.bus.psram_controller.read(1, 4), m.bus.psram_controller.read(1, 0x34));
    eprintln!("mspi-pads-held={} dqs-requested={} pin-d={:?} pin-dqs={:?}", m.bus.mspi_pads.held(), m.bus.mspi_pads.dqs_requested(), m.bus.mspi_pads.read(0x2058401c), m.bus.mspi_pads.read(0x2058403c));
    eprintln!("pmu-updates={} dbias-requests={} readonly-write-attempts={}", m.bus.power.policy.updates, m.bus.power.policy.dbias_requests, m.bus.power.policy.readonly_write_attempts);
    eprintln!("permission-writes={}", m.bus.permissions.writes);
    eprintln!("apm-writes={} function-controls={:x?} last-denial={:?}", m.bus.apm.writes, m.bus.apm.function_controls(), m.bus.apm.last_denial);
    eprintln!("rtc-calibration measurements={} clock={:?} config={:?} result={:?} timeout={:?}", m.bus.rtc_calibration.measurements, m.bus.rtc_calibration.read(0x20587118, 0), m.bus.rtc_calibration.read(0x20580068, m.bus.mwdt[0].clock_read()), m.bus.rtc_calibration.read(0x2058006c, m.bus.mwdt[0].clock_read()), m.bus.rtc_calibration.read(0x20580080, m.bus.mwdt[0].clock_read()));
    eprintln!("rtc-timer-counter={}", m.bus.rtc_timer.counter());
    eprintln!("reset-controls={:?}/{:?} pending={:?}", m.bus.reset.read(0x2070103c), m.bus.reset.read(0x20701040), m.bus.reset.request);
    if m.bus.first_fault.is_some() { eprintln!("fault-registers={:08x?}", if m.last_core == 0 { &m.cpu.x } else { &m.cpu1.x }); }
    if m.bus.first_fault.as_ref().is_some_and(|f| (0x20583064..=0x2058306b).contains(&f.address)) {
        for pin in 0..62 {
            if let Ok(mux) = m.bus.pads.read(esp32s31::pads::IO_MUX_BASE + pin * 4) {
                if mux & 512 != 0 { eprintln!("pad-input pin={pin} mux={mux:#x} out-sel={:?} drive={:?}", m.bus.pads.read(esp32s31::pads::GPIO_BASE + 0xaf4 + pin * 4), m.bus.pads.drive(pin as usize)); }
            }
        }
    }
    eprintln!("analog-bod={:?} mode0={:?} irq={}",m.bus.analog.bod_register(),m.bus.power.mode0.read(0x20702000),m.bus.power.mode0.irq());
    eprintln!("analog-perif-glitch={:?}", m.bus.analog.perif_glitch());
    if usb_host { eprintln!("usb-host packets={packets} bytes={} data={:?}", usb.len(), String::from_utf8_lossy(&usb)); }
    Ok(())
}
