//! esp32sim — the command line, one front end for every chip (`--chip esp32|s3|c3|c6|p4`; the `esp32sim-c3`,
//! `esp32sim-c6` and `esp32sim-p4` binaries are `--chip c3` / `--chip c6` / `--chip p4`). Parsing and everything a run does are chip-agnostic over
//! `Machine<S>`; the few flags a chip owns (board, WiFi, camera, PSRAM, register presets) live in
//! its setup function.
use esp_soc::observers::{BlockProfile, Breakpoints, Coverage, IrqLatency, MmioHeat, PcHist, RegTrace, Trace, Vcd, Watch};
use esp_soc::{Machine, Soc, SocBus, Stop};
use emu_core::{Bus, Core};
use std::path::PathBuf;

pub mod cooja;
/// The P4 and its ESP32-C6 co-processor in lock-step (`esp32p4::coproc`), as the CLI's `--coproc` runs them.
pub use esp32p4::coproc as cosim;
mod camera;

fn usage(chip: &str) -> ! { help(chip, 2) }

/// `--help` prints this and exits 0; a usage error prints it and exits 2.
fn help(chip: &str, code: i32) -> ! {
    eprintln!("usage: esp32sim [--chip esp32|s3|c3|c6|p4] --boot rom|app --bootloader B.bin --ptable P.bin --app A.bin [--elf X.elf]... [options]");
    eprintln!("       esp32sim --flash-image flash.bin --boot rom [options]            (a whole flash dump instead of the three images)");
    eprintln!("       default chip here: {}; esp32sim-c3, esp32sim-c6 and esp32sim-p4 are --chip c3 / --chip c6 / --chip p4", chip);
    eprintln!();
    eprintln!("  --rom F               the chip's mask ROM ELF; default: $ESP32SIM_ROM_DIR, then ~/.espressif/tools/esp-rom-elfs/*/");
    eprintln!("                        (Espressif's, not in the tarball: tools/fetch-rom-elfs.sh DIR downloads all five)");
    eprintln!("  --board NAME          board model (S3: atech14, waveshare-lcd4b, waveshare-amoled18-v2, waveshare-cam, none;");
    eprintln!("                        C6: waveshare-c6-lcd147, none; P4: waveshare-p4-lcd35, none; classic: ttgo-tdisplay, ulanzi-tc001, esp32dev, none)");
    eprintln!("  --flash-mb N, --psram-mb N   memory sizes (S3 default 8 / 2, C3, C6 and classic 4 / 0, P4 16 / 0)");
    eprintln!("  --max-seconds S       stop after S emulated seconds (always give one: the run returns)");
    eprintln!("  --console usb|uart0|both|none   which consoles to print (uart0 shows the ROM and bootloader)");
    eprintln!("  --wifi ssid=…,psk=…   a virtual access point with DHCP, DNS, NTP and NAT to the host network");
    eprintln!("  --hostfwd tcp:H:HP-G:GP  publish a guest TCP port on the host (repeatable; needs --wifi)");
    eprintln!("  --script F            host actions at emulated times (button presses, touches, serial input, stop)");
    eprintln!("  --tft-png F, --wav F  save the display / the audio at the end of the run");
    eprintln!("  --web PORT            serve the board UI at http://127.0.0.1:PORT/ in real time");
    eprintln!("  --elf F               symbols for logs, traces and profiles (repeatable)");
    eprintln!("  --trace-fn PREFIX, --break PC, --watch ADDR, --peek ADDR,N, --profile, --vcd F, --debug AREAS, --no-jit");
    eprintln!("                        debugging; --no-jit is the oracle when a result looks wrong");
    eprintln!("  --adc PIN, --adc-raw PIN=N   last ADC sample at exit / a 12-bit count into that pad");
    eprintln!("  --sd IMG, --sd-rw IMG (P4) a card in the SD socket, copy-on-write / write-through;");
    eprintln!("                        --sd-prg-us N, --sd-serial HEX for its programming time and serial");
    eprintln!("  --usb-host F          (P4) a USB host on the USB-OTG port running F; --sd-timing for timed SD");
    eprintln!("  --coproc c6:KEY=V,... (P4) an ESP32-C6 co-processor on SDIO, run in lock-step");
    eprintln!("  --no-dump             skip the register dump at exit");
    eprintln!();
    eprintln!("Every flag: docs/cli.md. First run: docs/quick-start.md. Both are in the tarball and at github.com/mikroverk/esp32sim.");
    std::process::exit(code)
}

fn usage_error(message: &str) -> ! { eprintln!("{message}"); std::process::exit(2) }

fn apply_adc_raw<S: Soc>(m: &mut Machine<S>, specs: &[(u8, u16)]) {
    for &(pin, raw) in specs {
        if !m.bus.adc_set_raw(pin, raw) { usage_error(&format!("--adc-raw: GPIO{pin} is not an ADC pad on this chip")); }
    }
}

fn timing_cycles(value: &str, name: &str) -> Result<u32, String> {
    value.parse().map_err(|_| format!("{name}: expected a nonnegative u32 cycle count, got {value:?}"))
}

fn validate_timing(o: &Opts) -> Result<(), String> {
    if o.approximate_timing && !matches!(o.chip.as_str(), "s3" | "esp32s3") {
        return Err("--approximate-timing, --approximate-memory and --approximate-cache require --chip s3".into());
    }
    if o.memory_contention && o.approximate_memory.is_none() {
        return Err("--memory-contention requires --approximate-memory".into());
    }
    if o.approximate_memory.is_some() && o.boot.as_deref() != Some("rom") {
        return Err("--approximate-memory requires --boot rom for its MMU shadow".into());
    }
    Ok(())
}

fn cache_config() -> Result<esp32s3::approximate_cache::CacheConfig, String> {
    let mut cache = esp32s3::approximate_cache::CacheConfig::default();
    for (name, cycles) in [("ESP32SIM_CACHE_FILL", &mut cache.fill_cycles), ("ESP32SIM_CACHE_WRITEBACK", &mut cache.writeback_cycles)] {
        match std::env::var(name) {
            Ok(value) => *cycles = timing_cycles(&value, name)?,
            Err(std::env::VarError::NotPresent) => {},
            Err(_) => return Err(format!("{name}: expected a UTF-8 cycle count")),
        }
    }
    Ok(cache)
}

fn hex(s: &str, what: &str) -> u32 { u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or_else(|_| { eprintln!("--{}: bad hex {}", what, s); std::process::exit(2) }) }
fn pair(s: &str, dflt: usize) -> (u32, usize) { match s.split_once(',') { Some((a, n)) => (hex(a, "addr"), n.parse().unwrap_or(dflt)), None => (hex(s, "addr"), dflt) } }

use esp_soc::load::stub_spec;

fn console_mask(name: &str) -> u32 {
    esp_soc::Console::parse_mask(name).unwrap_or_else(|| {
        eprintln!("--console: unknown source {name:?}; expected usb, uart0, both, all or none");
        std::process::exit(2)
    })
}

/// Everything the command line can say, chip-agnostic; `None` means "the chip's default".
#[derive(Default)]
pub struct Opts {
    pub approximate_timing: bool,
    pub approximate_memory: Option<u32>,
    pub memory_contention: bool,
    pub approximate_cache: bool,
    pub chip: String,
    pub rom: Option<PathBuf>, pub bootloader: Option<String>, pub ptable: Option<String>, pub app: Option<String>, pub elfs: Vec<String>,
    pub flash_image: Option<String>, pub flash_at: Vec<String>, pub boot: Option<String>, pub flash_mb: Option<usize>, pub psram_mb: Option<usize>,
    /// `--flash-octal`: an octal SPI flash (ESP32-S3-WROOM-2)
    pub flash_octal: bool,
    pub mac: Option<[u8; 6]>, pub strap: Option<u32>, pub reset_cause: Option<u32>, pub efuse_regs: Option<String>, pub regs_init: Option<String>,
    /// `--sd IMAGE` (copy-on-write) or `--sd-rw IMAGE` (write-through): a card in the SD socket,
    /// with `--sd-prg-us` and `--sd-serial`
    pub sd: Option<(String, bool)>, pub sd_prg_us: Option<u64>, pub sd_serial: Option<u32>, pub sd_timing: bool,
    /// `--usb-host FILE`: a USB host on the chip's USB-OTG port, running the commands in FILE
    pub usb_host: Option<String>,
    pub board: String, pub wifi: Option<String>, pub wifi_nearby: Vec<String>, pub ble_nearby: Vec<String>, pub ble: bool, pub ble_full: bool, pub ble_observe: bool, pub ble_scan: bool, pub net: String, pub hostfwd: Vec<String>, pub cam_image: Option<String>, pub cam_stream: Option<String>, pub cam_size: Option<String>, pub cam_fps: f64,
    pub spi2_timing: bool, pub measured_te: bool,
    pub max_insns: u64, pub max_seconds: Option<f64>, pub script: Option<String>, pub serial: Option<String>,
    pub console: Option<String>, pub console_prefix: bool, pub realtime: bool, pub web_port: Option<u16>, pub web_dir: Option<String>, pub no_reboot: bool,
    pub i2s_tone: Vec<String>,
    pub wav: Option<String>, pub tft_png: Option<String>, pub gram_png: Option<String>, pub dump: bool,
    pub trace: bool, pub trace_from: u64, pub breaks: Vec<u32>, pub watch: Option<u32>, pub peeks: Vec<(u32, usize)>, pub pwm_pins: Vec<u8>, pub adc_pins: Vec<u8>, pub adc_raw: Vec<(u8, u16)>, pub disasms: Vec<(u32, usize)>,
    pub profile: bool, pub profile_blocks: bool, pub coverage: Option<Option<String>>, pub irq_latency: bool, pub vcd: Option<String>,
    pub regstat: Option<String>, pub regtrace: Option<String>, pub regtrace_max: u64, pub regtrace_from_pc: Option<u32>,
    pub stubs: Vec<String>, pub trace_fns: Vec<String>, pub stop_exc: u64, pub log_periph: bool, pub no_jit: bool, pub debug: Vec<String>,
    /// `--cooja`: run as a Cooja-NG external mote over NDJSON on stdin/stdout (ESP32-C6 only)
    pub cooja: bool, pub cooja_slice_us: u64, pub cooja_verbose: bool, pub cooja_rx_at_end: bool,
    /// `--coproc c6:KEY=VALUE,...`: an ESP32-C6 co-processor on the P4's SDIO bus (`cosim`)
    pub coproc: Option<String>,
}

