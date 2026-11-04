//! M5Stack S012, Sch_M5_C6_Lora v0.2.3 (2025-12-02), pages 1–2.
//! PI4IOE5V6408 at 0x43 controls SX1262 reset and FM8625H switch supply.
//! The documented Arduino RadioLib example drives P5/P6/P7 high. Digital
//! packets require the receive LNA enabled; gain, RF loss and range are absent.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,pi4ioe5v6408::{Pi4ioe5v6408,Pi4ioState}};
use esp_soc::{BoardModel,board::{BoardEdge,VirtualCycle,SpiPins},pins::PinRoutes};
use esp_soc::devices::{i2c_bitbang::I2cBitBang,sx1262::{Sx1262,LoRaInput}};

pub struct StampC6LoRa {io:Arc<Mutex<Pi4ioState>>,i2c:I2cBitBang,radio:Sx1262,cycle:u64,levels:[bool;5],edges:Vec<BoardEdge>,selected:bool,reset_edges:u64,gpio_events:u64}
impl StampC6LoRa {
    pub fn new()->Self {
        let mut s=Self{io:Arc::new(Mutex::new(Pi4ioState::default())),i2c:I2cBitBang::new(),radio:Sx1262::new(crate::periph::CPU_HZ),cycle:0,levels:[true,true,false,true,false],edges:Vec::new(),selected:false,reset_edges:0,gpio_events:0};s.sync();s
    }
    fn sync(&mut self) {
        let io=self.io.lock().expect("Stamp C6LoRa IO mutex poisoned");
        let reset=io.drive(7)==Some(true);let antenna=io.drive(6)==Some(true);
        // Preserve a low pulse even if several I2C writes precede observation.
        if self.reset_edges!=io.low_edges[7] {self.radio.controls(true,false,antenna);self.reset_edges=io.low_edges[7];}
        // In this explicit-packet subset a disabled LNA cannot complete an RX.
        // Temporarily removing the receive path cancels pending RX only; it
        // neither resets the transceiver nor disables a pending TX event.
        if io.drive(5)!=Some(true) {self.radio.controls(true,reset,false);}
        self.radio.controls(true,reset,antenna);
        for (i,(pin,level)) in [(3,io.irq_high()),(19,self.radio.busy()),(7,self.radio.irq())].into_iter().enumerate() {
            if self.levels[i+2]!=level {self.levels[i+2]=level;self.edges.push(BoardEdge{cycle:self.cycle,pin,level});}
        }
    }
    fn pins(&mut self,sda:bool,scl:bool,selected:bool) {
        let sda=self.i2c.drive(scl,sda);
        for (i,(pin,level)) in [(10,sda),(8,scl)].into_iter().enumerate() {if self.levels[i]!=level {self.levels[i]=level;self.edges.push(BoardEdge{cycle:self.cycle,pin,level});}}
        self.sync();self.selected=selected;self.radio.select(selected);self.sync();
    }
}
impl Default for StampC6LoRa {fn default()->Self {Self::new()}}
impl BoardModel for StampC6LoRa {
    fn name(&self)->&'static str {"stamp-c6lora"}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_output_at(&mut self,_cycle:VirtualCycle,changes:&[(u8,bool)],_enabled:u64,_output:u64) {self.gpio_events+=changes.len() as u64;}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        self.cycle=cycle;self.radio.advance_to(cycle);
        self.pins(routes.software_output(10)!=Some(false),routes.software_output(8)!=Some(false),routes.software_output(23)==Some(false));
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x43,Box::new(Pi4ioe5v6408::new(self.io.clone())));
        self.i2c.controller_devices(Some((10,8))).into_iter().map(|(a,d)|(0,a,d)).collect()
    }
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        self.sync();
        if host!=2 || pins.sclk&(1<<20)==0 || pins.mosi&(1<<21)==0 || pins.cs&(1<<23)==0 {return vec![0xff;rx_len];}
        // GPIO NSS supports split transfers (RadioLib); peripheral NSS delimits
        // this whole transaction. Both need the actual routed CS23 pin.
        if !self.selected {self.radio.select(true);}
        let rx=self.radio.transfer(tx,rx_len);
        if !self.selected {self.radio.select(false);}
        self.sync();if rx_len==0 || pins.miso==Some(22) {rx} else {vec![0xff;rx_len]}
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(9,true),(10,self.levels[0]),(8,self.levels[1]),(3,self.levels[2]),(19,self.levels[3]),(7,self.levels[4])]}
    fn advance_to(&mut self,cycle:VirtualCycle) {self.cycle=cycle;self.sync();self.radio.advance_to(cycle);self.sync();}
    fn next_deadline(&self)->Option<VirtualCycle> {self.radio.next_deadline()}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.sync();std::mem::take(&mut self.edges)}
    fn lora_receive_at(&mut self,cycle:VirtualCycle,p:&LoRaInput)->bool {
        self.advance_to(cycle);
        if self.io.lock().expect("Stamp C6LoRa IO mutex poisoned").drive(5)!=Some(true) {return false;}
        self.radio.receive(&p.config,&p.payload,p.rssi_dbm,p.snr_quarter_db,p.crc_error)
    }
    fn lora_snapshot(&self)->Option<String> {Some(self.radio.snapshot())}
    fn report(&self)->String {let io=self.io.lock().expect("Stamp C6LoRa IO mutex poisoned");format!("[stamp-c6lora] SX1262 commands={} tx={} rx={} rejected={} P5_LNA={:?} P6_ANT={:?} P7_NRST={:?}; digital-packet subset, no RF/analog gain/range",self.radio.stats.commands,self.radio.stats.tx,self.radio.stats.rx,self.radio.stats.rejected,io.drive(5),io.drive(6),io.drive(7))}
}

