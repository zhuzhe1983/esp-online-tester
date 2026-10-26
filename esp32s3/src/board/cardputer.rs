//! Original M5Stack Cardputer K132/K132-V11: ST7789V2 and GPIO-scanned 74HC138 keyboard.
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,SpiPins};
use esp_soc::pins::PinRoutes;
use super::cardputer_lcd::CardputerLcd;

const INPUTS:[u8;7]=[13,15,3,4,5,6,7];
pub struct Cardputer {lcd:CardputerLcd,keys:[[bool;14];4],selector:Option<u8>,drives:[Option<bool>;7],dirty:[bool;7],ir_clamped:bool,ir_dirty:bool,cycle:u64,gpio_events:u64}
impl Cardputer {
    pub fn new()->Self {Self{lcd:CardputerLcd::new(),keys:[[false;14];4],selector:None,drives:[None;7],dirty:[false;7],ir_clamped:true,ir_dirty:false,cycle:0,gpio_events:0}}
    fn scan(&mut self) {
        for j in 0..7 {
            // Both official schematic URLs (K132 / K132-V11) contain this same
            // 74HC138 wiring. M5Cardputer f1392858 IOMatrix.{h,cpp} maps Y0..7
            // onto physical 4x14 keys: even columns use Y4..7, odd use Y0..3.
            // No switch closes => release, preserving the MCU's selected pulls.
            let mut drive=None;
            if let Some(selected)=self.selector {for row in 0..4 {for parity in 0..2 {if self.keys[row][2*j+parity] {
                let y=(3-row+if parity==0 {4} else {0}) as u8;
                drive=Some(drive.unwrap_or(true) && y!=selected);
            }}}}
            // Opposing 74HC138 outputs from multiple held switches are represented
            // with LOW dominance. Analog contention/ghosting/bounce are not modeled.
            if drive!=self.drives[j] {self.drives[j]=drive;self.dirty[j]=true;}
        }
    }
}
impl Default for Cardputer {fn default()->Self {Self::new()}}
impl BoardModel for Cardputer {
    fn name(&self)->&'static str {"cardputer"}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {self.gpio_events+=changes.len() as u64;self.lcd.gpio(changes);}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        self.cycle=cycle;
        // Official K132/K132-V11 schematic page1: G44 -> IR1 -> R14 22Ω -> GND.
        // Bundled M5GFX 0.1.16 uses its LOW input despite MCU pull-up to distinguish
        // Cardputer from VAMeter. This digital diode load is released for GPIO output;
        // LED voltage/current, RMT emission and optical transmission are not modeled.
        let clamped=routes.software_output(44).is_none();
        if self.ir_clamped!=clamped {self.ir_clamped=clamped;self.ir_dirty=true;}
        self.selector=match (routes.software_output(8),routes.software_output(9),routes.software_output(11)) {
            (Some(a0),Some(a1),Some(a2))=>Some(u8::from(a0)|(u8::from(a1)<<1)|(u8::from(a2)<<2)),_=>None,
        };
        self.scan();
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn matrix_key_at(&mut self,cycle:VirtualCycle,row:u8,column:u8,pressed:bool)->bool {
        if row>=4 || column>=14 {return false;}
        self.cycle=cycle;self.keys[row as usize][column as usize]=pressed;self.scan();true
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {
        let mut edges=Vec::new();for (j,&pin) in INPUTS.iter().enumerate() {if self.dirty[j] {if let Some(level)=self.drives[j] {self.dirty[j]=false;edges.push(BoardEdge{cycle:self.cycle,pin,level});}}}if self.ir_dirty && self.ir_clamped {self.ir_dirty=false;edges.push(BoardEdge{cycle:self.cycle,pin:44,level:false});}edges
    }
    fn released_inputs(&mut self)->Vec<u8> {
        let mut pins=Vec::new();for (j,&pin) in INPUTS.iter().enumerate() {if self.dirty[j] && self.drives[j].is_none() {self.dirty[j]=false;pins.push(pin);}}if self.ir_dirty && !self.ir_clamped {self.ir_dirty=false;pins.push(44);}pins
    }
    fn input_levels(&self)->Vec<(u8,bool)> {std::iter::once((0,true)).chain(self.ir_clamped.then_some((44,false))).chain(INPUTS.iter().enumerate().filter_map(|(j,&pin)|self.drives[j].map(|level|(pin,level)))).collect()}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"boot"|"g0"=>Some(0),_=>None}}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {self.lcd.spi(pins,tx,rx_len)}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {Some(self.lcd.display())}
    fn display_version(&self)->u64 {self.lcd.version()}
    fn display_frames(&self)->u64 {self.lcd.panel.frames}
    fn report(&self)->String {format!("[cardputer] {} selector={:?}",self.lcd.report(),self.selector)}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_switch_requires_its_decoder_row_and_releases_on_key_up() {
        let mut b=Cardputer::new();assert!(!b.uses_gpio_waveform(),"static decoder must not force instruction-sized scheduling");
        assert!(!b.matrix_key_at(0,4,0,true));assert!(!b.matrix_key_at(0,0,14,true));
        // Top-left is Y7/G13; Enter is Y1/G7, per schematic page 2.
        for (row,col,selected,pin) in [(0,0,7,13),(2,13,1,7),(3,0,4,13),(3,13,0,7)] {
            b.selector=Some(selected);assert!(b.matrix_key_at(10,row,col,true));assert_eq!(b.take_edges(),[BoardEdge{cycle:10,pin,level:false}]);
            b.selector=Some(selected^1);b.scan();assert_eq!(b.take_edges(),[BoardEdge{cycle:10,pin,level:true}]);
            b.matrix_key_at(11,row,col,false);assert_eq!(b.released_inputs(),[pin]);assert!(b.take_edges().is_empty());
        }
    }
    #[test]
    fn scan_changes_coalesce_without_releasing_a_new_drive() {
        let mut b=Cardputer::new();b.selector=Some(1);b.matrix_key_at(0,2,13,true);b.take_edges();
        b.matrix_key_at(1,2,13,false);b.matrix_key_at(2,2,13,true);
        assert_eq!(b.take_edges(),[BoardEdge{cycle:2,pin:7,level:false}]);assert!(b.released_inputs().is_empty());
        b.matrix_key_at(3,3,12,true);assert!(b.take_edges().is_empty());
        b.selector=Some(4);b.scan();assert!(b.take_edges().is_empty(),"two held keys on the same input keep selected LOW");
        b.matrix_key_at(4,3,12,false);assert_eq!(b.take_edges(),[BoardEdge{cycle:4,pin:7,level:true}]);
        b.selector=None;b.scan();assert_eq!(b.released_inputs(),[7]);
    }
}
