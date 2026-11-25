//! C5 transport component: caller-supplied rates, cancellation and phase boundaries.
use super::*;
use emu_core::Bus;
use esp_soc::BoardModel;
use std::{cell::RefCell,rc::Rc};
const SPI:u32=0x6008_1000;const LEDC:u32=0x6000_7000;const SPI_CLK:u32=0x6009_60c8;const LEDC_CLK:u32=0x6009_604c;
// The fixture supplies Transport's source observations, not analog/BootClock readiness.
// Exercise Transport-owned registers directly; C5Bus owns GPIO/memory, DMA and
// board callbacks. Bus clock-provider integration has separate native tests.
fn component_write(b:&mut C5Bus,a:u32,v:u32)->bool{
 assert!(Transport::contains(a));let mut t=b.transport.take().unwrap_or_else(||Box::new(Transport::new(b.cycles)));
 let ok=t.write(a,v,b);if ok{t.pump(b);}b.transport=Some(t);b.irq_dirty=true;ok
}
fn put(b:&mut C5Bus,a:u32,v:u32){if Transport::contains(a){assert!(component_write(b,a,v),"component write {a:#x}");}else{b.write32(a,v).unwrap();}}
fn get(b:&mut C5Bus,a:u32)->u32{
 if !Transport::contains(a){return b.read32(a).unwrap();}
 let mut t=b.transport.take().unwrap_or_else(||Box::new(Transport::new(b.cycles)));
 let value=t.read(a,b);t.pump(b);b.transport=Some(t);b.irq_dirty=true;value.expect("component read")
}
fn matrix(b:&mut C5Bus,p:u32,s:u32){put(b,0x6009_0000+4*p,0x1800);put(b,0x6009_1ad4+4*p,s);put(b,0x6009_1038,1<<p);}
fn observe_sources(b:&mut C5Bus,rc:Option<u32>,pll:bool){
 observe_sources_with_gates(b,rc,pll,0x1ff);
}
fn observe_sources_with_gates(b:&mut C5Bus,rc:Option<u32>,pll:bool,gates:u16){
 let mut t=b.transport.take().unwrap_or_else(||Box::new(Transport::new(b.cycles)));
 t.set_clock_sources(b.cycles,rc,pll,gates);t.publish_pwm(b);b.transport=Some(t);b.irq_dirty=true;
}
type Calls=Rc<RefCell<Vec<Vec<u8>>>>;
struct Capture(Calls);
impl BoardModel for Capture{
 fn name(&self)->&'static str{"c5-observed-source-fixture"}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,_host:u8,_p:SpiQspiPins,tx:&[u8],n:usize)->Vec<u8>{self.0.borrow_mut().push(tx.to_vec());vec![0xa5;n]}
}
fn spi_fixture()->(C5Bus,Calls){
 let calls=Rc::new(RefCell::new(Vec::new()));let mut b=C5Bus::new();b.attach_board(Box::new(Capture(calls.clone())));
 for(p,s)in[(7,56),(6,58),(23,61)]{matrix(&mut b,p,s);}
 for(a,v)in[(SPI+0xc,1<<12),(SPI+0x10,1<<27),(SPI+0x98,0x04030201),(SPI+0x1c,31)]{put(&mut b,a,v);}(b,calls)
}
fn ledc_fixture(duty:bool)->C5Bus{
 let mut b=C5Bus::new();b.attach_board(crate::board::make_board("waveshare-c5-lcd147").unwrap());matrix(&mut b,10,0);
 for(a,v)in[(0x6009_6048,1),(0x6009_6050,2),(LEDC_CLK,1<<22),(LEDC+0xa0,10|(256<<5)|(1<<26))]{put(&mut b,a,v);}
 if duty{for(a,v)in[(LEDC+8,512<<4),(LEDC+12,1<<31),(LEDC,20),(LEDC+0xc8,1<<4)]{put(&mut b,a,v);}}b
}

