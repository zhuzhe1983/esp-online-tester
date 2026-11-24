//! Individually sourced older Waveshare S3 single-screen SPI boards.
//! Source/evidence boundaries: docs/platform/waveshare-spi-legacy-runtime.md.
//! The GC9A01 has its own 240-square RAM and does not inherit ST7789 RAMCTRL.
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex,atomic::Ordering}};
use esp_periph::i2c::I2cDevice;
use esp_soc::{board::{BoardModel,BoardEdge,SpiQspiPins,VirtualCycle},pins::PinRoutes,
    devices::{DcsPanel,i2c_bitbang::I2cBitBang,imu_motion::ImuMotion,qmi8658::{Qmi8658,Qmi8658Port}}};

const HZ:u64=240_000_000;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum LegacySpiVariant {Lcd128,TouchLcd128}
#[derive(Clone,Copy)]
struct Profile {
    name:&'static str,sku:&'static str,clock:u8,mosi:u8,cs:u8,dc:u8,reset:u8,backlight:u8,
    imu_pins:(u8,u8),imu_address:u8,imu_c:bool,touch:Option<(u8,u8)>,
}
impl LegacySpiVariant {
    fn profile(self)->Profile {match self {
        // SKU26541, schematic U4=QMI8658C / SA0=GND. QST C Rev A
        // §16.3.1 makes that 6Bh, correcting the older reversed inference.
        Self::Lcd128=>Profile{name:"waveshare-s3-lcd128",sku:"ESP32-S3-LCD-1.28",
            clock:10,mosi:11,cs:9,dc:8,reset:12,backlight:40,imu_pins:(6,7),imu_address:Qmi8658::i2c_address(false),imu_c:true,touch:None},
        // SKU25098, Rev3 drawing: CST816S RST13/IRQ5, U3=QMI8658A.
        // GPIO4/5 motor transistor footprints are explicitly NC on this revision.
        Self::TouchLcd128=>Profile{name:"waveshare-s3-touch-lcd128",sku:"ESP32-S3-Touch-LCD-1.28",
            clock:10,mosi:11,cs:9,dc:8,reset:14,backlight:2,imu_pins:(6,7),imu_address:Qmi8658::i2c_address(false),imu_c:false,touch:Some((13,5))},
    }}
    pub fn name(self)->&'static str {self.profile().name}
    pub fn sku(self)->&'static str {self.profile().sku}
    pub fn flash_mib(self)->usize {16}
    pub fn psram_mib(self)->usize {2}
}

// Board-local CST816S subset. The register declaration supplies 01h..06h
// reports, EDh pulse width and FAh masks; no numeric silicon/project/firmware
// ID is published. Leave those unknown rather than borrowing another CST ID.
struct CstState {
    powered:bool,asleep:bool,down:bool,x:u16,y:u16,irq_until:u64,pulse_width:u8,
    irq_mask:Option<u8>,disable_auto_sleep:u8,ptr:u8,first:bool,reads:u64,updates:u64,
}
impl Default for CstState {
    fn default()->Self {Self{powered:true,asleep:false,down:false,x:0,y:0,irq_until:0,pulse_width:10,
        irq_mask:None,disable_auto_sleep:0,ptr:0,first:true,reads:0,updates:0}}
}
impl CstState {
    fn reset(&mut self,powered:bool) {let (reads,updates)=(self.reads,self.updates);*self=Self{powered,reads,updates,..Self::default()};}
    fn irq_high(&self,cycle:u64)->bool {!self.powered||self.asleep||cycle>=self.irq_until}
    fn update(&mut self,cycle:u64,x:u16,y:u16,down:bool) {
        if !self.powered||self.asleep {return;}
        let changed=(self.x,self.y,self.down)!=(x,y,down);self.x=x;self.y=y;self.down=down;self.updates+=1;
        let pulse=self.irq_mask.map_or(changed,|m|m&0x40!=0&&down||m&0x20!=0&&changed);
        if pulse {self.irq_until=cycle.saturating_add(u64::from(self.pulse_width)*HZ/10_000);}
    }
}
struct Cst816s(Arc<Mutex<CstState>>);
impl I2cDevice for Cst816s {
    fn start(&mut self,read:bool)->bool {let mut s=self.0.lock().expect("CST816S state");if !s.powered||s.asleep {return false;}
        if !read {s.first=true;}true}
    fn write(&mut self,b:u8)->bool {
        let mut s=self.0.lock().expect("CST816S state");if !s.powered||s.asleep {return false;}
        if s.first {s.first=false;s.ptr=b;return matches!(b,0x01..=0x06|0x15|0xa5|0xa7..=0xa9|0xed|0xfa|0xfe);}
        match s.ptr {
            0xa5 if b==3=>{s.asleep=true;s.down=false;s.irq_until=0;},
            0xed if (1..=200).contains(&b)=>s.pulse_width=b,
            // Test/gesture/once-long-press modes need a scan/gesture engine.
            0xfa if b&!0x60==0=>s.irq_mask=Some(b),
            0xfe=>s.disable_auto_sleep=b,
            _=>return false,
        }s.ptr=s.ptr.wrapping_add(1);true
    }
    fn read(&mut self)->u8 {
        let mut s=self.0.lock().expect("CST816S state");if !s.powered||s.asleep {return 255;}
        let v=match s.ptr {0x01=>0,0x02=>{s.reads+=1;u8::from(s.down)},0x03=>(s.x>>8)as u8&15,0x04=>s.x as u8,
            0x05=>(s.y>>8)as u8&15,0x06=>s.y as u8,0x15|0xa7..=0xa9=>0,0xed=>s.pulse_width,0xfa=>s.irq_mask.unwrap_or(0),0xfe=>s.disable_auto_sleep,_=>255};
        s.ptr=s.ptr.wrapping_add(1);v
    }
}

