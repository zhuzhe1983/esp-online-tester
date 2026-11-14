//! Metalio INK4 Plus, current GPIO4/XSTL and GPIO19/MODE wiring. Sources:
//! CloudZao/Metalio-E-INK4-Plus 14aa1d06, boards/metalio-e-ink4-plus/config.h,
//! display/epd_board_metalio_eink4_plus.c and display/metalio_touch.c.
//! Digital power, touch and e-paper subset; no analog voltage/ramp/gray calibration.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,tca9555::{Tca9555,Tca9555State}};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,LcdRgbPins};

const WIDTH:usize=1216;
const HEIGHT:usize=684;

/// The public source specifies 16 MiB NOR and octal PSRAM, not its capacity or
/// NOR JEDEC part. Capacity is an explicit host configuration; the generic
/// Winbond identity/regi2c seeds are emulator choices, not a physical-board dump.
pub fn inputs(psram_bytes:usize)->crate::soc::BoardInputs {
    crate::soc::BoardInputs{psram_bytes,inputs:Vec::new(),..crate::soc::BoardInputs::devkit_w25q128()}
}

#[derive(Default)]
struct TouchState {powered:bool,down:bool,x:u16,y:u16}
struct Wiring {expander:Arc<Mutex<Tca9555State>>,touch:Arc<Mutex<TouchState>>}
impl Wiring {
    fn rails(&self)->u16 {self.expander.lock().expect("INK4 Plus expander poisoned").driven_high()}
    fn sync(&self) {
        let mut exp=self.expander.lock().expect("INK4 Plus expander poisoned");let high=exp.driven_high();
        // Nominal healthy TPS rails: PWRGOOD follows WAKEUP+PWRUP, not an
        // unconditional ACK. Analog settling/rail faults are not modeled.
        exp.set_input(7,high&6==6);
        let mut t=self.touch.lock().expect("INK4 Plus touch poisoned");t.powered=high&0x420==0x420;if !t.powered {t.down=false;}
    }
}
struct Expander {inner:Tca9555,wiring:Arc<Wiring>}
impl I2cDevice for Expander {
    fn pins(&self)->Option<(u8,u8)> {Some((0,1))}
    fn start(&mut self,r:bool)->bool {self.wiring.sync();self.inner.start(r)}
    fn write(&mut self,b:u8)->bool {let ack=self.inner.write(b);self.wiring.sync();ack}
    fn read(&mut self)->u8 {self.wiring.sync();self.inner.read()}
    fn stop(&mut self) {self.inner.stop();}
}
struct Touch {state:Arc<Mutex<TouchState>>,regs:[u8;256],ptr:u8,first:bool}
impl Touch {
    fn new(state:Arc<Mutex<TouchState>>)->Self {
        let mut regs=[0;256];regs[0xa3]=0x64;regs[0x9f]=0x26;
        // Model the published v04 application report protocol, not the FT MCU
        // instruction set/upgrader. IDs/version: ft6336u_upgrade_protocol.h.
        regs[0xa6]=4;Self{state,regs,ptr:0,first:true}
    }
}
impl I2cDevice for Touch {
    fn pins(&self)->Option<(u8,u8)> {Some((0,1))}
    fn start(&mut self,r:bool)->bool {if !r {self.first=true;}self.state.lock().expect("INK4 Plus touch poisoned").powered}
    fn write(&mut self,b:u8)->bool {
        if self.first {self.ptr=b;self.first=false;}else{if !matches!(self.ptr,2..=8|0x9f|0xa3|0xa6) {self.regs[usize::from(self.ptr)]=b;}self.ptr=self.ptr.wrapping_add(1);}true
    }
    fn read(&mut self)->u8 {
        let t=self.state.lock().expect("INK4 Plus touch poisoned");let visible=t.powered&&t.down&&self.regs[0xa5]!=3;
        let value=match self.ptr {2=>u8::from(visible),3=>0x80|((t.x>>8) as u8&15),4=>t.x as u8,5=>(t.y>>8) as u8&15,6=>t.y as u8,7=>u8::from(visible),8=>0,_=>self.regs[usize::from(self.ptr)]};
        self.ptr=self.ptr.wrapping_add(1);value
    }
}
struct Tps {wiring:Arc<Wiring>,regs:[u8;17],ptr:u8,first:bool}
impl Tps {fn new(wiring:Arc<Wiring>)->Self {Self{wiring,regs:[0;17],ptr:0,first:true}}}
impl I2cDevice for Tps {
    fn pins(&self)->Option<(u8,u8)> {Some((0,1))}
    fn start(&mut self,r:bool)->bool {if !r {self.first=true;}self.wiring.rails()&4!=0}
    fn write(&mut self,b:u8)->bool {
        if self.first {self.ptr=b;self.first=false;}else{if let Some(r)=self.regs.get_mut(usize::from(self.ptr)) {*r=if self.ptr==13&&b&0x80!=0 {0x20}else{b};}self.ptr=self.ptr.wrapping_add(1);}true
    }
    fn read(&mut self)->u8 {
        let v=match self.ptr {0=>25,15=>if self.wiring.rails()&6==6 {0xfa}else{0},16=>0,_=>self.regs.get(usize::from(self.ptr)).copied().unwrap_or(0)};
        self.ptr=self.ptr.wrapping_add(1);v
    }
}
struct WiredDevice(Box<dyn I2cDevice>);
impl I2cDevice for WiredDevice {
    fn pins(&self)->Option<(u8,u8)> {Some((0,1))}
    fn start(&mut self,r:bool)->bool {self.0.start(r)}
    fn write(&mut self,b:u8)->bool {self.0.write(b)}
    fn read(&mut self)->u8 {self.0.read()}
    fn stop(&mut self) {self.0.stop();}
}

