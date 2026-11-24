//! Sixteen exact Waveshare S3 core SKU profiles; source manifest and native reproduction:
//! docs/evidence/waveshare-s3-core-2026-11-23 and docs/research/waveshare-s3-industrial-2026-11-24/batch-04.
//! No display or invented peripherals. Header options do not change electrical routing.
//! Zero: official BlinkRGB/Arduino wire order GRB, GPIO21, FH4R2 (4F/2P).
//! Pico and DEV-KIT-N8R8: official rgb.py explicitly selects RGB wire order;
//! schematic GPIO21 / GPIO38 respectively, 16F/2P and 8F/8P.
//! Nano: schematic common-anode channel nets 46/0/45; R/G/B names follow its
//! official Arduino demo and Nano variant mapping. The conflicting schematic
//! package pin numbers are retained in the evidence. Separate GPIO48 SCK LED
//! is active high. The only documented physical key is RESET.
//! WS2812-class pulse limits and RGB intensity are digital display policy,
//! not calibrated LED timing, voltage, optical brightness or radio behavior.
use esp_soc::{board::{Board,BoardModel},devices::Ws2812Chain,pins::PinRoutes};

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Variant {Zero,ZeroM,ZeroN8R8,ZeroN8R8M,Pico,PicoM,Nano,NanoM,DevKitN8R8,DevKitN8R8M,DevKitN16R8,DevKitN16R8M,DevKitN32R16V,DevKitN32R16VM,DevKitN16R8U,DevKitN16R8UM}
impl Variant {
    pub const ALL:[Self;16]=[Self::Zero,Self::ZeroM,Self::ZeroN8R8,Self::ZeroN8R8M,Self::Pico,Self::PicoM,Self::Nano,Self::NanoM,Self::DevKitN8R8,Self::DevKitN8R8M,Self::DevKitN16R8,Self::DevKitN16R8M,Self::DevKitN32R16V,Self::DevKitN32R16VM,Self::DevKitN16R8U,Self::DevKitN16R8UM];
    pub fn name(self)->&'static str {match self {
        Self::Zero=>"waveshare-s3-zero",Self::ZeroM=>"waveshare-s3-zero-m",Self::ZeroN8R8=>"waveshare-s3-zero-n8r8",Self::ZeroN8R8M=>"waveshare-s3-zero-n8r8-m",
        Self::Pico=>"waveshare-s3-pico",Self::PicoM=>"waveshare-s3-pico-m",Self::Nano=>"waveshare-s3-nano",Self::NanoM=>"waveshare-s3-nano-m",
        Self::DevKitN8R8=>"waveshare-s3-devkit-n8r8",Self::DevKitN8R8M=>"waveshare-s3-devkit-n8r8-m",Self::DevKitN16R8=>"waveshare-s3-devkit-n16r8",Self::DevKitN16R8M=>"waveshare-s3-devkit-n16r8-m",
        Self::DevKitN32R16V=>"waveshare-s3-devkit-n32r16v",Self::DevKitN32R16VM=>"waveshare-s3-devkit-n32r16v-m",Self::DevKitN16R8U=>"waveshare-s3-devkit-n16r8-u",Self::DevKitN16R8UM=>"waveshare-s3-devkit-n16r8-um",
    }}
    pub fn memory_mib(self)->(usize,usize) {match self {
        Self::Zero|Self::ZeroM=>(4,2),Self::ZeroN8R8|Self::ZeroN8R8M|Self::DevKitN8R8|Self::DevKitN8R8M=>(8,8),Self::Pico|Self::PicoM=>(16,2),
        Self::DevKitN32R16V|Self::DevKitN32R16VM=>(32,16),_ =>(16,8),
    }}
    /// Exact WROOM-2 N32R16V option: SPI0/1 OPI identity and FLASH_TYPE eFuse.
    /// Other variants retain their documented QSPI flash; PSRAM size is separate.
    pub fn flash_octal(self)->bool {matches!(self,Self::DevKitN32R16V|Self::DevKitN32R16VM)}
    pub fn nano(self)->bool {matches!(self,Self::Nano|Self::NanoM)}
    pub fn boot(self)->Option<u8> {(!self.nano()).then_some(0)}
    fn addressable_pin(self)->Option<u8> {match self {Self::Zero|Self::ZeroM|Self::ZeroN8R8|Self::ZeroN8R8M|Self::Pico|Self::PicoM=>Some(21),Self::Nano|Self::NanoM=>None,_=>Some(38)}}
    fn rgb_wire(self)->bool {!matches!(self,Self::Zero|Self::ZeroM|Self::ZeroN8R8|Self::ZeroN8R8M|Self::Nano|Self::NanoM)}
}
pub fn variant(name:&str)->Option<Variant> {Variant::ALL.into_iter().find(|v|v.name()==name)}
pub fn make_board(name:&str)->Option<Board> {variant(name).map(|v|Box::new(WaveshareCore::new(v))as Board)}

