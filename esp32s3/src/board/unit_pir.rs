//! U004 nominal M5Stack 2 s retriggerable motion profile, not calibrated AS312 timing.
//! M5Stack Unit_PIR product page specifies 2 s, repeatable trigger, white OUT/yellow NC.
//! AS312/HT7533/1k OUT series resistor: official UNIT_PIR.pdf schematic.
//! Mirrored Senba AS312 manuals disagree at the typical table (2.3 s on/lock).
//! This profile does not model that lock window, warm-up, optical response or noise.
use esp_soc::board::{BoardEdge,VirtualCycle};

pub(super) struct UnitPir {
    cycle:u64,hz:u64,motion:bool,connected:bool,powered:bool,
    until:Option<u64>,level:Option<bool>,edges:Vec<BoardEdge>,release:bool,
}
impl UnitPir {
    pub fn new(hz:u64)->Self {Self{cycle:0,hz,motion:false,connected:false,powered:false,until:None,level:None,edges:Vec::new(),release:false}}
    fn drive(&mut self,cycle:VirtualCycle,level:Option<bool>) {
        if self.level==level {return;}self.level=level;
        if let Some(level)=level {self.release=false;self.edges.push(BoardEdge{cycle,pin:11,level});}
        else {self.edges.clear();self.release=true;}
    }
    pub fn input_at(&mut self,cycle:u64,connected:bool,motion:bool) {
        self.advance_to(cycle);self.connected=connected;
        if self.powered&&self.motion&&!motion {self.until=Some(self.cycle.saturating_add(self.hz.saturating_mul(2)));}
        if motion {self.until=None;}self.motion=motion;
        if self.powered&&motion {self.drive(self.cycle,Some(true));}
    }
    pub fn set_power(&mut self,supply:bool) {
        let powered=supply&&self.connected;if self.powered==powered {return;}
        self.powered=powered;self.until=None;
        // High impedance without supply is an explicit ideal-interface policy, not
        // a datasheet guarantee about AS312 leakage/backfeed in an unpowered unit.
        self.drive(self.cycle,powered.then_some(self.motion));
    }
    pub fn advance_to(&mut self,cycle:u64) {
        self.cycle=self.cycle.max(cycle);
        if let Some(until)=self.until {if self.cycle>=until {self.until=None;self.drive(until,Some(false));}}
    }
    pub fn deadline(&self)->Option<u64> {self.until}
    pub fn level(&self)->Option<bool> {self.level}
    pub fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    pub fn released_inputs(&mut self)->Vec<u8> {if std::mem::take(&mut self.release) {vec![11]}else{vec![]}}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn motion_stays_high_then_holds_for_two_seconds_and_retriggers() {
        let mut p=UnitPir::new(1000);p.input_at(0,true,false);p.set_power(true);assert_eq!(p.level(),Some(false));p.take_edges();
        p.input_at(100,true,true);assert_eq!(p.take_edges(),[BoardEdge{cycle:100,pin:11,level:true}]);p.advance_to(10_000);assert_eq!(p.level(),Some(true));assert_eq!(p.deadline(),None);
        p.input_at(10_000,true,false);assert_eq!(p.deadline(),Some(12_000));p.advance_to(11_999);assert!(p.take_edges().is_empty());
        p.input_at(11_999,true,true);assert_eq!(p.deadline(),None);p.input_at(12_000,true,false);assert_eq!(p.deadline(),Some(14_000));
        p.advance_to(15_000);assert_eq!(p.take_edges(),[BoardEdge{cycle:14_000,pin:11,level:false}]);assert_eq!(p.level(),Some(false));assert_eq!(p.deadline(),None);
    }
    #[test] fn supply_and_disconnect_release_drive_and_clear_old_hold() {
        let mut p=UnitPir::new(1000);p.input_at(0,true,true);assert_eq!(p.level(),None);p.set_power(true);assert_eq!(p.level(),Some(true));p.take_edges();
        p.input_at(20,true,false);assert_eq!(p.deadline(),Some(2020));p.set_power(false);assert_eq!(p.level(),None);assert_eq!(p.deadline(),None);assert_eq!(p.released_inputs(),[11]);
        p.set_power(true);assert_eq!(p.level(),Some(false));p.take_edges();p.input_at(30,false,true);p.set_power(true);assert_eq!(p.level(),None);assert_eq!(p.released_inputs(),[11]);p.advance_to(10_000);assert!(p.take_edges().is_empty());
        p.input_at(10_000,true,true);p.set_power(true);assert_eq!(p.level(),Some(true));assert_eq!(p.take_edges(),[BoardEdge{cycle:10_000,pin:11,level:true}]);
    }
}
