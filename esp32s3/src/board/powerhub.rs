//! C148 PowerHub / K148 kit: ESP32-S3-WROOM-1U-N16R8, main b0.3 schematic.
//! USR_SW2 -> G11 with R81 pull-up, switch to GND. Power buttons and RGB
//! belong to STM32G031 and must not be mapped to ESP GPIOs or invented LEDs.
use esp_soc::BoardModel;
pub struct PowerHub;
impl BoardModel for PowerHub {
 fn name(&self)->&'static str {"powerhub"}
 fn named_pin(&self,id:&str)->Option<u8> {matches!(id,"button-user").then_some(11)}
 fn input_levels(&self)->Vec<(u8,bool)> {vec![(11,true)]}
 fn report(&self)->String {"[powerhub] ESP32-S3 CPU/UART and physical G11 user key only; STM32G031, RGB, power buttons, rails, CAN/RS485, USB switching and RTC unmodeled; no battery simulation".into()}
}
#[cfg(test)]mod tests {
 use super::*;
 #[test]fn pmu_controls_are_not_fabricated_gpio_or_outputs() {
  let b=PowerHub;assert_eq!(b.named_pin("button-user"),Some(11));assert_eq!(b.input_levels(),vec![(11,true)]);
  for id in ["power-top","power-side","reset","button-a"]{assert_eq!(b.named_pin(id),None);}
  assert!(b.leds().is_none());assert!(b.display().is_none());
 }
}