pub fn parse(args: &[String], default_chip: &str) -> Opts {
    let mut o = Opts { chip: default_chip.to_string(), board: "atech14".into(), net: "nat".into(), cam_fps: 10.0, max_insns: u64::MAX, dump: true, regtrace_max: u64::MAX, stop_exc: u64::MAX, cooja_slice_us: 100, ..Default::default() };
    let mut i = 1;
    while i < args.len() {
        let a = args[i].as_str();
        let mut next = || { i += 1; args.get(i).cloned().unwrap_or_else(|| usage(default_chip)) };
        match a {
            "--approximate-timing" => o.approximate_timing = true,
            "--approximate-memory" => { o.approximate_timing = true; o.approximate_memory = Some(timing_cycles(&next(), "--approximate-memory").unwrap_or_else(|e| usage_error(&e))); }
            "--memory-contention" => o.memory_contention = true,
            "--approximate-cache" => { o.approximate_timing = true; o.approximate_cache = true; },
            "--chip" => o.chip = next().to_ascii_lowercase(),
            "--rom" => o.rom = Some(PathBuf::from(next())),
            "--bootloader" => o.bootloader = Some(next()),
            "--ptable" => o.ptable = Some(next()),
            "--app" => o.app = Some(next()),
            "--elf" => o.elfs.push(next()),
            "--flash-image" => o.flash_image = Some(next()),
            "--flash-at" => o.flash_at.push(next()),
            "--boot" => o.boot = Some(next()),
            "--flash-mb" => o.flash_mb = Some(next().parse().expect("mb")),
            "--psram-mb" => o.psram_mb = Some(next().parse().expect("mb")),
            "--flash-octal" => o.flash_octal = true,
            "--sd" => o.sd = Some((next(), false)),
            "--sd-rw" => o.sd = Some((next(), true)),
            "--sd-prg-us" => o.sd_prg_us = Some(next().parse().unwrap_or_else(|_| usage_error("--sd-prg-us: expected microseconds"))),
            "--sd-serial" => o.sd_serial = Some(hex(&next(), "sd-serial")),
            "--usb-host" => o.usb_host = Some(next()),
            "--sd-timing" => o.sd_timing = true,
            "--mac" => { let v = next(); let b: Vec<u8> = v.split(':').filter_map(|x| u8::from_str_radix(x, 16).ok()).collect(); if b.len() != 6 { eprintln!("--mac wants xx:xx:xx:xx:xx:xx"); std::process::exit(2); } let mut m = [0u8; 6]; m.copy_from_slice(&b); o.mac = Some(m); }
            "--strap" => o.strap = Some(hex(&next(), "strap")),
            "--reset-cause" => o.reset_cause = Some(hex(&next(), "reset-cause")),
            "--efuse-regs" => o.efuse_regs = Some(next()),
            "--regs-init" => o.regs_init = Some(next()),
            "--board" => o.board = next(),
            "--spi2-timing" => o.spi2_timing = true,
            "--measured-te" => o.measured_te = true,
            "--wifi" => o.wifi = Some(next()),
            "--hostfwd" => o.hostfwd.push(next()),
            "--wifi-nearby" => o.wifi_nearby.push(next()),
            "--ble-nearby" => o.ble_nearby.push(next()),
            "--ble-scan" => o.ble_scan = true,
            "--ble-observe" => o.ble_observe = true,
            "--ble" => {
                let full = args.get(i + 1).is_some_and(|s| s == "full");
                if full { i += 1; o.ble_full = true; } else { o.ble = true; }
            },
            "--net" => o.net = next(),
            "--cam-image" => o.cam_image = Some(next()),
            "--cam-stream" => o.cam_stream = Some(next()),
            "--cam-size" => o.cam_size = Some(next()),
            "--cam-fps" => o.cam_fps = next().parse().expect("fps"),
            "--max-insns" => o.max_insns = next().replace('_', "").parse().expect("max-insns"),
            "--max-seconds" => o.max_seconds = Some(next().parse().expect("seconds")),
            "--script" => o.script = Some(next()),
            "--serial" => o.serial = Some(next()),
            "--console" => o.console = Some(next()),
            "--console-prefix" => o.console_prefix = true,
            "--realtime" => o.realtime = true,
            "--web" => o.web_port = Some(next().parse().expect("port")),
            "--web-dir" => o.web_dir = Some(next()),
            "--no-reboot" => o.no_reboot = true,
            "--i2s-tone" => o.i2s_tone.push(next()),
            "--wav" => o.wav = Some(next()),
            "--tft-png" => o.tft_png = Some(next()),
            "--gram-png" => o.gram_png = Some(next()),
            "--no-dump" => o.dump = false,
            "--trace" => o.trace = true,
            "--trace-from" => { o.trace = true; o.trace_from = next().replace('_', "").parse().expect("trace-from") }
            "--break" => o.breaks.push(hex(&next(), "break")),
            "--watch" => o.watch = Some(hex(&next(), "watch")),
            "--peek" => o.peeks.push(pair(&next(), 8)),
            "--pwm" => o.pwm_pins.push(next().parse().unwrap_or_else(|_| usage_error("--pwm: expected a pin number in 0..255"))),
            "--adc" => o.adc_pins.push(next().parse().unwrap_or_else(|_| usage_error("--adc: expected a pin number in 0..255"))),
            "--adc-raw" => {
                let spec = next();
                let Some((pin, raw)) = spec.split_once('=') else { usage_error("--adc-raw: expected PIN=COUNT") };
                let pin: u8 = pin.parse().unwrap_or_else(|_| usage_error("--adc-raw: expected a pin number in 0..255"));
                let raw: u16 = raw.parse().unwrap_or_else(|_| usage_error("--adc-raw: expected a count in 0..4095"));
                if raw > 4095 { usage_error("--adc-raw: expected a count in 0..4095"); }
                o.adc_raw.push((pin, raw));
            }
            "--disasm" => o.disasms.push(pair(&next(), 16)),
            "--profile" => o.profile = true,
            "--profile-blocks" => o.profile_blocks = true,
            "--coverage" => o.coverage = Some(None),
            "--coverage-file" => o.coverage = Some(Some(next())),
            "--irq-latency" => o.irq_latency = true,
            "--vcd" => o.vcd = Some(next()),
            "--regstat" => o.regstat = Some(next()),
            "--regtrace" => o.regtrace = Some(next()),
            "--regtrace-max" => o.regtrace_max = next().parse().expect("n"),
            "--regtrace-from-pc" => o.regtrace_from_pc = Some(hex(&next(), "regtrace-from-pc")),
            "--stub" => o.stubs.push(next()),
            "--trace-fn" => o.trace_fns.push(next()),
            "--stop-after-exceptions" => o.stop_exc = next().parse().expect("count"),
            "--log-periph" => o.log_periph = true,
            "--no-jit" => o.no_jit = true,
            "--debug" => o.debug.push(next()),
            "--cooja" => o.cooja = true,
            "--coproc" => o.coproc = Some(next()),
            "--cooja-slice-us" => o.cooja_slice_us = next().parse().expect("slice-us"),
            "--cooja-verbose" => o.cooja_verbose = true,
            "--cooja-rx-timing" => o.cooja_rx_at_end = match next().as_str() { "start" => false, "end" => true, x => { eprintln!("--cooja-rx-timing {}: start or end", x); std::process::exit(2) } },
            "-h" | "--help" => help(default_chip, 0),
            "-V" | "--version" => { println!("esp32sim {}", env!("CARGO_PKG_VERSION")); std::process::exit(0) }
            _ => { eprintln!("unknown arg {}", a); usage(default_chip) }
        }
        i += 1;
    }
    if !o.hostfwd.is_empty() && o.wifi.is_none() { usage_error("--hostfwd requires --wifi"); }
    if !o.hostfwd.is_empty() && !matches!(o.net.split(',').next(), Some("nat" | "user")) { usage_error("--hostfwd requires --net nat"); }
    for spec in &o.hostfwd {
        if let Err(e) = esp_soc::nat::HostFwdSpec::parse(spec) { usage_error(&format!("--hostfwd: {e}")); }
    }
    if (o.ble_observe || o.ble_scan) && !o.ble_full { usage_error("BLE central/observer use requires --ble full"); }
    if o.ble_full && o.ble { usage_error("--ble full and --ble are mutually exclusive"); }
    if !o.ble_nearby.is_empty() {
        if !o.ble { usage_error("--ble-nearby needs the virtual controller of --ble"); }
        if let Err(e) = esp_soc::ble::peer::Nearby::parse_all(&o.ble_nearby) { usage_error(&format!("--ble-nearby: {e}")); }
    }
    if o.ble_full && !matches!(o.chip.as_str(), "c3" | "esp32c3") { usage_error("--ble full requires C3"); }
    if let Some(path) = &o.script {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| usage_error(&format!("script: {e}")));
        for (ln, line) in text.lines().enumerate() {
            let mut words = line.split_whitespace();
            if words.next().is_none_or(|s| s.starts_with('#')) || words.next() != Some("ble") { continue }
            let command = words.collect::<Vec<_>>().join(" ").parse::<esp_soc::ble::peer::Command>()
                .unwrap_or_else(|e| usage_error(&format!("script: line {}: {e}", ln + 1)));
            if o.ble_full && matches!(command, esp_soc::ble::peer::Command::Discover | esp_soc::ble::peer::Command::Read(_) | esp_soc::ble::peer::Command::Write(..)) { usage_error(&format!("script: line {}: command requires HCI --ble", ln + 1)); }
            if !o.ble && !o.ble_full { usage_error(&format!("script: line {}: BLE requires --ble and the application ELF", ln + 1)); }
        }
    }
    o
}

