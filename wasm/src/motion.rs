//! Cold observation of firmware-driven board mechanics. Never sets a servo target.
use crate::*;

/// Copy a JSON motion snapshot; return its required length, or 0 if unsupported.
/// Insufficient capacity leaves the destination untouched. Observation does not advance time.
/// # Safety
/// `e` is a live exclusive emulator; `p` is writable for `capacity` bytes.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_motion_snapshot(e:*mut Emu,p:*mut u8,capacity:usize)->usize {
    let bus=(unsafe {&mut *e}).m.bus();
    let Some(json)=bus.board().motion_snapshot() else {return 0;};
    if json.len()<=capacity {unsafe {std::ptr::copy_nonoverlapping(json.as_ptr(),p,json.len());}}
    json.len()
}

/// Cold FL/FR/RL/RR routed PWM as eight f64 values, (duty, Hz) per motor.
/// Returns 0 on unsupported boards. Does not advance the guest or change outputs.
/// # Safety
/// `e` is live/exclusive; `p` is writable for eight aligned f64 values.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_flight_motors(e:*mut Emu,p:*mut f64)->u32 {
    let Some(motors)=(unsafe {&mut *e}).m.bus().board().flight_motor_pwm() else {return 0;};
    for (i,(hz,duty)) in motors.into_iter().enumerate() {unsafe {p.add(i*2).write(f64::from(duty)/65535.0);p.add(i*2+1).write(hz);}}
    1
}

/// Cold ideal sensor environment, raw BMI units g/deg/s; ToF in mm, -1=no target.
/// This never changes PWM, firmware memory, routing, sensor power or ready flags.
/// # Safety
/// `e` references a live exclusively borrowed emulator.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_flight_sample(e:*mut Emu,ax:f64,ay:f64,az:f64,gx:f64,gy:f64,gz:f64,bottom:f64,front:f64)->u32 {
    if [bottom,front].iter().any(|v|!v.is_finite() || (*v != -1.0 && !(0.0..=4000.0).contains(v))) {return 1;}
    let accepted=(unsafe {&mut *e}).m.bus().board().set_flight_sample([ax,ay,az],[gx,gy,gz],(bottom>=0.0).then_some(bottom),(front>=0.0).then_some(front));
    if accepted {0}else{1}
}
