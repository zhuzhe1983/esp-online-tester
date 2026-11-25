//! Exact C5 startup peripheral shutdown configuration, fixed IDF5.5.3/5.5.4.
//! Named-field virtual POR values are explicit; TRM1.1 differs in several
//! defaults/READY fields. No peripheral data, IRQ, timer or RF engine exists
//! here. Existing UART/I2C/LEDC/SPI2/GDMA/ADC/SHA/clock owners stay separate.
//! Receipt: docs/evidence/c5-peripheral-clock-disable-2026-11-25.
const PCR_WORDS:usize=30;const WORDS:usize=32;
const RMT_CONF:usize=0;const RMT_SCLK:usize=1;
const PAU:usize=19;const CLK_OUT:usize=29;const LP_UART:usize=30;const LP_GPIO:usize=31;
#[derive(Clone,Copy)]
struct Config{address:u32,por:u32,clear:u32,set:u32,zero_only:bool}
const fn off(address:u32,por:u32,clear:u32)->Config{Config{address,por,clear,set:0,zero_only:false}}
// Only these exact words belong to this module. RO/POR, mux/divider,
// reserved and every unqualified reset bit must retain their current value.
const CONFIG:[Config;WORDS]=[
    off(0x6009_603c,0,1),off(0x6009_6040,0x0010_1000,1<<22),
    off(0x6009_6058,1<<22,1<<22),off(0x6009_6064,1<<22,1<<22),
    off(0x6009_602c,0,1<<22),off(0x6009_6028,4,1),
    off(0x6009_6034,0,1<<22),off(0x6009_6030,4,1),
    off(0x6009_6074,12,1),off(0x6009_6078,0x2000,1<<22),off(0x6009_6080,0x2000,1<<22),
    off(0x6009_609c,4,1),off(0x6009_60a0,4,1),
    off(0x6009_60a4,4,1),off(0x6009_60a8,0x4000,1<<22),
    off(0x6009_60b0,0,1<<18),off(0x6009_60b4,0,1<<18),off(0x6009_60ac,4,1),
    off(0x6009_6090,0,1<<22),
    Config{address:0x6009_60f8,por:0,clear:1,set:2,zero_only:false},
    off(0x6009_60d4,4,1),off(0x6009_60cc,4,1),off(0x6009_60dc,4,1),
    off(0x6009_6038,5,1),off(0x6009_60fc,0,1),
    off(0x6009_616c,5,1),off(0x6009_6170,5,1),
    off(0x6009_60b8,0x1c,1),off(0x6009_60bc,0,1<<22),
    Config{address:0x6009_612c,por:0x7ff,clear:0x7ff,set:0,zero_only:true},
    off(0x600b_1488,3<<24,3<<24),
    Config{address:0x600b_47f8,por:1,clear:1,set:0,zero_only:true},
];
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct BootPeripheralClock{words:[u32;WORDS],writes:[u64;2]}
impl BootPeripheralClock{
    pub fn new()->Self{Self{words:CONFIG.map(|c|c.por),writes:[0;2]}}
    fn index(address:u32)->Option<usize>{CONFIG.iter().position(|c|c.address==address)}
    pub fn contains(address:u32)->bool{Self::index(address).is_some()}
    pub fn read(&self,address:u32)->Option<u32>{Some(self.words[Self::index(address)?])}
    pub fn write(&mut self,address:u32,value:u32)->bool{
        let Some(i)=Self::index(address)else{return false;};let c=CONFIG[i];let old=self.words[i];
        if c.zero_only&&value!=0{return false;}
        if (value^old)&!(c.clear|c.set)!=0||value&c.clear&!old!=0||old&c.set&!value!=0{return false;}
        self.words[i]=value;let domain=usize::from(i>=PCR_WORDS);self.writes[domain]=self.writes[domain].saturating_add(1);true
    }
    /// Gate bits only, not a frequency/ready result. Every configuration
    /// accepted by this shutdown subset keeps RMT closed; the parent must
    /// actually block strict-profile RMT MMIO/tick/IRQ/deadline observations.
    pub fn rmt_clocked(&self)->bool{self.words[RMT_CONF]&3==1&&self.words[RMT_SCLK]&(1<<22)!=0}
    /// Declared virtual functional retention, not a per-flop silicon claim:
    /// CPU retains; HP restores PCR words; LP restores LP_UART/LP_GPIO;
    /// Chip/analog restores all. Raw deselection belongs to the parent.
    pub fn reset_domains(&mut self,hp:bool,lp:bool,analog:bool){
        if analog{*self=Self::new();return;}
        if hp{for(word,c)in self.words[..PCR_WORDS].iter_mut().zip(CONFIG){*word=c.por;}self.writes[0]=0;}
        if lp{for(word,c)in self.words[PCR_WORDS..].iter_mut().zip(&CONFIG[PCR_WORDS..]){*word=c.por;}self.writes[1]=0;}
    }
    pub fn report(&self)->String{format!("BOOT_PERIPH_CLK=disable_only virtual_POR=fixed_IDF553_554 pcr_words=30 writes={}/{} RMT={:#x}/{:#x} rmt_clocked={} PAU={:#x} held_reset={} CLK_OUT={:#x} LP_UART={:#x} LP_GPIO={:#x} READY=header_POR_only reset_completion_data_IRQ_RF=unsupported",
        self.writes[0],self.writes[1],self.words[RMT_CONF],self.words[RMT_SCLK],u8::from(self.rmt_clocked()),self.words[PAU],
        u8::from(self.words[PAU]&2!=0),self.words[CLK_OUT],self.words[LP_UART],self.words[LP_GPIO])}
}

