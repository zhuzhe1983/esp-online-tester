//! StackChan K151 base: PY32 I/O/RGB, Si12T and two SCSCL serial servos.
//! Protocol: m5stack/StackChan-BSP e2f3b02dbe7f28e26a02adc72bef3aa2c545e4a0,
//! drivers/{PY32IOExpander,Si12T,FTServo_Arduino} and M5StackChan.cpp.
//! Si12T register semantics: Si12T_Datasheet_EN.pdf rev0.1 (2023-11-13), §§12.2.3–7.
//! Position moves use the requested goal-time as ideal linear kinematics; no load,
//! inertia, stall or calibrated PWM-to-speed model. PWM is observable but does not
//! invent angular displacement. Neutral raw positions are BSP defaults, not calibration.
use std::sync::{Arc,Mutex};
use std::collections::VecDeque;
use esp_periph::i2c::I2cDevice;
use esp_soc::uart::{UartInput,UartRoute};

const HZ:u64=crate::periph::CPU_HZ;
const SERVO_BAUDS:[u32;12]=[1_000_000,500_000,250_000,128_000,115_200,76_800,57_600,38_400,19_200,14_400,9_600,4_800];
const NEUTRAL:[u16;2]=[460,620];

struct Servo {
    regs:[u8;71],position:f64,from:f64,target:f64,start:u64,end:u64,pending:Option<(u8,Vec<u8>)>,
}
impl Servo {
    fn new(id:u8,position:u16)->Self {
        // SCSCL RAM table: torque starts released; a position command engages it.
        let mut regs=[0;71];regs[5]=id;regs[6]=0;regs[48]=1;
        regs[11..13].copy_from_slice(&1000u16.to_be_bytes());regs[42..44].copy_from_slice(&position.to_be_bytes());
        Self{regs,position:position as f64,from:position as f64,target:position as f64,start:0,end:0,pending:None}
    }
    fn word(&self,r:usize)->u16 {u16::from_be_bytes([self.regs[r],self.regs[r+1]])}
    fn pwm_mode(&self)->bool {self.word(9)==0 && self.word(11)==0}
    fn pwm(&self)->i32 {let v=self.word(44);let n=(v&0x3ff) as i32;if v&0x400!=0 {-n} else {n}}
    fn moving(&self,supply:bool)->bool {supply && self.regs[40]!=0 && if self.pwm_mode() {self.pwm()!=0} else {self.position!=self.target}}
    fn advance(&mut self,cycle:u64,powered:bool)->bool {
        if !powered || self.regs[40]==0 || self.pwm_mode() {return false;}
        let before=self.position;
        self.position=if cycle>=self.end || self.end<=self.start {self.target} else {self.from+(self.target-self.from)*(cycle.saturating_sub(self.start) as f64)/(self.end-self.start) as f64};
        before!=self.position
    }
    fn stop(&mut self) {self.target=self.position;self.from=self.position;self.end=self.start;self.pending=None;self.regs[44]=0;self.regs[45]=0;}
    fn write(&mut self,reg:u8,data:&[u8],cycle:u64,powered:bool)->u8 {
        let start=reg as usize;let end=start.saturating_add(data.len());
        // Read-only feedback, unsupported addresses and malformed ranges never mutate state.
        if data.is_empty() || end>49 || (start..end).any(|r|!matches!(r,5|6|9..=12|26|27|40..=48)) {return 0x08;}
        let mut candidate=self.regs;candidate[start..end].copy_from_slice(data);
        if candidate[5]>=0xfe || candidate[6] as usize>=SERVO_BAUDS.len() || u16::from_be_bytes([candidate[42],candidate[43]])>1000 {return 0x08;}
        if u16::from_be_bytes([candidate[9],candidate[10]])>1000 || u16::from_be_bytes([candidate[11],candidate[12]])>1000 {return 0x08;}
        self.advance(cycle,powered);let was_torque=self.regs[40]!=0;self.regs=candidate;
        // FEETECH ftservo-wiki 5637b798, memory-scscl.md §4.1: goal position (42) automatically
        // enables torque (40). The factory firmware releases torque at rest and
        // relies on this for its next move; it does not send a separate enable.
        if powered && !self.pwm_mode() && start<=42 && end>=44 {self.regs[40]=1;}
        if self.regs[40]==0 {self.stop();}
        else if (start<48 && end>42) || (!was_torque && self.regs[40]!=0) {
            self.from=self.position;self.target=self.word(42) as f64;self.start=cycle;
            self.end=cycle.saturating_add((self.word(44) as u64).saturating_mul(HZ/1000));
            if !self.pwm_mode() && self.end==self.start && powered {self.position=self.target;}
        }
        0
    }
    fn read(&self,r:usize,supply:bool)->u8 {
        let pos=self.position.round().clamp(0.0,1000.0) as u16;
        match r {
            56=>pos.to_be_bytes()[0],57=>pos.to_be_bytes()[1],66=>u8::from(self.moving(supply)),
            // No virtual measured current, temperature, voltage or load is asserted.
            58..=65|69..=70=>0,
            5|6|9..=12|26|27|40..=48=>self.regs[r],
            _=>0xff,
        }
    }
}

