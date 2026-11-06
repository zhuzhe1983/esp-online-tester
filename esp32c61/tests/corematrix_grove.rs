use esp_periph::i2c::{I2cDevice,env4::Environment};
const HZ:u64=160_000_000;
const ENV:&str="sht40-bmp280-u001d";
const LIGHT:&str="bh1750fvi-u136";
type Devices=Vec<(u8,u8,Box<dyn I2cDevice>)>;
fn device(devices:&mut Devices,address:u8)->&mut dyn I2cDevice {devices.iter_mut().find(|(_,a,_)|*a==address).unwrap().2.as_mut()}
fn write(devices:&mut Devices,address:u8,bytes:&[u8]) {let d=device(devices,address);assert!(d.start(false));for &b in bytes {assert!(d.write(b));}d.stop();}
fn read(devices:&mut Devices,address:u8,n:usize)->Vec<u8> {let d=device(devices,address);assert!(d.start(true));let bytes=(0..n).map(|_|d.read()).collect();d.stop();bytes}
fn power(devices:&mut Devices,on:bool) {
    // IO5 output, push-pull; the other IOE pins remain unpowered.
    write(devices,0x4f,&[3,0x10]);write(devices,0x4f,&[0x13,0xef]);write(devices,0x4f,&[5,if on {0x10}else{0}]);
}
fn crc(bytes:&[u8])->u8 {let mut c=0xffu8;for b in bytes {c^=*b;for _ in 0..8 {c=if c&0x80!=0 {(c<<1)^0x31}else{c<<1};}}c}

#[test]
fn grove_requires_exact_attachment_and_supply_and_reuses_shared_i2c_pins() {
    let mut b=esp32c61::board::make_board("corematrix").unwrap();let mut ds=b.i2c_devices();
    assert!(!b.accessory_environment_at(0,"sht30-qmp6988","grove",true,Environment::default()));
    assert!(!b.accessory_illuminance_at(0,LIGHT,"internal-i2c",true,1000.0));
    for address in [0x44,0x76,0x23] {assert_eq!(device(&mut ds,address).pins(),Some((0,1)));assert!(!device(&mut ds,address).start(false));}
    assert!(b.accessory_environment_at(0,ENV,"grove",true,Environment::default()));
    assert!(b.accessory_illuminance_at(0,LIGHT,"grove",true,1000.0));
    assert!(!device(&mut ds,0x76).start(false));power(&mut ds,true);b.advance_to(HZ/100);
    write(&mut ds,0x76,&[0xd0]);assert_eq!(read(&mut ds,0x76,1),[0x58]);
    write(&mut ds,0x44,&[0xfd]);assert!(!device(&mut ds,0x44).start(true));b.advance_to(HZ/50);
    let sample=read(&mut ds,0x44,6);assert_eq!(sample[2],crc(&sample[..2]));assert_eq!(sample[5],crc(&sample[3..5]));
    let temperature=-45.0+175.0*f64::from(u16::from_be_bytes([sample[0],sample[1]]))/65535.0;assert!((temperature-25.0).abs()<0.01);
    write(&mut ds,0x23,&[0x20]);b.advance_to(HZ/4);assert_eq!(read(&mut ds,0x23,2),[4,176]);
    // Disconnect one Hub leaf without changing the other address or PM1.
    assert!(b.accessory_environment_at(HZ/4,ENV,"grove",false,Environment::default()));
    assert!(!device(&mut ds,0x76).start(false));assert!(device(&mut ds,0x23).start(false));assert!(device(&mut ds,0x6e).start(false));
    power(&mut ds,false);assert!(!device(&mut ds,0x23).start(false));assert!(device(&mut ds,0x6e).start(false));
    power(&mut ds,true);assert_eq!(read(&mut ds,0x23,2),[0,0],"supply loss must discard the old conversion");
}

#[test]
fn grove_keeps_attachment_across_cpu_bus_reattach_but_resets_on_dcdc_loss() {
    let mut b=esp32c61::board::make_board("corematrix").unwrap();let mut ds=b.i2c_devices();
    b.accessory_illuminance_at(0,LIGHT,"grove",true,1000.0);power(&mut ds,true);write(&mut ds,0x23,&[0x20]);b.advance_to(HZ/5);
    drop(ds);let mut ds=b.i2c_devices();assert_eq!(read(&mut ds,0x23,2),[4,176]);
    write(&mut ds,0x6e,&[0x0c,0xa1]);assert!(!device(&mut ds,0x23).start(false));
    b.button_at(HZ/5,"power",true);assert!(b.take_power_event().is_some());
    assert!(!device(&mut ds,0x23).start(false));power(&mut ds,true);assert_eq!(read(&mut ds,0x23,2),[0,0]);
    b.accessory_illuminance_at(HZ/5,LIGHT,"grove",false,1000.0);assert!(!device(&mut ds,0x23).start(false));
    assert!(!b.accessory_environment_at(HZ/5,ENV,"grove",true,Environment{temperature_c:f64::NAN,..Environment::default()}));
}
