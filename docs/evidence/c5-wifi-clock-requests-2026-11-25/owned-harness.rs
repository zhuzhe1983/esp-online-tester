//! Actual owned modules only; no Bus/Soc or guest execution.
#[path="../../../esp32c5/src/boot_analog.rs"]mod boot_analog;
#[path="../../../esp32c5/src/modem_wifi_clock.rs"]mod modem_wifi_clock;
#[cfg(test)]
mod integration{
    use super::{boot_analog::C5BootAnalog,modem_wifi_clock::ModemWifiClock};
    #[test]
    fn requests_use_actual_applied_modem_apb_and_never_reverse_grant_it(){
        let mut a=C5BootAnalog::virtual_usb3v3();let mut c=ModemWifiClock::new();let addr=0x600a_9c14;
        let before=c.clone();assert_eq!(c.read(addr,a.modem_apb_accessible()),None);assert!(!c.write(addr,0,a.modem_apb_accessible()));assert_eq!(c,before);
        for(address,value)in[(0x600b_000c,0x8000_0000),(0x600a_9c0c,0x6464_6400),(0x600a_f020,0x6666_0000),(0x600b_00dc,0x8000_0000),(0x600b_00d0,0x1000_0000)]{assert!(a.write(address,value));}
        assert!(a.modem_apb_accessible());let analog=a.clone();
        for(mask,expected)in[(0x400,0x400),(4,0x404),(0x200,0x604),(0x1fb,0x7ff)]{
            let old=c.read(addr,a.modem_apb_accessible()).unwrap();assert!(c.write(addr,old|mask,a.modem_apb_accessible()));assert_eq!(c.read(addr,true),Some(expected));
        }
        assert_eq!(a,analog);assert!(!a.pll_ready());assert_eq!(a.deadline(),None);assert_eq!(a.bod_threshold(),None);assert!(!a.sar_reserved_path_ready());
        assert!(a.write(0x600a_9c0c,0));assert!(a.modem_apb_accessible());assert!(a.write(0x600b_00d0,0x1000_0000));assert!(!a.modem_apb_accessible());
        let before=c.clone();assert_eq!(c.read(addr,a.modem_apb_accessible()),None);assert!(!c.write(addr,0,a.modem_apb_accessible()));assert_eq!(c,before);
        assert_eq!(c.requested_bits(),0x7ff);assert!(!a.modem_apb_accessible());
        assert!(a.write(0x600a_9c0c,0x6464_6400));assert!(a.write(0x600b_00d0,0x1000_0000));assert_eq!(c.read(addr,a.modem_apb_accessible()),Some(0x7ff));
    }
}
