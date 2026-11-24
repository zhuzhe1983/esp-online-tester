//! ESP32-S3-ePaper-13.3E6, fixed Waveshare f8e9456b schematic and IDF port.
//! WROOM-2-N32R16V; two 600x1600 halves, 4-bit six-color pixels. The exact
//! controller die name is not established by the supplied board/glass manuals.
//! The default matches the factory/shared port: BS1=1, CS-framed standard SPI.
//! The separate IDF 04 example's BS=00 D/C mode is explicit, never auto-detected.
//! 9-bit SPI, QSPI, analog LUTs, temperature and calibrated color are not modeled.
use esp_periph::i2c::I2cDevice;
use esp_soc::{board::{Board,BoardModel,BoardEdge,SpiPins},pins::PinRoutes,
    devices::{es8311::Es8311,es7210::Es7210,i2c_bitbang::I2cBitBang}};

const CPU_HZ:u64=240_000_000;
const HALF_BYTES:usize=600*1600/2;
const REFRESH_CYCLES:u64=19*CPU_HZ;
pub const NAME:&str="waveshare-s3-epaper133e6";
#[derive(Clone,Copy,Debug)]
enum SpiMode{CsFramed,Dc}

struct Die {
    ram:Vec<u8>,cursor:usize,invalid:u64,command:u8,params:[u8;9],param_count:usize,
    psr:bool,tres:bool,power:bool,sleep:bool,commands:u64,bytes:u64,overflow:u64,
}
impl Die {
    fn new()->Self {Self{ram:vec![0x11;HALF_BYTES],cursor:0,invalid:0,command:0xff,params:[0;9],param_count:0,psr:false,tres:false,power:false,sleep:false,commands:0,bytes:0,overflow:0}}
    fn reset(&mut self){self.cursor=0;self.invalid=0;self.command=0xff;self.param_count=0;self.psr=false;self.tres=false;self.power=false;self.sleep=false;}
    fn frame_ready(&self)->bool{self.psr&&self.tres&&self.power&&!self.sleep&&self.cursor==HALF_BYTES&&self.invalid==0&&self.overflow==0}
}
fn color(n:u8)->Option<u16>{match n{0=>Some(0),1=>Some(65535),2=>Some(0xffe0),3=>Some(0xf800),5=>Some(0x001f),6=>Some(0x07e0),_=>None}}

