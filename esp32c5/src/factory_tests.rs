//! Original vendor bytes and actual mask-ROM execution, not a diagnostic guest.
use crate::{Bringup,Stop};
#[test]
#[ignore="set C5_ROM_ELF and C5_LCD273_FACTORY to the fixed public rev100 ELF and original LCD2.73 FactoryOnly_20260724.bin"]
fn external_c5_lcd273_original_guest_configures_policy_and_completes_its_psram_test(){
    let rom=std::fs::read(std::env::var("C5_ROM_ELF").expect("set C5_ROM_ELF to fixed public rev100 ELF")).expect("read C5_ROM_ELF");
    let factory=std::fs::read(std::env::var("C5_LCD273_FACTORY").expect("set C5_LCD273_FACTORY to original FactoryOnly_20260724.bin")).expect("read C5_LCD273_FACTORY");
    let hash=|b:&[u8]|crate::image::sha256(b).iter().map(|v|format!("{v:02x}")).collect::<String>();
    assert_eq!(hash(&rom),"49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a");
    assert_eq!(hash(&factory),"cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942");
    let mut m=crate::rom_machine([0;6],32<<20);m.bus.set_xtal_mhz(48).unwrap();m.bus.configure_psram(8<<20).unwrap();
    m.bus.attach_board(crate::board::make_board("waveshare-c5-lcd273").unwrap());
    m.load_rom(&rom).unwrap();m.load_input(esp_soc::LoadKind::Flash,&factory).unwrap();
    // This original clock-init entry follows the firmware's own PSRAM test.
    m.add_observer(Box::new(esp_soc::observers::Breakpoints{pcs:vec![0x4211_f8a0]}));m.console.capture=true;m.boot_rom();
    assert!(matches!(m.run(30_000_000),esp_soc::Stop::Breakpoint(0x4211_f8a0)),"{:?}",m.bus.first_fault);
    assert!(m.bus.first_fault.is_none());assert!(String::from_utf8_lossy(&m.console.uart0).contains("SPI SRAM memory test OK"));
    let policy=m.cores[0].c5_memory_protection().unwrap().snapshot();assert_eq!(policy.pma_por_mask(),0);assert_eq!(policy.generation(),104);
    // IDF5.5.3 s_test_psram writes one u32 every32 bytes across the mapped8MiB.
    // Inspect the real backing cells in addition to the guest's success log.
    assert_eq!(m.bus.psram.len(),8<<20);
    for offset in(0..8<<20).step_by(32){assert_eq!(&m.bus.psram[offset..offset+4],&((offset as u32/4)^0xaaaa_aaaa).to_le_bytes(),"PSRAM byte{offset:#x}");}
    eprintln!("C5 original guest completed PMA/PMP and PSRAM stride32 test across8MiB;262144 actual cells matched; app_main/LCD/TF not exercised");
}
#[test]
#[ignore="set C5_ROM_ELF and C5_LCD273_FACTORY to the fixed public rev100 ELF and original LCD2.73 FactoryOnly_20260724.bin"]
fn external_c5_lcd273_original_bootloader_loads_and_enters_the_unchanged_application(){
    use emu_core::Bus;
    let rom=std::fs::read(std::env::var("C5_ROM_ELF").expect("set C5_ROM_ELF to the fixed public rev100 ELF")).expect("read C5_ROM_ELF");
    let factory=std::fs::read(std::env::var("C5_LCD273_FACTORY").expect("set C5_LCD273_FACTORY to original FactoryOnly_20260724.bin")).expect("read C5_LCD273_FACTORY");
    assert_eq!(crate::image::sha256(&factory).as_slice(),&[0xcf,0x10,0xa4,0x35,0x8d,0x03,0x89,0x84,0x58,0xc1,0x63,0x90,0xf3,0xf1,0x6c,0xeb,0x03,0x06,0xbe,0xfe,0x22,0x60,0x16,0x0d,0xe0,0x91,0x9d,0x75,0x5b,0x92,0x09,0x42]);
    let layout=crate::image::validate_merged(&factory,32<<20,0x8000).unwrap();let off=layout.apps.iter().find(|a|a.subtype==0).unwrap().offset;assert_eq!(off,0x20000);
    let app=crate::image::validate_at(&factory[off..],32<<20,off).unwrap().image;assert_eq!(app.segments.len(),5);
    let mut m=crate::rom_machine([0;6],32<<20);m.bus.set_xtal_mhz(48).unwrap();m.bus.configure_psram(8<<20).unwrap();
    m.load_rom(&rom).unwrap();m.load_input(esp_soc::LoadKind::Flash,&factory).unwrap();
    m.add_observer(Box::new(esp_soc::observers::Breakpoints{pcs:vec![app.entry]}));m.console.capture=true;m.boot_rom();
    assert!(matches!(m.run(25_000_000),esp_soc::Stop::Breakpoint(pc) if pc==app.entry),"{:?}",m.bus.first_fault);
    assert!(m.bus.first_fault.is_none());assert_eq!(m.cores[0].pc,app.entry);assert_eq!(m.bus.app_off,None,"no host app-direct staging");
    let(mut ram,mut mapped)=(0,0);
    for s in &app.segments{
        let bytes=&factory[off+s.file_off as usize..off+(s.file_off+s.len)as usize];
        if let Some(start)=s.load_addr.checked_sub(crate::bus::SRAM_BASE).filter(|&v|(v as usize)<crate::bus::SRAM_SIZE){
            assert_eq!(&m.bus.sram[start as usize..start as usize+s.len as usize],bytes);ram+=s.len;
        }else{
            for(i,&byte)in bytes.iter().enumerate(){assert_eq!(m.bus.read8(s.load_addr+i as u32).unwrap(),byte);}mapped+=s.len;
        }
    }
    assert_eq!((ram,mapped),(121_016,3_094_800));
    assert!(String::from_utf8_lossy(&m.console.uart0).contains("Loaded app from partition at offset 0x20000"));
    eprintln!("C5 unchanged ROM/bootloader->application entry={:#x} RAM={ram}B mapped={mapped}B insns={}; app_main/LCD/TF not exercised",app.entry,m.insns());
}
#[test]
#[ignore="set C5_ROM_ELF and C5_LCD273_FACTORY to the fixed public rev100 ELF and original LCD2.73 FactoryOnly_20260724.bin"]
fn external_c5_lcd273_mask_rom_loads_and_enters_the_unchanged_bootloader(){
    let rom=std::fs::read(std::env::var("C5_ROM_ELF").expect("set C5_ROM_ELF to the fixed public rev100 ELF")).expect("read C5_ROM_ELF");
    let factory=std::fs::read(std::env::var("C5_LCD273_FACTORY").expect("set C5_LCD273_FACTORY to original FactoryOnly_20260724.bin")).expect("read C5_LCD273_FACTORY");
    assert_eq!(crate::image::sha256(&factory).as_slice(),&[0xcf,0x10,0xa4,0x35,0x8d,0x03,0x89,0x84,0x58,0xc1,0x63,0x90,0xf3,0xf1,0x6c,0xeb,0x03,0x06,0xbe,0xfe,0x22,0x60,0x16,0x0d,0xe0,0x91,0x9d,0x75,0x5b,0x92,0x09,0x42]);
    let layout=crate::image::validate_merged(&factory,32<<20,0x8000).unwrap();
    assert_eq!(layout.bootloader_offset,0x2000);
    let boot=esp_soc::image::parse(&factory[0x2000..]).unwrap();assert_eq!(boot.entry,0x4084_bbaa);
    let mut m=Bringup::new();m.bus.set_xtal_mhz(48).unwrap();m.bus.configure_psram(8<<20).unwrap();
    m.bus.flash=vec![255;32<<20];m.bus.flash[..factory.len()].copy_from_slice(&factory);m.load_rom_elf(&rom).unwrap();
    // Chip-level boot-prefix fixture: no board/TF/display is attached or claimed.
    // No host stages segments in SRAM: only the actual ROM copies them.
    for _ in 0..1_000_000{if m.cpu.pc==boot.entry{break;}let run=m.run(1);assert_eq!(run.stop,Stop::Budget,"{:?}",m.bus.first_fault);}
    assert_eq!(m.cpu.pc,boot.entry);assert!(m.bus.first_fault.is_none());
    let mut copied=0;
    for s in &boot.segments{
        let start=s.load_addr.checked_sub(crate::bus::SRAM_BASE).expect("fixed bootloader segment in HP SRAM") as usize;
        let end=start+s.len as usize;let input=s.file_off as usize+0x2000;
        assert_eq!(&m.bus.sram[start..end],&factory[input..input+s.len as usize]);copied+=s.len;
    }
    assert!(String::from_utf8_lossy(&m.bus.uart[0].tx_out).ends_with("entry 0x4084bbaa\r\n"));
    eprintln!("C5 original LCD273 ROM->bootloader entry={:#x} copied={copied}B insns={} UART={}B; app/display/TF not exercised",m.cpu.pc,m.cpu.insn_count,m.bus.uart[0].tx_out.len());
}

