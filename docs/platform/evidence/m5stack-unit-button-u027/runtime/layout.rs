// Native public-field layout probe. Compile against each release build's C6 rlib;
// this does not instantiate a machine or change guest execution.
use std::mem::{size_of,align_of,offset_of};
use esp32c6::{Machine,bus::SocBus,periph::Peripherals};
fn main() {
    println!("Machine size={} align={}",size_of::<Machine>(),align_of::<Machine>());
    println!("SocBus size={} align={}",size_of::<SocBus>(),align_of::<SocBus>());
    println!("Peripherals size={} align={}",size_of::<Peripherals>(),align_of::<Peripherals>());
    macro_rules! fields {($ty:ty,$($field:ident),*)=>{$(println!(concat!(stringify!($ty),".",stringify!($field),"={}"),offset_of!($ty,$field));)*};}
    fields!(Machine,mac,reboots,stubs,stub_hits,fn_probes,cores,quantum,bus,symbols,dbg,observers,exceptions,interrupts,irq_hist,script,max_cycles,console,web,camera_input,web_restart,rt,vq_max,vq_stats,bb_max,bb_stats);
    fields!(SocBus,ble,rom,sram,lp_sram,flash,mmu,mmu_index,mmu_power_ctrl,periph,board,cycles,last_fault,irq_dirty,gpio_events,debug);
    fields!(Peripherals,adc,uart,usb,systimer,timg,gpio,io_mux,i2c,efuse,spi0,spi1,sha,aes,rsa,rmt,gdma,spi2,radio,modem_bb,wifi_mac,intmtx,intc,cache,lpsys,pcr,ana_mst,assist_debug,rng,cpu_sub,misc,spi_exec,ledc,mcpwm,optional,work_pending);
}
