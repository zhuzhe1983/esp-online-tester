//! Run a caller-provided C61 image through the genuine mask ROM for bring-up.
use esp_soc::SocBus;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(args.len() >= 3, "firmware ROM_ELF MERGED_BIN [INSNS] [PYTHON] [SD_IMAGE] [PYTHON_INSNS]");
    let mut m = esp32c61::machine([2, 0, 0, 0, 0, 0x61], 16 << 20);
    m.bus.set_flash_size(16 << 20); m.bus.set_psram_size(8 << 20).unwrap();
    m.bus.periph.usb.connected = true;
    m.bus.periph.spi2.log = std::env::var_os("COREMATRIX_SPI_TRACE").is_some();
    m.bus.board = esp32c61::board::make_board("corematrix").unwrap(); m.bus.attach_board_devices();
    if let Some(path)=args.get(5) {m.bus.attach_sd_card(Box::new(esp_periph::sdmmc::MemStore(std::fs::read(path).unwrap())),Default::default()).unwrap();}
    m.load_rom(&std::fs::read(&args[1]).unwrap()).unwrap();
    m.write_flash(0, &std::fs::read(&args[2]).unwrap()).unwrap();
    m.boot_rom(); m.dbg.stop_after_exceptions = 1;
    let stop = m.run(args.get(3).map(|x| x.parse().unwrap()).unwrap_or(50_000_000));
    if let Some(path) = args.get(4) {
        m.bus.serial_input(&[1]); m.run(2_000_000);
        for chunk in std::fs::read(path).unwrap().chunks(64) { m.bus.serial_input(chunk); m.run(1_000_000); }
        m.bus.serial_input(&[4]); m.run(args.get(6).map(|x|x.parse().unwrap()).unwrap_or(100_000_000)); m.bus.serial_input(&[2]); m.run(2_000_000);
    }
    if let Some((leds, generation)) = m.bus.board.leds() { eprintln!("matrix generation {generation} leds {leds:?}"); }
    eprintln!("report {}", m.bus.report());
    eprintln!("ADC oneshot {:08x} raw {:08x}", m.bus.periph.read32(0x6000e020), m.bus.periph.read32(0x6000e044));
    eprintln!("stop {stop:?} insns {} cycles {} pc {:08x} cause {:08x} mepc {:08x} regs {:x?}",m.insns(),m.bus.cycles,m.cores[0].pc,m.cores[0].mcause,m.cores[0].mepc,m.cores[0].x);
}
