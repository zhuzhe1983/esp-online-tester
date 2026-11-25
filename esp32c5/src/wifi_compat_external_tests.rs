use crate as esp32c5;
// Fixed original firmware: strict and assisted runs stay distinct.
use esp_soc::SocBus;
fn bytes(name:&str)->Vec<u8>{std::fs::read(std::env::var(name).unwrap_or_else(|_|panic!("set {name} to the unchanged pinned public input"))).unwrap_or_else(|_|panic!("read {name}"))}
#[test]
#[ignore="set C5_ROM_ELF, C5_LCD273_FACTORY and C5_SD_IMG to the unchanged pinned ROM/FactoryOnly/64MiB prepared card"]
fn external_original_factory_offline_driver_reaches_its_menu_with_real_event_queue(){
 let rom=bytes("C5_ROM_ELF");let factory=bytes("C5_LCD273_FACTORY");let sd=bytes("C5_SD_IMG");
 let hash=|b:&[u8]|esp32c5::image::sha256(b).iter().map(|v|format!("{v:02x}")).collect::<String>();
 assert_eq!(hash(&rom),"49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a");assert_eq!(hash(&factory),"cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942");assert_eq!(hash(&sd),"a0e026d9aeafab624884455d25147647980bd6a83011618da04140659c28bfc6");
 let setup=||{let mut m=esp32c5::rom_machine([0;6],32<<20);m.bus.set_xtal_mhz(48).unwrap();m.bus.configure_psram(8<<20).unwrap();m.bus.attach_board(esp32c5::board::make_board("waveshare-c5-lcd273").unwrap());m.bus.attach_sd_card(Box::new(esp_periph::sdmmc::MemStore(sd.clone())),Default::default()).unwrap();m.load_rom(&rom).unwrap();m.load_input(esp_soc::LoadKind::Flash,&factory).unwrap();m.console.capture=true;m};
 let mut strict=setup();strict.boot_rom();assert!(matches!(strict.run(500_000_000),esp_soc::Stop::ModelLimit{pc:0x4081_42c6,..}));assert_eq!((strict.run_steps(),strict.insns(),strict.bus.cycles()),(44_199_490,44_199_490,44_199_486));assert_eq!(hash(&strict.console.uart0),"f6aae87b2bcd45efc1595ba51ae1ebfb76331b3d9a95fefcf0b848357f0f79ad");assert!(strict.bus.wifi_driver_compat_snapshot().is_none());
 let mut assisted=setup();
 // NVS bytes do not change API identity. This one-byte negative fixture is
 // not a valid NVS record; restore it before executing the unchanged guest.
 let nvs=assisted.bus.flash[0x9f00];assisted.bus.write_flash(0x9f00,&[0x55]).unwrap();
 esp32c5::enable_wifi_driver_compat(&mut assisted,"waveshare-lcd273-idf553-offline-v1").unwrap();assisted.bus.write_flash(0x9f00,&[nvs]).unwrap();assisted.boot_rom();
 assert!(matches!(assisted.run(500_000_000),esp_soc::Stop::MaxInsns));assert!(assisted.bus.first_fault.is_none(),"{:?}",assisted.bus.first_fault);
 let uart=String::from_utf8_lossy(&assisted.console.uart0);assert!(uart.contains("APP菜单启动成功"));assert!(uart.contains("Returned from app_main()"));assert!(uart.contains("Wi-Fi connection failed"));assert_eq!(uart.matches("Retrying...").count(),3);
 let frame=assisted.bus.board.display().unwrap();assert_eq!((frame.0,frame.1),(320,320));assert!(frame.2.iter().any(|&pixel|pixel!=0));
 let state=assisted.bus.wifi_driver_compat_snapshot().unwrap();for field in["\"nativeWifi\":false","\"associated\":false","\"hasIp\":false","\"storage\":\"session-RAM\"","\"eventPosts\":\"5\"","\"noApFailuresQueued\":\"4\"","\"pendingCalls\":0","\"lastError\":null"]{assert!(state.contains(field),"missing {field}: {state}");}
 // Exercise the actual hook entry after the real ROM established its maps.
 // Each probe uses a fresh CPU policy and clears the old CPU's snapshot cache.
 let fresh=|m:&mut crate::Machine,pc:u32,base:u32,cfg:u32|{
  let mut cpu=crate::core();cpu.enable_c5_memory_protection().unwrap();cpu.try_write_csr(0x3b0,(base>>2)|15).unwrap();cpu.try_write_csr(0x3a0,cfg).unwrap();cpu.pc=pc;cpu.x[1]=0x4200_0020;cpu.x[2]=0x4085_d000;m.bus.monitor.as_ref().unwrap().borrow_mut().cpu_reset();m.cores[0]=cpu;
 };
 let out=0x4085_f07c;assisted.bus.compat_write(out,&[0xa5;6]).unwrap();fresh(&mut assisted,super::GET_MAC,out+4,0x99);assisted.cores[0].x[10]=0;assisted.cores[0].x[11]=out;
 assert!(super::intercept(&mut assisted.cores[0],&mut assisted.bus));assert_eq!(assisted.cores[0].x[10],0x102);assert_eq!(assisted.bus.compat_read(out,6).unwrap(),[0xa5;6]);assert_eq!(assisted.bus.monitor.as_ref().unwrap().borrow().machine_context(),None);
 let input=0x4085_e000;let mut cfg=[0;152];cfg[144..148].copy_from_slice(&0x1f2f_3f4fu32.to_le_bytes());assisted.bus.compat_write(input,&cfg).unwrap();fresh(&mut assisted,super::INIT,input,0x9c);assisted.cores[0].x[10]=input;assisted.bus.wifi_compat.as_mut().unwrap().initialized=false;
 assert!(super::intercept(&mut assisted.cores[0],&mut assisted.bus));assert_eq!(assisted.cores[0].x[10],0x102);assert!(!assisted.bus.wifi_compat.as_ref().unwrap().initialized);assisted.bus.wifi_compat.as_mut().unwrap().initialized=true;
 let callbacks=assisted.bus.wifi_compat.as_ref().unwrap().callbacks;assisted.bus.load_ram(input,&0x13u32.to_le_bytes(),4).unwrap();fresh(&mut assisted,super::REG_BUF,input,0x9b);assisted.cores[0].x[10]=input;assisted.cores[0].x[11]=input;
 assert!(super::intercept(&mut assisted.cores[0],&mut assisted.bus));assert_eq!(assisted.cores[0].x[10],0x102);assert_eq!(assisted.bus.wifi_compat.as_ref().unwrap().callbacks,callbacks);
 // Readable function bytes do not confer execute permission on an API hook.
 fresh(&mut assisted,super::GET_MAC,0x420d_0d80,0x9b);assisted.cores[0].x[10]=0;assisted.cores[0].x[11]=out;let before=assisted.bus.sram.clone();
 assert!(!super::intercept(&mut assisted.cores[0],&mut assisted.bus));assert_eq!(assisted.bus.sram,before);assert!(assisted.bus.wifi_driver_compat_snapshot().unwrap().contains("not executable"));assert_eq!(assisted.bus.monitor.as_ref().unwrap().borrow().machine_context(),None);
 assert!(matches!(assisted.run(1),esp_soc::Stop::ModelLimit{pc:super::GET_MAC,..}));
 let mut corrupted=setup();corrupted.bus.write_flash(0x201000,&[0x11]).unwrap();assert!(esp32c5::enable_wifi_driver_compat(&mut corrupted,"waveshare-lcd273-idf553-offline-v1").is_err());assert!(corrupted.bus.wifi_driver_compat_snapshot().is_none());
}
