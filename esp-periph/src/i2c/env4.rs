//! U001-D ENV-IV: SHT40-AD1B + BMP280, ideal ambient inputs and register protocol.
//! Sources: Sensirion SHT4x D1 v2 (2021-07) §§3.2,4; Bosch BST-BMP280-DS001-26
//! rev1.26 (2021-10) §§3,4,8. Exact URLs/hashes live in the U001-D source receipt.
//! No heater, environmental noise, thermal lag, or analog supply/bus model.

#[derive(Clone, Copy, Debug)]
pub struct Environment {pub temperature_c:f64,pub humidity_percent:f64,pub pressure_hpa:f64}
impl Default for Environment {fn default()->Self {Self{temperature_c:25.0,humidity_percent:50.0,pressure_hpa:1013.25}}}
impl Environment {
    // Intersection of SHT40 (-40..125 C) and BMP280 (-40..85 C) operating ranges.
    pub fn valid(self)->bool {self.temperature_c.is_finite()&&(-40.0..=85.0).contains(&self.temperature_c)&&self.humidity_percent.is_finite()&&(0.0..=100.0).contains(&self.humidity_percent)&&self.pressure_hpa.is_finite()&&(300.0..=1100.0).contains(&self.pressure_hpa)}
}
fn crc(bytes:&[u8])->u8 {let mut c=0xffu8;for b in bytes {c^=*b;for _ in 0..8 {c=if c&0x80!=0 {(c<<1)^0x31}else{c<<1};}}c}
fn words(a:u16,b:u16)->[u8;6] {let a=a.to_be_bytes();let b=b.to_be_bytes();[a[0],a[1],crc(&a),b[0],b[1],crc(&b)]}
#[derive(Default)]
struct Sht40 {ready:u64,response:Option<[u8;6]>,readout:[u8;6],at:usize,general:bool,written:bool}
impl Sht40 {
    fn reset(&mut self,now:u64,hz:u64) {*self=Self{ready:now.saturating_add(hz/1000),..Self::default()};}
    fn start(&mut self,now:u64,read:bool,general:bool)->bool {
        self.general=general;self.written=false;
        if read {if now<self.ready {return false;}let Some(data)=self.response.take()else{return false};self.readout=data;self.at=0;}
        // Write commands may abort a conversion using reset (datasheet §4.7).
        true
    }
    fn write(&mut self,now:u64,hz:u64,b:u8,sample:Environment)->bool {
        if self.written {return false;}self.written=true;
        if self.general {if b==6 {self.reset(now,hz);return true;}return false;}
        if b==0x94 {self.reset(now,hz);return true;}
        if now<self.ready {return false;}
        if b==0x89 {self.response=Some(words(0x4553,0x5034));return true;} // Stable virtual serial; not a physical sensor identity.
        let micros=match b {0xfd=>8200,0xf6=>4500,0xe0=>1700,_=>return false};
        let t=((sample.temperature_c+45.0)*65535.0/175.0).round() as u16;
        let h=((sample.humidity_percent+6.0)*65535.0/125.0).round() as u16;
        self.response=Some(words(t,h));self.ready=now.saturating_add(hz/1_000_000*micros);true
    }
    fn read(&mut self)->u8 {let value=self.readout.get(self.at).copied().unwrap_or(0xff);self.at=self.at.saturating_add(1);value}
}