pub struct WaveshareLegacySpi {
    p:Profile,panel:DcsPanel,command:u8,argn:u8,scan:u8,reset_high:bool,dc_driven:bool,
    backlight_gpio:bool,backlight:u32,changes:u64,rejected:u64,gpio_events:u64,
    imu:Arc<ImuMotion>,qmi:Rc<RefCell<Qmi8658>>,i2c:I2cBitBang,i2c_initialized:bool,cycle:u64,levels:[bool;3],edges:Vec<BoardEdge>,cst:Arc<Mutex<CstState>>,
}
impl WaveshareLegacySpi {
    pub fn new(variant:LegacySpiVariant)->Self {
        let panel=DcsPanel::new(240,240);let p=variant.profile();let imu=Arc::new(ImuMotion::default());
        let qmi=Rc::new(RefCell::new(if p.imu_c {Qmi8658::new_c(imu.clone(),HZ)}else{Qmi8658::new(imu.clone(),HZ)}));
        Self{p,panel,command:0,argn:0,scan:0,reset_high:false,dc_driven:false,
            backlight_gpio:false,backlight:0,changes:0,rejected:0,gpio_events:0,
            imu,qmi,i2c:I2cBitBang::new(),i2c_initialized:false,cycle:0,levels:[true;3],edges:Vec::new(),cst:Default::default()}
    }
    fn supported_command(c:u8)->bool {matches!(c,0x10|0x11|0x20|0x21|0x28|0x29|0x2a|0x2b|0x2c|0x36|0x3a)}
    fn reset_panel(&mut self) {self.panel.reset();self.command=0;self.argn=0;self.scan=0;self.changes=self.changes.wrapping_add(1);}
    fn sync_touch_irq(&mut self) {if let Some((_,pin))=self.p.touch {let level=self.cst.lock().expect("CST816S state").irq_high(self.cycle);
        if level!=self.levels[2] {self.levels[2]=level;self.edges.push(BoardEdge{cycle:self.cycle,pin,level});}}}
}
impl BoardModel for WaveshareLegacySpi {
    fn name(&self)->&'static str {self.p.name}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {self.gpio_events=self.gpio_events.wrapping_add(changes.len()as u64);}
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,r:PinRoutes<'_>) {
        self.advance_to(cycle);let p=self.p;
        let reset=r.software_output(p.reset)==Some(true);
        if reset!=self.reset_high {if !reset {self.reset_panel();}self.reset_high=reset;self.changes=self.changes.wrapping_add(1);}
        let dc=r.software_output(p.dc);self.dc_driven=dc.is_some();self.panel.dc=dc.unwrap_or(false);
        self.backlight_gpio=r.software_output(p.backlight)==Some(true);
        let (sda,scl)=p.imu_pins;let clock=r.software_output(scl).unwrap_or(true);
        let data=self.i2c.drive(clock,r.software_output(sda).unwrap_or(true));
        for (i,(pin,level)) in [(sda,data),(scl,clock)].into_iter().enumerate() {
            if level!=self.levels[i] {self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}
        }
        if let Some((pin,_))=p.touch {
            // CST816S datasheet 4.3: RSTn has an internal pull-up.
            let powered=r.software_output(pin).unwrap_or(true);let mut state=self.cst.lock().expect("CST816S state");
            if powered!=state.powered {state.reset(powered);}drop(state);self.sync_touch_irq();
        }
    }
    fn uses_gpio_pwm(&self)->bool {true}
    fn gpio_pwm_at(&mut self,cycle:VirtualCycle,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {
        self.advance_to(cycle);let duty=pwm(self.p.backlight).map_or(if self.backlight_gpio {65535}else{0},|(_,d)|d.min(65535));
        if duty!=self.backlight {self.backlight=duty;self.changes=self.changes.wrapping_add(1);}
    }
    // Opt in to the richer phase snapshot to reject accidental multi-lane SPI.
    // This board remains a single-data-wire command/data display.
    fn uses_qspi_pins(&self)->bool {true}
    fn spi_transfer_qspi_pins(&mut self,_host:u8,pins:SpiQspiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        let p=self.p;if pins.cs&(1<<p.cs)==0 {return vec![255;rx_len];}
        if pins.unsupported_mode||pins.sio||[pins.command_width,pins.address_width,pins.data_width,pins.read_width].iter().any(|w|*w>1)
            ||pins.address_bits!=0||!pins.command_bits.is_multiple_of(8)||!pins.data_bits.is_multiple_of(8)||pins.dummy_cycles!=0
            ||pins.sclk&(1<<p.clock)==0||pins.data[0]&(1<<p.mosi)==0||!self.dc_driven||!self.reset_high {
            self.rejected=self.rejected.wrapping_add(1);return vec![255;rx_len];
        }
        for &v in tx {
            if !self.panel.dc {self.command=v;self.argn=0;self.changes=self.changes.wrapping_add(1);}
            if self.command==0xb6 {if self.panel.dc {if self.argn==1 {self.scan=v&0x60;self.changes=self.changes.wrapping_add(1);}self.argn=self.argn.saturating_add(1);}continue;}
            if !Self::supported_command(self.command) {continue;}
            if self.panel.dc&&self.command==0x2c&&self.panel.colmod&7!=5 {self.rejected=self.rejected.wrapping_add(1);continue;}
            self.panel.byte(v);
        }
        // LCD SDO readback is unimplemented (Touch PCB routes it to GPIO12).
        // No fabricated chip IDs or analog voltage/gamma results.
        vec![255;rx_len]
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        // Digital MCU reboot does not remove the external sensor's 3V3 supply.
        // Reattach controller views to the same device state, without a fake POR.
        if !self.i2c_initialized {
            self.i2c.attach(self.p.imu_address,Box::new(Qmi8658Port::new(self.qmi.clone())));
            if self.p.touch.is_some() {self.i2c.attach(0x15,Box::new(Cst816s(self.cst.clone())));}
            self.i2c_initialized=true;
        }
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some(self.p.imu_pins)).into_iter().map(move|(a,d)|(bus,a,d))).collect()
    }
    fn advance_to(&mut self,cycle:VirtualCycle) {self.cycle=cycle;self.imu.cycle.store(cycle,Ordering::Relaxed);self.sync_touch_irq();}
    fn next_deadline(&self)->Option<VirtualCycle> {if self.p.touch.is_some() {let s=self.cst.lock().expect("CST816S state");if !s.irq_high(self.cycle) {return Some(s.irq_until);}}None}
    fn touch(&mut self,x:u16,y:u16,down:bool) {if self.p.touch.is_some() {self.cst.lock().expect("CST816S state").update(self.cycle,x.min(239),y.min(239),down);self.sync_touch_irq();}}
    fn touch_at(&mut self,cycle:VirtualCycle,x:u16,y:u16,down:bool) {self.advance_to(cycle);self.touch(x,y,down);}
    fn set_imu_motion(&mut self,mode:u32)->bool {self.qmi.borrow_mut().set_motion_mode(mode)}
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {self.qmi.borrow_mut().set_host_sample(accel_g,gyro_dps)}
    fn imu_motion(&self)->u32 {self.imu.mode.load(Ordering::Relaxed)}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn input_levels(&self)->Vec<(u8,bool)> {let mut pins=vec![(0,true),(self.p.imu_pins.0,self.levels[0]),(self.p.imu_pins.1,self.levels[1])];if let Some((_,irq))=self.p.touch {pins.push((irq,self.levels[2]));}pins}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"boot"|"button"=>Some(0),_=>None}}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let mut frame=vec![0;240*240];
        if self.reset_high&&self.panel.on&&!self.panel.sleeping&&self.backlight>0 {
            for y in 0..240 {for x in 0..240 {
                // Circular pixel-center aperture, not a measured optical edge.
                let dx=2*x as i32-239;let dy=2*y as i32-239;if dx*dx+dy*dy>240*240 {continue;}
                // GC9A01 pp73/158: B6 GS/SS change RAM-to-output scan, separate
                // from MADCTL's host-to-RAM address order. Board reference pose
                // is the official older init B6=00,20 / MADCTL=08. This is a
                // digital reference, not measured panel/RGB optical calibration.
                let col=if self.scan&0x20==0 {239-x}else{x};let row=if self.scan&0x40!=0 {239-y}else{y};
                let mut value=self.panel.gram[row*240+col];
                if (self.panel.madctl&8!=0)!=(self.scan&0x20!=0) {value=(value&0x07e0)|((value&0xf800)>>11)|((value&31)<<11);}
                if !self.panel.inverted {value^=0xffff;}
                let v=u32::from(value);let d=self.backlight;
                frame[y*240+x]=((((v>>11)&31)*d/65535)<<11|(((v>>5)&63)*d/65535)<<5|((v&31)*d/65535))as u16;
            }}
        }
        Some((240,240,frame,self.display_version()))
    }
    fn display_frames(&self)->u64 {self.panel.frames}
    fn display_version(&self)->u64 {self.panel.pixels_written.wrapping_add(self.panel.resets).wrapping_add(self.changes)}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),240,240))}
    fn report(&self)->String {let touch=if let Some((reset,irq))=self.p.touch {let s=self.cst.lock().expect("CST816S state");format!("CST816S@15 RST={reset} IRQ={irq} ready={} sleep={} reports={} hostUpdates={} pulse={}x0.1ms",s.powered,s.asleep,s.reads,s.updates,s.pulse_width)}else{"false".into()};
        let mut report=format!("{}: GC9A01A 240x240 CLK={} MOSI={} CS={} DC={} RST={} BL={} RAMWR={} pixels={} rejected={} madctl={:02x} colmod={:02x} scan={:02x} on={} sleep={} reset={} backlight={} I2C={:?} QMI8658{}@{:02x} motion={}; physical TF=false touch={touch}. Digital RGB565/common-DCS+B6 scan only; unsupported voltage/gamma/TE/scroll/18bit/readback/reset-pulse timing, analog battery/charger, QMI interrupts/calibration, CST gestures/scan cadence/auto-sleep and RF. CST IDs and reset FA mask unknown. Logical circular aperture, glass pose and linear brightness are uncalibrated.",
        self.p.sku,self.p.clock,self.p.mosi,self.p.cs,self.p.dc,self.p.reset,self.p.backlight,self.panel.frames,self.panel.pixels_written,self.rejected,self.panel.madctl,self.panel.colmod,self.scan,self.panel.on,self.panel.sleeping,self.reset_high,self.backlight,self.p.imu_pins,if self.p.imu_c {"C"}else{"A"},self.p.imu_address,self.imu_motion());
        if self.qmi.borrow().host_sample_active() {report.push_str(" Host IMU sample held in sensor axes; physical PCB pose uncalibrated.");}report}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn route(b:&mut WaveshareLegacySpi,dc:bool,reset:bool,light:bool) {
        let mut gpio=esp_periph::Gpio::new();let mut mux=esp_periph::RegRam::new();
        for (pin,high) in [(b.p.dc,dc),(b.p.reset,reset),(b.p.backlight,light)] {gpio.enable|=1<<pin;if high {gpio.out|=1<<pin;}gpio.func_out_sel[pin as usize]=256;mux.write(4+4*u32::from(pin),1<<12);}
        b.gpio_waveform_at(b.cycle,esp_soc::pins::ChipPins::S3.routes(&gpio,&mux));b.gpio_pwm_at(b.cycle,&|_|None);
    }
    fn pins(b:&WaveshareLegacySpi)->SpiQspiPins {SpiQspiPins{sclk:1<<b.p.clock,data:[1<<b.p.mosi,0,0,0],cs:1<<b.p.cs,data_width:1,..Default::default()}}
    fn cmd(b:&mut WaveshareLegacySpi,c:u8,args:&[u8]) {route(b,false,true,true);b.spi_transfer_qspi_pins(2,pins(b),&[c],0);route(b,true,true,true);b.spi_transfer_qspi_pins(2,pins(b),args,0);}
    #[test]
    fn lcd128_exact_routes_do_not_enable_st7789_endian_or_unwired_touch() {
        let mut b=WaveshareLegacySpi::new(LegacySpiVariant::Lcd128);assert_eq!(b.name(),LegacySpiVariant::Lcd128.name());assert_eq!(b.display().unwrap().2,vec![0;240*240]);
        for c in [0x11,0x29,0x21] {cmd(&mut b,c,&[]);}cmd(&mut b,0x36,&[8]);cmd(&mut b,0xb6,&[0,0x20]);cmd(&mut b,0x3a,&[5]);
        cmd(&mut b,0xb0,&[0,0xe8]);assert!(!b.panel.little_endian,"GC9A01 B0 is RGB signal control, not ST7789 endian");
        cmd(&mut b,0x2a,&[0,100,0,100]);cmd(&mut b,0x2b,&[0,120,0,120]);cmd(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[120*240+100],0xf800);
        let before=b.panel.pixels_written;let good=pins(&b);
        for bad in [SpiQspiPins{sclk:1<<9,..good},SpiQspiPins{data:[1<<12,0,0,0],..good},SpiQspiPins{data_width:4,..good},SpiQspiPins{dummy_cycles:1,..good},SpiQspiPins{command_bits:1,..good}] {b.spi_transfer_qspi_pins(2,bad,&[0,255],0);}
        assert_eq!(before,b.panel.pixels_written);route(&mut b,true,true,false);assert!(b.display().unwrap().2.iter().all(|v|*v==0));route(&mut b,true,false,true);assert!(!b.panel.on);
        let devices=b.i2c_devices();assert_eq!(devices.len(),2);assert!(devices.iter().all(|(_,a,d)|*a==0x6b&&d.pins()==Some((6,7))));
        assert_eq!(b.named_pin("boot"),Some(0));assert_eq!(b.named_pin("touch"),None);
    }
    #[test]
    fn gc9_scan_changes_output_without_rewriting_ram_or_enabling_st_ramctrl() {
        let mut b=WaveshareLegacySpi::new(LegacySpiVariant::TouchLcd128);
        for c in [0x11,0x29,0x21] {cmd(&mut b,c,&[]);}cmd(&mut b,0x36,&[8]);cmd(&mut b,0xb6,&[0,0x20]);cmd(&mut b,0x3a,&[5]);
        cmd(&mut b,0x2a,&[0,100,0,100]);cmd(&mut b,0x2b,&[0,120,0,120]);cmd(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[120*240+100],0xf800);
        let ram=b.panel.gram.clone();let written=b.panel.pixels_written;
        cmd(&mut b,0xb6,&[0,0]);assert_eq!(b.display().unwrap().2[120*240+139],31,"SS reverses source outputs including R/B assignment");
        cmd(&mut b,0x36,&[0]);assert_eq!(b.display().unwrap().2[120*240+139],0xf800);
        cmd(&mut b,0xb6,&[0,0x40]);assert_eq!(b.display().unwrap().2[119*240+139],0xf800);
        assert_eq!(b.panel.gram,ram);assert_eq!(b.panel.pixels_written,written);route(&mut b,true,false,true);assert_eq!(b.scan,0);
    }
    #[test]
    fn touch128_rev3_routes_real_reset_pullup_reports_and_timed_irq() {
        let mut b=WaveshareLegacySpi::new(LegacySpiVariant::TouchLcd128);assert_eq!((b.p.reset,b.p.backlight,b.p.touch,b.p.imu_c),(14,2,Some((13,5)),false));
        let mut devs=b.i2c_devices();assert_eq!(devs.len(),4);assert!(devs.iter().all(|(_,_,d)|d.pins()==Some((6,7))));
        let d=devs.iter_mut().find(|(bus,addr,_)|(*bus,*addr)==(0,0x15)).unwrap().2.as_mut();
        let read=|d:&mut dyn I2cDevice,r,n|{assert!(d.start(false));assert!(d.write(r));assert!(d.start(true));let v=(0..n).map(|_|d.read()).collect::<Vec<_>>();d.stop();v};
        b.touch_at(100,100,120,true);assert_eq!(b.next_deadline(),Some(100+HZ/1000));assert!(b.take_edges().iter().any(|e|e.pin==5&&!e.level));
        assert_eq!(read(d,1,6),[0,1,0,100,0,120]);assert_eq!(b.next_deadline(),Some(100+HZ/1000),"report reads do not truncate the physical pulse");
        b.advance_to(100+HZ/1000);assert!(b.take_edges().iter().any(|e|e.pin==5&&e.level));assert_eq!(b.next_deadline(),None);assert_eq!(read(d,2,1),[1],"report remains valid after pulse ends");
        assert_eq!(read(d,0xa7,3),[0;3]);assert!(d.start(false));assert!(d.write(0xa7));assert!(!d.write(0xb5));d.stop();
        assert!(d.start(false));assert!(d.write(0xed));assert!(!d.write(0));assert!(d.write(20));d.stop();
        b.touch_at(2*HZ/1000,110,130,false);assert_eq!(b.next_deadline(),Some(4*HZ/1000));assert_eq!(read(d,2,5),[0,0,110,0,130]);
        assert!(d.start(false));assert!(d.write(0xfa));assert!(!d.write(0x80));d.stop();
        assert!(d.start(false));assert!(d.write(0xa5));assert!(d.write(3));d.stop();assert!(!d.start(true));
        let mut gpio=esp_periph::Gpio::new();let mut mux=esp_periph::RegRam::new();gpio.enable=1<<13;gpio.func_out_sel[13]=256;mux.write(4+13*4,1<<12);
        b.gpio_waveform_at(5*HZ/1000,esp_soc::pins::ChipPins::S3.routes(&gpio,&mux));assert!(!d.start(false));
        gpio.enable=0;b.gpio_waveform_at(6*HZ/1000,esp_soc::pins::ChipPins::S3.routes(&gpio,&mux));assert_eq!(read(d,0xed,1),[10],"released reset uses the documented internal pull-up");assert_eq!(read(d,2,1),[0]);
        let mut input=esp_periph::Gpio::new();input.set_input(5,true);input.set_pin_config(5,0x2100);b.take_edges();
        b.touch_at(7*HZ/1000,90,80,true);assert!(esp_soc::gpio::drain_board_inputs(&mut b,&mut input,&mut None,7*HZ/1000));
        assert!(!input.level(5));assert!(input.irq(),"physical falling edge reaches the GPIO interrupt peripheral");
    }
    #[test]
    fn qmi_uses_grounded_address_and_shared_register_state_on_both_masters() {
        let mut b=WaveshareLegacySpi::new(LegacySpiVariant::Lcd128);let mut devices=b.i2c_devices();b.advance_to(20*HZ/1000);
        let a=devices.iter_mut().find(|(bus,_,_)|*bus==0).unwrap().2.as_mut();assert!(a.start(false));assert!(a.write(2));assert!(a.write(0x40));a.stop();
        let d=devices.iter_mut().find(|(bus,_,_)|*bus==1).unwrap().2.as_mut();assert!(d.start(false));assert!(d.write(2));assert!(d.start(true));assert_eq!(d.read(),0x40);d.stop();
        assert!(d.start(false));assert!(d.write(0));assert!(d.start(true));assert_eq!(d.read(),5);d.stop();
        // MCU reset reattaches its buses without removing the sensor supply.
        let mut reattached=b.i2c_devices();assert_eq!(reattached.len(),2);
        assert!(reattached.iter().all(|(_,addr,_)|*addr==0x6b));
        let d=reattached.iter_mut().find(|(bus,_,_)|*bus==0).unwrap().2.as_mut();
        assert!(d.start(false));assert!(d.write(2));assert!(d.start(true));assert_eq!(d.read(),0x40);d.stop();
        assert!(b.set_imu_motion(1));assert_eq!(b.imu_motion(),1);assert!(!b.set_imu_motion(2));
    }
    fn factory(variant:LegacySpiVariant) {
        use esp_soc::{LoadKind,SocBus,Stop};
        let source=std::env::var("ESP32SIM_WAVESHARE_LEGACY_SOURCES").expect("original pinned <SKU>/firmware/FactoryOnly image cache required");
        let rom=std::env::var("ESP32SIM_ROM_DIR").expect("original ESP32-S3 mask-ROM directory required");
        let filename=match variant {LegacySpiVariant::Lcd128=>"ESP32-S3-LCD-1.28_FactoryOnly_20250909.bin",LegacySpiVariant::TouchLcd128=>"ESP32-S3-Touch-LCD-1.28_FactoryOnly_20251120.bin"};
        let make=||{let mut m=crate::machine([2,0,0,0,0,1]);m.bus.board=Box::new(WaveshareLegacySpi::new(variant));m.bus.attach_board_devices();
            m.bus.set_flash_size(variant.flash_mib()<<20);m.bus.set_psram_size(variant.psram_mib()<<20).unwrap();m.console.capture=true;m.console.mask=2;
            m.load_input(LoadKind::Rom,&std::fs::read(format!("{rom}/esp32s3_rev0_rom.elf")).unwrap()).unwrap();
            m.load_input(LoadKind::Flash,&std::fs::read(format!("{source}/{}/firmware/{filename}",variant.sku())).unwrap()).unwrap();m.boot_rom();m};
        let mut m=make();
        let run=|m:&mut crate::Machine,ms:u64|{m.max_cycles=m.bus.cycles+ms*HZ/1000;let stop=m.run(u64::MAX);m.bus.refresh_board_outputs();
            eprintln!("{} time={}ms stop={stop:?} reboots={} {}",variant.sku(),m.bus.cycles/(HZ/1000),m.reboots,m.bus.board.report());assert!(matches!(stop,Stop::Halted));};
        let save=|m:&mut crate::Machine,label:&str|{if let Ok(root)=std::env::var("ESP32SIM_WAVESHARE_LEGACY_EVIDENCE") {let p=std::path::Path::new(&root).join(variant.sku());std::fs::create_dir_all(&p).unwrap();let(w,h,pixels,_)=m.bus.board.display().unwrap();
            std::fs::write(p.join(format!("{label}.rgb565")),pixels.iter().flat_map(|v|v.to_le_bytes()).collect::<Vec<_>>()).unwrap();m.write_tft_png(p.join(format!("{label}.png")).to_str().unwrap(),1).unwrap();
            std::fs::write(p.join(format!("{label}.uart.txt")),&m.console.all).unwrap();std::fs::write(p.join(format!("{label}.board.txt")),m.bus.board.report()).unwrap();
            std::fs::write(p.join(format!("{label}.state.json")),format!("{{\"guestCycles\":{},\"guestMilliseconds\":{},\"reboots\":{},\"width\":{w},\"height\":{h},\"ramwrCommands\":{},\"corePc\":[{},{}]}}\n",m.bus.cycles,m.bus.cycles/(HZ/1000),m.reboots,m.bus.board.display_frames(),m.cores[0].pc,m.cores[1].pc)).unwrap();}};
        let mut control=make();for ms in [5000,1000,150,850] {run(&mut control,ms);}save(&mut control,"matched-boot-control");let control_button=control.bus.board.display().unwrap().2;
        run(&mut control,1000);save(&mut control,"matched-motion-control");let control_motion=control.bus.board.display().unwrap().2;drop(control);
        run(&mut m,5000);save(&mut m,"boot");let boot=m.bus.board.display().unwrap().2;
        run(&mut m,1000);save(&mut m,"quiet");let quiet=m.bus.board.display().unwrap().2;
        m.bus.gpio_set_input(0,false);run(&mut m,150);m.bus.gpio_set_input(0,true);run(&mut m,850);save(&mut m,"boot-button");let button=m.bus.board.display().unwrap().2;
        assert!(m.bus.board.set_imu_motion(1));run(&mut m,1000);save(&mut m,"motion");let motion=m.bus.board.display().unwrap().2;
        eprintln!("{} PIXELS quiet={} boot={} motion={}",variant.sku(),boot.iter().zip(&quiet).filter(|(a,b)|a!=b).count(),quiet.iter().zip(&button).filter(|(a,b)|a!=b).count(),button.iter().zip(&motion).filter(|(a,b)|a!=b).count());
        eprintln!("{} MATCHED boot={} motion={}",variant.sku(),control_button.iter().zip(&button).filter(|(a,b)|a!=b).count(),control_motion.iter().zip(&motion).filter(|(a,b)|a!=b).count());
        assert_eq!(m.reboots,0);assert!(m.bus.board.display_frames()>0,"original image has not initialized its display");
    }
    #[test] #[ignore="external original FactoryOnly_20250909 and S3 ROM required"]
    fn external_lcd128_factory(){factory(LegacySpiVariant::Lcd128);}
    #[test] #[ignore="external original FactoryOnly_20251120 and S3 ROM required"]
    fn external_touch128_initial() {
        use esp_soc::{LoadKind,SocBus,Stop};let v=LegacySpiVariant::TouchLcd128;
        let source=std::env::var("ESP32SIM_WAVESHARE_LEGACY_SOURCES").unwrap();let rom=std::env::var("ESP32SIM_ROM_DIR").unwrap();
        let mut m=crate::machine([2,0,0,0,0,1]);m.bus.board=Box::new(WaveshareLegacySpi::new(v));m.bus.attach_board_devices();
        m.bus.set_flash_size(v.flash_mib()<<20);m.bus.set_psram_size(v.psram_mib()<<20).unwrap();m.console.capture=true;m.console.mask=2;
        m.load_input(LoadKind::Rom,&std::fs::read(format!("{rom}/esp32s3_rev0_rom.elf")).unwrap()).unwrap();
        m.load_input(LoadKind::Flash,&std::fs::read(format!("{source}/{}/firmware/ESP32-S3-Touch-LCD-1.28_FactoryOnly_20251120.bin",v.sku())).unwrap()).unwrap();m.boot_rom();
        for ms in [2000,4000,5000,6000,8000] {m.max_cycles=ms*HZ/1000;let stop=m.run(u64::MAX);m.bus.refresh_board_outputs();
            eprintln!("{} time={ms}ms stop={stop:?} reboots={} {}",v.sku(),m.reboots,m.bus.board.report());assert!(matches!(stop,Stop::Halted));
            if let Ok(root)=std::env::var("ESP32SIM_WAVESHARE_LEGACY_EVIDENCE") {let p=std::path::Path::new(&root).join(v.sku());std::fs::create_dir_all(&p).unwrap();
                m.write_tft_png(p.join(format!("initial-{ms}.png")).to_str().unwrap(),1).unwrap();std::fs::write(p.join(format!("initial-{ms}.uart.txt")),&m.console.all).unwrap();
                std::fs::write(p.join(format!("initial-{ms}.board.txt")),m.bus.board.report()).unwrap();}}
        assert_eq!(m.reboots,0);assert!(m.bus.board.display_frames()>0);
    }
    #[test] #[ignore="external original FactoryOnly_20251120 and S3 ROM required"]
    fn external_touch128_factory() {
        use esp_soc::{LoadKind,SocBus,Stop};let v=LegacySpiVariant::TouchLcd128;
        let source=std::env::var("ESP32SIM_WAVESHARE_LEGACY_SOURCES").unwrap();let rom=std::env::var("ESP32SIM_ROM_DIR").unwrap();
        let make=||{let mut m=crate::machine([2,0,0,0,0,1]);m.bus.board=Box::new(WaveshareLegacySpi::new(v));m.bus.attach_board_devices();
            m.bus.set_flash_size(v.flash_mib()<<20);m.bus.set_psram_size(v.psram_mib()<<20).unwrap();m.console.capture=true;m.console.mask=2;
            m.load_input(LoadKind::Rom,&std::fs::read(format!("{rom}/esp32s3_rev0_rom.elf")).unwrap()).unwrap();
            m.load_input(LoadKind::Flash,&std::fs::read(format!("{source}/{}/firmware/ESP32-S3-Touch-LCD-1.28_FactoryOnly_20251120.bin",v.sku())).unwrap()).unwrap();m.boot_rom();m};
        let run=|m:&mut crate::Machine,ms:u64|{m.max_cycles=m.bus.cycles+ms*HZ/1000;assert!(matches!(m.run(u64::MAX),Stop::Halted));m.bus.refresh_board_outputs();};
        let save=|m:&mut crate::Machine,label:&str|{eprintln!("{} {label} time={}ms reboots={} {}",v.sku(),m.bus.cycles/(HZ/1000),m.reboots,m.bus.board.report());
            if let Ok(root)=std::env::var("ESP32SIM_WAVESHARE_LEGACY_EVIDENCE") {let p=std::path::Path::new(&root).join(v.sku());std::fs::create_dir_all(&p).unwrap();let(w,h,pixels,_)=m.bus.board.display().unwrap();
                std::fs::write(p.join(format!("{label}.rgb565")),pixels.iter().flat_map(|v|v.to_le_bytes()).collect::<Vec<_>>()).unwrap();m.write_tft_png(p.join(format!("{label}.png")).to_str().unwrap(),1).unwrap();
                std::fs::write(p.join(format!("{label}.uart.txt")),&m.console.all).unwrap();std::fs::write(p.join(format!("{label}.board.txt")),m.bus.board.report()).unwrap();
                std::fs::write(p.join(format!("{label}.state.json")),format!("{{\"guestCycles\":{},\"guestMilliseconds\":{},\"reboots\":{},\"width\":{w},\"height\":{h},\"ramwrCommands\":{},\"corePc\":[{},{}],\"touchIrqPinConfig\":{},\"touchResetDrivenHigh\":{}}}\n",m.bus.cycles,m.bus.cycles/(HZ/1000),m.reboots,m.bus.board.display_frames(),m.cores[0].pc,m.cores[1].pc,m.bus.periph.gpio.pin[5],m.bus.periph.gpio.enable&(1<<13)!=0&&m.bus.periph.gpio.out&(1<<13)!=0)).unwrap();}};
        let swipe=|m:&mut crate::Machine,input:bool|{if input {m.bus.touch_input(120,200,true);}run(m,40);
            for i in 1..=20 {if input {m.bus.touch_input(120,200-6*i,true);}run(m,20);}
            if input {m.bus.touch_input(120,80,false);}run(m,60);run(m,400);};
        let mut control=make();run(&mut control,2000);run(&mut control,200);swipe(&mut control,false);save(&mut control,"matched-scroll-control");let reference=control.bus.board.display().unwrap().2;drop(control);
        let mut m=make();run(&mut m,2000);save(&mut m,"boot");let boot=m.bus.board.display().unwrap().2;run(&mut m,200);save(&mut m,"quiet");let quiet=m.bus.board.display().unwrap().2;
        swipe(&mut m,true);save(&mut m,"scroll");let scroll=m.bus.board.display().unwrap().2;
        run(&mut m,4900);save(&mut m,"late-before");let before=m.bus.board.display().unwrap().2;m.bus.touch_input(120,120,true);run(&mut m,100);m.bus.touch_input(120,120,false);run(&mut m,300);save(&mut m,"late-after");let after=m.bus.board.display().unwrap().2;
        let count=|a:&[u16],b:&[u16]|a.iter().zip(b).filter(|(a,b)|a!=b).count();
        eprintln!("{} MATCHED quiet={} early_scroll={} late_touch={}",v.sku(),count(&boot,&quiet),count(&reference,&scroll),count(&before,&after));
        assert_eq!(m.reboots,0);assert!(m.bus.board.display_frames()>0);
    }
}
