//! Run the catalog-pinned StickC-Plus image from the full ESP32 ROM.
//! Verify both input hashes against docs/platform/evidence/stick-k016-p/factory-inputs.json.
use esp32::board::stickc_plus::{PIN_BUTTON_A, PIN_BUTTON_B, StickCPlus};
use esp_periph::i2c::axp192::Axp192PowerInput;
use esp_periph::i2s::{PcmPins, PcmSource, PcmSources};
use esp_soc::SocBus;
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

const CPU_HZ: u64 = 240_000_000;
const PCM_HZ: u32 = 44_100;

fn advance(m: &mut esp32::Machine, seconds: f64, wall: Instant, tone: Option<f64>) {
    m.max_cycles = (seconds * CPU_HZ as f64) as u64;
    while m.bus.cycles < m.max_cycles {
        assert!(wall.elapsed() < Duration::from_secs(150), "guest timed out");
        if let Some(hz) = tone {
            let cycles = m.bus.cycles;
            let bank = m.bus.periph.i2s[0].inner.pcm_bank().as_mut().unwrap();
            bank.advance_to(cycles, CPU_HZ);
            let src = bank.inputs[0].as_mut().unwrap();
            let queued = src.queued_frames();
            let start = (cycles * u64::from(PCM_HZ) / CPU_HZ) + queued as u64;
            let frames: Vec<_> = (0..4410usize.saturating_sub(queued))
                .map(|index| {
                    let phase = (start + index as u64) as f64 * hz * std::f64::consts::TAU
                        / f64::from(PCM_HZ);
                    [(phase.sin() * 16_384.0).round() as i16; 2]
                })
                .collect();
            src.push(&frames);
        }
        let stop = m.run(2_000_000);
        assert!(
            matches!(stop, esp32::Stop::MaxInsns | esp32::Stop::Halted),
            "guest stopped at {} cycles: {stop:?}",
            m.bus.cycles
        );
    }
}

fn capture(m: &mut esp32::Machine, path: &Path, name: &str) -> (usize, u64) {
    m.bus.refresh_board_outputs();
    let (width, height, pixels, _) = m.bus.board.display().unwrap();
    let nonblack = pixels.iter().filter(|&&pixel| pixel != 0).count();
    let hash = pixels.iter().flat_map(|pixel| pixel.to_le_bytes()).fold(
        0xcbf29ce484222325_u64,
        |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3),
    );
    let frames = m.bus.board.display_frames();
    let receiver = &m.bus.periph.i2s[0].inner;
    println!(
        "{name} cycles={} instructions={} frame={frames} nonblack={nonblack} rgb565_fnv64={hash:016x} rx_hz={:?} source={:?}",
        m.bus.cycles,
        m.insns(),
        receiver.rx_rate(),
        receiver.rx_source()
    );
    let png = path.join(format!("{name}.png"));
    esp_soc::png::write_png_rgb565(
        png.to_str().expect("UTF-8 output path"),
        &pixels,
        width as usize,
        height as usize,
        1,
    )
    .unwrap();
    (nonblack, frames)
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert!(
        (3..=4).contains(&args.len()),
        "Usage: stickc_plus_pages_probe FACTORY_BIN CLASSIC_ROM OUTPUT_DIR [silence|tone1k|tone2k|wrong-pin|low-battery]"
    );
    let mode = args.get(3).map_or("silence", String::as_str);
    let tone = match mode {
        "silence" | "low-battery" => None,
        "tone1k" | "wrong-pin" => Some(1000.0),
        "tone2k" => Some(2000.0),
        _ => panic!("unknown input mode: {mode}"),
    };
    let firmware = fs::read(&args[0]).expect("read factory image");
    let rom = fs::read(&args[1]).expect("read classic ROM ELF");
    let out = Path::new(&args[2]);
    fs::create_dir_all(out).expect("create capture directory");

    let mut m = esp32::machine([2, 0, 0, 0, 0, 1], 4 << 20);
    let board = StickCPlus::new();
    if mode == "low-battery" {
        board.pmic.lock().unwrap().set_power_input(Axp192PowerInput::LOW_BATTERY_3V0);
    }
    m.bus.board = Box::new(board);
    m.bus.attach_board_devices();
    if tone.is_some() {
        let data = if mode == "wrong-pin" { 35 } else { 34 };
        let src = PcmSource::new(PCM_HZ, PcmPins::Pdm { clk: 0, data }).unwrap();
        let mut bank = PcmSources::default();
        bank.inputs.push(Some(src));
        *m.bus.periph.i2s[0].inner.pcm_bank() = Some(Box::new(bank));
    }
    m.load_rom(&rom).expect("load full ROM");
    m.write_flash(0, &firmware).expect("write merged image");
    m.boot_rom();
    let wall = Instant::now();
    if mode == "low-battery" {
        advance(&mut m, 6.0, wall, None);
        let (nonblack, _) = capture(&mut m, out, "low-battery");
        assert!(nonblack > 0, "expected a visible factory warning");
        return;
    }

    advance(&mut m, 11.0, wall, tone);
    assert!(capture(&mut m, out, "before-a").0 > 0);
    m.bus.gpio_set_input(PIN_BUTTON_A, false);
    advance(&mut m, 11.2, wall, tone);
    m.bus.gpio_set_input(PIN_BUTTON_A, true);
    advance(&mut m, 12.0, wall, tone);
    assert!(capture(&mut m, out, "after-a").0 > 0);
    m.bus.gpio_set_input(PIN_BUTTON_B, false);
    advance(&mut m, 12.2, wall, tone);
    m.bus.gpio_set_input(PIN_BUTTON_B, true);
    advance(&mut m, 14.5, wall, tone);
    let (_, previous_frames) = capture(&mut m, out, "after-b");
    advance(&mut m, 15.0, wall, tone);
    let (nonblack, frames) = capture(&mut m, out, "fft");
    assert!(frames > previous_frames, "guest FFT page must keep drawing");
    assert_eq!(nonblack > 0, matches!(mode, "tone1k" | "tone2k"));
}