/// ESP32SIM_ROM_DIR, then `~/.espressif/tools/esp-rom-elfs/*/<name>` (newest first); the tests use the same lookup.
fn find_rom(name: &str) -> Option<PathBuf> { esp_soc::rom::find(name) }

pub fn run_cli(default_chip: &str) {
    let args: Vec<String> = std::env::args().collect();
    let mut o = parse(&args, default_chip);
    if o.cam_stream.is_some() != o.cam_size.is_some() { usage_error("--cam-stream and --cam-size must be supplied together"); }
    validate_timing(&o).unwrap_or_else(|e| usage_error(&e));
    if o.approximate_cache { cache_config().unwrap_or_else(|e| usage_error(&e)); }
    if o.cooja { return run_cooja(&mut o); }
    if o.coproc.is_some() { return run_cosim(&o); }
    match o.chip.as_str() {
        "esp32" => { let m = setup_esp32(&o); run(m, &o) }
        "s3" | "esp32s3" => { let m = setup_s3(&o); run(m, &o) }
        "c3" | "esp32c3" => { let m = setup_c3(&o); run(m, &o) }
        "c6" | "esp32c6" => { let m = setup_c6(&o); run(m, &o) }
        "p4" | "esp32p4" if p4_rev3(&o) => { let m = setup_p4::<esp32p4::RevGe3>(&o); run(m, &o) }
        "p4" | "esp32p4" => { let m = setup_p4::<esp32p4::RevLt3>(&o); run(m, &o) }
        c => { eprintln!("--chip {}: esp32, s3, c3, c6 or p4", c); std::process::exit(2) }
    }
}

/// `--cooja`: the C6 as a Cooja-NG external mote. csim's `hello` comes first — its node id names
/// the MAC unless `--mac` did — then the machine is set up and booted exactly as for a normal
/// run, and the NDJSON exchange takes the place of the run loop. The guest console never
/// reaches stdout: it goes to csim as `log` events. The usual report goes to stderr at the end.
fn run_cooja(o: &mut Opts) {
    if !matches!(o.chip.as_str(), "c6" | "esp32c6") { eprintln!("--cooja: only the ESP32-C6 speaks the Cooja-NG lock-step protocol"); std::process::exit(2); }
    if o.max_insns != u64::MAX { eprintln!("--cooja: --max-insns is unsupported; use --max-seconds"); std::process::exit(2); }
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let hello = match cooja::read_hello(&mut input) { Ok(h) => h, Err(e) => { eprintln!("[cooja] {}", e); std::process::exit(2) } };
    if o.mac.is_none() { o.mac = Some(cooja::mac_for_node(hello.id)); }
    if o.console.is_none() { o.console = Some("uart0".into()); }
    let mut m = setup_c6(o);
    let boot = prepare(&mut m, o);
    let cfg = cooja::Config {
        slice_ns: o.cooja_slice_us.max(1).saturating_mul(1000),
        console_mask: console_mask(o.console.as_deref().unwrap_or("uart0")),
        rx_on_air: !o.cooja_rx_at_end,
        verbose: o.cooja_verbose,
        reboot: !o.no_reboot && boot == "rom",
    };
    eprintln!("[cooja] node {} ({}), mac {}, slice {} µs", hello.id, boot, o.mac.map(|m| m.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(":")).unwrap_or_default(), cfg.slice_ns / 1000);
    let t0 = std::time::Instant::now();
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    let summary = match cooja::run(&mut m, cfg, &hello, &mut input, &mut output) { Ok(s) => s, Err(e) => { eprintln!("[cooja] {}", e); std::process::exit(1) } };
    let dt = t0.elapsed().as_secs_f64();
    eprintln!("[cooja] {} steps, {} early yields, {} tx, {} rx ({} dropped), {} log lines; {:.3} s simulated in {:.1} s wall ({} cycles, {:.1} Mcycle/s)",
              summary.steps, summary.yields, summary.tx, summary.rx, summary.rx_dropped, summary.logs, summary.sim_ns as f64 / 1e9, dt, m.bus.cycles(), m.bus.cycles() as f64 / dt / 1e6);
    report(&mut m, o, match summary.stopped { Some(_) => Stop::Halted, None => Stop::MaxInsns }, dt);
}

fn set_camera_image(board: &mut esp_soc::Board, path: Option<&str>) {
    if let Some(p) = path {
        // Baseline JPEG goes to the sensor verbatim; it is never decoded.
        if let Some(data) = std::fs::read(p).ok().filter(|d| d.starts_with(&[0xff, 0xd8])) {
            let jpeg = esp_soc::picture::Jpeg::parse(data).and_then(|jpeg| {
                let (w, h, n) = (jpeg.w, jpeg.h, jpeg.bytes.len());
                board.set_camera_jpeg(jpeg).map(|()| eprintln!("[emu] camera JPEG {p} ({w}x{h}, {n} bytes)"))
            });
            if let Err(e) = jpeg { eprintln!("[emu] {p}: {e}"); std::process::exit(2); }
            return;
        }
        match esp_soc::picture::load(p) {
            Ok(pic) => {
                eprintln!("[emu] camera picture {} ({}x{})", p, pic.w, pic.h);
                board.set_camera_picture(pic).unwrap_or_else(|e| usage_error(e));
            }
            Err(e) => { eprintln!("[emu] {}", e); std::process::exit(2); }
        }
    }
}

/// `--coproc`: the P4 with an ESP32-C6 on its SDIO bus, both run in lock-step (`cosim`). The P4
/// is set up and booted as for a normal run; `--wifi`, `--net` and `--debug` also go to the C6.
/// Both consoles go to stdout line by line, the C6's prefixed `[c6] ` (or to `console=FILE`).
fn run_cosim(o: &Opts) {
    if !matches!(o.chip.as_str(), "p4" | "esp32p4") { usage_error("--coproc: the co-processor joins an ESP32-P4 (--chip p4)"); }
    let spec = cosim::CoprocSpec::parse(o.coproc.as_deref().unwrap()).unwrap_or_else(|e| usage_error(&format!("--coproc: {e}")));
    if p4_rev3(o) { run_cosim_on::<esp32p4::RevGe3>(o, &spec) } else { run_cosim_on::<esp32p4::RevLt3>(o, &spec) }
}

