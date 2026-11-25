use super::*;

#[test]
fn shared_cache_misses_queue_without_charging_hits_or_service_twice() {
    let mut bus = SocBus::new(65536, 65536, [0; 6]);
    bus.mmu[0] = MMU_SPIRAM;
    bus.enable_approximate_cache(Default::default());
    bus.set_approximate_cache_contention(true);
    bus.begin_timing_batch(0, 100);
    assert_eq!(bus.timing_penalty_pending(), Some(false));
    bus.read32(DBUS_LOW).unwrap();
    for _ in 0..4 { assert_eq!(bus.timing_penalty_pending(), Some(true)); }
    assert_eq!(bus.take_timing_penalty(), 120);
    assert_eq!(bus.timing_penalty_pending(), Some(false), "observing the actual miss must not drain or requeue its debt");
    bus.begin_timing_batch(1, 100);
    bus.read32(DBUS_LOW).unwrap(); // hit while the other core's resource is occupied
    assert_eq!(bus.take_timing_penalty(), 0);
    bus.read32(DBUS_LOW + 64).unwrap();
    assert_eq!(bus.take_timing_penalty(), 240); // 120 service + 120 queued
    assert_eq!(bus.approximate_cache_wait_cycles(), [0, 120]);
    bus.begin_timing_batch(0, 340);
    bus.read32(DBUS_LOW + 128).unwrap();
    assert_eq!(bus.take_timing_penalty(), 120);
}

#[test]
fn demand_ready_can_precede_resource_release() {
    let mut bus = SocBus::new(65536, 65536, [0; 6]);
    bus.mmu[0] = MMU_SPIRAM;
    bus.enable_approximate_cache(crate::approximate_cache::CacheConfig {
        fill_cycles: 96, ..Default::default()
    });
    bus.set_approximate_cache_contention(true);
    assert!(!bus.set_approximate_cache_fill_service(95));
    assert!(bus.set_approximate_cache_fill_service(160));
    bus.begin_timing_batch(0, 100);
    bus.read32(DBUS_LOW).unwrap();
    assert_eq!(bus.take_timing_penalty(), 96);
    // Data is ready at196. 20 cycles of CPU work hide20 of the64 remaining service.
    bus.begin_timing_batch(0, 216);
    bus.read32(DBUS_LOW + 64).unwrap();
    assert_eq!(bus.take_timing_penalty(), 44 + 96);
    // First resource burst100..260; second260..420. No extra wait after420.
    bus.begin_timing_batch(1, 420);
    bus.read32(DBUS_LOW + 128).unwrap();
    assert_eq!(bus.take_timing_penalty(), 96);
    assert_eq!(bus.approximate_cache_wait_cycles(), [44, 0]);
}

#[test]
fn packed_pie_loads_preserve_offchip_cache_accounting() {
    for (psram, flash_ready) in [(false, 96), (true, 96), (false, 128), (true, 128)] {
        let mut bus = SocBus::new(65536, 65536, [0; 6]);
        bus.mmu[0] = if psram { MMU_SPIRAM } else { 0 };
        let mut bulk = [0; 16];
        assert!(bus.read_bulk(DBUS_LOW, &mut bulk), "untimed bulk remains available");
        bus.enable_approximate_cache(crate::approximate_cache::CacheConfig {
            fill_cycles: 96, ..Default::default()
        });
        if flash_ready != 96 { assert!(bus.set_approximate_flash_timing(flash_ready, 475)); }
        // ee.vld.128.ip q0,a4,0: decode selects the packed executor.
        let bytes = 0x0083_0044u32.to_le_bytes();
        esp_soc::SocBus::load_bytes(&mut bus, IRAM_LOW, &bytes[..3]).unwrap();
        let mut cpu = xtensa_lx7::Cpu::new(0);
        cpu.pc = IRAM_LOW; cpu.ps = 0; cpu.cpenable = 8;
        cpu.set_ar(4, DBUS_LOW);
        xtensa_lx7::step(&mut cpu, &mut bus).unwrap();
        assert_eq!(cpu.qr[0], if psram { 0 } else { u128::MAX });
        let stats = bus.approximate_cache_stats().unwrap();
        assert_eq!((stats.line_fills, stats.hits), (1, 3), "packed load must account for four words");
        assert_eq!(bus.take_approximate_cache_penalty(), if psram { 96 } else { flash_ready });
        assert!(bus.read_bulk(DRAM_LOW, &mut bulk), "internal bulk stays fast in timed mode");
    }
}

