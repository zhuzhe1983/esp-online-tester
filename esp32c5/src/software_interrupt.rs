//! C5 FROM_CPU0 software interrupt, fixed IDF5.5.3/5.5.4 and TRM1.1 11.7.2.
//! HP INTPRI+90 bit0 is a R/W level latch, POR0, matrix source23. It is
//! distinct from CLINT MSIP/source3. No host yield, callback, CPU-line seed
//! or context switch occurs here; actual guest matrix/CLIC routing is external.
//! CPU_CLK/HP APB4 remain at the existing clock contract. INTPRI+a4 clock
//! controls and FROM_CPU1..3 remain unsupported, not generic register RAM.
//! Receipt: docs/evidence/c5-software-interrupt-2026-11-25.
const REGISTER:u32=0x600c_5090;const SOURCE:usize=23;
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct SoftwareInterrupt{pending:bool,writes:[u64;2],edges:[u64;2]}
impl SoftwareInterrupt{
    pub fn new()->Self{Self{pending:false,writes:[0;2],edges:[0;2]}}
    pub fn contains(address:u32)->bool{address==REGISTER}
    pub fn read(&self,address:u32)->Option<u32>{Self::contains(address).then_some(u32::from(self.pending))}
    pub fn write(&mut self,address:u32,value:u32)->bool{
        if !Self::contains(address)||value>1{return false;}
        let pending=value!=0;let i=value as usize;self.writes[i]=self.writes[i].saturating_add(1);
        if pending!=self.pending{self.edges[i]=self.edges[i].saturating_add(1);}self.pending=pending;true
    }
    /// Continuous source level. Reads/polls and CLIC observations do not
    /// acknowledge or consume it; the real guest handler must write zero.
    pub fn irq(&self)->bool{self.pending}
    /// Declared functional retention, not a per-flop silicon guarantee:
    /// CPU/LP retain this HP peripheral latch; HP/Chip restore POR0.
    /// Parent withdraws the optional owner on raw/SRAM deselection.
    pub fn reset_domains(&mut self,hp:bool,analog:bool){if hp||analog{*self=Self::new();}}
    pub fn report(&self)->String{format!("SW_IRQ=FROM_CPU0 source={SOURCE} level={} writes_clear/set={}/{} edges_clear/set={}/{} routing=external_guest_matrix_CLIC clock=unchanged_CPU_CLK_HP_APB4 other_words=unsupported",
        u8::from(self.irq()),self.writes[0],self.writes[1],self.edges[0],self.edges[1])}
}