/// `run_cosim` with the P4 revision the images were built for.
fn run_cosim_on<R: esp32p4::Revision>(o: &Opts, spec: &cosim::CoprocSpec) {
    use std::io::Write;
    let mut p4 = setup_p4::<R>(o);
    let boot = prepare(&mut p4, o);
    // the SDIO link's commands and data take their wire time, as `--sd-timing` gives the card
    if o.sd_timing { p4.bus.periph.sdmmc.timed = true; }
    let end_ns = if p4.max_cycles == u64::MAX { u64::MAX } else { (p4.max_cycles as u128 * 1_000_000_000 / esp32p4::periph::CPU_HZ as u128) as u64 };
    p4.max_cycles = u64::MAX;
    let c6 = setup_coproc(o, spec);
    let cfg = cosim::Config {
        slice_ns: spec.slice_ns.unwrap_or(cosim::DEFAULT_SLICE_NS), en_pin: spec.en_pin.unwrap_or(cosim::DEFAULT_EN_PIN), en_inverted: spec.en_inverted,
        reboot_p4: !o.no_reboot && boot == "rom", reboot_c6: !o.no_reboot, p4_console: p4.console.mask, c6_console: 2,
    };
    eprintln!("[cosim] ESP32-C6 on SDIO slot 1 (CLK 18, CMD 19, D0-D3 14-17), EN on GPIO {}{}, slices of {} µs",
              if cfg.en_inverted { "!" } else { "" }, cfg.en_pin, cfg.slice_ns as f64 / 1000.0);
    let mut c6_file = spec.console.as_ref().map(|p| std::fs::File::create(p).unwrap_or_else(|e| usage_error(&format!("--coproc console={p}: {e}"))));
    let prefix = o.console_prefix;
    let mut cs = cosim::CoSim::new(p4, c6, cfg);
    cs.c6.borrow_mut().faults.plan = spec.faults.clone();
    let t0 = std::time::Instant::now();
    let stdout = std::io::stdout();
    let mut stop = Stop::Halted;
    while cs.now_ns < end_ns {
        let step = (cs.now_ns + 1_000_000).min(end_ns);
        let r = cs.run_until(step);
        let mut out = stdout.lock();
        for (from_c6, line) in cs.lines.drain(..) {
            match (&mut c6_file, from_c6) {
                (Some(f), true) => { let _ = writeln!(f, "{line}"); }
                (None, true) => { let _ = writeln!(out, "[c6] {line}"); }
                (_, false) => { let _ = writeln!(out, "{}{line}", if prefix { "[p4] " } else { "" }); }
            }
        }
        let _ = out.flush();
        if let Err(s) = r { stop = s; break; }
    }
    let dt = t0.elapsed().as_secs_f64();
    report(&mut cs.p4, o, stop, dt);
    let c = cs.c6.borrow();
    let st = &c.m.bus.periph.optional.sdio.stats;
    eprintln!("[c6] stop: {} — {} insns; ran {:.3}s of {:.3}s ({} cycles); {} exceptions, {} interrupts; {} power-ups by EN, {} chip resets",
              c.halted.as_deref().unwrap_or(if c.held { "held in reset" } else { "running" }), c.m.insns(), c.m.seconds(), cs.now_ns as f64 / 1e9, c.m.bus.cycles(),
              c.m.exceptions, c.m.interrupts, c.power_ups, c.resets);
    let stale = if st.stale_new_packets > 0 { format!(", {} packets announced before their length", st.stale_new_packets) } else { String::new() };
    eprintln!("[c6] sdio: {} commands, {} bytes to the host, {} from it, {} underflows, {} overflows{stale}", st.commands, st.to_host, st.from_host, st.underflows, st.overflows);
    { let r = c.m.bus.report(); if !r.is_empty() { for l in r.lines() { eprintln!("[c6] {l}"); } } }
    for (f, ns) in &c.faults.fired { eprintln!("[cosim] fault {} fired at {:.6}s", fault_label(f), *ns as f64 / 1e9); }
    let late = if c.late_commands > 0 { format!(", {} commands late for the C6", c.late_commands) } else { String::new() };
    let late = if c.en_glitches > 0 { format!("{late}, {} EN pulses too short to reset the C6", c.en_glitches) } else { late };
    eprintln!("[cosim] {} slices, {} EN changes, {} P4 resets{late}{}", cs.summary.slices, cs.summary.en_changes, cs.summary.p4_resets, cs.summary.stopped.as_ref().map(|s| format!("; stopped: {s}")).unwrap_or_default());
}

/// The co-processor: a bare ESP32-C6 with the images of `--coproc`, booted from its ROM.
fn setup_coproc(o: &Opts, spec: &cosim::CoprocSpec) -> esp32c6::Machine {
    let flash = spec.flash_mb.unwrap_or(4) << 20;
    // 40:4C:CA is an Espressif OUI; the rest is made up, so the C6 reports a MAC of its own
    let mut m = esp32c6::machine(spec.mac.unwrap_or([0x40, 0x4c, 0xca, 0x00, 0x11, 0x22]), flash);
    m.bus.set_flash_size(flash);
    if !o.debug.is_empty() { let mut f = esp_soc::DebugFlags::from_env(); for d in &o.debug { f.parse(d); } m.set_debug(&f); }
    if let Some(spec) = &o.wifi {
        let (cfg, nat) = wifi_config(spec, &o.net, m.bus.debug.has("net"), &o.hostfwd);
        m.bus.attach_wifi(cfg, nat).unwrap_or_else(|e| usage_error(&format!("--wifi: {e}")));
    }
    let rom = spec.rom.as_ref().map(PathBuf::from).or_else(|| find_rom(<esp32c6::C6 as Soc>::ROM_ELF))
        .unwrap_or_else(|| usage_error("--coproc: no esp32c6_rev0_rom.elf found (rom=FILE)"));
    let image = std::fs::read(&rom).unwrap_or_else(|e| usage_error(&format!("--coproc rom {}: {e}", rom.display())));
    m.load_rom(&image).unwrap_or_else(|e| usage_error(&format!("--coproc rom {}: {e}", rom.display())));
    let read = |p: &str| std::fs::read(p).unwrap_or_else(|e| usage_error(&format!("--coproc {p}: {e}")));
    for (off, path) in [(0, &spec.bootloader), (0x8000, &spec.ptable), (0x10000, &spec.app)] {
        if let Some(p) = path { m.write_flash(off, &read(p)).unwrap_or_else(|e| usage_error(&format!("--coproc {p}: {e}"))); }
    }
    if let Some((p, off)) = &spec.otadata { m.write_flash(*off, &read(p)).unwrap_or_else(|e| usage_error(&format!("--coproc {p}: {e}"))); }
    for p in &spec.elfs { m.add_symbols(&read(p)).unwrap_or_else(|e| usage_error(&format!("--coproc elf {p}: {e}"))); }
    for st in &spec.stubs {
        let (name, val) = stub_spec(st).unwrap_or_else(|e| usage_error(&format!("--coproc stub: {e}")));
        let addr = m.resolve_stub(name).unwrap_or_else(|| usage_error(&format!("--coproc stub: unknown symbol {name}")));
        m.stubs.insert(addr, val);
    }
    if o.no_jit { for c in &mut m.cores { c.set_jit(false); } }
    m.boot_rom();
    m
}

fn setup_s3(o: &Opts) -> esp32s3::Machine {
    let size_error = o.cam_size.as_deref().and_then(|s| camera::size(s).err());
    for (message, invalid) in [
        ("--cam-stream requires --board waveshare-cam", o.cam_stream.is_some() && o.board != "waveshare-cam"),
        (size_error.as_deref().unwrap_or(""), size_error.is_some()),
    ] {
        if invalid { usage_error(message); }
    }
    let mut m = esp32s3::machine(o.mac.unwrap_or([0x44, 0x1b, 0xf6, 0x75, 0xdc, 0xe0]));
    m.bus.board = esp32s3::board::make_board(&o.board).unwrap_or_else(|| { eprintln!("unknown board '{}' (atech14, waveshare-cam, waveshare-lcd4b, waveshare-amoled18-v2, stopwatch, none)", o.board); std::process::exit(2) });
    if o.measured_te {
        assert_eq!(m.bus.board.name(), "waveshare-amoled18-v2", "--measured-te requires the AMOLED V2 board");
        m.bus.board = Box::new(esp32s3::board::WaveshareAmoled18V2::with_measured_te());
    }
    m.bus.spi2_timing = o.spi2_timing;
    m.bus.attach_board_devices();
    if !o.debug.is_empty() { let mut f = esp_soc::DebugFlags::from_env(); for d in &o.debug { f.parse(d); } m.set_debug(&f); }
    if let Some(spec) = &o.wifi {
        let (cfg, nat) = wifi_config(spec, &o.net, m.bus.debug.has("net"), &o.hostfwd);
        m.bus.attach_wifi(cfg, nat).unwrap_or_else(|e| { eprintln!("--wifi: {e}"); std::process::exit(2) });
    }
    attach_nearby_networks(&mut m.bus.periph.wifi.link, o);
    set_camera_image(&mut m.bus.board, o.cam_image.as_deref());
    m.bus.periph.lcd_cam.frame_cycles = (esp32s3::periph::CPU_HZ as f64 / o.cam_fps) as u64;
    let stopwatch = m.bus.board.name() == "stopwatch";
    let flash_mb = o.flash_mb.unwrap_or(if stopwatch { 16 } else { 8 });
    let psram_mb = o.psram_mb.unwrap_or(if stopwatch { 8 } else { 2 });
    if flash_mb != 8 { m.bus.set_flash_size(flash_mb << 20); }
    if psram_mb != 2 { m.bus.set_psram_size(psram_mb << 20).unwrap(); }
    if o.flash_octal { m.bus.set_flash_octal(true); }
    if let Some(p) = &o.efuse_regs {
        let txt = std::fs::read_to_string(p).expect("efuse file");
        let mut n = 0;
        for line in txt.lines() {
            let line = line.trim(); if line.is_empty() { continue; }
            let (addr_s, rest) = match line.split_once(':') { Some(x) => x, None => continue };
            let Ok(mut a) = u32::from_str_radix(addr_s.trim().trim_start_matches("0x"), 16) else { continue };
            for w in rest.split_whitespace() { if let Ok(v) = u32::from_str_radix(w, 16) { let off = if a >= 0x6000_7000 { a - 0x6000_7000 } else { a }; m.bus.periph.efuse.ram.write(off, v); a += 4; n += 1; } }
        }
        eprintln!("[emu] loaded {} efuse words from {}", n, p);
    }
    if let Some(p) = &o.regs_init {
        let txt = std::fs::read_to_string(p).expect("regs-init file");
        let mut n = 0;
        for line in txt.lines() {
            let (addr_s, rest) = match line.trim().split_once(':') { Some(x) => x, None => continue };
            let Ok(mut a) = u32::from_str_radix(addr_s.trim().trim_start_matches("0x"), 16) else { continue };
            for w in rest.split_whitespace() { if let Ok(v) = u32::from_str_radix(w, 16) { if m.bus.periph.init_regs(a, v) { n += 1; } a += 4; } }
        }
        eprintln!("[emu] applied {} reset-state register words from {}", n, p);
    }
    if let Some(p) = &o.regstat { m.add_observer(Box::new(MmioHeat::new(p, |a| { let b = a.wrapping_sub(esp32s3::periph::PERIPH_BASE) >> 12; format!("{}+0x{:03x}", esp32s3::periph::Peripherals::block_name_pub(b), a & 0xfff) }))); }
    m
}

