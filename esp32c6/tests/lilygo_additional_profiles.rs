#[path="../src/board/lilygo_additional.rs"]
mod lilygo_additional;
use esp32c6::periph;
use emu_core::Bus;
use esp_soc::{BoardModel,SocBus as _};
use esp_soc::devices::sx1262::{LoRaInput,LoRaConfig};
use lilygo_additional::{TLoRaC6,TPicoC6};

// ESP-IDF v5.5.4 C6 GPIO/IO_MUX/SPI register map; transfers traverse the
// guest-facing bus, GPIO matrix and existing SX1262 protocol implementation.
fn drive(b:&mut esp32c6::bus::SocBus,pin:u8,high:bool) {
    b.write32(0x6009_0004+4*u32::from(pin),1<<12|1<<9).unwrap();b.write32(0x6009_1554+4*u32::from(pin),128).unwrap();
    b.write32(0x6009_1024,1<<pin).unwrap();b.write32(if high {0x6009_1008}else{0x6009_100c},1<<pin).unwrap();
}
fn radio()->esp32c6::Machine {
    let mut m=esp32c6::machine([0;6],4<<20);let b=&mut m.bus;b.board=Box::new(TLoRaC6::new());b.attach_board_devices();
    for (pin,signal) in [(6,63),(0,65),(18,68)] {b.write32(0x6009_0004+4*pin,1<<12|1<<9).unwrap();b.write32(0x6009_1554+4*pin,signal).unwrap();}
    b.write32(0x6009_0008,1<<12|1<<9).unwrap();b.write32(0x6009_1254,0x80|1).unwrap();b.write32(0x6008_1020,0x3e).unwrap();
    drive(b,21,true);drive(b,14,false);drive(b,15,false);b.tick((periph::CPU_HZ/100) as u32);m
}
fn spi(b:&mut esp32c6::bus::SocBus,bytes:&[u8])->Vec<u8> {
    assert!(bytes.len()<=64 && !bytes.is_empty());
    for (index,word) in bytes.chunks(4).enumerate() {let mut w=[0;4];w[..word.len()].copy_from_slice(word);b.write32(0x6008_1098+4*index as u32,u32::from_le_bytes(w)).unwrap();}
    b.write32(0x6008_1010,1<<27|1<<28).unwrap();b.write32(0x6008_101c,(bytes.len()*8-1) as u32).unwrap();b.write32(0x6008_1000,1<<24).unwrap();
    (0..bytes.len()).map(|i|((b.read32(0x6008_1098+4*(i/4) as u32).unwrap()>>((i%4)*8))&255) as u8).collect()
}
fn command(b:&mut esp32c6::bus::SocBus,bytes:&[u8])->Vec<u8> {let rx=spi(b,bytes);b.tick((periph::CPU_HZ/100) as u32);rx}
fn snapshot(b:&esp32c6::bus::SocBus)->String {b.board.lora_snapshot().unwrap()}
fn configured()->esp32c6::Machine {
    let mut m=radio();for bytes in [vec![0x80,0],vec![0x8a,1],vec![0x86,0x36,0x40,0,0],vec![0x8b,7,4,1,0],vec![0x8c,0,8,0,3,1,0],vec![0x08,3,0xff,3,0xff,0,0,0,0],vec![0x0e,0,1,2,3]] {command(&mut m.bus,&bytes);}m
}
fn packet()->LoRaInput {LoRaInput{config:LoRaConfig{frequency:0x36400000,sf:7,bandwidth:4,coding_rate:1,sync_word:0x1424,implicit_header:false,crc:true,invert_iq:false},payload:vec![9,8,7],rssi_dbm:-70,snr_quarter_db:24,crc_error:false}}

#[test]
fn lora_spi_discovery_survives_disabled_antenna_but_obeys_mux_nss_reset_and_miso() {
    let mut m=radio();let b=&mut m.bus;let read=[0x1d,3,0x20,0,0,0,0,0,0,0];
    assert_eq!(&command(b,&read)[4..],b"SX1261","antenna off must not block SPI");
    assert!(snapshot(b).contains("\"antennaEnabled\":false"));assert!(b.board.display().is_none());assert!(!b.board.uses_gpio_waveform());
    b.write32(0x6009_0004,0).unwrap();assert_eq!(spi(b,&read),[255;10]);b.write32(0x6009_0004,1<<12).unwrap();
    b.write32(0x6009_1254,0x80|2).unwrap();assert_eq!(spi(b,&read),[255;10]);b.write32(0x6009_1254,0x80|1).unwrap();
    b.write32(0x6008_1020,0x3f).unwrap();assert_eq!(spi(b,&read),[255;10]);b.write32(0x6008_1020,0x3e).unwrap();
    // RadioLib can split a command while GPIO18 holds NSS low.
    b.write32(0x6008_1020,0x3f).unwrap();drive(b,18,false);spi(b,&read[..4]);assert_eq!(spi(b,&[0;6]),b"SX1261");drive(b,18,true);
    drive(b,21,false);drive(b,18,false);assert_eq!(spi(b,&read),[255;10]);assert_ne!(b.read32(0x6009_103c).unwrap()&(1<<22),0);
    drive(b,18,true);drive(b,21,true);b.tick((periph::CPU_HZ/100) as u32);drive(b,18,false);assert_eq!(&spi(b,&read)[4..],b"SX1261");drive(b,18,true);
}

