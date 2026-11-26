//! U195 Sch_Unit_Gateway_H2_v0.3; M141 Sch_Module-Gateway_H2_v0.4.
//! Both source ESP32-H2-MINI-1-N2, UART0 RX23/TX24, 2MiB Flash.
//! M141 uses its UART DIP path; no SPI slave or wireless gateway behavior.
use esp_soc::{BoardModel,uart::UartRoute};
pub struct GatewayH2 {unit:bool,tx:Vec<u8>,count:u64}
impl GatewayH2 {pub fn new(unit:bool)->Self {Self{unit,tx:Vec::new(),count:0}}}
impl BoardModel for GatewayH2 {
 fn name(&self)->&'static str {if self.unit {"m5-unit-gateway-h2"}else{"m5-module-gateway-h2"}}
 fn uses_uart_pins(&self)->bool {true}
 fn uart_tx(&mut self,_cycle:u64,r:UartRoute,byte:u8) {if r.port==0&&r.transmits_on(24)&&r.matches_baud(115200) {if self.tx.len()==64 {self.tx.remove(0);}self.tx.push(byte);self.count=self.count.wrapping_add(1);}}
 fn input_levels(&self)->Vec<(u8,bool)> {vec![(9,true)]}
 fn named_pin(&self,id:&str)->Option<u8> {(self.unit&&id=="boot").then_some(9)}
 fn hardware_diagnostics(&self)->Option<String> {Some(format!("{{\"schemaVersion\":1,\"board\":\"{}\",\"state\":{{\"uartTxCount\":\"{}\",\"uartTxTailHex\":\"{}\",\"txGpio\":24,\"rxGpio\":23,\"baud\":115200}},\"limits\":[\"Digital UART FIFO link only; no radio/ROM/Flash boot or line timing\"]}}",self.name(),self.count,self.tx.iter().map(|b|format!("{b:02x}")).collect::<String>()))}
 fn report(&self)->String {format!("[{}] H2-MINI-1-N2 2MiB; SRAM UART0 digital host link TX24/RX23 @115200; tx_count={} tail_hex={}; {}; ROM/Flash boot, SPI slave, RF/BLE/802.15.4/Thread/Matter unimplemented",self.name(),self.count,self.tx.iter().map(|b|format!("{b:02x}")).collect::<String>(),if self.unit {"G9 BOOT"}else{"UART DIP-path subset; DIP electrical switching not modeled"})}
}
