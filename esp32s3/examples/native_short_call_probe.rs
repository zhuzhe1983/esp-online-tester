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
    let hz=esp32s3::periph::CPU_HZ as f64;
    let mut matrix=Vec::new();let mut script=String::new();
    if args[6]!="-" {
        for line in std::fs::read_to_string(&args[6]).expect("script").lines() {
            let fields:Vec<_>=line.split_whitespace().collect();
            if fields.get(1)==Some(&"matrix") {
                assert_eq!(fields.len(),5,"seconds matrix row column down|up");
                let at:f64=fields[0].parse().expect("matrix seconds");assert!(at.is_finite()&&at>=0.0);
                let row:u8=fields[2].parse().expect("matrix row");let col:u8=fields[3].parse().expect("matrix column");
                assert!(row<4&&col<14);let down=match fields[4] {"down"=>true,"up"=>false,_=>panic!("matrix down|up")};
                matrix.push(((at*hz) as u64,row,col,down));
            } else {script.push_str(line);script.push('\n');}
        }
    }
    assert!(matrix.windows(2).all(|w|w[0].0<=w[1].0),"matrix edges must be ordered");
    // Passive checkpoints observe existing run-budget yields, rather than adding deadlines.
    // This diagnostic is left unset in throughput arms; reported values are workload counts.
    let phases:Vec<u64>=std::env::var("EOT_PROBE_PHASE_CYCLES").unwrap_or_default().split(',')
        .filter(|s|!s.is_empty()).map(|s|s.parse().expect("phase cycles")).collect();
    assert!(phases.windows(2).all(|w|w[0]<=w[1]));
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
    m.load_script(&script).unwrap();
    m.console.mask=2;m.console.capture=true;m.boot_rom();
    let target=(seconds*hz) as u64;let (mut edge,mut phase)=(0,0);
    let stop=loop {
        while let Some(&(at,row,col,down))=matrix.get(edge).filter(|&&(at,_,_,_)|at<=m.bus.cycles) {
            assert!(m.bus.board.matrix_key_at(m.bus.cycles,row,col,down));
            eprintln!("[edge] requested={at} actual={} row={row} column={col} pressed={down}",m.bus.cycles);edge+=1;
        }
        m.max_cycles=matrix.get(edge).map_or(target,|e|target.min(e.0));
        let stop=m.run(budget);
        while phase<phases.len()&&m.bus.cycles>=phases[phase] {
            eprintln!("[phase] requested={} actual={} insns={}/{} pc={:x}/{:x} display_version={}",phases[phase],m.bus.cycles,
                m.cores[0].insn_count(),m.cores[1].insn_count(),m.cores[0].pc(),m.cores[1].pc(),m.bus.board.display_version());phase+=1;
        }
        if m.bus.cycles>=target {break stop;}
        assert!(matches!(stop,esp32s3::Stop::MaxInsns)||(matches!(stop,esp32s3::Stop::Halted)&&m.bus.cycles>=m.max_cycles),"early stop {stop:?} at{}",m.bus.cycles);
    };
    m.drain_console();std::fs::write(format!("{}.console",args[5]),&m.console.all).unwrap();
    m.write_tft_png(&format!("{}.png",args[5]),1).unwrap();
    let adc=m.bus.adc_observation(10).unwrap();
    eprintln!("[probe] stop={stop:?} cycles={} insns={}/{} exceptions={} interrupts={} reboots={} adc={}/{} pc={:x}/{:x} jit={}/{} bytes={}/{}",
        m.bus.cycles,m.cores[0].insn_count(),m.cores[1].insn_count(),m.exceptions,m.interrupts,m.reboots,adc.raw,adc.generation,
        m.cores[0].pc(),m.cores[1].pc(),m.cores[0].blocks.jit_instructions,m.cores[1].blocks.jit_instructions,
        m.cores[0].blocks.code_bytes(),m.cores[1].blocks.code_bytes());
    for (index,core) in m.cores.iter().enumerate() {eprintln!("{}",core.dump(index,&|_|String::new()));}
    eprintln!("[keys] sent={edge} diagnostics={:?}",m.bus.board.matrix_key_diagnostics());
}