#[cfg(test)]
mod tests{
    use super::BootPeripheralClock;
    // Independent typed-image fixture, limited to this module's words.
    // tuple: absolute address, actual CLR/SET mask, expected stored word.
    const STORES:[(u32,u32,u32);34]=[
        (0x6009_603c,1,0),(0x6009_6040,1<<22,0x0010_1000),
        (0x6009_6058,1<<22,0),(0x6009_6064,1<<22,0),
        (0x6009_602c,1<<22,0),(0x6009_6028,1,4),(0x6009_6034,1<<22,0),(0x6009_6030,1,4),
        (0x6009_6074,1,12),(0x6009_6078,1<<22,0x2000),(0x6009_6080,1<<22,0x2000),
        (0x6009_609c,1,4),(0x6009_60a0,1,4),(0x6009_60a4,1,4),(0x6009_60a8,1<<22,0x4000),
        (0x6009_60b0,1<<18,0),(0x6009_60b4,1<<18,0),(0x6009_60ac,1,4),(0x6009_6090,1<<22,0),
        (0x6009_60f8,1,0),(0x6009_60f8,2,2),(0x6009_60d4,1,4),(0x6009_60cc,1,4),(0x6009_60dc,1,4),
        (0x6009_6038,1,4),(0x6009_60fc,1,0),(0x6009_616c,1,4),(0x6009_6170,1,4),
        (0x6009_60b8,1,0x1c),(0x6009_60bc,1<<22,0),(0x6009_612c,0,0),
        (0x600b_1488,1<<24,2<<24),(0x600b_1488,1<<25,0),(0x600b_47f8,1,0),
    ];
    const POR:[(u32,u32);32]=[
        (0x6009_603c,0),(0x6009_6040,0x0010_1000),(0x6009_6058,1<<22),(0x6009_6064,1<<22),
        (0x6009_602c,0),(0x6009_6028,4),(0x6009_6034,0),(0x6009_6030,4),
        (0x6009_6074,12),(0x6009_6078,0x2000),(0x6009_6080,0x2000),
        (0x6009_609c,4),(0x6009_60a0,4),(0x6009_60a4,4),(0x6009_60a8,0x4000),
        (0x6009_60b0,0),(0x6009_60b4,0),(0x6009_60ac,4),(0x6009_6090,0),(0x6009_60f8,0),
        (0x6009_60d4,4),(0x6009_60cc,4),(0x6009_60dc,4),(0x6009_6038,5),(0x6009_60fc,0),
        (0x6009_616c,5),(0x6009_6170,5),(0x6009_60b8,0x1c),(0x6009_60bc,0),(0x6009_612c,0x7ff),
        (0x600b_1488,3<<24),(0x600b_47f8,1),
    ];
    fn put(c:&mut BootPeripheralClock,address:u32,value:u32){assert!(c.write(address,value),"write {address:#x}={value:#x}");}
    fn initialize(c:&mut BootPeripheralClock){for(address,mask,expected)in STORES{
        let old=c.read(address).unwrap();let value=if address==0x6009_60f8&&mask==2{old|mask}else if address==0x6009_612c{0}else{old&!mask};
        assert_eq!(value,expected,"typed RMW {address:#x}");put(c,address,value);
    }}
    fn reject(c:&mut BootPeripheralClock,address:u32,value:u32){let before=c.clone();assert!(!c.write(address,value),"unexpected support {address:#x}={value:#x}");assert_eq!(*c,before);}
    #[test]
    fn fixed_sdk_named_por_and_complete_original_shutdown_sequence(){
        let mut c=BootPeripheralClock::new();for(address,por)in POR{assert!(BootPeripheralClock::contains(address));assert_eq!(c.read(address),Some(por));}
        assert!(!c.rmt_clocked());initialize(&mut c);for(address,_,expected)in STORES.iter().rev(){
            // Earlier LP TX and PAU stores intentionally have later updates.
            if (*address,*expected)==(0x600b_1488,2<<24)||(*address,*expected)==(0x6009_60f8,0){continue;}
            assert_eq!(c.read(*address),Some(*expected));
        }
        assert_eq!(c.writes,[31,3]);assert!(!c.rmt_clocked());assert!(c.report().contains("writes=31/3"));
        assert!(c.report().contains("held_reset=1 CLK_OUT=0x0 LP_UART=0x0 LP_GPIO=0x0"));
    }
    #[test]
    fn exact_decoder_has_no_existing_owner_or_neighbor_aliases(){
        let mut c=BootPeripheralClock::new();
        for address in(0x6009_6000..=0x6009_6fff).chain(0x600b_1400..=0x600b_14ff).chain(0x600b_4400..=0x600b_47ff){
            let expected=POR.iter().any(|&(a,_)|a==address);assert_eq!(BootPeripheralClock::contains(address),expected,"decode {address:#x}");
            if !expected{assert_eq!(c.read(address),None);reject(&mut c,address,0);}
        }
        for address in[0,u32::MAX,0x6000_6000,0x6000_6040,0x6000_8000,0x6000_8048,0x6008_0064,
            0x6009_6054,0x6009_605c,0x6009_6060,0x6009_6068,0x6009_6088,0x6009_60d0,0x6009_60e4,0x6009_60e8,
            0x6009_6100,0x6009_6104,0x6009_6128,0x6009_6130,0x6009_6160,0x6009_6174,
            0x600b_0400,0x600b_0404,0x600b_2800,0x600b_0c10,0x6009_97f8,0x600b_34f8]{
            assert!(!BootPeripheralClock::contains(address));assert_eq!(c.read(address),None);reject(&mut c,address,0);
        }
    }
    #[test]
    fn undeclared_bits_mux_divider_reset_and_ro_changes_reject_atomically(){
        let mut c=BootPeripheralClock::new();initialize(&mut c);
        // Every single-bit change is unsupported after closure except the
        // already asserted PAU reset, which also may not be released.
        for(address,_)in POR{let old=c.read(address).unwrap();for bit in 0..32{reject(&mut c,address,old^(1<<bit));}}
        for(address,_)in POR{reject(&mut c,address,u32::MAX);}
    }
    #[test]
    fn ordinary_close_is_idempotent_and_cannot_reenable(){
        let mut c=BootPeripheralClock::new();for(address,por)in POR{
            if matches!(address,0x6009_612c|0x600b_47f8){reject(&mut c,address,por);}else{put(&mut c,address,por);}
        }
        initialize(&mut c);for(address,por)in POR{let closed=c.read(address).unwrap();put(&mut c,address,closed);if por!=closed{reject(&mut c,address,por);}}
    }
    #[test]
    fn pau_disable_then_held_reset_has_no_release_or_retention_engine(){
        let mut c=BootPeripheralClock::new();let address=0x6009_60f8;
        assert_eq!(c.read(address),Some(0));put(&mut c,address,0);reject(&mut c,address,1);reject(&mut c,address,3);
        put(&mut c,address,2);put(&mut c,address,2);assert_eq!(c.read(address),Some(2));reject(&mut c,address,0);
        reject(&mut c,address,6);assert!(c.report().contains("PAU=0x2 held_reset=1"));
        for engine in[0x600a_d000,0x600a_d004]{assert_eq!(c.read(engine),None);reject(&mut c,engine,1);}
    }
    #[test]
    fn lp_uart_can_close_either_bit_first_but_not_reopen_or_reset(){
        for first in[24,25]{let mut c=BootPeripheralClock::new();let address=0x600b_1488;let value=(3<<24)&!(1<<first);
            put(&mut c,address,value);put(&mut c,address,value);assert_eq!(c.read(address),Some(value));
            reject(&mut c,address,3<<24);reject(&mut c,address,value|(1<<26));reject(&mut c,address,value|(1<<27));
            put(&mut c,address,0);for bad in[1,1<<24,1<<25,1<<26,1<<27,u32::MAX]{reject(&mut c,address,bad);}
        }
        let mut c=BootPeripheralClock::new();put(&mut c,0x600b_1488,0);assert_eq!(c.read(0x600b_1488),Some(0));
    }
    #[test]
    fn lp_gpio_closes_and_reports_actual_latch_readback(){
        let mut c=BootPeripheralClock::new();let address=0x600b_47f8;assert_eq!(c.read(address),Some(1));
        for bad in[1,2,3,u32::MAX]{reject(&mut c,address,bad);}put(&mut c,address,0);put(&mut c,address,0);
        assert_eq!(c.read(address),Some(0));reject(&mut c,address,1);
        // The other half of rtc_io_ll's polling loop remains LpPeri's word.
        assert_eq!(c.read(0x600b_2800),None);assert!(c.report().contains("LP_GPIO=0x0"));
    }
    #[test]
    fn clk_out_accepts_only_the_original_zero_and_cannot_change_other_blocks(){
        let mut c=BootPeripheralClock::new();let address=0x6009_612c;assert_eq!(c.read(address),Some(0x7ff));
        for bad in[1,0x3ff,0x7ff,0x800,u32::MAX]{reject(&mut c,address,bad);}let before=c.clone();put(&mut c,address,0);
        for(a,_)in POR{if a!=address{assert_eq!(c.read(a),before.read(a));}}put(&mut c,address,0);
        for outside in[0x6009_6128,0x6009_6110,0x6009_6018,0x600b_0400,0x600b_0404,0x600b_0c10]{assert_eq!(c.read(outside),None);reject(&mut c,outside,0);}
    }
    #[test]
    fn header_ro_por_is_retained_without_a_reset_ready_transition(){
        let mut c=BootPeripheralClock::new();for address in[0x6009_6028,0x6009_6030,0x6009_609c,0x6009_60cc]{
            assert_eq!(c.read(address),Some(4));for bad in[0,1,2,3,5,6,7]{reject(&mut c,address,bad);}put(&mut c,address,4);
        }
        // Fixed SDK omits RMT_READY despite TRM1.1's different layout/POR.
        assert_eq!(c.read(0x6009_603c),Some(0));reject(&mut c,0x6009_603c,4);reject(&mut c,0x6009_603c,2);
        assert_eq!(c.read(0x6009_60b8),Some(0x1c));reject(&mut c,0x6009_60b8,0);
        let before=c.clone();for _ in 0..20{for(address,_)in POR{c.read(address);}c.report();assert!(!c.rmt_clocked());}assert_eq!(c,before);
        assert!(c.report().contains("READY=header_POR_only reset_completion_data_IRQ_RF=unsupported"));
    }
    #[test]
    fn rmt_qualifier_uses_both_real_gate_bits_and_reset(){
        let mut c=BootPeripheralClock::new();assert!(!c.rmt_clocked());
        reject(&mut c,0x6009_603c,1);reject(&mut c,0x6009_6040,0x0050_1000);
        // Decoder-only fixture for the predicate, not a publicly accepted
        // RMT source/enable transaction or a claimed RMT execution engine.
        c.words[0]=1;assert!(!c.rmt_clocked());c.words[1]|=1<<22;assert!(c.rmt_clocked());
        c.words[0]|=2;assert!(!c.rmt_clocked());c.words[0]=0;assert!(!c.rmt_clocked());
    }
    #[test]
    fn declared_cpu_hp_lp_and_chip_resets_preserve_the_other_domain(){
        let mut c=BootPeripheralClock::new();initialize(&mut c);let initialized=c.clone();c.reset_domains(false,false,false);assert_eq!(c,initialized);
        c.reset_domains(true,false,false);for(address,por)in POR{assert_eq!(c.read(address),if address<0x600b_0000{Some(por)}else{initialized.read(address)});}
        assert_eq!(c.writes,[0,3]);assert!(!c.rmt_clocked());c=BootPeripheralClock::new();initialize(&mut c);c.reset_domains(false,true,false);
        for(address,por)in POR{assert_eq!(c.read(address),if address>=0x600b_0000{Some(por)}else{initialized.read(address)});}
        assert_eq!(c.writes,[31,0]);c.reset_domains(true,true,false);assert_eq!(c,BootPeripheralClock::new());
        initialize(&mut c);c.reset_domains(false,false,true);assert_eq!(c,BootPeripheralClock::new());
    }
    #[test]
    fn guest_diagnostics_saturate_without_wrapping_or_panicking(){
        let mut c=BootPeripheralClock::new();c.writes=[u64::MAX;2];put(&mut c,0x6009_603c,0);put(&mut c,0x600b_1488,0);
        assert_eq!(c.writes,[u64::MAX;2]);assert_eq!(c.read(u32::MAX),None);reject(&mut c,u32::MAX,u32::MAX);
    }
}
