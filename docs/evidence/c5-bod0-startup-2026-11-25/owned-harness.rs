//! Standalone owned modules only; no Bus/Soc, Cargo or guest ROM execution.
#[path="../../../esp32c5/src/boot_analog.rs"]mod boot_analog;
#[path="../../../esp32c5/src/boot_brownout.rs"]mod boot_brownout;
#[cfg(test)]
mod integration{
    use super::{boot_analog::C5BootAnalog,boot_brownout::BootBrownout};
    const BASE:u32=0x600b_2c00;const MASTER:u32=0x600a_f800;
    #[test]
    fn source_sequence_uses_real_target_rmw_before_detector_enable(){
        let mut a=C5BootAnalog::virtual_usb3v3();let mut b=BootBrownout::new();
        a.enable_virtual_bod_threshold();b.enable_ideal_usb3v3();assert!(b.observe_threshold(a.bod_threshold()));
        for (clear,set) in[(0x3ff<<8,2<<8),(0,1<<6),(0,1<<7),(0,1<<28),(1<<28,0),(0,0x3ff<<18),(1<<31,0),(0,1<<30)]{
            let old=b.read(BASE).unwrap();assert!(b.write(BASE,(old&!clear)|set));
        }
        let before=b.clone();assert!(!b.write(BASE,b.read(BASE).unwrap()|1<<29));assert_eq!(b,before);
        assert!(a.write(0x600a_f018,4));assert!(a.write(MASTER+0x1c,0x00ff_feff));
        assert!(a.write(MASTER+4,0x0561));let word=a.read(MASTER+4).unwrap();assert_eq!((word>>16)&255,0);
        let data=((word>>16)&255)&!7|7;assert!(a.write(MASTER+4,0x0100_0561|(data<<16)));
        assert_eq!(a.bod_threshold(),Some(7));assert!(b.observe_threshold(a.bod_threshold()));
        assert!(b.write(BASE,b.read(BASE).unwrap()|1<<29));assert_eq!(b.read(BASE),Some(0x6ffc_02c0));
        assert!(b.write(BASE+0x1c,1<<31));assert!(b.write(BASE+0x18,1<<31));
        assert_eq!(b.read(BASE+0x10),Some(0));assert_eq!(b.read(BASE+0x14),Some(0));assert!(!b.irq());
        assert!(a.report().contains("regi2c_reads=1 regi2c_writes=1"));
        // CPU/Core retain both domains. System clears only the digital
        // detector, and Chip requires a new actual analog threshold write.
        a.reset_domains(true,false,false);b.reset_domains(true,false,false);assert_eq!(b.read(BASE),Some(0x6ffc_02c0));
        a.reset_domains(true,true,false);b.reset_domains(true,true,false);assert_eq!(b.read(BASE),Some(0x0ffc_0100));assert_eq!(a.bod_threshold(),Some(7));
        a.reset_domains(true,true,true);b.reset_domains(true,true,true);assert!(b.observe_threshold(a.bod_threshold()));assert_eq!(a.bod_threshold(),None);
        assert!(!b.write(BASE,b.read(BASE).unwrap()|1<<29));
        a.disable_virtual_bod_threshold();b.disable_ideal_profile();assert_eq!(a.bod_threshold(),None);assert_eq!(b.read(BASE),None);assert!(!b.irq());
    }
}
