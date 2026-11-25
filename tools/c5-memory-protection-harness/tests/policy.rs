use c5_memory_protection_harness::policy::*;
#[path = "fixtures/factory.rs"]
mod factory;
#[path = "fixtures/rom.rs"]
mod rom;
fn ctx() -> GuardedMachineContext { GuardedMachineContext::new(true,false).unwrap() }
fn a(address: u32, kind: AccessKind) -> Access { Access { address, width: 4, kind } }
fn write(p: &mut C5MemoryProtection, csr: u32, value: u32) { p.write_csr(csr,value,ctx()).unwrap(); }
fn clear_pma(p: &mut C5MemoryProtection) { for i in 0..16 { write(p,0xbc0+i,0); } }
fn pma(p: &mut C5MemoryProtection, index: u32, addr: u32, cfg: u32) {
    write(p,0xbc0+index,0); write(p,0xbd0+index,addr); write(p,0xbc0+index,cfg);
}
fn pmp(p: &mut C5MemoryProtection, index: u32, addr: u32, cfg: u32) {
    write(p,0x3b0+index,addr); let csr=0x3a0+index/4;
    let old=p.read_csr(csr,ctx()).unwrap(); write(p,csr,old | cfg << (index%4*8));
}
fn allowed(d: PolicyDecision, entry: Entry) { assert!(matches!(d,PolicyDecision::Allow { entry:e,.. } if e==entry),"{d:?}"); }
fn denied(d: PolicyDecision, entry: Entry, cause: AccessFault) { assert_eq!(d,PolicyDecision::Deny { entry,cause }); }