struct State {
    powered:bool,py:[u8;256],touch:[u8;64],pads:[u8;3],leds:[[u8;3];12],led_version:u64,
    servos:[Servo;2],motion_version:u64,cycle:u64,rx:VecDeque<(u64,u32,Vec<u8>)>,packet:Vec<u8>,last_tx:u64,
}
impl State {
    fn new()->Self {
        // Version 1 identifies this emulated register protocol, not a measured PY32 firmware.
        let mut py=[0;256];py[2]=1;
        let mut touch=[0;64];touch[8]=0x23;touch[9]=7;touch[0x0a]=0xfe;touch[0x0b]=0x0f;touch[0x0c]=0xfe;touch[0x0d]=0x3f;
        Self{powered:true,py,touch,pads:[0;3],leds:[[0;3];12],led_version:0,servos:[Servo::new(1,NEUTRAL[0]),Servo::new(2,NEUTRAL[1])],motion_version:0,cycle:0,rx:VecDeque::new(),packet:Vec::new(),last_tx:0}
    }
    fn servo_power(&self)->bool {self.powered && self.py[3]&1!=0 && self.py[5]&1!=0}
    fn advance(&mut self,cycle:u64) {
        self.cycle=self.cycle.max(cycle);let powered=self.servo_power();
        let mut changed=false;for servo in &mut self.servos {changed|=servo.advance(self.cycle,powered);}
        if changed {self.motion_version=self.motion_version.wrapping_add(1);}
    }
    fn py_write(&mut self,r:u8,value:u8) {
        let was_power=self.servo_power();let r=r as usize;
        match r {
            3..=6|9..=16|19..=20|0x1b..=0x22|0x25..=0x26|0x30..=0x6f=>self.py[r]=value,
            0x11|0x12=>self.py[r]&=!value,
            0x24=>{
                self.py[r]=value&0x3f;
                // IO14 (zero-based13) must be an enabled push-pull output. A RAM
                // write alone cannot update the WS2812 chain; CFG bit6 emits it.
                if value&0x40!=0 && self.py[4]&0x20!=0 && self.py[0x14]&0x20==0 {
                    for i in 0..usize::from((value&0x3f).min(12)) {
                        let rgb=u16::from_le_bytes([self.py[0x30+i*2],self.py[0x31+i*2]]) as u32;
                        self.leds[i]=[(((rgb>>11)&31)*255/31) as u8,(((rgb>>5)&63)*255/63) as u8,((rgb&31)*255/31) as u8];
                    }
                    self.led_version=self.led_version.wrapping_add(1);
                }
            }
            _=>{},
        }
        if was_power!=self.servo_power() {
            self.packet.clear();self.rx.clear();
            if !self.servo_power() {for servo in &mut self.servos {servo.stop();}}
            self.motion_version=self.motion_version.wrapping_add(1);
        }
    }
    fn touch_output(&self,r:u8)->u8 {
        if r!=0x10 || self.touch[9]&8!=0 {return 0;}
        let mut value=0;
        for channel in 0..3 {
            if self.touch[0x0c]&(1<<channel)==0 && self.touch[0x0a]&(1<<channel)==0 {value|=self.pads[channel]<<(channel*2);}
        }
        value
    }
    fn reply(&mut self,id:u8,status:u8,data:&[u8],baud:u32) {
        // UART FIFO transport handles baud mismatch and wrong RX routing. Keep bounded
        // responses, and allow one complete 8N1 character after the command.
        if self.rx.len()>=16 {return;}
        let mut packet=vec![0xff,0xff,id,(data.len()+2) as u8,status];packet.extend_from_slice(data);
        let sum=packet[2..].iter().fold(0u8,|s,b|s.wrapping_add(*b));packet.push(!sum);
        self.rx.push_back((self.cycle.saturating_add(HZ*10/u64::from(baud)),baud,packet));
    }
    fn command(&mut self,packet:&[u8],route:UartRoute) {
        if packet.len()<6 || packet[2..].iter().fold(0u8,|s,b|s.wrapping_add(*b))!=0xff {return;}
        let id=packet[2];let instruction=packet[4];let params=&packet[5..packet.len()-1];
        let broadcast=id==0xfe;let supply=self.servo_power();let cycle=self.cycle;
        if broadcast && instruction==0x83 {
            if params.len()<2 || params[1]==0 {return;}
            let width=usize::from(params[1])+1;
            if !(params.len()-2).is_multiple_of(width) {return;}
            for chunk in params[2..].chunks_exact(width) {for servo in &mut self.servos {
                if servo.regs[5]==chunk[0] && route.matches_baud(SERVO_BAUDS[servo.regs[6] as usize]) {servo.write(params[0],&chunk[1..],cycle,supply);}
            }}
            self.motion_version=self.motion_version.wrapping_add(1);return;
        }
        for index in 0..self.servos.len() {
            let servo=&mut self.servos[index];let baud=SERVO_BAUDS[servo.regs[6] as usize];
            if (!broadcast && id!=servo.regs[5]) || !route.matches_baud(baud) {continue;}
            let mut status=0;let mut data=Vec::new();
            match instruction {
                1 if params.is_empty()=>{},
                2 if params.len()==2=>{let start=params[0] as usize;let end=start+params[1] as usize;if end<=71 {data=(start..end).map(|r|servo.read(r,supply)).collect();} else {status=0x08;}},
                3 if params.len()>1=>{status=servo.write(params[0],&params[1..],cycle,supply);},
                4 if params.len()>1=>{if params.len()<=50 {servo.pending=Some((params[0],params[1..].to_vec()));} else {status=0x08;}},
                5 if params.is_empty()=>{if let Some((r,bytes))=servo.pending.take() {status=servo.write(r,&bytes,cycle,supply);}},
                _=>status=0x40,
            }
            let reply_id=servo.regs[5];
            if !broadcast {self.reply(reply_id,status,&data,baud);}
            if matches!(instruction,3..=5) {self.motion_version=self.motion_version.wrapping_add(1);}
        }
    }
    fn tx(&mut self,cycle:u64,route:UartRoute,byte:u8) {
        self.advance(cycle);
        // Unrelated UART traffic cannot disturb a partial packet on G6.
        if !route.transmits_on(6) {return;}
        if !self.servo_power() || !self.servos.iter().any(|s|route.matches_baud(SERVO_BAUDS[s.regs[6] as usize])) {self.packet.clear();return;}
        if cycle.saturating_sub(self.last_tx)>HZ/50 {self.packet.clear();}self.last_tx=cycle;
        if self.packet.len()<2 {
            if byte==0xff {self.packet.push(byte);} else {self.packet.clear();}return;
        }
        if self.packet.len()==2 && byte==0xff {return;}
        self.packet.push(byte);
        if self.packet.len()==4 && self.packet[3]<2 {self.packet.clear();return;}
        if self.packet.len()>=4 && self.packet.len()==usize::from(self.packet[3])+4 {
            let packet=std::mem::take(&mut self.packet);self.command(&packet,route);
        }
    }
}

