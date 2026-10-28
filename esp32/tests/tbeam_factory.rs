//! Fixed official T-Beam V1.2 SX1276 merged image; no substituted firmware.
use std::{fs,path::Path};
use esp_soc::SocBus as _;
use esp_soc::devices::sx1262::{LoRaConfig,LoRaInput};
fn run_to(m:&mut esp32::Machine,seconds:f64) {m.max_cycles=(seconds*240_000_000.0) as u64;while m.bus.cycles<m.max_cycles {let stop=m.run(2_000_000);assert!(matches!(stop,esp32::Stop::MaxInsns|esp32::Stop::Halted),"unexpected {stop:?}");}m.drain_console();}
fn serial(m:&esp32::Machine)->String {String::from_utf8_lossy(&m.console.all).into_owned()}
#[test]
#[ignore = "set TBEAM_FACTORY_BIN to the pinned official image and ESP32SIM_ROM_DIR to the ROM directory"]
fn external_factory_pmu_user_button_tx_explicit_rx_and_console_configuration() {
    let firmware=fs::read(std::env::var_os("TBEAM_FACTORY_BIN").expect("set TBEAM_FACTORY_BIN to the pinned official image")).unwrap();assert_eq!(firmware.len(),1004784);assert_eq!(firmware.iter().fold(0xcbf29ce484222325u64,|h,b|(h^u64::from(*b)).wrapping_mul(0x100000001b3)),0xa3abe35666195998);
    let rom=fs::read(Path::new(&std::env::var_os("ESP32SIM_ROM_DIR").expect("set ESP32SIM_ROM_DIR")).join("esp32_rev300_rom.elf")).unwrap();
    let mut m=esp32::machine([2,0,0,0,0,1],4<<20);m.console.capture=true;m.bus.board=esp32::board::make_board("lilygo-t-beam-v1-2").unwrap();m.bus.attach_board_devices();m.load_rom(&rom).unwrap();m.write_flash(0,&firmware).unwrap();m.boot_rom();
    run_to(&mut m,42.0);let s=serial(&m);assert!(s.contains("AXP2101 PMU init succeeded"));assert!(s.contains("[SX1276]:LoRa Initialized Success!"));assert!(s.contains("Failed to find Display"));assert!(s.contains("PSRAM is disable!"));assert!(m.bus.board.display().is_none());println!("FACTORY boot: PMU and SX1276 initialized, optional OLED absent, PSRAM/GNSS unavailable");
    m.bus.gpio_set_input(38,false);run_to(&mut m,42.2);m.bus.gpio_set_input(38,true);run_to(&mut m,44.0);assert!(serial(&m).contains("Start transmit"),"{}",serial(&m));assert!(serial(&m).contains("[RadioTx] transmission finished!"));assert!(!serial(&m).contains("[RadioRx] Received packet!"));println!("FACTORY GPIO38: radio TX and DIO0 completion");
    // The factory performs blocking radio.transmit() inside its UI callback.
    // AceButton also rejects clicks held past its click timeout. Retry short
    // physical presses at varied guest phases, stopping on observed RX output.
    for attempt in 0..10 {if serial(&m).contains("Start receive"){break;}let now=m.bus.cycles as f64/240_000_000.0;m.bus.gpio_set_input(38,false);run_to(&mut m,now+0.15);m.bus.gpio_set_input(38,true);run_to(&mut m,now+0.77+f64::from(attempt)*0.013);}
    assert!(serial(&m).contains("Start receive"),"{}",serial(&m));let ready=m.bus.cycles as f64/240_000_000.0;
    let mut peer=LoRaInput{config:LoRaConfig{frequency:0x36400000,sf:10,bandwidth:4,coding_rate:2,sync_word:0x1424,implicit_header:false,crc:false,invert_iq:false},payload:7u32.to_le_bytes().to_vec(),rssi_dbm:-70,snr_quarter_db:24,crc_error:false};let cycle=m.bus.cycles;peer.config.frequency+=64;assert!(!m.bus.board.lora_receive_at(cycle,&peer));peer.config.frequency-=64;assert!(m.bus.board.lora_receive_at(cycle,&peer));run_to(&mut m,ready+1.0);let s=serial(&m);assert!(s.contains("[RadioRx] Received packet! COUNT:7"),"{s}");println!("FACTORY explicit peer: payload 07000000 decoded as COUNT:7; mismatch rejected");
    let cycle=m.bus.cycles;assert!(m.bus.board.button_at(cycle,"power",true));run_to(&mut m,ready+1.2);let cycle=m.bus.cycles;assert!(m.bus.board.button_at(cycle,"power",false));run_to(&mut m,ready+2.0);assert!(serial(&m).contains("isPekeyShortPress"));println!("FACTORY PMU power key: IRQ status consumed and short-press handler reached");
    m.bus.uart_input(0,b"freq:915.0\n");run_to(&mut m,ready+4.0);assert!(serial(&m).contains("Frequency set to 915.00 MHz"));let state=m.bus.board.lora_snapshot().unwrap();assert!(state.contains("\"frequency\":959447040"));println!("FACTORY console: freq:915.0 changed guest radio registers");
    assert_eq!(serial(&m).matches("setupBoards").count(),1);assert!(!serial(&m).contains("Guru Meditation"));println!("FACTORY PASS cycles={} instructions={} radio={state}",m.bus.cycles,m.insns());
}
