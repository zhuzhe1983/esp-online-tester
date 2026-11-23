//! M5Stack StackChan K151: the CoreS3 host plus the PY32/Si12T/SCSCL robot base.
use esp_periph::i2c::I2cDevice;
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,SpiPins};
use esp_soc::uart::{UartRoute,UartInput};
use super::stackchan_host::CoreS3Host;
use super::stackchan_base::StackChanBase;

pub struct StackChan {host:CoreS3Host,base:StackChanBase}
impl StackChan {
    pub fn new()->Self {
        // Explicit USB-powered base profile; VM_EN remains guest controlled.
        let mut base=StackChanBase::new();base.set_powered(true);
        Self{host:CoreS3Host::new(true),base}
    }
}
impl Default for StackChan {fn default()->Self {Self::new()}}
impl BoardModel for StackChan {
    fn name(&self)->&'static str {"stackchan"}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {self.host.gpio_changes(changes);}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {self.host.gpio_output_at(cycle,changes,enabled,output);}
    fn gpio_events(&self)->u64 {self.host.gpio_events()}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {self.host.spi_transfer_pins(host,pins,tx,rx_len)}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {self.host.i2c_devices(self.base.i2c_devices())}
    fn advance_to(&mut self,cycle:VirtualCycle) {self.host.advance_to(cycle);self.base.advance_to(cycle);}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.host.take_edges()}
    fn input_levels(&self)->Vec<(u8,bool)> {self.host.input_levels()}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"boot"=>Some(0),_=>None}}
    fn button_at(&mut self,cycle:VirtualCycle,id:&str,pressed:bool)->bool {self.advance_to(cycle);self.base.button(id,pressed)}
    fn touch_at(&mut self,cycle:VirtualCycle,x:u16,y:u16,down:bool) {self.host.touch_at(cycle,x,y,down);}
    fn touch(&mut self,x:u16,y:u16,down:bool) {self.host.touch(x,y,down);}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {self.host.display()}
    fn display_version(&self)->u64 {self.host.display_version()}
    fn display_frames(&self)->u64 {self.host.display_frames()}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {self.host.gram()}
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {self.host.set_imu_sample(accel_g,gyro_dps)}
    fn uses_uart_pins(&self)->bool {true}
    fn uart_tx(&mut self,cycle:VirtualCycle,route:UartRoute,byte:u8) {self.base.uart_tx(cycle,route,byte);}
    fn uart_rx(&mut self,cycle:VirtualCycle)->Vec<UartInput> {self.base.uart_rx(cycle)}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some(self.base.leds())}
    fn motion_snapshot(&self)->Option<String> {Some(self.base.motion_snapshot())}
    fn report(&self)->String {format!("[stackchan] ILI9342C frames={} pixels={} FT6336U=single-contact Si12T=3-pad PY32=12-RGB servos={} camera/audio/NFC/RTC=unimplemented",self.host.display_frames(),self.host.pixels_written(),self.base.motion_snapshot())}
}

#[cfg(test)] mod tests {
    use super::*;
    fn pins()->SpiPins {SpiPins{sclk:1<<36,mosi:1<<37,cs:1<<3,..Default::default()}}
    fn wr(d:&mut dyn I2cDevice,reg:u8,values:&[u8]) {assert!(d.start(false));assert!(d.write(reg));for v in values {assert!(d.write(*v));}d.stop();}
    fn rd(d:&mut dyn I2cDevice,reg:u8,n:usize)->Vec<u8> {wr(d,reg,&[]);assert!(d.start(true));(0..n).map(|_|d.read()).collect()}
    fn cmd(b:&mut StackChan,c:u8,data:&[u8]) {b.gpio_changes(&[(35,false)]);b.spi_transfer_pins(2,pins(),&[c],0);b.gpio_changes(&[(35,true)]);b.spi_transfer_pins(2,pins(),data,0);}
    #[test] fn display_requires_its_physical_route_reset_and_backlight() {
        let mut b=StackChan::new();let mut devices=b.i2c_devices();let aw=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x58).unwrap().2.as_mut();
        assert_eq!(b.spi_transfer_pins(2,pins(),&[4],3),[0xff;3]);wr(aw,3,&[2]);
        assert_eq!(b.spi_transfer_pins(2,pins(),&[4],3),[0xe3,0,0]);assert_eq!(b.spi_transfer_pins(2,SpiPins{cs:1<<4,..pins()},&[4],3),[0xff;3]);
        cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x2a,&[0,0,0,1]);cmd(&mut b,0x2b,&[0,0,0,0]);cmd(&mut b,0x2c,&[0xf8,0,7,0xe0]);assert!(b.display().unwrap().2.iter().all(|p|*p==0));
        let axp=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x34).unwrap().2.as_mut();wr(axp,0x90,&[0x80]);wr(axp,0x99,&[28]);assert_eq!(&b.display().unwrap().2[..2],&[0xf800,0x07e0]);wr(axp,0x90,&[0]);assert!(b.display().unwrap().2.iter().all(|p|*p==0));
        let aw=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x58).unwrap().2.as_mut();wr(aw,3,&[0]);b.advance_to(1);assert_eq!(b.spi_transfer_pins(2,pins(),&[4],3),[0xff;3]);assert!(!b.host.panel_on());
    }
    #[test] fn ft6336_reaches_guest_registers_through_aw_interrupt_and_resets() {
        let mut b=StackChan::new();let mut devices=b.i2c_devices();assert!(devices.iter().all(|(_,_,d)|d.pins()==Some((12,11))));
        let aw=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x58).unwrap().2.as_mut();wr(aw,2,&[1]);wr(aw,5,&[4]);b.touch_at(10,123,234,true);assert!(b.input_levels().contains(&(21,false)));
        let touch=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x38).unwrap().2.as_mut();assert_eq!(rd(touch,2,5),[1,0x80,123,0,234]);assert_eq!(rd(touch,2,5),[1,0x80,123,0,234]);b.touch_at(11,123,234,false);assert_eq!(rd(touch,2,1),[0]);
        let aw=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x58).unwrap().2.as_mut();assert_eq!(rd(aw,1,1)[0]&4,4);b.advance_to(12);assert!(b.input_levels().contains(&(21,true)));wr(aw,2,&[0]);b.advance_to(13);
        let touch=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x38).unwrap().2.as_mut();assert!(!touch.start(false));
    }
    #[test] fn board_registration_does_not_alias_other_products() {let b=super::super::make_board("m5stack-k151").unwrap();assert_eq!(b.name(),"stackchan");assert!(b.named_pin("power").is_none());assert!(b.motion_snapshot().is_some());assert!(super::super::make_board("none").unwrap().motion_snapshot().is_none());}
}