#[cfg(test)]
mod tests {
    use super::*;
    use emu_core::Bus;
    fn write(d:&mut dyn I2cDevice,r:u8,v:u8) {assert!(d.start(false));assert!(d.write(r));assert!(d.write(v));d.stop();}
    fn route()->SpiPins {SpiPins{sclk:1<<20,mosi:1<<21,miso:Some(22),cs:1<<23}}
    fn cmd(b:&mut StampC6LoRa,bytes:&[u8])->Vec<u8> {let r=b.spi_transfer_pins(2,route(),bytes,bytes.len());b.advance_to(b.cycle+crate::periph::CPU_HZ/100);r}
    fn ready()->(StampC6LoRa,Box<dyn I2cDevice>) {
        let mut b=StampC6LoRa::new();let (_,a,mut d)=b.i2c_devices().pop().unwrap();assert_eq!(a,0x43);assert_eq!(d.pins(),Some((10,8)));
        write(d.as_mut(),3,0xe0);write(d.as_mut(),7,0x1f);write(d.as_mut(),5,0xe0);b.advance_to(0);b.advance_to(crate::periph::CPU_HZ/100);(b,d)
    }
    #[test]
    fn registration_discrete_flash_identity_and_gpio_mux_are_board_specific() {
        assert_eq!(super::super::make_board("m5stack-s012").unwrap().name(),"stamp-c6lora");
        let mut m=crate::machine([0;6],16<<20);let before=m.bus.periph.efuse.read(0x50);
        m.bus.board=super::super::make_board("stamp-c6lora").unwrap();m.bus.attach_board_devices();assert_eq!(m.bus.periph.efuse.read(0x50)&(7<<24),0);
        let mut other=crate::machine([0;6],4<<20);other.bus.board=super::super::make_board("nanoc6").unwrap();other.bus.attach_board_devices();assert_eq!(other.bus.periph.efuse.read(0x50),before);
        // ESP-IDF v5.5.1 C6 gpio_reg.h OUT/ENABLE and FUNC23_OUT_SEL;
        // io_mux_reg.h IO_MUX_GPIO23_REG = base+0x60, MCU_SEL=1 for GPIO.
        let mut b=StampC6LoRa::new();m.bus.periph.gpio.enable=1<<23;m.bus.periph.gpio.out=0;
        b.gpio_waveform_at(0,esp_soc::pins::ChipPins::C6.routes(&m.bus.periph.gpio,&m.bus.periph.io_mux));assert!(!b.selected);
        m.bus.write32(0x6009_0060,1<<12).unwrap();m.bus.write32(0x6009_15b0,128).unwrap();
        b.gpio_waveform_at(1,esp_soc::pins::ChipPins::C6.routes(&m.bus.periph.gpio,&m.bus.periph.io_mux));assert!(b.selected);
        m.bus.write32(0x6009_0060,0).unwrap();b.gpio_waveform_at(2,esp_soc::pins::ChipPins::C6.routes(&m.bus.periph.gpio,&m.bus.periph.io_mux));assert!(!b.selected);
    }
    #[test]
    fn reset_high_z_wrong_routes_and_nss_boundaries() {
        let mut cold=StampC6LoRa::new();assert_eq!(cmd(&mut cold,&[0x1d,3,0x20,0,0,0,0,0,0,0]),[255;10]);assert!(cold.display().is_none());assert!(cold.leds().is_none());assert!(cold.named_pin("button").is_none());
        let (mut b,mut d)=ready();let read=[0x1d,3,0x20,0,0,0,0,0,0,0];assert_eq!(&cmd(&mut b,&read)[4..],b"SX1261");
        for p in [SpiPins{sclk:1<<19,..route()},SpiPins{mosi:1<<22,..route()},SpiPins{cs:1<<24,..route()},SpiPins{miso:Some(21),..route()}] {assert_eq!(b.spi_transfer_pins(2,p,&read,read.len()),[255;10]);}
        b.pins(true,true,true);assert_eq!(b.spi_transfer_pins(2,route(),&read[..4],4).len(),4);assert_eq!(b.spi_transfer_pins(2,route(),&[0;6],6),b"SX1261");b.pins(true,true,false);
        write(d.as_mut(),7,0x9f);b.advance_to(b.cycle);assert_eq!(cmd(&mut b,&read),[255;10],"high-Z reset is not a driven high");
        write(d.as_mut(),7,0x1f);b.advance_to(b.cycle);assert!(b.input_levels().contains(&(19,true)));b.advance_to(b.cycle+crate::periph::CPU_HZ/100);assert_eq!(&cmd(&mut b,&read)[4..],b"SX1261");
        write(d.as_mut(),5,0x60);write(d.as_mut(),5,0xe0);b.advance_to(b.cycle);assert!(b.input_levels().contains(&(19,true)),"short reset pulse remains observable");
    }
    #[test]
    fn guest_commands_schedule_tx_rx_and_physical_dio1_irq() {
        let (mut b,mut d)=ready();
        for c in [&[0x8a,1][..],&[0x86,0x36,0x40,0,0],&[0x8b,7,4,1,0],&[0x8c,0,8,0,3,1,0],&[0x08,0,0x43,0,0x43,0,0,0,0],&[0x0e,0,b'a',b'b',b'c'],&[0x83,0,0,0]] {cmd(&mut b,c);}
        b.advance_to(b.cycle+crate::periph::CPU_HZ);assert_eq!(b.radio.stats.tx,1);assert!(b.input_levels().contains(&(7,true)));assert!(b.lora_snapshot().unwrap().contains("616263"));
        cmd(&mut b,&[0x02,0xff,0xff]);assert!(b.input_levels().contains(&(7,false)));cmd(&mut b,&[0x82,0xff,0xff,0xff]);
        let mut p=LoRaInput{config:b.radio.config().unwrap(),payload:b"ACK".to_vec(),rssi_dbm:-62,snr_quarter_db:28,crc_error:false};
        p.config.frequency+=1;assert!(!b.lora_receive_at(b.cycle,&p));p.config.frequency-=1;
        write(d.as_mut(),5,0xc0);assert!(!b.lora_receive_at(b.cycle,&p),"disabled receive LNA");write(d.as_mut(),5,0xa0);assert!(!b.lora_receive_at(b.cycle,&p),"unpowered antenna switch");write(d.as_mut(),5,0xe0);
        assert!(b.lora_receive_at(b.cycle,&p));assert_eq!(b.radio.stats.rx,0,"acceptance is not delivery");b.advance_to(b.cycle+crate::periph::CPU_HZ);assert_eq!(b.radio.stats.rx,1);assert!(b.input_levels().contains(&(7,true)));
        assert_eq!(&cmd(&mut b,&[0x1e,0,0,0,0,0])[3..],b"ACK");
        cmd(&mut b,&[0x02,0xff,0xff]);assert!(b.lora_receive_at(b.cycle,&p));write(d.as_mut(),5,0xc0);b.advance_to(b.cycle+1);write(d.as_mut(),5,0xe0);b.advance_to(b.cycle+crate::periph::CPU_HZ);assert_eq!(b.radio.stats.rx,1,"LNA loss cancels an in-flight peer packet; re-enable does not replay it");
        write(d.as_mut(),5,0x60);b.advance_to(b.cycle);assert!(b.input_levels().contains(&(7,false)));assert!(!b.lora_receive_at(b.cycle,&p));
    }
}
