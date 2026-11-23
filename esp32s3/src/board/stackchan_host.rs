//! M5Stack CoreS3 host: ILI9342C, FT6336U, AW9523B, AXP2101 and optional BMI270.
//! Wiring: Sch_M5_CoreS3_v1.0.pdf; AW9523B-EN.pdf registers 00–07/10–13/7F;
//! M5GFX 7cb26b44 and M5Unified 44952cb8. USB supply is an explicit fixed
//! virtual profile; charging, battery dynamics, power-latch and sleep are not modeled.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,bmi270::{Bmi270,Bmi270Sample},ft6336::{Ft6336g,Ft6336State}};
use esp_soc::board::{BoardEdge,VirtualCycle,SpiPins};
use esp_soc::devices::{DcsPanel,i2c_bitbang::I2cBitBang};

pub(super) struct HostState {aw:[u8;256],axp:[u8;256],input:[u8;2],pending:[u8;2],pub version:u64,pub reset_version:u64,sd_power_cycles:u64}
impl HostState {
    pub fn new()->Self {
        let mut state=Self{aw:[0;256],axp:[0;256],input:[0xff;2],pending:[0;2],version:0,reset_version:0,sd_power_cycles:0};state.reset_aw();
        // AXP2101 ID, USB present, no emulated battery. DCDC1 supplies the host.
        for (r,v) in [(0,0x20),(1,0x10),(3,0x4a),(0x20,0x20),(0x27,0x14),(0x80,1),(0x82,18)] {state.axp[r]=v;}
        state
    }
    fn reset_aw(&mut self) {
        // AD1=AD0=GND (address58): power-on output levels and direction are 0.
        self.aw.fill(0);self.aw[0x10]=0x23;self.aw[0x12]=0xff;self.aw[0x13]=0xff;self.pending=[0;2];self.reset_version+=1;self.version+=1;
    }
    fn output(&self,port:usize,bit:u8)->bool {self.aw[0x12+port]&bit!=0 && self.aw[4+port]&bit==0 && self.aw[2+port]&bit!=0}
    pub fn lcd_reset(&self)->bool {self.output(1,2)}
    pub fn touch_reset(&self)->bool {self.output(0,1)}
    pub fn backlight(&self)->bool {self.axp[0x90]&0x80!=0 && self.axp[0x99]!=0}
    // CoreS3/SE V1.0 schematic: VDD_3V3_SD is AXP2101 ALDO4.
    // AXP2101 V1.0 §§6.13.2.75, .80: REG90 bit3 and REG95 voltage selector.
    fn sd_powered(&self)->bool {self.axp[0x90]&8!=0 && matches!(self.axp[0x95]&31,22..=30)}
    fn input_level(&mut self,port:usize,bit:u8,high:bool) {
        let old=self.input[port];if high {self.input[port]|=bit;} else {self.input[port]&=!bit;}
        self.pending[port]|=(old^self.input[port])&self.aw[4+port]&self.aw[0x12+port]&!self.aw[6+port];
    }
    pub fn touch_level(&mut self,high:bool) {self.input_level(1,4,high);}
    pub fn irq(&self)->bool {(0..2).any(|p|self.pending[p]&self.aw[4+p]&self.aw[0x12+p]&!self.aw[6+p]!=0)}
    fn aw_read(&mut self,reg:u8)->u8 {
        if reg<2 {let p=usize::from(reg);self.pending[p]=0;return (self.input[p]&self.aw[4+p])|(self.aw[2+p]&!self.aw[4+p]);}
        self.aw[usize::from(reg)]
    }
    fn aw_write(&mut self,reg:u8,value:u8) {
        let old_reset=self.lcd_reset();
        match reg {0x02..=0x07|0x11..=0x13|0x20..=0x2f=>{self.aw[usize::from(reg)]=value;self.version+=1;},0x7f if value==0=>self.reset_aw(),_=>{}}
        if old_reset && !self.lcd_reset() {self.reset_version+=1;}
    }
    fn axp_read(&self,reg:u8)->u8 {
        // Enabled ADC channels see fixed USB=5V / VSYS=3.3V; no battery sample.
        match reg {0x38|0x39 if self.axp[0x30]&4!=0=>(5000u16>>if reg==0x38 {8} else {0}) as u8,0x3a|0x3b if self.axp[0x30]&8!=0=>(3300u16>>if reg==0x3a {8} else {0}) as u8,_=>self.axp[usize::from(reg)]}
    }
    fn axp_write(&mut self,reg:u8,value:u8) {
        let changed_sd_rail=matches!(reg,0x90|0x95) && self.sd_powered();
        match reg {0x48..=0x4a=>self.axp[usize::from(reg)]&=!value,0x04..=0x07|0x10|0x12..=0x1a|0x20..=0x30|0x40..=0x42|0x50..=0x6a|0x80..=0x9a=>self.axp[usize::from(reg)]=value,_=>{}}
        if changed_sd_rail && !self.sd_powered() {self.sd_power_cycles=self.sd_power_cycles.wrapping_add(1);}
        self.version+=1;
    }
}