#[test]
fn non_xtal_sources_require_actual_observations_and_zero_is_unavailable(){
 let(mut b,calls)=spi_fixture();let original=b.transport.as_ref().unwrap().report();
 for sel in 1..=3{put(&mut b,SPI_CLK,(1<<22)|(sel<<20));assert!(!component_write(&mut b,SPI,1<<24));}
 assert_eq!(b.transport.as_ref().unwrap().rejected,3);assert!(calls.borrow().is_empty());assert_eq!(get(&mut b,SPI+0x3c),0);
 observe_sources(&mut b,Some(0),false);assert!(!b.transport.as_ref().unwrap().precise_ledc);assert_eq!(b.transport.as_ref().unwrap().rc_fast_hz,None);
 observe_sources(&mut b,Some(1),true);put(&mut b,SPI_CLK,(1<<22)|(2<<20)|(255<<12));assert!(!component_write(&mut b,SPI,1<<24),"an unrepresentable divided source must not divide by zero");
 let mut l=ledc_fixture(false);for sel in 1..=3{put(&mut l,LEDC_CLK,(1<<22)|(sel<<20));assert_eq!(l.transport.as_ref().unwrap().ledc.external_clock_hz,0);}
 observe_sources(&mut l,Some(17_500_000),true);assert_eq!(l.transport.as_ref().unwrap().ledc.external_clock_hz,0,"LEDC mux3 remains unsupported with PLL ready");
 assert_eq!(original,"SPI2=0 transfers rejected=0 serial-time-floor; GDMA3/full32-bit; LEDC6/static-duty XTAL-only");
}

#[test]
fn serial_floor_uses_each_observed_source_and_pcr_divider(){
 // 32 data bits / SPI_CLOCK divide2: 64 selected-source clocks, 240MHz virtual scheduler.
 for(sel,div,duration)in[(0,0,320),(1,0,96),(2,0,878),(3,0,128),(1,3,384),(3,255,32768)]{
  let(mut b,calls)=spi_fixture();observe_sources(&mut b,Some(17_500_000),true);put(&mut b,SPI_CLK,(1<<22)|(sel<<20)|(div<<12));put(&mut b,SPI,1<<24);
  assert_eq!(b.transport.as_ref().unwrap().deadline,Some(duration));b.advance(duration-1);assert!(calls.borrow().is_empty());assert_eq!(get(&mut b,SPI+0x3c),0);
  b.advance(1);assert_eq!(&*calls.borrow(),&[vec![1,2,3,4]]);assert_ne!(get(&mut b,SPI+0x3c)&(1<<12),0);
 }
}

#[test]
fn inactive_slave_clear_policies_allow_master_bytes_only_at_the_serial_deadline(){
 for flags in[0,1<<19,1<<20,3<<19]{
  let(mut b,calls)=spi_fixture();put(&mut b,SPI+0x10,(1<<27)|(1<<28)|1);put(&mut b,SPI+0x30,flags);
  assert_eq!(get(&mut b,SPI+0x30)&(3<<19),flags);assert!(calls.borrow().is_empty());assert_eq!(get(&mut b,SPI+0x3c),0);
  put(&mut b,SPI,1<<24);assert_eq!(b.transport.as_ref().unwrap().rejected,0);assert_eq!(get(&mut b,SPI)&(1<<24),1<<24);
  b.advance(319);assert!(calls.borrow().is_empty());assert_eq!(get(&mut b,SPI+0x3c),0);
  b.advance(1);assert_eq!(&*calls.borrow(),&[vec![1,2,3,4]]);assert_eq!(get(&mut b,SPI+0x98),0xa5a5_a5a5);
  assert_eq!(get(&mut b,SPI)&(1<<24),0);assert_eq!(get(&mut b,SPI+0x3c),1<<12);
 }
}

#[test]
fn active_segment_slave_qpi_subbyte_and_cpu_overflow_modes_stay_rejected(){
 for case in 0..5{
  let(mut b,calls)=spi_fixture();put(&mut b,SPI+0x30,3<<19);
  match case{0=>put(&mut b,SPI+0x30,(3<<19)|(1<<18)),1=>put(&mut b,SPI+0xe0,1<<26),
   2=>put(&mut b,SPI+0x10,(1<<27)|8),3=>put(&mut b,SPI+0x1c,30),_=>put(&mut b,SPI+0x1c,519)}
  assert!(!component_write(&mut b,SPI,1<<24));assert_eq!(b.transport.as_ref().unwrap().rejected,1);
  b.advance(1000);assert!(calls.borrow().is_empty());assert!(b.transport.as_ref().unwrap().pending.is_none());assert_eq!(get(&mut b,SPI+0x3c),0);
 }
}

#[test]
fn inactive_clear_policies_do_not_supply_a_missing_clock(){
 let(mut b,calls)=spi_fixture();put(&mut b,SPI+0x30,3<<19);put(&mut b,SPI_CLK,(1<<22)|(1<<20));
 assert!(!component_write(&mut b,SPI,1<<24));b.advance(1000);assert!(calls.borrow().is_empty());
 assert_eq!(get(&mut b,SPI+0x3c),0);assert!(b.transport.as_ref().unwrap().pending.is_none());
}