/// `--wifi-nearby SPEC` (repeatable): the networks around the station, which a scan lists and
/// nothing can join; they need the access point of `--wifi`. Each one kept is named, and each one
/// left out (the access point's own SSID or BSSID) says why.
fn attach_nearby_networks(link: &mut esp_soc::wifi::StationLink, o: &Opts) {
    if o.wifi_nearby.is_empty() { return; }
    if o.wifi.is_none() { usage_error("--wifi-nearby needs the access point of --wifi"); }
    let list = esp_soc::wifi::BeaconAp::parse_all(&o.wifi_nearby).unwrap_or_else(|e| { eprintln!("--wifi-nearby: {e}"); std::process::exit(2) });
    for (b, why) in link.set_beacons(list).unwrap_or_default() {
        eprintln!("[emu] --wifi-nearby '{}' left out: it has {}", b.ssid, why);
    }
    for b in link.ap().map_or(&[][..], |ap| &ap.beacons) {
        eprintln!("[emu] network around: '{}' bssid {} channel {} {} dBm ({})", b.ssid, esp_soc::wifi::mac_str(&b.bssid), b.channel, b.rssi, if b.secure { "WPA2-PSK" } else { "open" });
    }
}

/// `--wifi SPEC` with `--net`: the virtual access point's configuration and, for `--net nat`,
/// the NAT to the host, the same for every chip.
fn fault_label(f: &cosim::Fault) -> String {
    match f.when {
        cosim::When::Nth { n, byte: Some(k) } => format!("{}@{n}:{k}", f.kind.name()),
        cosim::When::Nth { n, byte: None } => format!("{}@{n}", f.kind.name()),
        cosim::When::At { ns, len: 0 } => format!("{}@{}s", f.kind.name(), ns as f64 / 1e9),
        cosim::When::At { ns, len } => format!("{}@{}s+{}s", f.kind.name(), ns as f64 / 1e9, len as f64 / 1e9),
    }
}

/// `--wifi` and `--net MODE[,epoch=UNIX_SECONDS]`: the access point, and the NAT for `nat` (or
/// `user`); `epoch` fixes the virtual network's clock (its SNTP answers) at power-on.
fn wifi_config(spec: &str, net: &str, net_log: bool, hostfwd: &[String]) -> (esp_soc::wifi::ApConfig, Option<esp_soc::nat::Nat>) {
    let mut cfg = esp_soc::wifi::ApConfig::parse(spec).unwrap_or_else(|e| { eprintln!("--wifi: {e}"); std::process::exit(2) });
    let (mode, opts) = net.split_once(',').unwrap_or((net, ""));
    for kv in opts.split(',').filter(|kv| !kv.is_empty()) {
        match kv.split_once('=') {
            Some(("epoch", v)) => cfg.epoch_s = Some(esp_soc::net::parse_epoch(v).unwrap_or_else(|e| usage_error(&format!("--net: {e}")))),
            _ => usage_error(&format!("--net: unknown option {kv:?} (epoch=UNIX_SECONDS)")),
        }
    }
    let nat = mode == "nat" || mode == "user";
    eprintln!("[emu] virtual AP '{}' bssid {} channel {} ({})", cfg.ssid, esp_soc::wifi::mac_str(&cfg.bssid), cfg.channel, if cfg.psk.is_some() { "WPA2-PSK" } else { "open" });
    let mut nat = nat.then(|| esp_soc::nat::Nat::new(net_log));
    if let Some(nat) = &mut nat {
        eprintln!("[emu] NAT to the host network enabled (DNS via {}.{}.{}.{})", nat.resolver[0], nat.resolver[1], nat.resolver[2], nat.resolver[3]);
        for spec in hostfwd {
            let parsed = esp_soc::nat::HostFwdSpec::parse(spec).unwrap_or_else(|e| usage_error(&format!("--hostfwd: {e}")));
            let bound = nat.listen(&parsed).unwrap_or_else(|e| usage_error(&format!("--hostfwd {spec}: {e}")));
            eprintln!("[emu] hostfwd tcp {} -> {}.{}.{}.{}:{}", bound, parsed.guest_ip[0], parsed.guest_ip[1], parsed.guest_ip[2], parsed.guest_ip[3], parsed.guest_port);
        }
    }
    let net = esp_soc::net::VirtualNet::new(false);   // its fixed addresses, for the line below
    eprintln!("[emu] virtual network: station {}.{}.{}.{}, gateway {}.{}.{}.{} (DHCP, ARP, ICMP, DNS, NTP)", net.sta_ip[0], net.sta_ip[1], net.sta_ip[2], net.sta_ip[3], net.gw_ip[0], net.gw_ip[1], net.gw_ip[2], net.gw_ip[3]);
    (cfg, nat)
}

fn setup_c3(o: &Opts) -> esp32c3::Machine {
    let mut m = esp32c3::machine(o.mac.unwrap_or([0x60, 0x55, 0xf9, 0x00, 0x11, 0x22]), o.flash_mb.unwrap_or(4) << 20);
    m.bus.set_flash_size(o.flash_mb.unwrap_or(4) << 20);   // the JEDEC capacity follows the size
    if !o.debug.is_empty() { let mut f = esp_soc::DebugFlags::from_env(); for d in &o.debug { f.parse(d); } m.set_debug(&f); }
    if o.ble_full {
        m.bus.enable_ble_full(o.ble_observe).unwrap_or_else(|e| usage_error(&e));
        m.bus.periph.ble_lc.scan(o.ble_scan);
    }
    if let Some(spec) = &o.wifi {
        let (cfg, nat) = wifi_config(spec, &o.net, m.bus.debug.has("net"), &o.hostfwd);
        m.bus.attach_wifi(cfg, nat).unwrap_or_else(|e| { eprintln!("--wifi: {e}"); std::process::exit(2) });
    }
    attach_nearby_networks(&mut m.bus.periph.wifi.link, o);
    for (flag, on) in [("--board", o.board != "atech14" && o.board != "none"), ("--cam-image", o.cam_image.is_some()), ("--cam-stream", o.cam_stream.is_some()), ("--cam-size", o.cam_size.is_some()), ("--psram-mb", o.psram_mb.is_some()), ("--flash-octal", o.flash_octal), ("--efuse-regs", o.efuse_regs.is_some()), ("--regs-init", o.regs_init.is_some()), ("--regstat", o.regstat.is_some())] {
        if on { eprintln!("{} is not available on the C3", flag); std::process::exit(2); }
    }
    m
}

fn setup_esp32(o: &Opts) -> esp32::Machine {
    let mut m = esp32::machine(o.mac.unwrap_or([0x24, 0x6f, 0x28, 0x00, 0x11, 0x22]), o.flash_mb.unwrap_or(4) << 20);
    m.bus.set_flash_size(o.flash_mb.unwrap_or(4) << 20);
    if !o.debug.is_empty() { let mut f = esp_soc::DebugFlags::from_env(); for d in &o.debug { f.parse(d); } m.set_debug(&f); }
    let board = if o.board == "atech14" { "none" } else { &o.board };
    m.bus.board = esp32::board::make_board(board).unwrap_or_else(|| usage_error(&format!("unknown classic ESP32 board '{board}' (none, bare, esp32dev, ttgo-tdisplay, ai-thinker-cam, ulanzi-tc001)")));
    m.bus.attach_board_devices();
    set_camera_image(&mut m.bus.board, o.cam_image.as_deref());
    if let Some(spec) = &o.wifi {
        let (cfg, nat) = wifi_config(spec, &o.net, m.bus.debug.has("net"), &o.hostfwd);
        m.bus.attach_wifi(cfg, nat).unwrap_or_else(|e| { eprintln!("--wifi: {e}"); std::process::exit(2) });
    }
    attach_nearby_networks(&mut m.bus.periph.wifi.link, o);
    if let Some(mb) = o.psram_mb {
        if !matches!(mb, 0 | 4) { usage_error("--psram-mb: classic ESP32 supports only 0 or 4 MiB"); }
        m.bus.set_psram_size(mb << 20).unwrap_or_else(|e| usage_error(&e));
    }
    for (flag, on) in [("--flash-octal", o.flash_octal), ("--efuse-regs", o.efuse_regs.is_some()), ("--regs-init", o.regs_init.is_some()), ("--regstat", o.regstat.is_some())] {
        if on { eprintln!("{} is not available on the classic ESP32", flag); std::process::exit(2); }
    }
    m
}