pub(super) struct HostDevice {state:Arc<Mutex<HostState>>,axp:bool,ptr:u8,first:bool}
impl HostDevice {pub fn new(state:Arc<Mutex<HostState>>,axp:bool)->Self {Self{state,axp,ptr:0,first:true}}}
impl I2cDevice for HostDevice {
    fn start(&mut self,read:bool)->bool {if !read {self.first=true;}true}
    fn write(&mut self,value:u8)->bool {if self.first {self.ptr=value;self.first=false;} else {let mut state=self.state.lock().expect("CoreS3 host mutex poisoned");if self.axp {state.axp_write(self.ptr,value);} else {state.aw_write(self.ptr,value);}self.ptr=self.ptr.wrapping_add(1);}true}
    fn read(&mut self)->u8 {let mut state=self.state.lock().expect("CoreS3 host mutex poisoned");let value=if self.axp {state.axp_read(self.ptr)} else {state.aw_read(self.ptr)};self.ptr=self.ptr.wrapping_add(1);value}
}

pub(super) struct HostTouch {device:Ft6336g,state:Arc<Mutex<HostState>>,touch:Arc<Mutex<Ft6336State>>}
impl HostTouch {pub fn new(state:Arc<Mutex<HostState>>,touch:Arc<Mutex<Ft6336State>>)->Self {Self{device:Ft6336g::new(touch.clone()),state,touch}}}
impl I2cDevice for HostTouch {
    fn start(&mut self,read:bool)->bool {
        let powered=self.state.lock().expect("CoreS3 host mutex poisoned").touch_reset();let mut touch=self.touch.lock().expect("CoreS3 touch mutex poisoned");touch.powered=powered;if !powered {touch.reset();}drop(touch);self.device.start(read)
    }
    fn write(&mut self,value:u8)->bool {self.device.write(value)}
    fn read(&mut self)->u8 {self.device.read()}
    fn stop(&mut self) {self.device.stop();}
}

