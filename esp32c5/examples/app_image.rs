//! Native smoke of the same Machine/App/boot_app contract used by the browser ABI.
use esp_soc::SocBus;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let board = args.next().ok_or("usage: app_image stamp-c5|stamp-c5-dip app.bin [UART-bytes]")?;
    let path = args.next().ok_or("missing ESP app image")?; let input = args.next().unwrap_or_else(|| "10W".into());
    if args.next().is_some() { return Err("unexpected argument".into()); }
    let mut m = esp32c5::machine([0; 6], 4 << 20); m.console.capture = true; m.console.mask = 2;
    m.bus.attach_board(esp32c5::board::make_board(&board).ok_or("unknown C5 board")?);
    m.load_input(esp_soc::LoadKind::App, &std::fs::read(path)?)?; m.boot_app(0x10000)?;
    for next in std::iter::once(None).chain(input.bytes().map(Some)) {
        if let Some(byte) = next { m.bus.uart_input(0, &[byte]); }
        m.max_cycles = m.bus.cycles() + 2000;
        let stop = m.run(u64::MAX);
        if !matches!(stop, esp_soc::Stop::MaxInsns | esp_soc::Stop::Halted) { return Err(format!("C5 stopped: {stop:?}; {:?}", m.bus.first_fault).into()); }
        eprintln!("{}", m.bus.board_ref().report());
    }
    print!("{}", String::from_utf8_lossy(&m.console.uart0));
    eprintln!("C5 app-direct cycles={} slots={} retired={} IRQ={} exceptions={}", m.bus.cycles(), m.insns(), m.cores[0].retired_count, m.interrupts, m.exceptions);
    Ok(())
}