// Deterministic virtual trimming, from M5Unit-ENV's datasheet_trimming test at
// 28c521a137cb6696f4860cdce93ca7df9ce9a6af. Not calibration measured on a unit.
const TRIM:[i32;12]=[27504,26435,-1000,36477,-10685,3024,2855,140,-7,15500,-14600,6000];
fn temperature(raw:u32)->(f64,f64) {
    let v1=(f64::from(raw)/16384.0-f64::from(TRIM[0])/1024.0)*f64::from(TRIM[1]);
    let x=f64::from(raw)/131072.0-f64::from(TRIM[0])/8192.0;
    let fine=v1+x*x*f64::from(TRIM[2]);(fine/5120.0,fine)
}
fn pressure(raw:u32,fine:f64)->f64 {
    let mut v1=fine/2.0-64000.0;
    let mut v2=v1*v1*f64::from(TRIM[8])/32768.0+v1*f64::from(TRIM[7])*2.0;
    v2=v2/4.0+f64::from(TRIM[6])*65536.0;
    v1=(f64::from(TRIM[5])*v1*v1/524288.0+f64::from(TRIM[4])*v1)/524288.0;
    v1=(1.0+v1/32768.0)*f64::from(TRIM[3]);
    let p=(1048576.0-f64::from(raw)-v2/4096.0)*6250.0/v1;
    p+(f64::from(TRIM[11])*p*p/2147483648.0+p*f64::from(TRIM[10])/32768.0+f64::from(TRIM[9]))/16.0
}
fn nearest(target:f64,increasing:bool,f:impl Fn(u32)->f64)->u32 {
    let(mut lo,mut hi)=(0,0xfffff);
    while lo<hi {let mid=lo+(hi-lo)/2;if (f(mid)<target)==increasing {lo=mid+1;}else{hi=mid;}}
    if lo>0&&(f(lo-1)-target).abs()<(f(lo)-target).abs() {lo-1}else{lo}
}
fn raw_values(sample:Environment)->[u32;2] {
    let t=nearest(sample.temperature_c,true,|x|temperature(x).0);let fine=temperature(t).1;
    [nearest(sample.pressure_hpa*100.0,false,|x|pressure(x,fine)),t]
}
struct Bmp280 {ptr:u8,first:bool,ctrl:u8,config:u8,reset_until:u64,deadline:Option<u64>,raw:[u32;2],filtered:[Option<f64>;2],burst:[u8;6]}
impl Default for Bmp280 {fn default()->Self {Self{ptr:0,first:true,ctrl:0,config:0,reset_until:0,deadline:None,raw:[0x80000;2],filtered:[None;2],burst:[0;6]}}}
impl Bmp280 {
    fn reset(&mut self,now:u64,hz:u64) {*self=Self{reset_until:now.saturating_add(hz/500),..Self::default()};}
    fn osrs(value:u8)->u64 {if value==0 {0}else{1<<value.min(5).saturating_sub(1)}}
    fn duration(&self,hz:u64)->u64 {let t=Self::osrs(self.ctrl>>5);let p=Self::osrs((self.ctrl>>2)&7);hz/1_000_000*(1250+2300*t+if p>0 {2300*p+575}else{0})}
    fn standby(&self,hz:u64)->u64 {hz/2000*[1,125,250,500,1000,2000,4000,8000][usize::from(self.config>>5)]}
    fn advance(&mut self,now:u64,hz:u64,sample:Environment) {
        let Some(end)=self.deadline else{return};if now<end {return;}
        let normal=self.ctrl&3==3;let period=self.duration(hz).saturating_add(self.standby(hz)).max(1);
        let count=if normal {1+(now-end)/period}else{1};
        let raw=raw_values(sample);let filter=(self.config>>2)&7;let n=if filter==0 {1.0}else{f64::from(1u32<<filter.min(4))};
        for (i,target) in raw.into_iter().enumerate() {
            let oversampling=if i==0 {(self.ctrl>>2)&7}else{self.ctrl>>5};
            if oversampling==0 {self.raw[i]=0x80000;continue;}
            let value=self.filtered[i].map_or(f64::from(target),|old|f64::from(target)+(old-f64::from(target))*(1.0-1.0/n).powf(count as f64));
            self.filtered[i]=Some(value);self.raw[i]=value.round() as u32;
        }
        if normal {self.deadline=Some(end.saturating_add(count.saturating_mul(period)));}
        else {self.ctrl&=!3;self.deadline=None;}
    }
    fn start(&mut self,read:bool) {if read {for (i,raw) in self.raw.into_iter().enumerate() {self.burst[i*3]=(raw>>12) as u8;self.burst[i*3+1]=(raw>>4) as u8;self.burst[i*3+2]=(raw<<4) as u8;}}else{self.first=true;}}
    fn write(&mut self,now:u64,hz:u64,b:u8)->bool {
        if self.first {self.ptr=b;self.first=false;return true;}
        match self.ptr {
            0xe0 if b==0xb6=>{self.reset(now,hz);return true;}
            0xf4=>{self.ctrl=b;self.deadline=if b&3!=0 {Some(now.max(self.reset_until).saturating_add(self.duration(hz)))}else{None};}
            0xf5 if self.ctrl&3==0=>{self.config=b&0xfc;self.filtered=[None;2];} // SPI3W is not wired on U001-D.
            _=>{}
        }
        self.ptr=self.ptr.wrapping_add(1);true
    }
    fn read(&mut self,now:u64,hz:u64)->u8 {
        let value=match self.ptr {
            0x88..=0x9f=>{let at=usize::from(self.ptr-0x88);(TRIM[at/2] as u16).to_le_bytes()[at%2]}
            0xd0=>0x58,0xf3=>u8::from(now<self.reset_until)|if self.deadline.is_some_and(|d|now>=d.saturating_sub(self.duration(hz))) {8}else{0},
            0xf4=>self.ctrl,0xf5=>self.config,0xf7..=0xfc=>self.burst[usize::from(self.ptr-0xf7)],_=>0,
        };self.ptr=self.ptr.wrapping_add(1);value
    }
}

