//! Reusable pre/post-change S2 baseline. Only the original SRAM address aliases
//! and integer interpreter are used; copy this example unchanged into both
//! checkouts before their isolated builds. No PSRAM relative-performance claim.
use emu_core::{Bus,Core};
use esp_soc::SocBus;
use esp32s2::{S2Bus,S2Core,bus::{IRAM_BASE,DRAM_BASE}};
fn main() {
    let instructions=std::env::args().nth(1).map_or(40_000_000,|v|v.parse::<u32>().expect("instruction count"));
    assert!(instructions>0&&instructions.is_multiple_of(4),"positive u32 multiple of4");
    // l32i a3,a4,0; addi a3,a3,1; s32i a3,a4,0; j -13.
    // Xtensa RRI8/J encodings; the original S2 core supports every instruction.
    let words=[0x002432u32,0x01c332,0x006432,6|(((-13i32)as u32&0x3ffff)<<6)];
    let program:Vec<_>=words.iter().flat_map(|w|w.to_le_bytes()[..3].to_vec()).collect();
    let mut bus=S2Bus::new();bus.load_bytes(IRAM_BASE,&program).unwrap();
    let mut cpu=S2Core::new();cpu.pc=IRAM_BASE;cpu.ps=0;cpu.set_ar(4,DRAM_BASE+0x1000);cpu.set_jit(false);
    let start=std::time::Instant::now();let (executed,trap)=cpu.run(&mut bus,instructions);let seconds=start.elapsed().as_secs_f64();
    assert_eq!(executed,instructions);assert!(trap.is_none(),"{trap:?}");assert_eq!(cpu.pc,IRAM_BASE);
    let checksum=bus.read32(DRAM_BASE+0x1000).unwrap();assert_eq!(checksum,instructions/4);
    assert_eq!(bus.read32(IRAM_BASE+0x1000),Ok(checksum));assert!(bus.first_fault.is_none());
    println!("{{\"workload\":\"s2-sram-l32i-addi-s32i-j-v1\",\"instructions\":{executed},\"checksum\":{checksum},\"pc\":{},\"elapsedSeconds\":{seconds:.9},\"psramEnabled\":false,\"jitEnabled\":false}}",cpu.pc);
}
