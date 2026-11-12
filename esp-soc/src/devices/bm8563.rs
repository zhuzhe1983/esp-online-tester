//! BM8563 calendar subset, compatible with M5Unified's PCF8563 register driver.
//! BM8563 datasheet pp6-10 (M5Stack copy, no revision), NXP PCF8563 rev11.1
//! §§8.4-8.5, 8.10: BCD, century toggle, four-year leap rule, STOP and burst lock.
//! No host clock, alarm/timer/INT, CLKOUT, analog voltage or oscillator calibration.
use esp_periph::i2c::I2cDevice;
use std::sync::{Arc,Mutex};

const MASKS:[u8;16]=[0xa8,0x1f,0xff,0x7f,0x3f,0x3f,7,0x9f,0xff,0xff,0xbf,0xbf,0x87,0x83,0x83,0xff];
pub struct Bm8563State {regs:[u8;16],cycle:u64,hz:u64,next_tick:u64,access:bool,pending:bool}
impl Bm8563State {
    pub fn new(hz:u64)->Self {
        // BM8563 p6 reset differs from NXP: TF=1, CLKOUT disabled. Calendar
        // starts zero/invalid with VL set; guest software must supply its date.
        let mut regs=[0;16];regs[0]=8;regs[1]=4;regs[2]=0x80;regs[9..13].fill(0x80);regs[14]=3;
        let hz=hz.max(1);Self{regs,cycle:0,hz,next_tick:hz,access:false,pending:false}
    }
    pub fn advance_to(&mut self,cycle:u64) {
        if cycle<=self.cycle {return;}self.cycle=cycle;
        // TEST1 external-clock mode has no supplied source in this subset.
        if self.regs[0]&0xa0!=0 || cycle<self.next_tick {return;}
        let seconds=(cycle-self.next_tick)/self.hz+1;
        self.next_tick=cycle.saturating_add(self.hz-(cycle-self.next_tick)%self.hz);
        if self.access {self.pending=true;}else{self.add_seconds(seconds);}
    }
    fn add_seconds(&mut self,seconds:u64) {
        let bcd=|b:u8,max:u8|->Option<u8>{let n=(b>>4)*10+(b&15);((b&15)<10&&(b>>4)<10&&n<=max).then_some(n)};
        let (Some(sec),Some(min),Some(hour),Some(mut day),Some(mut month),Some(mut year))=(bcd(self.regs[2]&0x7f,59),bcd(self.regs[3],59),bcd(self.regs[4],23),bcd(self.regs[5],31),bcd(self.regs[7]&0x1f,12),bcd(self.regs[8],99)) else {return;};
        // Undefined guest BCD/calendar values are retained, never normalized or
        // used as an index. This is a model boundary, not a silicon prediction.
        if month==0 || day==0 || day>month_days(month,year) || self.regs[6]>6 {return;}
        let time=u64::from(hour)*3600+u64::from(min)*60+u64::from(sec)+seconds%86400;
        let days=seconds/86400+time/86400;let time=time%86400;
        self.regs[2]=(self.regs[2]&0x80)|encode((time%60) as u8);self.regs[3]=encode((time/60%60) as u8);self.regs[4]=encode((time/3600) as u8);
        self.regs[6]=((u64::from(self.regs[6])+days%7)%7) as u8;
        // The hardware's two-digit year treats 00 as leap, regardless of C.
        // Two 100-year cycles restore calendar and C; weekday was handled above.
        let mut left=days%73050;let mut century=self.regs[7]&0x80;
        while left>0 {let remaining=u64::from(month_days(month,year)-day);if left<=remaining {day+=left as u8;break;}left-=remaining+1;day=1;month+=1;if month==13 {month=1;year+=1;if year==100 {year=0;century^=0x80;}}}
        self.regs[5]=encode(day);self.regs[7]=century|encode(month);self.regs[8]=encode(year);
    }
    fn write(&mut self,reg:u8,value:u8) {
        let index=usize::from(reg);let value=value&MASKS[index];
        if reg==0 && (self.regs[0]^value)&0xa0!=0 {
            // STOP resets the divider. Use a deterministic one-second restart;
            // the silicon's subsecond oscillator phase is deliberately unmodeled.
            self.next_tick=self.cycle.saturating_add(self.hz);self.pending=false;
        }
        self.regs[index]=if reg==1 {(value&0x13)|(self.regs[1]&value&0x0c)}else{value};
    }
    fn stop(&mut self) {self.access=false;if self.pending {self.pending=false;self.add_seconds(1);}}
}
fn encode(value:u8)->u8 {(value/10)<<4|value%10}
fn month_days(month:u8,year:u8)->u8 {match month {2=>if year.is_multiple_of(4){29}else{28},4|6|9|11=>30,_=>31}}