#[test]
fn lora_packets_require_correct_direction_switches_and_cancel_interrupted_rx() {
    let mut m=configured();let b=&mut m.bus;
    drive(b,14,false);drive(b,15,true);command(b,&[0x83,0,0,0]);b.tick(periph::CPU_HZ as u32);
    assert!(snapshot(b).contains("\"transmitted\":[]"));assert!(snapshot(b).contains("\"droppedTx\":1"));
    drive(b,15,false);drive(b,14,true);command(b,&[0x02,0xff,0xff]);command(b,&[0x83,0,0,0]);b.tick(periph::CPU_HZ as u32);
    assert!(snapshot(b).contains("\"payloadHex\":\"010203\""));assert_ne!(b.gpio_input()&(1<<23),0);
    command(b,&[0x02,0xff,0xff]);assert_eq!(b.gpio_input()&(1<<23),0);
    command(b,&[0x82,0xff,0xff,0xff]);assert!(!b.board.lora_receive_at(b.cycles(),&packet()),"TX path cannot receive");
    drive(b,14,false);drive(b,15,true);assert!(b.board.lora_receive_at(b.cycles(),&packet()));b.tick(periph::CPU_HZ as u32);
    assert!(snapshot(b).contains("\"rxCount\":1"));assert_eq!(&command(b,&[0x1e,0,0,0,0,0])[3..],[9,8,7]);
    command(b,&[0x02,0xff,0xff]);assert!(b.board.lora_receive_at(b.cycles(),&packet()));drive(b,14,true);b.tick(periph::CPU_HZ as u32);
    assert!(snapshot(b).contains("\"rxCount\":1"));assert_eq!(b.gpio_input()&(1<<23),0,"both RF switches high cancel pending RX");
    assert!(!b.board.lora_receive_at(b.cycles(),&packet()));
    drive(b,14,false);assert!(b.board.lora_receive_at(b.cycles(),&packet()));drive(b,21,false);b.tick(periph::CPU_HZ as u32);
    assert!(snapshot(b).contains("\"rxCount\":1"));assert!(snapshot(b).contains("\"config\":null"));
}

#[test]
fn lora_led_and_boot_are_electrical_and_pico_does_not_claim_rp2350_peripherals() {
    let mut m=radio();let b=&mut m.bus;drive(b,7,false);assert_eq!(b.board.leds().unwrap().0,[[0,255,0]]);
    b.write32(0x6009_1554+4*7,128|1<<8).unwrap();assert_eq!(b.board.leds().unwrap().0,[[0;3]]);
    drive(b,7,false);b.write32(0x6009_1028,1<<7).unwrap();assert_eq!(b.board.leds().unwrap().0,[[0;3]]);
    drive(b,7,false);b.write32(0x6009_0004+4*7,0).unwrap();assert_eq!(b.board.leds().unwrap().0,[[0;3]]);
    drive(b,7,true);assert_eq!(b.board.leds().unwrap().0,[[0;3]]);
    for mut board in [Box::new(TLoRaC6::new()) as Box<dyn BoardModel>,Box::new(TPicoC6::new())] {
        board.take_edges();assert_eq!(board.named_pin("boot"),Some(9));assert!(board.button_at(20,"boot",true));assert!(board.button_at(21,"boot",true));
        assert_eq!(board.take_edges(),[esp_soc::board::BoardEdge{cycle:20,pin:9,level:false}]);assert!(board.button_at(30,"boot",false));
        assert_eq!(board.take_edges(),[esp_soc::board::BoardEdge{cycle:30,pin:9,level:true}]);assert!(!board.button_at(31,"reset",true));
    }
    let mut pico=TPicoC6::new();assert_eq!(pico.name(),"lilygo-t-pico-c6");assert!(pico.display().is_none());assert!(pico.leds().is_none());assert!(pico.i2c_devices().is_empty());assert!(pico.sd_card_slot().is_none());
    assert!(!pico.button_at(1,"user",true));assert!(pico.lora_snapshot().is_none());assert!(pico.report().contains("ESP32-C6 coprocessor only"));
}
