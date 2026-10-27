//! U136 Unit DLight, BH1750FVI fixed ADDR=L (0x23), ideal illuminance input.
//! ROHM BH1750FVI Technical Note 2011.11 Rev.D pp.2,4-7,10-12: opcodes,
//! conversion maximums, big-endian data, gain/MTreg, one opcode per STOP.
//! Sources and electrical wiring are pinned in the U136 evidence receipt.
//! No optical spectrum/angle, sensor variation, thermal noise or analog supply model.

pub struct Bh1750 {
    pub lux:f64,pub connected:bool,powered:bool,on:bool,cycle:u64,hz:u64,
    mode:u8,mt:u8,period:u64,deadline:Option<u64>,energy:f64,
    data:u16,readout:[u8;2],at:usize,written:bool,
}
impl Bh1750 {
    pub fn new(hz:u64)->Self {Self{lux:1000.0,connected:false,powered:false,on:false,cycle:0,hz,mode:0,mt:69,period:0,deadline:None,energy:0.0,data:0,readout:[0;2],at:0,written:false}}
    pub fn valid(lux:f64)->bool {lux.is_finite()&&(0.0..=100_000.0).contains(&lux)}
    pub fn update(&mut self,cycle:u64,connected:bool,lux:f64)->bool {
        if !Self::valid(lux) {return false;}self.advance_to(cycle);self.connected=connected;self.lux=lux;true
    }
    pub fn set_power(&mut self,supply:bool) {
        let powered=supply&&self.connected;if powered==self.powered {return;}
        // Ideal supply/DVI reset. The physical RC rise time and brownout are not modeled.
        self.powered=powered;self.on=false;self.mt=69;self.data=0;self.deadline=None;self.energy=0.0;self.written=false;
    }
    fn quantize(&self,lux:f64)->u16 {
        let gain=1.2*f64::from(self.mt)/69.0*if self.mode&3==1 {2.0}else{1.0};
        let raw=(lux*gain).round().clamp(0.0,65535.0) as u16;
        // A deterministic nominal low-resolution quantizer (4 raw counts). The note
        // specifies typical 4 lx, not exact low-bit transfer/error on a physical unit.
        if self.mode&3==3 {raw&!3}else{raw}
    }
    pub fn advance_to(&mut self,cycle:u64) {
        let cycle=cycle.max(self.cycle);let previous=self.cycle;self.cycle=cycle;
        let Some(end)=self.deadline else{return};
        if cycle<end {self.energy+=self.lux*(cycle-previous) as f64;return;}
        self.energy+=self.lux*(end.saturating_sub(previous)) as f64;self.data=self.quantize(self.energy/self.period as f64);self.energy=0.0;
        if self.mode&0x20!=0 {self.on=false;self.deadline=None;return;}
        let elapsed=cycle-end;let completed=elapsed/self.period;
        if completed>0 {self.data=self.quantize(self.lux);}
        self.energy=self.lux*(elapsed%self.period) as f64;
        self.deadline=Some(end.saturating_add(completed.saturating_add(1).saturating_mul(self.period)));
    }
    pub fn start(&mut self,read:bool)->bool {
        if !self.powered {return false;}
        if read {self.readout=self.data.to_be_bytes();self.at=0;}true
    }
    pub fn write(&mut self,command:u8)->bool {
        if !self.powered||self.written {return false;}self.written=true;
        match command {
            0=>{self.on=false;self.deadline=None;self.energy=0.0;}
            1=>{self.on=true;self.deadline=None;self.energy=0.0;}
            7=>{if !self.on {return false;}self.data=0;}
            0x10|0x11|0x13|0x20|0x21|0x23=>{
                if !(31..=254).contains(&self.mt) {return false;}
                self.on=true;self.mode=command;self.energy=0.0;
                let micros=if command&3==3 {24_000}else{180_000};
                self.period=(self.hz.saturating_mul(micros)*u64::from(self.mt)/69/1_000_000).max(1);
                self.deadline=Some(self.cycle.saturating_add(self.period));
            }
            0x40..=0x47=>self.mt=(self.mt&31)|((command&7)<<5),
            0x60..=0x7f=>self.mt=(self.mt&0xe0)|(command&31),
            _=>return false, // Undefined opcode: intentionally unsupported, not silicon behavior.
        }true
    }
    pub fn read(&mut self)->u8 {if !self.powered {return 0xff;}let value=self.readout.get(self.at).copied().unwrap_or(0xff);self.at=self.at.saturating_add(1);value}
    pub fn stop(&mut self) {self.written=false;}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn active(lux:f64)->Bh1750 {let mut d=Bh1750::new(1_000_000);assert!(d.update(0,true,lux));d.set_power(true);d}
    fn command(d:&mut Bh1750,c:u8)->bool {assert!(d.start(false));let ok=d.write(c);d.stop();ok}
    fn read(d:&mut Bh1750)->u16 {assert!(d.start(true));let value=u16::from_be_bytes([d.read(),d.read()]);d.stop();value}
    #[test] fn datasheet_vectors_busy_old_result_and_one_time() {
        let mut d=active(28066.6666667);assert!(command(&mut d,0x20));d.advance_to(179999);assert_eq!(read(&mut d),0);d.advance_to(180000);assert_eq!(read(&mut d),0x8390);
        assert!(!command(&mut d,7));d.update(200000,true,7.5);d.advance_to(1_000_000);assert_eq!(read(&mut d),0x8390);
        assert!(command(&mut d,0x21));d.advance_to(1_180_000);assert_eq!(read(&mut d),18);
        assert!(command(&mut d,1));assert!(command(&mut d,7));assert_eq!(read(&mut d),0);
    }
    #[test] fn mtreg_gain_deadline_and_saturation() {
        let mut d=active(1000.0);assert!(command(&mut d,0x44));assert!(command(&mut d,0x6a));assert_eq!(d.mt,138);assert!(command(&mut d,0x10));d.advance_to(359999);assert_eq!(read(&mut d),0);d.advance_to(360000);assert_eq!(read(&mut d),2400);
        d.update(360000,true,100_000.0);d.advance_to(720000);assert_eq!(read(&mut d),65535);
        assert!(command(&mut d,0x40));assert!(command(&mut d,0x7f));assert!(command(&mut d,0x20));d.advance_to(900000);assert_eq!(read(&mut d),53913);
        assert!(command(&mut d,0x47));assert!(command(&mut d,0x7f));assert!(!command(&mut d,0x10));
    }
    #[test] fn integration_power_stop_and_invalid_input() {
        let mut d=active(0.0);assert!(command(&mut d,0x10));d.update(90000,true,1000.0);d.advance_to(180000);assert_eq!(read(&mut d),600);d.advance_to(360000);assert_eq!(read(&mut d),1200);
        assert!(d.start(false));assert!(d.write(0));assert!(d.start(false));assert!(!d.write(1));d.stop();assert!(command(&mut d,1));
        assert!(!d.update(360000,true,f64::NAN));assert_eq!(d.lux,1000.0);d.update(360000,false,1000.0);d.set_power(true);assert!(!d.start(true));d.update(400000,true,1000.0);d.set_power(true);assert_eq!(read(&mut d),0);assert_eq!(d.mt,69);
    }
}
