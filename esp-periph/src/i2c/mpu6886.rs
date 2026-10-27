//! MPU6886 polled register subset. Host-fed samples, no FIFO, interrupts or ODR clock.
//! DS-000193 rev1.1 pp31/37/41-49 and M5Unified 368ad608,
//! utility/imu/MPU6886_Class.{hpp,inl}; sources and limits in
//! docs/platform/atoms3-runtime.md. Register values are sensor-specific, not SoC registers.
use super::I2cDevice;
use std::sync::{Arc,Mutex};

#[derive(Clone,Copy,Debug)]
pub struct Mpu6886Sample {pub accel_g:[f64;3],pub gyro_dps:[f64;3],pub version:u64}
impl Default for Mpu6886Sample {fn default()->Self {Self{accel_g:[0.0,0.0,1.0],gyro_dps:[0.0;3],version:1}}}
pub struct Mpu6886 {regs:[u8;128],ptr:u8,first:bool,sample:Arc<Mutex<Mpu6886Sample>>,version:u64}
impl Mpu6886 {
    pub fn new(sample:Arc<Mutex<Mpu6886Sample>>)->Self {let mut d=Self{regs:[0;128],ptr:0,first:true,sample,version:0};d.reset();d}
    fn reset(&mut self) {self.regs.fill(0);self.regs[0x75]=0x19;self.regs[0x6b]=0x41;self.regs[0x1a]=0x80;self.version=0;}
    fn latch(&mut self) {
        let sample=*self.sample.lock().expect("MPU6886 sample mutex poisoned");
        let awake=self.regs[0x6b]&0x40==0 && self.regs[0x6b]&7!=7;
        if awake && self.version!=sample.version {self.regs[0x3a]|=1;self.version=sample.version;}
        if !awake {self.regs[0x3a]=0;}
        for (start,values,range,disable_shift) in [(0x3b,sample.accel_g,2u32<<((self.regs[0x1c]>>3)&3),3),(0x43,sample.gyro_dps,250u32<<((self.regs[0x1b]>>3)&3),0)] {
            for (axis,value) in values.into_iter().enumerate() {
                let enabled=awake && self.regs[0x6c]&(1<<(disable_shift+2-axis))==0 && (start!=0x43 || self.regs[0x6b]&0x10==0);
                let raw=if enabled && value.is_finite() {(value*32768.0/range as f64).round().clamp(-32768.0,32767.0) as i16} else {0};
                self.regs[start+axis*2..start+axis*2+2].copy_from_slice(&raw.to_be_bytes());
            }
        }
        // Zero temperature ADC means 25 C in the official driver; no thermal simulation.
    }
}
impl I2cDevice for Mpu6886 {
    fn start(&mut self,read:bool)->bool {if read {self.latch();} else {self.first=true;}true}
    fn write(&mut self,value:u8)->bool {
        if self.first {self.ptr=value&0x7f;self.first=false;return true;}
        let reg=self.ptr as usize;
        if reg==0x6b && value&0x80!=0 {self.reset();}
        else if (0x13..=0x1e).contains(&reg) || [0x23,0x37,0x38,0x69,0x6a,0x6b,0x6c].contains(&reg) {
            self.regs[reg]=value;
            if reg==0x6b {self.version=0;}
            // FIFO reset self-clears; FIFO count/data stay empty, never invented samples.
            if reg==0x6a {self.regs[reg]&=!0x04;}
        }
        self.ptr=self.ptr.wrapping_add(1)&0x7f;true
    }
    fn read(&mut self)->u8 {
        let reg=self.ptr as usize;let value=self.regs[reg];
        if reg==0x3a || self.regs[0x37]&0x10!=0 {self.regs[0x3a]=0;}
        self.ptr=self.ptr.wrapping_add(1)&0x7f;value
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn write(d:&mut Mpu6886,reg:u8,bytes:&[u8]) {d.start(false);d.write(reg);for b in bytes {d.write(*b);}}
    fn read(d:&mut Mpu6886,reg:u8,n:usize)->Vec<u8> {write(d,reg,&[]);d.start(true);(0..n).map(|_|d.read()).collect()}
    fn vector(bytes:&[u8])->Vec<i16> {bytes.as_chunks::<2>().0.iter().map(|b|i16::from_be_bytes(*b)).collect()}
    #[test]
    fn official_init_ranges_standby_and_reset() {
        let sample=Arc::new(Mutex::new(Mpu6886Sample{accel_g:[1.0,-2.0,100.0],gyro_dps:[1000.0,-500.0,f64::NAN],version:1}));let mut d=Mpu6886::new(sample);
        assert_eq!(read(&mut d,0x75,1),[0x19]);assert_eq!(read(&mut d,0x3b,14),[0;14]);
        for (r,v) in [(0x6b,0x80),(0x6b,1),(0x1c,0x10),(0x1b,0x18),(0x1a,1),(0x19,3),(0x37,0xc0),(0x38,0),(0x1d,0),(0x6a,0),(0x23,0)] {write(&mut d,r,&[v]);if r!=0x6b {assert_eq!(read(&mut d,r,1),[v]);}}
        assert_eq!(vector(&read(&mut d,0x3b,14)),[4096,-8192,32767,0,16384,-8192,0]);
        write(&mut d,0x6c,&[0x28]);assert_eq!(vector(&read(&mut d,0x3b,6)),[0,-8192,0]);
        write(&mut d,0x6b,&[0x80]);assert_eq!(read(&mut d,0x6b,1),[0x41]);assert_eq!(read(&mut d,0x1a,1),[0x80]);assert_eq!(read(&mut d,0x75,1),[0x19]);assert_eq!(read(&mut d,0x3b,14),[0;14]);
    }
    #[test]
    fn burst_is_coherent_data_ready_tracks_host_and_fifo_stays_empty() {
        let sample=Arc::new(Mutex::new(Mpu6886Sample::default()));let mut d=Mpu6886::new(sample.clone());write(&mut d,0x6b,&[1]);
        assert_eq!(read(&mut d,0x3a,1),[1]);assert_eq!(read(&mut d,0x3a,1),[0]);write(&mut d,0x3b,&[]);d.start(true);let first=d.read();
        *sample.lock().unwrap()=Mpu6886Sample{accel_g:[-1.0,0.0,0.0],gyro_dps:[0.0;3],version:2};
        let mut bytes=vec![first];bytes.extend((0..5).map(|_|d.read()));assert_eq!(vector(&bytes),[0,0,16384]);assert_eq!(read(&mut d,0x3a,1),[1]);assert_eq!(vector(&read(&mut d,0x3b,6)),[-16384,0,0]);
        write(&mut d,0x6a,&[0x44]);assert_eq!(read(&mut d,0x6a,1),[0x40]);assert_eq!(read(&mut d,0x72,3),[0,0,0]);
        write(&mut d,0x75,&[0xff]);assert_eq!(read(&mut d,0x75,1),[0x19]);
    }
}