/// Digital host wiring common to the standalone CoreS3 and StackChan's
/// CoreS3 carrier. The caller owns any variant-specific peripherals.
pub(super) struct CoreS3Host {
    panel:DcsPanel,command:u8,read_at:usize,read_index:u8,reset_version:u64,display_changes:u64,
    i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,host:Arc<Mutex<HostState>>,
    touch:Arc<Mutex<Ft6336State>>,imu:Option<Arc<Mutex<Bmi270Sample>>>,
    irq_high:bool,cycle:VirtualCycle,gpio_events:u64,
}
impl CoreS3Host {
    pub(super) fn new(has_imu:bool)->Self {
        Self{panel:DcsPanel::new(320,240),command:0,read_at:0,read_index:0,reset_version:0,display_changes:0,
            i2c:I2cBitBang::new(),levels:[true;2],edges:Vec::new(),host:Arc::new(Mutex::new(HostState::new())),
            touch:Default::default(),imu:has_imu.then(Arc::default),irq_high:true,cycle:0,gpio_events:0}
    }
    fn sync_host(&mut self) {
        let mut host=self.host.lock().expect("CoreS3 host mutex poisoned");
        if host.reset_version!=self.reset_version {self.reset_version=host.reset_version;self.panel.reset();self.command=0;self.read_at=0;self.read_index=0;self.display_changes+=1;}
        let mut touch=self.touch.lock().expect("CoreS3 touch mutex poisoned");touch.powered=host.touch_reset();if !touch.powered {touch.reset();host.touch_level(true);}
        let high=!host.irq();if high!=self.irq_high {self.irq_high=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:21,level:high});}
    }
    pub(super) fn gpio_changes(&mut self,changes:&[(u8,bool)]) {self.gpio_events+=changes.len() as u64;for &(pin,high) in changes {if pin==35 {self.panel.dc=high;}}}
    pub(super) fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.cycle=cycle;self.gpio_changes(changes);let released=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        let scl=released(11);let sda=self.i2c.drive(scl,released(12));
        for (index,(pin,level)) in [(12,sda),(11,scl)].into_iter().enumerate() {if level!=self.levels[index] {self.levels[index]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
        self.sync_host();
    }
    pub(super) fn gpio_events(&self)->u64 {self.gpio_events}
    pub(super) fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        self.sync_host();
        if !self.host.lock().expect("CoreS3 host mutex poisoned").lcd_reset() || pins.sclk&(1<<36)==0 || pins.cs&(1<<3)==0 || (pins.mosi&(1<<37)==0 && pins.miso!=Some(37)) {return vec![0xff;rx_len];}
        for &byte in tx {
            if !self.panel.dc {self.command=byte;self.read_at=0;self.display_changes+=1;}
            else if self.command==0xd9 {self.read_index=byte&15;}
            self.panel.byte(byte);
        }
        // ILITEK ILI9342C V102 §§8.2.3 / 8.2.46–48: ID E3/00/00; EXTC ID4=00/93/42.
        // SPI peripheral accounts for the guest's dummy phase, as with the other DCS boards.
        (0..rx_len).map(|_| {let index=self.read_at+usize::from(self.read_index.saturating_sub(1));let value=match self.command {0x04=>[0xe3,0,0].get(self.read_at).copied().unwrap_or(0),0xda=>0xe3,0xdb|0xdc=>0,0xd3=>[0,0x93,0x42].get(index).copied().unwrap_or(0),_=>0};self.read_at=self.read_at.saturating_add(1);value}).collect()
    }
    pub(super) fn i2c_devices(&mut self, additional: Vec<(u8,Box<dyn I2cDevice>)>)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();
        self.i2c.attach(0x58,Box::new(HostDevice::new(self.host.clone(),false)));self.i2c.attach(0x34,Box::new(HostDevice::new(self.host.clone(),true)));
        self.i2c.attach(0x38,Box::new(HostTouch::new(self.host.clone(),self.touch.clone())));
        if let Some(imu)=&self.imu {self.i2c.attach(0x69,Box::new(Bmi270::new(imu.clone())));}
        for (address,device) in additional {self.i2c.attach(address,device);}
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((12,11))).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    pub(super) fn take_edges(&mut self)->Vec<BoardEdge> {self.sync_host();std::mem::take(&mut self.edges)}
    pub(super) fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(21,self.irq_high),(12,self.levels[0]),(11,self.levels[1])]}
    pub(super) fn touch_at(&mut self,cycle:VirtualCycle,x:u16,y:u16,down:bool) {self.cycle=cycle;self.touch(x,y,down);}
    pub(super) fn touch(&mut self,x:u16,y:u16,down:bool) {
        self.sync_host();let mut touch=self.touch.lock().expect("CoreS3 touch mutex poisoned");if !touch.powered {return;}touch.update(x.min(319),y.min(239),down);drop(touch);
        self.host.lock().expect("CoreS3 host mutex poisoned").touch_level(!down);self.sync_host();
    }
    pub(super) fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {let h=self.host.lock().expect("CoreS3 host mutex poisoned");Some((320,240,if h.lcd_reset()&&h.backlight()&&self.panel.on&&!self.panel.sleeping {self.panel.gram.clone()} else {vec![0;320*240]},self.panel.pixels_written+self.panel.resets+self.display_changes+h.version))}
    pub(super) fn display_version(&self)->u64 {self.panel.pixels_written+self.panel.resets+self.display_changes+self.host.lock().expect("CoreS3 host mutex poisoned").version}
    pub(super) fn display_frames(&self)->u64 {self.panel.frames}
    pub(super) fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),320,240))}
    pub(super) fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {if !accel_g.iter().chain(gyro_dps.iter()).all(|v|v.is_finite()) {return false;}let Some(imu)=&self.imu else {return false;};let mut sample=imu.lock().expect("CoreS3 IMU mutex poisoned");sample.accel_g=accel_g;sample.gyro_dps=gyro_dps;sample.version=sample.version.wrapping_add(1);true}
    pub(super) fn advance_to(&mut self,cycle:VirtualCycle) {self.cycle=cycle;self.sync_host();}
    pub(super) fn sd_power(&self)->(bool,u64) {
        let host=self.host.lock().expect("CoreS3 host mutex poisoned");(host.sd_powered(),host.sd_power_cycles)
    }
    pub(super) fn sd_inserted(&mut self) {self.host.lock().expect("CoreS3 host mutex poisoned").input_level(0,1<<4,false);}
    pub(super) fn pixels_written(&self)->u64 {self.panel.pixels_written}
    #[cfg(test)]
    pub(super) fn panel_on(&self)->bool {self.panel.on}
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn aw_identity_reset_direction_and_masked_touch_interrupt() {
        let mut h=HostState::new();assert_eq!(h.aw_read(0x10),0x23);h.aw_write(0x10,0);assert_eq!(h.aw_read(0x10),0x23);assert!(!h.lcd_reset());
        h.aw_write(3,2);assert!(h.lcd_reset());h.aw_write(5,4);h.touch_level(false);assert!(h.irq());assert_eq!(h.aw_read(1)&4,0);assert!(!h.irq());
        h.aw_write(7,4);h.touch_level(true);assert!(!h.irq());h.aw_write(7,0);h.touch_level(false);assert!(h.irq());
        h.aw_write(0x7f,0);assert!(!h.irq());assert!(!h.lcd_reset());assert!(!h.touch_reset());assert_eq!(h.aw_read(5),0);
    }
    #[test] fn pmic_identity_backlight_and_adc_have_real_enable_registers() {
        let mut h=HostState::new();h.axp_write(3,0);assert_eq!(h.axp_read(3),0x4a);assert!(!h.backlight());h.axp_write(0x90,0x80);assert!(!h.backlight());h.axp_write(0x99,28);assert!(h.backlight());h.axp_write(0x90,0);assert!(!h.backlight());
        assert_eq!(h.axp_read(0x38),0);h.axp_write(0x30,12);assert_eq!(u16::from_be_bytes([h.axp_read(0x38),h.axp_read(0x39)]),5000);assert_eq!(h.axp_read(0)&8,0);
    }
}
