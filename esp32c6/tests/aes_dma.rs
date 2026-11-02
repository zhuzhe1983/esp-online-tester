use esp32c6::bus::{SocBus, SRAM_LOW as RAM};
const AES: u32 = 0x6008_8000;
const GDMA: u32 = 0x6008_0000;
fn channels(bus: &mut SocBus) -> (esp_periph::GdmaOutCh, esp_periph::GdmaInCh) { (bus.periph.gdma.gdma.out[0], bus.periph.gdma.gdma.inp[0]) }
include!("../../tests/aes_dma.rs");