pub struct Env4 {pub sample:Environment,pub connected:bool,powered:bool,cycle:u64,hz:u64,sht:Sht40,bmp:Bmp280}
impl Env4 {
    pub fn new(hz:u64)->Self {Self{sample:Environment::default(),connected:false,powered:false,cycle:0,hz,sht:Sht40::default(),bmp:Bmp280::default()}}
    pub fn update(&mut self,cycle:u64,connected:bool,sample:Environment)->bool {if !sample.valid(){return false;}self.advance_to(cycle);self.connected=connected;self.sample=sample;true}
    pub fn set_power(&mut self,supply:bool) {let powered=supply&&self.connected;if powered!=self.powered {self.powered=powered;self.sht.reset(self.cycle,self.hz);self.bmp.reset(self.cycle,self.hz);}}
    pub fn advance_to(&mut self,cycle:u64) {self.cycle=cycle;if self.powered {self.bmp.advance(cycle,self.hz,self.sample);}}
    pub fn start(&mut self,address:u8,read:bool,general:bool)->bool {
        if !self.powered {return false;}
        if address==0x44 {self.sht.start(self.cycle,read,general)}else if address==0x76&&!general {self.bmp.start(read);true}else{false}
    }
    pub fn write(&mut self,address:u8,byte:u8)->bool {if !self.powered {return false;}
        if address==0x44 {self.sht.write(self.cycle,self.hz,byte,self.sample)}else if address==0x76 {self.bmp.write(self.cycle,self.hz,byte)}else{false}}
    pub fn read(&mut self,address:u8)->u8 {if !self.powered {return 0xff;}
        if address==0x44 {self.sht.read()}else if address==0x76 {self.bmp.read(self.cycle,self.hz)}else{0xff}}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn active()->Env4 {let mut d=Env4::new(1_000_000);assert!(d.update(0,true,Environment::default()));d.set_power(true);d.advance_to(2_000);d}
    fn write(d:&mut Env4,reg:u8,value:u8) {assert!(d.start(0x76,false,false));assert!(d.write(0x76,reg));assert!(d.write(0x76,value));}
    fn read(d:&mut Env4,reg:u8)->u8 {assert!(d.start(0x76,false,false));assert!(d.write(0x76,reg));assert!(d.start(0x76,true,false));d.read(0x76)}
    #[test] fn sht40_crc_busy_once_reset_and_power() {
        assert_eq!(crc(&[0xbe,0xef]),0x92);let mut d=active();assert!(d.start(0x44,false,false));assert!(d.write(0x44,0xfd));assert!(!d.start(0x44,true,false));
        d.advance_to(10_200);assert!(d.start(0x44,true,false));let bytes:[u8;6]=std::array::from_fn(|_|d.read(0x44));assert_eq!(bytes,words(26214,29360));assert!(!d.start(0x44,true,false));
        assert!(d.start(0x44,false,false));assert!(!d.write(0x44,0x39));assert!(d.start(0x44,false,true));assert!(d.write(0x44,6));assert!(!d.start(0x44,true,false));
        d.set_power(false);assert!(!d.start(0x44,false,false));assert!(!d.start(0x76,false,false));
    }
    #[test] fn bmp280_compensation_vector_and_inverse_range() {
        let(t,fine)=temperature(519888);assert!((t-25.0824779).abs()<0.00001);assert!((pressure(415148,fine)-100653.2668).abs()<0.001);
        for t in [-40.0,0.0,25.0,85.0] {for p in [300.0,1013.25,1100.0] {let raw=raw_values(Environment{temperature_c:t,pressure_hpa:p,..Environment::default()});let(actual,fine)=temperature(raw[1]);assert!((actual-t).abs()<0.001);assert!((pressure(raw[0],fine)/100.0-p).abs()<0.005);}}
    }
    #[test] fn bmp280_forced_normal_sleep_reset_and_burst_shadow() {
        let mut d=active();assert_eq!(read(&mut d,0xd0),0x58);assert_eq!(read(&mut d,0x88),0x70);write(&mut d,0xf4,0x25);assert_ne!(read(&mut d,0xf3)&8,0);
        d.advance_to(9000);assert_eq!(read(&mut d,0xf4)&3,0);let old=d.bmp.raw;assert_ne!(old,[0x80000;2]);
        write(&mut d,0xf4,0x27);d.advance_to(16000);assert!(d.start(0x76,false,false));d.write(0x76,0xf7);assert!(d.start(0x76,true,false));let frozen=d.bmp.burst;d.update(17000,true,Environment{temperature_c:30.0,..Environment::default()});d.advance_to(100000);assert_ne!(d.bmp.raw,old);for b in frozen {assert_eq!(d.read(0x76),b);}
        write(&mut d,0xf4,0);let sleeping=d.bmp.raw;d.update(110000,true,Environment::default());d.advance_to(500000);assert_eq!(d.bmp.raw,sleeping);
        write(&mut d,0xe0,0xb6);assert_eq!(read(&mut d,0xf3)&1,1);assert_eq!(d.bmp.raw,[0x80000;2]);assert_eq!(read(&mut d,0xf4),0);
    }
    #[test] fn invalid_inputs_leave_state_and_missing_address_nacks() {let mut d=active();assert!(!d.update(0,true,Environment{temperature_c:f64::NAN,..Environment::default()}));assert_eq!(d.sample.temperature_c,25.0);assert!(!d.start(0x45,false,false));assert!(!d.start(0x77,false,false));}
}
