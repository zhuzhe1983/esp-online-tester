//! EX164 source-only comparison harness. Both feature sets use the same ROM boot, workload,
//! clock, scheduler and inputs. CPI1 is the ordinary timing path; other CPI values explicitly
//! select the already-existing rough model. Elapsed time is measured by an external paired runner.
use emu_core::Core;
use esp_soc::SocBus;

fn main() {
    let args:Vec<_>=std::env::args().collect();
    assert!(args.len()==8,"usage: native_short_call_probe ROM FLASH CPI GUEST_SECONDS PREFIX SCRIPT_OR_DASH RUN_BUDGET");
    let cpi:u32=args[3].parse().expect("CPI");let seconds:f64=args[4].parse().expect("guest seconds");
    let budget:u64=args[7].parse().expect("run budget");assert!(seconds.is_finite()&&seconds>0.0&&seconds<=60.0&&budget>0);
    let mut m=esp32s3::machine([0x44,0x1b,0xf6,0x75,0xdc,0xe0]);
    m.bus.board=esp32s3::board::make_board("cardputer-adv").unwrap();
    esp32s3::board::configure_chip_identity(m.bus.board.name(),&mut m.bus.periph.efuse);
    m.bus.set_psram_size(0).unwrap();m.bus.attach_board_devices();
    m.bus.analog_set(10,esp_periph::AnalogSource::Const(2.0135));
    m.load_rom(&std::fs::read(&args[1]).expect("ROM ELF")).unwrap();
    let flash=std::fs::read(&args[2]).expect("merged flash");
    esp_soc::load::check_flash_image::<esp32s3::S3>(&flash).unwrap();m.write_flash(0,&flash).unwrap();
    m.quantum=64;m.vq_max=1;m.bb_max=1;m.rt.enabled=false;
    if cpi!=1 {m.set_approximate_jit_timing(cpi,64).unwrap();m.set_approximate_jit_frontiers(false).unwrap();}
    if args[6]!="-" {m.load_script(&std::fs::read_to_string(&args[6]).expect("script")).unwrap();}
    m.console.mask=2;m.console.capture=true;m.boot_rom();
    m.max_cycles=(seconds*esp32s3::periph::CPU_HZ as f64) as u64;
    let stop=loop {
        let stop=m.run(budget);
        if m.bus.cycles>=m.max_cycles {break stop;}
        assert!(matches!(stop,esp32s3::Stop::MaxInsns),"early stop {stop:?} at{}",m.bus.cycles);
    };
    m.drain_console();std::fs::write(format!("{}.console",args[5]),&m.console.all).unwrap();
    m.write_tft_png(&format!("{}.png",args[5]),1).unwrap();
    let adc=m.bus.adc_observation(10).unwrap();
    eprintln!("[probe] stop={stop:?} cycles={} insns={}/{} exceptions={} interrupts={} reboots={} adc={}/{} pc={:x}/{:x} jit={}/{} bytes={}/{}",
        m.bus.cycles,m.cores[0].insn_count(),m.cores[1].insn_count(),m.exceptions,m.interrupts,m.reboots,adc.raw,adc.generation,
        m.cores[0].pc(),m.cores[1].pc(),m.cores[0].blocks.jit_instructions,m.cores[1].blocks.jit_instructions,
        m.cores[0].blocks.code_bytes(),m.cores[1].blocks.code_bytes());
    for (index,core) in m.cores.iter().enumerate() {eprintln!("{}",core.dump(index,&|_|String::new()));}
}