#[test]
fn inactive_clear_policies_do_not_replace_missing_dma_data_with_fifo_bytes(){
 let(mut b,calls)=spi_fixture();put(&mut b,SPI+0x30,(3<<19)|(1<<28));put(&mut b,SPI,1<<24);
 assert!(b.transport.as_ref().unwrap().spi.dma_tx_pending.is_some());assert_eq!(b.transport.as_ref().unwrap().deadline,None);
 b.advance(1000);assert!(calls.borrow().is_empty());assert_eq!(get(&mut b,SPI+0x3c),0);
 put(&mut b,0x6009_60c0,3);assert!(b.transport.as_ref().unwrap().pending.is_none());
 assert_eq!(get(&mut b,SPI)&(1<<24),0);assert_eq!(get(&mut b,SPI+0x3c),0);
}

#[test]
fn cold_spi_diagnostics_do_not_advance_or_complete_a_pending_transfer(){
 let(mut b,calls)=spi_fixture();put(&mut b,SPI+0x30,3<<19);put(&mut b,SPI,1<<24);
 let before=(b.cycles,b.transport.as_ref().unwrap().deadline,get(&mut b,SPI),get(&mut b,SPI+0x3c));
 for _ in 0..3{assert!(b.spi_transport_debug().contains("pending=true"));}
 assert_eq!((b.cycles,b.transport.as_ref().unwrap().deadline,get(&mut b,SPI),get(&mut b,SPI+0x3c)),before);assert!(calls.borrow().is_empty());
 b.advance(320);assert_eq!(calls.borrow().len(),1);
}

#[test]
fn selected_source_changes_cancel_whole_spi_transactions_without_forged_done(){
 let(mut b,calls)=spi_fixture();observe_sources(&mut b,Some(17_500_000),true);let clock=(1<<22)|(2<<20)|(3<<12);put(&mut b,SPI_CLK,clock);put(&mut b,SPI,1<<24);b.advance(2);
 observe_sources(&mut b,Some(18_000_000),true);assert_eq!(get(&mut b,SPI_CLK),clock);assert_eq!(get(&mut b,SPI)&(1<<24),0);assert_eq!(get(&mut b,SPI+0x3c),0);
 assert!(b.transport.as_ref().unwrap().pending.is_none());assert_eq!(b.transport.as_ref().unwrap().deadline,None);assert_eq!(b.transport.as_ref().unwrap().rejected,0);
 b.advance(10_000);assert!(calls.borrow().is_empty());put(&mut b,SPI,1<<24);let due=b.transport.as_ref().unwrap().deadline.unwrap();
 observe_sources(&mut b,Some(18_000_000),false);assert_eq!(b.transport.as_ref().unwrap().deadline,Some(due),"an unrelated PLL loss cannot cancel RC_FAST");b.advance(due-b.cycles);assert_eq!(calls.borrow().len(),1);
 put(&mut b,SPI+0x3c,1<<12);observe_sources(&mut b,Some(18_000_000),true);put(&mut b,SPI_CLK,(1<<22)|(1<<20));put(&mut b,SPI,1<<24);b.advance(1);observe_sources(&mut b,Some(18_000_000),false);
 assert_eq!(get(&mut b,SPI_CLK),(1<<22)|(1<<20));assert_eq!(get(&mut b,0x6009_60c4),5);assert_eq!(get(&mut b,SPI+0x3c),0);b.advance(1000);assert_eq!(calls.borrow().len(),1);
 assert!(!component_write(&mut b,SPI,1<<24));assert_eq!(b.transport.as_ref().unwrap().rejected,1);
}

#[test]
fn source_loss_also_cancels_a_dma_request_that_has_no_wire_deadline(){
 let(mut b,calls)=spi_fixture();observe_sources(&mut b,None,true);put(&mut b,SPI_CLK,(1<<22)|(3<<20));put(&mut b,SPI+0x30,1<<28);put(&mut b,SPI,1<<24);
 assert!(b.transport.as_ref().unwrap().spi.dma_tx_pending.is_some());assert_eq!(b.transport.as_ref().unwrap().deadline,None);
 observe_sources(&mut b,None,false);assert!(b.transport.as_ref().unwrap().spi.dma_tx_pending.is_none());assert!(b.transport.as_ref().unwrap().pending.is_none());
 b.advance(1000);assert!(calls.borrow().is_empty());assert_eq!(get(&mut b,SPI+0x3c),0);assert_eq!(get(&mut b,0x6008_0030),0);
}