#[test]
fn context_requires_reliable_machine_only_and_no_mprv() {
    assert!(GuardedMachineContext::new(true,false).is_ok());
    for (m,p) in [(false,false),(false,true),(true,true)] { assert_eq!(GuardedMachineContext::new(m,p),Err(UnsupportedReason::Context)); }
}
#[test]
fn exactly_52_csrs_are_owned_and_unknown_requests_are_transactional() {
    assert_eq!((0..4096).filter(|&n| accepts_csr(n)).count(),52);
    let mut p=C5MemoryProtection::new(); let before=p.snapshot();
    for number in [0,0x300,0x3a4,0x3c0,0xbbf,0xbe0,u32::MAX] {
        assert!(!accepts_csr(number)); assert_eq!(p.read_csr(number,ctx()),Err(UnsupportedReason::Csr(number)));
        assert_eq!(p.write_csr(number,0,ctx()),Err(UnsupportedReason::Csr(number))); assert_eq!(p.snapshot(),before);
    }
}
#[test]
fn por_is_raw_a2_with_only_its_unqualified_window_blocked() {
    let mut p=C5MemoryProtection::new(); let s=p.snapshot();
    assert_eq!(s.version(),1); assert_eq!(*s.pma_cfg(),[0x80000000;16]); assert_eq!(*s.pma_addr(),[0;16]); assert_eq!(s.pma_por_mask(),0xffff);
    assert_eq!(*s.pmp_cfg(),[0;4]); assert_eq!(*s.pmp_addr(),[0;16]);
    assert_eq!(p.read_csr(0xbc0,ctx()),Ok(0x80000000)); assert_eq!(p.read_csr(0xbd0,ctx()),Ok(0));
    assert!(matches!(p.check_pma(a(124,AccessKind::Load),ctx()),PolicyDecision::Unsupported(UnsupportedReason::PorWindow(_))));
    assert_eq!(p.check_pma(a(128,AccessKind::Load),ctx()),PolicyDecision::NoMatch);
    assert_eq!(p.check_pma(a(0x40000000,AccessKind::Fetch),ctx()),PolicyDecision::NoMatch);
    assert_eq!(p.check_pmp(a(0x40800000,AccessKind::Store),ctx()),PolicyDecision::NoMatch);
    assert_eq!(p.write_csr(0xbc0,0x80000000,ctx()),Err(UnsupportedReason::Na4(Entry::Pma(0)))); assert_eq!(p.snapshot(),s);
}
#[test]
fn por_address_staging_moves_only_the_reset_a2_window_and_keeps_readback_unqualified() {
    let mut p=C5MemoryProtection::new();
    for i in 1..16 { write(&mut p,0xbc0+i,0); }
    for value in [1,0x140007ff,0x1000ffff,0x3fffffff] {
        let before=p.snapshot(); assert_eq!(p.write_csr(0xbd0,value,ctx()),Ok(WriteEffect::Changed));
        let s=p.snapshot(); let start=(value<<2)&!127;
        assert_eq!(s.pma_cfg()[0],0x80000000); assert_eq!(s.pma_por_mask(),1); assert_eq!(s.pma_addr()[0],value);
        assert_eq!(s.generation(),before.generation()+1); assert_eq!(s.epoch(),before.epoch());
        assert_eq!(p.read_csr(0xbd0,ctx()),Err(UnsupportedReason::AddressReadback(Entry::Pma(0))));
        assert_eq!(p.read_csr(0xbc0,ctx()),Ok(0x80000000));
        for access in [a(start,AccessKind::Load),a(start+124,AccessKind::Store),Access {address:start+127,width:1,kind:AccessKind::Load}] {
            assert_eq!(p.check_pma(access,ctx()),PolicyDecision::Unsupported(UnsupportedReason::PorWindow(0)));
            assert_eq!(s.check_pma(access,ctx()),p.check_pma(access,ctx()));
        }
        if start!=0 { assert_eq!(p.check_pma(a(start-4,AccessKind::Load),ctx()),PolicyDecision::NoMatch); }
        assert_eq!(p.check_pma(a(0x4003ab62,AccessKind::Fetch),ctx()),PolicyDecision::NoMatch);
        // The stored address does not change A2 into a NAPOT range.
        if value==0x140007ff { assert_eq!(p.check_pma(a(0x50000000,AccessKind::Load),ctx()),PolicyDecision::NoMatch); }
        if value==0x1000ffff { assert_eq!(p.check_pma(a(0x50001f80,AccessKind::Load),ctx()),PolicyDecision::NoMatch); }
        let staged=p.snapshot();
        assert_eq!(p.write_csr(0xbd0,value,ctx()),Ok(WriteEffect::Unchanged));
        assert_eq!(p.write_csr(0xbd0,0x40000000,ctx()),Err(UnsupportedReason::PhysicalAddress(0x40000000)));
        assert_eq!(p.write_csr(0xbc0,0x80000000,ctx()),Err(UnsupportedReason::Na4(Entry::Pma(0))));
        assert_eq!(p.snapshot(),staged);
    }
    write(&mut p,0xbd0,0); assert_eq!(p.read_csr(0xbd0,ctx()),Ok(0)); assert_eq!(p.snapshot().pma_por_mask(),1);
    write(&mut p,0xbc0,0); assert_eq!(p.snapshot().pma_por_mask(),0);
    assert_eq!(p.check_pma(a(0,AccessKind::Load),ctx()),PolicyDecision::NoMatch);
}

fn rom_pair(p: &mut C5MemoryProtection, pair: rom::Pair) {
    // Both writes are CSRRW rd=x0. Their parameter readback is requested only
    // after the explicit cfg write, including when the old cfg is POR A2.
    for (word,csr) in [(pair.address_word,0xbd0+pair.index),(pair.cfg_word,0xbc0+pair.index)] {
        assert_eq!(word&0x000fffff,((word>>15)&31)<<15|0x1073); assert_eq!(word>>20,csr);
    }
    assert!(pair.address_pc<pair.cfg_pc);
    write(p,0xbd0+pair.index,pair.address); write(p,0xbc0+pair.index,pair.cfg);
    assert_eq!(p.read_csr(0xbd0+pair.index,ctx()),Ok(pair.address));
    assert_eq!(p.read_csr(0xbc0+pair.index,ctx()),Ok(pair.cfg));
    assert_eq!(pair.cfg&0x20000000,0);
}

