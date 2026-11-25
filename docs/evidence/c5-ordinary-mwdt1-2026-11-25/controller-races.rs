// Actual controllers; caller arbitration below is a contract fixture, not Bus code.
#[cfg(test)]mod controller_races{
    use crate::{lp_wdt::LpWdt,timg_wdt::{ClockInputs,TimgWdt}};
    const INPUT:ClockInputs=ClockInputs{xtal_hz:Some(48_000_000),apb_clocked:[true;2],in_reset:[false;2]};
    fn tg()->TimgWdt{
        let mut w=TimgWdt::new();w.enable_cold_spi_boot(0,INPUT).unwrap();
        for(address,value)in[(0x6000_8048,0x0044_8000),(0x6000_9048,0x0004_8000),(0x6000_904c,0x5dc0_0000),
            (0x6000_9050,600),(0x6000_9054,1200),(0x6000_9048,0x385f_8000),(0x6000_9070,4),(0x6000_9060,1),(0x6000_9048,0xb81f_8000)]{
            assert!(w.write_at(address,value,0,INPUT));
        }w
    }
    fn rwdt(at:u32,action:u32)->LpWdt{
        let mut w=LpWdt::new();assert!(w.enable_virtual_rwdt(0,0));assert!(w.set_slow_clock(0,Some(240_000_000)));
        w.set_cpu_reset_enabled(true);assert!(w.write(0x18,0x50d8_3aa1));assert!(w.write(4,at/2));
        assert!(w.write(0,1<<31|action<<28|1<<11));w
    }
    fn first(lp:&LpWdt,tg:&TimgWdt,at:u64)->(u8,u64){
        let mut l=lp.clone();let mut t=tg.clone();l.pump(at);assert!(t.pump(at,INPUT));
        let a=l.take_reset_event().map(|(cause,at)|(cause.reason(),at));let b=t.take_reset_event();
        let rank=|reason:u8|[0x12,0x10,9,7,8,0x0d,0x0b,0x11].iter().position(|r|*r==reason).unwrap();
        [a,b].into_iter().flatten().min_by_key(|&(cause,at)|(at,rank(cause))).unwrap()
    }
    #[test]fn earlier_tg1_core_preserves_the_later_lp_analog_expiry(){
        let mut lp=LpWdt::new();lp.enable_virtual_swd(0);let mut t=tg();assert_eq!(first(&lp,&t,1_000_000_000),(8,216_000_000));
        lp.pump(216_000_000);t.pump(216_000_000,INPUT);assert_eq!(t.take_reset_event(),Some((8,216_000_000)));
        assert_eq!(lp.take_reset_event(),None);assert_eq!(lp.deadline,Some(228_000_000));
        lp.reset_domains(216_000_000,false,false);t.reset_domains(216_000_000,true,false,false);
        lp.pump(1_000_000_000);assert_eq!(lp.take_reset_event().map(|(c,at)|(c.reason(),at)),Some((0x12,228_000_000)));
        t.pump(1_000_000_000,INPUT);assert_eq!(t.take_reset_event(),None);
    }
    #[test]fn earlier_lp_core_truncates_tg1_before_its_stage1_timeout(){
        let mut lp=rwdt(210_000_000,3);let mut t=tg();assert_eq!(first(&lp,&t,u64::MAX),(9,210_000_000));
        lp.pump(210_000_000);t.pump(210_000_000,INPUT);assert_eq!(t.take_reset_event(),None);assert_eq!(t.deadline(),Some(216_000_000));
        assert!(t.report().contains("stage=1 count=1150 phase=0"));assert_eq!(t.source(),(60,true));
        assert_eq!(lp.take_reset_event().map(|(c,at)|(c.reason(),at)),Some((9,210_000_000)));
        t.reset_domains(210_000_000,true,false,false);t.pump(u64::MAX,INPUT);assert_eq!(t.take_reset_event(),None);assert_eq!(t.source(),(60,false));
    }
    #[test]fn same_time_core8_outranks_lp_cpu13_but_lp_core9_outranks_core8(){
        let t=tg();assert_eq!(first(&rwdt(216_000_000,2),&t,u64::MAX),(8,216_000_000));
        assert_eq!(first(&rwdt(216_000_000,3),&t,u64::MAX),(9,216_000_000));
        assert_eq!(first(&rwdt(216_000_000,4),&t,u64::MAX),(0x10,216_000_000));
    }
}
