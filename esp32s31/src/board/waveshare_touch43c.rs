//! Waveshare ESP32-S31-Touch-LCD-4.3C (SKU36107/36105/36106).
//! Exact RGB565/power/input wiring: manufacturer documentation and repository
//! aeafb2ce1c37fc05409856cff1bfed7d83fc0d58; retained source receipt is in
//! docs/research/waveshare-p4-s31-2026-11-24. No S3 pin map is inherited.
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex}};
use esp_periph::i2c::{I2cDevice,gt911::{Gt911,Gt911State}};
use esp_soc::{Board,board::{BoardModel,BoardEdge,LcdRgbPins,VirtualCycle},devices::{imu_motion::ImuMotion,pcf85063::{Pcf85063,Pcf85063State},qmi8658::{Qmi8658,Qmi8658Port}}};
const WIDTH:u32=800;const HEIGHT:u32=480;const NS:u64=1_000_000_000;
const DATA:[u8;16]=[8,9,10,11,12,13,14,15,16,17,18,19,33,34,35,36];

/// Nominal N16R16 storage, with no invented measured NOR ID or board eFuses.
/// The devkit's Winbond/PLL/analog seeds remain explicit emulator assumptions.
pub fn inputs()->crate::soc::BoardInputs {
    crate::soc::BoardInputs{psram_bytes:16<<20,inputs:Vec::new(),..crate::soc::BoardInputs::devkit_w25q128()}
}
pub fn make_board(name:&str)->Option<Board> {
    (name=="waveshare-s31-touch-lcd43c").then(||Box::new(WaveshareTouch43c::new()) as Board)
}
struct TouchPort {device:Gt911,selected:Arc<Mutex<u8>>,address:u8}
impl I2cDevice for TouchPort {
    fn pins(&self)->Option<(u8,u8)>{Some((0,1))}
    fn start(&mut self,r:bool)->bool {*self.selected.lock().expect("GT911 address")==self.address&&self.device.start(r)}
    fn write(&mut self,b:u8)->bool{self.device.write(b)}
    fn read(&mut self)->u8{self.device.read()}
    fn stop(&mut self){self.device.stop();}
}
struct Wired(Box<dyn I2cDevice>);
impl I2cDevice for Wired {
    fn pins(&self)->Option<(u8,u8)>{Some((0,1))}
    fn start(&mut self,r:bool)->bool{self.0.start(r)}fn write(&mut self,b:u8)->bool{self.0.write(b)}
    fn read(&mut self)->u8{self.0.read()}fn stop(&mut self){self.0.stop();}
}
pub struct WaveshareTouch43c {
    frame:Vec<u16>,frames:u64,version:u64,routes:bool,scan:bool,cycle:u64,
    output:u64,enabled:u64,boot:bool,sd_present:bool,events:u64,edges:Vec<BoardEdge>,
    touch:Arc<Mutex<Gt911State>>,address:Arc<Mutex<u8>>,touch_reset:bool,touch_irq:bool,
    rtc:Arc<Mutex<Pcf85063State>>,imu:Arc<ImuMotion>,qmi:Rc<RefCell<Qmi8658>>,led:[[u8;3];1],led_version:u64,
}
impl Default for WaveshareTouch43c{fn default()->Self{Self::new()}}
impl WaveshareTouch43c {
    pub fn new()->Self {
        let touch=Arc::new(Mutex::new(Gt911State::new(800,480,NS/100)));touch.lock().expect("GT911").powered=false;
        let imu=Arc::new(ImuMotion::default());let qmi=Rc::new(RefCell::new(Qmi8658::new_c(imu.clone(),NS)));
        Self{frame:vec![0;(WIDTH*HEIGHT) as usize],frames:0,version:0,routes:false,scan:false,cycle:0,
            output:0,enabled:0,boot:true,sd_present:false,events:0,edges:Vec::new(),touch,address:Arc::new(Mutex::new(0x5d)),touch_reset:false,touch_irq:true,
            rtc:Arc::new(Mutex::new(Pcf85063State::new(NS))),imu,qmi,led:[[0;3]],led_version:0}
    }
    fn high(&self,p:u8)->bool{self.enabled&self.output&(1<<p)!=0}
    fn sync_touch(&mut self){
        let mut t=self.touch.lock().expect("GT911");t.powered=self.touch_reset;t.advance_to(self.cycle);
        let irq=!t.ready||!t.powered;drop(t);if irq!=self.touch_irq{self.touch_irq=irq;self.edges.push(BoardEdge{cycle:self.cycle,pin:2,level:irq});}
    }
}
impl BoardModel for WaveshareTouch43c {
    fn name(&self)->&'static str{"waveshare-s31-touch-lcd43c"}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64){
        let was_lit=self.high(47);self.cycle=cycle;self.events+=changes.len() as u64;self.enabled=enabled;self.output=output;
        if was_lit!=self.high(47){self.version+=1;}
        let reset=self.high(46);
        if reset&&!self.touch_reset{*self.address.lock().expect("GT911 address")=if self.high(2){0x14}else{0x5d};}
        if !reset&&self.touch_reset{*self.touch.lock().expect("GT911")=Gt911State::new(800,480,NS/100);}
        self.touch_reset=reset;self.sync_touch();
        let next=if self.high(55){[0,180,0]}else{[0;3]};if self.led[0]!=next{self.led[0]=next;self.led_version+=1;}
    }
    fn gpio_events(&self)->u64{self.events}
    fn input_levels(&self)->Vec<(u8,bool)>{vec![(61,self.boot),(2,self.touch_irq)]}
    fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(61)}
    fn button_at(&mut self,_cycle:VirtualCycle,id:&str,pressed:bool)->bool{
        if id!="boot"{return false;}let high=!pressed;if high!=self.boot{self.boot=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:61,level:high});}true
    }
    fn advance_to(&mut self,cycle:VirtualCycle){self.cycle=cycle;self.imu.cycle.store(cycle,std::sync::atomic::Ordering::Relaxed);self.rtc.lock().expect("RTC").advance_to(cycle);self.sync_touch();}
    fn take_edges(&mut self)->Vec<BoardEdge>{self.sync_touch();std::mem::take(&mut self.edges)}
    fn touch(&mut self,x:u16,y:u16,down:bool){let mut t=self.touch.lock().expect("GT911");if t.powered{t.update(x,y,down);}drop(t);self.sync_touch();}
    // Host input timestamps use CPU cycles; this board advances in SoC nanoseconds.
    fn touch_at(&mut self,_cycle:VirtualCycle,x:u16,y:u16,down:bool){self.touch(x,y,down);}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{
        let mut out=Vec::new();for bus in [0,1]{
            for address in [0x14,0x5d]{out.push((bus,address,Box::new(TouchPort{device:Gt911::new(self.touch.clone()),selected:self.address.clone(),address}) as Box<dyn I2cDevice>));}
            out.push((bus,0x51,Box::new(Wired(Box::new(Pcf85063::new(self.rtc.clone()))))));
            out.push((bus,0x6b,Box::new(Wired(Box::new(Qmi8658Port::new(self.qmi.clone()))))));
        }out
    }
    fn set_imu_motion(&mut self,mode:u32)->bool{self.qmi.borrow_mut().set_motion_mode(mode)}
    fn set_imu_sample(&mut self,a:[f64;3],g:[f64;3])->bool{self.qmi.borrow_mut().set_host_sample(a,g)}
    fn imu_motion(&self)->u32{self.imu.mode.load(std::sync::atomic::Ordering::Relaxed)}
    fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some(([24,25,20,21,22,23],self.sd_present&&self.enabled&(1<<56)!=0&&self.output&(1<<56)==0))}
    fn set_sd_card_present(&mut self,present:bool){self.sd_present=present;}
    fn lcd_rgb_pins(&mut self,p:LcdRgbPins){self.routes=p.width==16&&p.clock&(1<<40)!=0&&p.enable&(1<<43)!=0&&p.hsync&(1<<44)!=0&&p.vsync&(1<<45)!=0&&DATA.iter().enumerate().all(|(bit,pin)|p.data[bit]&(1<<pin)!=0);}
    fn lcd_rgb_start(&mut self,_cycle:VirtualCycle,w:u32,h:u32,bpp:u32,_clock:&dyn Fn(u8)->Option<(u64,u32)>){self.scan=self.routes&&(w,h,bpp)==(WIDTH,HEIGHT,2);}
    fn lcd_frame(&mut self,w:u32,h:u32,bytes:&[u8]){
        if !self.scan||!self.routes||(w,h)!=(WIDTH,HEIGHT)||bytes.len()!=(WIDTH*HEIGHT*2) as usize{return;}
        for (pixel,pair) in self.frame.iter_mut().zip(bytes.as_chunks::<2>().0){*pixel=u16::from_le_bytes(*pair);}self.frames+=1;self.version+=1;
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{Some((WIDTH,HEIGHT,if self.high(47){self.frame.clone()}else{vec![0;self.frame.len()]},self.version))}
    fn display_version(&self)->u64{self.version}fn display_frames(&self)->u64{self.frames}
    fn leds(&self)->Option<(&[[u8;3]],u64)>{Some((&self.led,self.led_version))}
    fn report(&self)->String{format!("[{}] RGB565800x480 frames={} physical-routes={} BL_GPIO47={}; GT911 address={:#x} reset46={} SDA0/SCL1 INT2; BOOT61; SD24/25/20..23 power56 active-low; QMI8658C@6b PCF85063@51. No LEDC-backlight/ES8389/analog battery/CAN-FD/RS485/USB-host/radio acceptance; no measured NOR/eFuse identity.",self.name(),self.frames,self.routes,self.high(47),*self.address.lock().expect("GT911 address"),self.touch_reset)}
}
#[cfg(test)]
mod tests{
    use super::*;
    fn pins()->LcdRgbPins{let mut data=[0;16];for (i,pin) in DATA.iter().enumerate(){data[i]=1<<pin;}LcdRgbPins{width:16,clock:1<<40,enable:1<<43,hsync:1<<44,vsync:1<<45,data}}
    fn read(d:&mut dyn I2cDevice,reg:u16,n:usize)->Vec<u8>{assert!(d.start(false));for x in reg.to_be_bytes(){assert!(d.write(x));}assert!(d.start(true));let r=(0..n).map(|_|d.read()).collect();d.stop();r}
    #[test]
    fn rgb565_requires_complete_physical_bus_frame_and_backlight(){
        let mut b=WaveshareTouch43c::new();let mut bytes=vec![0;(WIDTH*HEIGHT*2) as usize];bytes[..6].copy_from_slice(&[0,0xf8,0xe0,7,31,0]);
        b.lcd_rgb_pins(pins());b.lcd_rgb_start(0,800,480,2,&|_|None);b.lcd_frame(800,480,&bytes);assert_eq!(b.frames,1);assert!(b.display().unwrap().2.iter().all(|x|*x==0));
        b.gpio_output_at(0,&[],1<<47,1<<47);assert_eq!(&b.display().unwrap().2[..3],&[0xf800,0x07e0,0x001f]);
        for bad in 0..5{let mut p=pins();match bad{0=>p.data.swap(0,15),1=>p.clock=1<<41,2=>p.enable=0,3=>p.vsync=0,_=>p.width=8};b.lcd_rgb_pins(p);b.lcd_rgb_start(1,800,480,2,&|_|None);b.lcd_frame(800,480,&bytes);assert_eq!(b.frames,1);}
        b.lcd_rgb_pins(pins());b.lcd_rgb_start(2,800,480,1,&|_|None);b.lcd_frame(800,480,&bytes);assert_eq!(b.frames,1);
        b.lcd_rgb_start(3,800,480,2,&|_|None);b.lcd_frame(800,480,&bytes[..bytes.len()-2]);assert_eq!(b.frames,1);
    }
    #[test]
    fn reset_strap_touch_reports_and_sd_gate_follow_documented_signals(){
        let mut b=WaveshareTouch43c::new();let mut d=b.i2c_devices();assert!(d.iter().all(|(_,_,p)|p.pins()==Some((0,1))));
        assert!(!d[0].2.start(true));assert!(!d[1].2.start(true));b.gpio_output_at(0,&[],1<<46,1<<46);
        assert!(!d[0].2.start(true));assert_eq!(read(d[1].2.as_mut(),0x8140,4),b"911\0");
        assert!(d[1].2.start(false));for v in [0x81,0x4e,0]{assert!(d[1].2.write(v));}d[1].2.stop();b.touch(799,479,true);b.advance_to(NS/100);
        assert_eq!(read(d[1].2.as_mut(),0x814e,6),[0x81,0,31,3,223,1]);
        b.gpio_output_at(NS/100,&[],(1<<46)|(1<<2),1<<2);assert!(!d[1].2.start(true));
        b.gpio_output_at(NS/100+1,&[],(1<<46)|(1<<2),(1<<46)|(1<<2));assert!(!d[1].2.start(true));assert_eq!(read(d[0].2.as_mut(),0x8140,4),b"911\0");
        b.set_sd_card_present(true);assert!(!b.sd_card_slot().unwrap().1);b.gpio_output_at(NS/100+2,&[],1<<56,0);assert!(b.sd_card_slot().unwrap().1);b.gpio_output_at(NS/100+3,&[],1<<56,1<<56);assert!(!b.sd_card_slot().unwrap().1);
        assert!(b.button_at(123,"boot",true));assert!(b.input_levels().contains(&(61,false)));assert!(!b.button_at(123,"reset",true));assert!(inputs().efuse.is_empty());
    }