#[cfg(test)]
mod tests{
    use super::SoftwareInterrupt;
    const REG:u32=0x600c_5090;
    fn put(s:&mut SoftwareInterrupt,value:u32){assert!(s.write(REG,value));}
    fn reject(s:&mut SoftwareInterrupt,address:u32,value:u32){let before=s.clone();assert!(!s.write(address,value));assert_eq!(*s,before);}
    #[test]
    fn fixed_c5_por_and_source_level_are_explicit(){
        let s=SoftwareInterrupt::new();assert!(SoftwareInterrupt::contains(REG));assert_eq!(s.read(REG),Some(0));assert!(!s.irq());
        assert!(s.report().contains("FROM_CPU0 source=23 level=0"));assert_eq!(s.writes,[0;2]);assert_eq!(s.edges,[0;2]);
    }
    #[test]
    fn original_yield_store_poll_and_handler_clear_use_the_real_latch(){
        let mut s=SoftwareInterrupt::new();
        // Static original image: 4080c154 ->40802bf2/2b92;40802be8
        // stores1,4080c174 conditionally polls,40802b48 handler stores0.
        put(&mut s,1);assert_eq!(s.read(REG),Some(1));assert!(s.irq());let asserted=s.clone();
        for _ in 0..32{assert_eq!(s.read(REG),Some(1));assert!(s.irq());s.report();}assert_eq!(s,asserted);
        put(&mut s,0);assert_eq!(s.read(REG),Some(0));assert!(!s.irq());
        assert_eq!(s.writes,[1,1]);assert_eq!(s.edges,[1,1]);
    }
    #[test]
    fn idempotent_writes_do_not_queue_edges_and_reassertion_remains_possible(){
        let mut s=SoftwareInterrupt::new();put(&mut s,0);put(&mut s,0);assert_eq!(s.edges,[0;2]);
        put(&mut s,1);put(&mut s,1);assert!(s.irq());assert_eq!(s.edges,[0,1]);
        put(&mut s,0);put(&mut s,0);assert!(!s.irq());assert_eq!(s.edges,[1,1]);
        put(&mut s,1);assert!(s.irq());assert_eq!(s.writes,[4,3]);assert_eq!(s.edges,[1,2]);
    }
    #[test]
    fn reserved_requests_reject_atomically_while_clear_or_asserted(){
        for initial in[0,1]{let mut s=SoftwareInterrupt::new();put(&mut s,initial);
            for bit in 1..32{reject(&mut s,REG,1<<bit);reject(&mut s,REG,(1<<bit)|1);}
            reject(&mut s,REG,u32::MAX);assert_eq!(s.read(REG),Some(initial));assert_eq!(s.irq(),initial!=0);
        }
    }
    #[test]
    fn exact_word_has_no_from_cpu_clock_date_matrix_clic_or_clint_alias(){
        let mut s=SoftwareInterrupt::new();put(&mut s,1);
        for address in 0x600c_5000..=0x600c_5fff{let expected=address==REG;assert_eq!(SoftwareInterrupt::contains(address),expected);
            if !expected{assert_eq!(s.read(address),None);reject(&mut s,address,0);reject(&mut s,address,1);}
        }
        for address in[0,u32::MAX,0x2000_0000,0x2080_100c,0x6001_005c,0x6009_6098,0x600c_5094,0x600c_5098,0x600c_509c,0x600c_50a0,0x600c_50a4]{
            assert!(!SoftwareInterrupt::contains(address));assert_eq!(s.read(address),None);reject(&mut s,address,0);reject(&mut s,address,1);
        }assert!(s.irq());
    }
    #[test]
    fn declared_cpu_lp_retention_and_hp_chip_reset_do_not_emit_edges(){
        let mut s=SoftwareInterrupt::new();put(&mut s,1);let before=s.clone();
        // Both CPU-only and LP-only dispatch have hp=false/analog=false.
        s.reset_domains(false,false);assert_eq!(s,before);s.reset_domains(false,false);assert_eq!(s,before);
        for(hp,analog)in[(true,false),(false,true),(true,true)]{
            s=before.clone();s.reset_domains(hp,analog);assert_eq!(s,SoftwareInterrupt::new());assert!(!s.irq());assert_eq!(s.edges,[0;2]);
            s.reset_domains(hp,analog);assert_eq!(s,SoftwareInterrupt::new());
        }
    }
    #[test]
    fn pending_level_is_independent_of_observation_count(){
        let mut s=SoftwareInterrupt::new();put(&mut s,1);let before=s.clone();
        for _ in 0..1000{assert!(s.irq());assert_eq!(s.read(REG),Some(1));s.report();}assert_eq!(s,before);
        put(&mut s,0);let cleared=s.clone();for _ in 0..1000{assert!(!s.irq());assert_eq!(s.read(REG),Some(0));}assert_eq!(s,cleared);
    }
    #[test]
    fn diagnostic_counters_saturate_and_guest_values_cannot_panic(){
        let mut s=SoftwareInterrupt::new();s.writes=[u64::MAX;2];s.edges=[u64::MAX;2];put(&mut s,1);put(&mut s,0);
        assert_eq!(s.writes,[u64::MAX;2]);assert_eq!(s.edges,[u64::MAX;2]);reject(&mut s,u32::MAX,u32::MAX);reject(&mut s,REG,u32::MAX);
    }
}