pub struct WaveshareEpaper133 {
    dies:[Die;2],optical:Vec<u16>,frames:u64,cycle:u64,deadline:Option<u64>,pending:u8,
    powered:bool,reset_high:bool,dc:Option<bool>,busy_high:bool,edges:Vec<BoardEdge>,
    i2c:I2cBitBang,i2c_levels:[bool;2],rejected_busy:u64,unsupported:u64,
    refresh_requests:u64,unsupported_refresh:u64,trace:Vec<(u8,u8)>,
    mode:SpiMode,cs_active:u8,command_pending:u8,
}
impl Default for WaveshareEpaper133 {fn default()->Self{Self::new()}}
impl WaveshareEpaper133 {
    pub fn new()->Self{Self{dies:[Die::new(),Die::new()],optical:vec![65535;1600*1200],frames:0,cycle:0,deadline:None,pending:0,powered:false,reset_high:false,dc:None,busy_high:false,edges:Vec::new(),i2c:I2cBitBang::new(),i2c_levels:[true;2],rejected_busy:0,unsupported:0,refresh_requests:0,unsupported_refresh:0,trace:Vec::new(),mode:SpiMode::CsFramed,cs_active:0,command_pending:0}}
    /// A different physical BS switch configuration for the official IDF 04
    /// example. It is not selected automatically from guest byte patterns.
    pub fn dc_mode()->Self{Self{mode:SpiMode::Dc,..Self::new()}}
    fn set_busy(&mut self,at:u64,high:bool){if self.busy_high!=high{self.busy_high=high;self.edges.push(BoardEdge{cycle:at,pin:12,level:high});}}
    fn reset_logic(&mut self){for d in &mut self.dies{d.reset();}self.deadline=None;self.pending=0;self.cs_active=0;self.command_pending=0;}
    fn sync_busy(&mut self){self.set_busy(self.cycle,self.powered&&self.reset_high&&self.deadline.is_none());}
    fn write(&mut self,selected:u8,data:bool,bytes:&[u8]){
        self.write_phased(if data{0}else{selected},if data{selected}else{0},bytes);
    }
    fn write_phased(&mut self,command_mask:u8,data_mask:u8,bytes:&[u8]){
        for (at,&b) in bytes.iter().enumerate(){
            if self.deadline.is_some(){self.rejected_busy+=(bytes.len()-at) as u64;break;}
            let mut refresh=0;let mut digital_transition=false;
            for i in 0..2 {
                if (command_mask|data_mask)&(1<<i)==0{continue;}
                let d=&mut self.dies[i];if d.sleep{continue;}
                if command_mask&(1<<i)!=0 {
                    d.command=b;d.param_count=0;d.commands+=1;
                    if self.trace.len()<64{self.trace.push((i as u8,b));}
                    match b {
                        0x00=>d.psr=false,0x61=>d.tres=false,
                        0x10=>{d.cursor=0;d.invalid=0;d.overflow=0;},
                        0x04=>{d.power=true;digital_transition=true;},
                        0x01|0x02|0x05|0x06|0x07|0x12|0x50|0x60|0x74|0x86|0xb0|0xb1|0xb6|0xb7|0xe0|0xe3|0xf0=>{},
                        _=>self.unsupported+=1,
                    }
                }else if d.command==0x10 {
                    if d.cursor<HALF_BYTES{d.ram[d.cursor]=b;d.cursor+=1;d.bytes+=1;d.invalid+=u64::from(color(b>>4).is_none())+u64::from(color(b&15).is_none());}
                    else{d.overflow+=1;}
                }else{
                    if d.param_count<d.params.len(){d.params[d.param_count]=b;}
                    d.param_count=d.param_count.saturating_add(1);
                    match(d.command,d.param_count){
                        (0x00,2)=>d.psr=d.params[..2]==[0xdf,0x69],
                        (0x61,4)=>d.tres=d.params[..4]==[0x04,0xb0,0x03,0x20],
                        (0x07,1)if b==0xa5=>d.sleep=true,
                        (0x02,1)if b==0=>{d.power=false;digital_transition=true;},
                        (0x12,1)=>{self.refresh_requests+=1;if b==0&&d.frame_ready(){refresh|=1<<i;}else{self.unsupported_refresh+=1;}},
                        _=>{},
                    }
                    // Extra configuration bytes are not silently accepted as
                    // a verified PSR/TRES setting, nor used for allocation.
                    if d.command==0x00&&d.param_count>2{d.psr=false;}
                    if d.command==0x61&&d.param_count>4{d.tres=false;}
                }
            }
            if refresh!=0 {
                self.pending=refresh;self.deadline=Some(self.cycle.saturating_add(REFRESH_CYCLES));
            }else if digital_transition {
                // PON/POF completion is untimed digital behavior: one cycle,
                // explicitly not a fabricated power-rail settling duration.
                self.deadline=Some(self.cycle.saturating_add(1));
            }
            self.sync_busy();
        }
    }
    fn finish_refresh(&mut self){
        if self.pending==0{return;}
        for i in 0..2{if self.pending&(1<<i)!=0{
            for (at,&b) in self.dies[i].ram.iter().enumerate(){
                let ny=at/300;let nx=i*600+(at%300)*2;
                // Source landscape BMP -> native is (H-1-y,x). Its inverse
                // below presents 1600x1200 without stretching either half.
                self.optical[(1199-nx)*1600+ny]=color(b>>4).expect("validated six-color RAM");
                self.optical[(1198-nx)*1600+ny]=color(b&15).expect("validated six-color RAM");
            }
        }}
        self.frames+=1;self.pending=0;
    }
}
impl BoardModel for WaveshareEpaper133 {
    fn name(&self)->&'static str{NAME}
    // BUSY and I2C input edges must be drained by the SoC, including immediately
    // before GPIO_IN reads; this is independent of GPIO waveform scheduling.
    fn uses_gpio_edges(&self)->bool{true}
    fn uses_gpio_routes(&self)->bool{true}
    fn uses_spi_pins(&self)->bool{true}
    fn gpio_waveform_at(&mut self,cycle:u64,p:PinRoutes<'_>){
        self.advance_to(cycle);
        let powered=p.software_output(1)==Some(true);let reset=p.software_output(2)==Some(true);
        if (self.powered&&!powered)||(self.reset_high&&!reset){self.reset_logic();}
        self.powered=powered;self.reset_high=reset;self.dc=p.software_output(11);
        let cs=u8::from(p.software_output(10)==Some(false))|(u8::from(p.software_output(3)==Some(false))<<1);
        self.command_pending|=cs&!self.cs_active;self.command_pending&=cs;self.cs_active=cs;
        let scl=p.software_output(42).unwrap_or(true);let sda=self.i2c.drive(scl,p.software_output(41).unwrap_or(true));
        for (i,(pin,level)) in [(41,sda),(42,scl)].into_iter().enumerate(){if self.i2c_levels[i]!=level{self.i2c_levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
        self.sync_busy();
    }
    fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true),(12,self.busy_high),(4,true),(5,true),(7,true),(15,true),(16,true),(41,self.i2c_levels[0]),(42,self.i2c_levels[1])]}
    fn named_pin(&self,id:&str)->Option<u8>{match id{"boot"|"btn"|"btn1"=>Some(0),_=>None}}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{
        self.i2c.clear();self.i2c.attach(0x18,Box::new(Es8311::new()));self.i2c.attach(0x40,Box::new(Es7210::new()));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((41,42))).into_iter().map(move|(a,d)|(bus,a,d))).collect()
    }
    fn spi_transfer_pins(&mut self,host:u8,p:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8>{
        if host==3&&self.powered&&self.reset_high&&p.sclk&(1<<9)!=0&&p.mosi&(1<<46)!=0{
            let selected=u8::from(p.cs&(1<<10)!=0)|(u8::from(p.cs&(1<<3)!=0)<<1);
            if selected!=0{match self.mode{
                SpiMode::Dc=>if let Some(dc)=self.dc{self.write(selected,dc,tx);},
                SpiMode::CsFramed=>{
                    // The source uses explicit GPIO CS and keeps it asserted
                    // across 4096-byte SPI transactions. A host transaction
                    // boundary alone must never start a new command.
                    let selected=selected&self.cs_active;
                    if selected!=0{for &b in tx{let commands=self.command_pending&selected;self.command_pending&=!selected;self.write_phased(commands,selected&!commands,&[b]);}}
                },
            }}
        }
        // SI1..3 are unconnected on this PCB. Do not invent an ID/readback.
        vec![255;rx_len]
    }
    fn advance_to(&mut self,cycle:u64){
        self.cycle=cycle;
        if let Some(at)=self.deadline.filter(|&at|at<=cycle){self.finish_refresh();self.deadline=None;self.set_busy(at,self.powered&&self.reset_high);}
        self.sync_busy();
    }
    fn next_deadline(&self)->Option<u64>{self.deadline}
    fn take_edges(&mut self)->Vec<BoardEdge>{let mut e=std::mem::take(&mut self.edges);e.sort_by_key(|e|e.cycle);e}
    fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some(([6,7,5,4,16,15],true))}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{Some((1600,1200,self.optical.clone(),self.frames))}
    fn display_frames(&self)->u64{self.frames}
    fn display_version(&self)->u64{self.frames}
    fn display_push_hz(&self)->u64{5}
    fn report(&self)->String{
        format!("[{NAME}] 32F/16P E6 dual-die native1200x1600 view1600x1200 frames={} bytes={:?} cursor={:?} commands={:?} ready={:?} power={} reset={} busyLow={} deadline={:?} pending={} refreshRequests={} unsupportedRefresh={} rejectedBusy={} unsupportedCommands={} invalid={:?} overflow={:?} trace={:x?}; mode={:?} nominalFullRefresh=19000ms PON/POF=untimed1cycle SPI3=9/46/10+3/11/2/12 power1 SDMMC=6/7/5/4/16/15 codec18+40; analog/colorimetry/temperature/9bit/QSPI/audioPCM/radio unimplemented",self.frames,self.dies.each_ref().map(|d|d.bytes),self.dies.each_ref().map(|d|d.cursor),self.dies.each_ref().map(|d|d.commands),self.dies.each_ref().map(|d|d.frame_ready()),self.powered,!self.reset_high,!self.busy_high,self.deadline,self.pending,self.refresh_requests,self.unsupported_refresh,self.rejected_busy,self.unsupported,self.dies.each_ref().map(|d|d.invalid),self.dies.each_ref().map(|d|d.overflow),self.trace,self.mode)
    }
}
pub fn make_board(name:&str)->Option<Board>{(name==NAME).then(||Box::new(WaveshareEpaper133::new()) as Board)}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup()->(WaveshareEpaper133,esp_periph::Gpio,esp_periph::RegRam){
        let mut b=WaveshareEpaper133::dc_mode();let mut g=esp_periph::Gpio::new();let mut mux=esp_periph::RegRam::new();
        for pin in [1,2,11]{mux.write(4+4*pin,1<<12);g.enable|=1<<pin;}g.out=(1<<1)|(1<<2);
        b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&g,&mux));(b,g,mux)
    }
    fn pins(cs:u64)->SpiPins{SpiPins{sclk:1<<9,mosi:1<<46,cs,..Default::default()}}
    fn send(b:&mut WaveshareEpaper133,selected:u8,command:u8,data:&[u8]){b.write(selected,false,&[command]);b.write(selected,true,data);}
    fn frame(b:&mut WaveshareEpaper133){
        send(b,3,0x00,&[0xdf,0x69]);send(b,3,0x61,&[4,0xb0,3,0x20]);
        send(b,1,0x10,&vec![0x23;HALF_BYTES]);send(b,2,0x10,&vec![0x56;HALF_BYTES]);
        send(b,3,0x04,&[]);b.advance_to(b.cycle+1);
    }
    #[test]
    fn real_dual_half_data_busy_deadline_and_source_rotation(){
        let (mut b,_,_)=setup();frame(&mut b);let start=b.cycle;
        assert_eq!(b.frames,0);assert!(b.optical.iter().all(|&p|p==65535));
        send(&mut b,3,0x12,&[0]);assert_eq!(b.pending,3);assert!(!b.busy_high);assert_eq!(b.next_deadline(),Some(start+REFRESH_CYCLES));
        send(&mut b,1,0x10,&[0]);assert_eq!(b.rejected_busy,2);
        b.advance_to(start+REFRESH_CYCLES-1);assert_eq!(b.frames,0);
        b.advance_to(start+REFRESH_CYCLES);assert_eq!(b.frames,1);assert!(b.busy_high);
        assert_eq!(b.optical[1199*1600],0xffe0);assert_eq!(b.optical[1198*1600],0xf800);
        assert_eq!(b.optical[599*1600],0x001f);assert_eq!(b.optical[598*1600],0x07e0);
        assert_eq!(b.optical[1600-1],0x07e0);assert_eq!(b.optical[600*1600+1599],0xf800);
        let edges=b.take_edges();assert!(edges.contains(&BoardEdge{cycle:start+REFRESH_CYCLES,pin:12,level:true}));
    }
    #[test]
    fn routing_power_reset_and_retained_optical_image(){
        let (mut b,mut gpio,mut mux)=setup();
        let cmd=[0x74];let original=b.dies[0].commands;
        b.spi_transfer_pins(2,pins(1<<10),&cmd,0);
        b.spi_transfer_pins(3,SpiPins{sclk:1<<8,..pins(1<<10)},&cmd,0);
        b.spi_transfer_pins(3,SpiPins{mosi:1<<45,..pins(1<<10)},&cmd,0);
        b.spi_transfer_pins(3,pins(1<<8),&cmd,0);assert_eq!(b.dies[0].commands,original);
        b.spi_transfer_pins(3,pins(1<<3),&cmd,0);assert_eq!(b.dies[1].commands,1);assert_eq!(b.dies[0].commands,0);
        mux.write(4+4*11,0);b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&gpio,&mux));
        b.spi_transfer_pins(3,pins(1<<10),&cmd,0);assert_eq!(b.dies[0].commands,0,"D/C must be physically driven");
        mux.write(4+4*11,1<<12);b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&gpio,&mux));
        frame(&mut b);send(&mut b,3,0x12,&[0]);b.advance_to(b.cycle+REFRESH_CYCLES);let visible=b.optical.clone();
        send(&mut b,3,0x12,&[0]);gpio.enable&=!(1<<1);b.gpio_waveform_at(b.cycle+10,esp_soc::pins::ChipPins::S3.routes(&gpio,&mux));
        assert!(!b.powered);assert!(b.deadline.is_none());assert_eq!(b.frames,1);assert_eq!(b.optical,visible);
        gpio.enable|=1<<1;gpio.func_out_sel[1]|=1<<9;b.gpio_waveform_at(b.cycle+1,esp_soc::pins::ChipPins::S3.routes(&gpio,&mux));assert!(!b.powered,"inverted rail enable is off");
        gpio.func_out_sel[1]&=!(1<<9);gpio.out&=!(1<<2);b.gpio_waveform_at(b.cycle+1,esp_soc::pins::ChipPins::S3.routes(&gpio,&mux));assert!(!b.reset_high);assert_eq!(b.optical,visible);
    }
    #[test]
    fn incomplete_unknown_mode_invalid_colors_and_overflow_do_not_publish(){
        let (mut b,_,_)=setup();send(&mut b,3,0x12,&[0]);assert_eq!(b.unsupported_refresh,2);assert!(b.deadline.is_none());
        frame(&mut b);send(&mut b,1,0x61,&[255;9]);send(&mut b,1,0x12,&[0]);assert!(b.deadline.is_none());
        send(&mut b,1,0x61,&[4,0xb0,3,0x20]);send(&mut b,1,0x10,&vec![0x44;HALF_BYTES]);send(&mut b,1,0x12,&[0]);assert_eq!(b.dies[0].invalid,(HALF_BYTES*2)as u64);assert!(b.deadline.is_none());
        send(&mut b,1,0x10,&vec![0;HALF_BYTES+64]);send(&mut b,1,0x12,&[0]);assert_eq!(b.dies[0].ram.len(),HALF_BYTES);assert_eq!(b.dies[0].overflow,64);assert_eq!(b.frames,0);
        assert_eq!(b.named_pin("boot"),Some(0));for id in ["power","user","reset"]{assert_eq!(b.named_pin(id),None);}
        assert_eq!(b.sd_card_slot(),Some(([6,7,5,4,16,15],true)));
        assert_eq!(b.i2c_devices().iter().map(|(bus,addr,_)|(*bus,*addr)).collect::<Vec<_>>(),[(0,0x18),(0,0x40),(1,0x18),(1,0x40)]);
        assert_eq!(b.spi_transfer_pins(3,pins(1<<10),&[],3),[255;3]);
    }
    #[test]
    fn standard_spi_command_is_cs_edge_not_transfer_boundary_or_dc_level(){
        let (mut b,mut g,mut mux)=setup();b.mode=SpiMode::CsFramed;
        for pin in [3,10]{mux.write(4+4*pin,1<<12);g.enable|=1<<pin;g.out|=1<<pin;}
        b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&g,&mux));
        g.out&=!(1<<10);b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&g,&mux));
        b.spi_transfer_pins(3,pins(1<<10),&[0x10],0);
        b.spi_transfer_pins(3,pins(1<<10),&[0x12,0x35],0);
        g.out|=1<<11;b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&g,&mux));
        b.spi_transfer_pins(3,pins(1<<10),&[0x66],0);
        assert_eq!(&b.dies[0].ram[..3],&[0x12,0x35,0x66]);assert_eq!(b.dies[0].commands,1);assert_eq!(b.dies[1].cursor,0);
        g.out|=1<<10;g.out&=!(1<<3);b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&g,&mux));
        b.spi_transfer_pins(3,pins(1<<3),&[0x10,0x55],0);assert_eq!(b.dies[1].cursor,1);assert_eq!(b.dies[1].ram[0],0x55);
        g.out|=1<<3;b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&g,&mux));
        b.spi_transfer_pins(3,pins(1<<3),&[0x66],0);assert_eq!(b.dies[1].cursor,1,"a hardware-only transaction cannot masquerade as the source GPIO-CS framing");
    }
}