#[test]
#[ignore="set C5_ROM_ELF to the fixed public rev100 ELF"]
fn external_c5_swd_routes_real_requests_and_retains_system_reset_flag(){
    use emu_core::Bus;
    use esp_soc::SocBus;
    let rom=std::fs::read(std::env::var("C5_ROM_ELF").expect("set C5_ROM_ELF to the fixed public rev100 ELF")).expect("read C5_ROM_ELF");
    let mut m=crate::machine([0;6],32<<20);
    assert_eq!(m.bus.next_deadline(),None);m.load_rom(&rom).unwrap();
    // Isolate the SWD in this fixture: TIMG0's independently modeled POR
    // cycling calibration otherwise has an earlier real timeout deadline.
    m.bus.write32(0x6000_8068,0x0001_0000).unwrap();
    // Independently disable the real TG0 cold flashboot protection as the ROM does.
    m.bus.write32(0x6000_8064,0x50d8_3aa1).unwrap();let tg0=m.bus.read32(0x6000_8048).unwrap()&!(1<<14);
    m.bus.write32(0x6000_8048,tg0).unwrap();m.bus.write32(0x6000_8048,tg0|(1<<22)).unwrap();m.bus.write32(0x6000_8064,0).unwrap();
    assert_eq!(m.bus.next_deadline(),Some(204_000_000));
    // Actual matrix source18, independent of peripheral interrupt bit30.
    m.bus.write32(0x6001_0048,23).unwrap();m.bus.write32(0x600b_1c2c,1<<30).unwrap();
    m.bus.advance(204_000_000);assert_eq!(m.bus.irq_sources_of(0,7),vec![18]);
    assert!(!m.bus.sw_reset());assert_eq!(m.bus.next_deadline(),Some(24_000_000));
    m.bus.advance(24_000_000);assert!(m.bus.sw_reset());assert_eq!(m.bus.reset_cause(),0x12);
    assert_eq!(m.bus.read32(0x600b_1c1c).unwrap(),(300<<20)|1);
    assert!(m.load_rom(&rom).unwrap_err().contains("pending reset"));
    assert!(m.bus.sw_reset());assert_eq!(m.bus.reset_cause(),0x12);
    m.bus.reboot([0;6]);assert!(!m.bus.sw_reset());assert!(m.bus.has_validated_rom());
    assert_eq!(m.bus.read32(0x600b_1c1c).unwrap(),(300<<20)|1);
    assert_eq!(m.bus.read32(0x600b_1c20).unwrap(),0);
    assert_eq!(m.bus.read32(0x600b_1c24).unwrap(),1<<30);
    m.bus.write32(0x600b_1c20,0x50d8_3aa1).unwrap();
    m.bus.write32(0x600b_1c1c,(300<<20)|(1<<18)).unwrap();
    assert_eq!(m.bus.next_deadline(),Some(3000));m.bus.advance(2999);
    assert!(m.bus.report().contains("auto_feeds=0"));m.bus.advance(1);
    assert!(m.bus.report().contains("auto_feeds=1"));assert!(!m.bus.sw_reset());
    m.bus.write32(0x600b_1c30,1<<30).unwrap();assert_eq!(m.bus.read32(0x600b_1c24).unwrap(),0);
    m.bus.write32(0x600b_1c1c,(300<<20)|(1<<18)|(1<<19)).unwrap();
    assert_eq!(m.bus.read32(0x600b_1c1c).unwrap()&1,0);
    let deadline=m.bus.lp_wdt.deadline;m.bus.request_reset(3);m.bus.reboot([0;6]);
    // The independent HP calibration engine restarts on Core Reset; compare
    // the retained LP SWD's own absolute deadline rather than the shared minimum.
    assert_eq!(m.bus.lp_wdt.deadline,deadline);assert_eq!(m.bus.read32(0x600b_1c1c).unwrap(),(300<<20)|(1<<18));
    // Raw ROM writes deactivate the selected autonomous profile, despite the
    // report flag: no validated-only timing can leak into this mode.
    m.bus.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();assert!(!m.bus.has_validated_rom());
    assert_eq!(m.bus.next_deadline(),None);assert!(m.bus.read32(0x600b_1c24).is_err());
    m.bus.advance(1_000_000_000);assert!(!m.bus.sw_reset());
    // The alternate probe loader must also preserve a dispatched reset rather
    // than replacing the ROM while retained LP state still awaits its domains.
    let mut probe=Bringup::new();probe.bus.request_reset(3);let pc=probe.cpu.pc;
    assert!(probe.load_rom_elf(&rom).unwrap_err().contains("pending reset"));
    assert!(probe.bus.sw_reset());assert_eq!(probe.cpu.pc,pc);assert!(!probe.bus.has_validated_rom());
}
