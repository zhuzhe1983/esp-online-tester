//! Explicit, fixed-image, offline WiFi API substitute. Not a MAC/PHY/RF model.
//! ABI: ESP-IDF5.5.3@2c211b236707889e8400c4dc5644dd5c4ee071e0,
//! components/esp_wifi/include/{esp_wifi.h,esp_wifi_types_generic.h},
//! components/esp_event/esp_event.c and the fixed LCD273 vendor image.
//! API entries are matched by source callsites and vendor/library bytes, not
//! symbols from a reconstructed ELF. No guest instruction or ROM is patched.
use crate::{C5Bus,Machine};
use emu_core::Bus;
use riscv_rv32::Cpu;

pub const PROFILE:&str="waveshare-lcd273-idf553-offline-v1";
const OK:u32=0;const ARG:u32=0x102;
const NOT_INIT:u32=0x3001;const NOT_STARTED:u32=0x3002;const IFACE:u32=0x3004;
const MODE:u32=0x3005;const STATE:u32=0x3006;const SSID:u32=0x300a;const PASSWORD:u32=0x300b;const INIT_STATE:u32=0x3013;const NOT_STOPPED:u32=0x3003;const NOT_CONNECT:u32=0x300f;
const EVENT_POST:u32=0x4211_e894;const WIFI_EVENT:u32=0x4214_0704;
const INIT:u32=0x4209_00c8;const SET_MODE:u32=0x420d_164e;const SET_CONFIG:u32=0x420d_1c8c;
const START:u32=0x420d_169e;const CONNECT:u32=0x420d_1884;const GET_MAC:u32=0x420d_0dd4;
const REG_BUF:u32=0x420d_21f0;const GET_MODE:u32=0x420d_0d9e;
const STOP:u32=0x420d_16e2;const DEINIT:u32=0x4209_00b4;const DISCONNECT:u32=0x4209_0384;
const SCAN_START:u32=0x420d_1a36;const SCAN_RECORDS:u32=0x420d_1be8;
const GET_CONFIG:u32=0x420d_1d20;
const HOOKS:[u32;14]=[INIT,GET_MODE,GET_MAC,SET_MODE,START,CONNECT,SET_CONFIG,REG_BUF,STOP,DEINIT,DISCONNECT,SCAN_START,SCAN_RECORDS,GET_CONFIG];
const API_NAMES:[&str;14]=["esp_wifi_init","esp_wifi_get_mode","esp_wifi_get_mac","esp_wifi_set_mode","esp_wifi_start","esp_wifi_connect","esp_wifi_set_config","esp_wifi_internal_reg_netstack_buf_cb","esp_wifi_stop","esp_wifi_deinit","esp_wifi_disconnect","esp_wifi_scan_start","esp_wifi_scan_get_ap_records","esp_wifi_get_config"];
const ENTRIES:[&str;14]=[
 "01114ac837f983408347493f06ce22cc26ca6391072a3c5d2a84898bd5e7ef20",
 "411122c406c62a84eff05fc115c11305201019c8eff09fb883a7012283c70700",
 "411122c426c206c62a84ae84eff0bfbd05c9894763e987021305201089cceff0",
 "411122c406c62a84eff04fb60dc9b707864083a707f6614583a74717829715c5",
 "411106c6eff08fb115c5b707864083a707f6614583a74717829715c137f70c42",
 "411106c6eff02f930dcdb7e7834003c7a790854763fce702b707864083a707f6",
 "411126c24ac006c622c42a89ae84efe01fd225c1894763e62707bdc4b7078640",
 "b7170d427971938707872ac62ec83eca48087c00854506d63ecc02ceeff07fdf",
 "797106d622d426d24ad04ece52cc56ca5ac85ec662c466c2eff00fa405e9b709",
 "b7f7834083c7473f99c36ff05fee0d650505828001114ac837f983408347493f",
 "6f10845501458280828001458280014582808280014582800145828001458280",
 "011126ca52c406ce22cc4ac84ec656c2aa842e8aefe01ff763070518b7e78340",
 "411122c426c206c62a84ae84efe07fdc49c1b7d7834013878770834627200547",
 "411122c426c206c6aa842e84efe0ffc815c9894763ef970221c081e822852244",
];
const POST_ENTRY:&str="ba8737f783402a880325473019c53687b2862e86c2856f00f02b130530108280";
const RANGES:[(usize,usize,&str);3]=[
 (0x2000,0x8000,"3ce04f6bd5152da65dcc375b9ff8872714af9fb128b9519bb99a8184f93ba190"),
 (0x8000,0x9000,"ec03ffe0e58a44ea48920963b3dd0e8b8a40f7b414fed5c2a44895843bea3baf"),
 (0x20000,0x331230,"69d6362c81efc6d55048723a1af0c864b26e1990b6a8028d8fc127072bbd5f28"),
];
fn hex(bytes:&[u8])->String{bytes.iter().map(|v|format!("{v:02x}")).collect()}
fn digest(bytes:&[u8])->String{hex(&crate::image::sha256(bytes))}
fn pinned(bus:&C5Bus)->Result<(),String>{
 if !bus.has_validated_rom(){return Err("compat requires the validated MP rev100 ROM".into());}
 if bus.board.name()!="waveshare-c5-lcd273"||bus.flash.len()!=32<<20||bus.psram.len()!=8<<20{return Err("compat requires the exact LCD273 32MiB/8MiB profile".into());}
 for &(a,z,sha) in &RANGES{if digest(bus.flash.get(a..z).ok_or("compat image is truncated")?)!=sha{return Err("compat immutable bootloader/partition/application identity mismatch".into());}}
 Ok(())
}
/// Select only after ROM and Flash have been loaded and restored. Mutable NVS,
/// otadata and PHY-data partitions are not part of the immutable API identity.
/// This does not change firmware bytes, persistence or any MAC register.
pub fn enable_wifi_driver_compat(m:&mut Machine,profile:&str)->Result<(),String>{
 if profile!=PROFILE{return Err("unknown C5 WiFi driver compatibility profile".into());}
 if m.bus.wifi_compat.is_some(){return Err("C5 WiFi compatibility already selected; use a fresh machine".into());}
 if m.run_steps()!=0{return Err("select C5 WiFi compatibility before executing any guest instruction".into());}
 pinned(&m.bus)?;m.bus.wifi_compat=Some(Box::new(WifiCompat::new()));Ok(())
}
#[derive(Clone)]
struct Pending{pc:u32,sp:u32,registers:[u32;32],saved:Vec<u8>,event:u8,lifecycle:u64}
pub(crate) struct WifiCompat{
 initialized:bool,started:bool,mode:u32,config:Option<[u8;184]>,callbacks:Option<[u32;2]>,
 counts:[u64;14],lifecycle:u64,returns:u64,event_posts:u64,event_errors:u64,attempts:u64,failures:u64,
 pending:Vec<Pending>,limit:Option<String>,epochs:u64,
}
impl WifiCompat{
 fn new()->Self{Self{initialized:false,started:false,mode:0,config:None,callbacks:None,counts:[0;14],lifecycle:0,returns:0,event_posts:0,event_errors:0,attempts:0,failures:0,pending:Vec::new(),limit:None,epochs:0}}
 pub(crate) fn limited(&self)->bool{self.limit.is_some()}
 pub(crate) fn reset(&mut self){let epoch=self.epochs.saturating_add(1);*self=Self::new();self.epochs=epoch;}
 fn fail(&mut self,why:&str){if self.limit.is_none(){self.limit=Some(why.into());}}
 fn state(&self)->&str{if self.limit.is_some(){"unsupported"}else if self.started{"started-offline"}else if self.initialized{"initialized"}else{"uninitialized"}}
 fn json(&self)->String{
  let calls=self.counts.iter().copied().fold(0,u64::saturating_add);let error=self.limit.as_ref().map_or("null".into(),|s|format!("\"{}\"",s.replace('\\',"\\\\").replace('"',"\\\"")));
  let apis=API_NAMES.iter().zip(self.counts).map(|(name,n)|format!("\"{name}\":\"{n}\"")).collect::<Vec<_>>().join(",");
  format!("{{\"enabled\":true,\"profile\":\"{PROFILE}\",\"mode\":\"compatible-driver-assisted\",\"state\":\"{}\",\"nativeWifi\":false,\"associated\":false,\"hasIp\":false,\"storage\":\"session-RAM\",\"hookCalls\":\"{calls}\",\"virtualSlots\":\"{}\",\"eventPosts\":\"{}\",\"eventPostFailures\":\"{}\",\"connectAttempts\":\"{}\",\"noApFailuresQueued\":\"{}\",\"pendingCalls\":{},\"epoch\":\"{}\",\"failureDelayCycles\":\"0\",\"failureReason\":201,\"bssidMeasured\":false,\"rssiMeasured\":false,\"lastError\":{error},\"apiCalls\":{{{apis}}}}}",self.state(),self.returns,self.event_posts,self.event_errors,self.attempts,self.failures,self.pending.len(),self.epochs)
 }
}
impl C5Bus{pub fn wifi_driver_compat_snapshot(&self)->Option<String>{self.wifi_compat.as_ref().map(|w|w.json())}}
pub(crate) fn hooks(bus:&C5Bus)->&[u32]{if bus.wifi_compat.is_some(){&HOOKS}else{&[]}}
// HLE arguments and scratch frames are constrained to existing internal SRAM.
// Each individual access still crosses the original APM/PMA/PMP bus checks.
fn read_span(bus:&mut C5Bus,at:u32,len:usize)->Result<Vec<u8>,String>{bus.compat_read(at,len)}
fn write_span(bus:&mut C5Bus,at:u32,data:&[u8])->Result<(),String>{bus.compat_write(at,data)}
fn word(bytes:&[u8],at:usize)->u32{u32::from_le_bytes(bytes[at..at+4].try_into().unwrap())}
fn entry(bus:&mut C5Bus,pc:u32,expected:&str)->bool{
 let Some(base)=pc.checked_sub(0x4200_0000).and_then(|p|p.checked_add(0x20_0000))else{return false;};
 let mut bytes=[0;32];for(i,v)in bytes.iter_mut().enumerate(){
  let at=pc+i as u32;if bus.mspi.translate(at)!=Some((false,base as usize+i)){return false;}
  let Ok(b)=bus.read8(at)else{return false;};*v=b;
 }hex(&bytes)==expected
}
// HLE consumes a single declared scheduler slot, but no architectural retired
// instruction. Native event-post code keeps its ordinary guest accounting.
fn returned(cpu:&mut Cpu,w:&mut WifiCompat,value:u32){cpu.x[10]=value;cpu.pc=cpu.x[1]&!1;cpu.insn_count=cpu.insn_count.saturating_add(1);cpu.cycle_count=cpu.cycle_count.saturating_add(1);w.returns=w.returns.saturating_add(1);}
fn reserve_event(cpu:&mut Cpu,bus:&mut C5Bus,w:&mut WifiCompat,id:u8,payload:&[u8])->Result<(),String>{
 if payload.len()>80{return Err("compat event payload exceeds bounded frame".into());}
 if w.pending.len()>=8{return Err("compat guest event call nesting limit".into());}
 if !entry(bus,EVENT_POST,POST_ENTRY){return Err("compat esp_event_post entry mismatch".into());}
 let sp=cpu.x[2].checked_sub(96).ok_or("compat event stack overflow")?;
 if sp&15!=0{return Err("compat event stack is not RV32 ABI aligned".into());}
 let saved=read_span(bus,sp,96)?;bus.compat_check_write(sp,96)?;
 let mut frame=saved.clone();frame[16..16+payload.len()].copy_from_slice(payload);
 write_span(bus,sp,&frame)?;
 w.pending.push(Pending{pc:cpu.pc,sp,registers:cpu.x,saved,event:id,lifecycle:w.lifecycle});
 cpu.x[1]=cpu.pc;cpu.x[2]=sp;cpu.x[10]=WIFI_EVENT;cpu.x[11]=u32::from(id);cpu.x[12]=if payload.is_empty(){0}else{sp+16};cpu.x[13]=payload.len()as u32;cpu.x[14]=0;cpu.pc=EVENT_POST;
 // One virtual call-setup slot; no synthetic instruction enters MINSTRET.
 cpu.insn_count=cpu.insn_count.saturating_add(1);cpu.cycle_count=cpu.cycle_count.saturating_add(1);w.returns=w.returns.saturating_add(1);Ok(())
}
fn init_config(bus:&mut C5Bus,pointer:u32)->Result<u32,String>{
 if pointer&7!=0{return Ok(ARG);}
 let Ok(c)=read_span(bus,pointer,152)else{return Ok(ARG);};
 if word(&c,144)!=0x1f2f_3f4f{return Ok(ARG);}
 if word(&c,0)!=0x4081_8a7c{return Err("compat init uses an unsupported OSI table".into());}
 let osi=read_span(bus,word(&c,0),488)?;
 if digest(&osi)!="ac4367f8686aee0fcf3dd67b17e58f85c44911bd2f2239b5a285aa80e679741f"||digest(&c[4..48])!="cf9cf5c94320dbf047be434efbbf1c9d885ce7673c939103e89eff54b49c1742"{return Err("compat init OSI/crypto ABI mismatch".into());}
 let fields=[10,32,1,0,32,0,5,0,0,1,1,0,1,0,6,0,752,32];
 if fields.iter().enumerate().any(|(i,&n)|word(&c,48+4*i)!=n)||word(&c,120)!=417||word(&c,124)!=0||c[128]!=1||word(&c,132)!=7||word(&c,136)!=3||c[140]!=0{return Err("compat init configuration exceeds this fixed offline profile".into());}Ok(OK)
}
fn config(bus:&mut C5Bus,pointer:u32)->Result<Result<[u8;184],u32>,String>{
 if pointer&3!=0{return Ok(Err(ARG));}
 let Ok(c)=read_span(bus,pointer,184)else{return Ok(Err(ARG));};
 // GCC14.2 DWARF confirms bitfield storage overlaps PMF/retry bytes:
 // named RM/BTM/etc bits128[21:16], HE/VHT144[20:8], identifier151..182.
 // Threshold.rssi_5g_adjustment is124; byte183 and unnamed padding are ignored.
 if word(&c,96)!=0||c[100]!=0||c[101..107].iter().any(|&n|n!=0)||c[107]!=0||c[108..110].iter().any(|&n|n!=0)||word(&c,112)!=0||c[116]!=0||word(&c,120)!=3||c[124]!=0||word(&c,128)&0x003f_ffff!=0||word(&c,132)&0x03ff_ffff!=0||word(&c,136)!=3||word(&c,140)!=0||word(&c,144)&0x001f_ffff!=0||word(&c,148)&0x0007_ffff!=0||c[151..183].iter().any(|&n|n!=0){return Err("compat STA configuration fields exceed this fixed offline profile".into());}
 let pass=&c[32..96];let n=pass.iter().position(|&b|b==0).unwrap_or(64);
 if (1..8).contains(&n)||n==64&&!pass.iter().all(u8::is_ascii_hexdigit){return Ok(Err(PASSWORD));}
 Ok(Ok(c.try_into().map_err(|_|"compat STA ABI size")?))
}
// Machine function hooks run before on_insn observers. Never reuse a previous
// instruction's context or the monitor's host-inspection (None) permissions.
struct ContextGuard(crate::cpu_monitor::Monitor);
impl Drop for ContextGuard{fn drop(&mut self){self.0.borrow_mut().end();}}
fn in_cpu_context<T>(cpu:&mut Cpu,bus:&mut C5Bus,body:impl FnOnce(&mut Cpu,&mut C5Bus)->T)->T{
 bus.note_pc(cpu.pc);let monitor=bus.cpu_monitor();
 {let mut m=monitor.borrow_mut();m.end();let before=m.regs.irq();m.begin_core(cpu.pc,cpu);bus.irq_dirty|=before!=m.regs.irq();}
 let _guard=ContextGuard(monitor);body(cpu,bus)
}
pub(crate) fn intercept(cpu:&mut Cpu,bus:&mut C5Bus)->bool{
 let Some(index)=HOOKS.iter().position(|&pc|pc==cpu.pc)else{return false;};
 let Some(mut w)=bus.wifi_compat.take()else{return false;};
 if w.limited(){bus.wifi_compat=Some(w);return false;}
 let handled=in_cpu_context(cpu,bus,|cpu,bus|match dispatch(cpu,bus,&mut w,index){Ok(())=>true,Err(reason)=>{w.fail(&reason);false}});
 bus.wifi_compat=Some(w);handled
}
fn dispatch(cpu:&mut Cpu,bus:&mut C5Bus,w:&mut WifiCompat,index:usize)->Result<(),String>{
 pinned(bus)?;
 if bus.fetch(cpu.pc).is_err(){return Err("compat API entry is not executable in the current CPU memory policy".into());}
 if !entry(bus,cpu.pc,ENTRIES[index]){return Err("compat API entry bytes changed".into());}
 if cpu.x[1]==cpu.pc{
  let p=w.pending.iter().position(|p|p.pc==cpu.pc&&p.sp==cpu.x[2]).ok_or("compat event continuation stack mismatch")?;
  return complete_post(cpu,bus,w,p);

 }
 w.counts[index]=w.counts[index].saturating_add(1);
 api(cpu,bus,w,index)
}
fn complete_post(cpu:&mut Cpu,bus:&mut C5Bus,w:&mut WifiCompat,p:usize)->Result<(),String>{
 let call=w.pending.get(p).ok_or("compat event continuation missing")?;
 if cpu.x[1]!=call.pc||cpu.x[2]!=call.sp||[3,4,8,9,18,19,20,21,22,23,24,25,26,27].iter().any(|&r|cpu.x[r]!=call.registers[r]){return Err("compat native event post violated RV32 callee-saved ABI".into());}
 // Validate restoration before removing the continuation or touching registers.
 write_span(bus,call.sp,&call.saved)?;
 let p=w.pending.remove(p);let result=cpu.x[10];cpu.x=p.registers;
 if result==OK{w.event_posts=w.event_posts.saturating_add(1);if p.event==5{w.failures=w.failures.saturating_add(1);}}
 else{w.event_errors=w.event_errors.saturating_add(1);if p.lifecycle==w.lifecycle{if p.event==2{w.started=false;}else if p.event==3{w.started=true;}}}
 returned(cpu,w,result);Ok(())
}
fn api(cpu:&mut Cpu,bus:&mut C5Bus,w:&mut WifiCompat,index:usize)->Result<(),String>{
 let a=cpu.x[10];let b=cpu.x[11];
 let result=match HOOKS[index]{
  INIT=>{if w.initialized{INIT_STATE}else{let v=init_config(bus,a)?;if v==OK{w.initialized=true;}v}},
  SET_MODE=>{if !w.initialized{NOT_INIT}else if a>4{ARG}else if a>1{return Err("compat supports STA or NULL mode only; AP/APSTA/NAN are unavailable".into());}else if w.started{STATE}else{w.mode=a;OK}},
  SET_CONFIG=>{if !w.initialized{NOT_INIT}else if a!=0{IFACE}else if w.mode!=1{MODE}else{match config(bus,b)?{Ok(c)=>{w.config=Some(c);OK},Err(v)=>v}}},
  START=>{if !w.initialized{NOT_INIT}else if w.mode!=1{MODE}else if w.started{STATE}else{reserve_event(cpu,bus,w,2,&[])?;w.lifecycle=w.lifecycle.saturating_add(1);w.started=true;if let Some(p)=w.pending.last_mut(){p.lifecycle=w.lifecycle;}return Ok(());}},
  CONNECT=>{if !w.initialized{NOT_INIT}else if !w.started{NOT_STARTED}else if w.mode!=1{MODE}else if w.config.as_ref().is_none_or(|c|c[0]==0){SSID}else{
   let c=w.config.as_ref().ok_or("compat missing config")?;let mut payload=[0;41];let n=c[..32].iter().position(|&v|v==0).unwrap_or(32);payload[..32].copy_from_slice(&c[..32]);payload[32]=n as u8;payload[39]=201;
   // No radio attempt exists. The zero-delay virtual failure is delivered
   // asynchronously by the unchanged guest event queue, never by a host call
   // to the app handler. BSSID/RSSI zeros mean unknown, not measurements.
   reserve_event(cpu,bus,w,5,&payload)?;w.attempts=w.attempts.saturating_add(1);return Ok(());
  }},
  GET_MAC=>{if !w.initialized{NOT_INIT}else if a!=0{IFACE}else{let mac=bus.compat_mac();if write_span(bus,b,&mac).is_err(){ARG}else{OK}}},
  GET_CONFIG=>{if !w.initialized{NOT_INIT}else if a!=0{IFACE}else if w.mode!=1{MODE}else if let Some(c)=&w.config{if b&3!=0||write_span(bus,b,c).is_err(){ARG}else{OK}}else{STATE}},
  GET_MODE=>{if !w.initialized{NOT_INIT}else if a&3!=0||write_span(bus,a,&w.mode.to_le_bytes()).is_err(){ARG}else{OK}},
  REG_BUF=>{if !w.initialized{NOT_INIT}else if [a,b].iter().any(|&pc|pc&1!=0||bus.fetch(pc).is_err()){ARG}else{w.callbacks=Some([a,b]);OK}},
  STOP=>{if !w.initialized{NOT_INIT}else if !w.started{NOT_STARTED}else{reserve_event(cpu,bus,w,3,&[])?;w.lifecycle=w.lifecycle.saturating_add(1);w.started=false;if let Some(p)=w.pending.last_mut(){p.lifecycle=w.lifecycle;}return Ok(());}},
  DEINIT=>{if !w.initialized{NOT_INIT}else if w.started{NOT_STOPPED}else if !w.pending.is_empty(){STATE}else{w.initialized=false;w.mode=0;w.config=None;w.callbacks=None;OK}},
  DISCONNECT=>{if !w.initialized{NOT_INIT}else if !w.started{NOT_STARTED}else{NOT_CONNECT}},
  SCAN_START|SCAN_RECORDS=>{if !w.initialized{NOT_INIT}else{return Err("compat scan API is outside the offline first-screen profile".into());}},
  _=>return Err("compat unsupported API".into()),
 };
 returned(cpu,w,result);Ok(())
}

