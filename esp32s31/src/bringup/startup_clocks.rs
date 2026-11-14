//! S31 startup clock requests for otherwise unimplemented peripheral domains.
//! ESP-IDF6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! soc/esp32s31/register/soc/{hp_sys_clkrst,hp_alive_sys}_reg.h and
//! esp_hal_clock/esp32s31/include/hal/clk_gate_ll.h::periph_ll_clk_gate_set_default.
//! No peripheral data/ready registers are mapped here. Existing UART/SHA/TIMG/
//! Flash/PSRAM/modem clocks retain their actual owners; this is not a broad RegRam.
use emu_core::Fault;
#[derive(Clone,Copy)]
struct Word { address: u32, reset: u32, mask: u32, writable: u32 }
const WORDS: [Word;20] = [
    Word { address:0x205870a8,reset:0x40,mask:0x7e,writable:0x42 }, // ETM APB/core; alternate source/reset absent
    Word { address:0x20587024,reset:3,mask:255,writable:3 }, // TRACE CPU/SYS; trace engine absent
    Word { address:0x20587078,reset:1,mask:7,writable:1 }, // AXI PDMA SYS
    Word { address:0x2058707c,reset:1,mask:7,writable:1 }, // AHB PDMA SYS
    Word { address:0x20587080,reset:1,mask:3,writable:1 }, // REGDMA SYS
    Word { address:0x205871b0,reset:1,mask:7,writable:1 }, // ASRC SYS
    Word { address:0x2058706c,reset:0x800043,mask:0xffffff,writable:0x800043 }, // GPSPI2 clocks,not transfers
    Word { address:0x20587070,reset:0x800043,mask:0xffffff,writable:0x800043 }, // GPSPI3
    Word { address:0x205871d0,reset:1,mask:7,writable:1 }, // HP I2C master APB
    Word { address:0x20589004,reset:0xc0000000,mask:0xc0000000,writable:0xc0000000 }, // PARLIO TX/RX pad
    Word { address:0x20589008,reset:0x80000000,mask:0x80000000,writable:0x80000000 }, // I2S0 MCLK
    Word { address:0x2058900c,reset:0x80000000,mask:0x80000000,writable:0x80000000 }, // I2S1 MCLK
    // tee_reg.h/lp_tee_reg.h CLOCK_GATE: keep-on1 vs automatic gating0.
    // These do not disable permissions or claim TEE transitions are implemented.
    Word { address:0x205043f8,reset:1,mask:1,writable:1 },
    Word { address:0x20706bf8,reset:1,mask:1,writable:1 },
    // cnnt_sys_reg.h: UTMI-FS clock only; OTG PHY/AHB/APB stay in reset.
    Word { address:0x20359030,reset:0xe0800000,mask:0xef800000,writable:0x00800000 },
    // lp_gpio_reg.h free-running/automatic clock request, no LP GPIO data plane.
    Word { address:0x207133f8,reset:1,mask:1,writable:1 },
    // lp_peri_clkrst_reg.h; LP analog-I2C/touch/debug data paths absent.
    Word { address:0x20710018,reset:0x40000000,mask:0xc0000000,writable:0x40000000 },
    Word { address:0x2071002c,reset:0x40000000,mask:0xc0000000,writable:0x40000000 },
    Word { address:0x20710050,reset:0x80000000,mask:0x80000000,writable:0x80000000 },
    // The startup LL selects BBPLL120/div2 while disabled. Retain this requested
    // source/divisor; enabling LCDCAM or a fractional/other source still faults.
    Word { address:0x20587154,reset:0,mask:0x7ffffff,writable:0x7fb },
];
pub struct StartupClocks { values:[u32;20] }
impl Default for StartupClocks { fn default()->Self { Self { values:WORDS.map(|word|word.reset) } } }
impl StartupClocks {
    pub fn is_mmio(address:u32)->bool { matches!(address & !3, 0x205870a8 | 0x20587024 | 0x20587078 | 0x2058707c | 0x20587080 | 0x205871b0 | 0x2058706c | 0x20587070 | 0x205871d0 | 0x20589004 | 0x20589008 | 0x2058900c | 0x205043f8 | 0x20706bf8 | 0x20359030 | 0x207133f8 | 0x20710018 | 0x2071002c | 0x20710050 | 0x20587154) }
    pub fn read(&self,address:u32)->Result<u32,Fault> {
        let i=WORDS.iter().position(|w|w.address==address).ok_or(Fault::Unmapped)?; Ok(self.values[i])
    }
    pub fn write(&mut self,address:u32,value:u32)->Result<(),Fault> {
        let i=WORDS.iter().position(|w|w.address==address).ok_or(Fault::Unmapped)?;let w=WORDS[i];let value=value&w.mask;
        if (value^self.values[i]) & !w.writable!=0 { return Err(Fault::Prohibited); }
        if address==0x20587154 && value&3>1 { return Err(Fault::Prohibited); }
        self.values[i]=value;Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_domains_keep_independent_requests_and_reject_unimplemented_resets() {
        let mut c=StartupClocks::default();
        for w in WORDS { assert_eq!(c.read(w.address),Ok(w.reset)); }
        c.write(0x205870a8,0).unwrap();assert_eq!(c.read(0x205870a8),Ok(0));assert_eq!(c.read(0x20587024),Ok(3));
        c.write(0x20587078,0).unwrap();assert_eq!(c.read(0x2058707c),Ok(1));
        assert_eq!(c.write(0x20587078,2),Err(Fault::Prohibited));assert_eq!(c.read(0x20587078),Ok(0));
        assert_eq!(c.write(0x205870a8,0x10),Err(Fault::Prohibited));
        c.write(0x20587154,9).unwrap();assert_eq!(c.read(0x20587154),Ok(9));
        assert_eq!(c.write(0x20587154,13),Err(Fault::Prohibited));assert_eq!(c.read(0x20587154),Ok(9));
        for a in [0x20587058,0x20587114,0x2058711c,0x20587088,0x20587068,0x2010f018] { assert!(!StartupClocks::is_mmio(a)); }
        assert_eq!(c.read(0x205870a9),Err(Fault::Unmapped));assert_eq!(c.write(0x2050a000,0),Err(Fault::Unmapped));
    }
}