fn setup_c6(o: &Opts) -> esp32c6::Machine {
    let mut m = esp32c6::machine(o.mac.unwrap_or([0xdc, 0x1e, 0xd5, 0x6e, 0x8c, 0xdc]), o.flash_mb.unwrap_or(4) << 20);
    m.bus.set_flash_size(o.flash_mb.unwrap_or(4) << 20);   // the JEDEC capacity follows the size
    if !o.debug.is_empty() { let mut f = esp_soc::DebugFlags::from_env(); for d in &o.debug { f.parse(d); } m.set_debug(&f); }
    let name = if o.board == "atech14" { "none" } else { o.board.as_str() };   // the S3 default means "bare module" here
    match esp32c6::board::make_board(name) { Some(b) => m.bus.board = b, None => { eprintln!("--board {}: none or waveshare-c6-lcd147 on the C6", name); std::process::exit(2) } }
    m.bus.attach_board_devices();
    if let Some(spec) = &o.wifi {
        let (cfg, nat) = wifi_config(spec, &o.net, m.bus.debug.has("net"), &o.hostfwd);
        m.bus.attach_wifi(cfg, nat).unwrap_or_else(|e| { eprintln!("--wifi: {e}"); std::process::exit(2) });
    }
    attach_nearby_networks(&mut m.bus.periph.wifi_mac.link, o);
    for (flag, on) in [("--cam-image", o.cam_image.is_some()), ("--cam-stream", o.cam_stream.is_some()), ("--cam-size", o.cam_size.is_some()), ("--psram-mb", o.psram_mb.is_some()), ("--flash-octal", o.flash_octal), ("--efuse-regs", o.efuse_regs.is_some()), ("--regs-init", o.regs_init.is_some()), ("--regstat", o.regstat.is_some())] {
        if on { eprintln!("{} is not available on the C6", flag); std::process::exit(2); }
    }
    m
}

/// The images are a rev >= 3 build: the bootloader's header says so (`--bootloader`, or the image
/// `--flash-at` places at 0x2000), or else the app's, or that of another image `--flash-at`
/// places, or that of the bootloader in the flash image (`esp32p4::built_for_rev3`). The silicon
/// revision follows them.
fn p4_rev3(o: &Opts) -> bool {
    let head = |p: &str, at: usize| std::fs::read(p).ok().and_then(|d| d.get(at..).map(esp32p4::built_for_rev3));
    let placed = |boot: bool| o.flash_at.iter().filter_map(|s| s.split_once('='))
        .filter(move |(off, _)| (usize::from_str_radix(off.trim_start_matches("0x"), 16) == Ok(0x2000)) == boot).map(|(_, p)| p);
    o.bootloader.as_deref().and_then(|p| head(p, 0))
        .or_else(|| placed(true).find_map(|p| head(p, 0)))
        .or_else(|| o.app.as_deref().and_then(|p| head(p, 0)))
        .or_else(|| placed(false).find_map(|p| head(p, 0)))
        .or_else(|| o.flash_image.as_deref().and_then(|p| head(p, 0x2000))).unwrap_or(false)
}

/// The ESP32-P4, a bare module: 16 MB of flash unless told otherwise, PSRAM with `--psram-mb`.
fn setup_p4<R: esp32p4::Revision>(o: &Opts) -> esp_soc::Machine<esp32p4::Esp32p4<R>> {
    let flash = o.flash_mb.unwrap_or(16) << 20;
    let mut m = esp32p4::machine_rev::<R>(o.mac.unwrap_or([0x30, 0xed, 0xa0, 0x00, 0x11, 0x22]), flash);
    m.bus.set_flash_size(flash);   // the JEDEC capacity follows the size
    if let Some(mb) = o.psram_mb { m.bus.set_psram_size(mb << 20).unwrap(); }
    if !o.debug.is_empty() { let mut f = esp_soc::DebugFlags::from_env(); for d in &o.debug { f.parse(d); } m.set_debug(&f); }
    let board = if o.board == "atech14" { "none" } else { o.board.as_str() };   // the S3 default means "bare module" here
    match esp32p4::board::make_board(board) { Some(b) => m.bus.board = b, None => usage_error(&format!("--board {board}: none or waveshare-p4-lcd35 on the P4")) }
    m.bus.attach_board_devices();
    for (flag, on) in [("--wifi", o.wifi.is_some() && o.coproc.is_none()), ("--cam-image", o.cam_image.is_some()), ("--cam-stream", o.cam_stream.is_some()), ("--cam-size", o.cam_size.is_some()), ("--flash-octal", o.flash_octal), ("--efuse-regs", o.efuse_regs.is_some()), ("--regs-init", o.regs_init.is_some()), ("--regstat", o.regstat.is_some())] {
        if on { eprintln!("{} is not available on the P4{}", flag, if flag == "--wifi" { " (it goes to a co-processor: --coproc)" } else { "" }); std::process::exit(2); }
    }
    m
}

/// Everything after the chip is set up: images, boot, observers, the run, the reports.
fn run<S: Soc>(mut m: Machine<S>, o: &Opts) {
    let approximate = o.approximate_timing.then(|| {
        let mut config = esp32s3::ApproximateTimingConfig::default();
        if o.approximate_cache && o.approximate_memory.is_none() {
            config.data_cache = Some(cache_config().unwrap_or_else(|e| usage_error(&e)));
        }
        esp32s3::ApproximateCostModel::new(config)
    });
    let memory_model = o.approximate_memory.map(|extra| {
        use esp32s3::rough_memory::{MemoryConfig, MemoryPrice};
        let external = MemoryPrice { latency: extra, ..MemoryPrice::FREE };
        let model = esp32s3::memory_cost_model::MemoryCostModel::new(approximate.as_ref().unwrap().clone(),
            MemoryConfig { flash: external, psram: external, contention: o.memory_contention, ..Default::default() });
        if o.approximate_cache { model.with_cache(cache_config().unwrap_or_else(|e| usage_error(&e))) } else { model }
    });
    if let Some(model) = &approximate {
        let cost: Box<dyn emu_core::CostModel> = match &memory_model {
            Some(memory) => Box::new(memory.clone()), None => Box::new(model.clone()),
        };
        m.set_cost_model(cost).unwrap_or_else(|e| usage_error(&format!("--approximate-timing: {e}")));
        eprintln!("[emu] APPROXIMATE timing: {:?}; use --boot rom; accuracy unvalidated", model.config);
    }
    let boot = prepare(&mut m, o);
    let cmds = o.usb_host.as_ref().map(|path| {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| usage_error(&format!("--usb-host {path}: {e}")));
        let cmds = esp_soc::usb_host::script::parse(&text).unwrap_or_else(|e| usage_error(&format!("--usb-host {path}: {e}")));
        m.bus.attach_usb_host(Default::default()).unwrap_or_else(|e| usage_error(&format!("--usb-host: {e}")));
        eprintln!("[emu] USB host on the device port: {} commands from {}", cmds.len(), path);
        cmds
    });
    let t0 = std::time::Instant::now();
    m.web_restart = !o.no_reboot;
    let (reboot, app) = (!o.no_reboot && boot == "rom", boot == "app");
    let (insns, ended) = match cmds { Some(cmds) => run_usb_host(&mut m, &cmds, o.max_insns, reboot, app), None => (o.max_insns, None) };
    let stop = match ended {
        Some(stop) => stop,
        None => run_with_reboots(&mut m, insns, reboot, app),
    };
    let dt = t0.elapsed().as_secs_f64();
    report(&mut m, o, stop, dt);
    let pending = m.bus.ble_pending_commands();
    if pending != 0 { eprintln!("[ble] {pending} command(s) still pending at end of run"); }
    if let Some(model) = approximate { eprintln!("[emu] approximate timing totals: {:?}", model.stats()); }
    if let Some(model) = memory_model {
        eprintln!("[emu] approximate memory totals [internal, ROM, flash, PSRAM, MMIO]: {:?}", model.memory.borrow().stats);
        if let Some(cache) = &model.cache { eprintln!("[emu] approximate physical data cache: {:?}", cache.borrow().stats()); }
    }
}

