//! Execute the original pinned ROM getters, not a host-side rewrite of their algorithm.
use emu_core::Bus;
const RETURN:u32=0x4000_0400;
fn setup(pc:u32, enabled:bool)->esp32::Machine {
    let mut m=esp32::machine([0;6],4<<20);
    m.load_rom(&std::fs::read(esp_soc::rom::require("esp32_rev300_rom.elf")).unwrap()).unwrap();
    m.bus.periph.write32(0x3ff0_00cc,if enabled {(1<<11)|(1<<16)|(1<<17)} else {0});
    let c=&mut m.cores[0];c.pc=pc;c.ps=(1<<18)|(1<<16);c.windowbase=0;c.windowstart=1;
    c.set_ar(1,0x3ffe_8000);c.set_ar(4,RETURN);
    m
}
fn steps(m:&mut esp32::Machine,n:usize)->bool {
    for _ in 0..n {if m.cores[0].pc==RETURN {return true;}
        xtensa_lx7::step(&mut m.cores[0],&mut m.bus).unwrap();m.bus.tick(1);
    }m.cores[0].pc==RETURN
}
#[test]
#[ignore="needs pinned esp32_rev300_rom.elf; actual ROM execution"]
fn external_native_clock_three_original_getters_complete_and_return_expected_units() {
    for (pc,wrapped_expected) in [(0x4003c9e4,0),(0x40015894,0),(0x400158d4,0x07ff_ffff)] {
        for (coarse,expected) in [(20u64,10u32),(0x0fff_ffff,wrapped_expected)] {
            let mut m=setup(pc,true);m.bus.periph.bt_clock.advance(coarse*75000+10000);
            assert!(steps(&mut m,2000),"ROM getter {pc:#x} stuck at {:#x}",m.cores[0].pc);
            assert_eq!(m.cores[0].get_ar(6),expected,"ROM getter {pc:#x}");
            assert_eq!(m.bus.periph.bt_clock.completed,1);assert_eq!(m.bus.periph.bt_clock.requests,1);
        }
    }
}
#[test]
#[ignore="needs pinned esp32_rev300_rom.elf; actual ROM execution"]
fn external_native_clock_original_rom_waits_for_real_guest_time_and_clock_enable() {
    let mut m=setup(0x4003c9e4,false);
    assert!(!steps(&mut m,400));assert!(matches!(m.cores[0].pc,0x4003ca00|0x4003ca03|0x4003ca05));
    assert_eq!(m.bus.periph.bt_clock.completed,0);
    m.bus.periph.write32(0x3ff0_00cc,(1<<11)|(1<<16)|(1<<17));
    assert!(steps(&mut m,2000));assert_eq!(m.bus.periph.bt_clock.completed,1);
}