pub struct Ink4Plus {
    wiring:Arc<Wiring>,frame:Vec<u16>,version:u64,frames:u64,cycle:u64,
    gpio_events:u64,output:u64,enabled:u64,irq:[bool;2],edges:Vec<BoardEdge>,
    rgb_routes:bool,scan_pending:bool,scan:bool,
}
impl Ink4Plus {
    pub fn new()->Self {
        let b=Self{wiring:Arc::new(Wiring{expander:Default::default(),touch:Default::default()}),frame:vec![0xffff;WIDTH*HEIGHT],version:0,frames:0,cycle:0,gpio_events:0,output:0,enabled:0,irq:[true;2],edges:Vec::new(),rgb_routes:false,scan_pending:false,scan:false};b.wiring.sync();b
    }
    fn powered(&self)->bool {self.wiring.rails()&0x27==0x27 && self.output&self.enabled&(1<<18)!=0}
    fn sync_irq(&mut self) {
        self.wiring.sync();let levels=[self.wiring.expander.lock().expect("INK4 Plus expander poisoned").irq_high(),!self.wiring.touch.lock().expect("INK4 Plus touch poisoned").down];
        for (i,&level) in levels.iter().enumerate() {if level!=self.irq[i] {self.irq[i]=level;self.edges.push(BoardEdge{cycle:self.cycle,pin:if i==0 {2}else{5},level});}}
    }
}
impl BoardModel for Ink4Plus {
    fn name(&self)->&'static str {"metalio-ink4-plus"}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {self.gpio_events+=changes.len() as u64;for &(pin,high) in changes {if high {self.output|=1u64<<pin;}else{self.output&=!(1u64<<pin);}}}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        let old=self.output;self.cycle=cycle;self.gpio_changes(changes);self.output=output;self.enabled=enabled;
        if old&(1<<35)!=0 && output&(1<<35)==0 && enabled&(1<<35)!=0 {self.scan_pending=true;}
        if !self.powered() {self.scan=false;}
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(2,self.irq[0]),(5,self.irq[1]),(61,true)]}
    fn advance_to(&mut self,cycle:VirtualCycle) {self.cycle=cycle;self.sync_irq();}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.sync_irq();std::mem::take(&mut self.edges)}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        vec![(1,0x20,Box::new(Expander{inner:Tca9555::new(self.wiring.expander.clone()),wiring:self.wiring.clone()})),
             (1,0x38,Box::new(Touch::new(self.wiring.touch.clone()))),(1,0x68,Box::new(Tps::new(self.wiring.clone()))),
             (1,0x55,Box::new(WiredDevice(Box::new(crate::bq27220::Bq27220::new()))))]
    }
    fn touch(&mut self,x:u16,y:u16,down:bool) {
        self.wiring.sync();{let mut t=self.wiring.touch.lock().expect("INK4 Plus touch poisoned");if t.powered {t.x=x.min(4095);t.y=y.min(4095);t.down=down;}}self.sync_irq();
    }
    fn button_at(&mut self,cycle:VirtualCycle,id:&str,pressed:bool)->bool {
        self.cycle=cycle;match id {
            "power"|"volume-up"|"volume-down"=>self.wiring.expander.lock().expect("INK4 Plus expander poisoned").set_input(match id {"power"=>15,"volume-up"=>8,_=>9},!pressed),
            "home"|"previous"|"next"=>{self.touch(match id {"home"=>100,"previous"=>200,_=>300},1600,pressed);},
            _=>return false,
        }self.sync_irq();true
    }
    fn named_pin(&self,id:&str)->Option<u8> {if id=="boot" {Some(61)}else{None}}
    fn sd_card_slot(&self)->Option<([u8;6],bool)> {Some(([24,25,20,21,22,23],true))}
    fn lcd_rgb_pins(&mut self,pins:LcdRgbPins) {
        self.rgb_routes=pins.width==8 && pins.clock&(1<<16)!=0 && pins.hsync&(1<<17)!=0 && pins.enable&(1<<4)!=0
            && pins.data[..8].iter().enumerate().all(|(bit,mask)|mask&(1<<(bit+8))!=0);
    }
    fn lcd_rgb_start(&mut self,_cycle:VirtualCycle,w:u32,h:u32,bpp:u32,clock:&dyn Fn(u8)->Option<(u64,u32)>) {
        let pending=std::mem::take(&mut self.scan_pending);
        self.scan=pending && self.rgb_routes && self.powered() && bpp==1 && matches!((w,h),(304,684)|(305,688))
            && clock(40).is_some_and(|(period,pulses)|period!=0&&pulses>=h);
    }
    fn lcd_frame(&mut self,w:u32,h:u32,bytes:&[u8]) {
        let scan=std::mem::take(&mut self.scan);
        if !scan || !self.rgb_routes || !self.powered() || !matches!((w,h),(304,684)|(305,688)) || bytes.len()!=w as usize*h as usize {return;}
        // CloudZao ad8edf4 uses a leading dummy byte and four padded rows;
        // 14aa1d06 uses native rows with no dummy. Each frame is ONE waveform
        // phase: 00/11 retain, 01 black, 10 white. This optical endpoint subset
        // does not claim calibrated pulse-dose grayscale or real ghosting.
        let stride=w as usize;let dummy=stride-304;let mut changed=false;
        for y in 0..HEIGHT {let line=&bytes[y*stride+dummy..(y+1)*stride];for x in 0..WIDTH {
            let pixel=&mut self.frame[y*WIDTH+x];let next=match (line[x/4]>>(6-2*(x%4)))&3 {1=>0,2=>0xffff,_=>*pixel};changed|=*pixel!=next;*pixel=next;
        }}
        self.frames+=1;if changed {self.version+=1;}
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let mut out=vec![0xffff;WIDTH*HEIGHT];for y in 0..HEIGHT {for x in 0..WIDTH {out[x*HEIGHT+HEIGHT-1-y]=self.frame[y*WIDTH+x];}}Some((HEIGHT as u32,WIDTH as u32,out,self.version))
    }
    fn display_version(&self)->u64 {self.version}
    fn display_frames(&self)->u64 {self.frames}
    fn display_quiet_push(&self)->bool {true}
    fn report(&self)->String {format!("[metalio-ink4-plus] ED047TC2 frames={} rails={:#06x}; FT6336U v04 report protocol; TPS nominal25C; BQ27220 nominal battery; SDMMC slot0; audio/modem/IMU/RTC/charger=unmodeled",self.frames,self.wiring.rails())}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(d:&mut dyn I2cDevice,r:u8,bytes:&[u8]) {assert!(d.start(false));assert!(d.write(r));for &b in bytes {assert!(d.write(b));}d.stop();}
    fn read(d:&mut dyn I2cDevice,r:u8,n:usize)->Vec<u8> {write(d,r,&[]);assert!(d.start(true));let v=(0..n).map(|_|d.read()).collect();d.stop();v}
    #[test]
    fn expander_power_good_touch_reset_and_irq_follow_real_device_ports() {
        let mut b=Ink4Plus::new();let mut devices=b.i2c_devices();
        assert!(devices.iter().all(|(bus,_,d)|*bus==1&&d.pins()==Some((0,1))));
        let mut tps=devices.swap_remove(2).2;let mut touch=devices.swap_remove(1).2;let exp=&mut devices[0].2;
        assert!(!touch.start(true));assert!(!tps.start(true));
        write(exp.as_mut(),2,&[0x20,0x14]);assert!(!touch.start(true),"input pullups cannot power touch");
        write(exp.as_mut(),6,&[0xc0,0xe3]);assert!(touch.start(true));assert!(!tps.start(true));
        assert_eq!(read(touch.as_mut(),0xa3,1),[0x64]);assert_eq!(read(touch.as_mut(),0x9f,1),[0x26]);
        assert_eq!(read(exp.as_mut(),0,1)[0]&0x80,0);
        write(exp.as_mut(),2,&[0x24]);assert!(tps.start(true));assert_eq!(read(tps.as_mut(),15,1),[0]);
        write(exp.as_mut(),2,&[0x27]);assert_ne!(read(exp.as_mut(),0,1)[0]&0x80,0);assert_eq!(read(tps.as_mut(),15,1),[0xfa]);
        write(tps.as_mut(),13,&[0x80]);assert_eq!(read(tps.as_mut(),13,1),[0x20]);assert_eq!(read(tps.as_mut(),0,1),[25]);
        b.touch(300,1100,true);assert!(b.take_edges().iter().any(|e|e.pin==5&&!e.level));assert_eq!(read(touch.as_mut(),2,5),[1,0x81,44,4,76]);
        b.touch(300,1100,false);assert!(b.take_edges().iter().any(|e|e.pin==5&&e.level));assert_eq!(read(touch.as_mut(),2,1),[0]);
        write(exp.as_mut(),3,&[0x10]);assert!(!touch.start(true));b.touch(100,100,true);assert!(!b.wiring.touch.lock().unwrap().down);
        write(exp.as_mut(),2,&[0]);assert!(!tps.start(true));assert_eq!(read(exp.as_mut(),0,1)[0]&0x80,0);
    }
    #[test]
    fn semantic_keys_are_expander_or_touch_reports_not_invented_esp_pins() {
        let mut b=Ink4Plus::new();let mut devices=b.i2c_devices();let exp=&mut devices[0].2;
        write(exp.as_mut(),2,&[0x20,0x14]);write(exp.as_mut(),6,&[0xc0,0xe3]);read(exp.as_mut(),0,2);
        for (id,mask) in [("power",0x80),("volume-up",1),("volume-down",2)] {
            assert_eq!(b.named_pin(id),None);assert!(b.button_at(1,id,true));assert_eq!(read(exp.as_mut(),1,1)[0]&mask,0);
            assert!(b.button_at(2,id,false));assert_ne!(read(exp.as_mut(),1,1)[0]&mask,0);
        }
        for (id,x) in [("home",100),("previous",200),("next",300)] {assert!(b.button_at(3,id,true));let t=b.wiring.touch.lock().unwrap();assert_eq!((t.x,t.y,t.down),(x,1600,true));drop(t);assert!(b.button_at(4,id,false));}
        assert!(!b.button_at(5,"invented",true));assert_eq!(b.named_pin("boot"),Some(61));
        let inputs=inputs(16<<20);assert!(inputs.efuse.is_empty());assert!(inputs.inputs.is_empty());assert_eq!(inputs.psram_bytes,16<<20);
    }
    fn panel()->Ink4Plus {
        let mut b=Ink4Plus::new();let mut devices=b.i2c_devices();write(devices[0].2.as_mut(),2,&[0x27,0x14]);write(devices[0].2.as_mut(),6,&[0xc0,0xe3]);
        let mut data=[0;16];for (i,pin) in data[..8].iter_mut().enumerate() {*pin=1<<(8+i);}
        b.lcd_rgb_pins(LcdRgbPins{clock:1<<16,hsync:1<<17,vsync:0,enable:1<<4,data,width:8});
        b.gpio_output_at(0,&[],(1<<18)|(1<<35),(1<<18)|(1<<35));b.gpio_output_at(1,&[(35,false)],(1<<18)|(1<<35),1<<18);b
    }
    #[test]
    fn native_and_legacy_waveforms_preserve_pixels_skip_dummy_and_padding() {
        for (w,h) in [(304,684),(305,688)] {
            let mut b=panel();let mut bytes=vec![0;w*h];let dummy=w-304;
            if dummy!=0 {bytes[0]=0x55;bytes[684*w..].fill(0x55);}
            bytes[dummy]=0x6c; // 01,10,11,00: black, white, retain, retain.
            bytes[683*w+dummy+303]=1; // final electrode of the final gate row.
            b.lcd_rgb_start(2,w as u32,h as u32,1,&|pin|if pin==40 {Some((17_000,h as u32+5))}else{None});b.lcd_frame(w as u32,h as u32,&bytes);
            assert_eq!(b.frames,1);assert_eq!(&b.frame[..4],&[0,0xffff,0xffff,0xffff]);assert_eq!(*b.frame.last().unwrap(),0);assert_eq!(b.frame.iter().filter(|&&p|p==0).count(),2);
            let output=b.display().unwrap();assert_eq!((output.0,output.1),(684,1216));assert_eq!(output.2[683],0);assert_eq!(output.2[(1216-1)*684],0);
            let pixels=b.frame.clone();b.lcd_frame(w as u32,h as u32,&vec![0x55;w*h]);assert_eq!(b.frames,1,"one SPV scan cannot consume two phases");
            b.gpio_output_at(3,&[],0,0);assert_eq!(b.frame,pixels,"e-paper retains optical state without power");
        }
    }
    #[test]
    fn scan_requires_routes_power_start_pulse_complete_frame_and_gate_clock() {
        for bad in 0..7 {
            let mut b=panel();match bad {0=>b.rgb_routes=false,1=>b.output=0,2=>b.scan_pending=false,_=>{}}
            b.lcd_rgb_start(2,304,684,1,&|_|match bad {3=>None,4=>Some((17_000,683)),_=>Some((17_000,689))});
            if bad==5 {b.rgb_routes=false;}
            b.lcd_frame(304,684,&vec![0x55;if bad==6 {304*684-1}else{304*684}]);assert_eq!(b.frames,0,"invalid scan {bad}");assert!(b.frame.iter().all(|&p|p==0xffff));
        }
    }
}