#[test]
fn original_rom_four_entry_staging_does_not_invent_napot_or_bypass_checks() {
    let mut p=C5MemoryProtection::new(); p.reset_core().unwrap();
    for pair in rom::INIT {
        let before=p.snapshot(); write(&mut p,0xbd0+pair.index,pair.address);
        let staged=p.snapshot(); assert_eq!(staged.pma_cfg()[pair.index as usize],0x80000000);
        assert_eq!(staged.pma_por_mask(),before.pma_por_mask());
        assert_eq!(p.read_csr(0xbd0+pair.index,ctx()),Err(UnsupportedReason::AddressReadback(Entry::Pma(pair.index as u8))));
        assert_eq!(p.check_pma(a((pair.address<<2)&!127,AccessKind::Load),ctx()),PolicyDecision::Unsupported(UnsupportedReason::PorWindow(pair.index as u8)));
        let fetch=p.check_pma(a(pair.address_pc+4,AccessKind::Fetch),ctx());
        if pair.index<=13 { assert_eq!(fetch,PolicyDecision::NoMatch); }
        else { allowed(fetch,Entry::Pma(13)); }
        // Synthetic instruction-PC query: an A3 NAPOT union would block
        // entry13's own next ROM fetch, although cfg.A remains2 until cfg_pc.
        assert_eq!(staged.check_pma(a(pair.address_pc+4,AccessKind::Fetch),ctx()),fetch);
        rom_pair(&mut p,pair); assert_eq!(p.generation(),before.generation()+2);
        assert_eq!(p.snapshot().pma_por_mask(),before.pma_por_mask()&!(1<<pair.index));
    }
    let s=p.snapshot(); assert_eq!(s.epoch(),1); assert_eq!(s.generation(),9); assert_eq!(s.pma_por_mask(),0x0fff);
    assert_eq!(*s.pmp_cfg(),[0;4]); assert_eq!(*s.pmp_addr(),[0;16]);
    for (address,index,read,store,fetch) in [(0x50000000,12,true,true,false),(0x4003ab62,13,true,false,true),
        (0x40800000,14,true,true,false),(0x42000000,15,true,false,false)] {
        for (kind,ok) in [(AccessKind::Load,read),(AccessKind::Store,store),(AccessKind::Fetch,fetch)] {
            let d=p.check_pma(a(address,kind),ctx());
            if ok { allowed(d,Entry::Pma(index)); } else { denied(d,Entry::Pma(index),kind_fault(kind)); }
        }
    }
    // Coarse ROM/SRAM descriptors grant no physical address-decoder support.
    allowed(p.check_pma(a(0x4007fffc,AccessKind::Load),ctx()),Entry::Pma(13));
    allowed(p.check_pma(a(0x4087fffc,AccessKind::Load),ctx()),Entry::Pma(14));
}
fn kind_fault(kind: AccessKind) -> AccessFault {
    match kind { AccessKind::Fetch=>AccessFault::Instruction,AccessKind::Load|AccessKind::AtomicRead=>AccessFault::Load,_=>AccessFault::Store }
}

#[test]
fn original_rom_boot_variants_are_unlocked_explicit_cfg_changes() {
    for pairs in [&rom::FLASH_BOOT[..],&rom::DIRECT_BOOT[..],&rom::RAM_BOOT[..],&rom::DEEP_SLEEP_X[..]] {
        let mut p=C5MemoryProtection::new(); for pair in rom::INIT { rom_pair(&mut p,pair); }
        for &pair in pairs { rom_pair(&mut p,pair); }
        let s=p.snapshot(); assert_eq!(s.pma_por_mask(),0x0fff);
        for (index,address) in [(12,0x50000000),(14,0x40800000),(15,0x42000000)] {
            let fetch=p.check_pma(a(address,AccessKind::Fetch),ctx());
            if s.pma_cfg()[index]&4!=0 { allowed(fetch,Entry::Pma(index as u8)); }
            else { denied(fetch,Entry::Pma(index as u8),AccessFault::Instruction); }
        }
    }
}