    /// Real S31 instructions produce the RGB565 bytes and all DMA descriptors in
    /// SRAM, configure the physical RGB routes and read BOOT61 back into SRAM.
    /// No host framebuffer or MMIO-result injection is used. This is a bounded
    /// guest fixture, not the manufacturer's boot image or a timing acceptance.
    #[test]
    fn s31_guest_rgb_dma_and_boot_feedback_require_the_exact_board_routes(){
        use crate::{bus::{SRAM_BASE,AXI_DMA},pads::{GPIO_BASE,IO_MUX_BASE},lcd};
        use emu_core::Bus;
        fn li(code:&mut Vec<u32>,reg:u32,value:u32){code.push((value.wrapping_add(0x800)&0xfffff000)|(reg<<7)|0x37);code.push(((value&0xfff)<<20)|(reg<<15)|(reg<<7)|0x13);}
        fn write(code:&mut Vec<u32>,addr:u32,value:u32){li(code,1,addr);li(code,2,value);code.push((2<<20)|(1<<15)|(2<<12)|0x23);}
        for (clock_pin,backlight) in [(40,true),(41,true),(40,false)] {
            let mut m=crate::soc::machine([0;6],16<<20);
            m.bus.board=make_board("waveshare-s31-touch-lcd43c").unwrap();m.bus.attach_board_devices();
            let mut code=Vec::new();
            for (bit,pin) in DATA.iter().enumerate(){
                write(&mut code,IO_MUX_BASE+u32::from(*pin)*4,1<<12);
                write(&mut code,GPIO_BASE+0xaf4+u32::from(*pin)*4,154+bit as u32);
            }
            for (pin,signal) in [(clock_pin,149),(43,151),(44,152),(45,153)] {
                write(&mut code,IO_MUX_BASE+pin*4,1<<12);write(&mut code,GPIO_BASE+0xaf4+pin*4,signal);
            }
            write(&mut code,IO_MUX_BASE+47*4,1<<12);write(&mut code,GPIO_BASE+0x44,1<<15);
            write(&mut code,GPIO_BASE+0x14,if backlight{1<<15}else{0});
            write(&mut code,IO_MUX_BASE+61*4,(1<<12)|(1<<9));
            let buffer=SRAM_BASE+0x20000;let descriptors=SRAM_BASE+0x22000;
            for i in 0..1023 {write(&mut code,buffer+i*4,[0x07e0_f800,0xf800_001f,0x001f_07e0][i as usize%3]);}
            let bytes=800u32*480*2;let count=bytes.div_ceil(4092);
            for i in 0..count {
                let length=(bytes-i*4092).min(4092);let final_desc=i+1==count;let at=descriptors+i*12;
                write(&mut code,at,(1<<31)|if final_desc{1<<30}else{0}|(length<<12)|length);
                write(&mut code,at+4,buffer);write(&mut code,at+8,if final_desc{0}else{at+12});
            }
            for (addr,value) in [(AXI_DMA+0x138+0x44,0),(AXI_DMA+0x138+0x24,descriptors),(AXI_DMA+0x138+0x20,2),
                (lcd::BUS_CLOCK,5),(lcd::PIXEL_CLOCK,4),(lcd::SHARED_CLOCK,4),(lcd::BASE+0x18,8),
                (lcd::BASE+0x20,799|(799<<16)),(lcd::BASE+0x1c,479|(479<<16)),(lcd::USER,(1<<27)|(1<<24)|(1<<19))] {
                write(&mut code,addr,value);
            }
            li(&mut code,1,GPIO_BASE+0x68);code.push((1<<15)|(2<<12)|(2<<7)|3);
            li(&mut code,1,SRAM_BASE+0x24000);code.push((2<<20)|(1<<15)|(2<<12)|0x23);code.push(0x6f);
            m.bus.load_bytes(SRAM_BASE,&code.iter().flat_map(|i|i.to_le_bytes()).collect::<Vec<_>>()).unwrap();m.cores[0].pc=SRAM_BASE;
            assert!(matches!(m.run(4_000_000),esp_soc::Stop::MaxInsns));
            assert_eq!(m.bus.board.display_frames(),u64::from(clock_pin==40));
            let frame=m.bus.board.display().unwrap();assert_eq!((frame.0,frame.1,frame.2.len()),(800,480,800*480));
            if clock_pin==40&&backlight {assert_eq!(&frame.2[..6],&[0xf800,0x07e0,0x001f,0xf800,0x07e0,0x001f]);assert_eq!(frame.2[800*480-1],0x001f);}
            else {assert!(frame.2.iter().all(|p|*p==0));}
            assert_ne!(m.bus.read32(SRAM_BASE+0x24000).unwrap()&(1<<29),0);
            assert!(m.bus.board.button_at(0,"boot",true));m.cores[0].pc=SRAM_BASE;
            assert!(matches!(m.run(100_000),esp_soc::Stop::MaxInsns));assert_eq!(m.bus.read32(SRAM_BASE+0x24000).unwrap()&(1<<29),0);
            assert!(m.bus.inner.unmodelled.is_empty()&&m.bus.inner.refused.is_empty());
        }
    }
}