#[test]
fn unchanged_default_observations_preserve_exact_legacy_xtal_progression(){
 let mut b=ledc_fixture(true);b.advance(1);observe_sources(&mut b,None,false);b.advance(1);observe_sources(&mut b,None,false);
 let t=b.transport.as_ref().unwrap();assert!(!t.precise_ledc);assert_eq!(t.ledc_remainder,2);assert_eq!(t.deadline,Some(5121));
 b.advance(5118);assert_eq!(get(&mut b,LEDC+16),0);b.advance(1);assert_eq!(get(&mut b,LEDC+16),512<<4);assert!(b.board.report().contains("duty=32768"));
}

#[test]
fn one_and_two_cpu_cycle_boundaries_keep_independent_phase_integrals(){
 for prefix in [1,2]{for(sel,hz)in[(2,80_000_000u32),(1,17_500_000)]{
  let mut b=ledc_fixture(false);b.advance(prefix);let(rc,pll)=if sel==1{(Some(hz),false)}else{(None,true)};observe_sources(&mut b,rc,pll);put(&mut b,LEDC_CLK,(1<<22)|(sel<<20));
  // Independent units: sum(elapsed CPU cycles * source Hz) / 240MHz.
  let mut numerator=u128::from(prefix)*48_000_000;
  for _ in 0..1000{b.advance(1);numerator+=u128::from(hz);assert_eq!(get(&mut b,LEDC+0xa4),((numerator/240_000_000)%1024)as u32);assert_eq!(b.transport.as_ref().unwrap().ledc_fraction,(numerator%3)as u64);}
  b.advance(2);numerator+=2*u128::from(hz);observe_sources(&mut b,None,false);let stopped=get(&mut b,LEDC+0xa4);b.advance(777);assert_eq!(get(&mut b,LEDC+0xa4),stopped);
  assert_eq!(b.transport.as_ref().unwrap().ledc_fraction,(numerator%3)as u64);observe_sources(&mut b,rc,pll);
  b.advance(1);numerator+=u128::from(hz);put(&mut b,LEDC_CLK,sel<<20);b.advance(2);assert_eq!(get(&mut b,LEDC+0xa4),((numerator/240_000_000)%1024)as u32);
  put(&mut b,LEDC_CLK,(1<<22)|(sel<<20));b.advance(2);numerator+=2*u128::from(hz);assert_eq!(get(&mut b,LEDC+0xa4),((numerator/240_000_000)%1024)as u32);
  assert_eq!(b.transport.as_ref().unwrap().ledc_fraction,(numerator%3)as u64);
 }}
}

#[test]
fn pwm_latches_at_the_new_clock_wrap_and_publishes_the_actual_frequency(){
 for prefix in [1,2]{for(sel,hz)in[(2,80_000_000u64),(1,17_500_000)]{
  let mut b=ledc_fixture(true);b.advance(prefix);observe_sources(&mut b,Some(17_500_000),true);put(&mut b,LEDC_CLK,(1<<22)|(sel<<20));
  let remaining=(1024*240_000_000-prefix*48_000_000).div_ceil(hz);
  b.advance(remaining-1);assert_eq!(get(&mut b,LEDC+16),0);assert_eq!(get(&mut b,LEDC+0xc4),0);assert!(b.board.report().contains("duty=0"));
  b.advance(1);assert_eq!(get(&mut b,LEDC+16),512<<4);assert_eq!(get(&mut b,LEDC+0xc4),1<<4);assert!(b.board.report().contains("duty=32768"));
  let(frequency,duty)=b.transport.as_ref().unwrap().ledc.output(&b.gpio,10).unwrap();assert!((frequency-hz as f64/1024.0).abs()<1e-9);assert_eq!(duty,32768);
  observe_sources(&mut b,None,false);assert!(b.board.report().contains("duty=0"));observe_sources(&mut b,Some(17_500_000),true);assert!(b.board.report().contains("duty=32768"));
 }}
}

#[test]
fn pending_pwm_duty_stops_without_an_irq_and_replans_only_after_source_returns(){
 for(sel,hz)in[(2,80_000_000u64),(1,17_500_000)]{
  let mut b=ledc_fixture(true);b.advance(1);let(rc,pll)=if sel==1{(Some(hz as u32),false)}else{(None,true)};
  observe_sources(&mut b,rc,pll);put(&mut b,LEDC_CLK,(1<<22)|(sel<<20));b.advance(2);observe_sources(&mut b,None,false);
  assert_eq!(b.transport.as_ref().unwrap().deadline,None);b.advance(777);assert_eq!(get(&mut b,LEDC+16),0);assert_eq!(get(&mut b,LEDC+0xc4),0);
  assert!(b.board.report().contains("duty=0"));observe_sources(&mut b,rc,pll);
  let remaining=(1024*240_000_000-48_000_000-2*hz).div_ceil(hz);assert!(b.transport.as_ref().unwrap().deadline.is_some());
  b.advance(remaining-1);assert_eq!(get(&mut b,LEDC+16),0);assert_eq!(get(&mut b,LEDC+0xc4),0);
  b.advance(1);assert_eq!(get(&mut b,LEDC+16),512<<4);assert_eq!(get(&mut b,LEDC+0xc4),1<<4);assert!(b.board.report().contains("duty=32768"));
 }
}

