//! C5 boot analog controls, real REGI2C byte transfers and virtual SPLL.
//! ESP-IDF v5.5.3/v5.5.4 C5 register headers, LL and HP REGI2C patch driver;
//! source/reset/profile limits: docs/evidence/c5-boot-analog-2026-11-24.
//! Modem ICG follow-up: docs/evidence/c5-modem-domain-2026-11-24.
//! WiFi ICG map fields: docs/evidence/c5-wifi-modem-map-2026-11-25.
//! COEX gate request: docs/evidence/c5-coex-gate-2026-11-25.
//! Targeted DIG_REG/PLL lifecycle: docs/evidence/c5-pll-boot-2026-11-24.
//! SAR configuration: docs/evidence/c5-saradc-entropy-2026-11-24.
//! Ideal OCODE handshake: docs/evidence/c5-ocode-handshake-2026-11-24.
//! BOD threshold byte: docs/evidence/c5-bod0-startup-2026-11-25.
//! The stable USB3V3 supply and VDET byte are explicit virtual board inputs,
//! not silicon POR measurements. Physical ADC/glitch events unsupported.
const PCR_BYPASS:u32=0x6009_6174;
const MODEM_CLK:u32=0x600a_f018;const MODEM_FORCE:u32=0x600a_f01c;
const MODEM_SELECT:u32=0x600a_9c04;const RF_PWC:u32=0x600b_0158;
const BOD1:u32=0x600b_2c04;const GLITCH:u32=0x600b_2c08;const FIB:u32=0x600b_2c0c;
const MASTER:u32=0x600a_f800;const SAR_ROUTE:u32=0x00ff_fdff;
const PMU_ACTIVE_MODEM:u32=0x600b_000c;const PMU_MODEM_UPDATE:u32=0x600b_00dc;const PMU_SWITCH_UPDATE:u32=0x600b_00d0;
const MODEM_MAP:u32=0x600a_9c0c;const LPCON_MAP:u32=0x600a_f020;
const MODEM_MAP_INIT:u32=0x6464_6400;const LPCON_MAP_INIT:u32=0x6666_0000;
const DIG_ROUTE:u32=0x00ff_fbff;const PLL_ROUTE:u32=0x00ff_ff7f;
const ULP_ROUTE:u32=0x00ff_feff;
#[path="boot_analog_pll.rs"]mod pll;
#[path="boot_analog_sar.rs"]mod sar;
#[path="boot_analog_ocode.rs"]mod ocode;
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct C5BootAnalog{
    bypass:u32,clock:u32,force:u32,select:u32,rf:u32,bod:u32,glitch:u32,fib:u32,
    ctrl:[u32;2],conf1:u32,conf2:u32,vdet:u8,reads:u64,writes:u64,
    active_modem:u32,captured_code:u8,modem_map:u32,lp_map:u32,permissions:Option<u8>,code_updates:u64,switch_updates:u64,
    analog:pll::Analog,sar:sar::Sar,ocode:ocode::Ocode,
    bod_threshold_profile:bool,bod_byte:u8,bod_written:bool,
}

