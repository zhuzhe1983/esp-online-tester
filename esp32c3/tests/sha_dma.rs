use esp32c3::bus::{SocBus, DRAM_LOW as RAM};
const SHA: u32 = 0x6003_b000;
const SELECT: u32 = 0x6003_f100;
const LINK: u32 = 0x6003_f0e0;
const CONF1: u32 = 0x6003_f0d4;
fn channel(bus: &mut SocBus) -> &mut esp_periph::GdmaOutCh { &mut bus.periph.gdma.gdma.out[0] }
include!("../../tests/sha_dma.rs");