#[derive(Clone,Copy)]
enum Kind {Py,Touch}
struct BaseDevice {state:Arc<Mutex<State>>,kind:Kind,ptr:u8,first:bool}
impl I2cDevice for BaseDevice {
    fn pins(&self)->Option<(u8,u8)> {Some((12,11))}
    fn start(&mut self,read:bool)->bool {if !read {self.first=true;}self.state.lock().expect("StackChan base state mutex poisoned").powered}
    fn write(&mut self,value:u8)->bool {
        let mut state=self.state.lock().expect("StackChan base state mutex poisoned");if !state.powered {return false;}
        if self.first {self.ptr=value;self.first=false;return true;}
        match self.kind {
            Kind::Py=>state.py_write(self.ptr,value),
            Kind::Touch=>if matches!(self.ptr,2..=0x0f|0x3b..=0x3d) {
                let ptr=self.ptr as usize;
                state.touch[ptr]=if ptr==9 {value&0x0c|3} else {value};
            },
        }
        self.ptr=self.ptr.wrapping_add(1);true
    }
    fn read(&mut self)->u8 {
        let state=self.state.lock().expect("StackChan base state mutex poisoned");
        let value=if !state.powered {0xff} else {match self.kind {
            Kind::Py=>match self.ptr {7=>state.py[5]&state.py[3],8=>state.py[6]&state.py[4],_=>state.py[self.ptr as usize]},
            Kind::Touch=>match self.ptr {0x10..=0x12=>state.touch_output(self.ptr),_=>state.touch.get(self.ptr as usize).copied().unwrap_or(0xff)},
        }};
        self.ptr=self.ptr.wrapping_add(1);value
    }
}

