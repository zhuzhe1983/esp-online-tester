//! C5 Serial/JTAG CDC TX FIFO with no attached USB host.
//! IDF5.5.3/5.5.4 C5 usb_serial_jtag_reg.h +0/+4: 64-byte FIFO, DATA_FREE,
//! WR_DONE. A sealed packet stays occupied until a host reads it; this profile
//! has no host transport, SOF/event producer, PHY or JTAG engine. No UART alias.
//! The separately qualified monitor subset is +8 RAW, +c ST, +10 ENA, +14 CLR.
//! Named RAW POR is 8 (IN_EMPTY), not 0; SOF bit1 is 0 without a host. C5 TRM1.1
//! pp1740-1741 defines SS, SC and WTC separately; RAW3 is R/WTC/SS, not SC.
//! TX/WR_DONE changes FIFO occupancy, not this sticky history. CLR3/RAW writes,
//! host/reset/line events remain outside this subset; no unqualified clear occurs.
//! Fixed sources, clock/reset prerequisites and static PCs: docs/evidence/
//! c5-usb-sticky-raw-2026-11-25/. No PHY-ready or guest-continuation claim.
pub(crate) struct DetachedUsb{tx:Vec<u8>,sealed:bool,monitor_raw:u32,monitor_ena:u32}
impl DetachedUsb{
    pub fn new()->Self{Self{tx:Vec::new(),sealed:false,monitor_raw:8,monitor_ena:0}}
    pub fn read(&self,off:u32)->Option<u32>{match off{
        0=>Some(0), // no OUT packet exists without a host
        4=>Some(u32::from(!self.sealed&&self.tx.len()<64)<<1),_=>None,
    }}
    pub fn write(&mut self,off:u32,value:u32)->bool{match off{
        0=>{if self.sealed||self.tx.len()>=64{return false;}self.tx.push(value as u8);},
        4=>{if value&1!=0{self.sealed=true;}},_=>return false,
    }true}
    // Caller proves actual APB/PLL48 availability and an unchanged cold USB PCR
    // CLK_EN=1/RST_EN=0 profile. This boolean neither models PHY readiness nor
    // requests a reset. Unknown/withdrawn clock or reset must pass false.
    pub fn read_monitor(&self,off:u32,clock_qualified:bool)->Option<u32>{
        if !clock_qualified{return None;}
        match off{0x08=>Some(self.monitor_raw),0x0c=>Some(self.monitor_raw&self.monitor_ena),0x10=>Some(self.monitor_ena),_=>None}
    }
    pub fn write_monitor(&mut self,off:u32,value:u32,clock_qualified:bool)->bool{
        if !clock_qualified||value&!2!=0{return false;}
        // CLR only clears SOF, which has no producer here. In particular it
        // cannot clear RAW3 or consume TX data. Other raw-bit causes reject.
        match off{0x10=>self.monitor_ena=value,0x14=>self.monitor_raw&=!value,_=>return false}true
    }
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn detached_fifo_reports_actual_capacity_and_never_fabricates_host_consumption(){
        let mut u=DetachedUsb::new();assert_eq!(u.read(4),Some(2));assert_eq!(u.read(0),Some(0));
        for byte in 0..64{assert!(u.write(0,byte));}assert_eq!(u.read(4),Some(0));
        assert!(!u.write(0,64));assert_eq!(u.tx,(0..64).collect::<Vec<u8>>());
        assert!(u.write(4,1));assert_eq!(u.read(4),Some(0));assert_eq!(u.tx.len(),64);
        assert!(u.write(4,0));assert_eq!(u.read(4),Some(0));assert_eq!(u.read(8),None);
    }
    #[test]fn write_done_seals_short_packet_and_zero_is_not_a_command(){
        let mut u=DetachedUsb::new();assert!(u.write(0,u32::from(b'E')));assert!(u.write(4,0));
        assert_eq!(u.read(4),Some(2));assert!(u.write(4,1));assert_eq!(u.read(4),Some(0));
        assert!(!u.write(0,u32::from(b'S')));assert_eq!(u.tx,b"E");assert!(!u.write(8,0));
    }
    #[test]fn monitor_named_por_keeps_in_empty_separate_from_absent_sof(){
        let u=DetachedUsb::new();assert_eq!(u.read_monitor(0x08,true),Some(8));
        assert_eq!(u.read_monitor(0x08,true).unwrap()&2,0);
        assert_eq!(u.read_monitor(0x0c,true),Some(0));assert_eq!(u.read_monitor(0x10,true),Some(0));
        assert_eq!(u.read_monitor(0x14,true),None);assert_eq!(u.read(8),None);
    }
    #[test]fn repeated_static_tick_register_pair_never_clears_raw3_or_creates_host_events(){
        let mut u=DetachedUsb::new();
        // Register component replay of 40801c8e/40801c98, not guest/tick execution.
        for clear in [2,2,0,2]{assert_eq!(u.read_monitor(8,true),Some(8));assert!(u.write_monitor(0x14,clear,true));}
        assert_eq!(u.read_monitor(8,true),Some(8));assert_eq!(u.read(4),Some(2));
        assert!(u.tx.is_empty());assert!(!u.sealed);assert_eq!(u.read_monitor(0x0c,true),Some(0));
    }
    #[test]fn only_sof_interrupt_enable_is_qualified_and_masked_status_stays_zero(){
        let mut u=DetachedUsb::new();
        for enable in [2,2,0,2]{assert!(u.write_monitor(0x10,enable,true));assert_eq!(u.read_monitor(0x10,true),Some(enable));
            assert_eq!(u.read_monitor(0x0c,true),Some(0));assert_eq!(u.read_monitor(8,true),Some(8));}
        for unsupported in [1,3,4,8,0xffff,0x7ffff,u32::MAX]{assert!(!u.write_monitor(0x10,unsupported,true));
            assert_eq!(u.read_monitor(0x10,true),Some(2));assert_eq!(u.read_monitor(8,true),Some(8));}
        assert!(u.write_monitor(0x10,0,true));assert_eq!(u.read_monitor(0x0c,true),Some(0));
    }
    #[test]fn tx_data_keeps_sticky_raw3_separate_from_fifo_occupancy(){
        let mut u=DetachedUsb::new();assert!(u.write(4,0));assert_eq!(u.read_monitor(8,true),Some(8));
        assert!(u.write(0,0x145));assert_eq!(u.tx,b"E");assert_eq!(u.read(4),Some(2));
        assert_eq!(u.read_monitor(8,true),Some(8));assert!(u.write_monitor(0x10,2,true));
        for clear in [0,2,2]{assert!(u.write_monitor(0x14,clear,true));assert_eq!(u.read_monitor(8,true),Some(8));}
        assert_eq!(u.read_monitor(0x0c,true),Some(0));assert_eq!(u.read_monitor(0x10,true),Some(2));
        assert_eq!(u.tx,b"E");assert!(!u.sealed);
    }
    #[test]fn write_done_keeps_sticky_raw3_and_preserves_sealed_packet(){
        let mut u=DetachedUsb::new();assert!(u.write(4,1));assert_eq!(u.read_monitor(8,true),Some(8));
        assert!(u.write_monitor(0x14,2,true));assert!(u.write_monitor(0x10,2,true));
        assert_eq!(u.read_monitor(8,true),Some(8));
        assert_eq!(u.read_monitor(0x0c,true),Some(0));assert!(!u.write(0,0));
        assert!(u.write(4,0));assert_eq!(u.read(4),Some(0));assert!(u.tx.is_empty());assert!(u.sealed);
    }
    #[test]fn unknown_clock_rejects_all_monitor_access_without_mutation(){
        let mut u=DetachedUsb::new();assert!(u.write_monitor(0x10,2,true));
        for off in [0,4,8,0x0c,0x10,0x14,0x18,u32::MAX]{assert_eq!(u.read_monitor(off,false),None);
            for value in [0,2,u32::MAX]{assert!(!u.write_monitor(off,value,false));}}
        assert_eq!(u.read_monitor(0x10,true),Some(2));assert_eq!(u.read_monitor(8,true),Some(8));
        assert!(u.tx.is_empty());assert!(!u.sealed);
    }
    #[test]fn unsupported_raw_clear_masks_and_unaligned_offsets_are_transactional(){
        let mut u=DetachedUsb::new();assert!(u.write_monitor(0x10,2,true));
        for off in [0,4,8,0x0c,0x11,0x15,0x18,u32::MAX]{for value in [0,2,8,u32::MAX]{
            assert!(!u.write_monitor(off,value,true));}}
        for value in [1,3,4,8,0xffff,u32::MAX]{assert!(!u.write_monitor(0x14,value,true));}
        for off in [0,4,9,0x0d,0x11,0x14,0x15,0x18,u32::MAX]{assert_eq!(u.read_monitor(off,true),None);}
        assert_eq!(u.read_monitor(8,true),Some(8));assert_eq!(u.read_monitor(0x10,true),Some(2));
        assert_eq!(u.read(4),Some(2));assert!(u.tx.is_empty());assert!(!u.sealed);
    }
    #[test]fn clock_qualification_changes_do_not_reset_fifo_enable_or_sticky_raw(){
        let mut u=DetachedUsb::new();assert!(u.write_monitor(0x10,2,true));assert!(u.write(0,42));assert!(u.write(4,1));
        assert!(!u.write_monitor(0x10,0,false));assert!(!u.write_monitor(0x14,2,false));
        assert_eq!(u.read_monitor(8,false),None);assert_eq!(u.read_monitor(8,true),Some(8));
        assert_eq!(u.read_monitor(0x10,true),Some(2));assert_eq!(u.read_monitor(0x0c,true),Some(0));
        assert_eq!(u.tx,[42]);assert!(u.sealed);assert_eq!(u.read(4),Some(0));
    }
    #[test]fn rejected_full_fifo_raw_and_clear3_writes_do_not_erase_sticky_history(){
        let mut u=DetachedUsb::new();for byte in 0..64{assert!(u.write(0,byte));}
        assert_eq!(u.read(4),Some(0));assert_eq!(u.read_monitor(8,true),Some(8));
        assert!(!u.write(0,64));assert!(!u.write_monitor(8,8,true));
        assert!(!u.write_monitor(0x14,8,true));assert!(!u.write_monitor(0x10,8,true));
        assert!(u.write(4,1));assert!(u.write_monitor(0x14,2,true));
        assert_eq!(u.read_monitor(8,true),Some(8));assert_eq!(u.read_monitor(0x0c,true),Some(0));
        assert_eq!(u.read(4),Some(0));assert_eq!(u.tx,(0..64).collect::<Vec<u8>>());assert!(u.sealed);
    }
}
