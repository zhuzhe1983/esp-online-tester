//! Boot an IDF v6.1 esp32s31 flash image from the mask ROM on the Machine path.
//!   cargo run -p esp32s31 --release --example s31_hello -- ROM.elf FW_DIR [insns]
use esp_soc::SocBus as _;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rom = std::fs::read(&a[1]).unwrap();
    let d = &a[2];
    let max: u64 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(200_000_000);
    let mut m = esp32s31::machine([0x10, 0x20, 0x30, 0x40, 0x50, 0x60], 16 << 20);
    m.bus.load_mask_rom(&rom).unwrap();
    for (off, f) in [(0x2000, "s31-hello-bootloader.bin"), (0x8000, "s31-hello-ptable.bin"), (0x10000, "s31-hello_world.bin")] {
        m.bus.write_flash(off, &std::fs::read(format!("{d}/{f}")).unwrap()).unwrap();
    }
    m.bus.set_board_inputs(esp32s31::soc::BoardInputs::devkit_w25q128()).unwrap();
    m.bus.set_strap(std::env::var("S31_STRAP").ok().and_then(|s| u32::from_str_radix(&s,16).ok()).unwrap_or(0x10)); m.boot_rom(); m.cores[0].x[2] = esp32s31::ROM_STACK;
    m.console.mask = 2;
    let mut done = 0u64;
    while done < max {
        let chunk = if std::env::var("S31_TRAP").is_ok() { 1 } else { 1_000_000 };
        let stop = m.run(chunk); done += chunk;
        if chunk == 1 { for h in 0..2 { let c = &m.cores[h]; if m.bus.inner.first_fault.is_some() { eprintln!("\n[s31] trap hart{h} pc={:#x} mepc={:#x} mcause={:#x} mtval={:#x} first={:?}", c.pc, c.mepc, c.mcause, c.mtval, m.bus.inner.first_fault); return; } } }
        match stop { esp_soc::Stop::MaxInsns => {}, esp_soc::Stop::SwReset => { eprintln!("\n[s31] SwReset at pc={:#x} first={:?} usb={:?}", m.cores[0].pc, m.bus.inner.first_fault, String::from_utf8_lossy(&m.console.usb)); if std::env::var("S31_NOREBOOT").is_ok() { break; } m.reboot(); }, s => { eprintln!("\n[s31] stop {s:?} pc={:#x}", m.cores[0].pc); break; } }
    }
    eprintln!("\n[s31] pc0={:#x} pc1={:#x} insns={} fault={:?} first={:?}", m.cores[0].pc, m.cores[1].pc, m.cores[0].insn_count, m.bus.last_fault(), m.bus.inner.first_fault);
}
