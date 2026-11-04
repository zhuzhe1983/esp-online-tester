//! S3/LX7 reset permits coprocessor use before an RTOS creates its first task.
//! Cadence LX7 Data Book (2017), section 9.1 / table 9-15; ESP-IDF v4.4.5
//! components/xtensa/esp32s3/include/xtensa/config/tie.h (XCHAL_CP_MAX=4).
use esp_soc::Soc;
use esp32s3::S3;
use xtensa_lx7::{step, FlatRam, Trap};
use xtensa_lx7::block::run_block;
use xtensa_lx7::state::{exc, ps, sr};

const BASE: u32 = 0x4037_0000;

#[test]
fn s3_reset_paths_enable_the_configured_coprocessor_slots() {
    for core in 0..2 {
        let mut cpu = S3::new_core(core);
        assert_eq!(cpu.read_sr(sr::CPENABLE), Some(0x0f));
        cpu.write_sr(sr::CPENABLE, 0);
        S3::reset_core(&mut cpu, core);
        assert_eq!(cpu.read_sr(sr::CPENABLE), Some(0x0f));
        cpu.write_sr(sr::CPENABLE, 0);
        S3::boot_core(&mut cpu, BASE);
        assert_eq!(cpu.read_sr(sr::CPENABLE), Some(0x0f));
    }
    let mut m = esp32s3::machine([0; 6]);
    m.cores[0].write_sr(sr::CPENABLE, 0);
    m.boot_rom();
    assert_eq!(m.cores[0].read_sr(sr::CPENABLE), Some(0x0f));
    for cpu in &mut m.cores { cpu.write_sr(sr::CPENABLE, 0); }
    m.reboot();
    assert!(m.cores.iter_mut().all(|cpu| cpu.read_sr(sr::CPENABLE) == Some(0x0f)));
}

#[test]
fn reset_fpu_load_succeeds_but_guest_disabling_still_traps() {
    for block in [false, true] {
        let mut cpu = S3::new_core(0);
        cpu.pc = BASE; cpu.ps = ps::UM;
        cpu.set_ar(2, BASE);
        cpu.set_ar(3, 0);
        let mut ram = FlatRam::new(BASE, 0x200);
        // lsi f1,a2,208; wsr.cpenable a3; rsync; lsi f1,a2,208.
        ram.mem[..12].copy_from_slice(&[0x13, 0x02, 0x34, 0x30, 0xe0, 0x13, 0x10, 0x20, 0x00, 0x13, 0x02, 0x34]);
        ram.mem[208..212].copy_from_slice(&1.25f32.to_bits().to_le_bytes());
        let exec = |cpu: &mut xtensa_lx7::Cpu, ram: &mut FlatRam| {
            if block { run_block(cpu, ram, 1).1 } else { step(cpu, ram).err() }
        };
        assert_eq!(exec(&mut cpu, &mut ram), None);
        assert_eq!(cpu.fr[1], 1.25f32.to_bits());
        assert_eq!(exec(&mut cpu, &mut ram), None);
        assert_eq!(exec(&mut cpu, &mut ram), None);
        assert_eq!(cpu.cpenable, 0);
        cpu.fr[1] = 0x1234_5678;
        assert_eq!(exec(&mut cpu, &mut ram), Some(Trap::Exception(exc::COPROCESSOR0_DISABLED)));
        assert_eq!(cpu.epc[1], BASE + 9);
        assert_eq!(cpu.fr[1], 0x1234_5678);
    }
}
