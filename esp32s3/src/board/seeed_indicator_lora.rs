//! Indicator variants with an actual SX1262 digital packet model. Ordinary
//! D1/D1S retain SeeedIndicator with no radio object or additional callbacks.
//! Meshtastic v2.5.11.8e2a3e5 variants/seeed-sensecap-indicator/variant.h and
//! platformio.ini: SPI41/48/47; expander P0=NSS, P1=RST, P2=BUSY, P3=DIO1,
//! IRQ=GPIO42. Seeed ed94f161 components/lora/sx126x_sensecap_board.c confirms
//! the same wires, active-low expander interrupt, and internal DIO2 RF switch.
//! USB-powered digital subset, no board power button or RF propagation model.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
use esp_soc::{board::{BoardModel,BoardEdge,LcdRgbPins,SpiPins,VirtualCycle},devices::sx1262::{Sx1262,LoRaInput},pins::PinRoutes};
use super::{seeed_indicator::{SeeedIndicator,IndicatorPanel},seeed_sensecap::Expander};

#[derive(Clone,Copy)]
pub enum Variant {D1L,D1Pro,Meshtastic}
impl Variant {
    fn name(self)->&'static str {match self {Self::D1L=>"seeed-sensecap-indicator-d1l",Self::D1Pro=>"seeed-sensecap-indicator-d1pro",Self::Meshtastic=>"seeed-sensecap-indicator-meshtastic"}}
}

struct Radio {
    chip:Sx1262,cycle:VirtualCycle,irq_high:bool,edges:Vec<BoardEdge>,rejected_routes:u64,
}
impl Radio {
    fn new()->Self {Self{chip:Sx1262::new(crate::periph::CPU_HZ),cycle:0,irq_high:true,edges:Vec::new(),rejected_routes:0}}
    fn sync(&mut self,io:&Expander) {
        let (powered,levels,driven)={let state=io.state.lock().expect("Indicator expander poisoned");(state.powered,state.levels(),state.driven_high())};
        // The manufacturer's board power-control callback writes no GPIO.
        // This USB profile assumes one ideal powered domain with the expander;
        // the undisclosed supply circuit is not an analog model. P1 must be
        // explicitly driven high before leaving reset; no PMIC is invented.
        self.chip.controls(powered,driven&2!=0,true);
        // P0's undriven input is high in the existing ideal expander model.
        // Virtual Arduino GPIO64 is a port alias, never a SoC GPIO or CS mask.
        self.chip.select(powered&&levels&1==0);
        self.publish(io);
    }
    fn publish(&mut self,io:&Expander) {
        let mut state=io.state.lock().expect("Indicator expander poisoned");
        state.set_input(2,self.chip.busy());state.set_input(3,self.chip.irq());
        let high=state.irq_high();if high!=self.irq_high {self.irq_high=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:42,level:high});}
    }
    fn advance(&mut self,cycle:VirtualCycle,io:&Expander) {self.cycle=cycle;self.chip.advance_to(cycle);self.sync(io);}
}

// Observe the physical output/configuration writes synchronously. Waiting for
// the next board tick could miss an NSS pulse around separate SPI calls. Reads
// preserve the expander's polarity/direction and per-port IRQ-clear behavior.
struct RadioExpander {device:Box<dyn I2cDevice>,io:Expander,radio:Arc<Mutex<Radio>>}
impl RadioExpander {fn sync(&self) {self.radio.lock().expect("Indicator radio poisoned").sync(&self.io);}}
impl I2cDevice for RadioExpander {
    fn pins(&self)->Option<(u8,u8)> {self.device.pins()}
    fn start(&mut self,read:bool)->bool {self.sync();self.device.start(read)}
    fn write(&mut self,b:u8)->bool {let ack=self.device.write(b);self.sync();ack}
    fn read(&mut self)->u8 {self.sync();let value=self.device.read();self.radio.lock().expect("Indicator radio poisoned").publish(&self.io);value}
    fn stop(&mut self) {self.device.stop();self.sync();}
}