#[test]
fn original_app_can_replace_unlocked_rom_descriptors_but_rom_replay_cannot_unlock_app() {
    let mut p=C5MemoryProtection::new();
    for pair in rom::INIT.into_iter().chain(rom::FLASH_BOOT) { rom_pair(&mut p,pair); }
    assert_eq!(factory::pma(&mut p,ctx()),64); factory::pmp(&mut p,ctx(),8*1024*1024);
    let locked=p.snapshot(); assert_eq!(*locked.pma_cfg(),factory::PMA_CFG); assert_eq!(*locked.pma_addr(),factory::PMA_ADDR);
    for pair in rom::INIT.into_iter().chain(rom::FLASH_BOOT) {
        assert_eq!(p.write_csr(0xbd0+pair.index,pair.address,ctx()),Ok(WriteEffect::IgnoredLocked));
        assert_eq!(p.write_csr(0xbc0+pair.index,pair.cfg,ctx()),Ok(WriteEffect::IgnoredLocked));
    }
    assert_eq!(p.snapshot(),locked);
    denied(p.check_pmp(a(0x40818400,AccessKind::Fetch),ctx()),Entry::Pmp(5),AccessFault::Instruction);
    allowed(p.check_pma(a(0x420713ac,AccessKind::Fetch),ctx()),Entry::Pma(7));
    p.reset_core().unwrap(); assert_eq!(p.snapshot().pma_por_mask(),0xffff);
    assert_eq!(p.write_csr(0xbdc,rom::INIT[0].address,ctx()),Ok(WriteEffect::Changed));
    assert_eq!(p.read_csr(0xbdc,ctx()),Err(UnsupportedReason::AddressReadback(Entry::Pma(12))));
}
#[test]
fn type_zero_denies_even_with_rwx_and_off_means_no_match() {
    let mut p=C5MemoryProtection::new(); clear_pma(&mut p);
    pma(&mut p,0,0x40800000>>2,0); pma(&mut p,1,0x40801000>>2,0x4000001c);
    for kind in [AccessKind::Fetch,AccessKind::Load,AccessKind::Store,AccessKind::AtomicRead,AccessKind::AtomicWrite,AccessKind::AtomicRmw] {
        denied(p.check_pma(a(0x40800080,kind),ctx()),Entry::Pma(1),match kind {AccessKind::Fetch=>AccessFault::Instruction,AccessKind::Load|AccessKind::AtomicRead=>AccessFault::Load,_=>AccessFault::Store});
    }
    write(&mut p,0xbc1,0x4000001d); allowed(p.check_pma(a(0x40800080,AccessKind::AtomicRmw),ctx()),Entry::Pma(1));
    write(&mut p,0xbc1,0x1d); assert_eq!(p.check_pma(a(0x40800080,AccessKind::Load),ctx()),PolicyDecision::NoMatch);
}
#[test]
fn pma_atomic_read_write_and_rmw_have_distinct_permissions() {
    let mut p=C5MemoryProtection::new(); clear_pma(&mut p); pma(&mut p,0,0x40800000>>2,0); write(&mut p,0xbd1,0x40801000>>2);
    for (cfg,lr,sc,rmw) in [(0x40000011,true,false,false),(0x40000009,false,true,false),(0x40000019,true,true,true)] {
        write(&mut p,0xbc1,cfg);
        for (kind,ok) in [(AccessKind::AtomicRead,lr),(AccessKind::AtomicWrite,sc),(AccessKind::AtomicRmw,rmw)] {
            assert_eq!(matches!(p.check_pma(a(0x40800080,kind),ctx()),PolicyDecision::Allow {..}),ok);
        }
    }
}
#[test]
fn pma_locks_cfg_and_addr_until_core_reset_even_when_off() {
    let mut p=C5MemoryProtection::new(); clear_pma(&mut p); pma(&mut p,0,0x40800000>>2,0x20000001);
    let before=p.snapshot();
    for (csr,value) in [(0xbc0,0),(0xbc0,u32::MAX),(0xbd0,u32::MAX)] { assert_eq!(p.write_csr(csr,value,ctx()),Ok(WriteEffect::IgnoredLocked)); assert_eq!(p.snapshot(),before); }
    p.reset_core().unwrap(); assert_eq!(p.epoch(),1); assert_eq!(p.generation(),before.generation()+1);
    write(&mut p,0xbc0,0); write(&mut p,0xbd0,0x40801000>>2);
}
#[test]
fn pma_tor_previous_lock_is_explicitly_unqualified_when_not_self_locked() {
    let mut p=C5MemoryProtection::new(); clear_pma(&mut p); pma(&mut p,0,0x40800000>>2,0);
    pma(&mut p,1,0x40801000>>2,0x60000011); let before=p.snapshot();
    assert_eq!(p.write_csr(0xbd0,0x40800080>>2,ctx()),Err(UnsupportedReason::PmaTorPreviousLock(0))); assert_eq!(p.snapshot(),before);
}
#[test]
fn attributes_are_diagnostic_and_pma_overlaps_are_not_given_a_guessed_priority() {
    let mut p=C5MemoryProtection::new(); clear_pma(&mut p); pma(&mut p,0,0x40800000>>2,0); pma(&mut p,1,0x40801000>>2,0x4f00001d);
    assert_eq!(p.check_pma(a(0x40800080,AccessKind::Load),ctx()),PolicyDecision::Allow {entry:Entry::Pma(1),attributes:Some(CacheAttributes {noncacheable:true,write_through:true,write_miss_no_allocate:true,read_miss_no_allocate:true})});
    pma(&mut p,2,0x408007ff>>2,0xc000001d);
    assert_eq!(p.check_pma(a(0x40800080,AccessKind::Load),ctx()),PolicyDecision::Unsupported(UnsupportedReason::PmaOverlap));
    assert_eq!(p.check_pma(a(0x50000000,AccessKind::Load),ctx()),PolicyDecision::NoMatch);
}
#[test]
fn unaligned_boundaries_and_small_napot_only_affect_their_windows() {
    let mut p=C5MemoryProtection::new(); clear_pma(&mut p); pma(&mut p,0,0x40800000>>2,0); pma(&mut p,1,(0x40801000>>2)+1,0x40000011);
    allowed(p.check_pma(a(0x40800080,AccessKind::Load),ctx()),Entry::Pma(1));
    assert_eq!(p.check_pma(a(0x40801000,AccessKind::Load),ctx()),PolicyDecision::Unsupported(UnsupportedReason::AddressWindow(Entry::Pma(1))));
    assert_eq!(p.check_pma(a(0x50000000,AccessKind::Load),ctx()),PolicyDecision::NoMatch);
    assert_eq!(p.read_csr(0xbd1,ctx()),Err(UnsupportedReason::AddressReadback(Entry::Pma(1))));
    pma(&mut p,1,0x40802000>>2,0xc0000011);
    assert!(matches!(p.check_pma(a(0x40802000,AccessKind::Load),ctx()),PolicyDecision::Unsupported(_)));
    assert_eq!(p.check_pma(a(0x40802080,AccessKind::Load),ctx()),PolicyDecision::NoMatch);
}
#[test]
fn pmp_minimum_priority_whole_access_and_unlocked_machine_semantics() {
    let mut p=C5MemoryProtection::new(); pmp(&mut p,0,0x40800000>>2,0); pmp(&mut p,1,0x40800100>>2,0x89);
    denied(p.check_pmp(a(0x40800080,AccessKind::Store),ctx()),Entry::Pmp(1),AccessFault::Store);
    pmp(&mut p,2,0x408003ff>>2,0x9f);
    denied(p.check_pmp(a(0x40800080,AccessKind::Store),ctx()),Entry::Pmp(1),AccessFault::Store);
    denied(p.check_pmp(Access {address:0x408000fe,width:4,kind:AccessKind::Load},ctx()),Entry::Pmp(1),AccessFault::Load);
    let mut p=C5MemoryProtection::new(); pmp(&mut p,0,0x40800000>>2,0); pmp(&mut p,1,0x40800100>>2,8);
    allowed(p.check_pmp(a(0x40800080,AccessKind::Store),ctx()),Entry::Pmp(1));
    denied(p.check_pmp(Access {address:0x408000fe,width:4,kind:AccessKind::Load},ctx()),Entry::Pmp(1),AccessFault::Load);
}
#[test]
fn pmp_atomic_lr_sc_and_rmw_and_tor_predecessor_lock() {
    let mut p=C5MemoryProtection::new(); pmp(&mut p,0,0x40800000>>2,0); pmp(&mut p,1,0x40801000>>2,0x89);
    allowed(p.check_pmp(a(0x40800080,AccessKind::AtomicRead),ctx()),Entry::Pmp(1));
    for kind in [AccessKind::AtomicWrite,AccessKind::AtomicRmw] { denied(p.check_pmp(a(0x40800080,kind),ctx()),Entry::Pmp(1),AccessFault::Store); }
    let before=p.snapshot();
    for csr in [0x3b0,0x3b1] { assert_eq!(p.write_csr(csr,u32::MAX,ctx()),Ok(WriteEffect::IgnoredLocked)); assert_eq!(p.snapshot(),before); }
    p.reset_core().unwrap(); pmp(&mut p,0,0x40800000>>2,0); pmp(&mut p,1,0x40801000>>2,0x8b);
    allowed(p.check_pmp(a(0x40800080,AccessKind::AtomicRmw),ctx()),Entry::Pmp(1));
}
#[test]
fn pmp_unqualified_grain_is_local_and_does_not_override_a_lower_match() {
    let mut p=C5MemoryProtection::new(); write(&mut p,0x3b0,(0x40800000>>2)|15);
    assert_eq!(p.read_csr(0x3b0,ctx()),Err(UnsupportedReason::AddressReadback(Entry::Pmp(0))));
    write(&mut p,0x3a0,0x19); assert_eq!(p.read_csr(0x3b0,ctx()),Ok((0x40800000>>2)|15));
    allowed(p.check_pmp(a(0x40800000,AccessKind::Store),ctx()),Entry::Pmp(0));
    pmp(&mut p,1,0x40800000>>2,0x19);
    allowed(p.check_pmp(a(0x40800000,AccessKind::Store),ctx()),Entry::Pmp(0));
    assert_eq!(p.check_pmp(a(0x50000000,AccessKind::Load),ctx()),PolicyDecision::NoMatch);
    let mut p=C5MemoryProtection::new(); pmp(&mut p,0,0x40800000>>2,0x19);
    assert_eq!(p.check_pmp(a(0x40800000,AccessKind::Load),ctx()),PolicyDecision::Unsupported(UnsupportedReason::AddressWindow(Entry::Pmp(0))));
    assert_eq!(p.check_pmp(a(0x40800080,AccessKind::Load),ctx()),PolicyDecision::NoMatch);
}
#[test]
fn illegal_packed_write_is_atomic_and_locked_bytes_ignore_illegal_new_values_first() {
    let mut virgin=C5MemoryProtection::new(); let initial=virgin.snapshot();
    assert_eq!(virgin.write_csr(0x3a0,0x4009,ctx()),Err(UnsupportedReason::PmpXl(1))); assert_eq!(virgin.snapshot(),initial);
    let mut p=C5MemoryProtection::new(); pmp(&mut p,0,0x40800000>>2,0x80); let before=p.snapshot();
    assert_eq!(p.write_csr(0x3a0,0x000040ff,ctx()),Err(UnsupportedReason::PmpXl(1))); assert_eq!(p.snapshot(),before);
    assert_eq!(p.write_csr(0x3a0,0xff,ctx()),Ok(WriteEffect::IgnoredLocked)); assert_eq!(p.snapshot(),before);
    for (v,reason) in [(0x00002080,UnsupportedReason::ReservedBits(0x3a0)),(0x00001080,UnsupportedReason::Na4(Entry::Pmp(1))),(0x00000280,UnsupportedReason::PmpWriteWithoutRead(1))] {
        assert_eq!(p.write_csr(0x3a0,v,ctx()),Err(reason)); assert_eq!(p.snapshot(),before);
    }
}
#[test]
fn generation_changes_only_with_committed_policy_or_core_reset() {
    let mut p=C5MemoryProtection::new(); let initial=p.snapshot();
    assert_eq!(p.write_csr(0x3a0,0,ctx()),Ok(WriteEffect::Unchanged)); assert_eq!(p.snapshot(),initial);
    write(&mut p,0xbc0,0); let configured=p.snapshot();
    assert_eq!(configured.generation(),initial.generation()+1); assert_eq!(configured.epoch(),initial.epoch());
    assert_eq!(p.write_csr(0xbc0,0,ctx()),Ok(WriteEffect::Unchanged)); assert_eq!(p.snapshot(),configured);
    assert_eq!(p.read_csr(0xbc0,ctx()),Ok(0)); p.check_pma(a(0x40800000,AccessKind::Load),ctx()); assert_eq!(p.snapshot(),configured);
    p.reset_core().unwrap(); assert_eq!(p.generation(),configured.generation()+1); assert_eq!(p.epoch(),configured.epoch()+1);
}
#[test]
fn malformed_width_range_and_unknown_writes_do_not_panic_or_mutate() {
    let mut p=C5MemoryProtection::new(); let before=p.snapshot();
    for (csr,v) in [(0xbc0,2),(0xbd0,u32::MAX),(0x3b0,u32::MAX)] { assert!(p.write_csr(csr,v,ctx()).is_err()); assert_eq!(p.snapshot(),before); }
    for access in [Access {address:0,width:0,kind:AccessKind::Load},Access {address:0,width:8,kind:AccessKind::Fetch},Access {address:1,width:4,kind:AccessKind::AtomicRmw},Access {address:u32::MAX,width:4,kind:AccessKind::Store}] {
        assert!(matches!(p.check_pma(access,ctx()),PolicyDecision::Unsupported(_))); assert!(matches!(p.check_pmp(access,ctx()),PolicyDecision::Unsupported(_)));
    }
    assert_eq!(p.snapshot(),before);
}
#[test]
fn snapshot_and_live_owner_share_decisions_without_observing_later_mutation() {
    let mut p=C5MemoryProtection::new(); factory::pma(&mut p,ctx()); factory::pmp(&mut p,ctx(),8*1024*1024);
    let cached=p.snapshot(); assert_eq!(cached.epoch(),p.epoch()); assert_eq!(cached.generation(),p.generation());
    for address in [0,124,128,0x20000000,0x40000000,0x40049c00,0x40800080,0x40818400,0x40870000,0x420713ac,0x42310000,0x50000000,0x600990c4,0xffffff80] {
        for kind in [AccessKind::Fetch,AccessKind::Load,AccessKind::Store,AccessKind::AtomicRead,AccessKind::AtomicWrite,AccessKind::AtomicRmw] {
            let access=a(address,kind);
            assert_eq!(cached.check_pma(access,ctx()),p.check_pma(access,ctx()));
            assert_eq!(cached.check_pmp(access,ctx()),p.check_pmp(access,ctx()));
        }
    }
    p.reset_core().unwrap(); assert_ne!((cached.epoch(),cached.generation()),(p.epoch(),p.generation()));
    denied(cached.check_pmp(a(0x40800080,AccessKind::Store),ctx()),Entry::Pmp(4),AccessFault::Store);
    assert_eq!(p.check_pmp(a(0x40800080,AccessKind::Store),ctx()),PolicyDecision::NoMatch);
}
#[test]
fn minimum_napot_grain_and_four_gib_upper_bound_do_not_wrap() {
    let mut p=C5MemoryProtection::new(); clear_pma(&mut p); pma(&mut p,0,(0x40800000>>2)|15,0xc000001d);
    // Encoded NAPOT: use a physical word address, minimum128 bytes (four low1s).
    allowed(p.check_pma(a(0x40800000,AccessKind::Load),ctx()),Entry::Pma(0));
    assert_eq!(p.check_pma(a(0x40800080,AccessKind::Load),ctx()),PolicyDecision::NoMatch);
    assert_eq!(p.read_csr(0xbd0,ctx()),Ok((0x40800000>>2)|15));
    pma(&mut p,0,0x1fffffff,0xc000001d);
    allowed(p.check_pma(Access {address:u32::MAX,width:1,kind:AccessKind::Store},ctx()),Entry::Pma(0));
    assert_eq!(p.check_pma(a(u32::MAX,AccessKind::Store),ctx()),PolicyDecision::Unsupported(UnsupportedReason::AddressOverflow));
}
#[test]
fn original_64_pma_writes_and_all_16_pmp_entries_replay_and_constrain_accesses() {
    for heap in [0,8*1024*1024] {
        let mut p=C5MemoryProtection::new(); assert_eq!(factory::pma(&mut p,ctx()),64);
        assert_eq!(p.generation(),48); factory::pmp(&mut p,ctx(),heap); let s=p.snapshot();
        assert_eq!(*s.pma_cfg(),factory::PMA_CFG); assert_eq!(*s.pma_addr(),factory::PMA_ADDR); assert_eq!(s.pma_por_mask(),0);
        assert_eq!(*s.pmp_cfg(),[0x8089809f,0x8d808b8d,0x808b8980,0x9b8b8d8b]);
        assert_eq!(*s.pmp_addr(),[0x09ffffff,0x10012700,0x10014000,0x10200000,0x10206100,0x10218000,0x10800000,0x1084c000,0x1084c000,0x108c4000,(0x42310000+heap)>>2,0x14000000,0x14000000,0x14000000,0x14001000,0x1801ffff]);
        allowed(p.check_pma(a(0x420713ac,AccessKind::Fetch),ctx()),Entry::Pma(7));
        allowed(p.check_pmp(a(0x420713ac,AccessKind::Fetch),ctx()),Entry::Pmp(7));
        denied(p.check_pmp(a(0x40800080,AccessKind::Store),ctx()),Entry::Pmp(4),AccessFault::Store);
        denied(p.check_pmp(a(0x40818400,AccessKind::Fetch),ctx()),Entry::Pmp(5),AccessFault::Instruction);
        allowed(p.check_pmp(a(0x40818400,AccessKind::AtomicRmw),ctx()),Entry::Pmp(5));
        denied(p.check_pma(a(0x40000080,AccessKind::Store),ctx()),Entry::Pma(4),AccessFault::Store);
        denied(p.check_pmp(a(0x40049c00,AccessKind::Fetch),ctx()),Entry::Pmp(2),AccessFault::Instruction);
        allowed(p.check_pmp(a(0x40049c00,AccessKind::Load),ctx()),Entry::Pmp(2));
        denied(p.check_pmp(a(0x42130000,AccessKind::Store),ctx()),Entry::Pmp(9),AccessFault::Store);
        denied(p.check_pmp(a(0x42130000,AccessKind::Fetch),ctx()),Entry::Pmp(9),AccessFault::Instruction);
        denied(p.check_pma(a(0x40870000,AccessKind::Load),ctx()),Entry::Pma(6),AccessFault::Load);
        assert_eq!(p.check_pma(a(0x50000000,AccessKind::Load),ctx()),PolicyDecision::NoMatch);
        allowed(p.check_pmp(a(0x50000000,AccessKind::Store),ctx()),Entry::Pmp(14));
        denied(p.check_pmp(a(0x50000000,AccessKind::Fetch),ctx()),Entry::Pmp(14),AccessFault::Instruction);
        assert_eq!(p.check_pma(a(0x20001000,AccessKind::Load),ctx()),PolicyDecision::NoMatch);
        allowed(p.check_pmp(a(0x20001000,AccessKind::Load),ctx()),Entry::Pmp(0));
        assert_eq!(p.check_pma(a(0x600990c4,AccessKind::Store),ctx()),PolicyDecision::NoMatch);
        allowed(p.check_pmp(a(0x600990c4,AccessKind::Store),ctx()),Entry::Pmp(15));
        denied(p.check_pmp(a(0x600990c4,AccessKind::Fetch),ctx()),Entry::Pmp(15),AccessFault::Instruction);
        assert_eq!(p.check_pma(a(0xffffff00,AccessKind::Load),ctx()),PolicyDecision::Deny {entry:Entry::Pma(11),cause:AccessFault::Load});
        assert_eq!(p.check_pma(a(0xffffff80,AccessKind::Load),ctx()),PolicyDecision::Unsupported(UnsupportedReason::AddressWindow(Entry::Pma(11))));
        assert_eq!(p.read_csr(0xbdb,ctx()),Err(UnsupportedReason::AddressReadback(Entry::Pma(11))));
        if heap!=0 { allowed(p.check_pmp(a(0x42310000,AccessKind::AtomicRmw),ctx()),Entry::Pmp(10)); }
        else { assert_eq!(p.check_pmp(a(0x42310000,AccessKind::AtomicRmw),ctx()),PolicyDecision::NoMatch); }
        // Replaying profile/ROM requests without a core reset must not unlock it.
        factory::pma(&mut p,ctx()); factory::pmp(&mut p,ctx(),heap); assert_eq!(p.snapshot(),s);
        p.reset_core().unwrap(); assert_eq!(p.snapshot().pma_por_mask(),0xffff); assert_eq!(*p.snapshot().pmp_cfg(),[0;4]);
    }
}