pub struct Bm8563 {state:Arc<Mutex<Bm8563State>>,pointer:u8,first:bool}
impl Bm8563 {pub fn new(state:Arc<Mutex<Bm8563State>>)->Self {Self{state,pointer:0,first:true}}}
impl I2cDevice for Bm8563 {
    fn start(&mut self,read:bool)->bool {self.state.lock().expect("BM8563 state poisoned").access=true;if !read {self.first=true;}true}
    fn write(&mut self,value:u8)->bool {if self.first {self.pointer=value&15;self.first=false;}else{self.state.lock().expect("BM8563 state poisoned").write(self.pointer,value);self.pointer=(self.pointer+1)&15;}true}
    fn read(&mut self)->u8 {let value=self.state.lock().expect("BM8563 state poisoned").regs[usize::from(self.pointer)];self.pointer=(self.pointer+1)&15;value}
    fn stop(&mut self) {self.state.lock().expect("BM8563 state poisoned").stop();}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup()->(Arc<Mutex<Bm8563State>>,Bm8563) {let s=Arc::new(Mutex::new(Bm8563State::new(1000)));let d=Bm8563::new(s.clone());(s,d)}
    fn write(d:&mut Bm8563,reg:u8,bytes:&[u8]) {assert!(d.start(false));assert!(d.write(reg));for &b in bytes {assert!(d.write(b));}d.stop();}
    fn read(d:&mut Bm8563,reg:u8,len:usize)->Vec<u8> {d.start(false);d.write(reg);d.start(true);let out=(0..len).map(|_|d.read()).collect();d.stop();out}
    #[test]
    fn guest_sets_time_and_only_guest_cycles_advance_it() {
        let(s,mut d)=setup();assert_eq!(read(&mut d,0,9),[8,4,0x80,0,0,0,0,0,0]);
        s.lock().unwrap().advance_to(10000);assert_eq!(read(&mut d,2,7),[0x80,0,0,0,0,0,0]);
        write(&mut d,0,&[0]);write(&mut d,2,&[0,0,0x12,1,1,1,0x77]);
        s.lock().unwrap().advance_to(10999);assert_eq!(read(&mut d,2,7),[0,0,0x12,1,1,1,0x77]);
        s.lock().unwrap().advance_to(11000);assert_eq!(read(&mut d,2,7),[1,0,0x12,1,1,1,0x77]);
        s.lock().unwrap().advance_to(71000);assert_eq!(read(&mut d,2,3),[1,1,0x12]);
    }
    #[test]
    fn stop_freezes_time_and_resets_fraction_without_clearing_vl() {
        let(s,mut d)=setup();write(&mut d,2,&[0x80,0,0x12,1,1,1,0x77]);s.lock().unwrap().advance_to(500);write(&mut d,0,&[0x20]);
        s.lock().unwrap().advance_to(10000);assert_eq!(read(&mut d,2,3),[0x80,0,0x12]);write(&mut d,0,&[0]);
        s.lock().unwrap().advance_to(10999);assert_eq!(read(&mut d,2,1),[0x80]);s.lock().unwrap().advance_to(11000);assert_eq!(read(&mut d,2,1),[0x81]);
        write(&mut d,2,&[0x09]);s.lock().unwrap().advance_to(12000);assert_eq!(read(&mut d,2,1),[0x10]);
    }
    #[test]
    fn carries_months_leap_days_weekdays_and_century() {
        for (day,month,year,want) in [(0x30,0x04,0x23,[0x01,0x05,0x23]),(0x28,0x02,0x24,[0x29,0x02,0x24]),(0x28,0x02,0x23,[0x01,0x03,0x23]),(0x28,0x82,0x00,[0x29,0x82,0x00]),(0x31,0x92,0x99,[0x01,0x01,0x00]),(0x31,0x12,0x99,[0x01,0x81,0x00])] {
            let(s,mut d)=setup();write(&mut d,2,&[0x59,0x59,0x23,day,6,month,year]);s.lock().unwrap().advance_to(1000);
            assert_eq!(read(&mut d,2,7),[0,0,0,want[0],0,want[1],want[2]]);
        }
    }
    #[test]
    fn a_burst_holds_the_calendar_and_services_only_one_pending_tick() {
        let(s,mut d)=setup();write(&mut d,2,&[0x59,0x59,0x23,0x31,2,0x12,0x77]);
        d.start(false);d.write(2);d.start(true);assert_eq!(d.read(),0x59);s.lock().unwrap().advance_to(3000);
        assert_eq!((0..6).map(|_|d.read()).collect::<Vec<_>>(),[0x59,0x23,0x31,2,0x12,0x77]);d.stop();assert_eq!(read(&mut d,2,7),[0,0,0,1,3,1,0x78]);
    }
    #[test]
    fn invalid_guest_dates_remain_visible_and_large_jumps_are_bounded() {
        for reg in 2..=8 {for value in 0..=255 {
            let(s,mut d)=setup();write(&mut d,2,&[0,0,0x12,1,1,1,0x77]);write(&mut d,reg,&[value]);s.lock().unwrap().advance_to(u64::MAX);let _=read(&mut d,2,7);
        }}
        let(s,mut d)=setup();write(&mut d,2,&[0x6a,0,0x12,1,1,0x13,0x77]);s.lock().unwrap().advance_to(100000);assert_eq!(read(&mut d,2,7),[0x6a,0,0x12,1,1,0x13,0x77]);
        write(&mut d,2,&[0,0,0,1,0,1,0]);s.lock().unwrap().advance_to(100000+73050*86400*1000);assert_eq!(read(&mut d,2,7),[0,0,0,1,5,1,0]);
    }
    #[test]
    fn flags_are_write_zero_to_clear_and_register_pointer_wraps() {
        let(_,mut d)=setup();write(&mut d,1,&[0xff]);assert_eq!(read(&mut d,1,1),[0x17]);write(&mut d,1,&[0]);write(&mut d,1,&[0x0c]);assert_eq!(read(&mut d,1,1),[0]);
        write(&mut d,0xff,&[0x5a,0xff]);assert_eq!(read(&mut d,0x0f,2),[0x5a,0xa8]);write(&mut d,3,&[0xff,0xff]);assert_eq!(read(&mut d,3,2),[0x7f,0x3f]);
    }
}
