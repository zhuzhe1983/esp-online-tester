//! A172 Stamp-AddOn C6 For P4: fixed schematic2026-03-16, C6FH4/4MiB.
//! Mezzanine CMD18/CLK19/D0=20/D1=21/D2=22/D3=23 uses native SDIO func0.
use esp_soc::BoardModel;
pub struct StampAddonC6;
impl BoardModel for StampAddonC6 {
 fn name(&self)->&'static str {"m5-stamp-addon-c6"}
 fn input_levels(&self)->Vec<(u8,bool)> {vec![(8,true),(9,true),(18,true),(20,true),(21,true),(22,true)]}
 fn report(&self)->String {"[m5-stamp-addon-c6] A172 ESP32-C6FH4 4MiB; native SDIO CMD18 CLK19 D0..3=20..23, source-backed mezzanine path; host CMD52/53 reaches guest HINF/SLC/SLCHOST and DMA; no invented user key/LED/display, external P4 execution or factory ESP-Hosted/RF compatibility".into()}
}
