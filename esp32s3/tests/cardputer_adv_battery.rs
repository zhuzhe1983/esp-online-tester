//! Cardputer-Adv battery divider through real S3 SENS one-shot conversions and host overrides.
use emu_core::{Bus,Core};
use esp_periph::{AnalogSource,sar_adc::{millivolts,Calibration}};
use esp_soc::SocBus as _;
type S3=esp32s3::bus::SocBus;

fn attach(b:&mut S3) {b.board=esp32s3::board::make_board("cardputer-adv").unwrap();b.attach_board_devices();}
fn sample(b:&mut S3,pin:u8)->u32 {
    // ESP-IDF v5.4.2 S3 sens_reg.h: SAR_ATTEN1 +0x14, SAR_MEAS1_CTRL2 +0x0c;
    // ADC1 channels0..9 are GPIO1..10, PAD_EN starts at bit19, START bit17, DONE bit16.
    let ch=u32::from(pin-1);let select=1<<(19+ch);
    b.write32(0x6000_8814,3<<(2*ch)).unwrap();
    b.write32(0x6000_880c,select).unwrap();assert_eq!(b.read32(0x6000_880c).unwrap()&(1<<16),0);
    b.write32(0x6000_880c,select|(1<<17)).unwrap();
    let result=b.read32(0x6000_880c).unwrap();assert_ne!(result&(1<<16),0);result&4095
}
fn pad_mv(b:&mut S3)->u32 {millivolts(sample(b,10),3,Calibration::S3Adc1)}

#[test]
fn nominal_battery_drives_only_gpio10_and_is_not_a_conversion() {
    let mut m=esp32s3::machine([0;6]);let b=&mut m.bus;
    assert_eq!(sample(b,10),0,"an unconnected bare board has no battery");
    let before=b.adc_observation(10).unwrap();attach(b);
    assert_eq!(b.adc_observation(10),Some(before),"attaching does not complete an ADC sample");
    assert!(pad_mv(b).abs_diff(1850)<=1,"nominal3.7V reaches the physical2:1 divider");
    assert_eq!(b.adc_observation(10).unwrap().generation,before.generation+1);
    assert_eq!(sample(b,9),0,"no source on neighbouring ADC1_CH8");
    assert_eq!(b.periph.rtc.analog.volts(11,0),0.0,"no source on ADC2_CH0");
}

#[test]
fn host_voltage_and_raw_inputs_survive_reattachment_and_reboot_including_zero() {
    let mut m=esp32s3::machine([0;6]);let b=&mut m.bus;
    b.analog_set(10,AnalogSource::Const(2.0135));attach(b);
    let measured=pad_mv(b);
    // The ADC curve targets integer2013mV; nearest-code ties can choose2012mV.
    // The divider doubles that quantization, not the original host voltage.
    assert_eq!(b.periph.rtc.analog.volts(10,0),2.0135);
    assert!((measured*2).abs_diff(4027)<=4,"host4.027V battery: pad={measured}mV");
    b.attach_board_devices();b.reboot([0;6]);
    assert!((pad_mv(b)*2).abs_diff(4027)<=4);
    for volts in [1.65,2.1,0.0] {
        b.analog_set(10,AnalogSource::Const(volts));b.attach_board_devices();b.reboot([0;6]);
        assert!(pad_mv(b).abs_diff((volts*1000.0) as u32)<=1,"host source {volts}V");
    }
    for raw in [1234,4095,0] {
        assert!(b.adc_set_raw(10,raw));b.attach_board_devices();b.reboot([0;6]);
        assert_eq!(sample(b,10),u32::from(raw),"explicit raw{raw} survives reset");
    }
}

#[test]
fn shared_host_stream_is_retained_across_reboot() {
    let mut m=esp32s3::machine([0;6]);let b=&mut m.bus;
    let stream=esp_periph::analog::AnalogStream::new(1,1.7,0).unwrap();
    b.analog_set(10,AnalogSource::Stream(stream.clone()));attach(b);
    assert!(pad_mv(b).abs_diff(1700)<=1);
    b.reboot([0;6]);stream.replace_current(2.0).unwrap();b.attach_board_devices();
    assert!(pad_mv(b).abs_diff(2000)<=1,"reboot must keep the same shared host source");
}

#[test]
fn guest_instructions_sample_the_battery_through_sens() {
    for jit in [false,true] {
        let mut m=esp32s3::machine([0;6]);attach(&mut m.bus);
        m.bus.analog_set(10,AnalogSource::Const(2.0135));
        // s32i a3,a2,0; s32i a5,a4,0; s32i a6,a4,0; l32i a7,a4,0; waiti0; j.
        // Guest writes attenuation and pulses START; the host does not convert for it.
        m.bus.load_bytes(0x4037_0000,&[0x32,0x62,0,0x52,0x64,0,0x62,0x64,0,0x72,0x24,0,0,0x70,0,6,0xff,0xff]).unwrap();
        let c=&mut m.cores[0];c.pc=0x4037_0000;c.ps=0;c.set_jit(jit);
        for (reg,value) in [(2,0x6000_8814),(3,3<<18),(4,0x6000_880c),(5,1<<28),(6,(1<<28)|(1<<17))] {c.set_ar(reg,value);}
        m.quantum=64;m.vq_max=1;m.bb_max=1;m.max_cycles=64;m.run(64);
        let sample=m.bus.adc_observation(10).unwrap();
        assert_eq!(sample.generation,1);assert_eq!(m.exceptions,0);
        assert_ne!(m.cores[0].get_ar(7)&(1<<16),0);assert_eq!(m.cores[0].get_ar(7)&4095,u32::from(sample.raw));
        assert!((millivolts(u32::from(sample.raw),3,Calibration::S3Adc1)*2).abs_diff(4027)<=4);
        if cfg!(all(target_arch="aarch64",any(target_os="macos",target_os="linux"))) {
            assert_eq!(m.cores[0].code_cache_stats().unwrap().2>0,jit,"exercise native generated code when requested");
        }
    }
}
