//! Open-drain software I2C slave adapter. The same devices can also be attached to
//! a SoC's hardware I2C master; register state is shared rather than duplicated.
//! START/STOP and eight data clocks plus ACK follow NXP UM10204 rev7.0 section3.1.
//! Ideal pull-ups, no electrical contention or clock stretching timing model.
use esp_periph::i2c::I2cDevice;
use std::{cell::RefCell,rc::Rc};
type Shared=Rc<RefCell<Box<dyn I2cDevice>>>;
struct Adapter(Shared, Option<(u8,u8)>);
impl I2cDevice for Adapter {
    fn pins(&self)->Option<(u8,u8)> {self.1}
    fn start(&mut self,read:bool)->bool {self.0.borrow_mut().start(read)}
    fn write(&mut self,b:u8)->bool {self.0.borrow_mut().write(b)}
    fn read(&mut self)->u8 {self.0.borrow_mut().read()}
    fn stop(&mut self) {self.0.borrow_mut().stop();}
}
#[derive(Clone,Copy)]
enum Phase {Idle,Receive{value:u8,bits:u8,address:bool},Ack{low:bool,read_next:bool,clocked:bool},Send{value:u8,bit:u8},MasterAck{seen:bool,ack:bool}}
pub struct I2cBitBang {devices:Vec<(u8,Shared)>,selected:Option<Shared>,phase:Phase,scl:bool,sda:bool,slave_low:bool}
impl Default for I2cBitBang {fn default()->Self {Self::new()}}
impl I2cBitBang {
    pub fn new()->Self {Self{devices:Vec::new(),selected:None,phase:Phase::Idle,scl:true,sda:true,slave_low:false}}
    pub fn clear(&mut self) {self.devices.clear();self.selected=None;self.phase=Phase::Idle;self.slave_low=false;}
    pub fn attach(&mut self,address:u8,device:Box<dyn I2cDevice>)->Box<dyn I2cDevice> {let shared=Rc::new(RefCell::new(device));self.devices.push((address,shared.clone()));Box::new(Adapter(shared,None))}
    pub fn controller_devices(&self,pins:Option<(u8,u8)>)->Vec<(u8,Box<dyn I2cDevice>)> {self.devices.iter().map(|(address,shared)|(*address,Box::new(Adapter(shared.clone(),pins)) as Box<dyn I2cDevice>)).collect()}
    fn next_byte(&mut self) {let value=self.selected.as_ref().map_or(0xff,|d|d.borrow_mut().read());self.phase=Phase::Send{value,bit:0};self.slave_low=value&0x80==0;}
    /// Master pin drives after applying pull-ups (released = HIGH). Return the
    /// resolved SDA level, including a selected slave's data/ACK low drive.
    pub fn drive(&mut self,scl:bool,sda:bool)->bool {
        if self.scl && scl && !self.slave_low && self.sda!=sda {
            if let Some(device)=&self.selected {device.borrow_mut().stop();}
            self.selected=None;self.slave_low=false;
            self.phase=if sda {Phase::Idle} else {Phase::Receive{value:0,bits:0,address:true}};
        } else if !self.scl && scl {
            match self.phase {
                Phase::Receive{value,bits,address} => {
                    let value=(value<<1)|u8::from(sda);
                    if bits==7 {
                        let read=address && value&1!=0;
                        let ack=if address {
                            self.selected=self.devices.iter().find(|(a,_)|*a==value>>1).map(|(_,d)|d.clone());
                            self.selected.as_ref().is_some_and(|d|d.borrow_mut().start(read))
                        } else {self.selected.as_ref().is_some_and(|d|d.borrow_mut().write(value))};
                        self.phase=Phase::Ack{low:ack,read_next:read,clocked:false};
                    } else {self.phase=Phase::Receive{value,bits:bits+1,address};}
                }
                Phase::Ack{low,read_next,..} => self.phase=Phase::Ack{low,read_next,clocked:true},
                Phase::Send{value,bit} => self.phase=if bit==7 {Phase::MasterAck{seen:false,ack:false}} else {Phase::Send{value,bit:bit+1}},
                Phase::MasterAck{..} => self.phase=Phase::MasterAck{seen:true,ack:!sda},
                Phase::Idle => {}
            }
        } else if self.scl && !scl {
            match self.phase {
                Phase::Ack{low,read_next,clocked} => {
                    if !clocked {self.slave_low=low;}
                    else if low && read_next {self.next_byte();}
                    else {self.slave_low=false;self.phase=Phase::Receive{value:0,bits:0,address:false};}
                }
                Phase::Send{value,bit} => self.slave_low=value&(0x80>>bit)==0,
                Phase::MasterAck{seen,ack} => {if seen && ack {self.next_byte();} else {self.slave_low=false;if seen {self.phase=Phase::Idle;}}}
                _ => {}
            }
        }
        self.scl=scl;self.sda=sda;sda&&!self.slave_low
    }
}
#[cfg(test)]
mod tests {
    use super::*;use esp_periph::i2c::Reg8Device;
    fn start(bus:&mut I2cBitBang) {bus.drive(false,true);bus.drive(true,true);bus.drive(true,false);}
    fn stop(bus:&mut I2cBitBang) {bus.drive(false,false);bus.drive(true,false);assert!(bus.drive(true,true));}
    fn send(bus:&mut I2cBitBang,value:u8)->bool {for bit in (0..8).rev(){bus.drive(false,value&(1<<bit)!=0);bus.drive(true,value&(1<<bit)!=0);}bus.drive(false,true);let ack=!bus.drive(true,true);bus.drive(false,true);ack}
    fn read(bus:&mut I2cBitBang,ack:bool)->u8 {let mut value=0;for _ in 0..8 {value=(value<<1)|u8::from(bus.drive(true,true));bus.drive(false,true);}bus.drive(false,!ack);bus.drive(true,!ack);bus.drive(false,!ack);value}
    #[test]
    fn bitbang_and_controller_share_register_state_and_nack_absent_addresses() {
        let mut bus=I2cBitBang::new();let mut hardware=bus.attach(0x32,Box::new(Reg8Device::new("test",&[])));
        start(&mut bus);assert!(!send(&mut bus,0xa0));stop(&mut bus);
        start(&mut bus);assert!(send(&mut bus,0x64));assert!(send(&mut bus,0x20));assert!(send(&mut bus,0xa5));assert!(send(&mut bus,0x5a));stop(&mut bus);
        hardware.start(false);hardware.write(0x20);hardware.start(true);assert_eq!(hardware.read(),0xa5);
        start(&mut bus);assert!(send(&mut bus,0x64));assert!(send(&mut bus,0x20));start(&mut bus);assert!(send(&mut bus,0x65));
        assert_eq!(read(&mut bus,true),0xa5);assert_eq!(read(&mut bus,false),0x5a);stop(&mut bus);
    }
}