pub struct SeeedIndicatorLoRa {variant:Variant,base:SeeedIndicator,io:Expander,radio:Arc<Mutex<Radio>>}
impl SeeedIndicatorLoRa {
    pub fn new(variant:Variant)->Self {
        let base=SeeedIndicator::new(IndicatorPanel::Gx);let io=base.expander();let radio=Arc::new(Mutex::new(Radio::new()));
        radio.lock().expect("Indicator radio poisoned").sync(&io);Self{variant,base,io,radio}
    }
    fn advance_radio(&mut self,cycle:VirtualCycle) {self.radio.lock().expect("Indicator radio poisoned").advance(cycle,&self.io);}
}
impl BoardModel for SeeedIndicatorLoRa {
    fn name(&self)->&'static str {self.variant.name()}
    fn uses_gpio_routes(&self)->bool {self.base.uses_gpio_routes()}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {self.base.gpio_output_at(cycle,changes,enabled,output);self.advance_radio(cycle);}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {self.base.gpio_waveform_at(cycle,routes);self.advance_radio(cycle);}
    fn gpio_events(&self)->u64 {self.base.gpio_events()}
    fn uses_gpio_pwm(&self)->bool {self.base.uses_gpio_pwm()}
    fn gpio_pwm_at(&mut self,cycle:VirtualCycle,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {self.base.gpio_pwm_at(cycle,pwm);}
    fn lcd_rgb_pins(&mut self,pins:LcdRgbPins) {self.base.lcd_rgb_pins(pins);}
    fn lcd_frame(&mut self,w:u32,h:u32,data:&[u8]) {self.base.lcd_frame(w,h,data);}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.base.i2c_devices().into_iter().map(|(bus,address,device)|{
            if address==0x20 {(bus,address,Box::new(RadioExpander{device,io:self.io.clone(),radio:self.radio.clone()}) as Box<dyn I2cDevice>)}else{(bus,address,device)}
        }).collect()
    }
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        let mut radio=self.radio.lock().expect("Indicator radio poisoned");radio.sync(&self.io);
        // Both existing SPI controllers can physically reach these pads; the
        // Seeed IDF BSP uses SPI3 and Meshtastic uses Arduino's SPI2. LCD NSS
        // must be inactive in this bounded single-responder bus subset.
        let lcd_idle=self.io.state.lock().expect("Indicator expander poisoned").levels()&16!=0;
        if !matches!(host,2|3)||pins.sclk&(1<<41)==0||pins.mosi&(1<<48)==0||!lcd_idle {radio.rejected_routes+=1;return vec![0xff;rx_len];}
        let rx=radio.chip.transfer(tx,rx_len);radio.publish(&self.io);
        if rx_len==0||pins.miso==Some(47){rx}else{vec![0xff;rx_len]}
    }
    fn touch(&mut self,x:u16,y:u16,down:bool) {self.base.touch(x,y,down);}
    fn touch_at(&mut self,cycle:VirtualCycle,x:u16,y:u16,down:bool) {self.base.touch_at(cycle,x,y,down);self.advance_radio(cycle);}
    fn input_levels(&self)->Vec<(u8,bool)> {let mut pins=self.base.input_levels();pins.push((42,self.radio.lock().expect("Indicator radio poisoned").irq_high));pins}
    fn named_pin(&self,name:&str)->Option<u8> {self.base.named_pin(name)}
    fn advance_to(&mut self,cycle:VirtualCycle) {self.base.advance_to(cycle);self.advance_radio(cycle);}
    fn next_deadline(&self)->Option<VirtualCycle> {self.radio.lock().expect("Indicator radio poisoned").chip.next_deadline()}
    fn take_edges(&mut self)->Vec<BoardEdge> {let mut radio=self.radio.lock().expect("Indicator radio poisoned");radio.sync(&self.io);std::mem::take(&mut radio.edges)}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {self.base.display()}
    fn display_version(&self)->u64 {self.base.display_version()}
    fn display_frames(&self)->u64 {self.base.display_frames()}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {self.base.gram()}
    fn lora_receive_at(&mut self,cycle:VirtualCycle,p:&LoRaInput)->bool {
        let mut radio=self.radio.lock().expect("Indicator radio poisoned");radio.advance(cycle,&self.io);
        let accepted=radio.chip.receive(&p.config,&p.payload,p.rssi_dbm,p.snr_quarter_db,p.crc_error);radio.publish(&self.io);accepted
    }
    fn lora_snapshot(&self)->Option<String> {Some(self.radio.lock().expect("Indicator radio poisoned").chip.snapshot())}
    fn report(&self)->String {
        let radio=self.radio.lock().expect("Indicator radio poisoned");format!("[{}] {} SX1262 commands={} tx={} rx={} rejected_routes={} SPI=41/48/47 NSS/RST/BUSY/DIO1=P0/1/2/3 IRQ=42; RF/RP2040/sensors absent",self.name(),self.base.rgb_diagnostics(),radio.chip.stats.commands,radio.chip.stats.tx,radio.chip.stats.rx,radio.rejected_routes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(d:&mut dyn I2cDevice,register:u8,data:&[u8]) {assert!(d.start(false));assert!(d.write(register));for &b in data{assert!(d.write(b));}d.stop();}
    fn read(d:&mut dyn I2cDevice,register:u8)->u8 {write(d,register,&[]);assert!(d.start(true));let value=d.read();d.stop();value}
    fn expander(b:&mut SeeedIndicatorLoRa)->Box<dyn I2cDevice> {b.i2c_devices().into_iter().find(|(bus,address,_)|*bus==0&&*address==0x20).unwrap().2}
    fn pins()->SpiPins {SpiPins{sclk:1<<41,mosi:1<<48,miso:Some(47),..Default::default()}}
    fn ready()->(SeeedIndicatorLoRa,Box<dyn I2cDevice>) {
        let mut b=SeeedIndicatorLoRa::new(Variant::Meshtastic);let mut d=expander(&mut b);
        write(d.as_mut(),2,&[0xff,0xff]);write(d.as_mut(),6,&[0xfc,0xff]);assert_ne!(read(d.as_mut(),0)&4,0);
        b.advance_to(crate::periph::CPU_HZ*4/1000);assert_eq!(read(d.as_mut(),0)&12,0);b.take_edges();(b,d)
    }
    fn finish_busy(b:&mut SeeedIndicatorLoRa) {if let Some(at)=b.next_deadline(){b.advance_to(at);}}
    fn cmd(b:&mut SeeedIndicatorLoRa,d:&mut dyn I2cDevice,data:&[u8])->Vec<u8> {
        write(d,2,&[0xfe]);let rx=b.spi_transfer_pins(2,pins(),data,data.len());write(d,2,&[0xff]);finish_busy(b);rx
    }
    fn configure(b:&mut SeeedIndicatorLoRa,d:&mut dyn I2cDevice) {
        for c in [vec![0x80,0],vec![0x8a,1],vec![0x86,0x36,0x40,0,0],vec![0x8b,12,3,4,1],vec![0x8c,0,10,0,5,1,0],vec![0x0d,7,0x40,0x34,0x44],vec![0x08,3,0xff,3,0xff,0,0,0,0]] {cmd(b,d,&c);}
    }
    #[test]
    fn radio_identity_needs_actual_expander_nss_and_each_spi_route() {
        let (mut b,mut d)=ready();let identity=[0x1d,3,0x20,0,0,0,0,0,0,0];
        assert_eq!(b.spi_transfer_pins(2,SpiPins{cs:u64::MAX,..pins()},&identity,10),[255;10],"a SoC CS mask cannot replace P0");
        for broken in [SpiPins{sclk:1<<40,..pins()},SpiPins{mosi:1<<47,..pins()},SpiPins{miso:Some(46),..pins()}] {
            write(d.as_mut(),2,&[0xfe]);assert_eq!(b.spi_transfer_pins(2,broken,&identity,10),[255;10]);write(d.as_mut(),2,&[0xff]);
        }
        assert_eq!(&cmd(&mut b,d.as_mut(),&identity)[4..],b"SX1261");
        write(d.as_mut(),2,&[0xfe]);assert_eq!(&b.spi_transfer_pins(3,pins(),&identity,10)[4..],b"SX1261");write(d.as_mut(),2,&[0xff]);
    }
    #[test]
    fn nss_edges_from_i2c_preserve_split_reads_without_board_ticks() {
        let (mut b,mut d)=ready();write(d.as_mut(),2,&[0xfe]);
        b.spi_transfer_pins(2,pins(),&[0x1d,3],2);let reply=b.spi_transfer_pins(2,pins(),&[0x20,0,0,0,0,0,0,0],8);assert_eq!(&reply[2..],b"SX1261");
        write(d.as_mut(),2,&[0xff]);write(d.as_mut(),2,&[0xfe]);assert_ne!(b.spi_transfer_pins(2,pins(),&[0x20,0,0,0,0,0,0,0],8)[2..],*b"SX1261");write(d.as_mut(),2,&[0xff]);
    }
    #[test]
    fn reset_busy_and_common_supply_gate_are_observed_through_port_registers() {
        let (mut b,mut d)=ready();cmd(&mut b,d.as_mut(),&[0x0d,7,0x40,0xaa]);assert_eq!(cmd(&mut b,d.as_mut(),&[0x1d,7,0x40,0,0])[4],0xaa);
        write(d.as_mut(),2,&[0xfd]);assert_ne!(read(d.as_mut(),0)&4,0);assert_eq!(cmd(&mut b,d.as_mut(),&[0x1d,3,0x20,0,0,0,0,0,0,0]),[255;10]);
        finish_busy(&mut b);assert_eq!(read(d.as_mut(),0)&4,0);assert_eq!(cmd(&mut b,d.as_mut(),&[0x1d,7,0x40,0,0])[4],0x14);
        b.io.state.lock().unwrap().powered=false;b.advance_to(crate::periph::CPU_HZ);assert!(!d.start(true));assert!(b.lora_snapshot().unwrap().contains("\"powered\":false"));
        assert_eq!(b.spi_transfer_pins(2,pins(),&[0xc0,0],2),[255;2]);assert!(!b.input_levels().contains(&(42,false)));
    }
    #[test]
    fn tx_completion_reaches_port_dio1_and_gpio42_then_only_read_port_clears_irq() {
        let (mut b,mut d)=ready();configure(&mut b,d.as_mut());cmd(&mut b,d.as_mut(),&[0x0e,0,b'H',b'e',b'l',b'l',b'o']);
        write(d.as_mut(),2,&[0xfe]);b.spi_transfer_pins(2,pins(),&[0x83,0,0,0],4);write(d.as_mut(),2,&[0xff]);
        while b.radio.lock().unwrap().chip.busy() {let at=b.next_deadline().unwrap();b.advance_to(at);}
        assert_eq!(b.radio.lock().unwrap().chip.stats.tx,0);assert_eq!(read(d.as_mut(),0)&12,0);b.take_edges();assert!(b.input_levels().contains(&(42,true)));
        let end=b.next_deadline().unwrap();b.advance_to(end-1);assert_eq!(b.radio.lock().unwrap().chip.stats.tx,0);assert!(b.input_levels().contains(&(42,true)));b.advance_to(end);
        assert!(b.take_edges().iter().any(|e|e.pin==42&&!e.level));assert!(b.input_levels().contains(&(42,false)));
        read(d.as_mut(),1);assert!(b.input_levels().contains(&(42,false)),"reading the other port cannot acknowledge P3");
        assert_ne!(read(d.as_mut(),0)&8,0);assert!(b.input_levels().contains(&(42,true)));assert_eq!(&cmd(&mut b,d.as_mut(),&[0x12,0,0,0])[2..],&[0,1]);
        let packet=b.radio.lock().unwrap().chip.take_tx().unwrap();assert_eq!(packet.payload,b"Hello");assert_eq!(packet.config.frequency,0x36400000);assert_eq!(b.radio.lock().unwrap().chip.stats.rx,0);
    }
    #[test]
    fn explicit_receive_requires_matching_radio_config_and_delivers_guest_fifo() {
        let (mut b,mut d)=ready();configure(&mut b,d.as_mut());cmd(&mut b,d.as_mut(),&[0x82,0xff,0xff,0xff]);
        let config=b.radio.lock().unwrap().chip.config().unwrap();let mut input=LoRaInput{config,payload:b"peer".to_vec(),rssi_dbm:-70,snr_quarter_db:24,crc_error:false};input.config.frequency+=1;
        let now=b.radio.lock().unwrap().cycle;assert!(!b.lora_receive_at(now,&input));input.config.frequency-=1;assert!(b.lora_receive_at(now,&input));assert!(!b.lora_receive_at(now,&input));
        let end=b.next_deadline().unwrap();b.advance_to(end-1);assert_eq!(b.radio.lock().unwrap().chip.stats.rx,0);b.advance_to(end);assert_eq!(b.radio.lock().unwrap().chip.stats.rx,1);
        assert_ne!(read(d.as_mut(),0)&8,0);assert_eq!(&cmd(&mut b,d.as_mut(),&[0x1e,0,0,0,0,0,0])[3..],b"peer");
        assert!(b.lora_snapshot().unwrap().contains("\"physicalRf\":false"));assert_eq!(b.radio.lock().unwrap().chip.stats.tx,0);
    }
    #[test]
    fn shared_lcd_selection_and_expander_input_direction_do_not_select_radio() {
        let (mut b,mut d)=ready();write(d.as_mut(),6,&[0xfd,0xff]);write(d.as_mut(),2,&[0xfe]);assert_eq!(b.spi_transfer_pins(2,pins(),&[0xc0,0],2),[255;2]);
        write(d.as_mut(),6,&[0xec,0xff]);write(d.as_mut(),2,&[0xee]);assert_eq!(b.spi_transfer_pins(2,pins(),&[0xc0,0],2),[255;2],"LCD and radio both selected is outside single-responder subset");
    }
}