#[test]
fn pll_divider_mask_never_grants_ready_and_each_spi_source_needs_its_own_tap(){
 for(sel,bit)in[(1,1),(3,2)]{
  let(mut b,calls)=spi_fixture();put(&mut b,SPI_CLK,(1<<22)|(sel<<20));
  observe_sources_with_gates(&mut b,None,false,0x1ff);assert!(!component_write(&mut b,SPI,1<<24));
  observe_sources_with_gates(&mut b,None,true,0x1ff&!(1<<bit));assert!(!component_write(&mut b,SPI,1<<24));
  assert_eq!(get(&mut b,SPI+0x3c),0);assert!(calls.borrow().is_empty());assert!(b.transport.as_ref().unwrap().pending.is_none());
  observe_sources_with_gates(&mut b,None,true,0x20|(1<<bit));assert!(component_write(&mut b,SPI,1<<24));
  let due=b.transport.as_ref().unwrap().deadline.unwrap();b.advance(due-b.cycles);assert_eq!(calls.borrow().len(),1);
 }
}

#[test]
fn selected_pll_tap_loss_cancels_spi_but_unrelated_tap_loss_preserves_the_transfer(){
 for(sel,bit,unrelated)in[(1,1,2),(3,2,1)]{
  let(mut b,calls)=spi_fixture();observe_sources(&mut b,None,true);put(&mut b,SPI_CLK,(1<<22)|(sel<<20));put(&mut b,SPI,1<<24);b.advance(1);
  let due=b.transport.as_ref().unwrap().deadline;observe_sources_with_gates(&mut b,None,true,0x1ff&!(1<<unrelated));assert_eq!(b.transport.as_ref().unwrap().deadline,due);
  observe_sources_with_gates(&mut b,None,true,0x1ff&!(1<<bit));assert!(b.transport.as_ref().unwrap().pending.is_none());assert_eq!(b.transport.as_ref().unwrap().deadline,None);
  assert_eq!(get(&mut b,SPI)&(1<<24),0);assert_eq!(get(&mut b,SPI+0x3c),0);b.advance(1000);assert!(calls.borrow().is_empty());
  assert!(!component_write(&mut b,SPI,1<<24));observe_sources_with_gates(&mut b,None,true,0x20|(1<<bit));assert!(component_write(&mut b,SPI,1<<24));
  let due=b.transport.as_ref().unwrap().deadline.unwrap();b.advance(due-b.cycles);assert_eq!(calls.borrow().len(),1);
 }
}

#[test]
fn pll80_gate_settles_old_ledc_edges_pauses_without_irq_and_resumes_saved_phase(){
 for prefix in[1,2]{
  let mut b=ledc_fixture(false);observe_sources(&mut b,None,true);put(&mut b,LEDC_CLK,(1<<22)|(2<<20));b.advance(prefix);
  let mut numerator=u128::from(prefix)*80_000_000;observe_sources_with_gates(&mut b,None,true,0x1f7);
  let stopped=get(&mut b,LEDC+0xa4);assert_eq!(stopped,((numerator/240_000_000)%1024)as u32);assert_eq!(b.transport.as_ref().unwrap().ledc_fraction,(numerator%3)as u64);
  b.advance(777);assert_eq!(get(&mut b,LEDC+0xa4),stopped);assert_eq!(get(&mut b,LEDC+0xc4),0);
  observe_sources_with_gates(&mut b,None,true,0x28);b.advance(1);numerator+=80_000_000;
  assert_eq!(get(&mut b,LEDC+0xa4),((numerator/240_000_000)%1024)as u32);assert_eq!(b.transport.as_ref().unwrap().ledc_fraction,(numerator%3)as u64);
  // Re-enable an unrelated160M tap;80M's elapsed edge/phase history continues.
  observe_sources_with_gates(&mut b,None,true,0x2a);b.advance(2);numerator+=160_000_000;
  assert_eq!(get(&mut b,LEDC+0xa4),((numerator/240_000_000)%1024)as u32);
 }
}