#[test]
fn flash_override_keeps_psram_prices_and_shared_contention() {
    let mut bus = SocBus::new(65536, 65536, [0; 6]);
    bus.mmu[0] = MMU_SPIRAM;
    bus.mmu[1] = 0;
    bus.enable_approximate_cache(crate::approximate_cache::CacheConfig { fill_cycles: 96, writeback_cycles: 160, ..Default::default() });
    assert!(bus.set_approximate_cache_fill_service(160));
    assert!(!bus.set_approximate_flash_timing(128, 127));
    assert!(bus.set_approximate_flash_timing(128, 475));
    bus.set_approximate_cache_contention(true);
    bus.begin_timing_batch(0, 100);
    assert_eq!(bus.read32(DBUS_LOW + PAGE).unwrap(), u32::MAX);
    assert_eq!(bus.take_timing_penalty(), 128);
    // Flash occupies100..575; a simultaneous PSRAM miss waits475, then needs96.
    bus.begin_timing_batch(1, 100);
    bus.read32(DBUS_LOW).unwrap();
    assert_eq!(bus.take_timing_penalty(), 475 + 96);
    bus.begin_timing_batch(0, 228);
    bus.read32(DBUS_LOW + PAGE).unwrap();
    assert_eq!(bus.take_timing_penalty(), 0, "hits have no extra source charge");
    bus.begin_timing_batch(0, 735);
    bus.read32(DBUS_LOW + PAGE + 64).unwrap();
    assert_eq!(bus.take_timing_penalty(), 128);
    bus.begin_timing_batch(1, 1210);
    bus.read32(DBUS_LOW + 64).unwrap();
    assert_eq!(bus.take_timing_penalty(), 96);
    let stats = bus.approximate_cache_stats().unwrap();
    assert_eq!((stats.line_fills, stats.hits, stats.extra_cycles), (4, 1, 448));
    assert_eq!(bus.approximate_cache_wait_cycles(), [0, 475]);
}

#[test]
fn flash_override_does_not_reprice_dirty_victims() {
    let mut bus = SocBus::new(65536, 65536, [0; 6]);
    bus.mmu[0] = MMU_SPIRAM;
    bus.mmu[1] = 0;
    bus.enable_approximate_cache(crate::approximate_cache::CacheConfig {
        capacity_bytes: 64, ways: 1, fill_cycles: 96, writeback_cycles: 160, ..Default::default()
    });
    bus.set_approximate_cache_fill_service(160);
    bus.set_approximate_flash_timing(128, 475);
    bus.set_approximate_cache_contention(true);
    bus.begin_timing_batch(0, 0);
    bus.write32(DBUS_LOW, 7).unwrap();
    assert_eq!(bus.take_timing_penalty(), 96);
    bus.begin_timing_batch(0, 160);
    bus.read32(DBUS_LOW + PAGE).unwrap();
    assert_eq!(bus.take_timing_penalty(), 160 + 128);
    assert_eq!(bus.cache_resource.busy_until, 160 + 160 + 475);
    let stats = bus.approximate_cache_stats().unwrap();
    assert_eq!((stats.line_fills, stats.dirty_writebacks, stats.extra_cycles), (2, 1, 384));
}

#[test]
fn flash_override_is_optional_and_reset_by_cache_configuration() {
    let mut bus = SocBus::new(65536, 65536, [0; 6]);
    bus.mmu[0] = 0;
    assert!(!bus.set_approximate_flash_timing(128, 475));
    let config = crate::approximate_cache::CacheConfig { fill_cycles: 96, ..Default::default() };
    bus.enable_approximate_cache(config);
    bus.set_approximate_flash_timing(64, 80);
    bus.read32(DBUS_LOW).unwrap();
    assert_eq!(bus.take_timing_penalty(), 64, "readiness override also works without contention");
    bus.enable_approximate_cache(config);
    bus.read32(DBUS_LOW).unwrap();
    assert_eq!(bus.take_timing_penalty(), 96, "no override means the original common price");
}

#[test]
fn approximate_cache_keeps_only_internal_mappings_direct() {
    let mut bus = SocBus::new(65536, 65536, [0; 6]);
    bus.mmu[0] = MMU_SPIRAM;
    bus.enable_approximate_cache(Default::default());
    bus.set_approximate_cache_fast_internal(true);
    assert!(bus.fast_mem().is_some());
    bus.write32(DRAM_LOW, 42).unwrap();
    assert_eq!(bus.tlb[tlb_idx(DRAM_LOW)].lo, DRAM_LOW);
    bus.write32(DBUS_LOW, 43).unwrap();
    let e = bus.tlb[tlb_idx(DBUS_LOW)];
    assert!(!(DBUS_LOW >= e.lo && DBUS_LOW < e.hi));
    assert_eq!(bus.take_timing_penalty(), 120);
    assert_eq!(bus.read32(DBUS_LOW).unwrap(), 43);
    assert_eq!(bus.take_timing_penalty(), 0);
    let stats = bus.approximate_cache_stats().unwrap();
    assert_eq!((stats.line_fills, stats.hits), (1, 1));
    // Fetches do not enter the data cache even when they use external memory.
    bus.fetch(IBUS_LOW).unwrap();
    assert_eq!(bus.approximate_cache_stats().unwrap(), stats);
    bus.irq_dirty = false;
    bus.set_approximate_cache_yield_miss(true);
    assert!(!bus.block_break());
    bus.read32(DBUS_LOW + 64).unwrap();
    assert!(bus.block_break());
    assert_eq!(bus.take_timing_penalty(), 120);
    assert!(!bus.block_break());
}


#[test]
fn cache_reconfiguration_discards_inline_and_fast_internal_modes() {
    let mut bus = SocBus::new(65536, 65536, [0; 6]);
    bus.enable_approximate_cache(Default::default());
    // Simulate the prior inline mode, including on native where inline codegen is unavailable.
    bus.approximate_cache_inline = true;
    bus.approximate_cache_fast_internal = true;
    bus.enable_approximate_cache(Default::default());
    assert!(!bus.approximate_cache_inline);
    assert!(!bus.approximate_cache_fast_internal);
    assert!(bus.fast_cache().is_none());
    bus.set_approximate_cache_fast_internal(true);
    assert!(!bus.approximate_cache_inline);
    assert!(bus.fast_cache().is_none());
}
