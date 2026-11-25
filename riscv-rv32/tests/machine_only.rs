//! Explicit limited execution profile; these are not silicon privilege tests.
use emu_core::Core;
use riscv_rv32::{Cpu,FlatRam,Trap,step,state::{csr,mstatus}};
const BASE:u32=0x4038_0000;
fn fixture(word:u32,strict:bool)->(Cpu,FlatRam){
    let mut cpu=Cpu::new_rv32imac();cpu.machine_only=strict;cpu.pc=BASE;cpu.mtvec=BASE+0x100;cpu.mepc=BASE+32;
    let mut ram=FlatRam::new(BASE,512);ram.mem[..4].copy_from_slice(&word.to_le_bytes());(cpu,ram)
}
#[test]fn lower_privilege_mret_stops_before_return_in_both_execution_paths(){
    for mpp in 0..3{
        for fast in[false,true]{
            let(mut cpu,mut ram)=fixture(0x3020_0073,true);cpu.mstatus=(mpp<<11)|mstatus::MPIE;
            if fast{let(_,trap)=cpu.run(&mut ram,1);assert_eq!(trap,Some(Trap::Exception(2)));}
            else{assert_eq!(step(&mut cpu,&mut ram),Err(Trap::Exception(2)));}
            assert_eq!(cpu.mepc,BASE);assert_eq!(cpu.mtval,0x3020_0073);assert_eq!(cpu.pc,BASE+0x100);
        }
        let(mut legacy,mut ram)=fixture(0x3020_0073,false);legacy.mstatus=(mpp<<11)|mstatus::MPIE;
        step(&mut legacy,&mut ram).unwrap();assert_eq!(legacy.pc,BASE+32);
    }
}
#[test]fn changing_saved_mpp_is_not_a_privilege_transition(){
    let(mut cpu,mut ram)=fixture(0x3020_0073,true);cpu.mstatus=mstatus::MPP|mstatus::MPIE;
    ram.mem[32..36].copy_from_slice(&0x3000_1073u32.to_le_bytes()); // csrw mstatus,zero
    ram.mem[36..40].copy_from_slice(&0x0015_0513u32.to_le_bytes());
    step(&mut cpu,&mut ram).unwrap();assert_eq!(cpu.mstatus&mstatus::MPP,mstatus::MPP);
    step(&mut cpu,&mut ram).unwrap();assert_eq!(cpu.mstatus&mstatus::MPP,0);assert!(cpu.machine_only);
    step(&mut cpu,&mut ram).unwrap();assert_eq!(cpu.x[10],1);assert_eq!(cpu.pc,BASE+40);
    cpu.reset();assert!(cpu.machine_only);assert_eq!(cpu.mstatus&mstatus::MPP,mstatus::MPP);assert_eq!(cpu.mstatus&mstatus::MPRV,0);
}
#[test]fn guest_mprv_write_rejects_without_publishing_the_csr_or_destination(){
    for word in[0x3005_9573,0x3005_a573]{ // csrrw / csrrs a0,mstatus,a1
        let(mut cpu,mut ram)=fixture(word,true);cpu.x[11]=mstatus::MPRV;cpu.x[10]=0x55;
        assert_eq!(step(&mut cpu,&mut ram),Err(Trap::Exception(2)));assert_eq!(cpu.x[10],0x55);
        assert_eq!(cpu.mstatus&mstatus::MPRV,0);assert_eq!(cpu.mtval,word);assert_eq!(cpu.mepc,BASE);
        let(mut legacy,mut ram)=fixture(word,false);legacy.x[11]=mstatus::MPRV;step(&mut legacy,&mut ram).unwrap();
        assert_ne!(legacy.mstatus&mstatus::MPRV,0);
    }
}
#[test]fn csr_reads_and_clearing_mprv_do_not_enable_an_unsupported_mode(){
    let(mut cpu,mut ram)=fixture(0x3000_2573,true);let old=cpu.mstatus; // csrr a0,mstatus
    step(&mut cpu,&mut ram).unwrap();assert_eq!(cpu.x[10],old);assert_eq!(cpu.mstatus,old);
    let(mut cpu,mut ram)=fixture(0x3005_b573,true); // csrrc a0,mstatus,a1
    cpu.write_csr(csr::MSTATUS,mstatus::MPP|mstatus::MPRV);cpu.x[11]=mstatus::MPRV;
    step(&mut cpu,&mut ram).unwrap();assert_eq!(cpu.mstatus&mstatus::MPRV,0);assert!(cpu.machine_only);
}
