//! Unchanged factory Flash/ROM in the common Machine scheduler. Experimental
//! native probe only; the public browser loader still enforces its own policy.
use esp_soc::SocBus;
use emu_core::Bus;
// Optional bounded register observations at caller-supplied PCs. No bus reads,
// register writes, jumps or firmware-specific success decisions are performed.
struct TracePcs{points:Vec<(u32,u8)>,arm:Option<u32>,mspi:bool,policy:bool}
impl esp_soc::observe::Observer<esp32c5::C5> for TracePcs{
    fn name(&self)->&'static str{"factory-pc-registers"}
    fn wants(&self)->esp_soc::observe::Wants{esp_soc::observe::Wants::INSN}
    fn on_insn(&mut self,cx:&esp_soc::observe::Ctx,_core:usize,cpu:&riscv_rv32::Cpu,bus:&mut esp32c5::C5Bus,pc:u32)->Option<esp_soc::Stop>{
        if self.arm.is_some_and(|arm|pc!=arm){return None;}self.arm=None;
        if let Some((_,left))=self.points.iter_mut().find(|(p,n)|*p==pc&&*n!=0){
            *left-=1;println!("trace_pc at={} pc={pc:#010x} a={:?} s={:?} sp={:#010x}",cx.cycles,&cpu.x[10..18],[cpu.x[8],cpu.x[9],cpu.x[18],cpu.x[19]],cpu.x[2]);
            if self.mspi{println!("trace_mspi at={} pc={pc:#010x} {}",cx.cycles,bus.psram_command_debug());}
            if self.policy{if let Some(p)=cpu.c5_memory_protection(){println!("trace_policy at={} pc={pc:#010x} {:?}",cx.cycles,p.snapshot());}}
        }None
    }
}
fn main()->Result<(),Box<dyn std::error::Error>>{
    let a:Vec<_>=std::env::args().collect();
    if !matches!(a.len(),7|8){return Err("usage: factory_machine board flash-MiB psram-MiB rom.elf factory.bin budget [sd.img]".into());}
    let flash=a[2].parse::<usize>()?.checked_mul(1<<20).ok_or("capacity overflow")?;
    let psram=a[3].parse::<usize>()?.checked_mul(1<<20).ok_or("capacity overflow")?;
    if !matches!(flash,0x400000|0x800000|0x1000000|0x2000000){return Err("unsupported Flash capacity".into());}
    let mut m=esp32c5::rom_machine([0;6],flash);m.bus.set_xtal_mhz(48)?;m.bus.configure_psram(psram)?;
    m.bus.attach_board(esp32c5::board::make_board(&a[1]).ok_or("unknown board")?);
    if let Some(p)=a.get(7){m.bus.attach_sd_card(Box::new(esp_periph::sdmmc::MemStore(std::fs::read(p)?)),Default::default())?;}
    m.load_rom(&std::fs::read(&a[4])?)?;m.load_input(esp_soc::LoadKind::Flash,&std::fs::read(&a[5])?)?;
    if let Ok(spec)=std::env::var("C5_FACTORY_TRACE_PCS"){
        let pcs:Result<Vec<_>,_>=spec.split(',').map(|p|u32::from_str_radix(p.trim().trim_start_matches("0x"),16).map(|pc|(pc,4))).collect();
        let points=pcs?;if points.len()>32{return Err("C5_FACTORY_TRACE_PCS accepts at most32 PCs".into());}
        let arm=std::env::var("C5_FACTORY_TRACE_ARM_PC").ok().map(|s|u32::from_str_radix(s.trim().trim_start_matches("0x"),16)).transpose()?;
        m.add_observer(Box::new(TracePcs{points,arm,mspi:std::env::var_os("C5_FACTORY_TRACE_MSPI").is_some(),policy:std::env::var_os("C5_FACTORY_TRACE_POLICY").is_some()}));
    }
    if let Ok(pc)=std::env::var("C5_FACTORY_STOP_PC"){
        let pc=u32::from_str_radix(pc.trim().trim_start_matches("0x"),16)?;
        m.add_observer(Box::new(esp_soc::observers::Breakpoints{pcs:vec![pc]}));
    }
    if let Ok(profile)=std::env::var("C5_WIFI_DRIVER_COMPAT"){esp32c5::enable_wifi_driver_compat(&mut m,&profile)?;}
    m.console.capture=true;m.boot_rom();let stop=m.run(a[6].parse()?);
    let frame=m.bus.board.display();let nonzero=frame.as_ref().map_or(0,|(_,_,v,_)|v.iter().filter(|&&n|n!=0).count());
    if let Ok(path)=std::env::var("C5_FACTORY_FRAME_OUT"){if let Some((width,height,pixels,_))=&frame{
        let bytes=pixels.iter().flat_map(|p|p.to_le_bytes()).collect::<Vec<_>>();use std::io::Write;let mut output=std::fs::OpenOptions::new().write(true).create_new(true).open(path)?;output.write_all(&bytes)?;
        println!("captured_rgb565 width={width} height={height} bytes={}",bytes.len());
    }}
    println!("stop={stop:?} steps={} insns={} cycles={} pc={:#010x} firstFault={:?} uartBytes={} nonzeroPixels={nonzero}",m.run_steps(),m.insns(),m.bus.cycles(),m.cores[0].pc,m.bus.first_fault,m.console.uart0.len()+m.bus.uart[0].tx_out.len());
    println!("trap_csrs mepc={:#010x} mcause={:#010x} mtval={:#010x}",m.cores[0].mepc,m.cores[0].mcause,m.cores[0].mtval);
    if let Some(e)=m.cores[0].c5_memory_unsupported(){println!("csr_model_limit {e:?}");}
    if let Some(e)=m.bus.memory_policy_unsupported(){println!("memory_model_limit {e:?}");}
    if std::env::var_os("C5_FACTORY_TRACE_MSPI").is_some(){println!("final_mspi {}",m.bus.psram_command_debug());}
    if std::env::var_os("C5_FACTORY_TRACE_USB").is_some(){println!("final_usb {}",m.bus.detached_usb_debug());}
    if std::env::var_os("C5_FACTORY_TRACE_SPI").is_some(){println!("final_spi {}",m.bus.spi_transport_debug());}
    if std::env::var_os("C5_FACTORY_TRACE_POLICY").is_some(){if let Some(p)=m.cores[0].c5_memory_protection(){println!("final_policy {:?}",p.snapshot());}}
    if let Some(f)=m.bus.first_fault.clone().filter(|f|f.write){
        // Decode only actual RV32IMAC store source registers for diagnostics.
        // Trap entry changes CSRs/PC, not these general-purpose registers.
        if let Ok(lo)=m.bus.read16(f.pc){
            let source=if lo&3==3{
                m.bus.read16(f.pc+2).ok().and_then(|hi|{let word=u32::from(lo)|(u32::from(hi)<<16);(word&0x7f==0x23).then_some(((word>>20)&31)as usize)})
            }else{match(lo&3,lo>>13){(0,6)=>Some(8+((lo>>2)&7)as usize),(2,6)=>Some(((lo>>2)&31)as usize),_=>None}};
            if let Some(reg)=source{println!("fault_store x{reg}={:#010x}",m.cores[0].x[reg]);}
        }
    }
    if m.bus.first_fault.as_ref().is_some_and(|f|f.pc==0x4002_1216&&f.address==0x600b_1c00){
        // Fixed original-ROM wdt_hal_config_stage store: sw a3,0(a5).
        println!("wdt_config_request={:#010x} prior={:?}",m.cores[0].x[13],m.bus.read32(0x600b_1c00));
    }
    if m.bus.first_fault.is_some(){
        for address in[0x6009_6018,0x6009_601c,0x6000_3008,0x6000_3018,0x6000_3020,0x6000_3024,0x6000_3028,0x6000_3034,0x6000_30d4,0x600b_1c00,0x600b_1c04,0x600b_1c18,0x6000_8048,0x6000_8064]{
            println!("probe_register {address:#010x}={:?}",m.bus.read32(address));
        }
    }
    if let Some(snapshot)=m.bus.wifi_driver_compat_snapshot(){println!("wifi_driver_compat {snapshot}");}
    let mut uart=m.console.uart0.clone();uart.extend_from_slice(&m.bus.uart[0].tx_out);
    println!("UART={:?}\n{}",String::from_utf8_lossy(&uart),m.bus.report());Ok(())
}
