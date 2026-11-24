//! Original guest initializes its own pads, serial mode and MMU, then verifies
//! every word of the configured 8 MiB die and executes code through the mapping.
use esp_soc::SocBus;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = std::env::args().nth(1).ok_or("usage: psram_probe probe.bin")?;
    let mut machine = esp32c5::machine([0; 6], 4 << 20);
    machine.bus.set_xtal_mhz(48)?;
    machine.bus.set_psram_size(8 << 20)?;
    machine.console.capture = true;
    machine.console.mask = 2;
    machine.load_input(esp_soc::LoadKind::App, &std::fs::read(app)?)?;
    machine.boot_app(0x10000)?;
    while !machine.console.uart0.ends_with(b"C5 PSRAM8 PASS\n") && machine.bus.cycles() < 100_000_000 {
        machine.max_cycles = machine.bus.cycles() + 2_000_000;
        let stop = machine.run(u64::MAX);
        if !matches!(stop, esp_soc::Stop::MaxInsns | esp_soc::Stop::Halted) {
            return Err(format!("stop={stop:?}; pc={:#x}; fault={:?}", machine.cores[0].pc, machine.bus.first_fault).into());
        }
    }
    if !machine.console.uart0.ends_with(b"C5 PSRAM8 PASS\n") || machine.exceptions != 0 || machine.bus.first_fault.is_some() {
        return Err(format!("guest validation failed: {:?}", String::from_utf8_lossy(&machine.console.uart0)).into());
    }
    eprint!("{}", String::from_utf8_lossy(&machine.console.uart0));
    println!("{{\"kind\":\"original-c5-sram-guest\",\"psramBytes\":{},\"wordsWrittenAndReadByGuest\":2097152,\"mappedPages\":128,\"nominalCrystalMHz\":48,\"cycles\":{},\"dispatched\":{},\"retired\":{},\"exceptions\":{},\"resets\":{},\"spiAndQpiReadWrite\":true,\"mappedReadWrite8_16_32\":true,\"executedFromPsram\":true,\"factoryCompatibilityVerified\":false}}", machine.bus.psram.len(), machine.bus.cycles(), machine.insns(), machine.cores[0].retired_count, machine.exceptions, machine.reboots);
    Ok(())
}
