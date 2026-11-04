//! M5Stack K138 StampFly carrier, official M5StampFly 2b96752c source.
//! Factory firmware uses IDF v4.4.5. Physical routes, digital sensors and
//! latched motor PWM only: no thrust, attitude, collision or radio controller.
use esp_soc::board::{BoardEdge,BoardModel,SpiPins,VirtualCycle};
use esp_soc::devices::Ws2812Chain;
use esp_soc::pins::PinRoutes;
use esp_periph::i2c::{I2cDevice,bmi270::{Bmi270,Bmi270Sample},ina3221::{Ina3221,Ina3221State}};
use esp_periph::i2c::vl53l3cx::{Vl53l3cx,Vl53l3cxState};
use std::sync::{Arc,Mutex};

const HZ:u64=crate::periph::CPU_HZ;
const MOTORS:[(&str,u8);4]=[("front-left",5),("front-right",42),("rear-left",10),("rear-right",41)];
pub struct StampFly {
    v11:bool,cycle:u64,stamp:Ws2812Chain,stamp_power:bool,carrier:Ws2812Chain,leds:[[u8;3];3],
    sample:Arc<Mutex<Bmi270Sample>>,imu:Bmi270,power:Arc<Mutex<Ina3221State>>,
    tof:[Arc<Mutex<Vl53l3cxState>>;2],tof_irq:[bool;2],
    motors:[(f64,u32);4],motor_version:u64,spi_transfers:u64,edges:Vec<BoardEdge>,
}
impl StampFly {
    pub fn new()->Self {
        let sample=Arc::new(Mutex::new(Bmi270Sample{accel_g:[0.0,0.0,1.0],..Default::default()}));
        let mut power=Ina3221State::new(HZ);power.set_sample(1,4.0,0.0);
        // Both official schematics use R4=4.7k to 3V3 on XSHUT. Their
        // initial address collision is real; firmware disables and renumbers one.
        let tof=std::array::from_fn(|_|{let mut s=Vl53l3cxState::new(HZ);s.set_enabled(0,true);Arc::new(Mutex::new(s))});
        Self{v11:false,cycle:0,stamp:Ws2812Chain::new(1),stamp_power:true,carrier:Ws2812Chain::new(2),leds:[[0;3];3],imu:Bmi270::new(sample.clone()),sample,power:Arc::new(Mutex::new(power)),tof,tof_irq:[true;2],motors:[(0.0,0);4],motor_version:0,spi_transfers:0,edges:Vec::new()}
    }
    pub fn v11()->Self {Self{v11:true,stamp_power:false,..Self::new()}}
}
impl Default for StampFly {fn default()->Self {Self::new()}}
impl BoardModel for StampFly {
    fn name(&self)->&'static str {if self.v11 {"stampfly-v11"}else{"stampfly"}}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        self.advance_to(cycle);
        // Stamp-S3A schematic v0.3.3: G38/DISP_BL -> AW35122 EN -> BL_3V3.
        let powered=!self.v11 || routes.software_output(38)==Some(true);
        if self.stamp_power && !powered {self.stamp.leds.fill([0;3]);self.stamp.updates+=1;self.leds[0]=[0;3];}
        self.stamp_power=powered;
        for (sensor,pin) in self.tof.iter().zip([7,9]) {sensor.lock().expect("StampFly ToF mutex poisoned").set_enabled(cycle,routes.software_output(pin).unwrap_or(true));}
        self.advance_to(cycle);
    }
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if pins.sclk&(1<<44)==0 || pins.mosi&(1<<14)==0 || pins.cs&(1<<46)==0 {return vec![0xff;rx_len];}
        self.spi_transfers=self.spi_transfers.saturating_add(1);
        let result=self.imu.spi_transfer(tx,rx_len);
        if pins.miso==Some(43) {result}else{vec![0xff;rx_len]}
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        let mut devices=Vec::new();
        for bus in 0..2 {
            devices.push((bus,0x40,Box::new(Ina3221::with_pins(self.power.clone(),(3,4))) as Box<dyn I2cDevice>));
            // Distinct attach keys retain both adapters; actual addresses are dynamic.
            for (sensor,key) in self.tof.iter().zip([0x29,0x2a]) {devices.push((bus,key,Box::new(Vl53l3cx::with_pins(sensor.clone(),(3,4)))));}
        }
        devices
    }
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {
        match pin {21 if self.stamp_power=>self.stamp.from_bits(bits),39=>self.carrier.from_bits(bits),_=>{}}
        self.leds[0]=self.stamp.leds[0];self.leds[1..].copy_from_slice(&self.carrier.leds);
    }
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.leds,self.stamp.updates+self.carrier.updates))}
    fn input_levels(&self)->Vec<(u8,bool)> {let mut levels=vec![(0,true)];for (s,pin) in self.tof.iter().zip([6,8]) {levels.push((pin,s.lock().expect("StampFly ToF mutex poisoned").irq_high()));}levels}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"button"|"btnA"|"boot"|"g0"=>Some(0),_=>None}}
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {
        if !accel_g.iter().chain(&gyro_dps).all(|v|v.is_finite()) {return false;}
        let mut sample=self.sample.lock().expect("StampFly IMU sample mutex poisoned");
        *sample=Bmi270Sample{accel_g,gyro_dps,version:sample.version.wrapping_add(1)};true
    }
    fn advance_to(&mut self,cycle:VirtualCycle) {
        let cycle=self.cycle.max(cycle);
        for (i,sensor) in self.tof.iter().enumerate() {
            let mut state=sensor.lock().expect("StampFly ToF mutex poisoned");let pin=[6,8][i];
            // Register writes can release IRQ synchronously at the prior bus horizon.
            let level=state.irq_high();if level!=self.tof_irq[i] {self.edges.push(BoardEdge{cycle:self.cycle,pin,level});self.tof_irq[i]=level;}
            while let Some(at)=state.next_deadline().filter(|at|*at<=cycle) {
                state.advance_to(at);let level=state.irq_high();
                if level!=self.tof_irq[i] {self.edges.push(BoardEdge{cycle:at,pin,level});self.tof_irq[i]=level;}
            }
            state.advance_to(cycle);
        }
        self.cycle=cycle;self.power.lock().expect("StampFly INA3221 mutex poisoned").advance_to(cycle);
    }
    fn next_deadline(&self)->Option<VirtualCycle> {self.tof.iter().filter_map(|s|s.lock().expect("StampFly ToF mutex poisoned").next_deadline()).chain(self.power.lock().expect("StampFly INA3221 mutex poisoned").next_deadline()).min()}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.advance_to(self.cycle);let mut edges=std::mem::take(&mut self.edges);edges.sort_by_key(|e|e.cycle);edges}
    fn uses_gpio_pwm(&self)->bool {true}
    fn gpio_pwm_at(&mut self,cycle:VirtualCycle,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {
        self.cycle=self.cycle.max(cycle);
        let motors=std::array::from_fn(|i|pwm(MOTORS[i].1).filter(|(hz,_)|hz.is_finite() && *hz>0.0).map_or((0.0,0),|(hz,duty)|(hz,duty.min(65535))));
        if motors!=self.motors {self.motors=motors;self.motor_version=self.motor_version.saturating_add(1);}
    }
    fn motion_snapshot(&self)->Option<String> {
        let motors=MOTORS.iter().zip(self.motors).map(|((id,pin),(hz,duty))|format!("{{\"id\":\"{id}\",\"gpio\":{pin},\"duty\":{:.8},\"frequencyHz\":{hz}}}",f64::from(duty)/65535.0)).collect::<Vec<_>>().join(",");
        Some(format!("{{\"type\":\"stampfly-motors\",\"version\":{},\"cycle\":{},\"clockHz\":{HZ},\"motors\":[{motors}],\"model\":\"latched-pwm\",\"flightSimulated\":false}}",self.motor_version.min((1<<53)-1),self.cycle.min((1<<53)-1)))
    }
    fn flight_motor_pwm(&self)->Option<[(f64,u32);4]> {Some(self.motors)}
    fn set_flight_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3],bottom_mm:Option<f64>,front_mm:Option<f64>)->bool {
        if !accel_g.iter().chain(&gyro_dps).all(|v|v.is_finite()) || [bottom_mm,front_mm].iter().flatten().any(|v|!v.is_finite() || !(0.0..=4000.0).contains(v)) {return false;}
        self.set_imu_sample(accel_g,gyro_dps);
        // Official tof.hpp: bottom XSHUT G7, front G9. These are external
        // optical targets, still consumed by scheduled raw histogram conversion.
        for (sensor,distance) in self.tof.iter().zip([bottom_mm,front_mm]) {sensor.lock().expect("StampFly ToF mutex poisoned").set_distance_mm(distance);}
        true
    }
    fn report(&self)->String {format!("[{}] BMI270 SPI={} RGB updates={} INA3221 conversions={} motor_updates={}; ideal IMU/battery inputs; VL53L3CX=raw-histogram-subset,no-target; BMP280/BMM150/PMW3901/flight/radio-controller unimplemented",self.name(),self.spi_transfers,self.stamp.updates+self.carrier.updates,self.power.lock().expect("StampFly INA3221 mutex poisoned").conversions,self.motor_version)}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flight_environment_is_cold_bounded_and_never_writes_motors() {
        let mut b=StampFly::new();let initial=b.flight_motor_pwm().unwrap();
        assert!(b.set_flight_sample([0.2,0.1,1.0],[2.0,1.0,-3.0],Some(500.0),None));
        assert_eq!(b.sample.lock().unwrap().accel_g,[0.2,0.1,1.0]);assert_eq!(b.cycle,0);
        assert_eq!(b.flight_motor_pwm().unwrap(),initial);
        assert!(!b.set_flight_sample([0.0;3],[0.0;3],Some(f64::NAN),None));
        assert!(!b.set_flight_sample([0.0;3],[0.0;3],None,Some(4001.0)));
        assert!(!b.set_flight_sample([f64::INFINITY,0.0,0.0],[0.0;3],None,None));
        assert_eq!(b.sample.lock().unwrap().accel_g,[0.2,0.1,1.0]);
        let mut bare=esp_soc::board::NoBoard;assert!(bare.flight_motor_pwm().is_none());assert!(!bare.set_flight_sample([0.0;3],[0.0;3],None,None));
    }
    fn pins()->SpiPins {SpiPins{sclk:1<<44,mosi:1<<14,miso:Some(43),cs:1<<46}}
    #[test]
    fn imu_requires_physical_routes_and_input_is_finite() {
        let mut b=StampFly::new();assert_eq!(b.spi_transfer_pins(2,pins(),&[0x80,0,0],3),[0xff,0,0x24]);
        for p in [SpiPins{sclk:1<<12,..pins()},SpiPins{mosi:0,..pins()},SpiPins{cs:1<<12,..pins()},SpiPins{miso:None,..pins()}] {assert_eq!(b.spi_transfer_pins(2,p,&[0x80,0,0],3),[0xff;3]);}
        assert!(b.set_imu_sample([1.0,0.0,0.0],[0.0;3]));assert!(!b.set_imu_sample([f64::NAN,0.0,0.0],[0.0;3]));
        assert_eq!(b.named_pin("button"),Some(0));assert!(b.display().is_none());
    }
    #[test]
    fn three_leds_and_latched_motor_observation() {
        let mut b=StampFly::new();b.rmt_frame(21,&[true;24]);b.rmt_frame(39,&[false;48]);assert_eq!(b.leds().unwrap(),(&[[255;3],[0;3],[0;3]][..],2));
        b.gpio_pwm_at(240,&|pin|if pin==42 {Some((150000.0,32768))}else{None});
        assert_eq!(b.motors,[(0.0,0),(150000.0,32768),(0.0,0),(0.0,0)]);assert_eq!(b.motor_version,1);
        b.gpio_pwm_at(480,&|pin|if pin==42 {Some((150000.0,32768))}else{None});assert_eq!(b.motor_version,1);
        assert!(b.motion_snapshot().unwrap().contains("\"cycle\":480"));
        b.gpio_pwm_at(720,&|_|None);assert_eq!(b.motors,[(0.0,0);4]);assert_eq!(b.motor_version,2);
        assert_eq!(super::super::make_board("m5stack-k138").unwrap().name(),"stampfly");assert_eq!(super::super::make_board("m5stack-k138-v11").unwrap().name(),"stampfly-v11");
    }
    #[test]
    fn v11_module_led_has_s3a_supply_gate_but_carrier_leds_do_not() {
        let mut b=StampFly::v11();b.rmt_frame(21,&[true;24]);b.rmt_frame(39,&[true;48]);assert_eq!(b.leds,[ [0;3],[255;3],[255;3] ]);
        let mut gpio=esp_periph::Gpio::new();gpio.enable=1<<38;gpio.out=1<<38;
        let mut mux=esp_periph::RegRam::new();mux.write(4+4*38,1<<12);
        b.gpio_waveform_at(1,esp_soc::pins::ChipPins::S3.routes(&gpio,&mux));b.rmt_frame(21,&[true;24]);assert_eq!(b.leds[0],[255;3]);
        gpio.enable=0;b.gpio_waveform_at(2,esp_soc::pins::ChipPins::S3.routes(&gpio,&mux));assert_eq!(b.leds,[[0;3],[255;3],[255;3]]);
    }
    #[test]
    fn two_tof_routes_xshut_addresses_and_scheduled_irq() {
        let mut b=StampFly::new();let mut devices=b.i2c_devices();assert_eq!(devices.len(),6);assert!(devices.iter().all(|(_,_,d)|d.pins()==Some((3,4))));
        assert!(devices[1].2.start_address(0x29,0x29,false));assert!(devices[2].2.start_address(0x2a,0x29,false));
        let mut gpio=esp_periph::Gpio::new();gpio.enable=(1<<7)|(1<<9);gpio.out=1<<9;
        let mut mux=esp_periph::RegRam::new();for pin in [7,9] {mux.write(4+4*pin,1<<12);}
        b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&gpio,&mux));assert!(!devices[1].2.start(false));
        let write=|d:&mut dyn I2cDevice,reg:u16,data:&[u8]| {assert!(d.start(false));for byte in reg.to_be_bytes().into_iter().chain(data.iter().copied()) {assert!(d.write(byte));}};
        write(devices[2].2.as_mut(),1,&[0x2a]);assert!(devices[2].2.start_address(0x2a,0x2a,false));
        gpio.out|=1<<7;b.gpio_waveform_at(HZ/1000,esp_soc::pins::ChipPins::S3.routes(&gpio,&mux));b.advance_to(HZ/100);
        assert!(devices[1].2.start_address(0x29,0x29,false));assert!(!devices[2].2.start_address(0x2a,0x29,false));
        let d=devices[1].2.as_mut();write(d,0x85,&[0]);write(d,0x472,&[0,7,0,7,0x29,0xc9,0x0e,0x40,0x28,0]);
        write(d,0x496,&[3,0x6d,3,0x6f,7,0x29]);write(d,0x470,&[1]);write(d,0x85,&[1]);write(d,0x5e,&[0,0x68,9,0,0x83,7]);write(d,0x64,&[7,0x11,0x22,0x10,0x12,0x32]);write(d,0x82,&[0]);write(d,0x86,&[1,0x23]);
        let due=b.tof[0].lock().unwrap().next_deadline().unwrap();b.advance_to(due-1);assert!(b.take_edges().is_empty());
        b.advance_to(due);assert_eq!(b.take_edges(),[BoardEdge{cycle:due,pin:6,level:false}]);assert!(b.input_levels().contains(&(8,true)));
        write(d,0x86,&[1]);assert_eq!(b.take_edges(),[BoardEdge{cycle:due,pin:6,level:true}]);
        gpio.out&=!(1<<7);b.gpio_waveform_at(due,esp_soc::pins::ChipPins::S3.routes(&gpio,&mux));assert!(!devices[1].2.start(false));assert!(devices[2].2.start_address(0x2a,0x2a,false));
    }
}
