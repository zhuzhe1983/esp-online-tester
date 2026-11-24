//! C5 Touch LCD3.5, fixed manufacturer revision a39067a9.
//! ST7796, FT6336 and CH32V006 are supplied by AXP2101 DCDC1/VCC3V3.
//! This board's explicit nominal digital scenario requires a requested 3300mV.
use esp_periph::{i2c::I2cDevice, sdmmc::{SdCardConfig, SdStore, card::spi::SpiSdCard}};
use esp_soc::{BoardModel, board::{BoardEdge, SpiQspiPins}, devices::{DcsPanel,
    axp2101::{Axp2101, Axp2101Power, PowerRail}, es8311::Es8311,
    imu_motion::ImuMotion, pcf85063::{Pcf85063, Pcf85063State}, qmi8658::Qmi8658}, pins::PinRoutes};
use std::{cell::RefCell, rc::Rc, sync::{Arc, Mutex, atomic::Ordering}};
use super::{waveshare_ch32v006::{Ch32V006, Ch32V006Port}, waveshare_ft6336::{Ft6336, Ft6336State}};

struct Control {
    port: Arc<Mutex<Ch32V006Port>>, touch: Arc<Mutex<Ft6336State>>,
    power: Arc<Mutex<Axp2101Power>>, motion: Arc<ImuMotion>, touch_falling: Mutex<u64>,
}
impl Control {
    fn powered(&self) -> bool { self.power.lock().expect("AXP2101 state").output_mv(PowerRail::Dcdc1) == Some(3300) }
    fn sync(&self) {
        let power = self.power.lock().expect("AXP2101 state");
        let on = power.output_mv(PowerRail::Dcdc1) == Some(3300);
        let irq_high = !power.irq_asserted(); drop(power);
        let mut port = self.port.lock().expect("CH32V006 state"); port.set_powered(on);
        // R41 pulls LCD_RST high in input mode. IRQ sources are quiescent in
        // this scene; PWR EX6 is unpressed, unconnected header inputs are low.
        port.inputs = (1 << 0) | (1 << 2) | (u16::from(irq_high) << 3) | (1 << 4);
        let high = port.level(1) == Some(true); let falling = port.falling_edges(1); drop(port);
        let mut last = self.touch_falling.lock().expect("touch reset edge");
        let mut touch = self.touch.lock().expect("FT6336 state");
        if *last != falling { touch.set_available(false); } *last = falling;
        touch.set_available(on && high);
    }
}
struct Part { device: Box<dyn I2cDevice>, epoch: u64 }
type PersistentPart = Rc<RefCell<Part>>;
struct RoutedDevice { address: u8, part: PersistentPart, control: Arc<Control> }
impl RoutedDevice {
    fn ready(&self) -> bool {
        self.control.sync();
        let epoch = self.control.port.lock().expect("CH32V006 state").power_epoch;
        let mut part = self.part.borrow_mut();
        if epoch != part.epoch {
            match self.address {
                0x6b => part.device = Box::new(Qmi8658::new_c(self.control.motion.clone(), crate::bus::CPU_HZ)),
                0x18 => part.device = Box::new(Es8311::new()),
                _ => {}, // CH32 and FT own their reset state; RTC has its separate RTC rail.
            }
            part.epoch = epoch;
        }
        matches!(self.address, 0x34 | 0x51) || self.control.powered()
    }
}
impl I2cDevice for RoutedDevice {
    fn pins(&self) -> Option<(u8,u8)> { Some((27,26)) }
    fn start_address_with_clock(&mut self, c:u8, a:u8, r:bool, hz:Option<u32>) -> bool {
        self.ready() && self.part.borrow_mut().device.start_address_with_clock(c,a,r,hz)
    }
    fn write(&mut self, value:u8) -> bool {
        if !self.ready() { return false; }
        let ack = self.part.borrow_mut().device.write(value); self.control.sync(); ack
    }
    fn read(&mut self) -> u8 { if self.ready() { self.part.borrow_mut().device.read() } else { 0xff } }
    fn stop(&mut self) { self.part.borrow_mut().device.stop(); self.control.sync(); }
}

