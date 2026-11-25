#[cfg(test)]mod tg0_lp_races{
    use crate::{lp_wdt::LpWdt,timg_wdt::{ClockInputs,TimgWdt}};
    const INPUT:ClockInputs=ClockInputs{xtal_hz:Some(48_000_000),apb_clocked:[true;2],in_reset:[false;2]};
    fn tg0()->TimgWdt{
        let mut w=TimgWdt::new();w.enable_cold_spi_boot(0,INPUT).unwrap();
        for(address,value)in[(0x6000_8048,0x0044_8000),(0x6000_804c,1<<16),(0x6000_8050,2),(0x6000_8054,4),
            (0x6000_8048,0x385f_8000),(0x6000_8070,4),(0x6000_8060,1),(0x6000_8048,0xb81f_8000)]{
            assert!(w.write_at(address,value,0,INPUT));
        }w
    }
    fn lp(at:u32,action:u32)->LpWdt{
        let mut w=LpWdt::new();assert!(w.enable_virtual_rwdt(0,0));assert!(w.set_slow_clock(0,Some(240_000_000)));w.set_cpu_reset_enabled(true);
        assert!(w.write(0x18,0x50d8_3aa1));assert!(w.write(4,at/2));assert!(w.write(0,1<<31|action<<28|1<<11));w
    }
    fn first(lp:&LpWdt,tg:&TimgWdt)->(u8,u64){
        let mut l=lp.clone();let mut t=tg.clone();l.pump(u64::MAX);assert!(t.pump(u64::MAX,INPUT));
        let a=l.take_reset_event().map(|(c,at)|(c.reason(),at));let b=t.take_reset_event();
        let rank=|reason:u8|[0x12,0x10,9,7,8,0x0d,0x0b,0x11].iter().position(|r|*r==reason).unwrap();
        [a,b].into_iter().flatten().min_by_key(|&(cause,at)|(at,rank(cause))).unwrap()
    }
    #[test]fn earlier_lp_core_truncates_tg0_after_irq_and_before_core7(){
        let mut l=lp(20,3);let mut t=tg0();assert_eq!(first(&l,&t),(9,20));l.pump(20);t.pump(20,INPUT);
        assert_eq!(t.take_reset_event(),None);assert_eq!(t.sources(),[(58,true),(60,false)]);assert_eq!(t.deadline(),Some(30));
        assert!(t.report().contains("stage=1 count=2 phase=0"));assert_eq!(l.take_reset_event().map(|(c,at)|(c.reason(),at)),Some((9,20)));
        t.reset_domains(20,true,false,false);t.pump(100,INPUT);assert_eq!(t.take_reset_event(),None);assert_eq!(t.sources(),[(58,false),(60,false)]);
    }
    #[test]fn core7_ties_have_trm_priority_and_keep_a_later_lp_event(){
        let t=tg0();assert_eq!(first(&lp(30,2),&t),(7,30));assert_eq!(first(&lp(30,3),&t),(9,30));
        let mut later=lp(40,3);let mut t=t;assert_eq!(first(&later,&t),(7,30));later.pump(30);t.pump(30,INPUT);
        assert_eq!(t.take_reset_event(),Some((7,30)));assert_eq!(later.take_reset_event(),None);assert_eq!(later.deadline,Some(40));
        t.reset_domains(30,true,false,false);later.reset_domains(30,false,false);later.pump(100);
        assert_eq!(later.take_reset_event().map(|(c,at)|(c.reason(),at)),Some((9,40)));
    }
}
