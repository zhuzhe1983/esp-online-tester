//! Explicit virtual C5 clocks. This never creates PLL readiness or calibrated
//! eFuse bytes. Nominal oscillator inputs do not describe physical die timing.
use crate::clock_calibration::{Calibration,Rate};
const PLL_DIV_MASK:u16=0x1ff;const TIMG_READY:u32=0x1c;
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub(crate) struct ClockInputs{pub pll_ready:bool,pub rc_fast_powered:bool,pub slow_dcap:Option<u8>}
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub(crate) struct Root{pub source:u8,pub cpu:u8,pub ahb:u8,pub apb:u8,pub decrease:u8}
pub(crate) struct ClockTree{cycle:u64,inputs:ClockInputs,active:Root,lp_conf:u32,fosc:u32,hp_gate:u32,updates:u64,
    pll_div_gate:u16,tg_conf:u32,tg1_conf:u32,lp_po:u32,xtal_conf:u32,cal_clock:u32,cal:Calibration}
impl ClockTree{
    pub fn new(cycle:u64,active:Root)->Self{Self{cycle,inputs:ClockInputs::default(),active,lp_conf:4,fosc:172<<22,hp_gate:0xf000_0000,updates:0,
        pll_div_gate:PLL_DIV_MASK,tg_conf:1,tg1_conf:1,lp_po:0x7ff,xtal_conf:4,cal_clock:0x700,cal:Calibration::new()}}
    pub fn pump(&mut self,cycle:u64,inputs:ClockInputs){
        let cycle=self.cycle.max(cycle);let rate=self.calibration_rate();let reference_on=self.reference_on();
        self.cal.advance(cycle-self.cycle,rate,reference_on);self.cycle=cycle;self.inputs=inputs;
    }
    fn reference_on(&self)->bool{self.xtal_conf&5==4&&self.tg_conf&2==0}
    fn calibration_rate(&self)->Option<Rate>{match self.cal_clock&7{
        3 if self.hp_gate&(1<<29)!=0=>Some(Rate{hz:self.rtc_slow_hz()?,divider:1}),
        4=>Some(Rate{hz:self.rc_fast_hz()?,divider:(((self.cal_clock>>8)&255)+1)as u16}),
        _=>None, // explicit absent RC32K, XTAL32K and external slow signals
    }}
    pub fn deadline(&self)->Option<u64>{self.cycle.checked_add(self.cal.next(self.calibration_rate(),self.reference_on())?)}
    fn source_hz(&self,source:u8)->Option<u32>{match source{
        0=>Some(48_000_000),1=>self.rc_fast_hz(),2=>self.pll_tap_hz(160),3=>self.pll_tap_hz(240),_=>None,
    }}
    pub fn pll_div_gate_mask(&self)->u16{self.pll_div_gate}
    /// Fixed IDF5.5.3/5.5.4 PCR+128 gates. TRM1.1 omits60/40/20/12M;
    /// these are source-pinned virtual signals, not silicon verification.
    /// Direct SPLL480 is separate and is not gated by this register.
    pub fn pll_tap_hz(&self,mhz:u16)->Option<u32>{
        let bit=match mhz{240=>0,160=>1,120=>2,80=>3,60=>4,48=>5,40=>6,20=>7,12=>8,_=>return None};
        (self.inputs.pll_ready&&self.pll_div_gate&(1<<bit)!=0).then_some(u32::from(mhz)*1_000_000)
    }
    pub fn rc_fast_osc_hz(&self)->Option<u32>{(self.inputs.rc_fast_powered&&self.fosc_dfreq()==100).then_some(17_500_000)}
    pub fn rc_slow_osc_hz(&self)->Option<u32>{(self.inputs.slow_dcap==Some(128)).then_some(136_000)}
    pub fn rc_fast_hz(&self)->Option<u32>{if self.hp_gate&(1<<31)!=0{self.rc_fast_osc_hz()}else{None}}
    /// LP mux is before ICG_HP_FOSC, which gates only the bridge to HP.
    /// Awake LP_DYN_FAST follows LP_FAST; sleep transitions are unsupported.
    pub fn rtc_fast_hz(&self)->Option<u32>{match(self.lp_conf>>2)&3{0=>self.rc_fast_osc_hz(),1=>Some(24_000_000),2=>Some(48_000_000),_=>None}}
    pub fn rtc_slow_hz(&self)->Option<u32>{if self.lp_conf&3==0{self.rc_slow_osc_hz()}else{None}}
    pub fn timg0_control(&self)->u32{self.tg_conf}
    pub fn timg1_control(&self)->u32{self.tg1_conf}
    pub fn cpu_hz(&self)->Option<u32>{Some(self.source_hz(self.active.source)?/(u32::from(self.active.cpu)+1))}
    pub fn cpu_uses_xtal(&self)->bool{self.active.source==0}
    pub fn ahb_hz(&self)->Option<u32>{Some(self.source_hz(self.active.source)?/(u32::from(self.active.ahb)+1))}
    pub fn apb_hz(&self)->Option<u32>{Some(self.ahb_hz()?/(u32::from(self.active.apb)+1))}
    pub fn set_apb(&mut self,apb:u8,decrease:u8){self.active.apb=apb;self.active.decrease=decrease;}
    pub fn commit(&mut self,root:Root)->bool{
        let Some(hz)=self.source_hz(root.source)else{return false;};let cpu=u32::from(root.cpu)+1;let ahb=u32::from(root.ahb)+1;
        if cpu>ahb||!ahb.is_multiple_of(cpu)||hz/ahb>48_000_000||!matches!(root.apb,0|1|3)||!matches!(root.decrease,0|1|3){return false;}
        self.active=root;self.updates=self.updates.saturating_add(1);true
    }
    pub fn fosc_dfreq(&self)->u16{(self.fosc>>22)as u16}
    pub fn report(&self)->String{format!("source={} updates={} {}",self.active.source,self.updates,self.cal.report())}
    pub fn read(&self,address:u32)->Option<u32>{Some(match address{
        0x6009_6144=>0,0x600b_0400=>self.lp_conf,0x600b_0404=>self.lp_po,0x600b_0418=>self.fosc,0x600b_0420=>self.hp_gate,
        0x6009_6128=>u32::from(self.pll_div_gate),
        0x6009_6054=>self.tg_conf|if self.tg_conf&2==0{TIMG_READY}else{0},
        0x6009_6060=>self.tg1_conf|if self.tg1_conf&2==0{TIMG_READY}else{0},0x6009_6130=>self.cal_clock,0x6009_6160=>self.xtal_conf,
        0x6000_8068|0x6000_806c|0x6000_8080=>{if self.tg_conf&1==0{return None;}return self.cal.read(address-0x6000_8000);},_=>return None,
    })}
    pub fn write(&mut self,address:u32,value:u32)->bool{match address{
        0x6009_6128=>{
            // No unsupported USB clock withdrawal: the current USB engine
            // has no independent PLL48 gate input. Do not stop the running
            // CPU root; a later staged commit also checks the actual gate.
            if value&!u32::from(PLL_DIV_MASK)!=0||value&(1<<5)==0
                ||self.active.source==2&&value&2==0||self.active.source==3&&value&1==0{return false;}
            self.pll_div_gate=value as u16;
        },
        // Fixed C5 LL/header fast mux0/1/2. Keep SLOW_SEL0 and the reserved
        // LP_PERI divider0; no external slow signal or mux3 is fabricated.
        // An unpowered/untuned RC input stays unavailable, never XTAL/2.
        0x600b_0400=>{if !matches!(value,0|4|8){return false;}self.lp_conf=value;},
        // Fixed IDF5.5.3/5.5.4 lp_clkrst_reg.h: eleven named pad outputs.
        // Only the original whole-word disable is qualified; internal rates
        // do not consume this latch, and no pad waveform is synthesized.
        0x600b_0404=>{if value!=0{return false;}self.lp_po=0;},
        0x600b_0418=>{if value&0x003f_ffff!=0||value!=self.fosc&&value>>22!=100{return false;}self.fosc=value;},
        0x600b_0420=>{if (value^self.hp_gate)&!0xb000_0000!=0{return false;}self.hp_gate=value;},
        0x6009_6054=>{if value&!0x1f!=0{return false;}
            if value&2!=0{self.cal=Calibration::new();}self.tg_conf=value&3;},
        // pcr_reg.h, IDF5.5.3/5.5.4: TG1 APB0/reset1, RO READY[4:2].
        // Match TG0's control/readback contract without its RTC-cal reset.
        0x6009_6060=>{if value&!0x1f!=0{return false;}self.tg1_conf=value&3;},
        0x6009_6130=>{if value&!0xff07!=0||value&7>4{return false;}
            if value!=self.cal_clock{self.cal.source_changed();}self.cal_clock=value;},
        0x6009_6160=>{if value&!5!=0{return false;}
            if value&1!=0{self.cal=Calibration::new();}self.xtal_conf=value;},
        0x6000_8068|0x6000_806c|0x6000_8080=>{if self.tg_conf&3!=1{return false;}return self.cal.write(address-0x6000_8000,value);},
        _=>return false,
    }true}
    pub fn reset_domains(&mut self,cycle:u64,hp:bool,lp:bool,analog:bool){
        self.pump(cycle,self.inputs);if hp||analog{self.active=Root::default();self.pll_div_gate=PLL_DIV_MASK;self.tg_conf=1;self.tg1_conf=1;self.xtal_conf=4;self.cal_clock=0x700;self.cal=Calibration::new();}
        if lp||analog{self.lp_po=0x7ff;self.lp_conf=4;self.fosc=172<<22;self.hp_gate=0xf000_0000;}
        if analog{self.inputs=ClockInputs::default();}
    }
}