impl C5BootAnalog{
    /// Stable virtual USB3V3 board: all four VDET inputs initially enabled.
    /// Public sources name these bits but give no analog-byte POR value.
    pub fn virtual_usb3v3()->Self{Self{bypass:2,clock:0,force:0,select:0x0020_0002,rf:0x0800_0000,bod:0,glitch:0,fib:u32::MAX,
        ctrl:[0;2],conf1:0x2d,conf2:4,vdet:0x0f,reads:0,writes:0,
        active_modem:0,captured_code:0,modem_map:0,lp_map:0,permissions:None,code_updates:0,switch_updates:0,analog:pll::Analog::new(),sar:sar::Sar::new(),ocode:ocode::Ocode::new(),
        bod_threshold_profile:false,bod_byte:0,bod_written:false}}
    pub fn enable_virtual_pll(&mut self,cycle:u64){self.analog.enable(cycle);self.reconcile_pll();}
    pub fn disable_virtual_pll(&mut self){self.analog.disable();self.ocode.reconcile(false);}
    pub fn pll_ready(&self)->bool{self.analog.ready()}
    pub fn rc_fast_powered(&self)->bool{self.analog.rc_fast_powered()}
    pub fn slow_dcap(&self)->Option<u8>{self.analog.slow_dcap()}
    /// Applied LPCON LP_APB permission; bootstrap I2C routing is not a grant.
    pub fn lp_apb_accessible(&self)->bool{self.permissions.is_some_and(|p|p&2!=0)}
    /// Applied MODEM_APB permission only; no bootstrap or WiFi-request grant.
    pub fn modem_apb_accessible(&self)->bool{self.permissions.is_some_and(|p|p&1!=0)}
    /// CLK_CONF.COEX_EN is a request only, not an effective RF/timer clock.
    pub fn coex_clock_requested(&self)->bool{self.clock&2!=0}
    pub fn enable_virtual_sar(&mut self){self.sar.enable();}
    pub fn disable_virtual_sar(&mut self){self.sar.disable();}
    /// Explicit ideal reference/target clock and virtual100us completion,
    /// with no numeric code or physical voltage/calibration implication.
    /// Re-selecting an active profile preserves its actual protocol history.
    pub fn enable_virtual_ocode_ideal(&mut self,cycle:u64){self.pump(cycle);self.ocode.enable(cycle);}
    pub fn disable_virtual_ocode(&mut self){self.ocode.disable();}
    pub fn virtual_ocode_enabled(&self)->bool{self.ocode.enabled}
    /// Explicit code7-only BOD target profile. Byte0 is virtual, not POR;
    /// selection does not qualify a detector until a real write succeeds.
    pub fn enable_virtual_bod_threshold(&mut self){self.bod_threshold_profile=true;}
    pub fn disable_virtual_bod_threshold(&mut self){self.bod_threshold_profile=false;}
    pub fn bod_threshold(&self)->Option<u8>{(self.bod_threshold_profile&&self.bod_written).then_some(self.bod_byte&7)}
    /// Supply the actual committed CPU root observation, never a staged mux
    /// or !pll_ready shortcut. Settle elapsed time with the old input first.
    pub fn observe_ocode_cpu_xtal(&mut self,cycle:u64,cpu_xtal:bool){
        if !self.virtual_ocode_enabled(){return;}
        self.pump(cycle);self.ocode.observe_cpu_xtal(cpu_xtal,self.analog.ocode_quiet());
    }
    /// Actual guest target writes and PMU reset-release/power, independent of
    /// the master clock used to configure them. Not ADC conversion readiness.
    pub fn sar_reserved_path_ready(&self)->bool{self.rf&0x0c00_0000==0x0c00_0000&&self.sar.ready()}
    pub fn deadline(&self)->Option<u64>{match(self.analog.deadline,self.ocode.deadline){(Some(a),Some(b))=>Some(a.min(b)),(a,b)=>a.or(b)}}
    fn reconcile_pll(&mut self){if self.analog.enabled{self.analog.reconcile(self.clocked(),self.conf1==PLL_ROUTE);}}
    pub fn pump(&mut self,cycle:u64){self.analog.pump(cycle,self.clocked(),self.conf1==PLL_ROUTE);self.ocode.pump(cycle,self.analog.ocode_quiet());}
    pub fn read_at(&mut self,address:u32,cycle:u64)->Option<u32>{self.pump(cycle);self.read(address)}
    pub fn write_at(&mut self,address:u32,value:u32,cycle:u64)->bool{self.pump(cycle);self.write(address,value)}
    fn clocked(&self)->bool{
        // rtc_clk_init pre-enables MODEM_APB/I2C_MST/LP_APB for the master.
        // FORCE bypasses its functional permission, never either APB gate.
        // None preserves the actually observed pre-preinit bootstrap access;
        // header0 maps alone do not establish silicon reset-latch routing.
        self.permissions.is_none_or(|p|p&3==3)&&
            (self.force&4!=0||self.clock&4!=0&&self.permissions.is_none_or(|p|p&4!=0))
    }
    fn apply_domains(&mut self){
        let bit=1u32<<self.captured_code;
        self.permissions=Some(u8::from((self.modem_map>>28)&bit!=0)|
            u8::from((self.lp_map>>28)&bit!=0)<<1|u8::from((self.lp_map>>24)&bit!=0)<<2);
    }
    fn control(word:&mut u32,value:u32,mask:u32)->bool{if (*word^value)&!mask!=0{return false;}*word=value;true}
    fn supported_icg_map(value:u32,initial:u32,mask:u32)->bool{
        if value&!mask!=0{return false;}
        // Fixed C5 initial_gating_mode: 4=ACTIVE, 6=ACTIVE|MODEM.
        // Retain0/4 for every named field; admit6 only where the original
        // table uses it. No SLEEP, MODEM-only or code3 permission is modeled.
        for shift in(0..32).step_by(4){
            let init=(initial>>shift)&15;let bitmap=(value>>shift)&15;
            if init!=0&&bitmap!=0&&bitmap!=4&&bitmap!=init{return false;}
        }true
    }
    fn controller_reset(&mut self){self.ctrl=[0;2];self.conf1=0x2d;self.conf2=4;}
    /// TRM9.1 domains, supplied by the host reset dispatcher. CPU-only is all
    /// false. Core resets HP; System also LP; Chip also analog. The return
    /// value applies the *previous* system-bus bypass policy to this reset.
    /// The modeled bus is stateless; no APM/TEE retention is implemented.
    pub fn reset_domains(&mut self,hp:bool,lp:bool,analog:bool)->bool{
        let reset_bus=analog||hp&&self.bypass&2==0;
        self.analog.reset(hp,lp,analog);
        self.ocode.reset(hp,lp,analog);
        if hp||analog{self.bypass=2;self.clock=0;self.force=0;self.select=0x0020_0002;self.controller_reset();self.modem_map=0;self.lp_map=0;self.permissions=None;}
        if lp||analog{self.rf=0x0800_0000;self.bod=0;self.glitch=0;self.fib=u32::MAX;self.active_modem=0;self.captured_code=0;self.permissions=None;}
        // Internal analog byte retention is a declared functional profile.
        // A PMU interface reset/power gate does not reseed its configuration.
        if analog{self.vdet=0x0f;self.sar.reset_analog();self.bod_byte=0;self.bod_written=false;self.reads=0;self.writes=0;self.code_updates=0;self.switch_updates=0;}reset_bus
    }
    pub fn read(&mut self,address:u32)->Option<u32>{
        Some(match address{
            PCR_BYPASS=>self.bypass,MODEM_CLK=>self.clock,MODEM_FORCE=>self.force,MODEM_SELECT=>self.select,
            RF_PWC=>self.rf,BOD1=>self.bod,GLITCH=>self.glitch,FIB=>self.fib,
            PMU_ACTIVE_MODEM=>self.active_modem,MODEM_MAP=>self.modem_map,LPCON_MAP=>self.lp_map,
            PMU_MODEM_UPDATE|PMU_SWITCH_UPDATE=>0,
            0x600b_00ac if self.analog.enabled=>self.analog.read_fosc(),0x600b_00cc if self.analog.enabled=>0,
            _=>{if !self.clocked(){return None;}match address.checked_sub(MASTER)?{0=>self.ctrl[0],4=>self.ctrl[1],0x18 if self.analog.enabled=>self.analog.read_conf(),0x1c=>self.conf1,0x20=>self.conf2,_=>return None,}}
        })
    }
    fn transfer(&mut self,n:usize,value:u32)->bool{
        // Driver source decodes slave7:0, address15:8, data23:16, WR24;
        // BUSY25 is RO. Every accepted command actually accesses the target.
        if value&0xfc00_0000!=0{return false;}
        let command=value&0x01ff_ffff;
        let block=command as u8;let reg=(command>>8)as u8;let write=command&(1<<24)!=0;let data=(command>>16) as u8;
        let result=match block{
            0x69=>{
                if self.rf&0x0c00_0000!=0x0c00_0000||self.conf1!=SAR_ROUTE||n!=usize::from(self.conf2&(1<<11)==0){return false;}
                if reg==10{
                    // Preserve the original four named virtual VDET bits.
                    if write&&data&0xf0!=0{return false;}
                    if write{self.vdet=data;}self.vdet
                }else{let Some(value)=self.sar.transfer(reg,write.then_some(data))else{return false;};value}
            }
            0x6d|0x66 if self.analog.enabled=>{
                let(route,bit)=if block==0x6d{(DIG_ROUTE,12)}else{(PLL_ROUTE,9)};
                if self.conf1!=route||n!=usize::from(self.conf2&(1<<bit)==0)||block==0x66&&!self.analog.powered(){return false;}
                let Some(old)=self.analog.byte(block,reg)else{return false;};
                if write{if !self.analog.write_byte(block,reg,data){return false;}data}else{old}
            }
            0x61 if self.ocode.enabled||self.bod_threshold_profile=>{
                if self.conf1!=ULP_ROUTE||n!=usize::from(self.conf2&(1<<10)==0){return false;}
                if reg==5&&self.bod_threshold_profile{
                    // Fixed regi2c_brownout.h: only THRESHOLD[2:0] is named.
                    // Keep virtual upper bits0 and reject unmodeled codes.
                    if write{if data!=7{return false;}self.bod_byte=data;self.bod_written=true;}self.bod_byte
                }else{
                    let Some(value)=self.ocode.transfer(reg,write.then_some(data),self.analog.ocode_quiet())else{return false;};value
                }
            }
            _=>return false,
        };
        // Synchronous transfer result is exposed only after target access.
        self.ctrl[n]=command|(1<<25);
        if write{self.writes=self.writes.saturating_add(1);}else{self.reads=self.reads.saturating_add(1);}
        self.ctrl[n]=(command&!0x00ff_0000)|((result as u32)<<16);true
    }
    pub fn write(&mut self,address:u32,value:u32)->bool{
        let ok=self.write_inner(address,value);if ok{self.reconcile_pll();self.ocode.reconcile(self.analog.ocode_quiet());}ok
    }
    fn write_inner(&mut self,address:u32,value:u32)->bool{
        match address{
            // APM/TEE bypass and all other control fields remain unsupported.
            PCR_BYPASS=>Self::control(&mut self.bypass,value,2),
            // Fixed C5 CLK_CONF: WIFIPWR_EN0/COEX_EN1 are configuration
            // requests, independent of I2C_MST_EN2 and source selection in
            // SYSCON+04 bit12. No COEX/RF execution or readiness is derived.
            // LP_TIMER_EN3 and COEX/FEMEM/timer force engines stay unsupported.
            MODEM_CLK=>Self::control(&mut self.clock,value,7),MODEM_FORCE=>Self::control(&mut self.force,value,4),
            MODEM_SELECT=>Self::control(&mut self.select,value,1<<12),RF_PWC=>Self::control(&mut self.rf,value,0x0c00_0000),
            BOD1=>Self::control(&mut self.bod,value,1<<31),GLITCH=>Self::control(&mut self.glitch,value,0x7800_0000),
            FIB=>Self::control(&mut self.fib,value,0x3e),
            0x600b_00ac if self.analog.enabled=>self.analog.write_fosc(value),0x600b_00cc if self.analog.enabled=>self.analog.write_power(value),
            PMU_ACTIVE_MODEM=>{if !matches!(value,0|0x8000_0000){return false;}self.active_modem=value;true}
            MODEM_MAP=>{if !Self::supported_icg_map(value,MODEM_MAP_INIT,0xffff_ff00){return false;}self.modem_map=value;true}
            LPCON_MAP=>{if !Self::supported_icg_map(value,LPCON_MAP_INIT,0xffff_0000){return false;}self.lp_map=value;true}
            // Headers mark WT0; the vendor boot routine pairs these commands.
            // Capture-then-apply is a declared functional latch abstraction,
            // not independently documented silicon micro-sequencing.
            PMU_MODEM_UPDATE=>{
                if value&!0x8000_0000!=0{return false;}
                if value!=0{self.captured_code=(self.active_modem>>30)as u8;self.code_updates=self.code_updates.saturating_add(1);}true
            }
            PMU_SWITCH_UPDATE=>{
                if value&!0x1000_0000!=0{return false;}
                if value!=0{self.apply_domains();self.switch_updates=self.switch_updates.saturating_add(1);}true
            }
            _=>{
                if !self.clocked(){return false;}match address.checked_sub(MASTER){
                    Some(0)=>self.transfer(0,value),Some(4)=>self.transfer(1,value),
                    // Only driver-confirmed routes of enabled target models.
                    Some(0x18)if self.analog.enabled=>self.analog.write_conf(value),
                    Some(0x1c)=>{if !matches!(value,0x2d|SAR_ROUTE)&&!(self.analog.enabled&&matches!(value,DIG_ROUTE|PLL_ROUTE))&&!((self.ocode.enabled||self.bod_threshold_profile)&&value==ULP_ROUTE){return false;}self.conf1=value;true}
                    Some(0x20)=>{let mask=if self.analog.enabled{(1<<9)|(1<<11)|(1<<12)}else{1<<11};Self::control(&mut self.conf2,value,mask|if self.ocode.enabled||self.bod_threshold_profile{1<<10}else{0})},_=>false,
                }
            }
        }
    }
    pub fn report(&self)->String{
        let bod=self.fib&2!=0||self.bod&(1<<31)!=0;
        let glitch=(self.fib>>2)&15|(self.glitch>>27)&15;
        let report=format!("boot_analog=virtual_usb3v3 supply=stable vdet={:#04x} bod1={} glitch={:#x} regi2c_reads={} regi2c_writes={} select_160m={} {} modem_icg={} code={} updates={}/{}",
            self.vdet,u8::from(bod),glitch,self.reads,self.writes,u8::from(self.select&(1<<12)!=0),
            if self.analog.enabled{self.analog.report()}else{"pll_calibration=unsupported".into()},
            self.permissions.map_or_else(||"bootstrap_unmanaged".into(),|p|format!("permissions{p:#x}")),self.captured_code,self.code_updates,self.switch_updates);
        let report=if self.sar.enabled{format!("{report} {} reserved_path_configured={}",self.sar.report(),u8::from(self.sar_reserved_path_ready()))}else{report};
        let report=if self.clock&1!=0{format!("{report} wifipwr_lp_gate_requested=1 RF_execution=unsupported")}else{report};
        let report=if self.coex_clock_requested(){format!("{report} coex_lp_gate_requested=1 COEX_execution=unsupported")}else{report};
        let report=if self.bod_threshold_profile{format!("{report} BOD_threshold_byte={:#04x} BOD_threshold_written={}",self.bod_byte,u8::from(self.bod_written))}else{report};
        if self.virtual_ocode_enabled(){format!("{report} {} power_latches={:#x} ana_route={:#010x} ana_selector={:#x} ctrl0={:#010x} ctrl1={:#010x}",
            self.ocode.report(),self.analog.power_latches(),self.conf1,self.conf2,self.ctrl[0],self.ctrl[1])}else{report}
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    fn put(a:&mut C5BootAnalog,address:u32,value:u32){assert!(a.write(address,value),"write {address:#x}={value:#x}");}
    fn rmw(a:&mut C5BootAnalog,address:u32,clear:u32,set:u32){let v=a.read(address).unwrap();put(a,address,(v&!clear)|set);}
    fn hardware_init(a:&mut C5BootAnalog){rmw(a,PCR_BYPASS,2,0);rmw(a,MODEM_CLK,0,4);rmw(a,MODEM_FORCE,0,4);rmw(a,MODEM_SELECT,0,1<<12);rmw(a,FIB,2,0);rmw(a,BOD1,0,1<<31);}
    fn sar_enable(a:&mut C5BootAnalog){rmw(a,RF_PWC,0,1<<26);rmw(a,RF_PWC,0,1<<27);put(a,MASTER+0x1c,SAR_ROUTE);}
    fn clear_vdet(a:&mut C5BootAnalog,bit:u32)->u8{
        let n=if a.read(MASTER+0x20).unwrap()&(1<<11)!=0{0}else{1};let address=MASTER+n*4;
        assert_eq!(a.read(address).unwrap()&(1<<25),0);put(a,address,0x0a69);
        let value=a.read(address).unwrap();assert_eq!(value&(1<<25),0);let data=(value>>16)&255;
        put(a,address,0x0100_0a69|((data&!(1<<bit))<<16));assert_eq!(a.read(address).unwrap()&(1<<25),0);a.vdet
    }
    #[test]
    fn unchanged_bootloader_sequence_performs_three_actual_masked_transfers(){
        let mut a=C5BootAnalog::virtual_usb3v3();assert_eq!(a.read(PCR_BYPASS),Some(2));assert_eq!(a.read(RF_PWC),Some(0x0800_0000));
        assert_eq!(a.read(MODEM_SELECT),Some(0x0020_0002));assert_eq!(a.read(FIB),Some(u32::MAX));assert_eq!(a.vdet,15);
        hardware_init(&mut a);assert_eq!(a.read(MODEM_SELECT),Some(0x0020_1002));assert_eq!(a.vdet,15);sar_enable(&mut a);
        assert_eq!(clear_vdet(&mut a,0),14);assert_eq!(clear_vdet(&mut a,2),10);assert_eq!(clear_vdet(&mut a,3),2);
        rmw(&mut a,FIB,0x3c,0);rmw(&mut a,GLITCH,0x7800_0000,0x7800_0000);
        assert_eq!(a.read(FIB),Some(0xffff_ffc1));assert_eq!(a.read(GLITCH),Some(0x7800_0000));assert_eq!((a.reads,a.writes),(3,3));
        assert_eq!(a.read(MASTER),Some(0));assert_eq!(a.read(MASTER+4),Some(0x0102_0a69));
        for _ in 0..8{assert_eq!(a.read(MASTER+4),Some(0x0102_0a69));}assert_eq!((a.reads,a.writes),(3,3));
        assert!(a.report().contains("virtual_usb3v3 supply=stable vdet=0x02 bod1=1 glitch=0xf regi2c_reads=3 regi2c_writes=3"));
    }
    #[test]
    fn real_clock_power_reset_and_sar_route_gate_transfers(){
        let mut a=C5BootAnalog::virtual_usb3v3();assert_eq!(a.read(MASTER+0x20),None);assert!(!a.write(MASTER+4,0x0a69));
        put(&mut a,MODEM_CLK,4);assert_eq!(a.read(MASTER+0x20),Some(4));assert!(!a.write(MASTER+4,0x0a69));
        put(&mut a,MASTER+0x1c,SAR_ROUTE);assert!(!a.write(MASTER+4,0x0a69));put(&mut a,RF_PWC,0x0400_0000);assert!(!a.write(MASTER+4,0x0a69));
        put(&mut a,RF_PWC,0x0c00_0000);put(&mut a,MASTER+4,0x0a69);assert_eq!(a.read(MASTER+4),Some(0x000f_0a69));
        put(&mut a,MODEM_CLK,0);let before=a.clone();assert!(!a.write(MASTER+4,0x0a69));assert_eq!(a,before);
        put(&mut a,MODEM_FORCE,4);put(&mut a,MASTER+4,0x0a69);put(&mut a,MASTER+0x20,0x804);
        let before=a.clone();assert!(!a.write(MASTER+4,0x0a69));assert_eq!(a,before);put(&mut a,MASTER,0x0a69);
        assert_eq!(a.read(MASTER),Some(0x000f_0a69));assert_eq!((a.reads,a.writes),(3,0));
        put(&mut a,RF_PWC,0x0800_0000);assert!(!a.write(MASTER,0x0a69));assert_eq!(a.vdet,15);
    }
    #[test]
    fn unknown_engines_bits_targets_and_security_controls_reject_transactionally(){
        let mut a=C5BootAnalog::virtual_usb3v3();hardware_init(&mut a);sar_enable(&mut a);
        let bad=[(PCR_BYPASS,1),(PCR_BYPASS,4),(MODEM_CLK,8),(MODEM_FORCE,5),(MODEM_SELECT,0x1000),
            (RF_PWC,0x1c00_0000),(BOD1,1),(GLITCH,0x8000_0000),(FIB,0xffff_fffc),(MASTER+0x1c,0x00ff_ff7f),
            (MASTER+0x20,0x204),(MASTER+4,0x0a66),(MASTER+4,0x0b69),(MASTER+4,0x0110_0a69),
            (MASTER+4,0x0400_0a69),(MASTER+0x18,8),(MASTER+8,0),(MASTER+0x10,1),(MASTER+1,0),(0,0),(u32::MAX,0)];
        for(address,value)in bad{let before=a.clone();assert!(!a.write(address,value),"accepted {address:#x}={value:#x}");assert_eq!(a,before);}
        for off in[8,0xc,0x10,0x14,0x18,0x24,0x28,0x2c,0x30,0x34,0x38]{assert_eq!(a.read(MASTER+off),None);}
        // RO BUSY cannot be written into existence. A supported read still
        // returns its actual byte even if the supplied data bits are stale.
        put(&mut a,MASTER+4,0x02aa_0a69);assert_eq!(a.read(MASTER+4),Some(0x000f_0a69));assert_eq!((a.reads,a.writes),(1,0));
    }
    #[test]
    fn detector_permission_and_virtual_stable_supply_do_not_invent_events(){
        let mut a=C5BootAnalog::virtual_usb3v3();assert!(a.report().contains("bod1=1 glitch=0xf"));
        rmw(&mut a,FIB,0x3e,0);assert!(a.report().contains("bod1=0 glitch=0x0"));
        put(&mut a,BOD1,1<<31);put(&mut a,GLITCH,5<<27);assert!(a.report().contains("bod1=1 glitch=0x5"));
        let before=a.clone();for _ in 0..16{a.report();a.read(BOD1);a.read(GLITCH);}assert_eq!(a,before);
        assert_eq!(a.read(0x600b_2c10),None);assert!(!a.write(0x600b_2c18,1<<31));
    }
    #[test]
    fn reset_domains_preserve_analog_and_lp_until_their_actual_domain_resets(){
        let mut a=C5BootAnalog::virtual_usb3v3();hardware_init(&mut a);sar_enable(&mut a);clear_vdet(&mut a,0);rmw(&mut a,FIB,0x3c,0);put(&mut a,GLITCH,3<<27);
        let before=a.clone();assert!(!a.reset_domains(false,false,false));assert_eq!(a,before);
        assert!(a.reset_domains(true,false,false));assert_eq!(a.read(PCR_BYPASS),Some(2));assert_eq!(a.read(MODEM_CLK),Some(0));
        assert_eq!(a.read(RF_PWC),Some(0x0c00_0000));assert_eq!(a.read(GLITCH),Some(3<<27));assert_eq!(a.vdet,14);assert_eq!((a.reads,a.writes),(1,1));
        assert!(!a.reset_domains(true,true,false));assert_eq!(a.read(RF_PWC),Some(0x0800_0000));assert_eq!(a.read(BOD1),Some(0));assert_eq!(a.read(FIB),Some(u32::MAX));assert_eq!(a.vdet,14);
        hardware_init(&mut a);sar_enable(&mut a);put(&mut a,MASTER+0x20,0x804);assert!(a.reset_domains(false,false,true));assert_eq!(a,C5BootAnalog::virtual_usb3v3());
        put(&mut a,MODEM_CLK,4);assert_eq!(a.read(MASTER),Some(0));assert_eq!(a.read(MASTER+4),Some(0));assert_eq!(a.read(MASTER+0x1c),Some(0x2d));assert_eq!(a.read(MASTER+0x20),Some(4));
    }
    fn domains(a:&mut C5BootAnalog,modem:u32,lp:u32){
        put(a,PMU_ACTIVE_MODEM,0x8000_0000);put(a,MODEM_MAP,modem);put(a,LPCON_MAP,lp);
        put(a,PMU_MODEM_UPDATE,0x8000_0000);put(a,PMU_SWITCH_UPDATE,0x1000_0000);
    }
    #[test]
    fn original_clock_preinit_writes_real_permissions_and_wt_commands_read_zero(){
        let mut a=C5BootAnalog::virtual_usb3v3();hardware_init(&mut a);sar_enable(&mut a);
        assert_eq!(clear_vdet(&mut a,0),14);assert_eq!(clear_vdet(&mut a,2),10);assert_eq!(clear_vdet(&mut a,3),2);
        for address in[PMU_ACTIVE_MODEM,MODEM_MAP,LPCON_MAP,PMU_MODEM_UPDATE,PMU_SWITCH_UPDATE]{assert_eq!(a.read(address),Some(0));}
        rmw(&mut a,PMU_ACTIVE_MODEM,0xc000_0000,0x8000_0000);rmw(&mut a,MODEM_MAP,0xf000_0000,0x4000_0000);
        rmw(&mut a,LPCON_MAP,0x0f00_0000,0x0400_0000);rmw(&mut a,LPCON_MAP,0xf000_0000,0x4000_0000);
        assert_eq!(a.permissions,None);rmw(&mut a,PMU_MODEM_UPDATE,0,0x8000_0000);assert_eq!(a.captured_code,2);assert_eq!(a.permissions,None);
        rmw(&mut a,PMU_SWITCH_UPDATE,0,0x1000_0000);assert_eq!(a.permissions,Some(7));assert_eq!((a.code_updates,a.switch_updates),(1,1));
        for address in[PMU_MODEM_UPDATE,PMU_SWITCH_UPDATE]{assert_eq!(a.read(address),Some(0));put(&mut a,address,0);}
        assert_eq!((a.code_updates,a.switch_updates),(1,1));put(&mut a,MASTER+4,0x0a69);assert_eq!(a.read(MASTER+4),Some(0x0002_0a69));
        assert_eq!((a.reads,a.writes),(4,3));assert!(a.report().contains("modem_icg=permissions0x7 code=2 updates=1/1"));
    }
    #[test]
    fn applied_modem_apb_and_functional_gates_control_only_the_actual_master(){
        let mut a=C5BootAnalog::virtual_usb3v3();hardware_init(&mut a);sar_enable(&mut a);domains(&mut a,0x4000_0000,0x4400_0000);
        put(&mut a,MODEM_FORCE,0);put(&mut a,MASTER+4,0x0a69);
        put(&mut a,LPCON_MAP,0x4000_0000);assert!(a.read(MASTER).is_some(),"shadow changes need the declared WT apply");
        put(&mut a,PMU_SWITCH_UPDATE,0x1000_0000);assert_eq!(a.permissions,Some(3));assert_eq!(a.read(MASTER),None);assert!(!a.write(MASTER+4,0x0a69));
        put(&mut a,MODEM_FORCE,4);put(&mut a,MASTER+4,0x0a69);
        for(modem,lp)in[(0,0x4400_0000),(0x4000_0000,0x0400_0000),(0,0)]{
            domains(&mut a,modem,lp);let before=a.clone();assert_eq!(a.read(MASTER),None);assert!(!a.write(MASTER+4,0x0a69));assert_eq!(a,before);
            assert_eq!(a.read(BOD1),Some(1<<31));assert_eq!(a.read(RF_PWC),Some(0x0c00_0000));
        }
        domains(&mut a,0x4000_0000,0x4400_0000);put(&mut a,MODEM_FORCE,0);put(&mut a,MODEM_CLK,0);assert_eq!(a.read(MASTER),None);
        put(&mut a,MODEM_CLK,4);put(&mut a,MASTER+4,0x0a69);
        put(&mut a,PMU_ACTIVE_MODEM,0);put(&mut a,PMU_SWITCH_UPDATE,0x1000_0000);assert!(a.read(MASTER).is_some(),"old captured code remains effective");
        put(&mut a,PMU_MODEM_UPDATE,0x8000_0000);assert!(a.read(MASTER).is_some());put(&mut a,PMU_SWITCH_UPDATE,0x1000_0000);assert_eq!(a.permissions,Some(0));assert_eq!(a.read(MASTER),None);
    }
    #[test]
    fn unsupported_domain_modes_are_transactional_and_reset_domains_are_explicit(){
        let mut a=C5BootAnalog::virtual_usb3v3();hardware_init(&mut a);sar_enable(&mut a);domains(&mut a,0x4000_0000,0x4400_0000);
        for(address,value)in[(PMU_ACTIVE_MODEM,0x4000_0000),(PMU_ACTIVE_MODEM,0xc000_0000),(PMU_ACTIVE_MODEM,1),
            (MODEM_MAP,0x2000_0000),(MODEM_MAP,0x4200_0000),(LPCON_MAP,0x8800_0000),(LPCON_MAP,0x0020_0000),
            (PMU_MODEM_UPDATE,1),(PMU_MODEM_UPDATE,0x9000_0000),(PMU_SWITCH_UPDATE,0x2000_0000),(PMU_SWITCH_UPDATE,0x9000_0000)]{
            let before=a.clone();assert!(!a.write(address,value));assert_eq!(a,before);
        }
        let before=a.clone();a.reset_domains(false,false,false);assert_eq!(a,before);
        a.reset_domains(true,false,false);assert_eq!(a.read(PMU_ACTIVE_MODEM),Some(0x8000_0000));assert_eq!(a.captured_code,2);
        assert_eq!(a.read(MODEM_MAP),Some(0));assert_eq!(a.read(LPCON_MAP),Some(0));assert_eq!(a.permissions,None);
        hardware_init(&mut a);sar_enable(&mut a);put(&mut a,MASTER+4,0x0a69);
        a.reset_domains(true,true,false);assert_eq!(a.read(PMU_ACTIVE_MODEM),Some(0));assert_eq!(a.captured_code,0);assert_eq!(a.permissions,None);
        a.reset_domains(false,false,true);assert_eq!(a,C5BootAnalog::virtual_usb3v3());
    }
    #[test]
    fn modem_apb_observation_uses_only_applied_permission_without_granting_i2c(){
        let mut a=C5BootAnalog::virtual_usb3v3();assert!(!a.modem_apb_accessible());put(&mut a,MODEM_CLK,4);
        put(&mut a,PMU_ACTIVE_MODEM,0x8000_0000);put(&mut a,MODEM_MAP,0x4000_0000);assert!(!a.modem_apb_accessible());
        put(&mut a,PMU_MODEM_UPDATE,0x8000_0000);assert!(!a.modem_apb_accessible());put(&mut a,PMU_SWITCH_UPDATE,0x1000_0000);
        assert!(a.modem_apb_accessible());assert!(!a.lp_apb_accessible());assert_eq!(a.read(MASTER),None);
        let before=a.clone();for _ in 0..16{assert!(a.modem_apb_accessible());}assert_eq!(a,before);
        put(&mut a,MODEM_MAP,0);assert!(a.modem_apb_accessible());put(&mut a,PMU_SWITCH_UPDATE,0x1000_0000);assert!(!a.modem_apb_accessible());
    }
    #[test]
    fn actual_coex_rmw_preserves_wifipwr_and_i2c_requests_without_an_engine(){
        let mut a=C5BootAnalog::virtual_usb3v3();assert!(!a.coex_clock_requested());let legacy=a.report();
        put(&mut a,MODEM_CLK,1);let old=a.read(MODEM_CLK).unwrap();assert_eq!(old,1);
        put(&mut a,MODEM_CLK,(old&!2)|(1<<1));assert_eq!(a.read(MODEM_CLK),Some(3));assert!(a.coex_clock_requested());
        assert!(a.report().contains("coex_lp_gate_requested=1 COEX_execution=unsupported"));
        for base in[0,1,4,5]{put(&mut a,MODEM_CLK,base);let old=a.read(MODEM_CLK).unwrap();put(&mut a,MODEM_CLK,old|2);
            assert_eq!(a.read(MODEM_CLK),Some(base|2));assert!(a.coex_clock_requested());let old=a.read(MODEM_CLK).unwrap();put(&mut a,MODEM_CLK,old&!2);
            assert_eq!(a.read(MODEM_CLK),Some(base));assert!(!a.coex_clock_requested());
        }
        put(&mut a,MODEM_CLK,0);assert_eq!(a.report(),legacy);assert!(!a.pll_ready());assert_eq!(a.deadline(),None);
    }
    #[test]
    fn coex_value_three_never_enables_i2c_or_changes_the_160m_selector(){
        let mut a=C5BootAnalog::virtual_usb3v3();hardware_init(&mut a);sar_enable(&mut a);domains(&mut a,0x4000_0000,0x4400_0000);
        let source=a.read(MODEM_SELECT);put(&mut a,MODEM_FORCE,0);put(&mut a,MODEM_CLK,1);assert_eq!(a.read(MASTER),None);
        put(&mut a,MODEM_CLK,3);assert_eq!(a.read(MODEM_SELECT),source);assert_eq!(a.permissions,Some(7));assert_eq!(a.read(MASTER),None);
        let before=a.clone();assert!(!a.write(MASTER+4,0x0a69));assert_eq!(a,before);
        put(&mut a,MODEM_CLK,7);put(&mut a,MASTER+4,0x0a69);assert_eq!(a.read(MASTER+4),Some(0x000f_0a69));
        put(&mut a,MODEM_CLK,3);assert_eq!(a.read(MASTER),None);put(&mut a,MODEM_FORCE,4);put(&mut a,MASTER+4,0x0a69);
        assert_eq!(a.read(MASTER+4),Some(0x000f_0a69));assert_eq!(a.read(MODEM_SELECT),source);
        domains(&mut a,0,0x4400_0000);assert_eq!(a.read(MASTER),None);assert!(a.coex_clock_requested());
    }
    #[test]
    fn pure_coex_toggle_preserves_settled_analog_inputs_deadline_and_all_targets(){
        let mut a=analog_start();bb_config(&mut a);a.pump(100);let before=a.clone();let at=a.deadline();
        assert!(a.write_at(MODEM_CLK,6,100));assert_eq!(a.deadline(),at);assert!(!a.pll_ready());
        assert_eq!((a.analog.clone(),a.sar.clone(),a.ocode.clone(),a.vdet,a.bod,a.fib,a.rf,a.select,a.ctrl,a.reads,a.writes),
            (before.analog.clone(),before.sar.clone(),before.ocode.clone(),before.vdet,before.bod,before.fib,before.rf,before.select,before.ctrl,before.reads,before.writes));
        assert!(a.write_at(MODEM_CLK,4,100));assert_eq!(a,before);
        assert_eq!(a.bod_threshold(),None);assert_eq!(a.deadline(),at);
    }
    #[test]
    fn combined_write_settles_old_i2c_clock_before_withdrawing_it(){
        let mut early=analog_start();bb_config(&mut early);put(&mut early,MODEM_FORCE,0);let at=early.deadline().unwrap();
        assert!(early.write_at(MODEM_CLK,3,at-1));assert_eq!(early.deadline(),None);early.pump(at);
        assert_eq!(early.analog.read_conf()&(1<<24),0);assert!(!early.pll_ready());assert_eq!(early.read(MASTER),None);
        let mut due=analog_start();bb_config(&mut due);put(&mut due,MODEM_FORCE,0);let at=due.deadline().unwrap();
        assert!(due.write_at(MODEM_CLK,3,at));assert_ne!(due.analog.read_conf()&(1<<24),0,"old clock actually reached its deadline");
        assert!(!due.pll_ready(),"real stop sequence is still required");assert_eq!(due.read(MASTER),None);
        put(&mut due,MODEM_FORCE,4);stop_calibration(&mut due);assert!(due.pll_ready());
    }
    #[test]
    fn coex_configuration_uses_existing_hp_reset_domain_and_read_only_observation(){
        let mut a=C5BootAnalog::virtual_usb3v3();hardware_init(&mut a);put(&mut a,MODEM_CLK,3);let before=a.clone();
        for _ in 0..8{assert!(a.coex_clock_requested());assert_eq!(a.read(MODEM_CLK),Some(3));a.report();}assert_eq!(a,before);
        a.reset_domains(false,false,false);assert_eq!(a,before);a.reset_domains(false,true,false);assert_eq!(a.read(MODEM_CLK),Some(3));assert!(a.coex_clock_requested());
        a.reset_domains(true,false,false);assert_eq!(a.read(MODEM_CLK),Some(0));assert!(!a.coex_clock_requested());assert!(!a.report().contains("coex_lp_gate_requested"));
        put(&mut a,MODEM_CLK,3);a.reset_domains(false,false,true);assert_eq!(a,C5BootAnalog::virtual_usb3v3());
    }
    #[test]
    fn coex_request_does_not_admit_timer_force_phy_or_unknown_target_modes(){
        let mut a=C5BootAnalog::virtual_usb3v3();hardware_init(&mut a);sar_enable(&mut a);put(&mut a,MODEM_CLK,3);
        for(address,value)in[(MODEM_CLK,8),(MODEM_CLK,0x10),(MODEM_CLK,u32::MAX),(MODEM_FORCE,2),(MODEM_FORCE,6),
            (MODEM_SELECT,0x0020_1003),(MASTER+4,0x0461),(MASTER+4,0x0b69),(MASTER+4,0x0a66),
            (0x600a_f024,1),(0x600a_9c08,1)]{
            let before=a.clone();assert!(!a.write(address,value));assert_eq!(a,before);
        }
        assert_eq!(a.read(0x600b_2c00),None);assert!(!a.pll_ready());assert!(!a.sar_reserved_path_ready());assert_eq!(a.bod_threshold(),None);
    }
    fn byte(a:&mut C5BootAnalog,block:u8,reg:u8,data:Option<u8>)->u8{
        let(route,bit)=if block==0x6d{(DIG_ROUTE,12)}else{(PLL_ROUTE,9)};put(a,MASTER+0x1c,route);
        let n=usize::from(a.read(MASTER+0x20).unwrap()&(1<<bit)==0);let ctrl=MASTER+n as u32*4;
        let command=(reg as u32)<<8|block as u32|data.map_or(0,|v|(1<<24)|(v as u32)<<16);
        put(a,ctrl,command);let result=a.read(ctrl).unwrap();assert_eq!(result&(1<<25),0);(result>>16)as u8
    }
    fn byte_mask(a:&mut C5BootAnalog,b:u8,r:u8,mask:u8,value:u8){let old=byte(a,b,r,None);byte(a,b,r,Some((old&!mask)|value));}
    fn analog_start()->C5BootAnalog{
        let mut a=C5BootAnalog::virtual_usb3v3();a.enable_virtual_pll(0);hardware_init(&mut a);sar_enable(&mut a);
        clear_vdet(&mut a,0);clear_vdet(&mut a,2);clear_vdet(&mut a,3);domains(&mut a,0x4000_0000,0x4400_0000);a
    }
    fn dig_config(a:&mut C5BootAnalog){
        byte_mask(a,0x6d,14,255,128);byte_mask(a,0x6d,5,0x80,0x80);byte_mask(a,0x6d,7,0x80,0x80);
        byte_mask(a,0x6d,13,4,0);byte_mask(a,0x6d,13,8,0);
    }
    fn start_calibration(a:&mut C5BootAnalog){rmw(a,MASTER+0x18,4,0);rmw(a,MASTER+0x18,0,8);}
    fn bb_config(a:&mut C5BootAnalog){
        put(a,0x600b_00cc,0x7000_0000);put(a,0x600b_00cc,0x0200_0000);start_calibration(a);
        byte(a,0x66,2,Some(0x51));byte(a,0x66,3,Some(10));byte_mask(a,0x66,5,7,1);byte_mask(a,0x66,5,0x70,0x10);
        byte_mask(a,0x66,6,0xc0,0x40);byte_mask(a,0x66,6,0x30,0x30);byte_mask(a,0x66,9,3,3);
    }
    fn stop_calibration(a:&mut C5BootAnalog){rmw(a,MASTER+0x18,8,0);rmw(a,MASTER+0x18,0,4);}
    #[test]
    fn actual_dig_and_pll_transfers_then_elapsed_calibration_and_stop_produce_readiness(){
        let mut a=analog_start();assert_eq!(a.slow_dcap(),None);assert!(a.rc_fast_powered());dig_config(&mut a);assert_eq!(a.slow_dcap(),Some(128));
        assert_eq!(a.analog.byte(0x6d,5),Some(0x80));assert_eq!(a.analog.byte(0x6d,7),Some(0x80));assert_eq!(a.analog.byte(0x6d,13),Some(0));
        a.pump(500);bb_config(&mut a);assert_eq!((a.reads,a.writes),(13,15));assert_eq!(a.deadline(),Some(24_500));assert!(!a.pll_ready());
        for _ in 0..20{assert_eq!(a.read(MASTER+0x18).unwrap()&(1<<24),0);}assert_eq!(a.deadline(),Some(24_500));
        assert_eq!(a.read_at(MASTER+0x18,24_499).unwrap()&(1<<24),0);assert_eq!(a.read_at(MASTER+0x18,24_500).unwrap()&(1<<24),1<<24);
        assert_eq!(a.deadline(),None);assert!(!a.pll_ready());stop_calibration(&mut a);assert!(a.pll_ready());assert_eq!(a.read(0x600b_00cc),Some(0));
        put(&mut a,MODEM_CLK,0);put(&mut a,MODEM_FORCE,0);assert!(a.pll_ready(),"stopped oscillator does not need the I2C controller clock");
        assert_eq!(a.read(MASTER+0x18),None);assert!(a.report().contains("pll_profile=virtual100us ready=1 cal_done=1 attempts=1 locks=1"));
    }
    #[test]
    fn wifi_lp_gate_request_does_not_grant_master_or_apb_access_or_withdraw_a_locked_pll(){
        let mut a=C5BootAnalog::virtual_usb3v3();put(&mut a,MODEM_CLK,1);
        assert_eq!(a.read(MODEM_CLK),Some(1));assert_eq!(a.read(MASTER),None);assert!(!a.lp_apb_accessible());assert!(!a.pll_ready());
        assert!(a.report().contains("wifipwr_lp_gate_requested=1 RF_execution=unsupported"));
        let mut a=analog_start();bb_config(&mut a);a.pump(24_000);stop_calibration(&mut a);assert!(a.pll_ready());
        put(&mut a,MODEM_CLK,5);put(&mut a,MODEM_FORCE,0);assert!(a.read(MASTER).is_some());
        let transfers=(a.reads,a.writes);put(&mut a,MODEM_CLK,1);assert_eq!(a.read(MASTER),None);assert!(a.pll_ready());
        put(&mut a,MODEM_CLK,3);assert_eq!(a.read(MASTER),None);assert!(a.pll_ready());assert!(a.coex_clock_requested());
        assert_eq!((a.reads,a.writes),transfers);let before=a.clone();assert!(!a.write(MODEM_CLK,8));assert_eq!(a,before);
    }
    #[test]
    fn incomplete_or_changed_configuration_gates_and_powerdown_cannot_complete(){
        let mut a=analog_start();put(&mut a,0x600b_00cc,0x7200_0000);start_calibration(&mut a);a.pump(100_000);assert_eq!(a.deadline(),None);assert!(!a.pll_ready());
        bb_config(&mut a);let original=a.deadline().unwrap();byte(&mut a,0x66,3,Some(9));assert_eq!(a.deadline(),None);a.pump(original+100_000);assert_eq!(a.read(MASTER+0x18).unwrap()&(1<<24),0);
        byte(&mut a,0x66,3,Some(10));assert_eq!(a.deadline(),Some(original+124_000));
        put(&mut a,MODEM_CLK,0);put(&mut a,MODEM_FORCE,0);assert_eq!(a.deadline(),None);a.pump(original+200_000);assert!(!a.pll_ready());
        put(&mut a,MODEM_CLK,4);let at=a.deadline().unwrap();a.pump(at);stop_calibration(&mut a);assert!(a.pll_ready());
        put(&mut a,0x600b_00cc,1);assert!(!a.pll_ready(),"global output gate must be enabled");put(&mut a,0x600b_00cc,1<<25);assert!(a.pll_ready());
        put(&mut a,0x600b_00cc,1<<5);assert!(!a.pll_ready());assert_eq!(a.deadline(),None);assert_eq!(a.read(MASTER+0x18).unwrap()&(1<<24),0);
        let before=a.clone();assert!(!a.write(MASTER+4,0x0266));assert_eq!(a,before);
    }
    #[test]
    fn premature_stop_routes_and_unsupported_analog_fields_do_not_fake_lock(){
        let mut a=analog_start();bb_config(&mut a);stop_calibration(&mut a);assert_eq!(a.deadline(),None);a.pump(100_000);assert!(!a.pll_ready());
        start_calibration(&mut a);assert_eq!(a.deadline(),Some(124_000));put(&mut a,MASTER+0x1c,DIG_ROUTE);assert_eq!(a.deadline(),None);
        a.pump(200_000);put(&mut a,MASTER+0x1c,PLL_ROUTE);assert_eq!(a.deadline(),Some(224_000));
        for(address,value)in[(0x600b_00cc,0x0200_0001),(0x600b_00cc,1<<27),(0x600b_00ac,1<<29),(MASTER+0x18,0x00e41c),
            (MASTER+0x18,0x00e409),(MASTER+4,0x0180_0266),(MASTER+4,0x0180_0566),(MASTER+4,0x0101_0666),(MASTER+4,0x0800_0066)]{
            let before=a.clone();assert!(!a.write(address,value),"accepted{address:#x}={value:#x}");assert_eq!(a,before);
        }
        for command in[0x0866,0x0066,0x046d,0x096d]{let before=a.clone();assert!(!a.write(MASTER+4,command));assert_eq!(a,before);}
        assert_eq!(a.analog.byte(0x66,8),None);assert!(!a.pll_ready());
    }
    #[test]
    fn strict_selection_clock_inputs_and_reset_retention_follow_actual_guest_state(){
        let mut a=C5BootAnalog::virtual_usb3v3();assert!(!a.pll_ready());assert!(!a.rc_fast_powered());assert_eq!(a.slow_dcap(),None);assert_eq!(a.read(0x600b_00ac),None);
        let report=a.report();a.enable_virtual_pll(0);a.disable_virtual_pll();assert_eq!(a.report(),report);
        a=analog_start();assert_eq!(byte(&mut a,0x6d,14,None),0);assert_eq!(a.slow_dcap(),None);dig_config(&mut a);assert_eq!(a.slow_dcap(),Some(128));
        assert!(a.write_at(0x600b_00ac,0,700));assert!(!a.rc_fast_powered());assert_eq!(a.read_at(0x600b_00ac,701),Some(0));put(&mut a,0x600b_00ac,1<<30);
        bb_config(&mut a);let at=a.deadline().unwrap();a.pump(at);stop_calibration(&mut a);assert!(a.pll_ready());
        let before=a.clone();a.reset_domains(false,false,false);assert_eq!(a,before);a.reset_domains(true,false,false);assert!(!a.pll_ready());assert_eq!(a.slow_dcap(),Some(128));
        a.reset_domains(true,true,false);assert!(a.rc_fast_powered());assert_eq!(a.slow_dcap(),Some(128));a.reset_domains(false,false,true);assert_eq!(a.slow_dcap(),None);
        assert_eq!(a.analog.byte(0x6d,13),Some(0x0c));hardware_init(&mut a);dig_config(&mut a);bb_config(&mut a);assert!(a.deadline().is_some());
        let bytes=a.analog.byte(0x66,3);a.disable_virtual_pll();assert!(!a.pll_ready());assert!(!a.rc_fast_powered());assert_eq!(a.slow_dcap(),None);assert_eq!(a.deadline(),None);
        a.pump(u64::MAX);assert_eq!(a.read(MASTER+0x18),None);assert_eq!(a.analog.byte(0x66,3),bytes);let before=a.clone();assert!(!a.write(0x600b_00cc,0x7200_0000));assert_eq!(a,before);
    }
    #[test]
    fn dig_and_pll_master_selection_power_separation_and_timeline_limit_are_real(){
        let mut a=C5BootAnalog::virtual_usb3v3();a.enable_virtual_pll(0);hardware_init(&mut a);
        put(&mut a,MASTER+0x20,4|(1<<9)|(1<<12));put(&mut a,MASTER+0x1c,DIG_ROUTE);
        let before=a.clone();assert!(!a.write(MASTER+4,0x0100_0e6d));assert_eq!(a,before);
        assert_eq!(byte(&mut a,0x6d,14,Some(128)),128);assert_eq!(a.slow_dcap(),Some(128));
        put(&mut a,MASTER+0x1c,SAR_ROUTE);assert!(!a.write(MASTER+4,0x0a69),"SAR still needs its separate PMU reset-release/power");
        bb_config(&mut a);let at=a.deadline().unwrap();a.pump(at);stop_calibration(&mut a);assert!(a.pll_ready());
        put(&mut a,MASTER+0x1c,DIG_ROUTE);assert!(a.pll_ready());
        let mut end=analog_start();end.pump(u64::MAX-10);bb_config(&mut end);assert_eq!(end.deadline(),None);end.pump(u64::MAX);
        let before=end.clone();for _ in 0..8{end.read_at(MASTER+0x18,u64::MAX);end.pump(u64::MAX);}assert_eq!(end,before);assert!(!end.pll_ready());
    }
    fn sar_byte(a:&mut C5BootAnalog,reg:u8,data:Option<u8>)->u8{
        put(a,MASTER+0x1c,SAR_ROUTE);let n=usize::from(a.read(MASTER+0x20).unwrap()&(1<<11)==0);
        let address=MASTER+n as u32*4;put(a,address,(reg as u32)<<8|0x69|data.map_or(0,|v|1<<24|(v as u32)<<16));
        let word=a.read(address).unwrap();assert_eq!(word&(1<<25),0);(word>>16)as u8
    }
    fn sar_mask(a:&mut C5BootAnalog,reg:u8,mask:u8,value:u8){let old=sar_byte(a,reg,None);sar_byte(a,reg,Some((old&!mask)|value));}
    fn sar_init(a:&mut C5BootAnalog){
        sar_mask(a,7,3,0);sar_mask(a,7,0x40,0x40);sar_mask(a,8,1,1);sar_mask(a,8,4,4);
        sar_mask(a,1,15,8);sar_mask(a,0,255,102);sar_mask(a,4,15,8);sar_mask(a,3,255,102);
    }
    #[test]
    fn actual_boot_sar_init_targets_are_real_transfers_and_do_not_change_vdet(){
        let mut a=C5BootAnalog::virtual_usb3v3();a.enable_virtual_sar();hardware_init(&mut a);sar_enable(&mut a);clear_vdet(&mut a,0);
        assert!(!a.sar_reserved_path_ready());assert_eq!(a.sar.codes(),None);let counts=(a.reads,a.writes);let vdet=a.vdet;
        for reg in [0,1,3,4,7,8]{assert_eq!(sar_byte(&mut a,reg,None),0);}assert!(!a.sar_reserved_path_ready());assert_eq!(a.sar.codes(),None);
        sar_init(&mut a);assert!(a.sar_reserved_path_ready());assert_eq!(a.sar.codes(),Some([2150;2]));assert_eq!(a.vdet,vdet);
        assert_eq!((a.reads-counts.0,a.writes-counts.1),(14,8));let report=a.report();
        assert!(report.contains("sar_regi2c=virtual_zero_initial written=0x3f initial_codes=Some([2150, 2150]) reserved_path_configured=1"));
        put(&mut a,MODEM_CLK,0);put(&mut a,MODEM_FORCE,0);assert!(a.sar_reserved_path_ready(),"master clock does not substitute for ADC clock");
    }
    #[test]
    fn sar_virtual_bytes_are_strict_opt_in_and_unknown_analog_modes_reject(){
        let mut a=C5BootAnalog::virtual_usb3v3();hardware_init(&mut a);sar_enable(&mut a);let before=a.clone();assert!(!a.write(MASTER+4,0x0769));assert_eq!(a,before);
        a.enable_virtual_sar();for(reg,data)in [(1,0x10),(4,0x80),(7,4),(7,0x80),(8,2),(8,8),(2,0),(5,0),(6,0),(9,0),(11,0)]{
            let before=a.clone();assert!(!a.write(MASTER+4,1<<24|(data as u32)<<16|(reg as u32)<<8|0x69));assert_eq!(a,before);
        }
        for address in [MASTER,MASTER+4]{put(&mut a,MASTER+0x20,if address==MASTER{0x804}else{4});put(&mut a,address,0x0769);assert_eq!(a.read(address).unwrap()>>16&255,0);}
        put(&mut a,RF_PWC,0x0800_0000);let before=a.clone();assert!(!a.write(MASTER,0x0100_0769));assert_eq!(a,before);assert!(!a.sar_reserved_path_ready());
    }
    #[test]
    fn calibration_targets_derive_guest_values_and_deinit_withdraws_reserved_route(){
        let mut a=C5BootAnalog::virtual_usb3v3();a.enable_virtual_sar();hardware_init(&mut a);sar_enable(&mut a);sar_init(&mut a);
        sar_mask(&mut a,1,15,0);sar_mask(&mut a,0,255,0);sar_mask(&mut a,4,15,15);sar_mask(&mut a,3,255,255);
        assert_eq!(a.sar.codes(),Some([0,4095]));assert!(a.sar_reserved_path_ready());sar_mask(&mut a,7,0x40,0);assert!(!a.sar_reserved_path_ready());
        sar_mask(&mut a,8,1,0);sar_mask(&mut a,8,4,0);assert_eq!(sar_byte(&mut a,7,None),0);assert_eq!(sar_byte(&mut a,8,None),0);
        assert_eq!(a.sar.codes(),Some([0,4095]));assert!(!a.sar_reserved_path_ready());
    }
    #[test]
    fn sar_target_retention_is_explicit_and_raw_deselection_never_supplies_configured_inputs(){
        let mut a=C5BootAnalog::virtual_usb3v3();let report=a.report();a.enable_virtual_sar();a.disable_virtual_sar();assert_eq!(a.report(),report);
        a.enable_virtual_sar();hardware_init(&mut a);sar_enable(&mut a);sar_init(&mut a);let bytes=a.sar.clone();
        a.enable_virtual_sar();assert_eq!(a.sar,bytes);a.reset_domains(false,false,false);assert_eq!(a.sar,bytes);
        a.reset_domains(true,false,false);assert_eq!(a.sar,bytes);assert!(a.sar_reserved_path_ready());
        a.reset_domains(true,true,false);assert_eq!(a.sar,bytes);assert!(!a.sar_reserved_path_ready());
        hardware_init(&mut a);sar_enable(&mut a);assert!(a.sar_reserved_path_ready());a.disable_virtual_sar();assert!(!a.sar_reserved_path_ready());
        let before=a.clone();assert!(!a.write(MASTER+4,0x0769));assert_eq!(a,before);a.enable_virtual_sar();assert_eq!(a.sar,bytes);
        a.reset_domains(false,false,true);assert_eq!(a.sar.codes(),None);assert!(!a.sar_reserved_path_ready());assert_eq!(a.vdet,15);
    }
    #[test]
    fn app_xtal32k_power_request_is_stored_without_manufacturing_a_source_or_pll_lock(){
        let mut a=C5BootAnalog::virtual_usb3v3();a.enable_virtual_pll(0);assert_eq!(a.read(0x600b_00ac),Some(0x4000_0000));
        put(&mut a,0x600b_00ac,0x5000_0000);assert_eq!(a.read(0x600b_00ac),Some(0x5000_0000));assert!(a.rc_fast_powered());assert!(!a.pll_ready());assert_eq!(a.slow_dcap(),None);
        put(&mut a,0x600b_00ac,0x1000_0000);assert!(!a.rc_fast_powered());assert!(!a.pll_ready());
        for value in [0x6000_0000,0xc000_0000,1,u32::MAX]{let before=a.clone();assert!(!a.write(0x600b_00ac,value));assert_eq!(a,before);}
        a.reset_domains(false,true,false);assert_eq!(a.read(0x600b_00ac),Some(0x4000_0000));
        a.disable_virtual_pll();assert_eq!(a.read(0x600b_00ac),None);assert!(!a.write(0x600b_00ac,0x5000_0000));
    }
    fn ocode_setup(cycle:u64)->C5BootAnalog{
        let mut a=C5BootAnalog::virtual_usb3v3();a.enable_virtual_pll(cycle);a.enable_virtual_ocode_ideal(cycle);
        a.observe_ocode_cpu_xtal(cycle,true);put(&mut a,MODEM_CLK,4);put(&mut a,MASTER+0x1c,ULP_ROUTE);a
    }
    fn ocode_command(reg:u8,data:Option<u8>)->u32{0x61|u32::from(reg)<<8|data.map_or(0,|v|(1<<24)|u32::from(v)<<16)}
    fn ocode_byte(a:&mut C5BootAnalog,reg:u8,data:Option<u8>,cycle:u64)->u8{
        let n=usize::from(a.conf2&(1<<10)==0);let address=MASTER+n as u32*4;
        assert!(a.write_at(address,ocode_command(reg,data),cycle));
        let value=a.read_at(address,cycle).unwrap();assert_eq!(value&(1<<25),0);(value>>16)as u8
    }
    fn ocode_start(a:&mut C5BootAnalog,cycle:u64){assert_eq!(ocode_byte(a,0,Some(0),cycle),0);assert_eq!(ocode_byte(a,0,Some(1),cycle),1);}
    #[test]
    fn ocode_requires_real_reset_then_elapsed_ideal_target_time_not_busy(){
        let mut a=ocode_setup(100);assert_eq!(ocode_byte(&mut a,0,None,100),0);assert_eq!(ocode_byte(&mut a,3,None,100),0);
        let before=a.clone();assert!(!a.write_at(MASTER+4,ocode_command(0,Some(1)),100));assert_eq!(a,before);
        ocode_start(&mut a,100);assert_eq!(a.deadline(),Some(24_100));assert_eq!(a.read(MASTER+4),Some(0x0101_0061));
        assert_eq!(ocode_byte(&mut a,3,None,100),0);assert_eq!(ocode_byte(&mut a,3,None,24_099),0);assert_eq!(a.deadline(),Some(24_100));
        assert_eq!(ocode_byte(&mut a,3,None,24_100),9);assert_eq!(a.deadline(),None);assert!(!a.pll_ready());
        assert!(a.report().contains("attempts=1 completions=1"));assert!(a.report().contains("code=unsupported physical_calibration=unmodeled"));
        assert!(a.report().contains("reset0_at=Some(100) reset1_at=Some(100) start_cpu_observed_at=Some(100)"));
        assert!(a.report().contains("polls=4 first_reg3=Some((100, 0)) last_reg3=Some((24100, 9))"));
        assert_eq!((a.reads,a.writes),(5,2));
    }
    #[test]
    fn ocode_repeated_one_cannot_restart_or_skip_a_new_reset(){
        let mut a=ocode_setup(0);ocode_start(&mut a,0);a.pump(12_000);
        let before=a.clone();assert!(!a.write(MASTER+4,ocode_command(0,Some(1))));assert_eq!(a,before);assert_eq!(a.deadline(),Some(24_000));
        a.enable_virtual_ocode_ideal(12_000);assert_eq!(a,before);assert_eq!(ocode_byte(&mut a,3,None,24_000),9);
        let before=a.clone();assert!(!a.write(MASTER+4,ocode_command(0,Some(1))));assert_eq!(a,before);
        assert_eq!(ocode_byte(&mut a,0,Some(0),24_000),0);assert_eq!(ocode_byte(&mut a,3,None,24_000),0);
        assert_eq!(ocode_byte(&mut a,0,Some(1),24_001),1);assert_eq!(a.deadline(),Some(48_001));assert_eq!(ocode_byte(&mut a,3,None,48_000),0);
        assert_eq!(ocode_byte(&mut a,3,None,48_001),9);assert!(a.report().contains("resets=2 attempts=2 completions=2"));
    }
    #[test]
    fn ocode_actual_pll_power_latches_and_committed_cpu_observation_guard_start(){
        let mut a=ocode_setup(0);a.observe_ocode_cpu_xtal(0,false);ocode_byte(&mut a,0,Some(0),0);
        let before=a.clone();assert!(!a.write(MASTER+4,ocode_command(0,Some(1))));assert_eq!(a,before);
        a.observe_ocode_cpu_xtal(0,true);
        for(high,low)in[(25,0),(29,4),(30,5)]{
            put(&mut a,0x600b_00cc,1<<high);assert!(!a.pll_ready());let before=a.clone();
            assert!(!a.write(MASTER+4,ocode_command(0,Some(1))));assert_eq!(a,before);put(&mut a,0x600b_00cc,1<<low);
        }
        // BB_I2C is deliberately retained by the fixed disable sequence.
        put(&mut a,0x600b_00cc,1<<28);assert!(a.report().contains("power_latches=0x2"));
        ocode_byte(&mut a,0,Some(1),0);assert_eq!(a.deadline(),Some(24_000));
        a.disable_virtual_pll();assert_eq!(a.deadline(),None);assert!(a.report().contains("state=Aborted"));
        a.enable_virtual_pll(0);let before=a.clone();assert!(!a.write(MASTER+4,ocode_command(0,Some(1))));assert_eq!(a,before);
        ocode_start(&mut a,0);assert_eq!(a.deadline(),Some(24_000));
    }
    #[test]
    fn ocode_cpu_loss_cancels_with_old_input_first_and_does_not_resume(){
        let mut a=ocode_setup(100);ocode_start(&mut a,100);a.observe_ocode_cpu_xtal(24_099,false);
        assert_eq!(a.deadline(),None);assert_eq!(ocode_byte(&mut a,3,None,24_100),0);
        a.observe_ocode_cpu_xtal(30_000,true);a.pump(100_000);assert_eq!(a.deadline(),None);assert_eq!(ocode_byte(&mut a,3,None,100_000),0);
        let before=a.clone();assert!(!a.write(MASTER+4,ocode_command(0,Some(1))));assert_eq!(a,before);
        ocode_start(&mut a,100_000);assert_eq!(a.deadline(),Some(124_000));
        // At the exact deadline the old valid input completes first.
        a.observe_ocode_cpu_xtal(124_000,false);assert_eq!(ocode_byte(&mut a,3,None,124_000),9);
        assert!(a.report().contains("attempts=2 completions=1 cancellations=1"));
    }
    #[test]
    fn ocode_quiet_loss_cancels_before_deadline_but_restored_pll_keeps_done(){
        let mut a=ocode_setup(0);ocode_start(&mut a,0);assert!(a.write_at(0x600b_00cc,1<<30,23_999));
        assert_eq!(a.deadline(),None);assert_eq!(ocode_byte(&mut a,3,None,24_000),0);
        assert!(a.write_at(0x600b_00cc,1<<5,25_000));a.pump(50_000);assert_eq!(ocode_byte(&mut a,3,None,50_000),0);
        let before=a.clone();assert!(!a.write(MASTER+4,ocode_command(0,Some(1))));assert_eq!(a,before);
        ocode_start(&mut a,50_000);assert!(a.write_at(0x600b_00cc,1<<25,74_000));a.observe_ocode_cpu_xtal(74_000,false);
        assert_eq!(ocode_byte(&mut a,3,None,74_000),9);a.pump(100_000);assert_eq!(ocode_byte(&mut a,3,None,100_000),9);
        assert!(a.report().contains("power_latches=0x1"));
    }
    #[test]
    fn ocode_master_selector_route_and_gates_constrain_transactions_not_target_clock(){
        let mut a=ocode_setup(0);let before=a.clone();assert!(!a.write(MASTER,ocode_command(0,None)));assert_eq!(a,before);
        put(&mut a,MASTER+0x20,0x404);ocode_start(&mut a,0);assert_eq!(a.read(MASTER),Some(0x0101_0061));
        let before=a.clone();assert!(!a.write(MASTER+4,ocode_command(3,None)));assert_eq!(a,before);
        put(&mut a,MASTER+0x1c,SAR_ROUTE);let before=a.clone();assert!(!a.write(MASTER,ocode_command(3,None)));assert_eq!(a,before);
        put(&mut a,MODEM_CLK,0);assert_eq!(a.read(MASTER),None);a.pump(24_000);assert_eq!(a.deadline(),None);
        put(&mut a,MODEM_FORCE,4);put(&mut a,MASTER+0x1c,ULP_ROUTE);assert_eq!(ocode_byte(&mut a,3,None,24_000),9);
        // The SAR RF/reset gate is neither a prerequisite nor a target clock.
        assert_eq!(a.rf,0x0800_0000);assert!(!a.sar_reserved_path_ready());
        domains(&mut a,0,0);let before=a.clone();assert!(!a.write(MASTER,ocode_command(3,None)));assert_eq!(a,before);
        domains(&mut a,0x4000_0000,0x4000_0000);assert_eq!(ocode_byte(&mut a,3,None,24_000),9);
    }
    #[test]
    fn ocode_unknown_target_bits_code_bytes_and_status_writes_stay_unsupported(){
        let mut a=ocode_setup(0);
        for(reg,data)in[(0,Some(2)),(0,Some(4)),(0,Some(0x10)),(0,Some(0x40)),(0,Some(0xff)),(3,Some(0)),(3,Some(9)),
            (1,None),(2,None),(4,None),(5,None),(6,None),(4,Some(0)),(5,Some(0x40)),(6,Some(28))]{
            let before=a.clone();assert!(!a.write(MASTER+4,ocode_command(reg,data)));assert_eq!(a,before);
        }
        for value in[0x00ff_ffbf,0x00ff_ffff,u32::MAX]{let before=a.clone();assert!(!a.write(MASTER+0x1c,value));assert_eq!(a,before);}
        let before=a.clone();assert!(!a.write(MASTER+0x20,0x104));assert_eq!(a,before);
        // Existing controller semantics ignore supplied RO BUSY25; the
        // actual accepted target transaction returns the real byte, BUSY0.
        put(&mut a,MASTER+4,ocode_command(0,None)|(1<<25));assert_eq!(a.read(MASTER+4),Some(0x61));
    }
    #[test]
    fn ocode_reset_domains_and_profile_withdrawal_have_explicit_virtual_retention(){
        let mut a=ocode_setup(0);ocode_start(&mut a,0);a.pump(100);let before=a.clone();a.reset_domains(false,false,false);assert_eq!(a,before);
        a.reset_domains(true,false,false);assert_eq!(a.deadline(),None);put(&mut a,MODEM_CLK,4);put(&mut a,MASTER+0x1c,ULP_ROUTE);
        a.observe_ocode_cpu_xtal(100,true);assert_eq!(ocode_byte(&mut a,3,None,100),0);let before=a.clone();
        assert!(!a.write(MASTER+4,ocode_command(0,Some(1))));assert_eq!(a,before);ocode_start(&mut a,100);a.pump(24_100);
        a.reset_domains(true,false,false);put(&mut a,MODEM_CLK,4);put(&mut a,MASTER+0x1c,ULP_ROUTE);assert_eq!(ocode_byte(&mut a,3,None,24_100),9);
        a.reset_domains(false,true,false);assert_eq!(ocode_byte(&mut a,3,None,24_100),0);assert_eq!(ocode_byte(&mut a,0,None,24_100),0);
        ocode_byte(&mut a,0,Some(0),24_100);let before=a.clone();assert!(!a.write(MASTER+4,ocode_command(0,Some(1))));assert_eq!(a,before);
        a.observe_ocode_cpu_xtal(24_100,true);ocode_byte(&mut a,0,Some(1),24_100);assert_eq!(a.deadline(),Some(48_100));
        a.reset_domains(false,false,true);assert_eq!(a.deadline(),None);put(&mut a,MODEM_CLK,4);put(&mut a,MASTER+0x1c,ULP_ROUTE);
        assert_eq!(ocode_byte(&mut a,3,None,24_100),0);assert!(a.report().contains("resets=0 attempts=0 completions=0"));
        a.observe_ocode_cpu_xtal(24_100,true);ocode_start(&mut a,24_100);a.disable_virtual_ocode();a.pump(100_000);
        assert_eq!(a.deadline(),None);assert!(!a.report().contains("ocode_profile="));let before=a.clone();
        assert!(!a.write(MASTER+4,ocode_command(3,None)));assert!(!a.write(MASTER+0x1c,ULP_ROUTE));assert_eq!(a,before);
        a.enable_virtual_ocode_ideal(100_000);assert_eq!(ocode_byte(&mut a,3,None,100_000),0);
        let before=a.clone();assert!(!a.write(MASTER+4,ocode_command(0,Some(1))));assert_eq!(a,before);
    }
    #[test]
    fn ocode_unselected_and_deselected_profile_preserve_legacy_register_behavior(){
        let mut a=C5BootAnalog::virtual_usb3v3();let before=a.clone();a.observe_ocode_cpu_xtal(u64::MAX,true);a.disable_virtual_ocode();a.pump(u64::MAX);assert_eq!(a,before);
        a.enable_virtual_ocode_ideal(123);a.disable_virtual_ocode();assert_eq!(a,before);
        hardware_init(&mut a);let before=a.clone();assert!(!a.write(MASTER+0x1c,ULP_ROUTE));assert!(!a.write(MASTER+0x20,0x404));
        assert!(!a.write(MASTER+4,ocode_command(0,None)));assert_eq!(a,before);assert_eq!(a.deadline(),None);
        assert!(!a.report().contains("ocode"));sar_enable(&mut a);assert_eq!(clear_vdet(&mut a,0),14);
    }
    #[test]
    fn ocode_lp_reset_and_guest_reset_cancel_pending_without_late_completion(){
        let mut a=ocode_setup(0);ocode_start(&mut a,0);ocode_byte(&mut a,0,Some(0),23_999);a.pump(24_000);
        assert_eq!(ocode_byte(&mut a,3,None,24_000),0);assert_eq!(a.deadline(),None);ocode_byte(&mut a,0,Some(1),24_000);
        a.pump(24_001);a.reset_domains(false,true,false);a.pump(100_000);assert_eq!(ocode_byte(&mut a,3,None,100_000),0);
        assert!(a.report().contains("attempts=2 completions=0 cancellations=2"));
        a.observe_ocode_cpu_xtal(100_000,true);let before=a.clone();assert!(!a.write(MASTER+4,ocode_command(0,Some(1))));assert_eq!(a,before);
        ocode_start(&mut a,100_000);assert_eq!(ocode_byte(&mut a,3,None,124_000),9);
    }
    #[test]
    fn ocode_profile_withdrawal_preserves_an_independent_existing_pll_attempt(){
        let mut a=analog_start();bb_config(&mut a);let before=a.clone();let at=a.deadline().unwrap();
        a.enable_virtual_ocode_ideal(0);a.observe_ocode_cpu_xtal(0,false);a.disable_virtual_ocode();assert_eq!(a,before);
        assert_eq!(a.deadline(),Some(at));a.pump(at);stop_calibration(&mut a);assert!(a.pll_ready());assert!(!a.report().contains("ocode_profile="));
    }
    #[test]
    fn unselected_ocode_observation_does_not_advance_an_existing_pll_or_change_its_report(){
        let mut a=analog_start();bb_config(&mut a);let at=a.deadline().unwrap();let before=a.clone();let report=a.report();
        a.observe_ocode_cpu_xtal(at,true);assert_eq!(a,before);assert_eq!(a.report(),report);assert!(!report.contains("power_latches="));
        a.enable_virtual_ocode_ideal(0);a.disable_virtual_ocode();a.observe_ocode_cpu_xtal(u64::MAX,false);
        assert_eq!(a,before);assert_eq!(a.report(),report);
    }
    #[test]
    fn ocode_checked_deadline_overflow_and_retrograde_cycle_never_half_start(){
        let mut a=ocode_setup(u64::MAX-23_999);ocode_byte(&mut a,0,Some(0),u64::MAX-23_999);let before=a.clone();
        assert!(!a.write(MASTER+4,ocode_command(0,Some(1))));assert_eq!(a,before);assert_eq!(a.deadline(),None);
        let mut a=ocode_setup(u64::MAX-24_000);ocode_start(&mut a,u64::MAX-24_000);assert_eq!(a.deadline(),Some(u64::MAX));
        a.pump(0);assert_eq!(a.deadline(),Some(u64::MAX));assert_eq!(ocode_byte(&mut a,3,None,u64::MAX-1),0);
        a.observe_ocode_cpu_xtal(u64::MAX,false);assert_eq!(ocode_byte(&mut a,3,None,u64::MAX),9);assert_eq!(a.deadline(),None);
    }
}

#[cfg(test)]
mod bod_threshold_tests{
    use super::*;
    fn selected()->C5BootAnalog{let mut a=C5BootAnalog::virtual_usb3v3();a.enable_virtual_bod_threshold();assert!(a.write(MODEM_CLK,4));assert!(a.write(MASTER+0x1c,ULP_ROUTE));a}
    fn read_byte(a:&mut C5BootAnalog,n:u32)->u8{assert!(a.write(MASTER+n*4,0x0561));(a.read(MASTER+n*4).unwrap()>>16)as u8}
    #[test]
    fn real_bod_masked_rmw_qualifies_threshold_only_after_target_write(){
        let mut a=selected();assert_eq!(a.bod_threshold(),None);assert_eq!(read_byte(&mut a,1),0);assert_eq!(a.bod_threshold(),None);
        assert!(a.write(MASTER+4,0x0107_0561));assert_eq!(a.bod_threshold(),Some(7));assert_eq!((a.reads,a.writes),(1,1));
        let before=a.clone();for _ in 0..8{assert_eq!(a.read(MASTER+4),Some(0x0107_0561));assert_eq!(a.bod_threshold(),Some(7));}assert_eq!(a,before);
        assert_eq!(read_byte(&mut a,1),7);assert!(a.report().contains("BOD_threshold_byte=0x07 BOD_threshold_written=1"));
    }
    #[test]
    fn target_profile_route_master_and_real_clock_remain_required(){
        let mut a=C5BootAnalog::virtual_usb3v3();let before=a.clone();assert!(!a.write(MASTER+4,0x0561));assert_eq!(a,before);
        a.enable_virtual_bod_threshold();assert!(!a.write(MASTER+4,0x0561));assert!(a.write(MODEM_CLK,4));assert!(!a.write(MASTER+4,0x0561));
        assert!(a.write(MASTER+0x1c,ULP_ROUTE));assert!(a.write(MASTER+0x20,0x404));let before=a.clone();
        assert!(!a.write(MASTER+4,0x0107_0561));assert_eq!(a,before);assert_eq!(read_byte(&mut a,0),0);assert!(a.write(MASTER,0x0107_0561));
        assert!(a.write(MODEM_CLK,0));let before=a.clone();assert!(!a.write(MASTER,0x0561));assert_eq!(a,before);
        assert!(a.write(MODEM_FORCE,4));assert_eq!(read_byte(&mut a,0),7);
    }
    #[test]
    fn other_fields_codes_targets_and_ocode_aliases_reject_atomically(){
        let mut a=selected();
        for v in[0x0100_0561,0x0106_0561,0x0108_0561,0x0187_0561,0x0061,0x0361,0x0461,0x0661,0x0562,0x0400_0561]{
            let before=a.clone();assert!(!a.write(MASTER+4,v),"{v:#x}");assert_eq!(a,before);
        }
        assert_eq!(a.bod_threshold(),None);assert_eq!((a.reads,a.writes),(0,0));
    }
    #[test]
    fn bod_byte_does_not_need_ocode_quiet_and_never_changes_other_targets(){
        let mut a=selected();a.enable_virtual_pll(0);a.enable_virtual_ocode_ideal(0);let before=(a.analog.clone(),a.ocode.clone(),a.sar.clone(),a.vdet,a.bod,a.fib);
        assert_eq!(read_byte(&mut a,1),0);assert!(a.write(MASTER+4,0x0107_0561));
        assert_eq!((a.analog.clone(),a.ocode.clone(),a.sar.clone(),a.vdet,a.bod,a.fib),before);
        a.disable_virtual_bod_threshold();assert_eq!(a.bod_threshold(),None);let before=a.clone();assert!(!a.write(MASTER+4,0x0561));assert_eq!(a,before);
        assert!(a.write(MASTER+4,0x0061));assert_eq!(a.read(MASTER+4),Some(0x0061));
    }
    #[test]
    fn analog_byte_retains_across_digital_reset_and_chip_requires_real_rewrite(){
        let mut a=selected();assert!(a.write(MASTER+4,0x0107_0561));let before=a.clone();assert!(!a.reset_domains(false,false,false));assert_eq!(a,before);
        a.reset_domains(true,false,false);assert_eq!(a.bod_threshold(),Some(7));a.reset_domains(true,true,false);assert_eq!(a.bod_threshold(),Some(7));
        a.reset_domains(true,true,true);assert_eq!(a.bod_threshold(),None);assert_eq!(a.bod_byte,0);assert!(a.bod_threshold_profile);
        assert!(a.write(MODEM_CLK,4));assert!(a.write(MASTER+0x1c,ULP_ROUTE));assert_eq!(read_byte(&mut a,1),0);assert_eq!(a.bod_threshold(),None);
        assert!(a.write(MASTER+4,0x0107_0561));a.disable_virtual_bod_threshold();assert_eq!(a.bod_threshold(),None);let before=a.clone();
        assert!(!a.write(MASTER+4,0x0561));assert_eq!(a,before);a.enable_virtual_bod_threshold();assert_eq!(a.bod_threshold(),Some(7));
    }
}

#[cfg(test)]
mod wifi_icg_tests{
    use super::*;
    const FIELDS:[(u32,u32,u32);10]=[(MODEM_MAP,28,6),(MODEM_MAP,24,4),(MODEM_MAP,20,6),
        (MODEM_MAP,16,4),(MODEM_MAP,12,6),(MODEM_MAP,8,4),(LPCON_MAP,28,6),
        (LPCON_MAP,24,6),(LPCON_MAP,20,6),(LPCON_MAP,16,6)];
    fn put(a:&mut C5BootAnalog,address:u32,value:u32){assert!(a.write(address,value),"{address:#x}={value:#x}");}
    fn preinit()->C5BootAnalog{
        let mut a=C5BootAnalog::virtual_usb3v3();put(&mut a,MODEM_CLK,4);put(&mut a,PMU_ACTIVE_MODEM,0x8000_0000);
        put(&mut a,MODEM_MAP,0x4000_0000);put(&mut a,LPCON_MAP,0x4400_0000);put(&mut a,PMU_MODEM_UPDATE,0x8000_0000);
        put(&mut a,PMU_SWITCH_UPDATE,0x1000_0000);a
    }
    fn init_maps(a:&mut C5BootAnalog){for(address,shift,init)in FIELDS{let old=a.read(address).unwrap();put(a,address,old|(init<<shift));}}
    #[test]
    fn original_ten_field_or_sequence_keeps_awake_permissions_and_real_target_access(){
        let mut a=preinit();let mut words=Vec::new();
        for(address,shift,init)in FIELDS{
            let old=a.read(address).unwrap();put(&mut a,address,old|(init<<shift));words.push(a.read(address).unwrap());
            assert_eq!((a.permissions,a.active_modem,a.captured_code,a.code_updates,a.switch_updates),(Some(7),0x8000_0000,2,1,1));
            assert!(a.clocked());assert!(a.lp_apb_accessible());assert!(!a.pll_ready());assert_eq!(a.deadline(),None);
        }
        assert_eq!(words,[0x6000_0000,0x6400_0000,0x6460_0000,0x6464_0000,0x6464_6000,0x6464_6400,
            0x6400_0000,0x6600_0000,0x6660_0000,0x6666_0000]);
        assert_eq!((a.modem_map,a.lp_map),(MODEM_MAP_INIT,LPCON_MAP_INIT));
        let before=a.clone();init_maps(&mut a);assert_eq!(a,before);
        put(&mut a,RF_PWC,0x0c00_0000);put(&mut a,MASTER+0x1c,SAR_ROUTE);put(&mut a,MASTER+4,0x0a69);
        assert_eq!(a.read(MASTER+4),Some(0x000f_0a69));assert_eq!((a.reads,a.writes),(1,0));
        let before=a.clone();for _ in 0..8{a.read(MODEM_MAP);a.read(LPCON_MAP);a.lp_apb_accessible();a.report();}assert_eq!(a,before);
    }
    #[test]
    fn every_field_accepts_only_its_exact_bitmap_and_mixed_words_remain_transactional(){
        let mut a=preinit();
        for(address,shift,init)in FIELDS{for bitmap in 0..16{
            let value=(a.read(address).unwrap()&!(15<<shift))|(bitmap<<shift);let before=a.clone();
            let expected=bitmap==0||bitmap==4||bitmap==init;assert_eq!(a.write(address,value),expected,"field {address:#x}:{shift} bitmap{bitmap}");
            if !expected{assert_eq!(a,before);}else{assert_eq!(a.read(address),Some(value));assert_eq!(a.permissions,Some(7));}
        }}
        for(sys,lp)in[(0x0460_4000,0x4604_0000),(0x6444_6000,0x6046_0000),(MODEM_MAP_INIT,LPCON_MAP_INIT)]{
            put(&mut a,MODEM_MAP,sys);put(&mut a,LPCON_MAP,lp);assert_eq!((a.modem_map,a.lp_map),(sys,lp));
        }
        for(address,low_bits)in[(MODEM_MAP,8),(LPCON_MAP,16)]{for bit in 0..low_bits{
            let value=a.read(address).unwrap()|(1<<bit);let before=a.clone();assert!(!a.write(address,value));assert_eq!(a,before);
        }}
        for(address,value)in[(MODEM_MAP,0x6264_6400),(MODEM_MAP,0x6466_6400),(MODEM_MAP,0x6464_6600),
            (LPCON_MAP,0x6266_0000),(LPCON_MAP,0x1666_0000),(LPCON_MAP,0x8666_0000),
            (PMU_ACTIVE_MODEM,0x4000_0000),(PMU_ACTIVE_MODEM,0xc000_0000)]{
            let before=a.clone();assert!(!a.write(address,value));assert_eq!(a,before);
        }
    }
    #[test]
    fn shadow_writes_and_code_capture_do_not_grant_until_actual_apply(){
        let mut a=C5BootAnalog::virtual_usb3v3();put(&mut a,MODEM_CLK,4);put(&mut a,PMU_SWITCH_UPDATE,0x1000_0000);
        assert_eq!(a.permissions,Some(0));assert_eq!(a.read(MASTER),None);assert!(!a.lp_apb_accessible());
        init_maps(&mut a);put(&mut a,PMU_ACTIVE_MODEM,0x8000_0000);assert_eq!(a.permissions,Some(0));assert_eq!(a.captured_code,0);
        put(&mut a,PMU_MODEM_UPDATE,0x8000_0000);assert_eq!(a.captured_code,2);assert_eq!(a.read(MASTER),None);
        put(&mut a,PMU_SWITCH_UPDATE,0x1000_0000);assert_eq!(a.permissions,Some(7));assert_eq!(a.read(MASTER),Some(0));
        let before=a.clone();put(&mut a,PMU_MODEM_UPDATE,0);put(&mut a,PMU_SWITCH_UPDATE,0);assert_eq!(a,before);
        assert_eq!(a.read(PMU_MODEM_UPDATE),Some(0));assert_eq!(a.read(PMU_SWITCH_UPDATE),Some(0));
        put(&mut a,PMU_SWITCH_UPDATE,0x1000_0000);assert_eq!(a.permissions,Some(7));assert_eq!((a.code_updates,a.switch_updates),(1,3));
    }
    #[test]
    fn reapply_uses_actual_active_bit_and_force_never_bypasses_apb(){
        let mut a=preinit();init_maps(&mut a);put(&mut a,MODEM_MAP,0x0464_6400);assert_eq!(a.read(MASTER),Some(0));
        put(&mut a,PMU_SWITCH_UPDATE,0x1000_0000);assert_eq!(a.permissions,Some(6));put(&mut a,MODEM_FORCE,4);assert_eq!(a.read(MASTER),None);
        put(&mut a,MODEM_MAP,MODEM_MAP_INIT);assert_eq!(a.read(MASTER),None);put(&mut a,PMU_SWITCH_UPDATE,0x1000_0000);assert_eq!(a.read(MASTER),Some(0));
        put(&mut a,MODEM_FORCE,0);put(&mut a,LPCON_MAP,0x6066_0000);put(&mut a,PMU_SWITCH_UPDATE,0x1000_0000);
        assert_eq!(a.permissions,Some(3));assert!(a.lp_apb_accessible());assert_eq!(a.read(MASTER),None);put(&mut a,MODEM_FORCE,4);assert_eq!(a.read(MASTER),Some(0));
        put(&mut a,LPCON_MAP,0x0666_0000);put(&mut a,PMU_SWITCH_UPDATE,0x1000_0000);assert_eq!(a.permissions,Some(5));assert!(!a.lp_apb_accessible());assert_eq!(a.read(MASTER),None);
        assert_eq!(a.read(BOD1),Some(0));assert_eq!(a.read(RF_PWC),Some(0x0800_0000));assert!(!a.pll_ready());
    }
    #[test]
    fn map_configuration_never_supplies_rf_phy_calibration_or_analog_readiness(){
        let mut a=preinit();a.enable_virtual_pll(0);a.enable_virtual_sar();a.enable_virtual_ocode_ideal(0);a.enable_virtual_bod_threshold();
        let before=(a.analog.clone(),a.sar.clone(),a.ocode.clone(),a.vdet,a.bod,a.fib,a.rf,a.ctrl,a.reads,a.writes);
        init_maps(&mut a);assert_eq!((a.analog.clone(),a.sar.clone(),a.ocode.clone(),a.vdet,a.bod,a.fib,a.rf,a.ctrl,a.reads,a.writes),before);
        assert!(!a.pll_ready());assert!(!a.sar_reserved_path_ready());assert_eq!(a.slow_dcap(),None);assert_eq!(a.bod_threshold(),None);assert_eq!(a.deadline(),None);
        assert_eq!((a.active_modem,a.captured_code,a.permissions),(0x8000_0000,2,Some(7)));
        for address in[0x600a_9c00,0x600a_9c08,0x600a_9c10,0x600a_f024]{let before=a.clone();assert!(!a.write(address,u32::MAX));assert_eq!(a,before);}
    }
    #[test]
    fn cpu_retains_maps_hp_resets_both_and_lp_resets_only_the_pmu_code(){
        let mut a=preinit();init_maps(&mut a);let before=a.clone();a.reset_domains(false,false,false);assert_eq!(a,before);
        a.reset_domains(true,false,false);assert_eq!((a.modem_map,a.lp_map,a.permissions),(0,0,None));assert_eq!((a.active_modem,a.captured_code),(0x8000_0000,2));
        init_maps(&mut a);put(&mut a,PMU_SWITCH_UPDATE,0x1000_0000);assert_eq!(a.permissions,Some(7));
        let mut lp=preinit();init_maps(&mut lp);lp.reset_domains(false,true,false);
        assert_eq!((lp.modem_map,lp.lp_map),(MODEM_MAP_INIT,LPCON_MAP_INIT));assert_eq!((lp.active_modem,lp.captured_code,lp.permissions),(0,0,None));
        put(&mut lp,PMU_SWITCH_UPDATE,0x1000_0000);assert_eq!(lp.permissions,Some(0));assert_eq!(lp.read(MASTER),None);
        a.reset_domains(true,true,true);assert_eq!(a,C5BootAnalog::virtual_usb3v3());
    }
}
