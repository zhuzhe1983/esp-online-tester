//! Checked instruction access, including CSRRW's architecturally omitted read.
use riscv_rv32::{Cpu,FlatRam,Trap,step,c5_memory_protection::UnsupportedReason};
const BASE:u32=0x4038_0000;
fn csrw(n:u32,rs:u32,rd:u32)->u32{(n<<20)|(rs<<15)|(1<<12)|(rd<<7)|0x73}
fn fixture(words:&[u32])->(Cpu,FlatRam){
    let mut cpu=Cpu::new_rv32imac();cpu.machine_only=true;cpu.enable_c5_memory_protection().unwrap();cpu.pc=BASE;cpu.mtvec=BASE+0x100;
    let mut ram=FlatRam::new(BASE,512);for(i,word)in words.iter().enumerate(){ram.mem[i*4..i*4+4].copy_from_slice(&word.to_le_bytes());}(cpu,ram)
}
#[test]fn csrrw_zero_skips_an_unqualified_old_address_read_but_a_requested_read_stops_the_model(){
    for rd in[0,10]{
        let(mut cpu,mut ram)=fixture(&[csrw(0xbc0,11,0),csrw(0xbd0,12,0),csrw(0xbd0,13,rd)]);
        cpu.x[11]=0;cpu.x[12]=0x1020_01ff;cpu.x[13]=0x1020_05ff;cpu.x[10]=0x55;
        step(&mut cpu,&mut ram).unwrap();step(&mut cpu,&mut ram).unwrap();
        assert!(matches!(cpu.try_read_csr(0xbd0),Err(UnsupportedReason::AddressReadback(_))));
        let result=step(&mut cpu,&mut ram);
        if rd==0{result.unwrap();assert_eq!(cpu.c5_memory_protection().unwrap().snapshot().pma_addr()[0],0x1020_05ff);}
        else{assert_eq!(result,Err(Trap::ModelLimit(BASE+8)));assert_eq!(cpu.c5_memory_protection().unwrap().snapshot().pma_addr()[0],0x1020_01ff);assert_eq!(cpu.x[10],0x55);assert_eq!((cpu.pc,cpu.mepc,cpu.mcause,cpu.mtval),(BASE+8,0,0,0));assert!(matches!(cpu.c5_memory_unsupported(),Some(UnsupportedReason::AddressReadback(_))));}
    }
}
#[test]fn locked_csr_writes_stay_ignored_until_real_core_reset(){
    let(mut cpu,mut ram)=fixture(&[csrw(0xbc0,11,0),csrw(0xbd0,12,0),csrw(0xbc0,13,0),csrw(0xbd0,14,0)]);
    cpu.x[11]=0;cpu.x[12]=0x1020_01ff;cpu.x[13]=0xe000_001d;cpu.x[14]=u32::MAX;
    for _ in 0..3{step(&mut cpu,&mut ram).unwrap();}let before=cpu.c5_memory_protection().unwrap().snapshot();
    step(&mut cpu,&mut ram).unwrap();assert_eq!(cpu.c5_memory_protection().unwrap().snapshot(),before);
    cpu.reset();let after=cpu.c5_memory_protection().unwrap().snapshot();assert_ne!(after.epoch(),before.epoch());assert_eq!(after.pma_cfg()[0],0x8000_0000);assert_eq!(after.pma_addr()[0],0);
}
#[test]fn a_default_cpu_keeps_its_previous_unmodeled_csr_storage(){
    let mut cpu=Cpu::new_rv32imac();assert!(cpu.c5_memory_protection().is_none());
    cpu.try_write_csr(0xbc0,u32::MAX).unwrap();assert_eq!(cpu.try_read_csr(0xbc0),Ok(u32::MAX));
    assert!(cpu.enable_c5_memory_protection().is_err());assert!(cpu.c5_memory_protection().is_none());
}
#[test]fn an_unsupported_policy_write_does_not_become_a_guest_illegal_instruction(){
    let(mut cpu,mut ram)=fixture(&[csrw(0x3a0,11,10)]);cpu.x[11]=0x40;cpu.x[10]=0x55;
    let before=cpu.c5_memory_protection().unwrap().snapshot();let status=cpu.mstatus;
    assert_eq!(step(&mut cpu,&mut ram),Err(Trap::ModelLimit(BASE)));
    assert_eq!(cpu.c5_memory_protection().unwrap().snapshot(),before);assert_eq!(cpu.x[10],0x55);
    assert_eq!((cpu.pc,cpu.mepc,cpu.mcause,cpu.mtval,cpu.mstatus),(BASE,0,0,0,status));
    assert_eq!(cpu.c5_memory_unsupported(),Some(UnsupportedReason::PmpXl(0)));
}