pub struct StackChanBase {state:Arc<Mutex<State>>,led_cache:[[u8;3];12],led_version:u64}
impl Default for StackChanBase {fn default()->Self {Self::new()}}
impl StackChanBase {
    pub fn new()->Self {Self{state:Arc::new(Mutex::new(State::new())),led_cache:[[0;3];12],led_version:0}}
    pub fn i2c_devices(&self)->Vec<(u8,Box<dyn I2cDevice>)> {
        [(0x6f,Kind::Py),(0x68,Kind::Touch)].into_iter().map(|(address,kind)|(address,Box::new(BaseDevice{state:self.state.clone(),kind,ptr:0,first:true}) as Box<dyn I2cDevice>)).collect()
    }
    pub fn button(&mut self,id:&str,pressed:bool)->bool {
        // BSP TouchSensor reverses the raw channels: OUT3 front, OUT2 middle, OUT1 back.
        let channel=match id {"touch-pad-front"=>2,"touch-pad-middle"=>1,"touch-pad-back"=>0,_=>return false};
        let mut state=self.state.lock().expect("StackChan base state mutex poisoned");state.pads[channel]=if pressed {3} else {0};true
    }
    pub fn set_powered(&mut self,powered:bool) {
        let mut state=self.state.lock().expect("StackChan base state mutex poisoned");
        if state.powered==powered {return;}
        let positions=[state.servos[0].position,state.servos[1].position];let led_version=state.led_version.wrapping_add(1);let version=state.motion_version.wrapping_add(1);let cycle=state.cycle;
        *state=State::new();state.powered=powered;state.cycle=cycle;state.led_version=led_version;state.motion_version=version;
        for (servo,position) in state.servos.iter_mut().zip(positions) {servo.position=position;servo.stop();}
        self.led_cache=state.leds;self.led_version=state.led_version;
    }
    pub fn advance_to(&mut self,cycle:u64) {let mut state=self.state.lock().expect("StackChan base state mutex poisoned");state.advance(cycle);self.led_cache=state.leds;self.led_version=state.led_version;}
    pub fn uart_tx(&mut self,cycle:u64,route:UartRoute,byte:u8) {self.state.lock().expect("StackChan base state mutex poisoned").tx(cycle,route,byte);}
    pub fn uart_rx(&mut self,cycle:u64)->Vec<UartInput> {
        self.advance_to(cycle);let mut state=self.state.lock().expect("StackChan base state mutex poisoned");let mut inputs=Vec::new();
        if !state.servo_power() {state.rx.clear();return inputs;}
        while state.rx.front().is_some_and(|(at,_,_)|*at<=cycle) {if let Some((_,baud,data))=state.rx.pop_front() {inputs.push(UartInput::new(7,baud,data));}}
        inputs
    }
    pub fn leds(&self)->(&[[u8;3]],u64) {(&self.led_cache,self.led_version)}
    pub fn motion_snapshot(&self)->String {
        let state=self.state.lock().expect("StackChan base state mutex poisoned");let supply=state.servo_power();
        let axes=state.servos.iter().enumerate().map(|(i,s)|format!("{{\"id\":\"{}\",\"positionDegrees\":{:.4},\"targetDegrees\":{:.4},\"enabled\":{},\"moving\":{},\"mode\":\"{}\",\"pwm\":{}}}",if i==0 {"pan"} else {"tilt"},(s.position-f64::from(NEUTRAL[i]))*5.0/16.0,(s.target-f64::from(NEUTRAL[i]))*5.0/16.0,supply&&s.regs[40]!=0,s.moving(supply),if s.pwm_mode() {"pwm"} else {"position"},if s.pwm_mode() {s.pwm()} else {0})).collect::<Vec<_>>().join(",");
        format!("{{\"axes\":[{axes}],\"version\":{},\"supply\":{supply}}}",state.motion_version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const BAUD:u32=1_000_000;
    fn write(d:&mut dyn I2cDevice,r:u8,data:&[u8]) {assert!(d.start(false));assert!(d.write(r));for &b in data {assert!(d.write(b));}d.stop();}
    fn read(d:&mut dyn I2cDevice,r:u8,n:usize)->Vec<u8> {assert!(d.start(false));d.write(r);assert!(d.start(true));let bytes=(0..n).map(|_|d.read()).collect();d.stop();bytes}
    fn powered()->StackChanBase {let b=StackChanBase::new();let mut d=b.i2c_devices();let py=d[0].1.as_mut();write(py,3,&[1]);write(py,5,&[1]);b}
    fn packet(id:u8,instruction:u8,params:&[u8])->Vec<u8> {let mut bytes=vec![0xff,0xff,id,(params.len()+2) as u8,instruction];bytes.extend_from_slice(params);let sum=bytes[2..].iter().fold(0u8,|s,b|s.wrapping_add(*b));bytes.push(!sum);bytes}
    fn send(b:&mut StackChanBase,cycle:u64,id:u8,instruction:u8,params:&[u8])->Vec<u8> {for byte in packet(id,instruction,params) {b.uart_tx(cycle,UartRoute::new(1,1<<6,Some(7),Some(BAUD)),byte);}b.uart_rx(cycle+HZ/1000).into_iter().flat_map(|r|r.data).collect()}
    #[test]
    fn rgb_requires_output_route_and_refresh_then_power_cycle_discards_old_color() {
        let mut b=StackChanBase::new();let mut d=b.i2c_devices();assert_eq!(d[0].1.pins(),Some((12,11)));let py=d[0].1.as_mut();
        write(py,0x30,&[0,0xf8,0xe0,7,0x1f,0]);write(py,0x24,&[0x43]);b.advance_to(1);assert_eq!(b.leds().0,[[0;3];12]);
        write(py,4,&[0x20]);b.advance_to(2);assert_eq!(b.leds().0,[[0;3];12]);write(py,0x24,&[0x43]);b.advance_to(3);assert_eq!(&b.leds().0[..3],&[[255,0,0],[0,255,0],[0,0,255]]);assert_eq!(read(py,0x24,1),[3]);
        let version=b.leds().1;write(py,0x30,&[0,0]);b.advance_to(4);assert_eq!(b.leds().1,version);assert_eq!(b.leds().0[0],[255,0,0]);
        b.set_powered(false);assert!(!py.start(false));assert_eq!(b.leds().0,[[0;3];12]);b.set_powered(true);write(py,0x24,&[0x4c]);b.advance_to(5);assert_eq!(b.leds().0,[[0;3];12]);
    }
    #[test]
    fn three_touch_zones_are_read_only_with_hold_and_reset_gates() {
        let mut b=StackChanBase::new();let mut d=b.i2c_devices();let t=d[1].1.as_mut();write(t,0x0a,&[0,0,0,0]);
        assert!(b.button("touch-pad-front",true));assert_eq!(read(t,0x10,3),[0x30,0,0]);b.button("touch-pad-back",true);assert_eq!(read(t,0x10,1),[0x33]);
        write(t,0x10,&[0]);assert_eq!(read(t,0x10,1),[0x33]);write(t,0x0c,&[4]);assert_eq!(read(t,0x10,1),[3]);write(t,9,&[0x0f]);assert_eq!(read(t,0x10,1),[0]);write(t,9,&[7]);assert_eq!(read(t,0x10,1),[3]);
        b.button("touch-pad-back",false);assert_eq!(read(t,0x10,1),[0]);assert!(!b.button("touch-left",true));
    }
    #[test]
    fn uart_routes_checksum_ids_and_power_are_required() {
        let mut b=powered();let command=packet(1,1,&[]);
        for &(pin,baud) in &[(5,BAUD),(6,115_200)] {for &byte in &command {b.uart_tx(1,UartRoute::new(1,1<<pin,Some(7),Some(baud)),byte);}assert!(b.uart_rx(HZ/100).is_empty());}
        let mut bad=command.clone();*bad.last_mut().unwrap()^=1;for byte in bad {b.uart_tx(HZ/100,UartRoute::new(1,1<<6,Some(7),Some(BAUD)),byte);}assert!(b.uart_rx(HZ/50).is_empty());
        assert!(send(&mut b,HZ/50,3,1,&[]).is_empty());
        for byte in packet(2,1,&[]) {b.uart_tx(HZ/40,UartRoute::new(1,1<<6,Some(7),Some(BAUD)),byte);b.uart_tx(HZ/40,UartRoute::new(0,1<<43,Some(44),Some(115_200)),b'x');}
        assert_eq!(b.uart_rx(HZ/40+HZ/1000)[0].data,[0xff,0xff,2,2,0,0xfb]);assert_eq!(send(&mut b,HZ/30,1,1,&[]),[0xff,0xff,1,2,0,0xfc]);
        let mut d=b.i2c_devices();write(d[0].1.as_mut(),5,&[0]);assert!(send(&mut b,HZ/20,1,1,&[]).is_empty());
    }
    #[test]
    fn servo_feedback_tracks_goal_time_and_power_loss_cancels_movement() {
        let mut b=powered();let at=HZ;assert_eq!(send(&mut b,at,1,3,&[42,0x02,0x8c,0,100,0,0]),[0xff,0xff,1,2,0,0xfc]);
        // 460 -> 652 raw, 60 degrees, requested 100ms. Read at 50ms reports midpoint.
        let reply=send(&mut b,at+HZ/20,1,2,&[56,2]);assert_eq!(&reply[5..7],&556u16.to_be_bytes());assert!(b.motion_snapshot().contains("\"moving\":true"));
        b.advance_to(at+HZ/10);assert!(b.motion_snapshot().contains("\"positionDegrees\":60.0000"));assert_eq!(&send(&mut b,at+HZ/10,1,2,&[66,1])[5..6],&[0]);
        send(&mut b,at+HZ/5,1,3,&[42,0x03,0x20,0,100,0,0]);let mut d=b.i2c_devices();write(d[0].1.as_mut(),5,&[0]);b.advance_to(at+HZ);let stopped=b.motion_snapshot();write(d[0].1.as_mut(),5,&[1]);b.advance_to(at+2*HZ);assert!(stopped.contains("\"supply\":false"));assert!(!b.motion_snapshot().contains("\"moving\":true"));
    }
    #[test]
    fn position_command_reengages_released_torque_but_invalid_writes_and_reads_do_not() {
        let mut b=powered();let at=HZ;
        send(&mut b,at,1,3,&[40,0]);
        assert_eq!(&send(&mut b,at+HZ/100,1,2,&[40,2])[5..7],&[0,0]);
        assert_eq!(send(&mut b,at+HZ/50,1,3,&[42,0xff,0xff])[4],8);
        send(&mut b,at+HZ/40,1,3,&[48,0]);b.advance_to(at+HZ/30);
        assert!(b.motion_snapshot().contains("\"positionDegrees\":0.0000,\"targetDegrees\":0.0000,\"enabled\":false"));
        // The same six-byte WritePos used by official V1.5.1, following its idle release.
        send(&mut b,at+HZ/10,1,3,&[42,0x02,0x8c,0,100,0,0]);
        assert_eq!(&send(&mut b,at+HZ*11/100,1,2,&[40,2])[5..7],&[1,0]);
        b.advance_to(at+HZ/5);assert!(b.motion_snapshot().contains("\"positionDegrees\":60.0000"));
        send(&mut b,at+HZ/4,1,3,&[40,0]);b.advance_to(at+HZ/3);
        assert_eq!(&send(&mut b,at+HZ/3,1,2,&[40,2])[5..7],&[0,0]);
        assert_eq!(&send(&mut b,at+HZ*35/100,1,2,&[56,2])[5..7],&652u16.to_be_bytes());
    }
    #[test]
    fn queued_and_sync_position_commands_reengage_only_when_executed() {
        let mut b=powered();send(&mut b,HZ,0xfe,3,&[40,0]);
        send(&mut b,HZ+HZ/10,1,4,&[42,2,0,0,0,0,0]);
        assert_eq!(&send(&mut b,HZ+HZ/5,1,2,&[40,2])[5..7],&[0,0]);
        assert_eq!(&send(&mut b,HZ+HZ*21/100,1,2,&[56,2])[5..7],&460u16.to_be_bytes());
        send(&mut b,HZ+HZ/4,0xfe,5,&[]);
        assert_eq!(&send(&mut b,HZ+HZ/3,1,2,&[40,2])[5..7],&[1,0]);
        assert_eq!(&send(&mut b,HZ+HZ*35/100,1,2,&[56,2])[5..7],&512u16.to_be_bytes());
        send(&mut b,2*HZ,0xfe,3,&[40,0]);
        send(&mut b,2*HZ+HZ/10,0xfe,0x83,&[42,6,1,1,0xf4,0,0,0,0,2,2,0xd0,0,0,0,0]);
        for (id,pos) in [(1,500u16),(2,720)] {
            let at=3*HZ+u64::from(id)*HZ/10;
            assert_eq!(&send(&mut b,at,id,2,&[40,2])[5..7],&[1,0]);
            assert_eq!(&send(&mut b,at+HZ/100,id,2,&[56,2])[5..7],&pos.to_be_bytes());
        }
    }
    #[test]
    fn sync_and_registered_writes_are_atomic_and_bad_ranges_do_not_panic() {
        let mut b=powered();assert!(send(&mut b,HZ/1000,0xfe,0x83,&[42,6,1,1,0xf4,0,0,0,0,2,2,0xd0,0,0,0,0]).is_empty());
        assert_eq!(&send(&mut b,HZ/200,1,2,&[56,2])[5..7],&500u16.to_be_bytes());assert_eq!(&send(&mut b,HZ/150,2,2,&[56,2])[5..7],&720u16.to_be_bytes());
        send(&mut b,HZ/100,1,4,&[42,2,0,0,0,0,0]);assert_eq!(&send(&mut b,HZ/50,1,2,&[56,2])[5..7],&500u16.to_be_bytes());send(&mut b,HZ/40,0xfe,5,&[]);assert_eq!(&send(&mut b,HZ/30,1,2,&[56,2])[5..7],&512u16.to_be_bytes());
        assert_eq!(send(&mut b,HZ/20,1,3,&[42,0xff,0xff])[4],8);assert_eq!(send(&mut b,HZ/10,1,2,&[255,255])[4],8);assert_eq!(&send(&mut b,HZ/5,1,2,&[56,2])[5..7],&512u16.to_be_bytes());
    }
}