#[cfg(test)]
mod tests{
 use super::*;
 use esp_soc::SocBus;
 fn sta()->[u8;184]{let mut c=[0;184];c[..4].copy_from_slice(b"TEST");c[32..40].copy_from_slice(b"password");c[120..124].copy_from_slice(&3u32.to_le_bytes());c[136..140].copy_from_slice(&3u32.to_le_bytes());c}
 fn config_input(bus:&mut C5Bus,data:&[u8])->u32{let address=crate::bus::SRAM_BASE+0x1000;bus.load_ram(address,data,data.len()).unwrap();address}
 fn call(cpu:&mut Cpu,bus:&mut C5Bus,w:&mut WifiCompat,pc:u32,a:u32,b:u32)->Result<u32,String>{cpu.pc=pc;cpu.x[1]=crate::bus::SRAM_BASE+0x100;cpu.x[10]=a;cpu.x[11]=b;api(cpu,bus,w,HOOKS.iter().position(|&p|p==pc).unwrap())?;Ok(cpu.x[10])}
 #[test]
 fn default_machine_has_no_profile_hooks_or_state(){let mut m=crate::machine([0;6],32<<20);assert!(hooks(&m.bus).is_empty());assert!(m.bus.wifi_driver_compat_snapshot().is_none());assert!(enable_wifi_driver_compat(&mut m,"unknown").is_err());assert!(enable_wifi_driver_compat(&mut m,PROFILE).is_err());assert!(hooks(&m.bus).is_empty());}
 #[test]
 fn abi_failures_never_write_a_partial_output_or_mmio(){let mut b=C5Bus::new();let end=crate::bus::SRAM_BASE+crate::bus::SRAM_SIZE as u32;b.sram.fill(0xa5);let before=b.sram.clone();assert!(write_span(&mut b,end-2,&[1,2,3]).is_err());assert!(write_span(&mut b,0x600a_d858,&[0;4]).is_err());assert!(read_span(&mut b,0x600a_d858,4).is_err());assert_eq!(b.sram,before);assert!(b.first_fault.is_none());assert!(init_config(&mut b,end-150).is_ok_and(|v|v==ARG));assert!(config(&mut b,end-180).is_ok_and(|v|v==Err(ARG)));}
 #[test]
 fn scalar_returns_preserve_callee_registers_and_do_not_retire_instructions(){let mut c=crate::core();c.x=std::array::from_fn(|i|i as u32*0x101);c.x[1]=0x4200_0101;c.pc=INIT;c.retired_count=17;c.insn_count=30;c.cycle_count=40;let before=c.x;let mut w=WifiCompat::new();returned(&mut c,&mut w,ARG);assert_eq!(c.pc,0x4200_0100);for(i,&v)in before.iter().enumerate(){if i!=10{assert_eq!(c.x[i],v);}}assert_eq!(c.retired_count,17);assert_eq!((c.insn_count,c.cycle_count,w.returns),(31,41,1));}
 #[test]
 fn all_defined_sta_controls_are_checked_but_padding_is_ignored(){let mut b=C5Bus::new();let c=sta();let at=config_input(&mut b,&c);assert!(config(&mut b,at).unwrap().is_ok());
  // Every noncredential semantic byte outside the two accepted enum words.
  for off in [96,100,101,106,107,108,109,112,116,124,128,129,130,132,136,140,144,145,146,148,149,150,151,182]{let mut bad=c;bad[off]^=1;let at=config_input(&mut b,&bad);assert!(config(&mut b,at).is_err(),"control byte{off}");}
  for off in [110,111,117,118,119,125,126,127,183]{let mut padded=c;padded[off]=0xa5;let at=config_input(&mut b,&padded);assert!(config(&mut b,at).unwrap().is_ok(),"padding byte{off}");}
  // Named bitfields end before their C storage unit: upper bits are padding.
  for(off,mask)in[(130,0xc0),(131,0xff),(135,0xfc),(146,0xe0),(147,0xff),(150,0xf8)]{let mut padded=c;padded[off]|=mask;let at=config_input(&mut b,&padded);assert!(config(&mut b,at).unwrap().is_ok(),"padding bits{off}");}
 }
 #[test]
 fn config_rejects_bad_passwords_without_mutating_driver_or_guest_memory(){let mut b=C5Bus::new();let mut c=sta();c[32..96].fill(0);c[32..35].copy_from_slice(b"bad");let at=config_input(&mut b,&c);let memory=b.sram.clone();assert_eq!(config(&mut b,at).unwrap(),Err(PASSWORD));assert_eq!(b.sram,memory);c[32..96].fill(b'g');let at=config_input(&mut b,&c);assert_eq!(config(&mut b,at).unwrap(),Err(PASSWORD));c[32..96].fill(b'a');let at=config_input(&mut b,&c);assert!(config(&mut b,at).unwrap().is_ok());}
 #[test]
 fn offline_lifecycle_and_errors_never_create_association_or_leak_credentials(){let mut c=crate::core();let mut b=C5Bus::new();let mut w=WifiCompat::new();for pc in[SET_MODE,SET_CONFIG,START,CONNECT,GET_MAC,GET_MODE,STOP,DEINIT,DISCONNECT,SCAN_START,SCAN_RECORDS,GET_CONFIG]{assert_eq!(call(&mut c,&mut b,&mut w,pc,0,0).unwrap(),NOT_INIT);}
  w.initialized=true;assert!(call(&mut c,&mut b,&mut w,SET_MODE,2,0).is_err());assert_eq!(w.mode,0);assert_eq!(call(&mut c,&mut b,&mut w,SET_MODE,1,0).unwrap(),OK);assert_eq!(call(&mut c,&mut b,&mut w,CONNECT,0,0).unwrap(),NOT_STARTED);w.started=true;assert_eq!(call(&mut c,&mut b,&mut w,CONNECT,0,0).unwrap(),SSID);assert_eq!(call(&mut c,&mut b,&mut w,DISCONNECT,0,0).unwrap(),NOT_CONNECT);assert_eq!(call(&mut c,&mut b,&mut w,DEINIT,0,0).unwrap(),NOT_STOPPED);
  w.started=false;let at=config_input(&mut b,&sta());assert_eq!(call(&mut c,&mut b,&mut w,SET_CONFIG,0,at).unwrap(),OK);let out=crate::bus::SRAM_BASE+0x2000;assert_eq!(call(&mut c,&mut b,&mut w,GET_CONFIG,0,out).unwrap(),OK);assert_eq!(read_span(&mut b,out,184).unwrap(),sta());let json=w.json();assert!(json.contains("\"associated\":false"));assert!(json.contains("\"hasIp\":false"));assert!(!json.contains("password"));assert!(!json.contains("TEST"));assert!(call(&mut c,&mut b,&mut w,SCAN_START,0,1).is_err());assert_eq!(call(&mut c,&mut b,&mut w,DEINIT,0,0).unwrap(),OK);assert!(w.config.is_none());assert!(w.callbacks.is_none());assert!(!w.initialized);
 }
 #[test]
 fn guest_post_continuations_restore_only_the_matching_stack_and_lifecycle(){
  let mut b=C5Bus::new();let mut c=crate::core();let mut w=WifiCompat::new();let first=crate::bus::SRAM_BASE+0x3000;let second=first+0x100;
  b.compat_write(first,&[0x11;96]).unwrap();b.compat_write(second,&[0x22;96]).unwrap();
  let mut regs1=[0;32];regs1[1]=0x4200_1000;regs1[2]=first+96;regs1[8]=17;
  let mut regs2=regs1;regs2[1]=0x4200_2000;regs2[2]=second+96;regs2[8]=23;
  w.pending.push(Pending{pc:START,sp:first,registers:regs1,saved:vec![0xaa;96],event:2,lifecycle:1});
  w.pending.push(Pending{pc:STOP,sp:second,registers:regs2,saved:vec![0xbb;96],event:3,lifecycle:2});
  w.started=false;w.lifecycle=2;c.x=regs2;c.x[1]=STOP;c.x[2]=second;c.x[10]=OK;complete_post(&mut c,&mut b,&mut w,1).unwrap();assert_eq!(c.x[2],second+96);assert_eq!(c.x[8],23);assert_eq!(c.pc,0x4200_2000);assert_eq!(b.compat_read(second,96).unwrap(),[0xbb;96]);assert_eq!(b.compat_read(first,96).unwrap(),[0x11;96]);assert_eq!(w.pending.len(),1);
  // An older START's failed queue operation must not undo the newer STOP.
  c.x=regs1;c.x[1]=START;c.x[2]=first;c.x[10]=0x103;complete_post(&mut c,&mut b,&mut w,0).unwrap();assert!(!w.started);assert_eq!(c.x[2],first+96);assert_eq!(c.x[8],17);assert_eq!(c.pc,0x4200_1000);assert_eq!(b.compat_read(first,96).unwrap(),[0xaa;96]);assert!(w.pending.is_empty());assert_eq!((w.event_posts,w.event_errors),(1,1));assert_eq!(c.retired_count,0);
 }
 #[test]
 fn actual_cpu_pmp_denies_a_partial_output_and_context_is_always_cleared(){
  let mut b=C5Bus::new();let mut c=crate::core();c.enable_c5_memory_protection().unwrap();let boundary=crate::bus::SRAM_BASE+0x5080;let out=boundary-4;
  c.try_write_csr(0x3b0,(boundary>>2)|15).unwrap();c.try_write_csr(0x3a0,0x99).unwrap();b.compat_write(out,&[0xa5;6]).unwrap();let before=b.sram.clone();
  let mut w=WifiCompat::new();w.initialized=true;c.pc=GET_MAC;c.x[10]=0;c.x[11]=out;c.x[1]=0x4200_1000;
  in_cpu_context(&mut c,&mut b,|c,b|api(c,b,&mut w,2)).unwrap();assert_eq!(c.x[10],ARG);assert_eq!(b.sram,before);assert_eq!(b.monitor.as_ref().unwrap().borrow().machine_context(),None);
  // Host inspection remains host inspection after a failed CPU-qualified API.
  b.compat_write(out,&[7;6]).unwrap();assert_eq!(b.compat_read(out,6).unwrap(),[7;6]);
 }
 #[test]
 fn actual_cpu_pma_and_apm_reject_inputs_without_inheriting_host_permissions(){
  let mut b=C5Bus::new();let mut c=crate::core();c.enable_c5_memory_protection().unwrap();let at=crate::bus::SRAM_BASE+0x6000;
  c.try_write_csr(0xbc0,0).unwrap();c.try_write_csr(0xbd0,(at>>2)|15).unwrap();c.try_write_csr(0xbc0,0xc000_0005).unwrap();let mut input=[0;152];input[144..148].copy_from_slice(&0x1f2f_3f4fu32.to_le_bytes());b.compat_write(at,&input).unwrap();
  assert!(in_cpu_context(&mut c,&mut b,|_,b|init_config(b,at)).is_ok_and(|v|v==ARG));assert_eq!(b.monitor.as_ref().unwrap().borrow().machine_context(),None);
  b.select_validated_rom_profile().unwrap();b.rom_boot=true;c.machine_only=false;
  assert!(in_cpu_context(&mut c,&mut b,|_,b|read_span(b,at,4)).is_err());assert_eq!(b.monitor.as_ref().unwrap().borrow().machine_context(),None);
 }
 #[test]
 fn callback_fetch_is_checked_even_when_its_bytes_are_readable(){
  let mut b=C5Bus::new();let mut c=crate::core();c.enable_c5_memory_protection().unwrap();let code=crate::bus::SRAM_BASE+0x6000;b.load_ram(code,&0x13u32.to_le_bytes(),4).unwrap();c.try_write_csr(0x3b0,(code>>2)|15).unwrap();c.try_write_csr(0x3a0,0x9b).unwrap();let mut w=WifiCompat::new();w.initialized=true;
  in_cpu_context(&mut c,&mut b,|c,b|{assert!(read_span(b,code,4).is_ok());assert_eq!(call(c,b,&mut w,REG_BUF,code,code).unwrap(),ARG);});assert!(w.callbacks.is_none());assert_eq!(b.monitor.as_ref().unwrap().borrow().machine_context(),None);
 }
 #[test]
 fn failed_event_preflight_and_restore_leave_driver_and_frame_intact(){
  let mut b=C5Bus::new();let mut c=crate::core();let mut w=WifiCompat::new();w.initialized=true;w.mode=1;c.pc=START;c.x[1]=0x4200_0100;c.x[2]=15;let regs=c.x;let memory=b.sram.clone();
  assert!(api(&mut c,&mut b,&mut w,4).is_err());assert_eq!(c.x,regs);assert_eq!(b.sram,memory);assert!(!w.started);assert_eq!(w.lifecycle,0);assert!(w.pending.is_empty());
  let bad=crate::bus::SRAM_BASE+crate::bus::SRAM_SIZE as u32-8;let mut original=[0;32];original[1]=0x4200_1000;original[2]=bad+96;original[8]=7;
  w.pending.push(Pending{pc:START,sp:bad,registers:original,saved:vec![0xaa;96],event:2,lifecycle:0});c.x=original;c.x[1]=START;c.x[2]=bad;let before=c.x;
  assert!(complete_post(&mut c,&mut b,&mut w,0).is_err());assert_eq!(w.pending.len(),1);assert_eq!(c.x,before);assert_eq!(b.sram,memory);
  w.pending[0].sp=crate::bus::SRAM_BASE+0x7000;c.x[2]=w.pending[0].sp;c.x[8]^=1;assert!(complete_post(&mut c,&mut b,&mut w,0).unwrap_err().contains("callee-saved"));assert_eq!(w.pending.len(),1);
 }
 #[test]
 fn raw_rom_and_app_direct_deselect_compatibility_hooks(){
  use esp_soc::SocBus;let mut b=C5Bus::new();b.wifi_compat=Some(Box::new(WifiCompat::new()));assert!(!hooks(&b).is_empty());b.deselect_validated_rom_profile();assert!(hooks(&b).is_empty());
  b.wifi_compat=Some(Box::new(WifiCompat::new()));b.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();assert!(hooks(&b).is_empty());assert!(b.wifi_driver_compat_snapshot().is_none());
  let ram=crate::bus::SRAM_BASE;let mut app=vec![0;48];app[0]=0xe9;app[1]=1;app[4..8].copy_from_slice(&ram.to_le_bytes());app[12]=0x17;app[24..28].copy_from_slice(&ram.to_le_bytes());app[28..32].copy_from_slice(&4u32.to_le_bytes());app[32..36].copy_from_slice(&0x13u32.to_le_bytes());app[47]=0xef^0x13;b.flash=app;b.wifi_compat=Some(Box::new(WifiCompat::new()));b.boot_app(0).unwrap();assert!(hooks(&b).is_empty());assert!(b.wifi_driver_compat_snapshot().is_none());
 }
 #[test]
 fn compatibility_limit_precedes_an_already_pending_real_irq(){
  let mut m=crate::rom_machine([0;6],4<<20);m.quantum=1;m.bus.select_validated_rom_profile().unwrap();m.bus.rom_boot=true;
  let ram=crate::bus::SRAM_BASE;m.cores[0].pc=GET_MAC;m.cores[0].mtvec=(ram+0x200)|3;m.cores[0].mstatus|=8;
  m.cores[0].mepc=0x1234;m.cores[0].mcause=99;m.cores[0].mtval=88;
  for(a,v)in[(0x2080_0000,3),(0x2080_1044,0x3fc0_0100),(0x6001_005c,17),(0x600c_5090,1)]{m.bus.write32(a,v).unwrap();}
  assert_eq!(m.bus.irq_sources_of(0,1),vec![23]);let mut w=WifiCompat::new();w.fail("unsupported API fixture");m.bus.wifi_compat=Some(Box::new(w));
  assert!(matches!(m.run(1),esp_soc::Stop::ModelLimit{pc:GET_MAC,..}));assert_eq!(m.insns(),0);assert_eq!((m.cores[0].mepc,m.cores[0].mcause,m.cores[0].mtval),(0x1234,99,88));assert_eq!(m.interrupts,0);
 }
 #[test]
 fn reset_cancels_continuations_and_ram_configuration(){let mut w=WifiCompat::new();w.initialized=true;w.started=true;w.config=Some(sta());w.callbacks=Some([0x4000_0000;2]);w.pending.push(Pending{pc:START,sp:0x4080_1000,registers:[0;32],saved:vec![0;96],event:2,lifecycle:0});w.fail("unsupported test");w.reset();assert!(!w.initialized&&!w.started);assert!(w.config.is_none()&&w.callbacks.is_none()&&w.pending.is_empty()&&w.limit.is_none());assert_eq!(w.epochs,1);assert!(w.json().contains("session-RAM"));}
}

#[cfg(test)] #[path="wifi_compat_external_tests.rs"] mod external_tests;