pub struct WaveshareCore {variant:Variant,chain:Ws2812Chain,leds:[[u8;3];2],version:u64,gpio:[Option<bool>;4]}
impl WaveshareCore {
    pub fn new(variant:Variant)->Self {Self{variant,chain:Ws2812Chain::new(1).with_gpio_clock(240_000_000),leds:[[0;3];2],version:0,gpio:[None;4]}}
    fn sync_chain(&mut self) {let [r,g,b]=self.chain.leds[0];self.leds[0]=if self.variant.rgb_wire(){[g,r,b]}else{[r,g,b]};self.version=self.chain.updates;}
    fn sync_nano(&mut self,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {
        let mut channel=[0;4];
        for (i,pin) in [46,0,45,48].into_iter().enumerate() {
            let high=pwm(pin).filter(|(hz,d)|hz.is_finite()&&*hz>0.0&&*d<=65535).map(|(_,d)|d)
                .or(self.gpio[i].map(|v|if v {65535}else{0}));
            if let Some(high)=high {let duty=if i<3 {65535-high}else{high};channel[i]=((duty*255+32767)/65535)as u8;}
        }
        let next=[[channel[0],channel[1],channel[2]],[channel[3],channel[3],0]];
        if self.leds!=next {self.leds=next;self.version=self.version.wrapping_add(1);}
    }
}
impl BoardModel for WaveshareCore {
    fn name(&self)->&'static str {self.variant.name()}
    fn uses_gpio_edges(&self)->bool {false}
    fn uses_gpio_routes(&self)->bool {true}
    fn uses_gpio_waveform(&self)->bool {self.variant.addressable_pin().is_some()}
    fn uses_gpio_pwm(&self)->bool {self.variant.nano()}
    fn gpio_waveform_at(&mut self,cycle:u64,p:PinRoutes<'_>) {
        if let Some(pin)=self.variant.addressable_pin() {let level=p.software_output(pin);self.chain.gpio_drive(cycle,level.is_some(),level.unwrap_or(false));self.sync_chain();}
        else {self.gpio=[46,0,45,48].map(|pin|p.software_output(pin));self.sync_nano(&|_|None);}
    }
    fn gpio_pwm_at(&mut self,_cycle:u64,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {if self.variant.nano() {self.sync_nano(pwm);}}
    fn next_deadline(&self)->Option<u64> {self.variant.addressable_pin().and_then(|_|self.chain.gpio_deadline())}
    fn advance_to(&mut self,cycle:u64) {if self.variant.addressable_pin().is_some(){self.chain.advance_gpio(cycle);self.sync_chain();}}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if self.variant.addressable_pin()==Some(pin){self.chain.from_bits(bits);self.sync_chain();}}
    fn input_levels(&self)->Vec<(u8,bool)> {self.variant.boot().into_iter().map(|pin|(pin,true)).collect()}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"boot"|"btn"|"btn1"=>self.variant.boot(),_=>None}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.leds[..if self.variant.nano(){2}else{1}],self.version))}
    fn report(&self)->String {let (flash,psram)=self.variant.memory_mib();format!("[{}] {flash}F/{psram}P boot={:?} ledUpdates={} rgb={:?} wireFrames={} addressablePin={:?} RGBwire={} discreteRGB={} radio/USBhub/analog=unmodeled",self.name(),self.variant.boot(),self.version,self.leds().unwrap().0,self.chain.updates,self.variant.addressable_pin(),self.variant.rgb_wire(),self.variant.nano())}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bits(bytes:&[u8])->Vec<bool> {bytes.iter().flat_map(|b|(0..8).map(move |i|b&(0x80>>i)!=0)).collect()}
    #[test]
    fn exact_variant_memory_buttons_and_addressable_color_order() {
        for (v,memory,pin,rgb) in [(Variant::Zero,(4,2),21,[0x34,0x12,0x56]),(Variant::Pico,(16,2),21,[0x12,0x34,0x56]),(Variant::DevKitN8R8,(8,8),38,[0x12,0x34,0x56])] {
            let mut b=WaveshareCore::new(v);assert_eq!(v.memory_mib(),memory);assert_eq!(b.input_levels(),[(0,true)]);assert_eq!(b.named_pin("boot"),Some(0));assert!(b.display().is_none());assert!(b.i2c_devices().is_empty());
            b.rmt_frame(pin+1,&bits(&[0x12,0x34,0x56]));assert_eq!(b.version,0);b.rmt_frame(pin,&[true;23]);assert_eq!(b.version,0);
            b.rmt_frame(pin,&bits(&[0x12,0x34,0x56]));assert_eq!(b.leds().unwrap(),(&[rgb][..],1));
        }
        let mut nano=WaveshareCore::new(Variant::Nano);assert_eq!(nano.variant.memory_mib(),(16,8));assert_eq!(nano.named_pin("boot"),None);assert!(nano.input_levels().is_empty());nano.rmt_frame(21,&[true;24]);assert_eq!(nano.version,0);assert!(make_board("waveshare-s3-zero-n32r16v").is_none());
    }
    #[test]
    fn nano_discrete_leds_use_physical_enable_mux_inversion_and_pwm() {
        let mut b=WaveshareCore::new(Variant::Nano);let mut g=esp_periph::Gpio::new();let mut mux=esp_periph::RegRam::new();
        for pin in [0,45,46,48] {mux.write(4+4*pin,1<<12);g.enable|=1<<pin;}
        g.out=1<<48;b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&g,&mux));assert_eq!(b.leds,[ [255;3],[255,255,0] ]);
        g.out|=1<<46;g.enable&=!(1<<0);mux.write(4+4*45,0);b.gpio_waveform_at(1,esp_soc::pins::ChipPins::S3.routes(&g,&mux));assert_eq!(b.leds[0],[0;3]);
        g.func_out_sel[46]|=1<<9;b.gpio_waveform_at(2,esp_soc::pins::ChipPins::S3.routes(&g,&mux));assert_eq!(b.leds[0],[255,0,0]);
        g.func_out_sel[46]&=!(1<<9);g.func_out_sel[46]=73;g.func_out_sel[48]=74;b.gpio_waveform_at(3,esp_soc::pins::ChipPins::S3.routes(&g,&mux));
        b.gpio_pwm_at(3,&|pin|match pin {46=>Some((1000.,16384)),48=>Some((500.,32768)),_=>None});assert_eq!(b.leds,[[191,0,0],[128,128,0]]);
        b.gpio_pwm_at(4,&|_|None);assert_eq!(b.leds,[[0;3];2]);
    }
    #[test]
    fn gpio_addressable_pixels_require_routed_pulses_and_latch() {
        let mut b=WaveshareCore::new(Variant::Zero);let mut g=esp_periph::Gpio::new();let mut mux=esp_periph::RegRam::new();let pin=21;
        mux.write(4+4*pin,1<<12);g.enable|=1<<pin;let mut cycle=0;
        for bit in bits(&[1,2,3]) {g.out|=1<<pin;b.gpio_waveform_at(cycle,esp_soc::pins::ChipPins::S3.routes(&g,&mux));cycle+=if bit{192}else{96};g.out&=!(1<<pin);b.gpio_waveform_at(cycle,esp_soc::pins::ChipPins::S3.routes(&g,&mux));cycle+=if bit{108}else{204};}
        let deadline=b.next_deadline().unwrap();b.advance_to(deadline-1);assert_eq!(b.version,0);b.advance_to(deadline);assert_eq!(b.leds().unwrap(),(&[[2,1,3]][..],1));
        // An unrouted pin cannot keep a partly-received stream alive.
        g.out|=1<<pin;b.gpio_waveform_at(deadline+100,esp_soc::pins::ChipPins::S3.routes(&g,&mux));g.enable=0;b.gpio_waveform_at(deadline+200,esp_soc::pins::ChipPins::S3.routes(&g,&mux));assert!(b.next_deadline().is_none());assert_eq!(b.version,1);
    }
}