/// `--usb-host`: the host's commands, each running the machine for as long as it takes, under the
/// run's limits and its reboot policy (`run_with_reboots`); the run then goes on to its time
/// limit. The instructions left, and how the run ended if the script's run ended it.
fn run_usb_host<S: Soc>(m: &mut Machine<S>, cmds: &[(usize, esp_soc::usb_host::script::Cmd)], insns: u64, reboot: bool, app: bool) -> (u64, Option<Stop>) {
    let mut port = CliPort { m, insns, reboot, app, ended: None };
    let mut r = esp_soc::usb_host::script::Runner::new(&mut port);
    match r.run(cmds) {
        Ok(()) => r.host().say(format_args!("script done")),
        Err((ln, e)) => r.host().say(format_args!("line {ln} failed: {e}")),
    }
    drop(r);
    // `run_machine_until` leaves max_cycles at 0 when the run's own --script stopped it.
    if port.ended.is_none() && port.m.max_cycles == 0 { port.m.drain_console(); port.ended = Some(Stop::Halted); }
    let ended = port.ended.or((port.insns == 0).then_some(Stop::MaxInsns));
    (port.insns, ended)
}

/// The machine as the USB host's port sees it in a CLI run: `--max-insns` counts, and a chip reset
/// comes back up only where `run_with_reboots` would bring it back.
struct CliPort<'a, S: Soc> { m: &'a mut Machine<S>, insns: u64, reboot: bool, app: bool, ended: Option<Stop> }

impl<S: Soc> esp_soc::usb_host::UsbPort for CliPort<'_, S> {
    fn link(&mut self) -> Option<&mut esp_periph::usb_otg::UsbLink> { self.m.bus.usb_link() }
    fn run_until(&mut self, t_ns: u64) -> Result<(), String> {
        if let Some(s) = &self.ended { return Err(format!("the run ended: {s:?}")); }
        let (reboot, app, ended) = (self.reboot, self.app, &mut self.ended);
        esp_soc::usb_host::run_machine_until(self.m, t_ns, &mut self.insns, |m| {
            let button = m.take_button_reset();
            if !reboot && !(button && app) { *ended = Some(Stop::SwReset); return Err("the chip reset, and this run does not reboot".into()); }
            m.reboot();
            if !reboot {
                let entry = m.boot_app(0x10000).map_err(|e| { *ended = Some(Stop::SwReset); format!("restart: {e}") })?;
                eprintln!("[emu] restart: app entry {:#010x} {}", entry, m.sym(entry));
            }
            Ok(())
        })
    }
    fn console(&mut self, text: &str) { self.m.bus.uart_input(0, text.as_bytes()); }
}

/// Run, coming back up after a chip reset where that is possible: through the ROM when it booted
/// the run (`reboot`), and for an app-mode run (`app`) when the reset was the page's Restart — the
/// chip is reset and the app mapped and entered again, as at startup. A reset the firmware asked
/// for still ends an app-mode run: there is no ROM to take it through.
fn run_with_reboots<S: Soc>(m: &mut Machine<S>, mut remaining: u64, reboot: bool, app: bool) -> Stop {
    loop {
        let before = m.run_steps();
        let stop = m.run(remaining);
        remaining = remaining.saturating_sub(m.run_steps().saturating_sub(before));
        if let Stop::SwReset = stop {
            let cause = m.bus.reset_cause();
            eprintln!("[emu] chip reset at t={:.3}s: cause {:#x} ({})", m.seconds(), cause, esp_periph::reset_cause_name(cause));
            let button = m.take_button_reset();
            if !reboot && !(button && app) { return stop; }
            if remaining == 0 { return Stop::MaxInsns; }
            m.reboot();
            if !reboot {
                match m.boot_app(0x10000) {
                    Ok(entry) => eprintln!("[emu] restart: app entry {:#010x} {}", entry, m.sym(entry)),
                    Err(e) => { eprintln!("[emu] restart: {}", e); return stop; }
                }
            }
        } else { return stop; }
    }
}

/// Images, boot, observers, scripts: everything before the first instruction. Returns the boot mode.
fn prepare<S: Soc>(m: &mut Machine<S>, o: &Opts) -> String {
    let rom_boot = S::ROM_BOOT_DEFAULT;
    let boot = o.boot.clone().unwrap_or_else(|| if rom_boot { "rom".into() } else { "app".into() });
    let console = o.console.clone().unwrap_or_else(|| if rom_boot { "uart0".into() } else { "both".into() });
    m.bus.misc().log_unknown = o.log_periph;
    if !o.breaks.is_empty() { m.add_observer(Box::new(Breakpoints { pcs: o.breaks.clone() })); }
    if o.trace { m.add_observer(Box::new(Trace { from: o.trace_from })); }
    let rom = o.rom.clone().or_else(|| find_rom(S::ROM_ELF));
    match &rom {
        Some(r) => match std::fs::read(r) { Ok(d) => { m.load_rom(&d).expect("rom"); eprintln!("[emu] ROM loaded from {}", r.display()); } Err(e) => eprintln!("[emu] no ROM ({}): {}", r.display(), e) },
        None if boot == "rom" => { eprintln!("[emu] no {} mask ROM ELF found (pass --rom, or use --boot app)", S::NAME); std::process::exit(2) }
        None => {}
    }
    // an image the model cannot run (one built for another silicon revision) stops the run before it starts
    let refuse = |path: &str, r: Result<(), String>| if let Err(e) = r { eprintln!("[emu] {path} {e}"); std::process::exit(2) };
    if let Some(p) = &o.flash_image { let d = std::fs::read(p).expect("flash image"); refuse(p, esp_soc::load::check_flash_image::<S>(&d)); m.write_flash(0, &d).unwrap(); }
    if let Some(p) = &o.bootloader { let d = std::fs::read(p).expect("bootloader"); refuse(p, S::check_image(&d)); m.write_flash(S::BOOTLOADER_OFFSET, &d).unwrap(); }
    if let Some(p) = &o.ptable { m.write_flash(0x8000, &std::fs::read(p).expect("ptable")).unwrap(); }
    if let Some(p) = &o.app { let d = std::fs::read(p).expect("app"); refuse(p, S::check_image(&d)); m.write_flash(0x10000, &d).unwrap(); }
    for spec in &o.flash_at {
        let (off, path) = spec.split_once('=').unwrap_or_else(|| { eprintln!("--flash-at needs OFFSET=FILE"); std::process::exit(2) });
        let off = usize::from_str_radix(off.trim_start_matches("0x"), 16).unwrap_or_else(|_| { eprintln!("--flash-at: bad offset {}", off); std::process::exit(2) });
        let data = std::fs::read(path).unwrap_or_else(|e| { eprintln!("--flash-at: {}: {}", path, e); std::process::exit(2) });
        refuse(path, S::check_image(&data));
        m.write_flash(off, &data).unwrap_or_else(|e| { eprintln!("--flash-at: {}", e); std::process::exit(2) });
        eprintln!("[emu] flash {:#x}: {} ({} bytes)", off, path, data.len());
    }
    if let Some((path, rw)) = &o.sd {
        let image = if *rw { esp_periph::sdmmc::FileStore::write_through(path) } else { esp_periph::sdmmc::FileStore::copy_on_write(path) };
        let image = image.unwrap_or_else(|e| usage_error(&format!("--sd {path}: {e}")));
        let config = esp_periph::sdmmc::SdCardConfig { prg_ns: o.sd_prg_us.unwrap_or(0).saturating_mul(1000), serial: o.sd_serial, timed_bus: o.sd_timing, ..Default::default() };
        match m.bus.attach_sd_card(Box::new(image), config) { Ok(w) => eprintln!("[emu] SD card: {w}"), Err(e) => usage_error(&format!("--sd: {e}")) }
    }
    let mut ble_elf = esp_soc::elf::Elf::default();
    for p in &o.elfs {
        let data = std::fs::read(p).expect("elf");
        m.add_symbols(&data).expect("elf symbols");
        if o.ble { let elf = esp_soc::elf::parse(&data).expect("BLE ELF symbols"); if elf.by_name.contains_key("esp_bt_controller_init") { ble_elf = elf; } }
    }
    if o.ble { m.bus.enable_ble(&ble_elf).unwrap_or_else(|e| usage_error(&format!("--ble: {e}"))); }
    if !o.ble_nearby.is_empty() {
        let list = esp_soc::ble::peer::Nearby::parse_all(&o.ble_nearby).unwrap_or_else(|e| usage_error(&format!("--ble-nearby: {e}")));
        for n in &list {
            let fields = esp_soc::ble::peer::advertising_fields(&n.adv);
            eprintln!("[emu] device around: {} ({}) {} dBm {}", esp_soc::ble::peer::addr_text(&n.addr), if n.addr_type == 0 { "public" } else { "random" }, n.rssi, fields.join(" "));
        }
        m.bus.ble_nearby(list).unwrap_or_else(|e| usage_error(&format!("--ble-nearby: {e}")));
    }
    if let Some(p) = &o.script { m.load_script(&std::fs::read_to_string(p).unwrap_or_else(|e| usage_error(&format!("script: {e}")))).unwrap_or_else(|e| usage_error(&format!("script: {e}"))); }
    if let Some(s) = &o.serial { m.bus.serial_input(s.as_bytes()); }
    apply_adc_raw(m, &o.adc_raw);
    for pre in &o.trace_fns {
        let n = m.trace_fns(pre);
        eprintln!("[emu] --trace-fn {}: {} functions", pre, n);
    }
    for st in &o.stubs {
        let (name, val) = stub_spec(st).unwrap_or_else(|e| { eprintln!("--stub: {e}"); std::process::exit(2) });
        let addr = m.resolve_stub(name).unwrap_or_else(|| { eprintln!("--stub: unknown symbol {}", name); std::process::exit(2) });
        eprintln!("[emu] stub {} @ {:#010x} -> returns {:#x}", name, addr, val);
        m.stubs.insert(addr, val);
    }
    if o.no_jit { for c in &mut m.cores { c.set_jit(false); } }
    match boot.as_str() {
        "app" => match m.boot_app(0x10000) { Ok(entry) => eprintln!("[emu] app boot: entry {:#010x} {}", entry, m.sym(entry)), Err(e) => { eprintln!("[emu] {}", e); std::process::exit(2) } },
        "rom" => { m.boot_rom(); eprintln!("[emu] ROM boot from reset vector {:#010x}", m.cores[0].pc()); }
        _ => { eprintln!("--boot app|rom"); std::process::exit(2); }
    }
    // Match a real board's boot conditions: the ROM prints the reset cause and the strapping-derived boot mode
    if let Some(c) = o.reset_cause { m.bus.set_reset_cause(c); }
    if let Some(v) = o.strap { m.bus.set_strap(v); }
    for &(a, n) in &o.peeks { eprintln!("[peek before run]\n{}", m.peek(a, n)); }
    m.dbg.stop_after_exceptions = o.stop_exc;
    m.console.mask = console_mask(&console);
    m.console.prefix = o.console_prefix;
    if let Some(p) = &o.regtrace { m.add_observer(Box::new(RegTrace::new(std::fs::File::create(p).expect("regtrace file"), o.regtrace_max, o.regtrace_from_pc))); }
    if let Some(port) = o.web_port {
        let dir = o.web_dir.clone().unwrap_or_else(|| { let exe = std::env::current_exe().unwrap(); let mut d = exe.parent().unwrap().to_path_buf(); for _ in 0..3 { if d.join("web").exists() { break; } d = d.parent().unwrap().to_path_buf(); } d.join("web").to_string_lossy().to_string() });
        let w = esp_soc::web::WebServer::start(port, dir.clone()).expect("web server");
        eprintln!("[emu] board UI: http://127.0.0.1:{}/  (serving {})", port, dir);
        m.web = Some(w); m.rt.enabled = true;
    }
    if let Some(path) = &o.cam_stream {
        let (width, height) = camera::size(o.cam_size.as_deref().unwrap()).unwrap();
        let input = std::sync::Arc::new(std::sync::Mutex::new(None));
        m.camera_input = Some(input.clone());
        let path = path.clone();
        std::thread::spawn(move || {
            let reader: std::io::Result<Box<dyn std::io::Read>> = if path == "-" { Ok(Box::new(std::io::stdin())) }
                else { std::fs::File::open(path).map(|file| Box::new(file) as Box<dyn std::io::Read>) };
            let result = reader.and_then(|reader| camera::read_frames(reader, width, height, |frame| *input.lock().unwrap() = Some(frame)));
            if let Err(e) = result { eprintln!("[emu] camera stream: {e}"); }
        });
        m.rt.enabled = true;
    }
    if o.realtime { m.rt.enabled = true; }
    if o.profile { m.add_observer(Box::new(PcHist::new(12))); }
    if let Some(wa) = o.watch { let v = m.bus.read32_unpriced(wa).unwrap_or(0); m.add_observer(Box::new(Watch { addr: wa, value: v })); }
    if o.profile_blocks { m.add_observer(Box::new(BlockProfile::new(20))); }
    if let Some(path) = &o.coverage { m.add_observer(Box::new(Coverage::new(path.clone()))); }
    if o.irq_latency { m.add_observer(Box::new(IrqLatency::new(S::CORES))); }
    if let Some(p) = &o.vcd { m.add_observer(Box::new(Vcd::new(p, S::CPU_HZ))); }
    for tone in &o.i2s_tone {
        let fields: Vec<_> = tone.split(':').collect();
        let configure = || -> Result<(usize, f64, f64), String> {
            let [p, h, a] = fields[..] else { return Err("expected PORT:HZ:AMPLITUDE".into()); };
            Ok((p.parse().map_err(|_| "invalid port")?, h.parse().map_err(|_| "invalid frequency")?, a.parse().map_err(|_| "invalid amplitude")?))
        };
        let result = configure().and_then(|(port, hz, amplitude)| {
            m.bus.i2s_input(port).ok_or_else(|| "controller absent".to_string())?.tone(hz, amplitude).map_err(String::from)
        });
        if let Err(e) = result { eprintln!("--i2s-tone: {e}"); std::process::exit(2); }
    }

    if let Some(sec) = o.max_seconds { m.max_cycles = (sec * S::CPU_HZ as f64) as u64; }
    boot
}

