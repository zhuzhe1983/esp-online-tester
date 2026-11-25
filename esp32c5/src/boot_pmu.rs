//! C5 PMU mode configuration for the explicit awake virtual profile.
//! Fixed IDF5.5.3/5.5.4 C5 pmu_reg.h/pmu_struct.h, pmu_init.c/param.c;
//! TRM1.1 13.4.2. Receipt: docs/evidence/c5-app-pmu-config-2026-11-24.
//! Future MODEM/SLEEP words are target configuration only. No sleep entry,
//! regulator voltage/readiness, retention execution or power transition.
const BASE:u32=0x600b_0000;
#[derive(Clone,Copy)]
enum Rule{Passive,ActivePower,ActiveIcg,ActiveSystem,ActiveClock,ActiveBias,ActiveSysclk,ActiveDriver,ActiveXtal,NoEnable(u32),Force,Memory,SleepProtect}
#[derive(Clone,Copy)]
struct Spec{offset:u16,mask:u32,por:u32,rule:Rule}
const fn spec(offset:u16,mask:u32,por:u32,rule:Rule)->Spec{Spec{offset,mask,por,rule}}
const SPECS:[Spec;51]=[
    spec(0x000,0xffe00000,0x00000000,Rule::ActivePower), // HP_ACTIVE_DIG_POWER
    spec(0x004,0xffffffff,0xffffffff,Rule::ActiveIcg), // HP_ACTIVE_ICG_HP_FUNC
    spec(0x008,0xffffffff,0xffffffff,Rule::ActiveIcg), // HP_ACTIVE_ICG_HP_APB
    spec(0x010,0x3f000000,0x00000000,Rule::ActiveSystem), // HP_ACTIVE_HP_SYS_CNTL
    spec(0x014,0x7c000000,0x00000000,Rule::ActiveClock), // HP_ACTIVE_HP_CK_POWER
    spec(0x018,0xfe000000,0x00000000,Rule::ActiveBias), // HP_ACTIVE_BIAS
    spec(0x01c,0x6fffdcf0,0x00000000,Rule::NoEnable(0x60001800)), // HP_ACTIVE_BACKUP
    spec(0x020,0xffffffff,0x00000000,Rule::Passive), // HP_ACTIVE_BACKUP_CLK
    spec(0x024,0xfc000000,0x00000000,Rule::ActiveSysclk), // HP_ACTIVE_SYSCLK
    spec(0x02c,0xffffff00,0x00000000,Rule::ActiveDriver), // HP_ACTIVE_HP_REGULATOR1
    spec(0x030,0x80000000,0x80000000,Rule::ActiveXtal), // HP_ACTIVE_XTAL
    spec(0x034,0xffe00000,0x00000000,Rule::Passive), // HP_MODEM_DIG_POWER
    spec(0x038,0xffffffff,0xffffffff,Rule::Passive), // HP_MODEM_ICG_HP_FUNC
    spec(0x03c,0xffffffff,0xffffffff,Rule::Passive), // HP_MODEM_ICG_HP_APB
    spec(0x040,0xc0000000,0x00000000,Rule::Passive), // HP_MODEM_ICG_MODEM
    spec(0x044,0x3f000000,0x00000000,Rule::Passive), // HP_MODEM_HP_SYS_CNTL
    spec(0x048,0x7c000000,0x00000000,Rule::Passive), // HP_MODEM_HP_CK_POWER
    spec(0x04c,0xfe000000,0x00000000,Rule::Passive), // HP_MODEM_BIAS
    spec(0x050,0x21f0cc30,0x00000000,Rule::NoEnable(0x20000800)), // HP_MODEM_BACKUP
    spec(0x054,0xffffffff,0x00000000,Rule::Passive), // HP_MODEM_BACKUP_CLK
    spec(0x058,0xfc000000,0x00000000,Rule::Passive), // HP_MODEM_SYSCLK
    spec(0x060,0xffffff00,0x00000000,Rule::Passive), // HP_MODEM_HP_REGULATOR1
    spec(0x064,0x80000000,0x80000000,Rule::Passive), // HP_MODEM_XTAL
    spec(0x068,0xffe00000,0x00000000,Rule::Passive), // HP_SLEEP_DIG_POWER
    spec(0x06c,0xffffffff,0xffffffff,Rule::Passive), // HP_SLEEP_ICG_HP_FUNC
    spec(0x070,0xffffffff,0xffffffff,Rule::Passive), // HP_SLEEP_ICG_HP_APB
    spec(0x074,0xc0000000,0x00000000,Rule::Passive), // HP_SLEEP_ICG_MODEM
    spec(0x078,0x3f000000,0x00000000,Rule::Passive), // HP_SLEEP_HP_SYS_CNTL
    spec(0x07c,0x7c000000,0x00000000,Rule::Passive), // HP_SLEEP_HP_CK_POWER
    spec(0x080,0xfe000000,0x00000000,Rule::Passive), // HP_SLEEP_BIAS
    spec(0x084,0xffff37c0,0x00000000,Rule::NoEnable(0xc0003000)), // HP_SLEEP_BACKUP
    spec(0x088,0xffffffff,0x00000000,Rule::Passive), // HP_SLEEP_BACKUP_CLK
    spec(0x08c,0xfc000000,0x00000000,Rule::Passive), // HP_SLEEP_SYSCLK
    spec(0x090,0xffff8000,0xc6678000,Rule::Passive), // HP_SLEEP_HP_REGULATOR0
    spec(0x094,0xffffff00,0x00000000,Rule::Passive), // HP_SLEEP_HP_REGULATOR1
    spec(0x098,0x80000000,0x80000000,Rule::Passive), // HP_SLEEP_XTAL
    spec(0x0a0,0xf0000000,0x00000000,Rule::ActiveDriver), // HP_SLEEP_LP_REGULATOR1
    spec(0x0a8,0xc0000000,0x00000000,Rule::ActivePower), // HP_SLEEP_LP_DIG_POWER
    spec(0x0b4,0xffe00000,0xc6600000,Rule::Passive), // LP_SLEEP_LP_REGULATOR0
    spec(0x0b8,0xf0000000,0x00000000,Rule::Passive), // LP_SLEEP_LP_REGULATOR1
    spec(0x0bc,0x80000000,0x80000000,Rule::Passive), // LP_SLEEP_XTAL
    spec(0x0c0,0xc0000000,0x00000000,Rule::Passive), // LP_SLEEP_LP_DIG_POWER
    spec(0x0c4,0xf0000000,0x40000000,Rule::Passive), // LP_SLEEP_LP_CK_POWER
    spec(0x0c8,0xfe000000,0x00000000,Rule::Passive), // LP_SLEEP_BIAS
    spec(0x0f8,0xf80007ff,0x0000001c,Rule::Force), // POWER_PD_TOP_CNTL
    spec(0x0fc,0xf80007ff,0x0000001c,Rule::Force), // POWER_PD_HPAON_CNTL
    spec(0x100,0xf80007ff,0x0000001c,Rule::Force), // POWER_PD_HPCPU_CNTL
    spec(0x108,0xf80007ff,0x0000001c,Rule::Force), // POWER_PD_HPWIFI_CNTL
    spec(0x10c,0x0000003f,0x0000001c,Rule::Force), // POWER_PD_LPPERI_CNTL
    spec(0x110,0xff0000ff,0xff000000,Rule::Memory), // POWER_PD_MEM_CNTL
    spec(0x130,0x0003ffff,0x00000000,Rule::SleepProtect), // SLP_WAKEUP_CNTL3
];
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct BootPmu{words:[u32;51],writes:u64}
impl BootPmu{
    pub fn new()->Self{Self{words:std::array::from_fn(|i|SPECS[i].por),writes:0}}
    fn index(address:u32)->Option<usize>{
        let offset=address.checked_sub(BASE)?;if offset>0x130||!offset.is_multiple_of(4){return None;}
        SPECS.iter().position(|s|u32::from(s.offset)==offset)
    }
    pub fn contains(address:u32)->bool{Self::index(address).is_some()}
    pub fn read(&self,address:u32)->Option<u32>{Some(self.words[Self::index(address)?])}
    pub fn write(&mut self,address:u32,value:u32)->bool{
        let Some(i)=Self::index(address)else{return false;};let s=SPECS[i];let old=self.words[i];
        if (old^value)&!s.mask!=0{return false;}
        let safe=match s.rule{
            Rule::Passive=>true,
            Rule::ActivePower|Rule::ActiveSystem|Rule::ActiveDriver=>value==0,
            Rule::ActiveIcg=>value==u32::MAX,
            // Initial zero targets are POR configuration, not an observed
            // powered-down live PLL/bias. Only safe enable or unchanged is
            // accepted; an active power-down/isolate/stop is never ignored.
            Rule::ActiveClock=>value==old||value==0x7000_0000,
            Rule::ActiveBias=>value==old||value==0x0200_0000,
            Rule::ActiveSysclk=>value==old||value==0x0800_0000,
            Rule::ActiveXtal=>value==0x8000_0000,
            Rule::NoEnable(mask)=>value&mask==0,
            Rule::Force=>(old^value)&!0x3f==0&&value&0x3f&!old==0,
            Rule::Memory=>(old^value)&!0x0f00_0000==0&&value&0x0f00_0000&!old==0,
            Rule::SleepProtect=>(old^value)&!0x0003_0000==0,
        };
        if !safe{return false;}self.words[i]=value;self.writes=self.writes.saturating_add(1);true
    }
    /// The PMU table is retained by CPU/Core reset in this declared functional
    /// profile; LP/System or Chip reset restores complete header defaults.
    /// Public sources do not identify every per-flop retention distinction.
    pub fn reset_domains(&mut self,lp:bool,analog:bool){if lp||analog{*self=Self::new();}}
    pub fn report(&self)->String{format!("PMU=awake_target_config active_pd={:#x} active_ck_target={:#x} active_bias_target={:#x} active_sysclk_target={:#x} sleep_protect={} writes={} future_MODEM_SLEEP=unexecuted transitions=unsupported voltage_ready=unmodeled retention=unexecuted",
        self.read(BASE).unwrap(),self.read(BASE+0x14).unwrap(),self.read(BASE+0x18).unwrap(),self.read(BASE+0x24).unwrap(),self.read(BASE+0x130).unwrap()>>16,self.writes)}
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::boot_bias::C5BootBias;
    use crate::boot_analog::C5BootAnalog;
    struct World{pmu:BootPmu,bias:C5BootBias,analog:C5BootAnalog}
    impl World{
        fn new()->Self{
            let mut w=Self{pmu:BootPmu::new(),bias:C5BootBias::virtual_usb3v3(),analog:C5BootAnalog::virtual_usb3v3()};
            for offset in [0x28,0x5c,0x9c]{let v=w.r(offset);assert!(w.bias.write(BASE+offset,(v&!0xf800_0000)|28<<27));}
            w.analog.enable_virtual_pll(0);for(a,v)in[(BASE+0xc,0x8000_0000),(0x600a_9c0c,0x4000_0000),(0x600a_f020,0x4400_0000),(BASE+0xdc,0x8000_0000),(BASE+0xd0,0x1000_0000)]{assert!(w.analog.write(a,v));}w
        }
        fn r(&mut self,offset:u32)->u32{
            let a=BASE+offset;if BootPmu::contains(a){self.pmu.read(a)}else if C5BootBias::contains(a){self.bias.read(a)}else{self.analog.read(a)}.unwrap()
        }
        fn w(&mut self,offset:u32,value:u32){
            let a=BASE+offset;let ok=if BootPmu::contains(a){self.pmu.write(a,value)}else if C5BootBias::contains(a){self.bias.write_awake_config(a,value)}else{self.analog.write(a,value)};
            assert!(ok,"{a:#x}={value:#x}");
        }
        fn rmw(&mut self,offset:u32,mask:u32,value:u32){let old=self.r(offset);self.w(offset,(old&!mask)|(value&mask));}
        fn hp(&mut self,mode:u32){
            // Original app rodata, fixed typed functions4212059c..421206ce.
            // Three source parameter tables; unused regulator INIT/RO fields
            // are deliberately not applied by pmu_hp_system_init.
            let power=[[0,0x70000000,0x80000000],[0x20000000,0x70000000,0x80000000],[0x08200000,0x1c000000,0]];
            let clock=[[u32::MAX,u32::MAX,0x80000000,0x08000000],[0x00100000,0x200,0x40000000,0xb8000000],[0,0,0,0x30000000]];
            let digital=[0,0x31000000,0x39000000];let analog=[[0x02000000,0xe00478d8,0],[0,0xe0040000,0],[0,0x08040000,0]];
            let backup=[0x010200a0,0x00100010,0x21100200];let n=mode as usize;let b=mode*0x34;
            self.w(b,power[n][0]);self.w(b+0x14,power[n][1]);self.rmw(b+0x30,1<<31,power[n][2]);
            self.w(b+4,clock[n][0]);self.w(b+8,clock[n][1]);self.rmw(b+0xc,3<<30,clock[n][2]);
            for mask in [1<<26,1<<27,1<<28,1<<29,3<<30]{self.rmw(b+0x24,mask,clock[n][3]);}
            for bit in 24..=29{self.rmw(b+0x10,1<<bit,digital[n]);}
            for mask in [1<<25,15<<26,1<<30,1<<31]{self.rmw(b+0x18,mask,analog[n][0]);}
            for mask in [1<<16,1<<17,1<<18,15<<19,15<<23,31<<27]{self.rmw(b+0x28,mask,analog[n][1]);}
            self.rmw(b+0x2c,0xffffff00,analog[n][2]);self.w(b+0x1c,backup[n]);self.w(b+0x20,u32::MAX);
            self.w(0xdc,1<<31);self.w(0xd0,1<<28);self.rmw(0x130,3<<16,2<<16);
        }
        fn lp(&mut self,mode:u32){
            let b=0x9c+mode*0x18;self.w(b+0xc,0);self.w(b+0x10,if mode==0{0x50000000}else{0});self.rmw(0xbc,1<<31,0);
            let bias=if mode==0{0}else{0xc0000000};for mask in [1<<25,15<<26,1<<30,1<<31]{self.rmw(0xc8,mask,bias);}
            let regulator=if mode==0{0xe0400000}else{0x60400000};for mask in [1<<21,1<<22,15<<23,31<<27]{self.rmw(b,mask,regulator);}
            self.rmw(b+4,15<<28,0);
        }
        fn force_default(&mut self){for offset in [0xf8,0xfc,0x100,0x108,0x10c]{for bit in [2,3,4,5,1,0]{self.rmw(offset,1<<bit,0);}}self.rmw(0x110,15<<24,0);}
        fn init(&mut self){for mode in 0..3{self.hp(mode);}for mode in 0..2{self.lp(mode);}self.force_default();}
    }
    #[test]
    fn complete_named_por_and_no_owner_aliases_are_retained(){
        let p=BootPmu::new();assert_eq!(SPECS.len(),51);for s in SPECS{assert_eq!(p.read(BASE+u32::from(s.offset)),Some(s.por));}
        for(off,por)in[(0,0),(4,u32::MAX),(8,u32::MAX),(0x30,0x80000000),(0x90,0xc6678000),(0xb4,0xc6600000),(0xc4,0x40000000),(0xf8,0x1c),(0x110,0xff000000)]{assert_eq!(p.read(BASE+off),Some(por));}
        for off in [0xc,0x28,0x5c,0x9c,0xac,0xcc,0xd0,0xdc]{assert!(!BootPmu::contains(BASE+off));}
    }
    #[test]
    fn actual_typed_app_sequence_configures_all_banks_without_inventing_a_mode_transition(){
        let mut w=World::new();w.init();
        for(off,value)in[(0,0),(4,u32::MAX),(8,u32::MAX),(0x10,0),(0x14,0x70000000),(0x18,0x02000000),(0x1c,0x010200a0),(0x20,u32::MAX),(0x24,0x08000000),(0x30,0x80000000),
            (0x34,0x20000000),(0x38,0x00100000),(0x3c,0x200),(0x40,0x40000000),(0x44,0x31000000),(0x58,0xb8000000),
            (0x68,0x08200000),(0x78,0x39000000),(0x7c,0x1c000000),(0x84,0x21100200),(0x8c,0x30000000),(0x90,0x08048000),(0x98,0),
            (0xa8,0),(0xb4,0x60400000),(0xbc,0),(0xc0,0),(0xc4,0),(0xc8,0xc0000000),(0x130,0x20000)]{assert_eq!(w.r(off),value,"+{off:#x}");}
        assert_eq!(w.r(0x28),0xe004f9c0);assert_eq!(w.r(0x5c),0xe0048000);assert_eq!(w.r(0x9c),0xe0400000);assert_eq!(w.r(0xac),0x50000000);
        assert_eq!((w.bias.active_hp_target(),w.bias.active_lp_target()),(Some(28),Some(28)));assert_eq!(w.r(0x28)&8,0);
        assert!(w.analog.report().contains("permissions0x7 code=2 updates=4/4"));assert!(!w.analog.pll_ready());assert!(w.analog.rc_fast_powered());
        for off in [0xf8,0xfc,0x100,0x108,0x10c]{assert_eq!(w.r(off),0);}assert_eq!(w.r(0x110),0xf0000000);
        assert!(w.pmu.report().contains("future_MODEM_SLEEP=unexecuted transitions=unsupported"));
    }
    #[test]
    fn active_power_clock_stall_and_analog_changes_are_never_ignored(){
        let mut w=World::new();w.init();for(off,value)in[(0,1<<21),(0,1<<22),(0,1<<23),(0,1<<27),(0,1<<28),(0,1<<29),(0,1<<30),(0,1<<31),
            (4,u32::MAX-1),(8,0),(0x10,1<<24),(0x10,1<<25),(0x10,1<<26),(0x10,1<<27),(0x10,1<<28),(0x10,1<<29),
            (0x14,0),(0x14,0x74000000),(0x14,0x78000000),(0x18,0),(0x18,0x06000000),(0x18,0x42000000),(0x18,0x82000000),
            (0x24,0),(0x24,0x18000000),(0x24,0xc8000000),(0x2c,0x100),(0x30,0),(0xa0,1<<28),(0xa8,1<<31)]{
            let before=w.pmu.clone();assert!(!w.pmu.write(BASE+off,value),"+{off:#x}={value:#x}");assert_eq!(w.pmu,before);}
    }
    #[test]
    fn safe_active_enables_do_not_allow_later_power_down_or_generate_ready(){
        let mut p=BootPmu::new();for(off,value)in[(0x14,0x70000000),(0x18,0x02000000),(0x24,0x08000000)]{
            assert!(p.write(BASE+off,0));assert!(p.write(BASE+off,value));let before=p.clone();assert!(!p.write(BASE+off,0));assert_eq!(p,before);assert!(p.write(BASE+off,value));}
        assert!(p.report().contains("voltage_ready=unmodeled"));
    }
    #[test]
    fn future_modes_store_only_named_configuration_without_touching_active_state(){
        let mut p=BootPmu::new();let active=p.read(BASE);
        for off in [0x34,0x68]{assert!(p.write(BASE+off,0xffe00000));assert_eq!(p.read(BASE+off),Some(0xffe00000));}
        for(off,value)in[(0x44,0x3f000000),(0x78,0x3f000000),(0x48,0x7c000000),(0x7c,0x7c000000),(0x90,0xffff8000),(0xb4,0),(0xc0,0xc0000000)]{assert!(p.write(BASE+off,value));}
        assert_eq!(p.read(BASE),active);assert!(p.report().contains("future_MODEM_SLEEP=unexecuted"));
    }
    #[test]
    fn backup_and_retention_enables_stay_unsupported_even_for_future_banks(){
        let mut p=BootPmu::new();for(off,bits)in[(0x1c,vec![11,12,29,30]),(0x50,vec![11,29]),(0x84,vec![12,13,30,31])]{
            for bit in bits{let before=p.clone();assert!(!p.write(BASE+off,1<<bit));assert_eq!(p,before);}}
        assert!(p.write(BASE+0x1c,0x010200a0));assert!(p.write(BASE+0x50,0x00100010));assert!(p.write(BASE+0x84,0x21100200));
    }
    #[test]
    fn force_controls_only_deassert_por_overrides_and_cannot_assert_reset_or_isolation(){
        let mut p=BootPmu::new();for off in [0xf8,0xfc,0x100,0x108,0x10c]{
            let mut old=0x1c;for bit in [2,3,4,5,1,0]{old&=!(1<<bit);assert!(p.write(BASE+off,old));assert_eq!(p.read(BASE+off),Some(old));}
            for value in [1,2,4,8,16,32,0x40,0xf8000000]{let before=p.clone();assert!(!p.write(BASE+off,value));assert_eq!(p,before);}}
        assert!(p.write(BASE+0x110,0xf5000000));assert!(p.write(BASE+0x110,0xf0000000));
        for value in [0xf0000001,0xf0000010,0xff000000,0xe0000000]{let before=p.clone();assert!(!p.write(BASE+0x110,value));assert_eq!(p,before);}
    }
    #[test]
    fn reserved_bits_sleep_requests_and_unimplemented_immediate_commands_do_not_alias(){
        let mut p=BootPmu::new();for address in [0,BASE+1,BASE+0xa4,BASE+0xb0,BASE+0xd4,BASE+0xd8,BASE+0xe0,BASE+0xe4,BASE+0xe8,
            BASE+0x104,BASE+0x114,BASE+0x124,BASE+0x128,BASE+0x134,BASE+0x154,BASE+0x180,BASE+0x184,BASE+0x188,u32::MAX]{
            let before=p.clone();assert!(!BootPmu::contains(address));assert_eq!(p.read(address),None);assert!(!p.write(address,u32::MAX));assert_eq!(p,before);}
        for(off,value)in[(0x34,1),(0x40,1),(0x90,1),(0xc8,1),(0x130,1),(0x130,1<<18)]{let before=p.clone();assert!(!p.write(BASE+off,value));assert_eq!(p,before);}
        assert!(p.write(BASE+0x130,2<<16));assert_eq!(p.read(BASE+0x130),Some(2<<16));
    }
    #[test]
    fn cpu_core_retention_and_explicit_lp_chip_reset_restore_complete_defaults(){
        let mut w=World::new();w.init();let before=w.pmu.clone();w.pmu.reset_domains(false,false);assert_eq!(w.pmu,before);
        w.pmu.reset_domains(true,false);assert_eq!(w.pmu,BootPmu::new());w.pmu.write(BASE+0x14,0x70000000);w.pmu.reset_domains(false,true);assert_eq!(w.pmu,BootPmu::new());
    }
}
