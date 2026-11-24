//! C5 HP I2C0: 60004000, PCR20/24, signal SDA47/SCL46, source56.
//! Byte scheduling at an explicit crystal rate; 7-bit master/FIFO subset.
//! No slave, multi-master collision, stretched clock or timeout simulation.
use crate::{bus::{C5Bus,PINS},i2c_regs};
use esp_periph::{Device,RegRam,i2c::{I2c,I2cDevice,INT_END_DETECT,INT_TRANS_COMPLETE,INT_NACK}};
const INTERRUPTS:u32=INT_END_DETECT|INT_TRANS_COMPLETE|INT_NACK;
pub(crate) struct C5I2c{controller:I2c,config:u32,clock:u32,last:u64,remainder:u64,pub deadline:Option<u64>}
impl C5I2c{
    pub fn new(now:u64)->Self{let mut s=Self{controller:I2c::new(),config:0,clock:0,last:now,remainder:0,deadline:None};s.reset();s}
    fn reset(&mut self){
        // Keep the physically attached device objects. Controller reset is not
        // a power cycle of the RTC/codec/expander. No synthetic I2C STOP is sent.
        self.controller.write(4,1<<10);self.controller.write(0x18,3<<12);
        self.controller.regs=RegRam::new();self.controller.int_raw=2;self.controller.int_ena=0;
        self.controller.set_external_clock_config(None);self.controller.set_pins(None);
        for off in (0..=0xf8).step_by(4){if let Some((reset,_))=i2c_regs::config(off){
            if (0x58..=0x74).contains(&off){self.controller.write(off,reset);}else{self.controller.regs.write(off,reset);}
        }}self.deadline=None;
    }
    pub fn attach(&mut self,address:u8,device:Box<dyn I2cDevice>){self.controller.attach(address,device);}
    pub fn clear_devices(&mut self){self.controller.clear_devices();}
    pub fn contains(a:u32)->bool{matches!(a,0x6000_4000..=0x6000_41fc|0x6009_6020..=0x6009_6024)}
    fn enabled(&self)->bool{self.config&3==1&&self.clock&((1<<22)|(1<<20))==1<<22&&self.controller.regs.read(0x54)&(1<<21)!=0}
    fn plan(&mut self,now:u64){self.deadline=self.controller.next_deadline().map(|n|now.saturating_add(n.saturating_mul(3).saturating_sub(self.remainder)));}
    pub fn sync(&mut self,bus:&mut C5Bus){
        let delta=bus.cycles.wrapping_sub(self.last)+self.remainder;self.last=bus.cycles;self.remainder=delta%3;
        bus.board.advance_to(bus.cycles);if self.enabled(){self.controller.tick(delta/3);}self.plan(bus.cycles);
        bus.irq_dirty|=esp_soc::gpio::drain_board_inputs(&mut *bus.board,&mut bus.gpio,&mut bus.gpio_events,bus.cycles);
    }
    pub fn irq(&self)->bool{self.config&3==1&&self.controller.irq()}
    fn cancel(&mut self){let ctr=self.controller.regs.read(4);self.controller.write(4,ctr|(1<<10));self.deadline=None;}
    fn register(off:u32)->Option<(u32,u32)>{
        // FIFO RAM aliases and slave SCL stretching/SP-config are unimplemented.
        if matches!(off,0x80|0x84|0x100..=0x1fc){None}else{i2c_regs::config(off)}
    }
    pub fn read(&mut self,a:u32,bus:&mut C5Bus)->Option<u32>{
        self.sync(bus);match a{0x6009_6020=>Some(self.config),0x6009_6024=>Some(self.clock),_=>{
            if self.config&3!=1{return None;}let off=a&0xfff;Self::register(off)?;
            // Follow the published C5 ACK polarity, not the older C3/S3
            // controller's measured inverse RESP_REC convention.
            Some(if off==8{(self.controller.read(off)^1)|0xc000}else if matches!(off,0xf8|0x24){self.controller.regs.read(off)}else{self.controller.read(off)})
        }}
    }
    pub fn write(&mut self,a:u32,v:u32,bus:&mut C5Bus)->bool{
        self.sync(bus);match a{
            0x6009_6020=>{if v&2!=0{self.reset();}self.config=v&3;if self.config&3!=1{self.cancel();}},
            0x6009_6024=>{let value=v&0x5f_ffff;if value!=self.clock{self.cancel();}self.clock=value;},
            _=>{
                if self.config&3!=1{return false;}let off=a&0xfff;let Some((_,mask))=Self::register(off)else{return false;};let mut value=v&mask;
                // The register header only names RX HRO here. C5 i2c_ll.h
                // write_txfifo() explicitly writes each TX byte through DATA.
                if off==0x1c{if(self.controller.read(8)>>18)&63>=32{return false;}value=v&255;}
                if off==0x28&&value&!INTERRUPTS!=0{return false;}
                if off==0x18&&value&(1<<10)!=0{return false;} // non-FIFO mode
                if off==0x20{self.controller.int_raw&=!value;}
                else if off==0xf8{self.controller.regs.write(off,value);}
                else if off==4{
                    if value&(1<<5)!=0&&!self.controller.is_active(){
                        if !self.enabled()||value&(1<<4)==0||value&((1<<6)|(1<<7)|(1<<12)|(1<<13))!=0{return false;}
                        let mut tx=0;let mut rx=0;let mut end=false;
                        for n in 0..8{let cmd=self.controller.read(0x58+4*n);match(cmd>>11)&7{1=>tx+=cmd&255,3=>rx+=cmd&255,2|4=>{end=true;break;},6=>{},_=>return false,}}
                        if !end||tx>(self.controller.read(8)>>18)&63||rx>32{return false;}
                        let r=PINS.routes(&bus.gpio,&bus.io_mux);self.controller.set_pins(r.i2c_pin(47).zip(r.i2c_pin(46)));
                        if self.controller.set_external_clock_hz(bus.xtal_hz,((self.clock>>12)&255)as u8,(self.clock&63)as u8,((self.clock>>6)&63)as u8).is_err(){return false;}
                    }
                    value&=!(1<<11);self.controller.write(off,value); // synchronizer is functional at the MMIO boundary
                }else{self.controller.write(off,value);if off==0x54&&!self.enabled(){self.cancel();}}
            }
        }self.plan(bus.cycles);true
    }
}