/// The end-of-run report on stderr: counts, faults, peeks, observer reports, captures, registers.
fn report<S: Soc>(m: &mut Machine<S>, o: &Opts, stop: Stop, dt: f64) {
    let total = m.insns();
    let per: String = if S::CORES == 1 { format!("{}", total) } else { m.cores.iter().enumerate().map(|(i, c)| format!("core{} {}", i, c.insn_count())).collect::<Vec<_>>().join(" + ") };
    eprintln!("\n[emu] stop: {:?} — {} insns in {:.1}s wall = {:.1} Minsn/s; emulated {:.3}s ({} cycles); {} exceptions, {} interrupts",
              stop, per, dt, total as f64 / dt / 1e6, m.seconds(), m.bus.cycles(), m.exceptions, m.interrupts);
    if let Stop::Unimplemented(pc, raw) = stop {
        if let Ok(b) = m.bus.fetch(pc) { eprintln!("[emu] unimplemented at {:08x} {}: {} (raw {:#x})", pc, m.sym(pc), m.cores[0].disasm(pc, b), raw); }
    }
    if let Some((a, w)) = m.bus.last_fault() { eprintln!("[emu] last bus fault: {} {:#010x}", if w { "write" } else { "read" }, a); }
    for &(a, n) in &o.peeks { eprintln!("[peek after run]\n{}", m.peek(a, n)); }
    for &pin in &o.pwm_pins {
        match m.bus.pwm_output(pin) {
            Some((hz, duty)) => eprintln!("[pwm] GPIO{}: {:.3} Hz, {:.2}% duty", pin, hz, duty as f64 * 100.0 / 65535.0),
            None => eprintln!("[pwm] GPIO{}: no supported running PWM", pin),
        }
    }
    for &pin in &o.adc_pins {
        match m.bus.adc_observation(pin) {
            Some(s) => eprintln!("[adc] GPIO{}: raw {}, generation {}", pin, s.raw, s.generation),
            None => eprintln!("[adc] GPIO{}: no ADC pad", pin),
        }
    }
    for &(a, n) in &o.disasms { eprintln!("[disasm {:#010x}]\n{}", a, m.disasm(a, n)); }
    { let r = m.reports(); if !r.is_empty() { eprintln!("{}", r); } }
    eprintln!("{}", m.irq_report());
    if let Some(w) = &o.wav { match m.write_wav(w) { Ok(n) => eprintln!("[emu] wrote {} samples ({:.2} s) to {}", n, n as f64 / m.bus.audio().1 as f64, w), Err(e) => eprintln!("[emu] wav: {}", e) } }
    { let r = m.bus.report(); if !r.is_empty() { eprintln!("{}", r); } }
    { let st: Vec<(u64, u64, u64, usize)> = m.cores.iter().filter_map(|c| c.code_cache_stats()).collect();
      if m.vq_stats[0] > 0 { eprintln!("[emu] virtual quanta: runs, quanta, stopped at a device register, at waiti = {:?}", m.vq_stats); }
      if st.iter().any(|s| s.0 > 0) { let b0 = st[0]; let b1 = st.get(1).copied().unwrap_or((0, 0, 0, 0)); eprintln!("[emu] blocks: {} built ({} cache flushes) core0, {} ({}) core1; jit: {} compiled, {} KB code", b0.0, b0.1, b1.0, b1.1, b0.2 + b1.2, (b0.3 + b1.3) / 1024); } }
    if m.stub_hits > 0 { eprintln!("[emu] stubs hit {} times", m.stub_hits); }
    if let Some(p) = &o.tft_png { match m.write_tft_png(p, 3) { Ok(()) => eprintln!("[emu] wrote {}", p), Err(e) => eprintln!("[emu] png: {}", e) } }
    if let Some(p) = &o.gram_png { match m.write_gram_png(p) { Ok(()) => eprintln!("[emu] wrote {}", p), Err(e) => eprintln!("[emu] png: {}", e) } }
    if o.dump { eprintln!("{}", m.dump_regs()); }
}

#[cfg(test)]
mod tests;