pub struct Touch35 {
    panel: DcsPanel, dc: Option<bool>, backlight:u8, lcd_reset_high:bool,
    lcd_falling:u64, power_epoch:u64, powered:bool, cmd:u8, generation:u64, rejected:u64,
    control: Arc<Control>, devices:Vec<(u8,PersistentPart)>, rtc:Arc<Mutex<Pcf85063State>>,
    cycle:u64, irq_high:bool, sd:Option<SpiSdCard>,
}
impl Default for Touch35 { fn default() -> Self { Self::new() } }
impl Touch35 {
    pub fn new() -> Self {
        let port = Arc::new(Mutex::new(Ch32V006Port::new()));
        let touch = Arc::new(Mutex::new(Ft6336State::new()));
        let power = Arc::new(Mutex::new(Axp2101Power::default()));
        let motion = Arc::new(ImuMotion::default());
        let rtc = Arc::new(Mutex::new(Pcf85063State::new(crate::bus::CPU_HZ)));
        let control = Arc::new(Control { port:port.clone(), touch:touch.clone(), power:power.clone(),
            motion:motion.clone(), touch_falling:Mutex::new(0) }); control.sync();
        let parts:Vec<(u8,Box<dyn I2cDevice>)> = vec![
            (0x24,Box::new(Ch32V006::new(port))), (0x38,Box::new(Ft6336::new(touch))),
            (0x34,Box::new(Axp2101::new(power))), (0x18,Box::new(Es8311::new())),
            // Schematic/README name QMI8658 without suffix. The bundled PDF
            // is A, while the board example is named qmi8658c. Use the stricter
            // shared A/C register intersection (C rejects A-only CTRL1 bits
            // and 2048dps), without asserting that the physical suffix is C.
            (0x6b,Box::new(Qmi8658::new_c(motion,crate::bus::CPU_HZ))),
            (0x51,Box::new(Pcf85063::new(rtc.clone()))),
        ];
        Self { panel:DcsPanel::new(320,480), dc:None, backlight:0, lcd_reset_high:false,
            lcd_falling:0,power_epoch:0,powered:true,cmd:0,generation:1,rejected:0,control,
            devices:parts.into_iter().map(|(a,device)|(a,Rc::new(RefCell::new(Part { device,epoch:0 })))).collect(),
            rtc,cycle:0,irq_high:true,sd:None }
    }
    fn sync_ports(&mut self) {
        self.control.sync(); let on = self.control.powered();
        let p = self.control.port.lock().expect("CH32V006 state");
        let (high,falling,brightness,epoch) = (p.level(0)==Some(true),p.falling_edges(0),p.pwm.unwrap_or(0),p.power_epoch); drop(p);
        if epoch != self.power_epoch {
            self.panel = DcsPanel::new(320,480); self.cmd=0;
            if let Some(sd) = &mut self.sd { sd.power_cycle(); }
            self.generation=self.generation.wrapping_add(1);
        } else if self.lcd_falling!=falling || self.lcd_reset_high&&!high {
            self.panel.reset(); self.cmd=0; self.generation=self.generation.wrapping_add(1);
        }
        if self.lcd_reset_high!=high || self.backlight!=brightness || self.powered!=on { self.generation=self.generation.wrapping_add(1); }
        self.lcd_falling=falling; self.lcd_reset_high=high; self.backlight=brightness; self.powered=on; self.power_epoch=epoch;
    }
}
impl BoardModel for Touch35 {
    fn name(&self)->&'static str { "waveshare-c5-touch35" }
    fn named_pin(&self,name:&str)->Option<u8> { matches!(name,"boot"|"button").then_some(28) }
    fn input_levels(&self)->Vec<(u8,bool)> { self.control.sync(); vec![(28,true),(3,self.control.touch.lock().expect("FT6336 state").irq_high())] }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.devices.iter().map(|(a,p)|(0,*a,Box::new(RoutedDevice { address:*a,part:p.clone(),control:self.control.clone() }) as Box<dyn I2cDevice>)).collect()
    }
    fn advance_to(&mut self,cycle:u64) {
        self.cycle=cycle; self.control.motion.cycle.store(cycle,Ordering::Relaxed);
        self.rtc.lock().expect("PCF85063 state").advance_to(cycle); self.sync_ports();
    }
    fn touch(&mut self,x:u16,y:u16,down:bool) { self.control.sync(); self.control.touch.lock().expect("FT6336 state").update(x,y,down); }
    fn touch_at(&mut self,cycle:u64,x:u16,y:u16,down:bool) { self.advance_to(cycle); self.touch(x,y,down); }
    fn touch_irq(&self)->Option<(u8,bool)> { Some((3,self.control.touch.lock().expect("FT6336 state").irq_high())) }
    fn take_edges(&mut self)->Vec<BoardEdge> {
        self.sync_ports(); let high=self.control.touch.lock().expect("FT6336 state").irq_high();
        if high==self.irq_high { return vec![]; } self.irq_high=high; vec![BoardEdge { cycle:self.cycle,pin:3,level:high }]
    }
    fn set_imu_motion(&mut self,mode:u32)->bool { self.control.motion.mode.store(mode,Ordering::Relaxed); true }
    fn imu_motion(&self)->u32 { self.control.motion.mode.load(Ordering::Relaxed) }
    fn uses_gpio_routes(&self)->bool { true }
    fn gpio_waveform_at(&mut self,cycle:u64,r:PinRoutes<'_>) {
        self.advance_to(cycle); self.dc=r.software_output(5);
        let gpio=r.software_output(9); let hardware=[61,101,102,103,104,105].into_iter().any(|s|r.output_pins(s).any(|p|p==9));
        if gpio==Some(true)||(gpio.is_none()&&!hardware) { if let Some(sd)=&mut self.sd { sd.deselect(); } }
    }
    fn uses_qspi_pins(&self)->bool { true }
    fn spi_transfer_qspi_pins(&mut self,host:u8,p:SpiQspiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        self.sync_ports(); let floating=vec![0xff;rx_len];
        if !self.powered { return floating; }
        if host!=2||p.unsupported_mode||p.sio||p.clock_idle_high||p.clock_output_edge
            ||[p.command_width,p.address_width,p.data_width,p.read_width].iter().any(|&w|w>1)
            ||p.sclk&(1<<6)==0||p.data[0]&(1<<7)==0||!p.command_bits.is_multiple_of(8)||!p.address_bits.is_multiple_of(8)
            ||!p.data_bits.is_multiple_of(8)||!p.read_bits.is_multiple_of(8)||p.dummy_cycles!=0 { self.rejected+=1; return floating; }
        let lcd=p.cs&(1<<8)!=0; let card=p.cs&(1<<9)!=0;
        if lcd&&card { self.rejected+=1; if let Some(sd)=&mut self.sd { sd.deselect(); } return floating; }
        if let Some(sd)=&mut self.sd {
            let rx=if p.full_duplex||tx.is_empty()||rx_len==0 { sd.transfer(card,tx,rx_len) } else { sd.transfer(card,tx,0); sd.transfer(card,&[],rx_len) };
            if card {
                if p.hardware_cs&(1<<9)!=0&&!p.keep_cs&&p.software_cs&(1<<9)==0 { sd.deselect(); }
                return if p.input[1]==Some(2) { rx } else { floating };
            }
        }
        if lcd&&self.lcd_reset_high&&self.dc.is_some() {
            if rx_len!=0||p.full_duplex { self.rejected+=1; return floating; }
            self.panel.dc=self.dc==Some(true);
            // The fixed ST7796 init list overwrites 55h with 05h: both select
            // DBI RGB565. Its 66h three-byte pixel mode is not this subset.
            if self.panel.dc&&self.cmd==0x2c&&!matches!(self.panel.colmod,0x05|0x55) { self.rejected+=1; return floating; }
            for &b in tx { if !self.panel.dc { self.cmd=b; } self.panel.byte(b); }
            if !tx.is_empty() { self.generation=self.generation.wrapping_add(1); }
        }
        floating
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String> {
        let sd=SpiSdCard::new(image,config)?; let description=sd.describe(); self.sd=Some(sd); Ok(description)
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let p=&self.panel; let visible=self.powered&&self.lcd_reset_high&&self.backlight!=0&&p.on&&!p.sleeping;
        // The source BSP explicitly calls MX=true/BGR/INVON its native portrait
        // view. These fixed output conventions do not erase guest MADCTL changes.
        let pixels=(0..480).flat_map(|y|(0..320).map(move|x| {
            if !visible { return 0; } let mut v=p.gram[y*320+(319-x)];
            if p.madctl&8==0 { v=(v&0x7e0)|((v&0xf800)>>11)|((v&31)<<11); }
            if !p.inverted { v= !v; }
            let d=u32::from(self.backlight);
            ((((u32::from(v>>11)*d/255)<<11)|((u32::from((v>>5)&63)*d/255)<<5))|(u32::from(v&31)*d/255))as u16
        })).collect(); Some((320,480,pixels,self.generation))
    }
    fn display_version(&self)->u64 { self.generation }
    fn display_frames(&self)->u64 { self.panel.frames }
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> { Some((self.panel.gram.clone(),320,480)) }
    fn report(&self)->String {
        format!("[waveshare-c5-touch35] ST7796 320x480 pixels={} RAMWR={} duty={}/255 rejected={}; DCDC1_nominal3300={} CH32V00624 EX0=LCD EX1=FT633638 irq3 reports={}; AXP210134 QMI8658-common6b PCF85063A51 ES831118; BOOT28; SD {}; virtual USB supply; factory/RF/audio/ADC/SHTC3/camera/power-key unverified",
            self.panel.pixels_written,self.panel.frames,self.backlight,self.rejected,self.powered,
            self.control.touch.lock().expect("FT6336 state").reports,self.sd.as_ref().map_or_else(||"absent".into(),SpiSdCard::report))
    }
}
