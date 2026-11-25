//! TCA8418 keypad FIFO/interrupt subset, TI SCPS215G (June2018), §§8.3.1/8.3.5/8.6.
//! Matrix scanning uses a 25ms digital scan and two stable observations (§8.6.2.15),
//! not the distinct 50us GPIO debounce. No analog ghosting, GPI events or unlock engine.
use super::I2cDevice;
use crate::keypad::{KeyEventAcceptance,KeyEventDiagnostics};
use std::{collections::VecDeque,sync::{Arc,Mutex}};

#[cfg_attr(test,derive(Clone,Debug,Eq,PartialEq))]
pub struct Tca8418State {
    regs:[u8;0x30],fifo:VecDeque<u8>,desired:[bool;80],sampled:[bool;80],stable:[bool;80],
    cycle:u64,period:u64,next_scan:Option<u64>,irq_hold_until:Option<u64>,accepted:u64,consumed:u64,
}
impl Tca8418State {
    pub fn new(cpu_hz:u64)->Self {Self{regs:[0;0x30],fifo:VecDeque::new(),desired:[false;80],sampled:[false;80],stable:[false;80],cycle:0,period:(cpu_hz/40).max(1),next_scan:None,irq_hold_until:None,accepted:0,consumed:0}}
    pub fn key(&mut self,cycle:u64,row:u8,column:u8,down:bool)->bool {
        if row>=8 || column>=10 {return false;}
        self.cycle=self.cycle.max(cycle);let index=usize::from(row)*10+usize::from(column);
        if self.desired[index]!=down {self.desired[index]=down;self.schedule();}true
    }
    /// Nonphysical, explicit opt-in transport for edges already debounced by a host.
    /// Do not shorten the ordinary key()/advance_to() matrix timing. FIFO backpressure
    /// is checked before changing the clock or any matrix/FIFO/interrupt state.
    pub fn try_key_debounced(&mut self,cycle:u64,row:u8,column:u8,down:bool)->KeyEventAcceptance {
        use KeyEventAcceptance::*;
        if row>=8 || column>=10 {return Invalid;}
        let index=usize::from(row)*10+usize::from(column);
        if !self.selected(index) || self.regs[3]&0x40!=0 {return Disabled;}
        if self.stable[index]==down {
            // Reconcile a pending raw sample if callers intentionally switch transport.
            self.desired[index]=down;self.sampled[index]=down;self.cycle=self.cycle.max(cycle);return Duplicate;
        }
        if self.fifo.len()==10 {return Full;}
        self.cycle=self.cycle.max(cycle);self.desired[index]=down;self.sampled[index]=down;self.stable[index]=down;
        self.push((index+1) as u8 | if down{0x80}else{0});Accepted
    }
    pub fn key_diagnostics(&self)->KeyEventDiagnostics {KeyEventDiagnostics{accepted:self.accepted,consumed:self.consumed,pending:self.fifo.len() as u8,interrupt_pending:self.regs[2]&self.regs[1]&1!=0}}
    fn schedule(&mut self) {if self.next_scan.is_none() {self.next_scan=Some(self.cycle.saturating_div(self.period).saturating_add(1).saturating_mul(self.period));}}
    fn selected(&self,index:usize)->bool {self.regs[0x1d]&(1<<(index/10))!=0 && self.regs[0x1e+(index%10)/8]&(1<<((index%10)%8))!=0}
    fn push(&mut self,event:u8) {
        if self.fifo.len()==10 {self.regs[2]|=8;if self.regs[1]&0x20==0 {return;}self.fifo.pop_front();}
        self.fifo.push_back(event);self.regs[2]|=1;self.accepted=self.accepted.saturating_add(1);
    }
    pub fn irq_high(&self)->bool {self.irq_hold_until.is_some() || self.regs[2]&self.regs[1]&0x0f==0}
    pub fn next_deadline(&self)->Option<u64> {self.next_scan.into_iter().chain(self.irq_hold_until).min()}
    pub fn advance_to(&mut self,cycle:u64)->Vec<(u64,bool)> {
        let mut edges=Vec::new();let mut high=self.irq_high();
        while let Some(at)=self.next_deadline().filter(|at|*at<=cycle) {
            self.cycle=at;
            if self.irq_hold_until==Some(at) {self.irq_hold_until=None;}
            if self.next_scan==Some(at) {
                self.next_scan=None;
                for index in 0..80 {
                    if self.desired[index]==self.sampled[index] && self.stable[index]!=self.desired[index] {
                        self.stable[index]=self.desired[index];
                        if self.selected(index) && self.regs[3]&0x40==0 {self.push((index+1) as u8 | if self.stable[index] {0x80} else {0});}
                    }
                    self.sampled[index]=self.desired[index];
                }
                if self.stable!=self.desired {self.next_scan=Some(at.saturating_add(self.period));}
            }
            let level=self.irq_high();if high!=level {edges.push((at,level));high=level;}
        }
        self.cycle=cycle;edges
    }
}
pub struct Tca8418 {state:Arc<Mutex<Tca8418State>>,ptr:u8,first:bool}
impl Tca8418 {pub const ADDRESS:u8=0x34;pub fn new(state:Arc<Mutex<Tca8418State>>)->Self {Self{state,ptr:0,first:true}}}
impl I2cDevice for Tca8418 {
    fn pins(&self)->Option<(u8,u8)> {Some((8,9))}
    fn start(&mut self,read:bool)->bool {if !read {self.first=true;}true}
    fn write(&mut self,value:u8)->bool {
        if self.first {self.ptr=value;self.first=false;return true;}
        let mut state=self.state.lock().expect("TCA8418 mutex poisoned");let reg=self.ptr as usize;
        match reg {
            2=>{state.regs[2]&=!value;if !state.fifo.is_empty() {state.regs[2]|=1;if value&1!=0 && state.regs[1]&0x10!=0 {state.irq_hold_until=Some(state.cycle.saturating_add((state.period/500).max(1)));}}}
            3=>state.regs[3]=value&0x40,
            0|4..=0x0d|0x14..=0x16=>{},
            0x11..=0x13=>state.regs[reg]&=!value,
            1|0x0e..=0x10|0x17..=0x2e=>state.regs[reg]=value,
            _=>{}
        }
        if state.regs[1]&0x80!=0 {self.ptr=self.ptr.wrapping_add(1);}true
    }
    fn read(&mut self)->u8 {
        let mut state=self.state.lock().expect("TCA8418 mutex poisoned");let reg=self.ptr as usize;
        // SCPS215G §8.6.2.4: drain by repeatedly reading KEY_EVENT_A; each read shifts
        // the FIFO. B-J observe that live shifted FIFO, not a snapshot taken before A.
        // M5Cardputer Adafruit_TCA8418::getEvent uses one readRegister8(0x04).
        let value=match reg {3=>(state.regs[3]&0xf0)|state.fifo.len() as u8,4=>match state.fifo.pop_front(){Some(v)=>{state.consumed=state.consumed.saturating_add(1);v},None=>0},5..=0x0d=>state.fifo.get(reg-4).copied().unwrap_or(0),_=>state.regs.get(reg).copied().unwrap_or(0)};
        if state.regs[1]&0x80!=0 {self.ptr=self.ptr.wrapping_add(1);}value
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn write(d:&mut Tca8418,r:u8,v:u8) {d.start(false);d.write(r);d.write(v);}
    fn read(d:&mut Tca8418,r:u8)->u8 {d.start(false);d.write(r);d.start(true);d.read()}
    fn setup()->(Arc<Mutex<Tca8418State>>,Tca8418) {let s=Arc::new(Mutex::new(Tca8418State::new(1_000_000)));let mut d=Tca8418::new(s.clone());write(&mut d,0x1d,0x7f);write(&mut d,0x1e,0xff);write(&mut d,1,1);(s,d)}
    #[test]
    fn two_scans_debounce_fifo_and_clear_with_pending_events() {
        let (s,mut d)=setup();s.lock().unwrap().key(0,6,6,true);s.lock().unwrap().advance_to(24999);assert_eq!(read(&mut d,3),0);
        s.lock().unwrap().advance_to(25000);assert_eq!(read(&mut d,3),0);assert_eq!(s.lock().unwrap().advance_to(50000),[(50000,false)]);
        assert_eq!(read(&mut d,3),1);write(&mut d,2,1);assert_eq!(read(&mut d,2),1);assert_eq!(read(&mut d,4),0xc3);assert_eq!(read(&mut d,4),0);
        write(&mut d,2,1);assert!(s.lock().unwrap().irq_high());
        s.lock().unwrap().key(50001,6,6,false);s.lock().unwrap().advance_to(100000);assert_eq!(read(&mut d,4),0x43);
        s.lock().unwrap().key(100001,1,1,true);s.lock().unwrap().key(100002,1,1,false);s.lock().unwrap().advance_to(150000);assert_eq!(read(&mut d,4),0);
    }
    #[test]
    fn masked_keys_overflow_modes_and_invalid_inputs() {
        let (s,mut d)=setup();assert!(!s.lock().unwrap().key(0,8,0,true));assert!(!s.lock().unwrap().key(0,0,10,true));
        for row in 0..8 {for col in 0..10 {s.lock().unwrap().key(0,row,col,true);}}
        s.lock().unwrap().advance_to(50000);assert_eq!(read(&mut d,3),10);assert_eq!(read(&mut d,2),9);assert_eq!(read(&mut d,4),0x81);
        let (s,mut d)=setup();write(&mut d,1,0x29);for row in 0..7 {for col in 0..8 {s.lock().unwrap().key(0,row,col,true);}}
        s.lock().unwrap().advance_to(50000);assert_eq!(read(&mut d,3),10);assert_eq!(read(&mut d,4),0xb9); // final ten: row5 col6 through row6 col7
    }
    #[test]
    fn pending_event_clear_pulses_irq_only_when_configured() {
        let (s,mut d)=setup();write(&mut d,1,0x11);s.lock().unwrap().key(0,0,0,true);
        s.lock().unwrap().advance_to(50000);assert!(!s.lock().unwrap().irq_high());
        write(&mut d,2,1);assert!(s.lock().unwrap().irq_high());assert_eq!(read(&mut d,2),1);
        assert!(s.lock().unwrap().advance_to(50049).is_empty());
        assert_eq!(s.lock().unwrap().advance_to(50050),[(50050,false)]);
        assert_eq!(read(&mut d,4),0x81);write(&mut d,2,1);assert!(s.lock().unwrap().irq_high());
        assert_eq!(s.lock().unwrap().next_deadline(),None,"idle keyboard schedules no scans");
    }
    #[test]
    fn m5_single_byte_event_reads_preserve_a_full_press_release_backlog() {
        let (s,mut d)=setup();let mut expected=Vec::new();
        // Five taps: each physical edge has62.5ms, longer than two25ms matrix scans.
        // Hold all ten events until the consumer runs, exercising a genuinely full FIFO.
        for edge in 0..10u64 {
            let column=(edge/2) as u8;let down=edge%2==0;let at=edge*62500;
            let mut state=s.lock().unwrap();assert!(state.key(at,0,column,down));state.advance_to(at+50000);
            expected.push((column+1) | if down {0x80} else {0});
        }
        assert_eq!(read(&mut d,3),10);assert_eq!(read(&mut d,2),1,"no overflow");
        for (n,event) in expected.into_iter().enumerate() {
            // Exactly the getEvent/readRegister8(KEY_EVENT_A), W1C, INT_STAT sequence
            // used by M5Cardputer's TCA8418KeyboardReader::update.
            assert_eq!(read(&mut d,4),event,"FIFO event{n}");
            assert_eq!(read(&mut d,3),9-n as u8);
            write(&mut d,2,1);
            assert_eq!(read(&mut d,2),u8::from(n!=9));
            assert_eq!(s.lock().unwrap().irq_high(),n==9);
        }
        assert_eq!(read(&mut d,4),0);
    }
    #[test]
    fn later_event_registers_do_not_consume_a_release_left_at_the_head() {
        let (s,mut d)=setup();
        for (at,down,column) in [(0,true,0),(62500,false,0),(125000,true,1),(187500,false,1)] {
            let mut state=s.lock().unwrap();state.key(at,0,column,down);state.advance_to(at+50000);
        }
        assert_eq!(read(&mut d,3),4);
        assert_eq!(read(&mut d,5),1,"B initially contains first release");
        assert_eq!(read(&mut d,3),4,"B does not pop");
        assert_eq!(read(&mut d,4),0x81);
        assert_eq!(read(&mut d,5),0x82,"after A shifts, B contains the following press");
        assert_eq!(read(&mut d,3),3);
        assert_eq!(read(&mut d,4),1,"the release remains at A and is not lost");
        assert_eq!(read(&mut d,4),0x82);assert_eq!(read(&mut d,4),2);assert_eq!(read(&mut d,4),0);
    }
    #[test]
    fn no_auto_increment_read_repeats_event_a_instead_of_skipping_to_b() {
        let (s,mut d)=setup();
        s.lock().unwrap().key(0,0,0,true);s.lock().unwrap().advance_to(50000);
        s.lock().unwrap().key(62500,0,0,false);s.lock().unwrap().advance_to(112500);
        assert_eq!(read(&mut d,1)&0x80,0,"M5 enableInterrupts leaves AI disabled");
        d.start(false);d.write(4);d.start(true);
        assert_eq!([d.read(),d.read(),d.read()],[0x81,1,0]);
    }
    fn snapshot(s:&Tca8418State)->Tca8418State {s.clone()}
    #[test]
    fn host_debounced_edges_are_immediate_and_consumption_is_only_event_a() {
        use KeyEventAcceptance::*;
        let (s,mut d)=setup();let mut state=s.lock().unwrap();
        assert_eq!(state.try_key_debounced(7,0,0,true),Accepted);
        assert_eq!(state.try_key_debounced(7,0,0,true),Duplicate);
        assert_eq!(state.try_key_debounced(8,0,0,false),Accepted);
        assert_eq!(state.key_diagnostics(),KeyEventDiagnostics{accepted:2,consumed:0,pending:2,interrupt_pending:true});drop(state);
        assert_eq!(read(&mut d,5),1);assert_eq!(read(&mut d,3),2);
        assert_eq!(s.lock().unwrap().key_diagnostics().consumed,0,"B/count reads are observations");
        assert_eq!(read(&mut d,4),0x81);assert_eq!(read(&mut d,4),1);assert_eq!(read(&mut d,4),0);
        assert_eq!(s.lock().unwrap().key_diagnostics(),KeyEventDiagnostics{accepted:2,consumed:2,pending:0,interrupt_pending:true});
        s.lock().unwrap().advance_to(100000);assert_eq!(read(&mut d,3),0,"no duplicate physical scan event");
    }
    #[test]
    fn diagnostics_observe_guest_irq_ack_without_mutation_or_masking_fifo() {
        let (s,mut d)=setup();
        assert_eq!(s.lock().unwrap().try_key_debounced(0,0,0,true),KeyEventAcceptance::Accepted);
        let before=snapshot(&s.lock().unwrap());
        for _ in 0..10 {assert!(s.lock().unwrap().key_diagnostics().interrupt_pending);}
        assert_eq!(snapshot(&s.lock().unwrap()),before);
        assert_eq!(read(&mut d,4),0x81);
        assert_eq!(s.lock().unwrap().key_diagnostics().pending,0);
        assert!(s.lock().unwrap().key_diagnostics().interrupt_pending,"FIFO pop precedes IRQ ACK");
        write(&mut d,2,1);assert!(!s.lock().unwrap().key_diagnostics().interrupt_pending);
        write(&mut d,1,0);
        assert_eq!(s.lock().unwrap().try_key_debounced(1,0,0,false),KeyEventAcceptance::Accepted);
        assert!(!s.lock().unwrap().key_diagnostics().interrupt_pending,"masked IRQ does not block a polling consumer");
        assert_eq!(read(&mut d,4),1);
    }
    #[test]
    fn host_full_is_atomic_even_with_hardware_overwrite_enabled_and_retry_loses_nothing() {
        use KeyEventAcceptance::*;
        let (s,mut d)=setup();write(&mut d,1,0x21); // raw hardware overflow policy remains separate
        for edge in 0..10 {assert_eq!(s.lock().unwrap().try_key_debounced(0,0,0,edge%2==0),Accepted);}
        let before=snapshot(&s.lock().unwrap());
        assert_eq!(s.lock().unwrap().try_key_debounced(999999,0,0,true),Full);
        assert_eq!(snapshot(&s.lock().unwrap()),before,"Full must not touch clock, scan, matrix, flags or counters");
        let mut events=vec![read(&mut d,4)];
        assert_eq!(s.lock().unwrap().try_key_debounced(1,0,0,true),Accepted);
        assert_eq!(s.lock().unwrap().try_key_debounced(2,0,0,false),Full);
        events.push(read(&mut d,4));assert_eq!(s.lock().unwrap().try_key_debounced(2,0,0,false),Accepted);
        while read(&mut d,3)>0 {events.push(read(&mut d,4));}
        assert_eq!(events,[0x81,1].repeat(6));assert_eq!(read(&mut d,2)&8,0,"host admission never overflows");
        assert_eq!(s.lock().unwrap().key_diagnostics(),KeyEventDiagnostics{accepted:12,consumed:12,pending:0,interrupt_pending:true});
    }
    #[test]
    fn host_disabled_and_invalid_are_atomic_but_interrupt_mask_does_not_disable_fifo() {
        use KeyEventAcceptance::*;
        let (s,mut d)=setup();write(&mut d,0x1d,0);
        for (row,col,status) in [(0,0,Disabled),(8,0,Invalid),(0,10,Invalid)] {
            let before=snapshot(&s.lock().unwrap());
            assert_eq!(s.lock().unwrap().try_key_debounced(900,row,col,true),status);
            assert_eq!(snapshot(&s.lock().unwrap()),before);
        }
        write(&mut d,0x1d,0x7f);write(&mut d,3,0x40);
        let before=snapshot(&s.lock().unwrap());assert_eq!(s.lock().unwrap().try_key_debounced(1,0,0,true),Disabled);assert_eq!(snapshot(&s.lock().unwrap()),before);
        write(&mut d,3,0);write(&mut d,1,0);
        assert_eq!(s.lock().unwrap().try_key_debounced(1,0,0,true),Accepted);assert!(s.lock().unwrap().irq_high());
        write(&mut d,1,1);assert!(!s.lock().unwrap().irq_high());assert_eq!(read(&mut d,4),0x81);
        assert!(!s.lock().unwrap().irq_high(),"reading A does not silently clear latched INT_STAT");
        write(&mut d,2,1);assert!(s.lock().unwrap().irq_high());
    }
    #[test]
    fn host_transport_preserves_other_raw_keys_two_scan_deadlines_and_fresh_reset_state() {
        use KeyEventAcceptance::*;
        let (s,mut d)=setup();s.lock().unwrap().key(0,0,1,true);
        assert_eq!(s.lock().unwrap().try_key_debounced(0,0,0,true),Accepted);
        assert_eq!(read(&mut d,4),0x81);s.lock().unwrap().advance_to(24999);assert_eq!(read(&mut d,3),0);
        s.lock().unwrap().advance_to(25000);assert_eq!(read(&mut d,3),0);
        s.lock().unwrap().advance_to(50000);assert_eq!(read(&mut d,4),0x82);
        assert_eq!(s.lock().unwrap().key_diagnostics(),KeyEventDiagnostics{accepted:2,consumed:2,pending:0,interrupt_pending:true});
        let fresh=Tca8418State::new(1_000_000);assert_eq!(fresh.key_diagnostics(),KeyEventDiagnostics::default());assert!(fresh.irq_high());
    }

}
