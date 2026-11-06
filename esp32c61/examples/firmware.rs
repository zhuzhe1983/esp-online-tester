//! Run a caller-provided C61 image through the genuine mask ROM for bring-up.
use esp_soc::SocBus;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(args.len() >= 3, "firmware ROM_ELF MERGED_BIN [INSNS]");
    let mut m = esp32c61::machine([2, 0, 0, 0, 0, 0x61], 16 << 20);
    m.bus.set_flash_size(16 << 20); m.bus.set_psram_size(8 << 20).unwrap();
    m.bus.periph.usb.connected = true;
    m.load_rom(&std::fs::read(&args[1]).unwrap()).unwrap();
    m.write_flash(0, &std::fs::read(&args[2]).unwrap()).unwrap();
    m.boot_rom(); m.dbg.stop_after_exceptions = 1;
    let stop = m.run(args.get(3).map(|x| x.parse().unwrap()).unwrap_or(50_000_000));
    eprintln!("ADC oneshot {:08x} raw {:08x}", m.bus.periph.read32(0x6000e020), m.bus.periph.read32(0x6000e044));
    eprintln!("stop {stop:?} insns {} cycles {} pc {:08x} cause {:08x} mepc {:08x} regs {:x?}",m.insns(),m.bus.cycles,m.cores[0].pc,m.cores[0].mcause,m.cores[0].mepc,m.cores[0].x);
}
