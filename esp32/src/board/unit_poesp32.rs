//! Exact U138 SCH_UNIT-PoESP32.pdf (SHA256b33df28d...) and product pin map.
//! WROOM-32U4MiB/no PSRAM + IP101G address1; NOT W5500. RESET_N GPIO5
//! has an external10k pullup; PHY supplies REF_CLK_50M to ESP32 GPIO0.
//! UART Grove direction is module RX16/TX17; internal download uses UART0.
use esp_soc::BoardModel;
pub const PHY_ADDRESS:u8=1;
pub const MDC:u8=23;
pub const MDIO:u8=18;
pub const RMII:[u8;6]=[21,19,22,25,26,27];
pub const REF_CLK:u8=0;
pub const RESET_N:u8=5;
pub const AT_UART:[u8;2]=[16,17];
pub struct UnitPoEsp32;
impl Default for UnitPoEsp32 {fn default()->Self {Self}}
impl BoardModel for UnitPoEsp32 {
    fn name(&self)->&'static str {"unit-poesp32"}
    fn emac_config(&self)->Option<esp_soc::board::EmacConfig> {Some(esp_soc::board::EmacConfig{phy:esp_soc::board::EthernetPhy::Ip101g,phy_addr:PHY_ADDRESS,mdc:MDC,mdio:MDIO,rmii:RMII,ref_clk:Some(REF_CLK),reset_pin:Some(RESET_N)})}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(RESET_N,true),(AT_UART[0],true)]}
    fn uses_gpio_edges(&self)->bool {false}
    fn hardware_diagnostics(&self)->Option<String> {Some(format!("{{\"phy\":\"IP101G\",\"address\":{},\"mdc\":{},\"mdio\":{},\"refClock\":{},\"resetN\":{},\"uartRx\":{},\"uartTx\":{}}}",PHY_ADDRESS,MDC,MDIO,REF_CLK,RESET_N,AT_UART[0],AT_UART[1]))}
    fn report(&self)->String {"[unit-poesp32] U138 WROOM32U4MiB IP101G/RMII addr1; PoE analog conversion and physical wireless unmodeled".into()}
}
